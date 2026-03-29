use std::{io, sync::Arc, time::Duration};

use futures::StreamExt;
use promkit_widgets::{
    core::{
        crossterm::{
            event::{
                DisableMouseCapture, EnableMouseCapture, Event, EventStream, MouseEvent,
                MouseEventKind,
            },
            execute, terminal,
        },
        render::SharedRenderer,
    },
    spinner::{self, Spinner, State},
};
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    completion::{self, CompletionAction, CompletionNavigator},
    config::{JsonConfig, Keybinds, ReactivityControl},
    guide::{self, GuideAction, GuideMessage},
    json_viewer::{self, RenderTrigger, SharedContext},
    query_editor::{self, QueryEditor, QueryEditorAction},
};

fn spawn_debouncer<T: Send + 'static>(
    mut debounce_rx: mpsc::Receiver<T>,
    last_tx: mpsc::Sender<T>,
    duration: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_query = None;
        let mut delay = tokio::time::interval(duration);
        loop {
            tokio::select! {
                maybe_query = debounce_rx.recv() => {
                    if let Some(query) = maybe_query {
                        last_query = Some(query);
                    } else {
                        break;
                    }
                },
                _ = delay.tick() => {
                    if let Some(text) = last_query.take() {
                        let _ = last_tx.send(text).await;
                    }
                },
            }
        }
    })
}

#[derive(Clone, Copy)]
enum Focus {
    Editor,
    Searcher,
    Processor,
}

