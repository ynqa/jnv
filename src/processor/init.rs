use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{Context, State, Visualizer};
use crate::{config::JsonViewerKeybinds, PaneIndex, Renderer};

#[async_trait]
pub trait ViewProvider {
    async fn provide(
        &mut self,
        item: &'static str,
        keybinds: JsonViewerKeybinds,
    ) -> anyhow::Result<impl Visualizer>;
}

pub struct ViewInitializer {
    shared: Arc<Mutex<Context>>,
}

impl ViewInitializer {
    pub fn new(shared: Arc<Mutex<Context>>) -> Self {
        Self { shared }
    }

    pub async fn initialize<'a, T: ViewProvider>(
        &self,
        provider: &'a mut T,
        item: &'static str,
        area: (u16, u16),
        shared_renderer: Arc<Mutex<Renderer>>,
        keybinds: JsonViewerKeybinds,
    ) -> anyhow::Result<impl Visualizer + 'a> {
        {
            let mut shared_state = self.shared.lock().await;
            if let Some(task) = shared_state.current_task.take() {
                task.abort();
            }
            shared_state.state = State::Loading;
        }

        let mut visualizer = provider.provide(item, keybinds).await?;
        let pane = visualizer.create_init_pane(area).await;

        // Set state to Idle to prevent overwriting by spinner frames in terminal.
        {
            let mut shared_state = self.shared.lock().await;
            shared_state.state = State::Idle;
        }
        {
            // TODO: error handling
            let _ = shared_renderer
                .lock()
                .await
                .update_and_draw([(PaneIndex::Processor, pane)]);
        }

        Ok(visualizer)
    }
}
