use tokio::sync::mpsc::Sender;

use promkit::{
    crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    json, text_editor,
};
use promkit_async::EventBundle;

pub type JsonKeymap = fn(&[EventBundle], &mut json::State) -> anyhow::Result<()>;
pub type FilterEditorKeymap =
    fn(&[EventBundle], &mut text_editor::State, Sender<()>) -> anyhow::Result<bool>;

pub fn default_json(
    event_buffer: &[EventBundle],
    json_state: &mut json::State,
) -> anyhow::Result<()> {
    for event in event_buffer {
        match event {
            EventBundle::VerticalCursorBuffer(up, down) => {
                json_state.stream.shift(*up, *down);
            }
            EventBundle::Others(event, times) => match event {
                // Move up.
                Event::Key(KeyEvent {
                    code: KeyCode::Char('k'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                }) => {
                    json_state.stream.shift(*times, 0);
                }

                // Move down.
                Event::Key(KeyEvent {
                    code: KeyCode::Char('j'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                }) => {
                    json_state.stream.shift(0, *times);
                }

                // Move to tail
                Event::Key(KeyEvent {
                    code: KeyCode::Char('h'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                }) => {
                    json_state.stream.move_to_tail();
                }

                // Move to head
                Event::Key(KeyEvent {
                    code: KeyCode::Char('l'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                }) => {
                    json_state.stream.move_to_head();
                }

                // Toggle collapse/expand
                Event::Key(KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                }) => {
                    json_state.stream.toggle();
                }

                Event::Key(KeyEvent {
                    code: KeyCode::Char('p'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                }) => {
                    json_state.stream.expand_all();
                }

                Event::Key(KeyEvent {
                    code: KeyCode::Char('n'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                }) => {
                    json_state.stream.collapse_all();
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}

pub fn default_query_editor(
    event_buffer: &[EventBundle],
    state: &mut text_editor::State,
    fin_sender: Sender<()>,
) -> anyhow::Result<bool> {
    let prev = state.texteditor.text_without_cursor();
    for event in event_buffer {
        match event {
            EventBundle::KeyBuffer(chars) => match state.edit_mode {
                text_editor::Mode::Insert => state.texteditor.insert_chars(&chars),
                text_editor::Mode::Overwrite => state.texteditor.overwrite_chars(&chars),
            },
            EventBundle::HorizontalCursorBuffer(left, right) => {
                state.texteditor.shift(*left, *right);
            }
            EventBundle::Others(event, times) => {
                match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => {
                        fin_sender.try_send(())?;
                    }

                    Event::Key(KeyEvent {
                        code: KeyCode::Char('a'),
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => state.texteditor.move_to_head(),
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => state.texteditor.move_to_tail(),

                    Event::Key(KeyEvent {
                        code: KeyCode::Char('b'),
                        modifiers: KeyModifiers::ALT,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => {
                        for _ in 0..*times {
                            state
                                .texteditor
                                .move_to_previous_nearest(&state.word_break_chars);
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('f'),
                        modifiers: KeyModifiers::ALT,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => {
                        for _ in 0..*times {
                            state
                                .texteditor
                                .move_to_next_nearest(&state.word_break_chars)
                        }
                    }

                    // Erase char(s).
                    Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        modifiers: KeyModifiers::NONE,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => {
                        for _ in 0..*times {
                            state.texteditor.erase();
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('u'),
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => state.texteditor.erase_all(),

                    // Erase to the nearest character.
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('w'),
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => {
                        for _ in 0..*times {
                            state
                                .texteditor
                                .erase_to_previous_nearest(&state.word_break_chars);
                        }
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('d'),
                        modifiers: KeyModifiers::ALT,
                        kind: KeyEventKind::Press,
                        state: KeyEventState::NONE,
                    }) => {
                        for _ in 0..*times {
                            state
                                .texteditor
                                .erase_to_next_nearest(&state.word_break_chars);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(prev != state.texteditor.text_without_cursor())
}
