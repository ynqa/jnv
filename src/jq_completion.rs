use std::collections::BTreeSet;

use tokio::{
    sync::RwLock,
    task::{self, JoinHandle},
};

use crate::json;

/// Completion request from the caller.
#[derive(Clone, Debug)]
pub struct CompletionRequest<'a> {
    pub query: &'a str,
    pub cursor_char: usize,
    pub trigger: CompletionTrigger,
    /// Maximum number of items to return.
    /// `0` means no limit.
    pub max_items: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionTrigger {
    Manual,
    Character(char),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CandidateKind {
    Function,
    Path,
    Keyword,
    Symbol,
    Operator,
    Variable,
    Module,
    Binding,
}

#[derive(Clone, Debug)]
pub struct CompletionCandidate {
    pub id: u64,
    pub label: String,
    pub insert_text: String,
    pub replace: CharRange,
    pub kind: CandidateKind,
    pub sort_score: i32,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub needs_resolve: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CompletionResponse {
    pub items: Vec<CompletionCandidate>,
    pub is_incomplete: bool,
}

/// Progress information for background loading.
#[derive(Clone, Debug, Default)]
pub struct LoadProgress {
    pub is_complete: bool,
    pub loaded_path_count: usize,
}

/// Lightweight placeholder for builtin/filter index.
///
/// NOTE:
/// This is intentionally a shell for upcoming extraction into a standalone crate.
#[derive(Clone, Debug, Default)]
pub struct BuiltinIndex {
    pub seeds: BTreeSet<String>,
}

/// Lightweight placeholder for JSON path index.
#[derive(Clone, Debug, Default)]
pub struct PathIndex {
    pub paths: BTreeSet<String>,
}

#[derive(Debug)]
struct LazyPathIndexStore {
    path_index: PathIndex,
    progress: LoadProgress,
}

#[derive(Clone)]
pub struct CompletionEngine {
    builtins: std::sync::Arc<BuiltinIndex>,
    state: std::sync::Arc<RwLock<LazyPathIndexStore>>,
}

impl CompletionEngine {
    /// Collect applied suggestion strings with current lazy-load progress.
    pub async fn suggest_strings(
        &self,
        query: &str,
        cursor_char: usize,
    ) -> (Vec<String>, LoadProgress) {
        let store = self.state.read().await;
        let req = CompletionRequest {
            query,
            cursor_char,
            trigger: CompletionTrigger::Manual,
            max_items: 0,
        };
        let res = complete(req, &self.builtins, &store.path_index);
        let items = res
            .items
            .into_iter()
            .map(|item| apply(query, &item))
            .collect();

        (items, store.progress.clone())
    }
}

/// Spawn completion indexes and start lazy background loading of JSON paths.
pub fn spawn_initialize(
    input: &'static str,
    max_streams: Option<usize>,
    chunk_size: usize,
) -> (CompletionEngine, JoinHandle<()>) {
    let engine = CompletionEngine {
        builtins: std::sync::Arc::new(BuiltinIndex::default()),
        state: std::sync::Arc::new(RwLock::new(LazyPathIndexStore {
            path_index: PathIndex::default(),
            progress: LoadProgress::default(),
        })),
    };

    let state_for_loading = engine.state.clone();
    let loader_task = task::spawn(async move {
        let iter = match json::get_all_paths(input, max_streams).await {
            Ok(iter) => iter,
            Err(_) => {
                let mut store = state_for_loading.write().await;
                store.progress.is_complete = true;
                return;
            }
        };

        let mut batch = Vec::with_capacity(chunk_size);
        for path in iter {
            batch.push(path);

            if batch.len() >= chunk_size {
                let loaded = batch.len();
                let mut store = state_for_loading.write().await;
                for item in batch.drain(..) {
                    store.path_index.paths.insert(item);
                }
                store.progress.loaded_path_count += loaded;
            }
        }

        let remaining = batch.len();
        let mut store = state_for_loading.write().await;
        for item in batch {
            store.path_index.paths.insert(item);
        }
        store.progress.loaded_path_count += remaining;
        store.progress.is_complete = true;
    });

    (engine, loader_task)
}

/// Core completion API (shell).
///
/// Current status:
/// - Returns no candidates
/// - Keeps input/return contracts stable for caller integration.
fn complete(
    req: CompletionRequest<'_>,
    _builtins: &BuiltinIndex,
    _path_index: &PathIndex,
) -> CompletionResponse {
    let _ = req;
    CompletionResponse::default()
}

/// Apply one completion candidate to query text.
fn apply(query: &str, item: &CompletionCandidate) -> String {
    replace_range(
        query,
        item.replace.start,
        item.replace.end,
        &item.insert_text,
    )
}

fn split_at_char(s: &str, char_idx: usize) -> (&str, &str) {
    if char_idx == 0 {
        return ("", s);
    }

    let mut target = s.len();
    for (count, (idx, _)) in s.char_indices().enumerate() {
        if count == char_idx {
            target = idx;
            break;
        }
    }

    if char_idx >= s.chars().count() {
        (s, "")
    } else {
        s.split_at(target)
    }
}

fn replace_range(query: &str, start_char: usize, end_char: usize, insert: &str) -> String {
    let (left, rest) = split_at_char(query, start_char);
    let (_, right) = split_at_char(rest, end_char.saturating_sub(start_char));
    format!("{left}{insert}{right}")
}

#[cfg(test)]
mod tests {
    use super::{apply, CandidateKind, CharRange, CompletionCandidate};

    #[test]
    fn apply_replaces_char_range() {
        let item = CompletionCandidate {
            id: 1,
            label: "map(".to_string(),
            insert_text: "map(".to_string(),
            replace: CharRange { start: 0, end: 4 },
            kind: CandidateKind::Function,
            sort_score: 0,
            detail: None,
            documentation: None,
            needs_resolve: false,
        };
        assert_eq!(apply("mapx", &item), "map(");
    }
}
