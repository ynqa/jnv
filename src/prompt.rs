use std::{io, sync::Arc, time::Duration};

use arboard::Clipboard;
use futures::StreamExt;
use promkit_widgets::{
    core::{
        Widget,
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
    spinner::{self, Spinner},
    status::{self, Severity},
};
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    config::{Keybinds, ReactivityControl},
    Context, ContextMonitor, Editor, Processor, SearchProvider, ViewInitializer,
    ViewProvider, Visualizer,
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

fn copy_to_clipboard(content: &str) -> status::State {
    match Clipboard::new() {
        Ok(mut clipboard) => match clipboard.set_text(content) {
            Ok(_) => status::State::new("Copied to clipboard", Severity::Success),
            Err(e) => {
                status::State::new(format!("Failed to copy to clipboard: {e}"), Severity::Error)
            }
        },
        // arboard fails (in the specific environment like linux?) on Clipboard::new()
        // suppress the errors (but still show them) not to break the prompt
        // https://github.com/1Password/arboard/issues/153
        Err(e) => status::State::new(format!("Failed to setup clipboard: {e}"), Severity::Error),
    }
}

fn empty_pane() -> StyledGraphemes {
    StyledGraphemes::default()
}

enum Focus {
    Editor,
    Processor,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Index {
    Editor = 0,
    Guide = 1,
    Search = 2,
    Processor = 3,
}

#[allow(clippy::too_many_arguments)]
pub async fn run<T: ViewProvider + SearchProvider>(
    item: &'static str,
    reactivity_control: ReactivityControl,
    provider: &mut T,
    editor: Editor,
    loading_suggestions_task: JoinHandle<anyhow::Result<()>>,
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

    let ctx = Arc::new(Mutex::new(Context::new(size)));

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
        let state = ContextMonitor::new(ctx.clone());
        let spin_duration = reactivity_control.spin_duration;
        async move {
            let spinner = Spinner::default().duration(spin_duration);
            let _ = spinner::run(&spinner, state, Index::Processor, shared_renderer).await;
        }
    });

    let mut focus = Focus::Editor;
    let (editor_event_tx, mut editor_event_rx) = mpsc::channel::<Event>(1);
    let (processor_event_tx, mut processor_event_rx) = mpsc::channel::<Event>(1);

    let (editor_copy_tx, mut editor_copy_rx) = mpsc::channel::<()>(1);
    let (processor_copy_tx, mut processor_copy_rx) = mpsc::channel::<()>(1);

    let (editor_focus_tx, mut editor_focus_rx) = mpsc::channel::<bool>(1);

    let mut text_diff = [editor.text(), editor.text()];
    let shared_editor = Arc::new(RwLock::new(editor));
    let processor = Processor::new(ctx.clone());
    let context_monitor = ContextMonitor::new(ctx.clone());
    let initializer = ViewInitializer::new(ctx.clone());
    let initializing = initializer.initialize(
        provider,
        item,
        size,
        shared_renderer.clone(),
        keybinds.on_json_viewer,
    );

    let main_task: JoinHandle<anyhow::Result<()>> = {
        let mut stream = EventStream::new();
        let shared_renderer = shared_renderer.clone();
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

                        match event {
                            Event::Resize(width, height) => {
                                debounce_resize_tx.send((width, height)).await?;
                            },
                            event if keybinds.exit.contains(&event) => {
                                break 'main
                            },
                            event if keybinds.copy_query.contains(&event) => {
                                editor_copy_tx.send(()).await?;
                            },
                            event if keybinds.copy_result.contains(&event) => {
                                if context_monitor.is_idle().await {
                                    processor_copy_tx.send(()).await?;
                                } else if !no_hint{
                                    let size = terminal::size()?;
                                    shared_renderer.update([
                                        (
                                            Index::Guide,
                                            status::State::new(
                                                "Failed to copy while rendering is in progress.",
                                                Severity::Warning,
                                            )
                                            .create_graphemes(size.0, size.1),
                                        ),
                                    ]).render().await?;
                                }
                            },
                            event if keybinds.switch_mode.contains(&event) => {
                                match focus {
                                    Focus::Editor => {
                                        if context_monitor.is_idle().await {
                                            focus = Focus::Processor;
                                            editor_focus_tx.send(false).await?;
                                            execute!(
                                                io::stdout(),
                                                terminal::EnterAlternateScreen,
                                                EnableMouseCapture,
                                            )?;
                                        } else if !no_hint{
                                            let size = terminal::size()?;
                                            shared_renderer.update([
                                                (
                                                    Index::Guide,
                                                    status::State::new(
                                                        "Failed to switch pane while rendering is in progress.",
                                                        Severity::Warning,
                                                    )
                                                    .create_graphemes(size.0, size.1),
                                                ),
                                            ]).render().await?;
                                        }
                                    },
                                    Focus::Processor => {
                                        focus = Focus::Editor;
                                        editor_focus_tx.send(true).await?;
                                        execute!(
                                            io::stdout(),
                                            terminal::LeaveAlternateScreen,
                                            DisableMouseCapture,
                                        )?;
                                    },
                                }
                            },
                            event => {
                                match focus {
                                    Focus::Editor => {
                                        editor_event_tx.send(event).await?;
                                    },
                                    Focus::Processor => {
                                        processor_event_tx.send(event).await?;
                                    },
                                }
                            },
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

    let editor_task: JoinHandle<anyhow::Result<()>> = {
        let shared_renderer = shared_renderer.clone();
        let shared_editor = shared_editor.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(focus) = editor_focus_rx.recv() => {
                        let (editor_pane, guide_pane) = {
                            let mut editor = shared_editor.write().await;
                            if focus {
                                editor.focus();
                            } else {
                                editor.defocus();
                            }
                            (
                                editor.create_editor_pane(size.0, size.1),
                                editor.create_guide_pane(size.0, size.1),
                            )
                        };
                        shared_renderer.update([
                            (Index::Editor, editor_pane),
                            (Index::Guide, if !no_hint { guide_pane } else { empty_pane() }),
                        ]).render().await?;
                    }
                    Some(()) = editor_copy_rx.recv() => {
                        let text = {
                            let editor = shared_editor.read().await;
                            editor.text()
                        };
                        let guide = copy_to_clipboard(&text);
                        if !no_hint {
                            let size = terminal::size()?;
                            let pane = guide.create_graphemes(size.0, size.1);
                            shared_renderer.update([
                                (Index::Guide, pane),
                            ]).render().await?;
                        }
                    }
                    Some(event) = editor_event_rx.recv() => {
                        let size = terminal::size()?;
                        let (editor_pane, guide_pane, searcher_pane) = {

                            let mut editor = shared_editor.write().await;
                            editor.operate(&event).await?;

                            let current_text = editor.text();
                            if current_text != text_diff[1] {
                                debounce_query_tx.send(current_text.clone()).await?;
                                text_diff[0] = text_diff[1].clone();
                                text_diff[1] = current_text;
                            }
                            (
                                editor.create_editor_pane(size.0, size.1),
                                editor.create_guide_pane(size.0, size.1),
                                editor.create_searcher_pane(size.0, size.1),
                            )
                        };
                        {
                            shared_renderer.update([
                                (Index::Editor, editor_pane),
                                (Index::Guide, if !no_hint { guide_pane } else { empty_pane() }),
                                (Index::Search, searcher_pane),
                            ]).render().await?;
                        }
                    }
                    else => {
                        break
                    }
                }
            }
            Ok(())
        })
    };

    let shared_visualizer = Arc::new(Mutex::new(initializing.await?));
    let processor_task: JoinHandle<anyhow::Result<()>> = {
        let shared_renderer = shared_renderer.clone();
        let shared_editor = shared_editor.clone();
        let shared_visualizer = shared_visualizer.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(()) = processor_copy_rx.recv() => {
                        let visualizer = shared_visualizer.lock().await;
                        let guide = copy_to_clipboard(&visualizer.content_to_copy().await);
                        if !no_hint {
                            let size = terminal::size()?;
                            let pane = guide.create_graphemes(size.0, size.1);
                            shared_renderer.update([
                                (Index::Guide, pane),
                            ]).render().await?;
                        }
                    }
                    Some(event) = processor_event_rx.recv() => {
                        let pane = {
                            let mut visualizer = shared_visualizer.lock().await;
                            visualizer.create_pane_from_event((size.0, size.1), &event).await
                        };
                        {
                            shared_renderer.update([
                                (Index::Processor, pane),
                            ]).render().await?;
                        }
                    }
                    Some(query) = last_query_rx.recv() => {
                        processor.render_result(
                            shared_visualizer.clone(),
                            query,
                            shared_renderer.clone(),
                        ).await;
                    }
                    Some(area) = last_resize_rx.recv() => {
                        let (editor_pane, guide_pane, searcher_pane) = {
                            let editor = shared_editor.read().await;
                            (
                                editor.create_editor_pane(size.0, size.1),
                                editor.create_guide_pane(size.0, size.1),
                                editor.create_searcher_pane(size.0, size.1),
                            )
                        };
                        {
                            shared_renderer.update([
                                (Index::Editor, editor_pane),
                                (Index::Guide, if !no_hint { guide_pane } else { empty_pane() }),
                                (Index::Search, searcher_pane),
                            ]).render().await?;
                        }
                        let text = {
                            let editor = shared_editor.read().await;
                            editor.text()
                        };
                        processor.render_on_resize(
                            shared_visualizer.clone(),
                            area,
                            text,
                            shared_renderer.clone(),
                        ).await;
                    }
                    else => {
                        break
                    }
                }
            }
            Ok(())
        })
    };

    main_task.await??;

    let output = if write_to_stdout {
        let visualizer = shared_visualizer.lock().await;
        Some(visualizer.content_to_copy().await)
    } else {
        None
    };

    loading_suggestions_task.abort();
    spinning.abort();
    query_debouncer.abort();
    resize_debouncer.abort();
    editor_task.abort();
    processor_task.abort();

    execute!(io::stdout(), cursor::Show, DisableMouseCapture)?;
    disable_raw_mode()?;

    Ok(output)
}
