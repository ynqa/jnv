use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use gag::Gag;

use promkit::{
    crossterm::{
        event::Event,
        style::{Attribute, Attributes, Color, ContentStyle},
    },
    json::{self, JsonNode, JsonPathSegment, JsonStream},
    keymap::KeymapManager,
    listbox,
    serde_json::{self, Deserializer},
    snapshot::Snapshot,
    style::StyleBuilder,
    suggest::Suggest,
    text, text_editor, Prompt, PromptSignal, Renderer,
};

mod editing;
mod keymap;
mod render;
mod trie;
use trie::QueryTrie;

pub struct Jnv {
    input_json_stream: Vec<serde_json::Value>,
    expand_depth: Option<usize>,
    no_hint: bool,

    query_editor_renderer: text_editor::Renderer,
    hint_message_renderer: text::Renderer,
    suggest: Suggest,
    suggest_renderer: listbox::Renderer,
    json_renderer: json::Renderer,
    keymap: KeymapManager<self::render::Renderer>,
}

impl Jnv {
    pub fn try_new(
        input_json_str: String,
        expand_depth: Option<usize>,
        no_hint: bool,
        edit_mode: text_editor::Mode,
        indent: usize,
        suggestion_list_length: usize,
    ) -> Result<Self> {
        let stream = deserialize_json(&input_json_str)?;
        let all_kinds = JsonStream::new(stream.clone(), None).flatten_kinds();
        let suggestions = all_kinds
            .iter()
            .filter_map(|kind| kind.path())
            .map(|segments| {
                if segments.is_empty() {
                    ".".to_string()
                } else {
                    segments
                        .iter()
                        .map(|segment| match segment {
                            JsonPathSegment::Key(key) => {
                                if key.contains('.') || key.contains('-') || key.contains('@') {
                                    format!(".\"{}\"", key)
                                } else {
                                    format!(".{}", key)
                                }
                            }
                            JsonPathSegment::Index(index) => format!("[{}]", index),
                        })
                        .collect::<String>()
                }
            });

        Ok(Self {
            input_json_stream: stream.clone(),
            expand_depth,
            no_hint,
            query_editor_renderer: text_editor::Renderer {
                texteditor: Default::default(),
                history: Default::default(),
                prefix: String::from("❯❯ "),
                mask: Default::default(),
                prefix_style: StyleBuilder::new().fgc(Color::Blue).build(),
                active_char_style: StyleBuilder::new().bgc(Color::Magenta).build(),
                inactive_char_style: StyleBuilder::new().build(),
                edit_mode,
                lines: Default::default(),
            },
            hint_message_renderer: text::Renderer {
                text: Default::default(),
                style: StyleBuilder::new()
                    .fgc(Color::Green)
                    .attrs(Attributes::from(Attribute::Bold))
                    .build(),
            },
            suggest: Suggest::from_iter(suggestions),
            suggest_renderer: listbox::Renderer {
                listbox: listbox::Listbox::from_iter(Vec::<String>::new()),
                cursor: String::from("❯ "),
                active_item_style: StyleBuilder::new()
                    .fgc(Color::Grey)
                    .bgc(Color::Yellow)
                    .build(),
                inactive_item_style: StyleBuilder::new().fgc(Color::Grey).build(),
                lines: Some(suggestion_list_length),
            },
            keymap: KeymapManager::new("default", self::keymap::default)
                .register("on_suggest", self::keymap::on_suggest),
            json_renderer: json::Renderer {
                stream: JsonStream::new(stream, expand_depth),
                theme: json::Theme {
                    curly_brackets_style: StyleBuilder::new()
                        .attrs(Attributes::from(Attribute::Bold))
                        .build(),
                    square_brackets_style: StyleBuilder::new()
                        .attrs(Attributes::from(Attribute::Bold))
                        .build(),
                    key_style: StyleBuilder::new().fgc(Color::Cyan).build(),
                    string_value_style: StyleBuilder::new().fgc(Color::Green).build(),
                    number_value_style: StyleBuilder::new().build(),
                    boolean_value_style: StyleBuilder::new().build(),
                    null_value_style: StyleBuilder::new().fgc(Color::Grey).build(),
                    active_item_attribute: Attribute::Bold,
                    inactive_item_attribute: Attribute::Dim,
                    lines: Default::default(),
                    indent,
                },
            },
        })
    }

    fn update_hint_message(
        &self,
        renderer: &mut self::render::Renderer,
        text: String,
        style: ContentStyle,
    ) {
        if !self.no_hint {
            renderer
                .hint_message_snapshot
                .after_mut()
                .replace(text::Renderer { text, style })
        }
    }

