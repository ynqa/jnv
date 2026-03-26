use std::sync::Arc;

use jaq_core::{
    load::{Arena, File, Loader},
    Compiler, Ctx, RcIter,
};
use jaq_json::Val;

use promkit_widgets::{
    core::{crossterm::event::Event, grapheme::StyledGraphemes, render::SharedRenderer, Widget},
    jsonstream::{self, jsonz, JsonStream},
    serde_json::{self, Deserializer, Value},
    status::{self, Severity},
};
use tokio::sync::Mutex;

use crate::{
    config::{JsonConfig, JsonViewerKeybinds},
    processor::{Context, State, Visualizer},
    prompt::Index,
};

/// Get all JSON paths from the input JSON string,
/// respecting the max_streams limit if provided.
pub async fn get_all_paths(
    json_str: &str,
    max_streams: Option<usize>,
) -> anyhow::Result<impl Iterator<Item = String>> {
    let stream = deserialize_json(json_str, max_streams)?;
    let paths = jsonz::get_all_paths(stream.iter()).collect::<Vec<_>>();
    Ok(paths.into_iter())
}

/// Deserialize JSON string into a vector of serde_json::Value.
/// If max_streams is given, only deserialize up to that many JSON values.
fn deserialize_json(
    json_str: &str,
    max_streams: Option<usize>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let deserializer: serde_json::StreamDeserializer<'_, serde_json::de::StrRead<'_>, Value> =
        Deserializer::from_str(json_str).into_iter::<serde_json::Value>();
    let results = match max_streams {
        Some(l) => deserializer.take(l).collect::<Result<Vec<_>, _>>(),
        None => deserializer.collect::<Result<Vec<_>, _>>(),
    };
    results.map_err(anyhow::Error::from)
}

pub struct JsonRuntime {
    state: jsonstream::State,
    json: Vec<serde_json::Value>,
    keybinds: JsonViewerKeybinds,
}

impl JsonRuntime {
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

        let input_stream = deserialize_json(input, config.max_streams)?;
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
        match run_jaq(&input, &self.json) {
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

fn run_jaq(
    query: &str,
    json_stream: &[serde_json::Value],
) -> anyhow::Result<Vec<serde_json::Value>> {
    let arena = Arena::default();
    let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
    let modules = loader
        .load(
            &arena,
            File {
                code: query,
                path: (),
            },
        )
        .map_err(|errs| anyhow::anyhow!("jq filter parsing failed: {errs:?}"))?;
    let filter = Compiler::default()
        .with_funs(jaq_std::funs().chain(jaq_json::funs()))
        .compile(modules)
        .map_err(|errs| anyhow::anyhow!("jq filter compilation failed: {errs:?}"))?;

    let mut ret = Vec::<serde_json::Value>::new();

    for input in json_stream {
        let inputs = RcIter::new(core::iter::empty());
        let out = filter.run((Ctx::new([], &inputs), Val::from(input.clone())));
        for item in out {
            match item {
                Ok(val) => ret.push(val.into()),
                Err(err) => return Err(anyhow::anyhow!("jq filter execution failed: {err}")),
            }
        }
    }

    Ok(ret)
}
