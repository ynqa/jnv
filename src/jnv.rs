use std::cell::RefCell;

use anyhow::{anyhow, Result};
use gag::Gag;

use promkit::{
    crossterm::{
        event::Event,
        style::{Attribute, Attributes, Color},
    },
    json::{self, JsonBundle, JsonNode, JsonPathSegment},
    keymap::KeymapManager,
    listbox, serde_json,
    snapshot::Snapshot,
    style::StyleBuilder,
    suggest::Suggest,
    text, text_editor, Prompt, PromptSignal, Renderer,
};

mod keymap;
mod render;
mod trie;
use trie::QueryTrie;

pub struct Jnv {
    input_json: String,
    expand_depth: Option<usize>,
    no_hint: bool,

    query_editor_renderer: text_editor::Renderer,
    hint_message_renderer: text::Renderer,
    suggest: Suggest,
    suggest_renderer: listbox::Renderer,
    json_bundle_renderer: json::bundle::Renderer,
    keymap: KeymapManager<self::render::Renderer>,
}

impl Jnv {
    pub fn try_new(
        input_json: String,
        expand_depth: Option<usize>,
        no_hint: bool,
        edit_mode: text_editor::Mode,
        indent: usize,
        suggestion_list_length: usize,
    ) -> Result<Self> {
        let kinds = JsonNode::try_new(input_json.clone(), None)?.flatten_visibles();
        let full = kinds.iter().filter_map(|kind| kind.path()).map(|segments| {
            if segments.is_empty() {
                ".".to_string()
            } else {
                segments
                    .iter()
                    .map(|segment| match segment {
                        JsonPathSegment::Key(key) => {
                            if key.contains('.') || key.contains('-') {
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
            input_json: input_json.clone(),
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
            suggest: Suggest::from_iter(full),
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
            json_bundle_renderer: json::bundle::Renderer {
                bundle: json::JsonBundle::new([JsonNode::try_new(
                    j9::run(".", &input_json)
                        .map_err(|_| {
                            anyhow!(format!(
                                "jq error with program: '.', input: {}",
                                &input_json
                            ))
                        })?
                        .first()
                        .map(|s| s.as_str())
                        .ok_or_else(|| anyhow!("No data found"))?,
                    expand_depth,
                )?]),
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

    pub fn prompt(self) -> Result<Prompt<String>> {
        let trie = RefCell::new(QueryTrie::default());
        Ok(Prompt::try_new(
            Box::new(self::render::Renderer {
                keymap: self.keymap,
                query_editor_snapshot: Snapshot::<text_editor::Renderer>::new(
                    self.query_editor_renderer,
                ),
                hint_message_snapshot: Snapshot::<text::Renderer>::new(self.hint_message_renderer),
                suggest: self.suggest,
                suggest_snapshot: Snapshot::<listbox::Renderer>::new(self.suggest_renderer),
                json_bundle_snapshot: Snapshot::<json::bundle::Renderer>::new(
                    self.json_bundle_renderer,
                ),
            }),
            Box::new(
                move |event: &Event,
                      renderer: &mut Box<dyn Renderer + 'static>|
                      -> promkit::Result<PromptSignal> {
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
                        let ret = j9::run(&completed, &self.input_json);
                        drop(ignore_err);

                        ret
                        .map(|ret| {
                            if ret.is_empty() {
                                if !self.no_hint {
                                    renderer.hint_message_snapshot.after_mut().replace(text::Renderer {
                                        text: format!("JSON query ('{}') was executed, but no results were returned.", &completed),
                                        style: StyleBuilder::new()
                                            .fgc(Color::Red)
                                            .attrs(Attributes::from(Attribute::Bold))
                                            .build(),
                                    });
                                }
                                if let Some(searched) = trie.borrow().prefix_search_value(&completed) {
                                    renderer.json_bundle_snapshot.after_mut().bundle = JsonBundle::new(searched.clone());
                                }
                            } else {
                                ret.iter().map(|string| {
                                    JsonNode::try_new(string.as_str(), self.expand_depth)
                                }).collect::<Result<Vec<JsonNode>, _>>()
                                .map(|nodes| {
                                    if nodes.len() == 1 && nodes.first().unwrap() == &JsonNode::Leaf(serde_json::Value::Null) {
                                        if !self.no_hint {
                                            renderer.hint_message_snapshot.after_mut().replace(text::Renderer {
                                                text: format!(
                                                    "JSON query resulted in 'null', which may indicate a typo or incorrect query: '{}'",
                                                    &completed,
                                                ),
                                                style: StyleBuilder::new()
                                                    .fgc(Color::Yellow)
                                                    .attrs(Attributes::from(Attribute::Bold))
                                                    .build(),
                                            });
                                        }
                                        if let Some(searched) = trie.borrow().prefix_search_value(&completed) {
                                            renderer.json_bundle_snapshot.after_mut().bundle = JsonBundle::new(searched.clone());
                                        }
                                    } else {
                                        // SUCCESS!
                                        trie.borrow_mut().insert(&completed, nodes.clone());
                                        renderer.json_bundle_snapshot.after_mut().bundle = JsonBundle::new(nodes);
                                    }
                                })
                                .unwrap_or_else(|e| {
                                    if !self.no_hint {
                                        renderer.hint_message_snapshot.after_mut().replace(text::Renderer{
                                            text: format!(
                                                "Failed to parse query result for viewing: {}",
                                                e
                                            ),
                                            style: StyleBuilder::new()
                                                .fgc(Color::Red)
                                                .attrs(Attributes::from(Attribute::Bold))
                                                .build(),
                                        })
                                    }
                                    if let Some(searched) = trie.borrow().prefix_search_value(&completed) {
                                        renderer.json_bundle_snapshot.after_mut().bundle = JsonBundle::new(searched.clone());
                                    }
                                });
                            }
                        })
                        .unwrap_or_else(|_| {
                            if !self.no_hint {
                                renderer.hint_message_snapshot.after_mut().replace(text::Renderer {
                                    text: format!("Failed to execute jq query '{}'", &completed),
                                    style: StyleBuilder::new()
                                        .fgc(Color::Red)
                                        .attrs(Attributes::from(Attribute::Bold))
                                        .build(),
                                    },
                                );
                            }
                            if let Some(searched) = trie.borrow().prefix_search_value(&completed) {
                                renderer.json_bundle_snapshot.after_mut().bundle = JsonBundle::new(searched.clone());
                            }
                        });
                    }
                    Ok(signal)
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
