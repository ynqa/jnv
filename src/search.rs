use std::{collections::BTreeSet, sync::Arc};

use promkit_widgets::{
    core::{grapheme::StyledGraphemes, Widget},
    listbox::{self, Listbox},
};
use tokio::{sync::Mutex, task};

use crate::json;

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

    /// Get the current load progress of suggestions
    pub async fn load_progress(&self) -> SuggestionLoadProgress {
        let store = self.0.lock().await;
        store.progress.clone()
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

pub struct IncrementalSearcher {
    shared_suggestions: SharedSuggestionStore,
    state: listbox::State,
    search_result_chunk_size: usize,
    search_chunk_remaining: Vec<String>,
}

impl IncrementalSearcher {
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

    pub fn create_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
    }

    pub fn leave_search(&mut self) {
        self.state.listbox = Listbox::from(Vec::<String>::new());
        self.search_chunk_remaining = Vec::<String>::new();
    }

    pub fn apply_search_items(&mut self, mut items: Vec<String>) -> Option<String> {
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

    pub async fn load_progress(&self) -> SuggestionLoadProgress {
        self.shared_suggestions.load_progress().await
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
