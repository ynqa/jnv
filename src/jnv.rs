use std::cell::RefCell;

use anyhow::Result;

use arboard::Clipboard;
use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};

use promkit::{
    crossterm::{
        event::Event,
        style::{Attribute, Attributes, Color, ContentStyle},
    },
    jsonstream::{self, JsonStream},
    jsonz::{self, format::RowFormatter, Value},
    listbox,
    pane::Pane,
    serde_json,
    snapshot::Snapshot,
    style::StyleBuilder,
    suggest::Suggest,
    switch::ActiveKeySwitcher,
    text, text_editor, PaneFactory, Prompt, PromptSignal,
};

use crate::trie::FilterTrie;

mod keymap;

fn run_jaq(
    query: &str,
    json_stream: &[serde_json::Value],
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
    json: jsonstream::State,

    // Store the filter history
    trie: FilterTrie,
    // Store the filter suggestions
    suggest: Suggest,

    no_hint: bool,
}

impl Jnv {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        input_stream: Vec<serde_json::Value>,
        filter_editor: text_editor::State,
        hint_message: text::State,
        suggestions: listbox::State,
        json_theme: JsonTheme,
        no_hint: bool,
    ) -> Result<Prompt<Self>> {
        let mut trie = FilterTrie::default();
        trie.insert(".", input_stream.clone());
        let stream = JsonStream::new(&input_stream);
        let suggest = Suggest::from_iter(jsonz::get_all_paths(&input_stream));

        Ok(Prompt {
            renderer: Self {
                keymap: RefCell::new(
                    ActiveKeySwitcher::new("default", self::keymap::default as keymap::Keymap)
                        .register("on_suggest", self::keymap::on_suggest),
                ),
                filter_editor: Snapshot::<text_editor::State>::new(filter_editor),
                hint_message: Snapshot::<text::State>::new(hint_message),
                suggestions,
                json: jsonstream::State {
                    stream,
                    formatter: RowFormatter {
                        curly_brackets_style: json_theme.curly_brackets_style,
                        square_brackets_style: json_theme.square_brackets_style,
                        key_style: json_theme.key_style,
                        string_value_style: json_theme.string_value_style,
                        number_value_style: json_theme.number_value_style,
                        boolean_value_style: json_theme.boolean_value_style,
                        null_value_style: json_theme.null_value_style,
                        active_item_attribute: json_theme.active_item_attribute,
                        inactive_item_attribute: json_theme.inactive_item_attribute,
                        indent: json_theme.indent,
                    },
                    lines: json_theme.lines,
                },
                trie,
                suggest,
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

    fn store_to_clipboard(&mut self, content: &str, hint: &str) {
        match Clipboard::new() {
            Ok(mut clipboard) => match clipboard.set_text(content) {
                Ok(_) => {
                    self.update_hint_message(
                        hint.to_string(),
                        StyleBuilder::new().fgc(Color::Grey).build(),
                    );
                }
                Err(e) => {
                    self.update_hint_message(
                        format!("Failed to copy to clipboard: {}", e),
                        StyleBuilder::new()
                            .fgc(Color::Red)
                            .attrs(Attributes::from(Attribute::Bold))
                            .build(),
                    );
                }
            },
            // arboard fails (in the specific environment like linux?) on Clipboard::new()
            // suppress the errors (but still show them) not to break the prompt
            // https://github.com/1Password/arboard/issues/153
            Err(e) => {
                self.update_hint_message(
                    format!("Failed to setup clipboard: {}", e),
                    StyleBuilder::new()
                        .fgc(Color::Red)
                        .attrs(Attributes::from(Attribute::Bold))
                        .build(),
                );
            }
        }
    }

    fn store_content_to_clipboard(&mut self) {
        self.store_to_clipboard(
            &self.json.formatter.format_raw_json(self.json.stream.rows()),
            "Copied selected content to clipboard!",
        );
    }

    fn store_query_to_clipboard(&mut self) {
        self.store_to_clipboard(
            &self
                .filter_editor
                .after()
                .texteditor
                .text_without_cursor()
                .to_string(),
            "Copied jq query to clipboard!",
        );
    }
}

impl promkit::Finalizer for Jnv {
    type Return = String;

    fn finalize(&mut self) -> anyhow::Result<Self::Return> {
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
                    self.json.stream = JsonStream::new(jsonl);
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
                    match run_jaq(&filter, &self.input_stream) {
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
                                    self.json.stream = JsonStream::new(searched);
                                }
                            } else {
                                let stream = JsonStream::new(ret.iter());

                                let is_null =
                                    stream.rows().iter().all(|node| node.v == Value::Null);
                                if is_null {
                                    self.update_hint_message(
                                        format!("JSON query resulted in 'null', which may indicate a typo or incorrect query: '{}'", &filter),
                                        StyleBuilder::new()
                                            .fgc(Color::Yellow)
                                            .attrs(Attributes::from(Attribute::Bold))
                                            .build(),
                                    );
                                    if let Some(searched) = self.trie.prefix_search(&filter) {
                                        self.json.stream = JsonStream::new(searched);
                                    }
                                } else {
                                    // SUCCESS!
                                    self.trie.insert(&filter, ret);
                                    self.json.stream = stream;
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
                                self.json.stream = JsonStream::new(searched);
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
