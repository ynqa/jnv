use std::sync::Arc;

use promkit::{pane::Pane, terminal::Terminal};
use tokio::{sync::Mutex, task::JoinHandle, time::Duration};

use super::{Context, State};
use crate::PaneIndex;

const LOADING_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct SpinnerSpawner {
    shared: Arc<Mutex<Context>>,
}

impl SpinnerSpawner {
    pub fn new(shared: Arc<Mutex<Context>>) -> Self {
        Self { shared }
    }

    pub fn spawn_spin_task(
        &self,
        spin_panes: Arc<Mutex<[Pane]>>,
        spin_terminal: Arc<Mutex<Terminal>>,
        spin_duration: Duration,
    ) -> JoinHandle<()> {
        let shared = self.shared.clone();
        let mut frame_index = 0;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(spin_duration);
            loop {
                interval.tick().await;

                {
                    let shared_state = shared.lock().await;
                    if shared_state.state == State::Idle {
                        continue;
                    }
                }

                frame_index = (frame_index + 1) % LOADING_FRAMES.len();

                let loading_pane = Pane::new(
                    vec![promkit::grapheme::StyledGraphemes::from(
                        LOADING_FRAMES[frame_index],
                    )],
                    0,
                );
                {
                    let mut panes = spin_panes.lock().await;
                    let mut terminal = spin_terminal.lock().await;
                    panes[PaneIndex::Processor as usize] = loading_pane;
                    // TODO: error handling
                    let _ = terminal.draw(&panes);
                }
            }
        })
    }
}
