use std::sync::Arc;

use async_trait::async_trait;
use crossterm::event::Event;
use promkit::{pane::Pane, terminal::Terminal};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::{PaneIndex, EMPTY_PANE, PANE_SIZE};
pub(crate) mod init;
pub(crate) mod monitor;
pub(crate) mod spinner;

#[derive(PartialEq)]
enum State {
    Idle,
    Loading,
    Processing,
}

#[async_trait]
pub trait ViewProvider: Send + 'static {
    async fn provide(&mut self, item: &'static str) -> anyhow::Result<impl Visualizer>;
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
        shared_terminal: Arc<Mutex<Terminal>>,
        shared_panes: Arc<Mutex<[Pane; PANE_SIZE]>>,
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
                let mut panes = shared_panes.lock().await;
                let mut shared_state = shared.lock().await;
                let mut terminal = shared_terminal.lock().await;
                panes[PaneIndex::ProcessorGuide as usize] =
                    maybe_guide.unwrap_or(EMPTY_PANE.to_owned());
                panes[PaneIndex::Processor as usize] = maybe_resp.unwrap_or(EMPTY_PANE.to_owned());
                shared_state.state = State::Idle;
                let _ = terminal.draw(&*panes);
            }
        })
    }

    pub async fn render_on_resize(
        &self,
        shared_visualizer: Arc<Mutex<impl Visualizer>>,
        area: (u16, u16),
        query: String,
        shared_terminal: Arc<Mutex<Terminal>>,
        shared_panes: Arc<Mutex<[Pane; PANE_SIZE]>>,
    ) {
        {
            let mut shared_state = self.shared.lock().await;
            shared_state.area = area;
            if let Some(task) = shared_state.current_task.take() {
                task.abort();
            }
        }

        let process_task =
            self.spawn_process_task(query, shared_visualizer, shared_terminal, shared_panes);

        {
            let mut shared_state = self.shared.lock().await;
            shared_state.current_task = Some(process_task);
        }
    }

    pub async fn render_result(
        &self,
        shared_visualizer: Arc<Mutex<impl Visualizer>>,
        query: String,
        shared_terminal: Arc<Mutex<Terminal>>,
        shared_panes: Arc<Mutex<[Pane; PANE_SIZE]>>,
    ) {
        {
            let mut shared_state = self.shared.lock().await;
            if let Some(task) = shared_state.current_task.take() {
                task.abort();
            }
        }

        let process_task =
            self.spawn_process_task(query, shared_visualizer, shared_terminal, shared_panes);

        {
            let mut shared_state = self.shared.lock().await;
            shared_state.current_task = Some(process_task);
        }
    }
}
