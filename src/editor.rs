use std::{future::Future, pin::Pin};

use promkit_widgets::{
    core::{
        Widget,
        crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
        grapheme::StyledGraphemes,
    },
    status::{self, Severity},
    text_editor,
};

use crate::{config::EditorKeybinds, search::IncrementalSearcher};

pub struct Editor {
    handler: Handler,
    state: text_editor::State,
    focus_config: text_editor::Config,
    defocus_config: text_editor::Config,
    guide: status::State,
    searcher: IncrementalSearcher,
    editor_keybinds: EditorKeybinds,
}

impl Editor {
    pub fn new(
        state: text_editor::State,
        searcher: IncrementalSearcher,
        focus_config: text_editor::Config,
        defocus_config: text_editor::Config,
        editor_keybinds: EditorKeybinds,
    ) -> Self {
        Self {
            handler: BOXED_EDITOR_HANDLER,
            state,
            focus_config,
            defocus_config,
            guide: status::State::default(),
            searcher,
            editor_keybinds,
        }
    }

    pub fn focus(&mut self) {
        self.state.config = self.focus_config.clone();
    }

    pub fn defocus(&mut self) {
        self.state.config = self.defocus_config.clone();

        self.searcher.leave_search();
        self.handler = BOXED_EDITOR_HANDLER;

        self.guide = status::State::default();
    }

    pub fn text(&self) -> String {
        self.state.texteditor.text_without_cursor().to_string()
    }

    pub fn create_editor_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
    }

    pub fn create_searcher_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.searcher.create_pane(width, height)
    }

    pub fn create_guide_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.guide.create_graphemes(width, height)
    }

    pub async fn operate(&mut self, event: &Event) -> anyhow::Result<()> {
        (self.handler)(event, self).await
    }
}

pub type Handler = for<'a> fn(
    &'a Event,
    &'a mut Editor,
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

const BOXED_EDITOR_HANDLER: Handler =
    |event, editor| -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(edit(event, editor))
    };
const BOXED_SEARCHER_HANDLER: Handler =
    |event, editor| -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(search(event, editor))
    };

pub async fn edit<'a>(event: &'a Event, editor: &'a mut Editor) -> anyhow::Result<()> {
    editor.guide = status::State::default();

    match event {
        key if editor.editor_keybinds.completion.contains(key) => {
            let prefix = editor.state.texteditor.text_without_cursor().to_string();
            match editor.searcher.start_search(&prefix) {
                Ok(result) => match result.head_item {
                    Some(head) => {
                        if result.load_state.loaded {
                            editor.guide = status::State::new(
                                format!(
                                    "Loaded all ({}) suggestions",
                                    result.load_state.loaded_item_len
                                ),
                                Severity::Success,
                            );
                        } else {
                            editor.guide = status::State::new(
                                format!(
                                    "Loaded partially ({}) suggestions",
                                    result.load_state.loaded_item_len
                                ),
                                Severity::Success,
                            );
                        }
                        editor.state.texteditor.replace(&head);
                        editor.handler = BOXED_SEARCHER_HANDLER;
                    }
                    None => {
                        editor.guide = status::State::new(
                            format!("No suggestion found for '{prefix}'"),
                            Severity::Warning,
                        );
                    }
                },
                Err(e) => {
                    editor.guide = status::State::new(
                        format!("Failed to lookup suggestions: {e}"),
                        Severity::Warning,
                    );
                }
            }
        }

        // Move cursor.
        key if editor.editor_keybinds.backward.contains(key) => {
            editor.state.texteditor.backward();
        }
        key if editor.editor_keybinds.forward.contains(key) => {
            editor.state.texteditor.forward();
        }
        key if editor.editor_keybinds.move_to_head.contains(key) => {
            editor.state.texteditor.move_to_head();
        }
        key if editor.editor_keybinds.move_to_tail.contains(key) => {
            editor.state.texteditor.move_to_tail();
        }

        // Move cursor to the nearest character.
        key if editor
            .editor_keybinds
            .move_to_previous_nearest
            .contains(key) =>
        {
            editor
                .state
                .texteditor
                .move_to_previous_nearest(&editor.state.config.word_break_chars);
        }
        key if editor.editor_keybinds.move_to_next_nearest.contains(key) => {
            editor
                .state
                .texteditor
                .move_to_next_nearest(&editor.state.config.word_break_chars);
        }

        // Erase char(s).
        key if editor.editor_keybinds.erase.contains(key) => {
            editor.state.texteditor.erase();
        }
        key if editor.editor_keybinds.erase_all.contains(key) => {
            editor.state.texteditor.erase_all();
        }

        // Erase to the nearest character.
        key if editor
            .editor_keybinds
            .erase_to_previous_nearest
            .contains(key) =>
        {
            editor
                .state
                .texteditor
                .erase_to_previous_nearest(&editor.state.config.word_break_chars);
        }
        key if editor.editor_keybinds.erase_to_next_nearest.contains(key) => {
            editor
                .state
                .texteditor
                .erase_to_next_nearest(&editor.state.config.word_break_chars);
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
        }) => match editor.state.config.edit_mode {
            text_editor::Mode::Insert => editor.state.texteditor.insert(*ch),
            text_editor::Mode::Overwrite => editor.state.texteditor.overwrite(*ch),
        },

        _ => {}
    }
    Ok(())
}

pub async fn search<'a>(event: &'a Event, editor: &'a mut Editor) -> anyhow::Result<()> {
    match event {
        key if editor.editor_keybinds.on_completion.down.contains(key) => {
            editor.searcher.down_with_load();
            editor
                .state
                .texteditor
                .replace(&editor.searcher.get_current_item());
        }

        key if editor.editor_keybinds.on_completion.up.contains(key) => {
            editor.searcher.up();
            editor
                .state
                .texteditor
                .replace(&editor.searcher.get_current_item());
        }

        _ => {
            editor.searcher.leave_search();
            editor.handler = BOXED_EDITOR_HANDLER;
            return edit(event, editor).await;
        }
    }

    Ok(())
}
