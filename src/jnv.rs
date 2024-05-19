use std::cell::RefCell;

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

use crate::trie::FilterTrie;

mod keymap;

/// Deserializes a JSON string into a vector of `serde_json::Value`.
///
/// This function takes a JSON string as input and attempts to parse it into a vector
/// of `serde_json::Value`, which represents any valid JSON value (e.g., object, array, string, number).
/// It leverages `serde_json::Deserializer` to parse the string and collect the results.
///
/// # Arguments
/// * `json_str` - A string slice that holds the JSON data to be deserialized.
///
/// # Returns
/// An `anyhow::Result` wrapping a vector of `serde_json::Value`. On success, it contains the parsed
/// JSON data. On failure, it contains an error detailing what went wrong during parsing.
fn deserialize_json(
    json_str: &str,
    limit_length: Option<usize>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let deserializer = Deserializer::from_str(json_str).into_iter::<serde_json::Value>();
    let results = match limit_length {
        Some(l) => deserializer.take(l).collect::<Result<Vec<_>, _>>(),
        None => deserializer.collect::<Result<Vec<_>, _>>(),
    };
    results.map_err(anyhow::Error::from)
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
        let inner_ret: Vec<String> = match j9::run(query, &v.to_string()) {
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

pub struct JsonTheme {
    /// Style for {}.
    pub curly_brackets_style: ContentStyle,
    /// Style for [].
    pub square_brackets_style: ContentStyle,
    /// Style for "key".
    pub key_style: ContentStyle,
    /// Style for string values.
    pub string_value_style: ContentStyle,
    /// Style for number values.
    pub number_value_style: ContentStyle,
    /// Style for boolean values.
    pub boolean_value_style: ContentStyle,
    /// Style for null values.
    pub null_value_style: ContentStyle,

    /// Attribute for the selected line.
    pub active_item_attribute: Attribute,
    /// Attribute for unselected lines.
    pub inactive_item_attribute: Attribute,

    /// Number of lines available for rendering.
    pub lines: Option<usize>,

    /// The number of spaces used for indentation in the rendered JSON structure.
    /// This value multiplies with the indentation level of a JSON element to determine
    /// the total indentation space. For example, an `indent` value of 4 means each
    /// indentation level will be 4 spaces wide.
    pub indent: usize,
}

pub struct Jnv {
    input_stream: Vec<serde_json::Value>,

    // Keybindings
    keymap: RefCell<ActiveKeySwitcher<keymap::Keymap>>,

    // For Rendering
    filter_editor: Snapshot<text_editor::State>,
    hint_message: Snapshot<text::State>,
    suggestions: listbox::State,
    json: json::State,

    // Store the filter history
    trie: FilterTrie,
    // Store the filter suggestions
    suggest: Suggest,

    json_expand_depth: Option<usize>,
    json_limit_length: Option<usize>,
    no_hint: bool,
}

impl Jnv {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        input: String,
        filter_editor: text_editor::State,
        hint_message: text::State,
        suggestions: listbox::State,
        json_theme: JsonTheme,
        json_expand_depth: Option<usize>,
        json_limit_length: Option<usize>,
        no_hint: bool,
    ) -> Result<Prompt<Self>> {
        let input_stream = deserialize_json(&input, json_limit_length)?;

        let mut trie = FilterTrie::default();
        trie.insert(".", input_stream.clone());

        let all_kinds = JsonStream::new(input_stream.clone(), None).flatten_kinds();
        let suggest = Suggest::from_iter(all_kinds.iter().filter_map(|kind| kind.path()).map(
            |segments| {
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
            },
        ));

        Ok(Prompt {
            renderer: Self {
                keymap: RefCell::new(
                    ActiveKeySwitcher::new("default", self::keymap::default as keymap::Keymap)
                        .register("on_suggest", self::keymap::on_suggest),
                ),
                filter_editor: Snapshot::<text_editor::State>::new(filter_editor),
                hint_message: Snapshot::<text::State>::new(hint_message),
                suggestions,
                json: json::State {
                    stream: JsonStream::new(input_stream.clone(), json_expand_depth),
                    curly_brackets_style: json_theme.curly_brackets_style,
                    square_brackets_style: json_theme.square_brackets_style,
                    key_style: json_theme.key_style,
                    string_value_style: json_theme.string_value_style,
                    number_value_style: json_theme.number_value_style,
                    boolean_value_style: json_theme.boolean_value_style,
                    null_value_style: json_theme.null_value_style,
                    active_item_attribute: json_theme.active_item_attribute,
                    inactive_item_attribute: json_theme.inactive_item_attribute,
                    lines: json_theme.lines,
                    indent: json_theme.indent,
                },
                trie,
                suggest,
                json_expand_depth,
                json_limit_length,
                no_hint,
                input_stream,
            },
        })
    }

    fn update_hint_message(&mut self, text: String, style: ContentStyle) {
        if !self.no_hint {
            self.hint_message
                .after_mut()
                .replace(text::State { text, style })
        }
    }
}

impl promkit::Finalizer for Jnv {
    type Return = String;

    fn finalize(&self) -> anyhow::Result<Self::Return> {
        Ok(self
            .filter_editor
            .after()
            .texteditor
            .text_without_cursor()
            .to_string())
    }
}

impl promkit::Renderer for Jnv {
    fn create_panes(&self, width: u16, height: u16) -> Vec<Pane> {
        vec![
            self.filter_editor.create_pane(width, height),
            self.hint_message.create_pane(width, height),
            self.suggestions.create_pane(width, height),
            self.json.create_pane(width, height),
        ]
    }

    fn evaluate(&mut self, event: &Event) -> anyhow::Result<PromptSignal> {
        let keymap = *self.keymap.borrow_mut().get();
        let signal = keymap(event, self);
        let filter = self
            .filter_editor
            .after()
            .texteditor
            .text_without_cursor()
            .to_string();

        // Check if the query has changed
        if filter
            != self
                .filter_editor
                .borrow_before()
                .texteditor
                .text_without_cursor()
                .to_string()
        {
            self.hint_message.reset_after_to_init();

            match self.trie.exact_search(&filter) {
                Some(jsonl) => {
                    self.json.stream = JsonStream::new(jsonl.clone(), self.json_expand_depth);
                    self.update_hint_message(
                        format!(
                            "JSON query ('{}') was already executed. Result was retrieved from cache.",
                            &filter
                        ),
                        StyleBuilder::new()
                            .fgc(Color::DarkGrey)
                            .attrs(Attributes::from(Attribute::Bold))
                            .build(),
                    );
                }
                None => {
                    match run_jq(&filter, &self.input_stream) {
                        Ok(ret) => {
                            if ret.is_empty() {
                                self.update_hint_message(
                                    format!(
                                        "JSON query ('{}') was executed, but no results were returned.",
                                        &filter
                                    ),
                                    StyleBuilder::new()
                                        .fgc(Color::Red)
                                        .attrs(Attributes::from(Attribute::Bold))
                                        .build(),
                                );
                                if let Some(searched) = self.trie.prefix_search(&filter) {
                                    self.json.stream =
                                        JsonStream::new(searched.clone(), self.json_expand_depth);
                                }
                            } else {
                                match deserialize_json(&ret.join("\n"), self.json_limit_length) {
                                    Ok(jsonl) => {
                                        let stream =
                                            JsonStream::new(jsonl.clone(), self.json_expand_depth);

                                        let is_null = stream.roots().iter().all(|node| {
                                            node == &JsonNode::Leaf(serde_json::Value::Null)
                                        });
                                        if is_null {
                                            self.update_hint_message(
                                                format!("JSON query resulted in 'null', which may indicate a typo or incorrect query: '{}'", &filter),
                                                StyleBuilder::new()
                                                    .fgc(Color::Yellow)
                                                    .attrs(Attributes::from(Attribute::Bold))
                                                    .build(),
                                            );
                                            if let Some(searched) = self.trie.prefix_search(&filter)
                                            {
                                                self.json.stream = JsonStream::new(
                                                    searched.clone(),
                                                    self.json_expand_depth,
                                                );
                                            }
                                        } else {
                                            // SUCCESS!
                                            self.trie.insert(&filter, jsonl);
                                            self.json.stream = stream;
                                        }
                                    }
                                    Err(e) => {
                                        self.update_hint_message(
                                            format!(
                                                "Failed to parse query result for viewing: {}",
                                                e
                                            ),
                                            StyleBuilder::new()
                                                .fgc(Color::Red)
                                                .attrs(Attributes::from(Attribute::Bold))
                                                .build(),
                                        );
                                        if let Some(searched) = self.trie.prefix_search(&filter) {
                                            self.json.stream = JsonStream::new(
                                                searched.clone(),
                                                self.json_expand_depth,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            self.update_hint_message(
                                format!("Failed to execute jq query '{}'", &filter),
                                StyleBuilder::new()
                                    .fgc(Color::Red)
                                    .attrs(Attributes::from(Attribute::Bold))
                                    .build(),
                            );
                            if let Some(searched) = self.trie.prefix_search(&filter) {
                                self.json.stream =
                                    JsonStream::new(searched.clone(), self.json_expand_depth);
                            }
                            return signal;
                        }
                    }
                }
            }
        }
        signal
    }
}
