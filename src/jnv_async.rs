use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use futures::Future;

use gag::Gag;
use tokio::sync::mpsc::Sender;

use promkit::{
    crossterm::style::{Attribute, Attributes, Color, ContentStyle},
    json::{self, JsonNode, JsonStream},
    listbox,
    pane::Pane,
    serde_json::{self, Deserializer},
    snapshot::Snapshot,
    style::StyleBuilder,
    suggest::Suggest,
    switch::ActiveKeySwitcher,
    text, text_editor, PaneFactory,
};
use promkit_async::{EventBundle, PaneSyncer, Prompt};

use crate::trie::FilterTrie;

use self::keymap::{FilterEditorKeymap, JsonKeymap};

mod keymap;

fn update_hint_message(
    no_hint: bool,
    text: String,
    style: ContentStyle,
    hint_message_snapshot: &mut Snapshot<text::State>,
) {
    if !no_hint {
        hint_message_snapshot
            .after_mut()
            .replace(text::State { text, style })
    }
}

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
    input_stream: Arc<Vec<serde_json::Value>>,

    // Keybindings
    filter_editor_keymap: RefCell<ActiveKeySwitcher<FilterEditorKeymap>>,
    json_keymap: RefCell<ActiveKeySwitcher<JsonKeymap>>,

    // For rendering
    filter_editor: Arc<Mutex<text_editor::State>>,
    hint_message: Arc<Mutex<Snapshot<text::State>>>,
    suggestions: listbox::State,
    json: Arc<Mutex<json::State>>,

    // Store the filter history
    trie: FilterTrie,
    // Store the filter suggestions
    suggest: Suggest,

    json_expand_depth: Option<usize>,
    no_hint: bool,

    // Channels
    fin_sender: Sender<()>,
    indexed_pane_sender: Sender<(usize, usize, Pane)>,
    loading_activation_sender: Sender<(usize, usize)>,
}

impl Jnv {
    pub fn try_new(
        input: String,
        filter_editor: text_editor::State,
        hint_message: text::State,
        suggestions: listbox::State,
        json_theme: json::Theme,
        json_expand_depth: Option<usize>,
        no_hint: bool,
        fin_sender: Sender<()>,
        indexed_pane_sender: Sender<(usize, usize, Pane)>,
        loading_activation_sender: Sender<(usize, usize)>,
    ) -> Result<Prompt<Self>> {
        let input_stream = deserialize_json(&input)?;

        let mut trie = FilterTrie::default();
        trie.insert(".", input_stream.clone());

        let suggest = Suggest::from_iter(vec![String::from(".fake_suggestion")]);
        // let all_kinds = JsonStream::new(stream.clone(), None).flatten_kinds();
        // let suggestions = all_kinds
        //     .iter()
        //     .filter_map(|kind| kind.path())
        //     .map(|segments| {
        //         if segments.is_empty() {
        //             ".".to_string()
        //         } else {
        //             segments
        //                 .iter()
        //                 .enumerate()
        //                 .map(|(i, segment)| match segment {
        //                     JsonPathSegment::Key(key) => {
        //                         if key.contains('.') || key.contains('-') || key.contains('@') {
        //                             format!(".\"{}\"", key)
        //                         } else {
        //                             format!(".{}", key)
        //                         }
        //                     }
        //                     JsonPathSegment::Index(index) => {
        //                         if i == 0 {
        //                             format!(".[{}]", index)
        //                         } else {
        //                             format!("[{}]", index)
        //                         }
        //                     }
        //                 })
        //                 .collect::<String>()
        //         }
        //     });

        Ok(Prompt {
            renderer: Self {
                filter_editor_keymap: RefCell::new(ActiveKeySwitcher::new(
                    "default",
                    self::keymap::default_query_editor,
                )),
                json_keymap: RefCell::new(ActiveKeySwitcher::new(
                    "default",
                    self::keymap::default_json as keymap::JsonKeymap,
                )),
                filter_editor: Arc::new(Mutex::new(filter_editor)),
                hint_message: Arc::new(Mutex::new(Snapshot::<text::State>::new(hint_message))),
                suggestions,
                json: Arc::new(Mutex::new(json::State {
                    stream: JsonStream::new(input_stream.clone(), json_expand_depth),
                    theme: json_theme,
                })),
                trie,
                suggest,
                json_expand_depth,
                no_hint,
                fin_sender,
                indexed_pane_sender,
                loading_activation_sender,
                input_stream: Arc::new(input_stream),
            },
        })
    }
}

impl PaneSyncer for Jnv {
    fn init_panes(&self, width: u16, height: u16) -> Vec<Pane> {
        vec![
            self.filter_editor
                .lock()
                .unwrap()
                .create_pane(width, height),
            self.hint_message.lock().unwrap().create_pane(width, height),
            self.suggestions.create_pane(width, height),
            self.json.lock().unwrap().create_pane(width, height),
        ]
    }

