use std::sync::Arc;

use promkit_widgets::{
    core::{
        crossterm::{
            event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
            terminal,
        },
        grapheme::StyledGraphemes,
        Widget,
    },
    text_editor,
};
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    completion::CompletionNavigator,
    config::EditorKeybinds,
    guide::{self, GuideAction},
    prompt::Index,
};

pub struct QueryEditor {
    state: text_editor::State,
    focus_config: text_editor::Config,
    defocus_config: text_editor::Config,
    editor_keybinds: EditorKeybinds,
}

impl QueryEditor {
    pub fn new(
        state: text_editor::State,
        focus_config: text_editor::Config,
        defocus_config: text_editor::Config,
        editor_keybinds: EditorKeybinds,
    ) -> Self {
        Self {
            state,
            focus_config,
            defocus_config,
            editor_keybinds,
        }
    }

    pub fn focus(&mut self) {
        self.state.config = self.focus_config.clone();
    }

    pub fn defocus(&mut self) {
        self.state.config = self.defocus_config.clone();
    }

    pub fn text(&self) -> String {
        self.state.texteditor.text_without_cursor().to_string()
    }

    pub fn create_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
    }

    pub fn replace_text(&mut self, text: &str) {
        self.state.texteditor.replace(text);
    }

    pub async fn operate(&mut self, event: &Event) -> anyhow::Result<()> {
        match event {
            key if self.editor_keybinds.backward.contains(key) => {
                self.state.texteditor.backward();
            }
            key if self.editor_keybinds.forward.contains(key) => {
                self.state.texteditor.forward();
            }
            key if self.editor_keybinds.move_to_head.contains(key) => {
                self.state.texteditor.move_to_head();
            }
            key if self.editor_keybinds.move_to_tail.contains(key) => {
                self.state.texteditor.move_to_tail();
            }
            key if self.editor_keybinds.move_to_previous_nearest.contains(key) => {
                self.state
                    .texteditor
                    .move_to_previous_nearest(&self.state.config.word_break_chars);
            }
            key if self.editor_keybinds.move_to_next_nearest.contains(key) => {
                self.state
                    .texteditor
                    .move_to_next_nearest(&self.state.config.word_break_chars);
            }
            key if self.editor_keybinds.erase.contains(key) => {
                self.state.texteditor.erase();
            }
            key if self.editor_keybinds.erase_all.contains(key) => {
                self.state.texteditor.erase_all();
            }
            key if self.editor_keybinds.erase_to_previous_nearest.contains(key) => {
                self.state
                    .texteditor
                    .erase_to_previous_nearest(&self.state.config.word_break_chars);
            }
            key if self.editor_keybinds.erase_to_next_nearest.contains(key) => {
                self.state
                    .texteditor
                    .erase_to_next_nearest(&self.state.config.word_break_chars);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(ch),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char(ch),
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => match self.state.config.edit_mode {
                text_editor::Mode::Insert => self.state.texteditor.insert(*ch),
                text_editor::Mode::Overwrite => self.state.texteditor.overwrite(*ch),
            },
            _ => {}
        }
        Ok(())
    }
}

pub enum QueryEditorAction {
    Focus(bool),
    CopyQuery,
    UserEvent(Event),
}

pub fn start_query_editor_task(
    mut action_rx: mpsc::Receiver<QueryEditorAction>,
    shared_renderer: promkit_widgets::core::render::SharedRenderer<Index>,
    shared_editor: Arc<RwLock<QueryEditor>>,
    shared_completion: Arc<RwLock<CompletionNavigator>>,
    text_diff: Arc<RwLock<[String; 2]>>,
    debounce_query_tx: mpsc::Sender<String>,
    guide_action_tx: mpsc::Sender<GuideAction>,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(action) = action_rx.recv() => {
                    let size = terminal::size()?;
                    let (editor_pane, completion_pane, maybe_text_for_debounce) = {
                        let mut editor = shared_editor.write().await;
                        match action {
                            QueryEditorAction::Focus(focus) => {
                                let mut completion = shared_completion.write().await;
                                if focus {
                                    editor.focus();
                                } else {
                                    editor.defocus();
                                    completion.leave();
                                }
                            }
                            QueryEditorAction::CopyQuery => {
                                let message = guide::copy_to_clipboard_message(&editor.text());
                                guide_action_tx.send(GuideAction::Show(message)).await?;
                            }
                            QueryEditorAction::UserEvent(event) => {
                                editor.operate(&event).await?;
                            }
                        }
                        let completion = shared_completion.read().await;
                        let current_text = editor.text();
                        (
                            editor.create_pane(size.0, size.1),
                            completion.create_pane(size.0, size.1),
                            current_text,
                        )
                    };

                    let mut diff = text_diff.write().await;
                    if maybe_text_for_debounce != diff[1] {
                        debounce_query_tx.send(maybe_text_for_debounce.clone()).await?;
                        diff[0] = diff[1].clone();
                        diff[1] = maybe_text_for_debounce;
                    }

                    shared_renderer
                        .update([(Index::Editor, editor_pane), (Index::Search, completion_pane)])
                        .render()
                        .await?;
                }
                else => break,
            }
        }
        Ok(())
    })
}
