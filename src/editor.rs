use std::{future::Future, pin::Pin};

use promkit_widgets::{
    core::{
        crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
        grapheme::StyledGraphemes,
        Widget,
    },
    status::{self, Severity},
    text_editor,
};

use crate::{
    config::EditorKeybinds,
    search::{IncrementalSearcher, SharedSuggestionStore},
};

pub struct Editor {
    handler: Handler,
    state: text_editor::State,
    focus_config: text_editor::Config,
    defocus_config: text_editor::Config,
    guide: status::State,
    shared_suggestions: SharedSuggestionStore,
    editor_keybinds: EditorKeybinds,
}

impl Editor {
    pub fn new(
        state: text_editor::State,
        shared_suggestions: SharedSuggestionStore,
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
            shared_suggestions,
            editor_keybinds,
        }
    }

    pub fn focus(&mut self) {
        self.state.config = self.focus_config.clone();
    }

    pub fn defocus(&mut self, searcher: &mut IncrementalSearcher) {
        self.state.config = self.defocus_config.clone();

        searcher.leave_search();
        self.handler = BOXED_EDITOR_HANDLER;

        self.guide = status::State::default();
    }

    pub fn text(&self) -> String {
        self.state.texteditor.text_without_cursor().to_string()
    }

    pub fn create_editor_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.state.create_graphemes(width, height)
    }

    pub fn create_searcher_pane(
        &self,
        searcher: &IncrementalSearcher,
        width: u16,
        height: u16,
    ) -> StyledGraphemes {
        searcher.create_pane(width, height)
    }

    pub fn create_guide_pane(&self, width: u16, height: u16) -> StyledGraphemes {
        self.guide.create_graphemes(width, height)
    }

    pub async fn operate(
        &mut self,
        event: &Event,
        searcher: &mut IncrementalSearcher,
    ) -> anyhow::Result<()> {
        (self.handler)(event, self, searcher).await
    }
}

pub type Handler = for<'a> fn(
    &'a Event,
    &'a mut Editor,
    &'a mut IncrementalSearcher,
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

const BOXED_EDITOR_HANDLER: Handler =
    |event, editor, searcher| -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(edit(event, editor, searcher))
    };
const BOXED_SEARCHER_HANDLER: Handler =
    |event, editor, searcher| -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(search(event, editor, searcher))
    };

pub async fn edit<'a>(
    event: &'a Event,
    editor: &'a mut Editor,
    searcher: &'a mut IncrementalSearcher,
) -> anyhow::Result<()> {
    editor.guide = status::State::default();

    match event {
        key if editor.editor_keybinds.completion.contains(key) => {
            let prefix = editor.state.texteditor.text_without_cursor().to_string();
            let (items, _) = editor.shared_suggestions.collect_matches(&prefix).await;
            let progress = searcher.load_progress().await;
            match searcher.apply_search_items(items) {
                Some(head) => {
                    if progress.is_complete {
                        editor.guide = status::State::new(
                            format!("Loaded all ({}) suggestions", progress.loaded_path_count),
                            Severity::Success,
                        );
                    } else {
                        editor.guide = status::State::new(
                            format!(
                                "Loaded partially ({}) suggestions",
                                progress.loaded_path_count
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

pub async fn search<'a>(
    event: &'a Event,
    editor: &'a mut Editor,
    searcher: &'a mut IncrementalSearcher,
) -> anyhow::Result<()> {
    match event {
        key if editor.editor_keybinds.on_completion.down.contains(key) => {
            searcher.down_with_load();
            editor
                .state
                .texteditor
                .replace(&searcher.get_current_item());
        }

        key if editor.editor_keybinds.on_completion.up.contains(key) => {
            searcher.up();
            editor
                .state
                .texteditor
                .replace(&searcher.get_current_item());
        }

        _ => {
            searcher.leave_search();
            editor.handler = BOXED_EDITOR_HANDLER;
            return edit(event, editor, searcher).await;
        }
    }

    Ok(())
}
