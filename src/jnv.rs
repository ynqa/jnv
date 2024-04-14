use std::{cell::RefCell, collections::HashSet};

use anyhow::Result;

use promkit::{
    crossterm::style::{Attribute, Attributes, Color},
    json::{self, JsonPathSegment, JsonStream},
    listbox,
    serde_json::{self},
    snapshot::Snapshot,
    style::StyleBuilder,
    suggest::Suggest,
    switch::ActiveKeySwitcher,
    text, text_editor, Prompt,
};

mod keymap;
mod render;
mod trie;
use trie::QueryTrie;

use crate::util;

pub struct Jnv {
    input_json_stream: Vec<serde_json::Value>,
    expand_depth: Option<usize>,
    no_hint: bool,

    query_editor_state: text_editor::State,
    hint_message_state: text::State,
    suggest: Suggest,
    suggest_state: listbox::State,
    json_state: json::State,
    keymap: ActiveKeySwitcher<keymap::Keymap>,
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
        let stream = util::deserialize_json(&input_json_str)?;
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

        Ok(Self {
            input_json_stream: stream.clone(),
            expand_depth,
            no_hint,
            query_editor_state: text_editor::State {
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
            },
            hint_message_state: text::State {
                text: Default::default(),
                style: StyleBuilder::new()
                    .fgc(Color::Green)
                    .attrs(Attributes::from(Attribute::Bold))
                    .build(),
            },
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
            keymap: ActiveKeySwitcher::new("default", self::keymap::default as keymap::Keymap)
                .register("on_suggest", self::keymap::on_suggest),
            json_state: json::State {
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

    pub fn prompt(self) -> Result<Prompt<render::Renderer>> {
        Ok(Prompt {
            renderer: render::Renderer {
                keymap: RefCell::new(self.keymap),
                query_editor_snapshot: Snapshot::<text_editor::State>::new(self.query_editor_state),
                hint_message_snapshot: Snapshot::<text::State>::new(self.hint_message_state),
                suggest: self.suggest,
                suggest_snapshot: Snapshot::<listbox::State>::new(self.suggest_state),
                json_snapshot: Snapshot::<json::State>::new(self.json_state),
                trie: QueryTrie::default(),
                input_json_stream: self.input_json_stream,
                expand_depth: self.expand_depth,
                no_hint: self.no_hint,
            },
        })
    }
}