    fn sync(
        &mut self,
        version: usize,
        event_buffer: &[EventBundle],
        width: u16,
        height: u16,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        let fin_sender = self.fin_sender.clone();
        let indexed_pane_sender = self.indexed_pane_sender.clone();
        let loading_activation_sender = self.loading_activation_sender.clone();

        let shared_filter_editor = Arc::clone(&self.filter_editor);
        let shared_hint_message = Arc::clone(&self.hint_message);
        let shared_json = Arc::clone(&self.json);

        let event_buffer = event_buffer.to_vec();

        let filter_editor_keymap = *self.filter_editor_keymap.borrow_mut().get();
        let json_keymap = *self.json_keymap.borrow_mut().get();

        let mut trie = self.trie.clone();

        let no_hint = self.no_hint;
        let input_stream = Arc::clone(&self.input_stream);
        let json_expand_depth = self.json_expand_depth;

        async move {
            loading_activation_sender.send((version, 3)).await?;
            let indexed_pane_sender_after_jq = indexed_pane_sender.clone();

            let mut local_query_editor_state = shared_filter_editor.lock().unwrap();
            let query_edited =
                filter_editor_keymap(&event_buffer, &mut local_query_editor_state, fin_sender)?;
            indexed_pane_sender.try_send((
                version,
                0,
                local_query_editor_state.create_pane(width, height),
            ))?;

            let completed = local_query_editor_state
                .texteditor
                .text_without_cursor()
                .to_string();

            if query_edited {
                tokio::spawn(async move {
                    let mut local_hint_message = shared_hint_message.lock().unwrap();
                    local_hint_message.reset_after_to_init();
                    let mut local_json_state = shared_json.lock().unwrap();

                    match run_jq(&completed, &input_stream) {
                        Ok(ret) => {
                            if ret.is_empty() {
                                update_hint_message(
                                        no_hint,
                                        format!(
                                            "JSON query ('{}') was executed, but no results were returned.",
                                            &completed
                                        ),
                                        StyleBuilder::new()
                                            .fgc(Color::Red)
                                            .attrs(Attributes::from(Attribute::Bold))
                                            .build(),
                                        &mut *local_hint_message,
                                    );
                                if let Some(searched) = trie.prefix_search(&completed) {
                                    local_json_state.stream =
                                        JsonStream::new(searched.clone(), json_expand_depth);
                                }
                            } else {
                                match deserialize_json(&ret.join("\n")) {
                                    Ok(jsonl) => {
                                        let stream =
                                            JsonStream::new(jsonl.clone(), json_expand_depth);

                                        let is_null = stream.roots().iter().all(|node| {
                                            node == &JsonNode::Leaf(serde_json::Value::Null)
                                        });
                                        if is_null {
                                            update_hint_message(
                                                    no_hint,
                                                    format!("JSON query resulted in 'null', which may indicate a typo or incorrect query: '{}'", &completed),
                                                    StyleBuilder::new()
                                                        .fgc(Color::Yellow)
                                                        .attrs(Attributes::from(Attribute::Bold))
                                                        .build(),
                                                    &mut *local_hint_message,
                                                );
                                            if let Some(searched) = trie.prefix_search(&completed) {
                                                local_json_state.stream = JsonStream::new(
                                                    searched.clone(),
                                                    json_expand_depth,
                                                );
                                            }
                                        } else {
                                            // SUCCESS!
                                            trie.insert(&completed, jsonl);
                                            local_json_state.stream = stream;
                                        }
                                    }
                                    Err(e) => {
                                        update_hint_message(
                                            no_hint,
                                            format!(
                                                "Failed to parse query result for viewing: {}",
                                                e
                                            ),
                                            StyleBuilder::new()
                                                .fgc(Color::Red)
                                                .attrs(Attributes::from(Attribute::Bold))
                                                .build(),
                                            &mut *local_hint_message,
                                        );
                                        if let Some(searched) = trie.prefix_search(&completed) {
                                            local_json_state.stream = JsonStream::new(
                                                searched.clone(),
                                                json_expand_depth,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            update_hint_message(
                                no_hint,
                                format!("Failed to execute jq query '{}'", &completed),
                                StyleBuilder::new()
                                    .fgc(Color::Red)
                                    .attrs(Attributes::from(Attribute::Bold))
                                    .build(),
                                &mut *local_hint_message,
                            );
                            if let Some(searched) = trie.prefix_search(&completed) {
                                local_json_state.stream =
                                    JsonStream::new(searched.clone(), json_expand_depth);
                            }
                        }
                    };
                    indexed_pane_sender_after_jq.try_send((
                        version,
                        3,
                        local_json_state.create_pane(width, height),
                    ))?;
                    indexed_pane_sender_after_jq.try_send((
                        version,
                        1,
                        local_hint_message.create_pane(width, height),
                    ))?;

                    Ok::<(), anyhow::Error>(())
                });
            } else {
                tokio::spawn(async move {
                    let mut local_json_state = shared_json.lock().unwrap();
                    json_keymap(&event_buffer, &mut local_json_state)?;
                    indexed_pane_sender.try_send((
                        version,
                        3,
                        local_json_state.create_pane(width, height),
                    ))?;
                    Ok::<(), anyhow::Error>(())
                });
            }

            Ok(())
        }
    }
}

impl promkit::Finalizer for Jnv {
    type Return = String;

    fn finalize(&self) -> anyhow::Result<Self::Return> {
        Ok(self
            .filter_editor
            .lock()
            .unwrap()
            .texteditor
            .text_without_cursor()
            .to_string())
    }
}
