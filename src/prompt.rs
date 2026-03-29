use std::sync::Arc;

use promkit_widgets::core::render::SharedRenderer;
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    completion::{self, CompletionAction, CompletionNavigator},
    config::Keybinds,
    context::SharedContext,
    guide::{self, GuideAction},
    json_viewer::{self, RenderTrigger, SharedJsonViewer},
    query_editor::{self, QueryEditor, QueryEditorAction},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Index {
    QueryEditor = 0,
    Guide = 1,
    Completion = 2,
    JsonViewer = 3,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    ctx: SharedContext,
    shared_renderer: SharedRenderer<Index>,
    shared_editor: Arc<RwLock<QueryEditor>>,
    shared_completion: Arc<RwLock<CompletionNavigator>>,
    no_hint: bool,
    keybinds: Keybinds,
    write_to_stdout: bool,
    debounce_query_tx: mpsc::Sender<String>,
    mut last_query_rx: mpsc::Receiver<String>,
    query_debouncer: JoinHandle<()>,
    mut last_resize_rx: mpsc::Receiver<(u16, u16)>,
    resize_debouncer: JoinHandle<()>,
    completion_loader_task: JoinHandle<()>,
    spinning: JoinHandle<()>,
    json_viewer_bootstrap: impl std::future::Future<Output = anyhow::Result<SharedJsonViewer>>,
    editor_action_tx: mpsc::Sender<QueryEditorAction>,
    editor_action_rx: mpsc::Receiver<QueryEditorAction>,
    completion_action_tx: mpsc::Sender<CompletionAction>,
    completion_action_rx: mpsc::Receiver<CompletionAction>,
    json_viewer_action_tx: mpsc::Sender<json_viewer::ViewerAction>,
    json_viewer_action_rx: mpsc::Receiver<json_viewer::ViewerAction>,
    guide_action_tx: mpsc::Sender<GuideAction>,
    guide_action_rx: mpsc::Receiver<GuideAction>,
    main_task: JoinHandle<anyhow::Result<()>>,
) -> anyhow::Result<Option<String>> {
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
        ctx.clone(),
        shared_editor.clone(),
        shared_renderer.clone(),
        completion_action_tx.clone(),
        debounce_query_tx.clone(),
        guide_action_tx.clone(),
    );

    let completion_task = completion::start_completion_task(
        completion_action_rx,
        ctx.clone(),
        shared_completion.clone(),
        shared_renderer.clone(),
        editor_action_tx.clone(),
        guide_action_tx.clone(),
        keybinds.on_editor.on_completion,
    );

    let shared_viewer_state = json_viewer_bootstrap.await?;
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
                    RenderTrigger::AreaResized { query: text },
                    ctx.clone(),
                    shared_viewer_state.clone(),
                    shared_renderer.clone(),
                    guide_action_tx.clone(),
                )
                .await;
            }
            Ok::<(), anyhow::Error>(())
        })
    };

    let processor_task = json_viewer::start_viewer_task(
        json_viewer_action_rx,
        ctx.clone(),
        shared_viewer_state.clone(),
        shared_renderer.clone(),
        guide_action_tx.clone(),
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
    completion_loader_task.abort();
    query_action_forwarder.abort();
    resize_action_forwarder.abort();
    guide_task.abort();
    editor_task.abort();
    completion_task.abort();
    processor_task.abort();

    Ok(output)
}
