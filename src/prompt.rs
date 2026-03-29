use std::sync::Arc;

use promkit_widgets::core::render::SharedRenderer;
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    completion::CompletionNavigator, config::Keybinds, context::SharedContext, guide::GuideAction,
    json_viewer::SharedJsonViewer, query_editor::QueryEditor, runtime_tasks,
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
    last_resize_rx: mpsc::Receiver<(u16, u16)>,
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
    let resize_render_task = runtime_tasks::spawn_resize_render_task(
        last_resize_rx,
        ctx.clone(),
        shared_renderer.clone(),
        shared_editor.clone(),
        shared_completion.clone(),
        shared_viewer_state.clone(),
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
    resize_render_task.abort();
    guide_task.abort();
    editor_task.abort();
    completion_task.abort();
    processor_task.abort();

    Ok(output)
}
