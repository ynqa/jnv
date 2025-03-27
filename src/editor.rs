use std::{future::Future, pin::Pin};

use crossterm::{
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    style::Color,
};
use promkit::{pane::Pane, style::StyleBuilder, text, text_editor, PaneFactory};

use crate::{
    config::{event::Matcher, EditorKeybinds, EditorTheme},
    search::IncrementalSearcher,
};

pub struct Editor {
    handler: Handler,
    state: text_editor::State,
    focus_theme: EditorTheme,
    defocus_theme: EditorTheme,
    guide: text::State,
    searcher: IncrementalSearcher,
    editor_keybinds: EditorKeybinds,
}

impl Editor {
    pub fn new(
        state: text_editor::State,
        searcher: IncrementalSearcher,
        focus_theme: EditorTheme,
        defocus_theme: EditorTheme,
        editor_keybinds: EditorKeybinds,
    ) -> Self {
        Self {
            handler: BOXED_EDITOR_HANDLER,
            state,
            focus_theme,
            defocus_theme,
            guide: text::State {
                text: Default::default(),
                style: Default::default(),
            },
            searcher,
            editor_keybinds,
        }
    }

    pub fn focus(&mut self) {
        self.state.prefix = self.focus_theme.prefix.clone();
        self.state.prefix_style = self.focus_theme.prefix_style;
        self.state.inactive_char_style = self.focus_theme.inactive_char_style;
        self.state.active_char_style = self.focus_theme.active_char_style;
    }

    pub fn defocus(&mut self) {
        self.state.prefix = self.defocus_theme.prefix.clone();
        self.state.prefix_style = self.defocus_theme.prefix_style;
        self.state.inactive_char_style = self.defocus_theme.inactive_char_style;
        self.state.active_char_style = self.defocus_theme.active_char_style;

        self.searcher.leave_search();
        self.handler = BOXED_EDITOR_HANDLER;

        self.guide.text = Default::default();
    }

    pub fn text(&self) -> String {
        self.state.texteditor.text_without_cursor().to_string()
    }

    pub fn create_editor_pane(&self, width: u16, height: u16) -> Pane {
        self.state.create_pane(width, height)
    }

    pub fn create_searcher_pane(&self, width: u16, height: u16) -> Pane {
        self.searcher.create_pane(width, height)
    }

    pub fn create_guide_pane(&self, width: u16, height: u16) -> Pane {
        self.guide.create_pane(width, height)
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
    editor.guide.text = Default::default();

    match event {
        key if editor.editor_keybinds.completion.matches(key) => {
            let prefix = editor.state.texteditor.text_without_cursor().to_string();
            match editor.searcher.start_search(&prefix) {
                Ok(result) => match result.head_item {
                    Some(head) => {
                        if result.load_state.loaded {
                            editor.guide.text = format!(
                                "Loaded all ({}) suggestions",
                                result.load_state.loaded_item_len
                            );
                            editor.guide.style = StyleBuilder::new().fgc(Color::Green).build();
                        } else {
                            editor.guide.text = format!(
                                "Loaded partially ({}) suggestions",
                                result.load_state.loaded_item_len
                            );
                            editor.guide.style = StyleBuilder::new().fgc(Color::Green).build();
                        }
                        editor.state.texteditor.replace(&head);
                        editor.handler = BOXED_SEARCHER_HANDLER;
                    }
                    None => {
                        editor.guide.text = format!("No suggestion found for '{}'", prefix);
                        editor.guide.style = StyleBuilder::new().fgc(Color::Yellow).build();
                    }
                },
                Err(e) => {
                    editor.guide.text = format!("Failed to lookup suggestions: {}", e);
                    editor.guide.style = StyleBuilder::new().fgc(Color::Yellow).build();
                }
            }
        }

        // Move cursor.
        key if editor.editor_keybinds.backward.matches(key) => {
            editor.state.texteditor.backward();
        }
        key if editor.editor_keybinds.forward.matches(key) => {
            editor.state.texteditor.forward();
        }
        key if editor.editor_keybinds.move_to_head.matches(key) => {
            editor.state.texteditor.move_to_head();
        }
        key if editor.editor_keybinds.move_to_tail.matches(key) => {
            editor.state.texteditor.move_to_tail();
        }

        // Move cursor to the nearest character.
        key if editor.editor_keybinds.move_to_previous_nearest.matches(key) => {
            editor
                .state
                .texteditor
                .move_to_previous_nearest(&editor.state.word_break_chars);
        }
        key if editor.editor_keybinds.move_to_next_nearest.matches(key) => {
            editor
                .state
                .texteditor
                .move_to_next_nearest(&editor.state.word_break_chars);
        }

        // Erase char(s).
        key if editor.editor_keybinds.erase.matches(key) => {
            editor.state.texteditor.erase();
        }
        key if editor.editor_keybinds.erase_all.matches(key) => {
            editor.state.texteditor.erase_all();
        }

        // Erase to the nearest character.
        key if editor
            .editor_keybinds
            .erase_to_previous_nearest
            .matches(key) =>
        {
            editor
                .state
                .texteditor
                .erase_to_previous_nearest(&editor.state.word_break_chars);
        }
        key if editor.editor_keybinds.erase_to_next_nearest.matches(key) => {
            editor
                .state
                .texteditor
                .erase_to_next_nearest(&editor.state.word_break_chars);
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
        }) => match editor.state.edit_mode {
            text_editor::Mode::Insert => editor.state.texteditor.insert(*ch),
            text_editor::Mode::Overwrite => editor.state.texteditor.overwrite(*ch),
        },

        _ => {}
    }
    Ok(())
}

pub async fn search<'a>(event: &'a Event, editor: &'a mut Editor) -> anyhow::Result<()> {
    match event {
        key if editor.editor_keybinds.on_completion.down.matches(key) => {
            editor.searcher.down_with_load();
            editor
                .state
                .texteditor
                .replace(&editor.searcher.get_current_item());
        }

        key if editor.editor_keybinds.on_completion.up.matches(key) => {
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
