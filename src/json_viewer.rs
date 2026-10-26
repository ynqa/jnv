use std::sync::Arc;

use promkit_widgets::{
    core::{crossterm::event::Event, render::SharedRenderer, CreatedGraphemes, Widget},
    json::{self as json_widget, Document},
    serde_json::{self, Value},
};
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinHandle,
};

use crate::{
    config::{JsonConfig, JsonViewerKeybinds},
    context::{Index, SharedContext, State},
    guide::{self, GuideAction, GuideMessage},
    json,
};

/// Represent the trigger for rendering views.
pub enum RenderTrigger {
    /// User actions such as key presses
    UserAction(Event),
    /// Query changes such as new jq filter input
    QueryChanged { query: String },
    /// Terminal resize events
    AreaResized { query: String },
}

/// JSON viewer that maintains the state of JSON stream
/// and handles user interactions and query processing.
pub struct JsonViewer {
    state: json_widget::State,
    json: Vec<serde_json::Value>,
    keybinds: JsonViewerKeybinds,
}

pub type SharedJsonViewer = Arc<Mutex<JsonViewer>>;

impl JsonViewer {
    /// Get the formatted content of current JSON stream.
    pub fn formatted_content(&self) -> String {
        self.state.render_pretty_json()
    }

    /// Handle user event and update the viewer state accordingly.
    fn handle_user_event(&mut self, event: &Event) {
        match event {
            // Move up.
            event if self.keybinds.up.contains(event) => {
                self.state.document.up();
            }

            // Move down.
            event if self.keybinds.down.contains(event) => {
                self.state.document.down();
            }

            // Move to head
            event if self.keybinds.move_to_head.contains(event) => {
                self.state.document.head();
            }

            // Move to tail
            event if self.keybinds.move_to_tail.contains(event) => {
                self.state.document.tail();
            }

            // Toggle collapse/expand
            event if self.keybinds.toggle.contains(event) => {
                self.state.document.toggle();
            }

            event if self.keybinds.expand.contains(event) => {
                self.state.document.set_nodes_visibility(false);
            }

            event if self.keybinds.collapse.contains(event) => {
                self.state.document.set_nodes_visibility(true);
            }

            _ => (),
        }
    }

    /// Process jq query and update the viewer state with the results.
    async fn refresh_view_with_query(
        &mut self,
        area: (u16, u16),
        input: String,
    ) -> (Option<GuideMessage>, Option<CreatedGraphemes>) {
        match json::run_jaq(&input, &self.json) {
            Ok(ret) => {
                let mut guide = None;
                if ret.iter().all(|val| *val == Value::Null) {
                    guide = Some(GuideMessage::JqReturnedNull(input));

                    self.state.document = Document::new(self.json.iter());
                } else {
                    self.state.document = Document::new(ret.iter());
                }

                (
                    guide,
                    Some(self.state.create_graphemes_in_viewport(area.0, area.1)),
                )
            }
            Err(e) => {
                self.state.document = Document::new(self.json.iter());

                (
                    Some(GuideMessage::JqFailed(e.to_string())),
                    Some(self.state.create_graphemes_in_viewport(area.0, area.1)),
                )
            }
        }
    }
}

/// Initialize the JSON viewer with the given input, configuration, keybinds, and shared context.
pub async fn initialize(
    input: &'static str,
    config: JsonConfig,
    keybinds: JsonViewerKeybinds,
    shared_ctx: SharedContext,
    shared_renderer: SharedRenderer<Index>,
) -> anyhow::Result<SharedJsonViewer> {
    // Set state to Loading to prevent overwriting by spinner frames in terminal.
    {
        let mut ctx = shared_ctx.lock().await;
        if let Some(task) = ctx.current_task.take() {
            task.abort();
        }
        ctx.state = State::Loading;
    }

    let input_stream = json::deserialize(input, config.max_streams)?;
    let document = Document::new(input_stream.iter());
    let state = json_widget::State {
        document,
        config: config.stream,
    };

    // Set state to Idle to prevent overwriting by spinner frames in terminal.
    {
        let mut ctx = shared_ctx.lock().await;
        ctx.state = State::Idle;
    }

    {
        let area = shared_ctx.area().await;

        // TODO: error handling
        let _ = shared_renderer
            .update([(
                Index::JsonViewer,
                state.create_graphemes_in_viewport(area.0, area.1),
            )])
            .render()
            .await;
    }

    Ok(Arc::new(Mutex::new(JsonViewer {
        json: input_stream,
        state,
        keybinds,
    })))
}

