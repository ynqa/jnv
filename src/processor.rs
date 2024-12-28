use std::sync::Arc;

use async_trait::async_trait;
use crossterm::event::Event;
use promkit::pane::Pane;
use tokio::{sync::Mutex, task::JoinHandle};

use crate::{PaneIndex, Renderer, EMPTY_PANE};
pub mod init;
pub use init::ViewProvider;
pub mod monitor;
pub mod spinner;

#[derive(PartialEq)]
enum State {
    Idle,
    Loading,
    Processing,
}

#[async_trait]
pub trait Visualizer: Send + Sync + 'static {
    async fn content_to_copy(&self) -> String;
    async fn create_init_pane(&mut self, area: (u16, u16)) -> Pane;
    async fn create_pane_from_event(&mut self, area: (u16, u16), event: &Event) -> Pane;
    async fn create_panes_from_query(
        &mut self,
        area: (u16, u16),
        query: String,
    ) -> (Option<Pane>, Option<Pane>);
}

pub struct Context {
    state: State,
    area: (u16, u16),
    current_task: Option<JoinHandle<()>>,
}

impl Context {
    pub fn new(area: (u16, u16)) -> Self {
        Self {
            state: State::Idle,
            area,
            current_task: None,
        }
    }
}

pub struct Processor {
    shared: Arc<Mutex<Context>>,
}

impl Processor {
    pub fn new(shared: Arc<Mutex<Context>>) -> Self {
        Self { shared }
    }

    fn spawn_process_task(
        &self,
        query: String,
        shared_visualizer: Arc<Mutex<impl Visualizer>>,
        shared_renderer: Arc<Mutex<Renderer>>,
    ) -> JoinHandle<()> {
        let shared = self.shared.clone();
        tokio::spawn(async move {
            {
                let mut shared_state = shared.lock().await;
                shared_state.state = State::Processing;
            }

            let (maybe_guide, maybe_resp) = {
                let shared_state = shared.lock().await;
                let area = shared_state.area;
                drop(shared_state);

                let mut visualizer = shared_visualizer.lock().await;
                visualizer.create_panes_from_query(area, query).await
            };

            {
                // TODO: error handling
                let _ = shared_renderer.lock().await.update_and_draw([
                    (
                        PaneIndex::ProcessorGuide,
                        maybe_guide.unwrap_or(EMPTY_PANE.to_owned()),
                    ),
                    (
                        PaneIndex::Processor,
                        maybe_resp.unwrap_or(EMPTY_PANE.to_owned()),
                    ),
                ]);
            }

            {
                let mut shared_state = shared.lock().await;
                shared_state.state = State::Idle;
            }
        })
    }

    pub async fn render_on_resize(
        &self,
        shared_visualizer: Arc<Mutex<impl Visualizer>>,
        area: (u16, u16),
        query: String,
        shared_renderer: Arc<Mutex<Renderer>>,
    ) {
        {
            let mut shared_state = self.shared.lock().await;
            shared_state.area = area;
            if let Some(task) = shared_state.current_task.take() {
                task.abort();
            }
        }

        let process_task = self.spawn_process_task(query, shared_visualizer, shared_renderer);

        {
            let mut shared_state = self.shared.lock().await;
            shared_state.current_task = Some(process_task);
        }
    }

    pub async fn render_result(
        &self,
        shared_visualizer: Arc<Mutex<impl Visualizer>>,
        query: String,
        shared_renderer: Arc<Mutex<Renderer>>,
    ) {
        {
            let mut shared_state = self.shared.lock().await;
            if let Some(task) = shared_state.current_task.take() {
                task.abort();
            }
        }

        let process_task = self.spawn_process_task(query, shared_visualizer, shared_renderer);

        {
            let mut shared_state = self.shared.lock().await;
            shared_state.current_task = Some(process_task);
        }
    }
}
