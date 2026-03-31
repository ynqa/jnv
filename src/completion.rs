use std::sync::Arc;

use promkit_widgets::{
    core::{crossterm::event::Event, grapheme::StyledGraphemes, Widget},
    listbox::{self, Listbox},
};
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    config::CompletionKeybinds,
    context::{Index, SharedContext},
    guide::{GuideAction, GuideMessage},
    jq_completion,
    query_editor::QueryEditorAction,
};

/// Navigator for managing the state of suggestions
/// and interactions in the completion view.
pub struct CompletionNavigator {
    state: listbox::State,
    /// Number of suggestions to load in each chunk
    /// when the user scrolls near the end of the list.
    search_result_chunk_size: usize,
    /// Buffered suggestions that are not yet visible in the listbox.
    remaining_items: Vec<String>,
}

impl CompletionNavigator {
    pub fn new(
        state: listbox::State,
        search_result_chunk_size: usize,
    ) -> Self {
        Self {
            state,
            search_result_chunk_size,
            remaining_items: Default::default(),
        }
    }

    /// Get the currently selected item in listbox.
    fn get_current_item(&self) -> String {
        self.state.listbox.get().to_string()
    }

    /// Create graphemes for rendering the completion navigator.
    pub fn create_graphemes(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
    }

    /// Returns true when the cursor is close enough to the visible tail
    /// and preloading the next chunk is beneficial.
    fn is_near_visible_tail(&self) -> bool {
        self.state
            .listbox
            .len()
            .saturating_sub(self.state.listbox.position())
            < self.state.config.lines.unwrap_or(1)
    }

    fn move_down(&mut self) {
        // First, move the cursor down by one item.
        self.state.listbox.forward();

        // Then, check if we need to load more items
        // when the cursor is close to the end.
        if self.is_near_visible_tail() {
            self.append_next_chunk_if_needed();
        }
    }

    fn append_next_chunk_if_needed(&mut self) {
        if self.remaining_items.is_empty() {
            return;
        }
        let items = self.remaining_items.drain(
            ..self
                .search_result_chunk_size
                .min(self.remaining_items.len()),
        );
        for item in items {
            self.state.listbox.push_string(item);
        }
    }

    /// Handle a user input event to update the completion navigator's state accordingly.
    /// Returns `Some(String)` if the event triggers a selection change that should update the query editor,
    fn handle_user_event(
        &mut self,
        event: &Event,
        completion_keybinds: &CompletionKeybinds,
    ) -> Option<String> {
        if self.state.listbox.is_empty() {
            return None;
        }

        // Move up.
        if completion_keybinds.up.contains(event) {
            self.state.listbox.backward();
            return Some(self.get_current_item());
        }

        // Move down (and load more if near the end).
        if completion_keybinds.down.contains(event) {
            self.move_down();
            return Some(self.get_current_item());
        }

        None
    }

    async fn enter(
        &mut self,
        completion_engine: &jq_completion::CompletionEngine,
        query: &str,
        cursor_char: usize,
    ) -> (Option<String>, jq_completion::LoadProgress) {
        let (items, progress) = completion_engine.suggest_strings(query, cursor_char).await;
        let head_item = self.initialize_session_items(items);
        (head_item, progress)
    }

    /// Initialize a completion session with a new search result set.
    /// This method always resets previous session state first.
    fn initialize_session_items(&mut self, mut items: Vec<String>) -> Option<String> {
        self.clear_session_state();

        if items.is_empty() {
            return None;
        }

        let used = items
            .drain(..self.search_result_chunk_size.min(items.len()))
            .collect::<Vec<_>>();
        self.remaining_items = items;
        self.state.listbox = Listbox::from(used);
        Some(self.state.listbox.get().to_string())
    }

    /// Reset completion session state.
    /// This clears both visible list items and buffered remaining items.
    fn clear_session_state(&mut self) {
        self.state.listbox = Listbox::from(Vec::<String>::new());
        self.remaining_items.clear();
    }
}

pub enum CompletionAction {
    /// Triggered when the user enters completion with current query and cursor position.
    Enter { query: String, cursor_char: usize },
    /// Triggered when the user leaves the completion view.
    Leave,
    /// Triggered on user input events within the completion view, such as navigation keys.
    UserEvent(Event),
}

/// Spawn a background task to manage the completion navigator's state and interactions.
pub fn start_completion_task(
    mut action_rx: mpsc::Receiver<CompletionAction>,
    shared_ctx: SharedContext,
    completion_engine: jq_completion::CompletionEngine,
    shared_completion: Arc<RwLock<CompletionNavigator>>,
    shared_renderer: promkit_widgets::core::render::SharedRenderer<Index>,
    query_editor_action_tx: mpsc::Sender<QueryEditorAction>,
    guide_action_tx: mpsc::Sender<GuideAction>,
    completion_keybinds: CompletionKeybinds,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(action) = action_rx.recv() => {
                    let area = shared_ctx.area().await;
                    let completion_view = {
                        let mut completion = shared_completion.write().await;
                        match action {
                            CompletionAction::Enter { query, cursor_char } => {
                                let (head_item, load_progress) = completion
                                    .enter(&completion_engine, &query, cursor_char)
                                    .await;
                                match head_item {
                                    Some(head) => {
                                        let message = if load_progress.is_complete {
                                            GuideMessage::LoadedAllSuggestions(load_progress.loaded_path_count)
                                        } else {
                                            GuideMessage::LoadedPartiallySuggestions(load_progress.loaded_path_count)
                                        };
                                        guide_action_tx.send(GuideAction::Show(message)).await?;
                                        query_editor_action_tx
                                            .send(QueryEditorAction::ReplaceText(head))
                                            .await?;
                                    }
                                    None => {
                                        guide_action_tx
                                            .send(GuideAction::Show(GuideMessage::NoSuggestionFound(query)))
                                            .await?;
                                        shared_ctx.set_active_index(Index::QueryEditor).await;
                                        completion.clear_session_state();
                                    }
                                }
                            }
                            CompletionAction::UserEvent(event) => {
                                if let Some(text) = completion.handle_user_event(&event, &completion_keybinds) {
                                    query_editor_action_tx
                                        .send(QueryEditorAction::ReplaceText(text))
                                        .await?;
                                } else {
                                    shared_ctx.set_active_index(Index::QueryEditor).await;
                                    completion.clear_session_state();
                                    query_editor_action_tx
                                        .send(QueryEditorAction::UserEvent(event))
                                        .await?;
                                }
                            }
                            CompletionAction::Leave => {
                                completion.clear_session_state();
                            }
                        }
                        completion.create_graphemes(area.0, area.1)
                    };

                    shared_renderer
                        .update([(Index::Completion, completion_view)])
                        .render()
                        .await?;
                }
                else => break,
            }
        }
        Ok(())
    })
}
