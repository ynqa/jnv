use std::sync::Arc;

use promkit_widgets::core::{grapheme::StyledGraphemes, render::SharedRenderer, Pane};
use tokio::{sync::Mutex, task::JoinHandle, time::Duration};

use crate::prompt::Index;

use super::{Context, State};

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
        shared_renderer: SharedRenderer<Index>,
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

                let pane = Pane::new(vec![StyledGraphemes::from(LOADING_FRAMES[frame_index])], 0);
                {
                    // TODO: error handling
                    let _ = shared_renderer
                        .update([(Index::Processor, pane)])
                        .render()
                        .await;
                }
            }
        })
    }
}
