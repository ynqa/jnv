use std::sync::Arc;

use promkit::{pane::Pane, terminal::Terminal};
use tokio::sync::Mutex;

use super::{Context, State, ViewProvider, Visualizer};
use crate::{PaneIndex, PANE_SIZE};

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
        shared_terminal: Arc<Mutex<Terminal>>,
        shared_panes: Arc<Mutex<[Pane; PANE_SIZE]>>,
    ) -> anyhow::Result<impl Visualizer + 'a> {
        {
            let mut shared_state = self.shared.lock().await;
            if let Some(task) = shared_state.current_task.take() {
                task.abort();
            }
            shared_state.state = State::Loading;
        }

        let mut visualizer = provider.provide(item).await?;
        let pane = visualizer.create_init_pane(area).await;

        {
            let mut panes = shared_panes.lock().await;
            let mut shared_state = self.shared.lock().await;
            let mut terminal = shared_terminal.lock().await;
            panes[PaneIndex::Processor as usize] = pane;
            shared_state.state = State::Idle;
            // TODO: error handling
            let _ = terminal.draw(&*panes);
        }

        Ok(visualizer)
    }
}
