use std::io;

use futures::StreamExt;
use promkit_widgets::{
    core::crossterm::{
        event::{
            DisableMouseCapture, EnableMouseCapture, Event, EventStream, MouseEvent, MouseEventKind,
        },
        execute, terminal,
    },
    spinner::State,
};
use tokio::{
    sync::mpsc,
    task::JoinHandle,
};

use crate::{
    completion::CompletionAction,
    config::Keybinds,
    context::SharedContext,
    guide::{GuideAction, GuideMessage},
    json_viewer,
    prompt::Index,
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
                    // Note: `HashSet<Event>::contains` compares full mouse events (including `column`/`row`),
                    // so wheel events are normalized to `(0, 0)` to match configured `ScrollUp`/`ScrollDown` bindings.
                    let event = match event {
                        Event::Mouse(mouse)
                            if matches!(
                                mouse.kind,
                                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                            ) =>
                        {
                            Event::Mouse(MouseEvent {
                                kind: mouse.kind,
                                column: 0,
                                row: 0,
                                modifiers: mouse.modifiers,
                            })
                        }
                        other => other,
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
                                                GuideMessage::FailedToSwitchPaneWhileRenderingInProgress,
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