enum GlobalAction {
    Resize(u16, u16),
    Exit,
    CopyQuery,
    CopyResult,
    SwitchMode,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Index {
    QueryEditor = 0,
    Guide = 1,
    Completion = 2,
    JsonViewer = 3,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    item: &'static str,
    ctx: SharedContext,
    shared_renderer: SharedRenderer<Index>,
    json_config: JsonConfig,
    reactivity_control: ReactivityControl,
    editor: QueryEditor,
    completion: CompletionNavigator,
    no_hint: bool,
    keybinds: Keybinds,
    write_to_stdout: bool,
) -> anyhow::Result<Option<String>> {
    let (last_query_tx, mut last_query_rx) = mpsc::channel(1);
    let (debounce_query_tx, debounce_query_rx) = mpsc::channel(1);
    let query_debouncer = spawn_debouncer(
        debounce_query_rx,
        last_query_tx,
        reactivity_control.query_debounce_duration,
    );
    if !editor.text().is_empty() {
        debounce_query_tx.send(editor.text()).await?;
    }

    let (last_resize_tx, mut last_resize_rx) = mpsc::channel::<(u16, u16)>(1);
    let (debounce_resize_tx, debounce_resize_rx) = mpsc::channel(1);
    let resize_debouncer = spawn_debouncer(
        debounce_resize_rx,
        last_resize_tx,
        reactivity_control.resize_debounce_duration,
    );

    let spinning = tokio::spawn({
        let shared_renderer = shared_renderer.clone();
        let ctx = ctx.clone();
        let spin_duration = reactivity_control.spin_duration;
        async move {
            let spinner = Spinner::default().duration(spin_duration);
            let _ = spinner::run(&spinner, ctx, Index::JsonViewer, shared_renderer).await;
        }
    });

    let mut focus = Focus::Editor;
    let (editor_action_tx, editor_action_rx) = mpsc::channel::<QueryEditorAction>(1);
    let (completion_action_tx, completion_action_rx) = mpsc::channel::<CompletionAction>(1);
    let (json_viewer_action_tx, json_viewer_action_rx) =
        mpsc::channel::<json_viewer::ViewerAction>(8);
    let (guide_action_tx, guide_action_rx) = mpsc::channel::<GuideAction>(8);

    let shared_editor = Arc::new(RwLock::new(editor));
    let shared_completion = Arc::new(RwLock::new(completion));
    let editor_keybinds = keybinds.on_editor.clone();
    let initializing = json_viewer::initialize(
        item,
        json_config,
        keybinds.on_json_viewer,
        shared_renderer.clone(),
        ctx.clone(),
    );

    let main_task: JoinHandle<anyhow::Result<()>> = {
        let mut stream = EventStream::new();
        let ctx = ctx.clone();
        let shared_editor = shared_editor.clone();
        let editor_action_tx = editor_action_tx.clone();
        let editor_keybinds = editor_keybinds.clone();
        let json_viewer_action_tx = json_viewer_action_tx.clone();
        let guide_action_tx = guide_action_tx.clone();
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

                        let global_action = if let Event::Resize(width, height) = event {
                            Some(GlobalAction::Resize(width, height))
                        } else if keybinds.exit.contains(&event) {
                            Some(GlobalAction::Exit)
                        } else if keybinds.copy_query.contains(&event) {
                            Some(GlobalAction::CopyQuery)
                        } else if keybinds.copy_result.contains(&event) {
                            Some(GlobalAction::CopyResult)
                        } else if keybinds.switch_mode.contains(&event) {
                            Some(GlobalAction::SwitchMode)
                        } else {
                            None
                        };

                        if let Some(action) = global_action {
                            match action {
                                GlobalAction::Resize(width, height) => {
                                    debounce_resize_tx.send((width, height)).await?;
                                }
                                GlobalAction::Exit => break 'main,
                                GlobalAction::CopyQuery => {
                                    editor_action_tx.send(QueryEditorAction::CopyQuery).await?;
                                }
                                GlobalAction::CopyResult => {
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
                                GlobalAction::SwitchMode => match focus {
                                    Focus::Editor | Focus::Searcher => {
                                        if ctx.is_idle().await {
                                            focus = Focus::Processor;
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
                                    Focus::Processor => {
                                        focus = Focus::Editor;
                                        editor_action_tx.send(QueryEditorAction::Enter).await?;
                                        execute!(
                                            io::stdout(),
                                            terminal::LeaveAlternateScreen,
                                            DisableMouseCapture,
                                        )?;
                                    }
                                },
                            }
                            continue;
                        }

                        match focus {
                            Focus::Editor => {
                                if editor_keybinds.completion.contains(&event) {
                                    focus = Focus::Searcher;
                                    let prefix = {
                                        let editor = shared_editor.read().await;
                                        editor.text()
                                    };
                                    completion_action_tx
                                        .send(CompletionAction::Enter { prefix })
                                        .await?;
                                } else {
                                    editor_action_tx
                                        .send(QueryEditorAction::UserEvent(event))
                                        .await?;
                                }
                            }
                            Focus::Searcher => {
                                if editor_keybinds.on_completion.down.contains(&event)
                                {
                                    completion_action_tx
                                        .send(CompletionAction::UserEvent(event))
                                        .await?;
                                } else if editor_keybinds.on_completion.up.contains(&event) {
                                    completion_action_tx
                                        .send(CompletionAction::UserEvent(event))
                                        .await?;
                                } else {
                                    focus = Focus::Editor;
                                    completion_action_tx.send(CompletionAction::Leave).await?;
                                    if !editor_keybinds.completion.contains(&event) {
                                        editor_action_tx
                                            .send(QueryEditorAction::UserEvent(event))
                                            .await?;
                                    }
                                }
                            }
                            Focus::Processor => {
                                json_viewer_action_tx
                                    .send(json_viewer::ViewerAction::UserEvent(event))
                                    .await?;
                            }
                        }
                    },
                    else => {
                        break 'main;
                    }
                }
            }
            Ok(())
        })
    };

    let query_action_forwarder = {
        let json_viewer_action_tx = json_viewer_action_tx.clone();
        tokio::spawn(async move {
            while let Some(query) = last_query_rx.recv().await {
                let _ = json_viewer_action_tx
                    .send(json_viewer::ViewerAction::QueryChanged(query))
                    .await;
            }
        })
    };

    let guide_task = guide::start_guide_task(
        guide_action_rx,
        shared_renderer.clone(),
        ctx.clone(),
        no_hint,
    );

    let editor_task = query_editor::start_query_editor_task(
        editor_action_rx,
        shared_renderer.clone(),
        shared_editor.clone(),
        ctx.clone(),
        debounce_query_tx.clone(),
        guide_action_tx.clone(),
    );

    let completion_task = completion::start_completion_task(
        completion_action_rx,
        shared_renderer.clone(),
        shared_completion.clone(),
        ctx.clone(),
        editor_action_tx.clone(),
        guide_action_tx.clone(),
        editor_keybinds.on_completion,
    );

    let shared_viewer_state = initializing.await?;
    let resize_action_forwarder = {
        let shared_renderer = shared_renderer.clone();
        let shared_editor = shared_editor.clone();
        let shared_completion = shared_completion.clone();
        let shared_viewer_state = shared_viewer_state.clone();
        let ctx = ctx.clone();
        let guide_action_tx = guide_action_tx.clone();
        tokio::spawn(async move {
            while let Some(area) = last_resize_rx.recv().await {
                ctx.set_area(area).await;
                let (editor_pane, completion_pane) = {
                    let editor = shared_editor.read().await;
                    let completion = shared_completion.read().await;
                    (
                        editor.create_graphemes(area.0, area.1),
                        completion.create_graphemes(area.0, area.1),
                    )
                };
                shared_renderer
                    .update([
                        (Index::QueryEditor, editor_pane),
                        (Index::Completion, completion_pane),
                    ])
                    .render()
                    .await?;
                let text = {
                    let editor = shared_editor.read().await;
                    editor.text()
                };
                json_viewer::render(
                    shared_viewer_state.clone(),
                    shared_renderer.clone(),
                    ctx.clone(),
                    guide_action_tx.clone(),
                    RenderTrigger::AreaResized { query: text },
                )
                .await;
            }
            Ok::<(), anyhow::Error>(())
        })
    };

    let processor_task = json_viewer::start_viewer_task(
        json_viewer_action_rx,
        guide_action_tx.clone(),
        shared_viewer_state.clone(),
        shared_renderer.clone(),
        ctx.clone(),
    );

    main_task.await??;

    let output = if write_to_stdout {
        let runtime = shared_viewer_state.lock().await;
        Some(runtime.formatted_content())
    } else {
        None
    };

    spinning.abort();
    query_debouncer.abort();
    resize_debouncer.abort();
    query_action_forwarder.abort();
    resize_action_forwarder.abort();
    guide_task.abort();
    editor_task.abort();
    completion_task.abort();
    processor_task.abort();

    Ok(output)
}
