use std::{collections::BTreeSet, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use promkit::{
    listbox::{self, Listbox},
    pane::Pane,
    PaneFactory,
};
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

#[async_trait]
pub trait SearchProvider: Clone + Send + 'static {
    async fn provide(
        &mut self,
        item: &str,
    ) -> anyhow::Result<Box<dyn Iterator<Item = String> + Send>>;
}

#[derive(Clone, Default)]
pub struct LoadState {
    pub loaded: bool,
    pub loaded_item_len: usize,
}

pub struct StartSearchResult {
    pub head_item: Option<String>,
    pub load_state: LoadState,
}

pub struct IncrementalSearcher {
    shared_set: Arc<Mutex<BTreeSet<String>>>,
    shared_load_state: Arc<RwLock<LoadState>>,
    state: listbox::State,
    search_result_chunk_size: usize,
    search_chunk_remaining: Vec<String>,
}

impl IncrementalSearcher {
    pub fn new(state: listbox::State, search_result_chunk_size: usize) -> Self {
        Self {
            shared_set: Default::default(),
            shared_load_state: Default::default(),
            state,
            search_result_chunk_size,
            search_chunk_remaining: Default::default(),
        }
    }

    pub fn spawn_load_task<T: SearchProvider>(
        &self,
        provider: &mut T,
        item: &'static str,
        chunk_size: usize,
    ) -> JoinHandle<anyhow::Result<()>> {
        let shared_set = self.shared_set.clone();
        let shared_load_state = self.shared_load_state.clone();
        let mut provider = provider.clone();
        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(chunk_size);
            let iter = provider.provide(item).await?;

            for v in iter {
                batch.push(v);

                if batch.len() >= chunk_size {
                    let mut set = shared_set.lock().await;
                    for item in batch.drain(..) {
                        set.insert(item);
                    }
                    let mut state = shared_load_state.write().await;
                    state.loaded_item_len += chunk_size;
                }
            }

            let remaining = batch.len();
            if !batch.is_empty() {
                let mut set = shared_set.lock().await;
                for item in batch {
                    set.insert(item);
                }
            }

            let mut state = shared_load_state.write().await;
            state.loaded = true;
            state.loaded_item_len += remaining;
            Ok(())
        })
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
            < self.state.lines.unwrap_or(1)
        {
            self.load_more();
        }
    }

    pub fn get_current_item(&self) -> String {
        self.state.listbox.get().to_string()
    }

    pub fn create_pane(&self, width: u16, height: u16) -> Pane {
        self.state.create_pane(width, height)
    }

    pub fn leave_search(&mut self) {
        self.state.listbox = Listbox::from_displayable(Vec::<String>::new());
        self.search_chunk_remaining = Vec::<String>::new();
    }

    pub fn start_search(&mut self, prefix: &str) -> anyhow::Result<StartSearchResult> {
        match (
            self.shared_load_state.try_read(),
            self.shared_set.try_lock(),
        ) {
            (Ok(state), Ok(set)) => {
                let mut items: Vec<_> = set
                    .iter()
                    .filter(|p| p.starts_with(prefix))
                    .cloned()
                    .collect();
                if items.is_empty() {
                    return Ok(StartSearchResult {
                        head_item: None,
                        load_state: state.clone(),
                    });
                }
                let used = items
                    .drain(..self.search_result_chunk_size.min(items.len()))
                    .collect::<Vec<_>>();
                self.search_chunk_remaining = items;
                self.state.listbox = Listbox::from_displayable(used);
                Ok(StartSearchResult {
                    head_item: Some(self.state.listbox.get().to_string()),
                    load_state: state.clone(),
                })
            }
            (Err(_), _) | (_, Err(_)) => Err(anyhow!(
                "Failed to acquire lock for ions. Please try again."
            )),
        }
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
