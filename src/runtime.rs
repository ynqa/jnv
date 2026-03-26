use std::sync::Arc;

use promkit_widgets::{
    core::{crossterm::event::Event, grapheme::StyledGraphemes, render::SharedRenderer, Widget},
    jsonstream::{self, JsonStream},
    serde_json::{self, Value},
    status::{self, Severity},
};
use tokio::sync::Mutex;

use crate::{
    config::{JsonConfig, JsonViewerKeybinds},
    json,
    processor::{Context, State, Visualizer},
    prompt::Index,
};

pub struct JsonRuntime {
    state: jsonstream::State,
    json: Vec<serde_json::Value>,
    keybinds: JsonViewerKeybinds,
}

impl JsonRuntime {
    /// Initialize JsonRuntime while deserializing the input JSON string and rendering the initial view.
    pub async fn initialize(
        input: &'static str,
        config: JsonConfig,
        keybinds: JsonViewerKeybinds,
        shared_renderer: SharedRenderer<Index>,
        shared_ctx: Arc<Mutex<Context>>,
    ) -> anyhow::Result<Self> {
        // Set state to Loading to prevent overwriting by spinner frames in terminal.
        {
            let mut shared_ctx = shared_ctx.lock().await;
            if let Some(task) = shared_ctx.current_task.take() {
                task.abort();
            }
            shared_ctx.state = State::Loading;
        }

        let input_stream = json::deserialize(input, config.max_streams)?;
        let stream = JsonStream::new(input_stream.iter());
        let state = jsonstream::State {
            stream,
            config: config.stream,
        };

        // Set state to Idle to prevent overwriting by spinner frames in terminal.
        {
            let mut shared_ctx = shared_ctx.lock().await;
            shared_ctx.state = State::Idle;
        }

        {
            let shared_ctx = shared_ctx.lock().await;
            let area = shared_ctx.area;
            drop(shared_ctx);

            // TODO: error handling
            let _ = shared_renderer
                .update([(Index::Processor, state.create_graphemes(area.0, area.1))])
                .render()
                .await;
        }

        Ok(Self {
            json: input_stream,
            state,
            keybinds,
        })
    }

    /// Get the formatted content of current JSON stream
    pub fn formatted_content(&self) -> String {
        self.state.config.format_raw_json(self.state.stream.rows())
    }

    fn operate(&mut self, event: &Event) {
        match event {
            // Move up.
            event if self.keybinds.up.contains(event) => {
                self.state.stream.up();
            }

            // Move down.
            event if self.keybinds.down.contains(event) => {
                self.state.stream.down();
            }

            // Move to head
            event if self.keybinds.move_to_head.contains(event) => {
                self.state.stream.head();
            }

            // Move to tail
            event if self.keybinds.move_to_tail.contains(event) => {
                self.state.stream.tail();
            }

            // Toggle collapse/expand
            event if self.keybinds.toggle.contains(event) => {
                self.state.stream.toggle();
            }

            event if self.keybinds.expand.contains(event) => {
                self.state.stream.set_nodes_visibility(false);
            }

            event if self.keybinds.collapse.contains(event) => {
                self.state.stream.set_nodes_visibility(true);
            }

            _ => (),
        }
    }
}

#[async_trait::async_trait]
impl Visualizer for JsonRuntime {
    async fn create_pane_from_event(&mut self, area: (u16, u16), event: &Event) -> StyledGraphemes {
        self.operate(event);
        self.state.create_graphemes(area.0, area.1)
    }

    async fn create_panes_from_query(
        &mut self,
        area: (u16, u16),
        input: String,
    ) -> (Option<StyledGraphemes>, Option<StyledGraphemes>) {
        match json::run_jaq(&input, &self.json) {
            Ok(ret) => {
                let mut guide = None;
                if ret.iter().all(|val| *val == Value::Null) {
                    guide = Some(
                        status::State::new(
                            format!(
                                "jq returned 'null', which may indicate a typo or incorrect filter: `{input}`"
                            ),
                            Severity::Warning,
                        )
                        .create_graphemes(area.0, area.1),
                    );

                    self.state.stream = JsonStream::new(self.json.iter());
                } else {
                    self.state.stream = JsonStream::new(ret.iter());
                }

                (guide, Some(self.state.create_graphemes(area.0, area.1)))
            }
            Err(e) => {
                self.state.stream = JsonStream::new(self.json.iter());

                (
                    Some(
                        status::State::new(format!("jq failed: `{e}`"), Severity::Error)
                            .create_graphemes(area.0, area.1),
                    ),
                    Some(self.state.create_graphemes(area.0, area.1)),
                )
            }
        }
    }
}
