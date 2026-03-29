use std::sync::Arc;

use promkit_widgets::core::render::SharedRenderer;
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    completion::CompletionNavigator,
    config::Keybinds,
    context::{Index, SharedContext},
    json_viewer::SharedJsonViewer,
    query_editor::QueryEditor,
};

#[allow(clippy::too_many_arguments)]
pub async fn run(
    _ctx: SharedContext,
    _shared_renderer: SharedRenderer<Index>,
    _shared_editor: Arc<RwLock<QueryEditor>>,
    _shared_completion: Arc<RwLock<CompletionNavigator>>,
    shared_viewer_state: SharedJsonViewer,
    _no_hint: bool,
    _keybinds: Keybinds,
    write_to_stdout: bool,
    _debounce_query_tx: mpsc::Sender<String>,
    query_debouncer: JoinHandle<()>,
    resize_debouncer: JoinHandle<()>,
    completion_loader_task: JoinHandle<()>,
    spinning: JoinHandle<()>,
    main_task: JoinHandle<anyhow::Result<()>>,
    query_action_forwarder: JoinHandle<()>,
    guide_task: JoinHandle<anyhow::Result<()>>,
    editor_task: JoinHandle<anyhow::Result<()>>,
    completion_task: JoinHandle<anyhow::Result<()>>,
    processor_task: JoinHandle<anyhow::Result<()>>,
    resize_render_task: JoinHandle<anyhow::Result<()>>,
) -> anyhow::Result<Option<String>> {
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
