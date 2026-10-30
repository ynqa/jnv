use std::sync::Arc;

use promkit_widgets::{
    core::{
        crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
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
    completion::CompletionAction,
    config::{EditorKeybinds, ViConfig},
    context::{Index, SharedContext},
    guide::{self, GuideAction},
    vi,
};

/// Editor for inputting jq query. It manages the state of the text editor
/// and handles user input events to update the query text accordingly.
pub struct QueryEditor {
    state: text_editor::State,
    focus_config: text_editor::Config,
    defocus_config: text_editor::Config,
    editor_keybinds: EditorKeybinds,
    /// vi modal state, present only when vi-style editing is enabled.
    vi: Option<vi::Editor>,
    /// Prefix shown in NORMAL mode (vi only).
    vi_normal_prefix: String,
    /// Prefix shown in INSERT mode (the configured focus prefix).
    insert_prefix: String,
    /// Undo / redo stacks of `(text, cursor)` snapshots (vi only).
    undo_stack: Vec<(String, usize)>,
    redo_stack: Vec<(String, usize)>,
}

impl QueryEditor {
    pub fn new(
        state: text_editor::State,
        focus_config: text_editor::Config,
        defocus_config: text_editor::Config,
        editor_keybinds: EditorKeybinds,
        vi_config: ViConfig,
    ) -> Self {
        let insert_prefix = focus_config.prefix.clone();
        let mut editor = Self {
            state,
            focus_config,
            defocus_config,
            editor_keybinds,
            vi: vi_config.enable.then(|| vi::Editor::new(vi::Mode::Normal)),
            vi_normal_prefix: vi_config.normal_prefix,
            insert_prefix,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        if editor.vi.is_some() {
            // NORMAL-mode cursor rests on a char, not the trailing append slot.
            let len = editor.text().chars().count();
            let cursor = editor.cursor().min(len.saturating_sub(1));
            editor.set_cursor(cursor);
            editor.refresh_prefix();
        }
        editor
    }

    /// Focus the query editor, applying the focus configuration.
    pub fn focus(&mut self) {
        self.state.config = self.focus_config.clone();
        self.refresh_prefix();
    }

    /// Defocus the query editor, applying the defocus configuration.
    pub fn defocus(&mut self) {
        self.state.config = self.defocus_config.clone();
    }

    /// Get the current text of the query editor without the cursor.
    pub fn text(&self) -> String {
        self.state.texteditor.text_without_cursor().to_string()
    }

    /// Create graphemes for rendering the query editor.
    pub fn create_graphemes(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
    }

    /// Replace the current text of the query editor with the given text.
    pub fn replace_text(&mut self, text: &str) {
        self.state.texteditor.replace(text);
    }

    /// The current cursor position as a `char` index.
    fn cursor(&self) -> usize {
        self.state.texteditor.position()
    }

    /// Move the cursor to an absolute `char` index.
    fn set_cursor(&mut self, pos: usize) {
        self.state.texteditor.move_to_head();
        for _ in 0..pos {
            if !self.state.texteditor.forward() {
                break;
            }
        }
    }

    /// Replace the buffer and place the cursor at an absolute `char` index.
    fn set_buffer(&mut self, text: &str, cursor: usize) {
        self.state.texteditor.replace(text);
        self.set_cursor(cursor);
    }

    /// Apply the vi NORMAL-mode prefix or the INSERT-mode (focus) prefix.
    fn refresh_prefix(&mut self) {
        let Some(mode) = self.vi.as_ref().map(|v| v.mode) else {
            return;
        };
        self.state.config.prefix = match mode {
            vi::Mode::Normal => self.vi_normal_prefix.clone(),
            vi::Mode::Insert => self.insert_prefix.clone(),
        };
    }

    fn push_undo(&mut self, text: String, cursor: usize) {
        self.undo_stack.push((text, cursor));
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some((text, cursor)) = self.undo_stack.pop() {
            self.redo_stack.push((self.text(), self.cursor()));
            self.set_buffer(&text, cursor);
        }
    }

    fn redo(&mut self) {
        if let Some((text, cursor)) = self.redo_stack.pop() {
            self.undo_stack.push((self.text(), self.cursor()));
            self.set_buffer(&text, cursor);
        }
    }

    /// Handle a user input event to update the query editor's state accordingly.
    /// Returns `true` if the event triggers the completion action, otherwise `false`.
    fn handle_user_event(&mut self, event: &Event) -> bool {
        if self.editor_keybinds.completion.contains(event) {
            return true;
        }

        if self.vi.is_some() {
            self.handle_vi_event(event);
        } else {
            self.handle_plain_event(event);
        }
        false
    }

    /// Route an event through the vi state machine. In INSERT mode all keys
    /// except <kbd>Esc</kbd> fall through to the normal text-editing path, so
    /// character insertion and the configured keybinds keep working.
    fn handle_vi_event(&mut self, event: &Event) {
        let Event::Key(key) = event else {
            return;
        };
        if key.kind != KeyEventKind::Press {
            return;
        }

        let mode = self.vi.as_ref().map(|v| v.mode).unwrap_or(vi::Mode::Insert);
        match mode {
            vi::Mode::Insert => {
                if key.code == KeyCode::Esc {
                    let cursor = self.cursor();
                    let new_cursor = self.vi.as_mut().expect("vi enabled").leave_insert(cursor);
                    self.set_cursor(new_cursor);
                    self.refresh_prefix();
                } else {
                    self.handle_plain_event(event);
                }
            }
            vi::Mode::Normal => {
                let pending = self.vi.as_ref().expect("vi enabled").is_pending();
                // `u` / Ctrl+R are undo/redo, but only when not mid-command
                // (so `fu`, `ru`, etc. still see `u` as their argument).
                if !pending {
                    if key.code == KeyCode::Char('u') && key.modifiers == KeyModifiers::NONE {
                        self.undo();
                        return;
                    }
                    if key.code == KeyCode::Char('r') && key.modifiers == KeyModifiers::CONTROL {
                        self.redo();
                        return;
                    }
                }

                let text = self.text();
                let cursor = self.cursor();
                let outcome = self
                    .vi
                    .as_mut()
                    .expect("vi enabled")
                    .handle_normal(key, &text, cursor);
                match outcome {
                    vi::Outcome::Move(pos) => self.set_cursor(pos),
                    vi::Outcome::Replace {
                        text: new_text,
                        cursor: new_cursor,
                    } => {
                        self.push_undo(text, cursor);
                        self.set_buffer(&new_text, new_cursor);
                    }
                    vi::Outcome::Noop => {}
                }
                self.refresh_prefix();
            }
        }
    }

    /// Handle an event as plain (non-modal) text editing — the original editor
    /// behavior, also used for vi INSERT mode.
    fn handle_plain_event(&mut self, event: &Event) {
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
    }
}

/// Represent the actions that can be performed on the query editor,
/// such as focusing, copying the query, or handling user events.
pub enum QueryEditorAction {
    /// Focus the query editor.
    Enter,
    /// Defocus the query editor.
    Leave,
    /// Copy the current query text to clipboard.
    CopyQuery,
    /// Replace the current query text.
    ReplaceText(String),
    /// Handle user input events to update the query editor's state.
    UserEvent(Event),
}

/// Spawn a background task to manage the query editor's state and interactions.
pub fn start_query_editor_task(
    mut action_rx: mpsc::Receiver<QueryEditorAction>,
    shared_ctx: SharedContext,
    shared_editor: Arc<RwLock<QueryEditor>>,
    shared_renderer: promkit_widgets::core::render::SharedRenderer<Index>,
    completion_action_tx: mpsc::Sender<CompletionAction>,
    debounce_query_tx: mpsc::Sender<String>,
    guide_action_tx: mpsc::Sender<GuideAction>,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        let mut last_text = {
            let editor = shared_editor.read().await;
            editor.text()
        };
        loop {
            tokio::select! {
                Some(action) = action_rx.recv() => {
                    let area = shared_ctx.area().await;
                    let (editor_view, current_text) = {
                        let mut editor = shared_editor.write().await;
                        match action {
                            QueryEditorAction::Enter => editor.focus(),
                            QueryEditorAction::Leave => editor.defocus(),
                            QueryEditorAction::CopyQuery => {
                                let message = guide::copy_to_clipboard_message(&editor.text());
                                guide_action_tx.send(GuideAction::Show(message)).await?;
                            }
                            QueryEditorAction::ReplaceText(text) => {
                                editor.replace_text(&text);
                            }
                            QueryEditorAction::UserEvent(event) => {
                                if editor.handle_user_event(&event) {
                                    shared_ctx.set_active_index(Index::Completion).await;
                                    completion_action_tx
                                        .send(CompletionAction::Enter {
                                            prefix: editor.text(),
                                        })
                                        .await?;
                                }
                            }
                        }
                        let current_text = editor.text();
                        (editor.create_graphemes(area.0, area.1), current_text)
                    };

                    // If the text has changed, send it to the debounce channel for processing.
                    if current_text != last_text {
                        debounce_query_tx.send(current_text.clone()).await?;
                        last_text = current_text;
                    }

                    // Update the renderer with the new editor view and render it.
                    shared_renderer
                        .update([(Index::QueryEditor, editor_view)])
                        .render()
                        .await?;
                }
                else => break,
            }
        }
        Ok(())
    })
}
