use std::cell::RefCell;

use gag::Gag;
use promkit::{
    crossterm::{
        event::Event,
        style::{Attribute, Attributes, Color, ContentStyle},
    },
    impl_as_any,
    json::{self, JsonNode, JsonStream},
    listbox,
    pane::Pane,
    serde_json::{self},
    snapshot::Snapshot,
    style::StyleBuilder,
    suggest::Suggest,
    switch::ActiveKeySwitcher,
    text, text_editor, PaneFactory, PromptSignal,
};

use crate::util;

use super::{keymap, trie::QueryTrie};

#[derive(Clone)]
pub struct Renderer {
    pub keymap: RefCell<ActiveKeySwitcher<keymap::Keymap>>,
    pub query_editor_snapshot: Snapshot<text_editor::State>,
    pub hint_message_snapshot: Snapshot<text::State>,
    pub suggest: Suggest,
    pub suggest_snapshot: Snapshot<listbox::State>,
    pub json_snapshot: Snapshot<json::State>,
    pub trie: QueryTrie,
    pub input_json_stream: Vec<serde_json::Value>,
    pub expand_depth: Option<usize>,
    pub no_hint: bool,
}

impl_as_any!(Renderer);

impl Renderer {
    fn update_hint_message(&mut self, text: String, style: ContentStyle) {
        if !self.no_hint {
            self.hint_message_snapshot
                .after_mut()
                .replace(text::State { text, style })
        }
    }
}

impl promkit::Renderer for Renderer {
    type Return = String;

    fn create_panes(&self, width: u16) -> Vec<Pane> {
        vec![
            self.query_editor_snapshot.create_pane(width),
            self.hint_message_snapshot.create_pane(width),
            self.suggest_snapshot.create_pane(width),
            self.json_snapshot.create_pane(width),
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
                            format!("Failed to execute jq query '{}'", &completed),
                            StyleBuilder::new()
                                .fgc(Color::Red)
                                .attrs(Attributes::from(Attribute::Bold))
                                .build(),
                        );
                        if let Some(searched) = self.trie.prefix_search_value(&completed) {
                            self.json_snapshot.after_mut().stream =
                                JsonStream::new(searched.clone(), self.expand_depth);
                        }
                        return signal;
                    }
                };
                flatten_ret.extend(inner_ret);
            }
            drop(ignore_err);

            if flatten_ret.is_empty() {
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
                    self.json_snapshot.after_mut().stream =
                        JsonStream::new(searched.clone(), self.expand_depth);
                }
            } else {
                match util::deserialize_json(&flatten_ret.join("\n")) {
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
                            if let Some(searched) = self.trie.prefix_search_value(&completed) {
                                self.json_snapshot.after_mut().stream =
                                    JsonStream::new(searched.clone(), self.expand_depth);
                            }
                        } else {
                            // SUCCESS!
                            self.trie.insert(&completed, jsonl);
                            self.json_snapshot.after_mut().stream = stream;
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
                            self.json_snapshot.after_mut().stream =
                                JsonStream::new(searched.clone(), self.expand_depth);
                        }
                    }
                }
                // flatten_ret.is_empty()
            }
            // before != completed
        }
        signal
    }

    fn finalize(&self) -> anyhow::Result<Self::Return> {
        Ok(self
            .query_editor_snapshot
            .after()
            .texteditor
            .text_without_cursor()
            .to_string())
    }
}
