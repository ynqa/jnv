use std::{cell::RefCell, collections::HashSet};

use anyhow::Result;

use gag::Gag;
use promkit::{
    crossterm::{
        event::Event,
        style::{Attribute, Attributes, Color, ContentStyle},
    },
    json::{self, JsonNode, JsonPathSegment, JsonStream},
    listbox,
    pane::Pane,
    serde_json::{self, Deserializer},
    snapshot::Snapshot,
    style::StyleBuilder,
    suggest::Suggest,
    switch::ActiveKeySwitcher,
    text, text_editor, PaneFactory, Prompt, PromptSignal,
};

mod keymap;
mod trie;
use trie::QueryTrie;

fn deserialize_json(json_str: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    Deserializer::from_str(json_str)
        .into_iter::<serde_json::Value>()
        .map(|res| res.map_err(anyhow::Error::from))
        .collect::<anyhow::Result<Vec<serde_json::Value>>>()
}

fn run_jq(query: &str, json_stream: &[serde_json::Value]) -> anyhow::Result<Vec<String>> {
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
    let mut jq_ret = Vec::<String>::new();
    for v in json_stream.iter() {
        let inner_ret: Vec<String> = match j9::run(&query, &v.to_string()) {
            Ok(ret) => ret,
            Err(e) => {
                return Err(anyhow::anyhow!(e));
            }
        };
        jq_ret.extend(inner_ret);
    }
    drop(ignore_err);
    Ok(jq_ret)
}

pub struct Jnv {
    keymap: RefCell<ActiveKeySwitcher<keymap::Keymap>>,
    query_editor_snapshot: Snapshot<text_editor::State>,
    hint_message_snapshot: Snapshot<text::State>,
    suggest: Suggest,
    suggest_state: listbox::State,
    json_state: json::State,
    trie: QueryTrie,
    input_json_stream: Vec<serde_json::Value>,
    expand_depth: Option<usize>,
    no_hint: bool,
}

impl Jnv {
    pub fn try_new(
        input_json_str: String,
        expand_depth: Option<usize>,
        no_hint: bool,
        edit_mode: text_editor::Mode,
        indent: usize,
        suggestion_list_length: usize,
    ) -> Result<Prompt<Self>> {
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
                        .enumerate()
                        .map(|(i, segment)| match segment {
                            JsonPathSegment::Key(key) => {
                                if key.contains('.') || key.contains('-') || key.contains('@') {
                                    format!(".\"{}\"", key)
                                } else {
                                    format!(".{}", key)
                                }
                            }
                            JsonPathSegment::Index(index) => {
                                if i == 0 {
                                    format!(".[{}]", index)
                                } else {
                                    format!("[{}]", index)
                                }
                            }
                        })
                        .collect::<String>()
                }
            });

        Ok(Prompt {
            renderer: Self {
                keymap: RefCell::new(
                    ActiveKeySwitcher::new("default", self::keymap::default as keymap::Keymap)
                        .register("on_suggest", self::keymap::on_suggest),
                ),
                query_editor_snapshot: Snapshot::<text_editor::State>::new(text_editor::State {
                    texteditor: Default::default(),
                    history: Default::default(),
                    prefix: String::from("❯❯ "),
                    mask: Default::default(),
                    prefix_style: StyleBuilder::new().fgc(Color::Blue).build(),
                    active_char_style: StyleBuilder::new().bgc(Color::Magenta).build(),
                    inactive_char_style: StyleBuilder::new().build(),
                    edit_mode,
                    word_break_chars: HashSet::from(['.', '|', '(', ')', '[', ']']),
                    lines: Default::default(),
                }),
                hint_message_snapshot: Snapshot::<text::State>::new(text::State {
                    text: Default::default(),
                    style: StyleBuilder::new()
                        .fgc(Color::Green)
                        .attrs(Attributes::from(Attribute::Bold))
                        .build(),
                }),
                suggest: Suggest::from_iter(suggestions),
                suggest_state: listbox::State {
                    listbox: listbox::Listbox::from_iter(Vec::<String>::new()),
                    cursor: String::from("❯ "),
                    active_item_style: StyleBuilder::new()
                        .fgc(Color::Grey)
                        .bgc(Color::Yellow)
                        .build(),
                    inactive_item_style: StyleBuilder::new().fgc(Color::Grey).build(),
                    lines: Some(suggestion_list_length),
                },
                json_state: json::State {
                    stream: JsonStream::new(stream.clone(), expand_depth),
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
                trie: QueryTrie::default(),
                input_json_stream: stream,
                expand_depth,
                no_hint,
            },
        })
    }

    fn update_hint_message(&mut self, text: String, style: ContentStyle) {
        if !self.no_hint {
            self.hint_message_snapshot
                .after_mut()
                .replace(text::State { text, style })
        }
    }
}