    fn evaluate(
        &self,
        event: &Event,
        renderer: &mut Box<dyn Renderer + 'static>,
        trie: RefCell<QueryTrie>,
    ) -> promkit::Result<PromptSignal> {
        let renderer = self::render::Renderer::cast_mut(renderer.as_mut())?;
        let signal = match renderer.keymap.get() {
            Some(f) => f(event, renderer),
            None => Ok(PromptSignal::Quit),
        }?;
        let completed = renderer
            .query_editor_snapshot
            .after()
            .texteditor
            .text_without_cursor()
            .to_string();

        if completed
            != renderer
                .query_editor_snapshot
                .borrow_before()
                .texteditor
                .text_without_cursor()
                .to_string()
        {
            renderer.hint_message_snapshot.reset_after_to_init();

            // libjq writes to the console when an internal error occurs.
            //
            // e.g.
            // ```
            // let _ = j9::run(". | select(.number == invalid_no_quote)", "{}");
            // jq: error: invalid_no_quote/0 is not defined at <top-level>, line 1:
            //     . | select(.number == invalid_no_quote)
            // ```
            //
            // While errors themselves are not an issue,
            // they interfere with the console output handling mechanism
            // in promkit and qjq (e.g., causing line numbers to shift).
            // Therefore, we'll ignore console output produced inside j9::run.
            //
            // It's possible that this could be handled
            // within github.com/ynqa/j9, but for now,
            // we'll proceed with this workaround.
            //
            // For reference, the functionality of a quiet mode in libjq is
            // also being discussed at https://github.com/jqlang/jq/issues/1225.
            let ignore_err = Gag::stderr().unwrap();

            let mut flatten_ret = Vec::<String>::new();
            for v in &self.input_json_stream {
                let inner_ret: Vec<String> = match j9::run(&completed, &v.to_string()) {
                    Ok(ret) => ret,
                    Err(_e) => {
                        self.update_hint_message(
                            renderer,
                            format!("Failed to execute jq query '{}'", &completed),
                            StyleBuilder::new()
                                .fgc(Color::Red)
                                .attrs(Attributes::from(Attribute::Bold))
                                .build(),
                        );
                        if let Some(searched) = trie.borrow().prefix_search_value(&completed) {
                            renderer.json_snapshot.after_mut().stream =
                                JsonStream::new(searched.clone(), self.expand_depth);
                        }
                        return Ok(signal);
                    }
                };
                flatten_ret.extend(inner_ret);
            }
            drop(ignore_err);

            if flatten_ret.is_empty() {
                self.update_hint_message(
                    renderer,
                    format!(
                        "JSON query ('{}') was executed, but no results were returned.",
                        &completed
                    ),
                    StyleBuilder::new()
                        .fgc(Color::Red)
                        .attrs(Attributes::from(Attribute::Bold))
                        .build(),
                );
                if let Some(searched) = trie.borrow().prefix_search_value(&completed) {
                    renderer.json_snapshot.after_mut().stream =
                        JsonStream::new(searched.clone(), self.expand_depth);
                }
            } else {
                match deserialize_json(&flatten_ret.join("\n")) {
                    Ok(jsonl) => {
                        let stream = JsonStream::new(jsonl.clone(), self.expand_depth);

                        let is_null = stream
                            .roots()
                            .iter()
                            .all(|node| node == &JsonNode::Leaf(serde_json::Value::Null));
                        if is_null {
                            self.update_hint_message(
                                renderer,
                                format!("JSON query resulted in 'null', which may indicate a typo or incorrect query: '{}'", &completed),
                                StyleBuilder::new()
                                    .fgc(Color::Yellow)
                                    .attrs(Attributes::from(Attribute::Bold))
                                    .build(),
                            );
                            if let Some(searched) = trie.borrow().prefix_search_value(&completed) {
                                renderer.json_snapshot.after_mut().stream =
                                    JsonStream::new(searched.clone(), self.expand_depth);
                            }
                        } else {
                            // SUCCESS!
                            trie.borrow_mut().insert(&completed, jsonl);
                            renderer.json_snapshot.after_mut().stream = stream;
                        }
                    }
                    Err(e) => {
                        self.update_hint_message(
                            renderer,
                            format!("Failed to parse query result for viewing: {}", e),
                            StyleBuilder::new()
                                .fgc(Color::Red)
                                .attrs(Attributes::from(Attribute::Bold))
                                .build(),
                        );
                        if let Some(searched) = trie.borrow().prefix_search_value(&completed) {
                            renderer.json_snapshot.after_mut().stream =
                                JsonStream::new(searched.clone(), self.expand_depth);
                        }
                    }
                }
                // flatten_ret.is_empty()
            }
            // before != completed
        }
        Ok(signal)
    }

    pub fn prompt(self) -> Result<Prompt<String>> {
        let rc_self = Rc::new(RefCell::new(self));
        let rc_self_clone = rc_self.clone();

        let keymap_clone = rc_self_clone.borrow().keymap.clone();
        let query_editor_renderer_clone = rc_self_clone.borrow().query_editor_renderer.clone();
        let hint_message_renderer_clone = rc_self_clone.borrow().hint_message_renderer.clone();
        let suggest_clone = rc_self_clone.borrow().suggest.clone();
        let suggest_renderer_clone = rc_self_clone.borrow().suggest_renderer.clone();
        let json_renderer_clone = rc_self_clone.borrow().json_renderer.clone();
        Ok(Prompt::try_new(
            Box::new(self::render::Renderer {
                keymap: keymap_clone,
                query_editor_snapshot: Snapshot::<text_editor::Renderer>::new(
                    query_editor_renderer_clone,
                ),
                hint_message_snapshot: Snapshot::<text::Renderer>::new(hint_message_renderer_clone),
                suggest: suggest_clone,
                suggest_snapshot: Snapshot::<listbox::Renderer>::new(suggest_renderer_clone),
                json_snapshot: Snapshot::<json::Renderer>::new(json_renderer_clone),
            }),
            Box::new(
                move |event: &Event,
                      renderer: &mut Box<dyn Renderer + 'static>|
                      -> promkit::Result<PromptSignal> {
                    let trie = RefCell::new(QueryTrie::default());
                    rc_self_clone.borrow().evaluate(event, renderer, trie)
                },
            ),
            |renderer: &(dyn Renderer + '_)| -> promkit::Result<String> {
                Ok(self::render::Renderer::cast(renderer)?
                    .query_editor_snapshot
                    .after()
                    .texteditor
                    .text_without_cursor()
                    .to_string())
            },
        )?)
    }
}

fn deserialize_json(json_str: &str) -> Result<Vec<serde_json::Value>> {
    Deserializer::from_str(json_str)
        .into_iter::<serde_json::Value>()
        .map(|res| res.map_err(anyhow::Error::new))
        .collect::<Result<Vec<serde_json::Value>>>()
}
