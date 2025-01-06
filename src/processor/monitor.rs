use std::sync::Arc;

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
