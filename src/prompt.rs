use std::{io, sync::Arc, time::Duration};

use futures::StreamExt;
use promkit_widgets::{
    core::{
        crossterm::{
            cursor,
            event::{
                DisableMouseCapture, EnableMouseCapture, Event, EventStream, MouseEvent,
                MouseEventKind,
            },
            execute,
            terminal::{self, disable_raw_mode, enable_raw_mode},
        },
        grapheme::StyledGraphemes,
        render::{Renderer, SharedRenderer},
    },
    spinner::{self, Spinner, State},
};
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    config::{JsonConfig, Keybinds, ReactivityControl},
    guide::{self, GuideAction, GuideMessage},
    json_viewer::{self, RenderTrigger, SharedContext},
    search::IncrementalSearcher,
    Editor,
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

fn empty_pane() -> StyledGraphemes {
    StyledGraphemes::default()
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

enum EditorAction {
    Focus(bool),
    CopyQuery,
    UserEvent(Event),
}

enum SearchAction {
    Start,
    UserEvent(Event),
    Leave,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Index {
    Editor = 0,
    Guide = 1,
    Search = 2,
    Processor = 3,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    item: &'static str,
    json_config: JsonConfig,
    reactivity_control: ReactivityControl,
    editor: Editor,
    searcher: IncrementalSearcher,
    no_hint: bool,
    keybinds: Keybinds,
    write_to_stdout: bool,
) -> anyhow::Result<Option<String>> {
    enable_raw_mode()?;
    execute!(io::stdout(), cursor::Hide)?;

    let size = terminal::size()?;

    let shared_renderer = SharedRenderer::new(
        Renderer::try_new_with_graphemes(
            [
                (Index::Editor, editor.create_editor_pane(size.0, size.1)),
                (Index::Guide, empty_pane()),
                (Index::Search, empty_pane()),
                (Index::Processor, empty_pane()),
            ]
            .into_iter(),
            true,
        )
        .await?,
    );

    let ctx = SharedContext::try_default()?;

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
            let _ = spinner::run(&spinner, ctx, Index::Processor, shared_renderer).await;
        }
    });

    let mut focus = Focus::Editor;
    let (editor_action_tx, mut editor_action_rx) = mpsc::channel::<EditorAction>(1);
    let (search_action_tx, mut search_action_rx) = mpsc::channel::<SearchAction>(1);
    let (json_viewer_action_tx, json_viewer_action_rx) =
        mpsc::channel::<json_viewer::ViewerAction>(8);
    let (guide_action_tx, guide_action_rx) = mpsc::channel::<GuideAction>(8);

    let text_diff = Arc::new(RwLock::new([editor.text(), editor.text()]));
    let shared_editor = Arc::new(RwLock::new(editor));
    let shared_searcher = Arc::new(RwLock::new(searcher));
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
                                    editor_action_tx.send(EditorAction::CopyQuery).await?;
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
                                            search_action_tx.send(SearchAction::Leave).await?;
                                            editor_action_tx.send(EditorAction::Focus(false)).await?;
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
                                        editor_action_tx.send(EditorAction::Focus(true)).await?;
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
                                    search_action_tx.send(SearchAction::Start).await?;
                                } else {
                                    editor_action_tx.send(EditorAction::UserEvent(event)).await?;
                                }
                            }
                            Focus::Searcher => {
                                if editor_keybinds.on_completion.down.contains(&event)
                                    || editor_keybinds.completion.contains(&event)
                                {
                                    search_action_tx
                                        .send(SearchAction::UserEvent(event))
                                        .await?;
                                } else if editor_keybinds.on_completion.up.contains(&event) {
                                    search_action_tx
                                        .send(SearchAction::UserEvent(event))
                                        .await?;
                                } else {
                                    focus = Focus::Editor;
                                    search_action_tx.send(SearchAction::Leave).await?;
                                    if !editor_keybinds.completion.contains(&event) {
                                        editor_action_tx
                                            .send(EditorAction::UserEvent(event))
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

    let guide_task = guide::start_guide_task(guide_action_rx, shared_renderer.clone(), no_hint);

    let editor_task: JoinHandle<anyhow::Result<()>> = {
        let shared_renderer = shared_renderer.clone();
        let shared_editor = shared_editor.clone();
        let shared_searcher = shared_searcher.clone();
        let text_diff = text_diff.clone();
        let debounce_query_tx = debounce_query_tx.clone();
        let guide_action_tx = guide_action_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(action) = editor_action_rx.recv() => {
                        let size = terminal::size()?;
                        let (editor_pane, searcher_pane, maybe_text_for_debounce) = {
                            let mut editor = shared_editor.write().await;
                            match action {
                                EditorAction::Focus(focus) => {
                                    let mut searcher = shared_searcher.write().await;
                                    if focus {
                                        editor.focus();
                                    } else {
                                        editor.defocus();
                                        searcher.leave_search();
                                    }
                                }
                                EditorAction::CopyQuery => {
                                    let message = guide::copy_to_clipboard_message(&editor.text());
                                    guide_action_tx.send(GuideAction::Show(message)).await?;
                                }
                                EditorAction::UserEvent(event) => {
                                    editor.operate(&event).await?;
                                }
                            }
                            let searcher = shared_searcher.read().await;
                            let current_text = editor.text();
                            (
                                editor.create_editor_pane(size.0, size.1),
                                searcher.create_pane(size.0, size.1),
                                current_text,
                            )
                        };
                        let mut diff = text_diff.write().await;
                        if maybe_text_for_debounce != diff[1] {
                            debounce_query_tx.send(maybe_text_for_debounce.clone()).await?;
                            diff[0] = diff[1].clone();
                            diff[1] = maybe_text_for_debounce;
                        }
                        shared_renderer.update([
                            (Index::Editor, editor_pane),
                            (Index::Search, searcher_pane),
                        ]).render().await?;
                    }
                    else => {
                        break
                    }
                }
            }
            Ok(())
        })
    };

    let searcher_task: JoinHandle<anyhow::Result<()>> = {
        let shared_renderer = shared_renderer.clone();
        let shared_editor = shared_editor.clone();
        let shared_searcher = shared_searcher.clone();
        let text_diff = text_diff.clone();
        let debounce_query_tx = debounce_query_tx.clone();
        let editor_keybinds = editor_keybinds.clone();
        let guide_action_tx = guide_action_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(action) = search_action_rx.recv() => {
                        let size = terminal::size()?;
                        let (editor_pane, searcher_pane) = {
                            let mut editor = shared_editor.write().await;
                            let mut searcher = shared_searcher.write().await;
                            match action {
                                SearchAction::Start => {
                                    let prefix = editor.text();
                                    let (head_item, load_progress) =
                                        searcher.start_search(&prefix).await;
                                    match head_item {
                                        Some(head) => {
                                            let message = if load_progress.is_complete {
                                                GuideMessage::LoadedAllSuggestions(
                                                    load_progress.loaded_path_count,
                                                )
                                            } else {
                                                GuideMessage::LoadedPartiallySuggestions(
                                                    load_progress.loaded_path_count,
                                                )
                                            };
                                            guide_action_tx.send(GuideAction::Show(message)).await?;
                                            editor.replace_text(&head);
                                        }
                                        None => {
                                            guide_action_tx.send(GuideAction::Show(
                                                GuideMessage::NoSuggestionFound(prefix),
                                            )).await?;
                                        }
                                    }
                                }
                                SearchAction::UserEvent(event) => {
                                    if editor_keybinds.on_completion.down.contains(&event)
                                        || editor_keybinds.completion.contains(&event)
                                    {
                                        searcher.down_with_load();
                                        editor.replace_text(&searcher.get_current_item());
                                    } else if editor_keybinds.on_completion.up.contains(&event) {
                                        searcher.up();
                                        editor.replace_text(&searcher.get_current_item());
                                    }
                                }
                                SearchAction::Leave => {
                                    searcher.leave_search();
                                }
                            }

                            let current_text = editor.text();
                            let mut diff = text_diff.write().await;
                            if current_text != diff[1] {
                                debounce_query_tx.send(current_text.clone()).await?;
                                diff[0] = diff[1].clone();
                                diff[1] = current_text;
                            }
                            (
                                editor.create_editor_pane(size.0, size.1),
                                searcher.create_pane(size.0, size.1),
                            )
                        };

                        shared_renderer.update([
                            (Index::Editor, editor_pane),
                            (Index::Search, searcher_pane),
                        ]).render().await?;
                    }
                    else => break,
                }
            }
            Ok(())
        })
    };

    let shared_viewer_state = initializing.await?;
    let resize_action_forwarder = {
        let shared_renderer = shared_renderer.clone();
        let shared_editor = shared_editor.clone();
        let shared_searcher = shared_searcher.clone();
        let shared_viewer_state = shared_viewer_state.clone();
        let ctx = ctx.clone();
        let guide_action_tx = guide_action_tx.clone();
        tokio::spawn(async move {
            while let Some(area) = last_resize_rx.recv().await {
                let size = terminal::size()?;
                let (editor_pane, searcher_pane) = {
                    let editor = shared_editor.read().await;
                    let searcher = shared_searcher.read().await;
                    (
                        editor.create_editor_pane(size.0, size.1),
                        searcher.create_pane(size.0, size.1),
                    )
                };
                shared_renderer
                    .update([(Index::Editor, editor_pane), (Index::Search, searcher_pane)])
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
                    RenderTrigger::AreaResized { area, query: text },
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
    searcher_task.abort();
    processor_task.abort();

    execute!(io::stdout(), cursor::Show, DisableMouseCapture)?;
    disable_raw_mode()?;

    Ok(output)
}