pub async fn render(
    trigger: RenderTrigger,
    shared_ctx: SharedContext,
    shared_viewer_state: SharedJsonViewer,
    shared_renderer: SharedRenderer<Index>,
    guide_action_tx: mpsc::Sender<GuideAction>,
) {
    match trigger {
        RenderTrigger::UserAction(event) => {
            handle_user_event(shared_viewer_state, shared_renderer, shared_ctx, event).await;
        }
        RenderTrigger::QueryChanged { query } => {
            handle_query_changed(
                shared_viewer_state,
                shared_renderer,
                shared_ctx,
                guide_action_tx,
                query,
            )
            .await;
        }
        RenderTrigger::AreaResized { query } => {
            handle_area_resized(
                shared_viewer_state,
                shared_renderer,
                shared_ctx,
                guide_action_tx,
                query,
            )
            .await;
        }
    }
}

async fn handle_user_event(
    shared_viewer_state: SharedJsonViewer,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
    event: Event,
) {
    let area = shared_ctx.area().await;

    let graphemes = {
        let mut viewer = shared_viewer_state.lock().await;
        viewer.handle_user_event(&event);
        viewer.state.create_graphemes_in_viewport(area.0, area.1)
    };

    // TODO: error handling
    let _ = shared_renderer
        .update([(Index::JsonViewer, graphemes)])
        .render()
        .await;
}

async fn handle_query_changed(
    shared_viewer_state: SharedJsonViewer,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
    guide_action_tx: mpsc::Sender<GuideAction>,
    query: String,
) {
    // Abort any ongoing processing task to prevent race conditions
    // and ensure the new render reflects the latest terminal size.
    {
        let mut ctx = shared_ctx.lock().await;
        if let Some(task) = ctx.current_task.take() {
            task.abort();
        }
    }

    let task = spawn_query_update_task(
        shared_viewer_state.clone(),
        shared_ctx.clone(),
        guide_action_tx,
        shared_renderer,
        query,
    );

    // Store the new processing task handle in shared context
    // to allow future cancellation if needed.
    {
        let mut ctx = shared_ctx.lock().await;
        ctx.current_task = Some(task);
    }
}

async fn handle_area_resized(
    shared_viewer_state: SharedJsonViewer,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
    guide_action_tx: mpsc::Sender<GuideAction>,
    query: String,
) {
    {
        let mut ctx = shared_ctx.lock().await;
        // Abort any ongoing processing task to prevent race conditions
        // and ensure the new render reflects the latest terminal size.
        if let Some(task) = ctx.current_task.take() {
            task.abort();
        }
    }

    let task = spawn_query_update_task(
        shared_viewer_state.clone(),
        shared_ctx.clone(),
        guide_action_tx,
        shared_renderer,
        query,
    );

    // Store the new processing task handle in shared context
    // to allow future cancellation if needed.
    {
        let mut ctx = shared_ctx.lock().await;
        ctx.current_task = Some(task);
    }
}

// Spawn a background task to process the jq query and update the viewer state with the results,
// while managing the viewer state to prevent race conditions and ensure the view reflects the latest terminal size.
fn spawn_query_update_task(
    shared_viewer_state: SharedJsonViewer,
    shared_ctx: SharedContext,
    guide_action_tx: mpsc::Sender<GuideAction>,
    shared_renderer: SharedRenderer<Index>,
    query: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Set state to Processing to prevent overwriting by spinner frames in terminal.
        {
            let mut ctx = shared_ctx.lock().await;
            ctx.state = State::Processing;
        }

        let (maybe_guide, maybe_resp) = {
            let area = shared_ctx.area().await;

            let mut runtime = shared_viewer_state.lock().await;
            runtime.refresh_view_with_query(area, query).await
        };

        // Set state to Idle to allow rendering of spinner frames in terminal.
        {
            let mut ctx = shared_ctx.lock().await;
            ctx.state = State::Idle;
        }

        if let Some(message) = maybe_guide {
            let _ = guide_action_tx.send(GuideAction::Show(message)).await;
        }

        // TODO: error handling
        let _ = shared_renderer
            .update([(
                Index::JsonViewer,
                maybe_resp.unwrap_or(CreatedGraphemes::default()),
            )])
            .render()
            .await;
    })
}

/// Represent the actions that can be performed in JSON viewer,
/// including copying results to clipboard, handling user events, and processing query changes.
pub enum ViewerAction {
    /// Copy the current JSON stream results to clipboard.
    CopyResult,
    /// Handle user events such as key presses for navigation and toggling.
    UserEvent(Event),
    /// Handle changes in jq query input for dynamic filtering of JSON stream.
    QueryChanged(String),
}

