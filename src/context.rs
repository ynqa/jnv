use std::{future::Future, sync::Arc};

use promkit_widgets::spinner;
use tokio::{sync::Mutex, task::JoinHandle};

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

pub(crate) struct Context {
    /// The current state of the processor, which can be Idle, Loading, or Processing.
    pub(crate) state: State,
    /// The current size of the terminal area.
    ///
    /// PERF NOTE: This currently lives with `state/current_task` in the same mutex
    /// for simplicity. If lock contention becomes visible, this can be split into
    /// a dedicated shared store (e.g. `Arc<RwLock<(u16, u16)>>`) to reduce lock
    /// granularity.
    pub(crate) area: (u16, u16),
    /// The current task being executed, if any.
    pub(crate) current_task: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct SharedContext(Arc<Mutex<Context>>);

impl SharedContext {
    pub fn new(area: (u16, u16)) -> Self {
        Self(Arc::new(Mutex::new(Context {
            state: State::Idle,
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
