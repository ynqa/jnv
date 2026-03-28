use promkit_widgets::{
    core::{
        crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
        grapheme::StyledGraphemes,
        Widget,
    },
    status::{self, Severity},
    text_editor,
};

use crate::config::EditorKeybinds;

pub struct Editor {
    state: text_editor::State,
    focus_config: text_editor::Config,
    defocus_config: text_editor::Config,
    guide: status::State,
    editor_keybinds: EditorKeybinds,
}

impl Editor {
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
            guide: status::State::default(),
            editor_keybinds,
        }
    }

    pub fn focus(&mut self) {
        self.state.config = self.focus_config.clone();
    }

    pub fn defocus(&mut self) {
        self.state.config = self.defocus_config.clone();
        self.guide = status::State::default();
    }

    pub fn text(&self) -> String {
        self.state.texteditor.text_without_cursor().to_string()
    }

    pub fn create_editor_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
    }

    pub fn create_guide_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.guide.create_graphemes(width, height)
    }

    pub fn clear_guide(&mut self) {
        self.guide = status::State::default();
    }

    pub fn replace_text(&mut self, text: &str) {
        self.state.texteditor.replace(text);
    }

    pub fn set_completion_found_guide(&mut self, loaded_path_count: usize, is_complete: bool) {
        if is_complete {
            self.guide = status::State::new(
                format!("Loaded all ({loaded_path_count}) suggestions"),
                Severity::Success,
            );
        } else {
            self.guide = status::State::new(
                format!("Loaded partially ({loaded_path_count}) suggestions"),
                Severity::Success,
            );
        }
    }

    pub fn set_completion_empty_guide(&mut self, prefix: &str) {
        self.guide = status::State::new(
            format!("No suggestion found for '{prefix}'"),
            Severity::Warning,
        );
    }

    pub async fn operate(&mut self, event: &Event) -> anyhow::Result<()> {
        self.clear_guide();

        match event {
            // Move cursor.
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

            // Move cursor to the nearest character.
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

            // Erase char(s).
            key if self.editor_keybinds.erase.contains(key) => {
                self.state.texteditor.erase();
            }
            key if self.editor_keybinds.erase_all.contains(key) => {
                self.state.texteditor.erase_all();
            }

            // Erase to the nearest character.
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

            // Input char.
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