impl promkit::Finalizer for Jnv {
    type Return = String;

    fn finalize(&self) -> anyhow::Result<Self::Return> {
        Ok(self
            .query_editor_snapshot
            .after()
            .texteditor
            .text_without_cursor()
            .to_string())
    }
}

impl promkit::Renderer for Jnv {
    fn create_panes(&self, width: u16, height: u16) -> Vec<Pane> {
        vec![
            self.query_editor_snapshot.create_pane(width, height),
            self.hint_message_snapshot.create_pane(width, height),
            self.suggest_state.create_pane(width, height),
            self.json_state.create_pane(width, height),
        ]
    }

    fn evaluate(&mut self, event: &Event) -> anyhow::Result<PromptSignal> {
        let keymap = *self.keymap.borrow_mut().get();
        let signal = keymap(event, self);
        let completed = self
            .query_editor_snapshot
            .after()
            .texteditor
            .text_without_cursor()
            .to_string();

        // Check if the query has changed
        if completed
            != self
                .query_editor_snapshot
                .borrow_before()
                .texteditor
                .text_without_cursor()
                .to_string()
        {
            self.hint_message_snapshot.reset_after_to_init();

            match run_jq(&completed, &self.input_json_stream) {
                Ok(ret) => {
                    if ret.is_empty() {
                        self.update_hint_message(
                            format!(
                                "JSON query ('{}') was executed, but no results were returned.",
                                &completed
                            ),
                            StyleBuilder::new()
                                .fgc(Color::Red)
                                .attrs(Attributes::from(Attribute::Bold))
                                .build(),
                        );
                        if let Some(searched) = self.trie.prefix_search_value(&completed) {
                            self.json_state.stream =
                                JsonStream::new(searched.clone(), self.expand_depth);
                        }
                    } else {
                        match deserialize_json(&ret.join("\n")) {
                            Ok(jsonl) => {
                                let stream = JsonStream::new(jsonl.clone(), self.expand_depth);

                                let is_null = stream
                                    .roots()
                                    .iter()
                                    .all(|node| node == &JsonNode::Leaf(serde_json::Value::Null));
                                if is_null {
                                    self.update_hint_message(
                                        format!("JSON query resulted in 'null', which may indicate a typo or incorrect query: '{}'", &completed),
                                        StyleBuilder::new()
                                            .fgc(Color::Yellow)
                                            .attrs(Attributes::from(Attribute::Bold))
                                            .build(),
                                    );
                                    if let Some(searched) =
                                        self.trie.prefix_search_value(&completed)
                                    {
                                        self.json_state.stream =
                                            JsonStream::new(searched.clone(), self.expand_depth);
                                    }
                                } else {
                                    // SUCCESS!
                                    self.trie.insert(&completed, jsonl);
                                    self.json_state.stream = stream;
                                }
                            }
                            Err(e) => {
                                self.update_hint_message(
                                    format!("Failed to parse query result for viewing: {}", e),
                                    StyleBuilder::new()
                                        .fgc(Color::Red)
                                        .attrs(Attributes::from(Attribute::Bold))
                                        .build(),
                                );
                                if let Some(searched) = self.trie.prefix_search_value(&completed) {
                                    self.json_state.stream =
                                        JsonStream::new(searched.clone(), self.expand_depth);
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    self.update_hint_message(
                        format!("Failed to execute jq query '{}'", &completed),
                        StyleBuilder::new()
                            .fgc(Color::Red)
                            .attrs(Attributes::from(Attribute::Bold))
                            .build(),
                    );
                    if let Some(searched) = self.trie.prefix_search_value(&completed) {
                        self.json_state.stream =
                            JsonStream::new(searched.clone(), self.expand_depth);
                    }
                    return signal;
                }
            }
        }
        signal
    }
}
