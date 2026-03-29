use std::sync::Arc;

use promkit_widgets::core::render::SharedRenderer;
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    completion::CompletionNavigator,
    config::Keybinds,
    context::SharedContext,
    guide::GuideAction,
    json_viewer::{self, RenderTrigger, SharedJsonViewer},
    query_editor::QueryEditor,
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
    shared_viewer_state: SharedJsonViewer,
    _no_hint: bool,
    _keybinds: Keybinds,
    write_to_stdout: bool,
    _debounce_query_tx: mpsc::Sender<String>,
    query_debouncer: JoinHandle<()>,
    mut last_resize_rx: mpsc::Receiver<(u16, u16)>,
    resize_debouncer: JoinHandle<()>,
    completion_loader_task: JoinHandle<()>,
    spinning: JoinHandle<()>,
    guide_action_tx: mpsc::Sender<GuideAction>,
    main_task: JoinHandle<anyhow::Result<()>>,
    query_action_forwarder: JoinHandle<()>,
    guide_task: JoinHandle<anyhow::Result<()>>,
    editor_task: JoinHandle<anyhow::Result<()>>,
    completion_task: JoinHandle<anyhow::Result<()>>,
    processor_task: JoinHandle<anyhow::Result<()>>,
) -> anyhow::Result<Option<String>> {
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
