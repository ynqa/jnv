use crossterm::{
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    style::{Attribute, Attributes},
};
use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};
use promkit::{
    crossterm::style::Color,
    jsonstream::{self, JsonStream},
    jsonz::{self, format::RowFormatter},
    pane::Pane,
    serde_json::{self, Deserializer, Value},
    style::StyleBuilder,
    text, PaneFactory,
};

use crate::{
    processor::{ViewProvider, Visualizer},
    search::SearchProvider,
};

#[derive(Clone)]
pub struct Json {
    state: jsonstream::State,
    json: &'static [serde_json::Value],
}

impl Json {
    pub fn new(
        formatter: RowFormatter,
        input_stream: &'static [serde_json::Value],
    ) -> anyhow::Result<Self> {
        Ok(Self {
            json: input_stream,
            state: jsonstream::State {
                stream: JsonStream::new(input_stream.iter()),
                formatter,
                lines: Default::default(),
            },
        })
    }

    fn operate(&mut self, event: &Event) {
        match event {
            // Move up.
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => {
                self.state.stream.up();
            }

            // Move down.
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => {
                self.state.stream.down();
            }

            // Move to tail
            Event::Key(KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => {
                self.state.stream.tail();
            }

            // Move to head
            Event::Key(KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => {
                self.state.stream.head();
            }

            // Toggle collapse/expand
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => {
                self.state.stream.toggle();
            }

            Event::Key(KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => {
                self.state.stream.set_nodes_visibility(false);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => {
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
                        text: format!("jq returned 'null', which may indicate a typo or incorrect filter: `{}`", input),
                        style: StyleBuilder::new()
                            .fgc(Color::Yellow)
                            .attrs(Attributes::from(Attribute::Bold))
                            .build(),
                    }.create_pane(area.0, area.1));
                }

                self.state.stream = JsonStream::new(ret.iter());

                (guide, Some(self.state.create_pane(area.0, area.1)))
            }
            Err(e) => (
                Some(
                    text::State {
                        text: format!("jq failed: `{}`", e),
                        style: StyleBuilder::new()
                            .fgc(Color::Red)
                            .attrs(Attributes::from(Attribute::Bold))
                            .build(),
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

pub struct JsonProvider {
    formatter: RowFormatter,
}

impl JsonProvider {
    pub fn new(formatter: RowFormatter) -> Self {
        Self { formatter }
    }
}

#[async_trait::async_trait]
impl ViewProvider for JsonProvider {
    async fn provide(&mut self, item: &'static str) -> anyhow::Result<Json> {
        let deserializer = Deserializer::from_str(item).into_iter::<serde_json::Value>();
        let stream = deserializer.collect::<Result<Vec<_>, _>>()?;
        let static_stream = Box::leak(stream.into_boxed_slice());
        Json::new(std::mem::take(&mut self.formatter), static_stream)
    }
}

#[async_trait::async_trait]
impl SearchProvider for JsonProvider {
    async fn provide(item: &str) -> anyhow::Result<Box<dyn Iterator<Item = String> + Send>> {
        let deserializer = Deserializer::from_str(item).into_iter::<serde_json::Value>();
        let stream = deserializer.collect::<Result<Vec<_>, _>>()?;
        let static_stream = Box::leak(stream.into_boxed_slice());
        Ok(Box::new(jsonz::get_all_paths(static_stream.iter())))
    }
}
