use std::{future::Future, sync::Arc};

use promkit_widgets::spinner;
use tokio::sync::Mutex;

use super::{Context, State};

pub struct ContextMonitor {
    shared: Arc<Mutex<Context>>,
}

impl ContextMonitor {
    pub fn new(shared: Arc<Mutex<Context>>) -> Self {
        Self { shared }
    }

    pub async fn is_idle(&self) -> bool {
        let context = self.shared.lock().await;
        context.state == State::Idle
    }
}

impl spinner::State for ContextMonitor {
    fn is_idle(&self) -> impl Future<Output = bool> + Send {
        let shared = self.shared.clone();
        async move {
            let context = shared.lock().await;
            context.state == State::Idle
        }
    }
}
