use std::{collections::BTreeSet, sync::Arc};

use promkit_widgets::{
    core::{
        crossterm::{event::Event, terminal},
        grapheme::StyledGraphemes,
        Widget,
    },
    listbox::{self, Listbox},
};
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    task::{self, JoinHandle},
};

use crate::{
    config::CompletionKeybinds,
    guide::{GuideAction, GuideMessage},
    json,
    prompt::Index,
    query_editor::QueryEditorAction,
};

/// Progress information for loading suggestions
#[derive(Clone, Default)]
pub struct SuggestionLoadProgress {
    pub is_complete: bool,
    pub loaded_path_count: usize,
}

/// Store for suggestions with thread-safe access
struct SuggestionStore {
    /// Set of all paths extracted from JSON input
    paths: BTreeSet<String>,
    progress: SuggestionLoadProgress,
}

#[derive(Clone)]
pub struct SharedSuggestionStore(Arc<Mutex<SuggestionStore>>);

impl SharedSuggestionStore {
    /// Collect suggestions that start with the given prefix
    pub async fn collect_matches(&self, prefix: &str) -> (Vec<String>, SuggestionLoadProgress) {
        let store = self.0.lock().await;
        let items = store
            .paths
            .iter()
            .filter(|p| p.starts_with(prefix))
            .cloned()
            .collect::<Vec<_>>();
        (items, store.progress.clone())
    }
}

/// Initialize shared suggestion store by loading paths from JSON input
pub async fn initialize(
    item: &'static str,
    max_streams: Option<usize>,
    chunk_size: usize,
) -> anyhow::Result<SharedSuggestionStore> {
    let shared = SharedSuggestionStore(Arc::new(Mutex::new(SuggestionStore {
        paths: BTreeSet::new(),
        progress: SuggestionLoadProgress::default(),
    })));

    let shared_for_loading = shared.clone();
    task::spawn(async move {
        // Load paths in a streaming manner and update the shared store incrementally
        let iter = match json::get_all_paths(item, max_streams).await {
            Ok(iter) => iter,
            Err(_) => {
                let mut store = shared_for_loading.0.lock().await;
                store.progress.is_complete = true;
                return;
            }
        };

        // Process paths in chunks to avoid holding the lock for too long
        let mut batch = Vec::with_capacity(chunk_size);
        for path in iter {
            batch.push(path);

            if batch.len() >= chunk_size {
                let loaded = batch.len();
                let mut store = shared_for_loading.0.lock().await;
                for item in batch.drain(..) {
                    store.paths.insert(item);
                }
                store.progress.loaded_path_count += loaded;
            }
        }

        // Insert any remaining paths after the loop
        let remaining = batch.len();
        let mut store = shared_for_loading.0.lock().await;
        for item in batch {
            store.paths.insert(item);
        }

        // Mark loading as complete and update progress
        store.progress.loaded_path_count += remaining;
        store.progress.is_complete = true;
    });

    Ok(shared)
}

/// Navigator for managing the state of suggestions
/// and interactions in the completion pane.
pub struct CompletionNavigator {
    shared_suggestions: SharedSuggestionStore,
    state: listbox::State,
    /// Number of suggestions to load in each chunk
    /// when the user scrolls near the end of the list.
    search_result_chunk_size: usize,
    search_chunk_remaining: Vec<String>,
}

impl CompletionNavigator {
    pub fn new(
        shared_suggestions: SharedSuggestionStore,
        state: listbox::State,
        search_result_chunk_size: usize,
    ) -> Self {
        Self {
            shared_suggestions,
            state,
            search_result_chunk_size,
            search_chunk_remaining: Default::default(),
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

    fn move_down(&mut self) {
        // First, move the cursor down by one item.
        self.state.listbox.forward();

        // Then, check if we need to load more items
        // when the cursor is close to the end.
        if self
            .state
            .listbox
            .len()
            .saturating_sub(self.state.listbox.position())
            < self.state.config.lines.unwrap_or(1)
        {
            self.append_next_chunk_if_needed();
        }
    }

    fn append_next_chunk_if_needed(&mut self) {
        if self.search_chunk_remaining.is_empty() {
            return;
        }
        let items = self.search_chunk_remaining.drain(
            ..self
                .search_result_chunk_size
                .min(self.search_chunk_remaining.len()),
        );
        for item in items {
            self.state.listbox.push_string(item);
        }
    }

    /// Handle a user input event to update the completion navigator's state accordingly.
    fn handle_user_event(
        &mut self,
        event: &Event,
        completion_keybinds: &CompletionKeybinds,
    ) -> Option<String> {
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

    fn apply_search_items(&mut self, mut items: Vec<String>) -> Option<String> {
        if items.is_empty() {
            return None;
        }
        let used = items
            .drain(..self.search_result_chunk_size.min(items.len()))
            .collect::<Vec<_>>();
        self.search_chunk_remaining = items;
        self.state.listbox = Listbox::from(used);
        Some(self.state.listbox.get().to_string())
    }

    async fn enter(&mut self, prefix: &str) -> (Option<String>, SuggestionLoadProgress) {
        let (items, progress) = self.shared_suggestions.collect_matches(prefix).await;
        let head_item = self.apply_search_items(items);
        (head_item, progress)
    }

    fn reset_session(&mut self) {
        self.state.listbox = Listbox::from(Vec::<String>::new());
        self.search_chunk_remaining = Vec::<String>::new();
    }
}

pub enum CompletionAction {
    Enter { prefix: String },
    Leave,
    UserEvent(Event),
}

pub fn start_completion_task(
    mut action_rx: mpsc::Receiver<CompletionAction>,
    shared_renderer: promkit_widgets::core::render::SharedRenderer<Index>,
    shared_completion: Arc<RwLock<CompletionNavigator>>,
    query_editor_action_tx: mpsc::Sender<QueryEditorAction>,
    guide_action_tx: mpsc::Sender<GuideAction>,
    completion_keybinds: CompletionKeybinds,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(action) = action_rx.recv() => {
                    let size = terminal::size()?;
                    let completion_pane = {
                        let mut completion = shared_completion.write().await;
                        match action {
                            CompletionAction::Enter { prefix } => {
                                let (head_item, load_progress) = completion.enter(&prefix).await;
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
                                            .send(GuideAction::Show(GuideMessage::NoSuggestionFound(prefix)))
                                            .await?;
                                    }
                                }
                            }
                            CompletionAction::UserEvent(event) => {
                                if let Some(text) = completion.handle_user_event(&event, &completion_keybinds) {
                                    query_editor_action_tx
                                        .send(QueryEditorAction::ReplaceText(text))
                                        .await?;
                                }
                            }
                            CompletionAction::Leave => {
                                completion.reset_session();
                            }
                        }
                        completion.create_graphemes(size.0, size.1)
                    };

                    shared_renderer
                        .update([(Index::Search, completion_pane)])
                        .render()
                        .await?;
                }
                else => break,
            }
        }
        Ok(())
    })
}
