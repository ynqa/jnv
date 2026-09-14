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
    config::EditorKeybinds,
    context::{Index, SharedContext},
    guide::{self, GuideAction},
};

/// Convert a `char` index into a byte offset within `s`. An index at or past
/// the end maps to `s.len()`. The result is always on a char boundary.
fn char_index_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
}

/// Editor for inputting jq query. It manages the state of the text editor
/// and handles user input events to update the query text accordingly.
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

    /// Focus the query editor, applying the focus configuration.
    pub fn focus(&mut self) {
        self.state.config = self.focus_config.clone();
    }

    /// Defocus the query editor, applying the defocus configuration.
    pub fn defocus(&mut self) {
        self.state.config = self.defocus_config.clone();
    }

    /// Get the current text of the query editor without the cursor.
    pub fn text(&self) -> String {
        self.state.texteditor.text_without_cursor().to_string()
    }

    /// Current text (without cursor) and the cursor as a BYTE offset into it.
    /// promkit's `position()` is a `char` index; this converts it to the byte
    /// offset the completion segmentation expects (always on a char boundary).
    pub fn text_and_cursor_byte(&self) -> (String, usize) {
        let text = self.text();
        let byte = char_index_to_byte(&text, self.state.texteditor.position());
        (text, byte)
    }

    /// Create graphemes for rendering the query editor.
    pub fn create_graphemes(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
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

    /// Replace the current text and place the cursor at an absolute `char`
    /// index. Used by completion to splice a suggestion mid-line and land the
    /// cursor right after the inserted text.
    pub fn replace_text_at(&mut self, text: &str, cursor: usize) {
        self.state.texteditor.replace(text);
        self.set_cursor(cursor);
    }

    /// Handle a user input event to update the query editor's state accordingly.
    /// Returns `true` if the event triggers the completion action, otherwise `false`.
    fn handle_user_event(&mut self, event: &Event) -> bool {
        if self.editor_keybinds.completion.contains(event) {
            return true;
        }

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
        false
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
    /// Replace the current query text and place the cursor at `cursor`
    /// (a `char` index into `text`).
    ReplaceText { text: String, cursor: usize },
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
                            QueryEditorAction::ReplaceText { text, cursor } => {
                                editor.replace_text_at(&text, cursor);
                            }
                            QueryEditorAction::UserEvent(event) => {
                                if editor.handle_user_event(&event) {
                                    shared_ctx.set_active_index(Index::Completion).await;
                                    let (query, cursor) = editor.text_and_cursor_byte();
                                    completion_action_tx
                                        .send(CompletionAction::Enter { query, cursor })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_index_to_byte_ascii_and_multibyte() {
        // ASCII: char index == byte offset.
        assert_eq!(char_index_to_byte("abc", 0), 0);
        assert_eq!(char_index_to_byte("abc", 2), 2);
        // At or past the end maps to len().
        assert_eq!(char_index_to_byte("abc", 3), 3);
        assert_eq!(char_index_to_byte("abc", 99), 3);
        assert_eq!(char_index_to_byte("", 0), 0);

        // Multi-byte: "café" is 'c','a','f','é' where 'é' is 2 bytes.
        assert_eq!(char_index_to_byte("café", 3), 3); // start of 'é'
        assert_eq!(char_index_to_byte("café", 4), 5); // end of string (3 + 2)
    }
}
