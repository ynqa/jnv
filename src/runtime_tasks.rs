use std::sync::Arc;

use promkit_widgets::core::render::SharedRenderer;
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};

use crate::{
    completion::CompletionNavigator,
    context::{Index, SharedContext},
    guide::GuideAction,
    json_viewer::{self, RenderTrigger, SharedJsonViewer},
    query_editor::QueryEditor,
};

/// Spawns a task that listens for query changes and forwards them to the JSON viewer action channel.
pub fn spawn_query_change_forward_task(
    mut last_query_rx: mpsc::Receiver<String>,
    json_viewer_action_tx: mpsc::Sender<json_viewer::ViewerAction>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(query) = last_query_rx.recv().await {
            let _ = json_viewer_action_tx
                .send(json_viewer::ViewerAction::QueryChanged(query))
                .await;
        }
    })
}

/// Spawns a task that listens for terminal resize events and triggers re-rendering of the UI components accordingly.
#[allow(clippy::too_many_arguments)]
pub fn spawn_resize_render_task(
    mut last_resize_rx: mpsc::Receiver<(u16, u16)>,
    ctx: SharedContext,
    shared_renderer: SharedRenderer<Index>,
    shared_editor: Arc<RwLock<QueryEditor>>,
    shared_completion: Arc<RwLock<CompletionNavigator>>,
    shared_viewer_state: SharedJsonViewer,
    guide_action_tx: mpsc::Sender<GuideAction>,
) -> JoinHandle<anyhow::Result<()>> {
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
}
