use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};

use promkit_widgets::{
    core::{
        crossterm::{
            event::Event,
            style::{Attribute, Attributes, Color, ContentStyle},
        },
        pane::Pane,
        PaneFactory,
    },
    jsonstream::{self, format::RowFormatter, jsonz, JsonStream},
    serde_json::{self, Deserializer, Value},
    text::{self, Text},
};

use crate::{
    config::{event::Matcher, JsonViewerKeybinds},
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
        formatter: RowFormatter,
        input_stream: &'static [serde_json::Value],
        keybinds: JsonViewerKeybinds,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            json: input_stream,
            state: jsonstream::State {
                stream: JsonStream::new(input_stream.iter()),
                formatter,
                lines: Default::default(),
            },
            keybinds,
        })
    }

    fn operate(&mut self, event: &Event) {
        match event {
            // Move up.
            event if self.keybinds.up.matches(event) => {
                self.state.stream.up();
            }

            // Move down.
            event if self.keybinds.down.matches(event) => {
                self.state.stream.down();
            }

            // Move to head
            event if self.keybinds.move_to_head.matches(event) => {
                self.state.stream.head();
            }

            // Move to tail
            event if self.keybinds.move_to_tail.matches(event) => {
                self.state.stream.tail();
            }

            // Toggle collapse/expand
            event if self.keybinds.toggle.matches(event) => {
                self.state.stream.toggle();
            }

            event if self.keybinds.expand.matches(event) => {
                self.state.stream.set_nodes_visibility(false);
            }

            event if self.keybinds.collapse.matches(event) => {
                self.state.stream.set_nodes_visibility(true);
            }

            _ => (),
        }
    }
}

#[async_trait::async_trait]
impl Visualizer for Json {
    async fn content_to_copy(&self) -> String {
        self.state
            .formatter
            .format_raw_json(self.state.stream.rows())
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
                        text: Text::from(format!("jq returned 'null', which may indicate a typo or incorrect filter: `{}`", input)),
                        style: ContentStyle {
                            foreground_color: Some(Color::Yellow),
                            attributes: Attributes::from(Attribute::Bold),
                            ..Default::default()
                        },
                        ..Default::default()
                    }.create_pane(area.0, area.1));
                }

                self.state.stream = JsonStream::new(ret.iter());

                (guide, Some(self.state.create_pane(area.0, area.1)))
            }
            Err(e) => (
                Some(
                    text::State {
                        text: Text::from(format!("jq failed: `{}`", e)),
                        style: ContentStyle {
                            foreground_color: Some(Color::Red),
                            attributes: Attributes::from(Attribute::Bold),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                    .create_pane(area.0, area.1),
                ),
                None,
            ),
        }
    }
}

fn run_jaq(
    query: &str,
    json_stream: &'static [serde_json::Value],
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut ret = Vec::<serde_json::Value>::new();

    for input in json_stream {
        let mut ctx = ParseCtx::new(Vec::new());
        ctx.insert_natives(jaq_core::core());
        ctx.insert_defs(jaq_std::std());

        let (f, errs) = jaq_parse::parse(query, jaq_parse::main());
        if !errs.is_empty() {
            let error_message = errs
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow::anyhow!(error_message));
        }

        let f = ctx.compile(f.unwrap());
        let inputs = RcIter::new(core::iter::empty());
        let mut out = f.run((Ctx::new([], &inputs), Val::from(input.clone())));

        while let Some(Ok(val)) = out.next() {
            ret.push(val.into());
        }
    }

    Ok(ret)
}

#[derive(Clone)]
pub struct JsonStreamProvider {
    formatter: RowFormatter,
    max_streams: Option<usize>,
}

impl JsonStreamProvider {
    pub fn new(formatter: RowFormatter, max_streams: Option<usize>) -> Self {
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