/// Spawn a background task to handle viewer actions such as user events and query changes,
/// and update the viewer state and rendered view accordingly.
pub fn start_viewer_task(
    mut action_rx: mpsc::Receiver<ViewerAction>,
    shared_ctx: SharedContext,
    shared_viewer_state: SharedJsonViewer,
    shared_renderer: SharedRenderer<Index>,
    guide_action_tx: mpsc::Sender<GuideAction>,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(action) = action_rx.recv() => {
                    match action {
                        ViewerAction::CopyResult => {
                            let runtime = shared_viewer_state.lock().await;
                            let message = guide::copy_to_clipboard_message(&runtime.formatted_content());
                            let _ = guide_action_tx.send(GuideAction::Show(message)).await;
                        }
                        ViewerAction::UserEvent(event) => {
                            render(
                                RenderTrigger::UserAction(event),
                                shared_ctx.clone(),
                                shared_viewer_state.clone(),
                                shared_renderer.clone(),
                                guide_action_tx.clone(),
                            )
                            .await;
                        }
                        ViewerAction::QueryChanged(query) => {
                            render(
                                RenderTrigger::QueryChanged { query },
                                shared_ctx.clone(),
                                shared_viewer_state.clone(),
                                shared_renderer.clone(),
                                guide_action_tx.clone(),
                            )
                            .await;
                        }
                    }
                }
                else => break,
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, DEFAULT_CONFIG};
    use promkit_widgets::core::{render::RendererLayout, WidthMode};

    fn viewer(input: &str) -> JsonViewer {
        let config = Config::load_from(DEFAULT_CONFIG).unwrap();
        let json = json::deserialize(input, None).unwrap();
        JsonViewer {
            state: json_widget::State {
                document: Document::new(json.iter()),
                config: config.json.stream,
            },
            json,
            keybinds: config.keybinds.on_json_viewer,
        }
    }

    #[test]
    fn folding_and_navigation_preserve_complete_json_stream_output() {
        let mut viewer = viewer("{\"nested\": [1, 2]}\n{\"message\": \"こんにちは\"}");
        let original = viewer.formatted_content();
        assert_eq!(
            original,
            "{\n  \"nested\": [\n    1,\n    2\n  ]\n},\n{\n  \"message\": \"こんにちは\"\n}"
        );

        let toggle = viewer.keybinds.toggle.iter().next().unwrap().clone();
        viewer.handle_user_event(&toggle);
        assert!(viewer
            .state
            .create_graphemes()
            .graphemes
            .to_string()
            .contains('…'));
        assert_eq!(viewer.formatted_content(), original);

        let tail = viewer.keybinds.move_to_tail.iter().next().unwrap().clone();
        viewer.handle_user_event(&tail);
        assert!(viewer.state.document.visible_position() > 0);
        let collapse = viewer.keybinds.collapse.iter().next().unwrap().clone();
        viewer.handle_user_event(&collapse);
        assert_eq!(viewer.state.document.visible_position(), 0);
        assert_eq!(viewer.formatted_content(), original);

        let expand = viewer.keybinds.expand.iter().next().unwrap().clone();
        viewer.handle_user_event(&expand);
        assert!(!viewer
            .state
            .create_graphemes()
            .graphemes
            .to_string()
            .contains('…'));
    }

    #[tokio::test]
    async fn filtered_results_use_the_configured_format_and_layout() {
        let mut viewer = viewer("{\"items\": [1, 2, 3]}");
        let (guide, view) = viewer
            .refresh_view_with_query((40, 6), ".items[] | . * 2".into())
            .await;
        assert!(guide.is_none());
        assert_eq!(viewer.formatted_content(), "2,\n4,\n6");
        assert_eq!(view.unwrap().layout.width_mode, WidthMode::Wrap);
    }

    #[tokio::test]
    async fn null_and_invalid_filters_restore_the_input_and_show_a_guide() {
        let mut viewer = viewer("{\"items\": [1, 2, 3]}");
        viewer
            .refresh_view_with_query((40, 6), ".items".into())
            .await;
        let (guide, _) = viewer
            .refresh_view_with_query((40, 6), ".missing".into())
            .await;
        assert!(matches!(guide, Some(GuideMessage::JqReturnedNull(_))));
        assert_eq!(
            json::deserialize(&viewer.formatted_content(), None).unwrap(),
            viewer.json
        );
        let (guide, _) = viewer.refresh_view_with_query((40, 6), ".[".into()).await;
        assert!(matches!(guide, Some(GuideMessage::JqFailed(_))));
        assert_eq!(
            json::deserialize(&viewer.formatted_content(), None).unwrap(),
            viewer.json
        );
    }

    #[test]
    fn long_documents_keep_the_selected_tail_visible_after_resize() {
        let mut viewer = viewer(&format!("{:?}", (0..1000).collect::<Vec<_>>()));
        viewer.state.document.tail();
        let mut layout = RendererLayout::default();
        for (width, height) in [(40, 10), (20, 5), (80, 24)] {
            let json = viewer.state.create_graphemes_in_viewport(width, height);
            assert!(json.graphemes.logical_lines().len() <= height as usize);
            let prepared = layout
                .layout(
                    [
                        (
                            Index::QueryEditor,
                            promkit_widgets::core::grapheme::StyledGraphemes::from("❯❯ .").into(),
                        ),
                        (Index::JsonViewer, json),
                    ],
                    width,
                    height,
                )
                .unwrap();
            let panes = prepared.panes();
            assert!(panes
                .last()
                .unwrap()
                .iter()
                .any(|row| row.to_string().contains(']')));
            assert!(prepared.visible_row_count() <= height as usize);
        }
    }
}
