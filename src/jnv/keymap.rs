use super::editing::add_to_nearest_integer;
use promkit::{
    crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    listbox::Listbox,
    text_editor, PromptSignal, Result,
};

pub fn default(event: &Event, renderer: &mut crate::jnv::render::Renderer) -> Result<PromptSignal> {
    let query_editor_after_mut = renderer.query_editor_snapshot.after_mut();
    let suggest_after_mut = renderer.suggest_snapshot.after_mut();
    let json_after_mut = renderer.json_snapshot.after_mut();

    match event {
        Event::Key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            let query = query_editor_after_mut
                .texteditor
                .text_without_cursor()
                .to_string();
            if let Some(mut candidates) = renderer.suggest.prefix_search(query) {
                candidates.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));

                suggest_after_mut.listbox = Listbox::from_iter(candidates);
                query_editor_after_mut
                    .texteditor
                    .replace(&suggest_after_mut.listbox.get());

                renderer.keymap.switch("on_suggest");
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
            query_editor_after_mut.texteditor.backward();
        }
        Event::Key(KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            query_editor_after_mut.texteditor.forward();
        }
        Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => query_editor_after_mut.texteditor.move_to_head(),
        Event::Key(KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => query_editor_after_mut.texteditor.move_to_tail(),

        // Erase char(s).
        Event::Key(KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => query_editor_after_mut.texteditor.erase(),
        Event::Key(KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => query_editor_after_mut.texteditor.erase_all(),

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
            json_after_mut.stream.backward();
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
            json_after_mut.stream.forward();
        }

        // Move to tail
        Event::Key(KeyEvent {
            code: KeyCode::Char('h'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            json_after_mut.stream.move_to_tail();
        }

        // Move to head
        Event::Key(KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            json_after_mut.stream.move_to_head();
        }

        // Toggle collapse/expand
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            json_after_mut.stream.toggle();
        }

        Event::Key(KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            json_after_mut.stream.expand_all();
        }

        Event::Key(KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            json_after_mut.stream.collapse_all();
        }

        Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            add_to_nearest_integer(query_editor_after_mut, 1);
        }
        Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            add_to_nearest_integer(query_editor_after_mut, -1);
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
        }) => match query_editor_after_mut.edit_mode {
            text_editor::Mode::Insert => query_editor_after_mut.texteditor.insert(*ch),
            text_editor::Mode::Overwrite => query_editor_after_mut.texteditor.overwrite(*ch),
        },

        _ => (),
    }
    Ok(PromptSignal::Continue)
}

pub fn on_suggest(
    event: &Event,
    renderer: &mut crate::jnv::render::Renderer,
) -> Result<PromptSignal> {
    let query_editor_after_mut = renderer.query_editor_snapshot.after_mut();
    let suggest_after_mut = renderer.suggest_snapshot.after_mut();

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
            suggest_after_mut.listbox.forward();
            query_editor_after_mut
                .texteditor
                .replace(&suggest_after_mut.listbox.get());
        }

        Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            suggest_after_mut.listbox.backward();
            query_editor_after_mut
                .texteditor
                .replace(&suggest_after_mut.listbox.get());
        }

        _ => {
            suggest_after_mut.listbox = Listbox::from_iter(Vec::<String>::new());
            renderer.keymap.switch("default");

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
                return default(event, renderer);
            }
        }
    }
    Ok(PromptSignal::Continue)
}
