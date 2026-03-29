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
    config::EditorKeybinds,
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

pub struct CompletionNavigator {
    shared_suggestions: SharedSuggestionStore,
    state: listbox::State,
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

    pub fn up(&mut self) {
        self.state.listbox.backward();
    }

    pub fn down_with_load(&mut self) {
        self.state.listbox.forward();
        if self
            .state
            .listbox
            .len()
            .saturating_sub(self.state.listbox.position())
            < self.state.config.lines.unwrap_or(1)
        {
            self.load_more();
        }
    }

    pub fn get_current_item(&self) -> String {
        self.state.listbox.get().to_string()
    }

    pub fn create_graphemes(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
    }

    pub fn leave(&mut self) {
        self.state.listbox = Listbox::from(Vec::<String>::new());
        self.search_chunk_remaining = Vec::<String>::new();
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

    pub async fn start(&mut self, prefix: &str) -> (Option<String>, SuggestionLoadProgress) {
        let (items, progress) = self.shared_suggestions.collect_matches(prefix).await;
        let head_item = self.apply_search_items(items);
        (head_item, progress)
    }

    fn load_more(&mut self) {
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
    editor_keybinds: EditorKeybinds,
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
                                let (head_item, load_progress) = completion.start(&prefix).await;
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
                                if editor_keybinds.on_completion.down.contains(&event)
                                    || editor_keybinds.completion.contains(&event)
                                {
                                    completion.down_with_load();
                                    query_editor_action_tx
                                        .send(QueryEditorAction::ReplaceText(completion.get_current_item()))
                                        .await?;
                                } else if editor_keybinds.on_completion.up.contains(&event) {
                                    completion.up();
                                    query_editor_action_tx
                                        .send(QueryEditorAction::ReplaceText(completion.get_current_item()))
                                        .await?;
                                }
                            }
                            CompletionAction::Leave => {
                                completion.leave();
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
