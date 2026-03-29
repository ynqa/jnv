use std::{future::Future, sync::Arc};

use promkit_widgets::spinner;
use tokio::{sync::Mutex, task::JoinHandle};

/// Represent the different sections of the UI, which can be used to manage focus and input handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Index {
    QueryEditor = 0,
    Guide = 1,
    Completion = 2,
    JsonViewer = 3,
}

#[derive(PartialEq)]
/// Represent the current state of the JSON viewer,
/// which can be used to control rendering behavior
/// and manage concurrent tasks like query processing and spinner animation.
pub enum State {
    /// The viewer is idle and ready for user interactions or query processing.
    Idle,
    /// The viewer is currently loading the JSON stream, which may involve deserialization
    Loading,
    /// The viewer is actively processing a jq query, which may involve executing the query
    /// and updating the view with the results.
    Processing,
}

pub struct Context {
    /// The current state of the processor, which can be Idle, Loading, or Processing.
    pub state: State,
    /// Current active index for user input handling.
    pub active_index: Index,
    /// The current size of the terminal area.
    ///
    /// PERF NOTE: This currently lives with `state/current_task` in the same mutex
    /// for simplicity. If lock contention becomes visible, this can be split into
    /// a dedicated shared store (e.g. `Arc<RwLock<(u16, u16)>>`) to reduce lock
    /// granularity.
    pub area: (u16, u16),
    /// The current task being executed, if any.
    pub current_task: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct SharedContext(Arc<Mutex<Context>>);

impl SharedContext {
    pub fn new(area: (u16, u16)) -> Self {
        Self(Arc::new(Mutex::new(Context {
            state: State::Idle,
            active_index: Index::QueryEditor,
            area,
            current_task: None,
        })))
    }

    pub async fn area(&self) -> (u16, u16) {
        let ctx = self.0.lock().await;
        ctx.area
    }

    pub async fn set_area(&self, area: (u16, u16)) {
        let mut ctx = self.0.lock().await;
        ctx.area = area;
    }

    pub async fn active_index(&self) -> Index {
        let ctx = self.0.lock().await;
        ctx.active_index
    }

    /// Set the active index, which controls which input field is currently focused.
    /// If the index is `Guide`, it will be ignored to prevent focus on the guide section.
    pub async fn set_active_index(&self, index: Index) {
        if index == Index::Guide {
            return;
        }
        let mut ctx = self.0.lock().await;
        ctx.active_index = index;
    }

    pub(crate) async fn lock(&self) -> tokio::sync::MutexGuard<'_, Context> {
        self.0.lock().await
    }
}

impl spinner::State for SharedContext {
    fn is_idle(&self) -> impl Future<Output = bool> + Send {
        let shared = self.0.clone();
        async move {
            let context = shared.lock().await;
            context.state == State::Idle
        }
    }
}
