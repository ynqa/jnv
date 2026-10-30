use std::io;

use futures::StreamExt;
use promkit_widgets::{
    core::crossterm::{
        event::{
            DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEvent, KeyEventKind,
            KeyEventState, MouseEvent, MouseEventKind,
        },
        execute, terminal,
    },
    spinner::State,
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    completion::CompletionAction,
    config::Keybinds,
    context::{Index, SharedContext},
    guide::{GuideAction, GuideMessage},
    json_viewer,
    query_editor::QueryEditorAction,
};

/// Actions that can be triggered by terminal events,
/// which are dispatched to the appropriate components.
enum Action {
    Resize(u16, u16),
    Exit,
    CopyQuery,
    CopyResult,
    /// Switch between query-editor/completion and JSON viewer.
    SwitchMode,
}

/// Canonicalize a terminal event so it can be matched against configured
/// keybinds, which rely on exact [`Event`] equality.
///
/// - Key events: only `Press` is actionable; `Release`/`Repeat` (emitted when
///   the keyboard enhancement protocol is active) return `None`. The
///   [`KeyEventState`] flags (e.g. Caps/Num Lock) are stripped so a binding
///   matches regardless of lock state.
/// - Mouse wheel events: `column`/`row` are zeroed so they match the
///   position-agnostic `ScrollUp`/`ScrollDown` bindings.
fn normalize_event(event: Event) -> Option<Event> {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return None;
            }
            Some(Event::Key(KeyEvent {
                state: KeyEventState::NONE,
                ..key
            }))
        }
        Event::Mouse(mouse)
            if matches!(
                mouse.kind,
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            ) =>
        {
            Some(Event::Mouse(MouseEvent {
                kind: mouse.kind,
                column: 0,
                row: 0,
                modifiers: mouse.modifiers,
            }))
        }
        other => Some(other),
    }
}

/// Spawn a background task to listen for terminal events and dispatch corresponding actions
/// to the appropriate components (query editor, completion navigator, JSON viewer, guide).
pub fn spawn_terminal_event_dispatch_task(
    ctx: SharedContext,
    keybinds: Keybinds,
    debounce_resize_tx: mpsc::Sender<(u16, u16)>,
    editor_action_tx: mpsc::Sender<QueryEditorAction>,
    completion_action_tx: mpsc::Sender<CompletionAction>,
    json_viewer_action_tx: mpsc::Sender<json_viewer::ViewerAction>,
    guide_action_tx: mpsc::Sender<GuideAction>,
) -> JoinHandle<anyhow::Result<()>> {
    let mut stream = EventStream::new();
    tokio::spawn(async move {
        'main: loop {
            tokio::select! {
                Some(Ok(event)) = stream.next() => {
                    // Keybinds are matched by exact `Event` equality, so incoming
                    // events must be canonicalized first. `None` means the event is
                    // not actionable (e.g. a key release) and should be ignored.
                    let event = match normalize_event(event) {
                        Some(event) => event,
                        None => continue,
                    };
                    guide_action_tx.send(GuideAction::Clear).await?;

                    let action = if let Event::Resize(width, height) = event {
                        Some(Action::Resize(width, height))
                    } else if keybinds.exit.contains(&event) {
                        Some(Action::Exit)
                    } else if keybinds.copy_query.contains(&event) {
                        Some(Action::CopyQuery)
                    } else if keybinds.copy_result.contains(&event) {
                        Some(Action::CopyResult)
                    } else if keybinds.switch_mode.contains(&event) {
                        Some(Action::SwitchMode)
                    } else {
                        None
                    };

                    if let Some(action) = action {
                        match action {
                            Action::Resize(width, height) => {
                                debounce_resize_tx.send((width, height)).await?;
                            }
                            Action::Exit => break 'main,
                            Action::CopyQuery => {
                                editor_action_tx.send(QueryEditorAction::CopyQuery).await?;
                            }
                            Action::CopyResult => {
                                if ctx.is_idle().await {
                                    json_viewer_action_tx
                                        .send(json_viewer::ViewerAction::CopyResult)
                                        .await?;
                                } else {
                                    guide_action_tx
                                        .send(GuideAction::Show(
                                            GuideMessage::FailedToCopyWhileRenderingInProgress,
                                        ))
                                        .await?;
                                }
                            }
                            Action::SwitchMode => match ctx.active_index().await {
                                Index::QueryEditor | Index::Completion => {
                                    if ctx.is_idle().await {
                                        ctx.set_active_index(Index::JsonViewer).await;
                                        completion_action_tx.send(CompletionAction::Leave).await?;
                                        editor_action_tx.send(QueryEditorAction::Leave).await?;
                                        execute!(
                                            io::stdout(),
                                            terminal::EnterAlternateScreen,
                                            EnableMouseCapture,
                                        )?;
                                    } else {
                                        guide_action_tx
                                            .send(GuideAction::Show(
                                                GuideMessage::FailedToSwitchModeWhileRenderingInProgress,
                                        ))
                                        .await?;
                                    }
                                }
                                Index::JsonViewer => {
                                    ctx.set_active_index(Index::QueryEditor).await;
                                    editor_action_tx.send(QueryEditorAction::Enter).await?;
                                    execute!(
                                        io::stdout(),
                                        terminal::LeaveAlternateScreen,
                                        DisableMouseCapture,
                                    )?;
                                }
                                Index::Guide => {}
                            },
                        }
                        continue;
                    }

                    match ctx.active_index().await {
                        Index::QueryEditor => {
                            editor_action_tx
                                .send(QueryEditorAction::UserEvent(event))
                                .await?;
                        }
                        Index::Completion => {
                            completion_action_tx
                                .send(CompletionAction::UserEvent(event))
                                .await?;
                        }
                        Index::JsonViewer => {
                            json_viewer_action_tx
                                .send(json_viewer::ViewerAction::UserEvent(event))
                                .await?;
                        }
                        Index::Guide => {}
                    }
                },
                else => {
                    break 'main;
                }
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use promkit_widgets::core::crossterm::event::{
        KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    };

    use super::*;

    fn key(kind: KeyEventKind, state: KeyEventState) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::CONTROL,
            kind,
            state,
        })
    }

    #[test]
    fn drops_non_press_key_events() {
        assert!(normalize_event(key(KeyEventKind::Release, KeyEventState::NONE)).is_none());
        assert!(normalize_event(key(KeyEventKind::Repeat, KeyEventState::NONE)).is_none());
    }

    #[test]
    fn strips_lock_state_from_press_events() {
        let normalized = normalize_event(key(KeyEventKind::Press, KeyEventState::CAPS_LOCK));
        assert_eq!(
            normalized,
            Some(key(KeyEventKind::Press, KeyEventState::NONE))
        );
    }

    #[test]
    fn zeroes_scroll_position() {
        let scroll = Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 42,
            row: 7,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            normalize_event(scroll),
            Some(Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            }))
        );
    }

    #[test]
    fn passes_through_other_mouse_events() {
        let click = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: 4,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(normalize_event(click.clone()), Some(click));
    }
}
