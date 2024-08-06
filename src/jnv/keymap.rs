use promkit::{
    crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    listbox::Listbox,
    text_editor, PromptSignal,
};

pub type Keymap = fn(&Event, &mut crate::jnv::Jnv) -> anyhow::Result<PromptSignal>;

pub fn default(event: &Event, jnv: &mut crate::jnv::Jnv) -> anyhow::Result<PromptSignal> {
    let filter_editor = jnv.filter_editor.after_mut();

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            let query = filter_editor.texteditor.text_without_cursor().to_string();
            if let Some(mut candidates) = jnv.suggest.prefix_search(query) {
                candidates.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));

                jnv.suggestions.listbox = Listbox::from_iter(candidates);
                filter_editor
                    .texteditor
                    .replace(&jnv.suggestions.listbox.get().to_string());

                jnv.keymap.borrow_mut().switch("on_suggest");
            }
        }

        Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => return Ok(PromptSignal::Quit),

        // Move cursor.
        Event::Key(KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            filter_editor.texteditor.backward();
        }
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            filter_editor.texteditor.forward();
        }
        Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => filter_editor.texteditor.move_to_head(),
        Event::Key(KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => filter_editor.texteditor.move_to_tail(),

        Event::Key(KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::ALT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => filter_editor
            .texteditor
            .move_to_previous_nearest(&filter_editor.word_break_chars),

        Event::Key(KeyEvent {
            code: KeyCode::Char('f'),
            modifiers: KeyModifiers::ALT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => filter_editor
            .texteditor
            .move_to_next_nearest(&filter_editor.word_break_chars),

        // Erase char(s).
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => filter_editor.texteditor.erase(),
        Event::Key(KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => filter_editor.texteditor.erase_all(),

        // Erase to the nearest character.
        Event::Key(KeyEvent {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => filter_editor
            .texteditor
            .erase_to_previous_nearest(&filter_editor.word_break_chars),

        Event::Key(KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::ALT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => filter_editor
            .texteditor
            .erase_to_next_nearest(&filter_editor.word_break_chars),

        // Move up.
        Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.json.stream.backward();
        }

        // Move down.
        Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.json.stream.forward();
        }

        // Move to tail
        Event::Key(KeyEvent {
            code: KeyCode::Char('h'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.json.stream.move_to_tail();
        }

        // Move to head
        Event::Key(KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.json.stream.move_to_head();
        }

        // Toggle collapse/expand
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.json.stream.toggle();
        }

        Event::Key(KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.json.stream.expand_all();
        }

        Event::Key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.store_content_to_clipboard();
        }

        Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.store_query_to_clipboard();
        }

        Event::Key(KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.json.stream.collapse_all();
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
        }) => match filter_editor.edit_mode {
            text_editor::Mode::Insert => filter_editor.texteditor.insert(*ch),
            text_editor::Mode::Overwrite => filter_editor.texteditor.overwrite(*ch),
        },

        _ => (),
    }
    Ok(PromptSignal::Continue)
}

pub fn on_suggest(event: &Event, jnv: &mut crate::jnv::Jnv) -> anyhow::Result<PromptSignal> {
    let query_editor_after_mut = jnv.filter_editor.after_mut();

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => return Ok(PromptSignal::Quit),

        Event::Key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.suggestions.listbox.forward();
            query_editor_after_mut
                .texteditor
                .replace(&jnv.suggestions.listbox.get().to_string());
        }

        Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            jnv.suggestions.listbox.backward();
            query_editor_after_mut
                .texteditor
                .replace(&jnv.suggestions.listbox.get().to_string());
        }

        _ => {
            jnv.suggestions.listbox = Listbox::from_iter(Vec::<String>::new());
            jnv.keymap.borrow_mut().switch("default");

            // This block is specifically designed to prevent the default action of toggling collapse/expand
            // from being executed when the Enter key is pressed. This is done from the perspective of user
            // experimentation, ensuring that pressing Enter while in the suggest mode does not trigger
            // the default behavior associated with the Enter key in the default mode.
            if let Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) = event
            {
            } else {
                return default(event, jnv);
            }
        }
    }
    Ok(PromptSignal::Continue)
}
