use jaq_core::{
    load::{Arena, File, Loader},
    Compiler, Ctx, RcIter,
};
use jaq_json::Val;

use promkit_widgets::{
    core::{
        crossterm::{
            event::Event,
            style::{Attribute, Attributes, Color, ContentStyle},
        },
        pane::Pane,
        PaneFactory,
    },
    jsonstream::{self, config::Config as JsonStreamConfig, jsonz, JsonStream},
    serde_json::{self, Deserializer, Value},
    text::{self, Text},
};

use crate::{
    config::JsonViewerKeybinds,
    processor::{ViewProvider, Visualizer},
    search::SearchProvider,
};

// #[derive(Clone)]
pub struct Json {
    state: jsonstream::State,
    json: &'static [serde_json::Value],
    keybinds: JsonViewerKeybinds,
}

impl Json {
    pub fn new(
        formatter: JsonStreamConfig,
        input_stream: &'static [serde_json::Value],
        keybinds: JsonViewerKeybinds,
    ) -> anyhow::Result<Self> {

        Ok(Self {
            json: input_stream,
            state: jsonstream::State {
                stream: JsonStream::new(input_stream.iter()),
                config: formatter,
            },
            keybinds,
        })
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
impl Visualizer for Json {
    async fn content_to_copy(&self) -> String {
        self.state.config.format_raw_json(self.state.stream.rows())
    }

    async fn create_init_pane(&mut self, area: (u16, u16)) -> Pane {
        self.state.create_pane(area.0, area.1)
    }

    async fn create_pane_from_event(&mut self, area: (u16, u16), event: &Event) -> Pane {
        self.operate(event);
        self.state.create_pane(area.0, area.1)
    }

    async fn create_panes_from_query(
        &mut self,
        area: (u16, u16),
        input: String,
    ) -> (Option<Pane>, Option<Pane>) {
        match run_jaq(&input, self.json) {
            Ok(ret) => {
                let mut guide = None;
                if ret.iter().all(|val| *val == Value::Null) {
                    guide = Some(text::State {
                        text: Text::from(format!("jq returned 'null', which may indicate a typo or incorrect filter: `{input}`")),
                        config: text::Config {
                            style: Some(ContentStyle {
                                foreground_color: Some(Color::Yellow),
                                attributes: Attributes::from(Attribute::Bold),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        ..Default::default()
                    }.create_pane(area.0, area.1));

                    self.state.stream = JsonStream::new(self.json.iter());
                } else {
                    self.state.stream = JsonStream::new(ret.iter());
                }

                (guide, Some(self.state.create_pane(area.0, area.1)))
            }
            Err(e) => {
                self.state.stream = JsonStream::new(self.json.iter());

                (
                    Some(
                        text::State {
                            text: Text::from(format!("jq failed: `{e}`")),
                            config: text::Config {
                                style: Some(ContentStyle {
                                    foreground_color: Some(Color::Red),
                                    attributes: Attributes::from(Attribute::Bold),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }
                        }
                        .create_pane(area.0, area.1),
                    ),
                    Some(self.state.create_pane(area.0, area.1)),
                )
            }
        }
    }
}

fn run_jaq(
    query: &str,
    json_stream: &'static [serde_json::Value],
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

#[derive(Clone)]
pub struct JsonStreamProvider {
    formatter: JsonStreamConfig,
    max_streams: Option<usize>,
}

impl JsonStreamProvider {
    pub fn new(formatter: JsonStreamConfig, max_streams: Option<usize>) -> Self {
        Self {
            formatter,
            max_streams,
        }
    }

    fn deserialize_json(&self, json_str: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let deserializer: serde_json::StreamDeserializer<'_, serde_json::de::StrRead<'_>, Value> =
            Deserializer::from_str(json_str).into_iter::<serde_json::Value>();
        let results = match self.max_streams {
            Some(l) => deserializer.take(l).collect::<Result<Vec<_>, _>>(),
            None => deserializer.collect::<Result<Vec<_>, _>>(),
        };
        results.map_err(anyhow::Error::from)
    }
}

#[async_trait::async_trait]
impl ViewProvider for JsonStreamProvider {
    async fn provide(
        &mut self,
        item: &'static str,
        keybinds: JsonViewerKeybinds,
    ) -> anyhow::Result<Json> {
        let stream = self.deserialize_json(item)?;
        let static_stream = Box::leak(stream.into_boxed_slice());
        Json::new(std::mem::take(&mut self.formatter), static_stream, keybinds)
    }
}

#[async_trait::async_trait]
impl SearchProvider for JsonStreamProvider {
    async fn provide(
        &mut self,
        item: &str,
    ) -> anyhow::Result<Box<dyn Iterator<Item = String> + Send>> {
        let stream = self.deserialize_json(item)?;
        let static_stream = Box::leak(stream.into_boxed_slice());
        Ok(Box::new(jsonz::get_all_paths(static_stream.iter())))
    }
}
