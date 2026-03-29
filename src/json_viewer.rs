use std::sync::Arc;

use promkit_widgets::{
    core::{crossterm::event::Event, grapheme::StyledGraphemes, render::SharedRenderer, Widget},
    jsonstream::{self, JsonStream},
    serde_json::{self, Value},
};
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinHandle,
};

use crate::{
    config::{JsonConfig, JsonViewerKeybinds},
    context::{SharedContext, State},
    guide::{self, GuideAction, GuideMessage},
    json,
    prompt::Index,
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
    state: jsonstream::State,
    json: Vec<serde_json::Value>,
    keybinds: JsonViewerKeybinds,
}

pub type SharedJsonViewer = Arc<Mutex<JsonViewer>>;

impl JsonViewer {
    /// Get the formatted content of current JSON stream.
    pub fn formatted_content(&self) -> String {
        self.state.config.format_raw_json(self.state.stream.rows())
    }

    /// Handle user event and update the viewer state accordingly.
    fn handle_user_event(&mut self, event: &Event) {
        match event {
            // Move up.
            event if self.keybinds.up.contains(event) => {
                self.state.stream.up();
            }

            // Move down.
            event if self.keybinds.down.contains(event) => {
                self.state.stream.down();
            }

            // Move to head
            event if self.keybinds.move_to_head.contains(event) => {
                self.state.stream.head();
            }

            // Move to tail
            event if self.keybinds.move_to_tail.contains(event) => {
                self.state.stream.tail();
            }

            // Toggle collapse/expand
            event if self.keybinds.toggle.contains(event) => {
                self.state.stream.toggle();
            }

            event if self.keybinds.expand.contains(event) => {
                self.state.stream.set_nodes_visibility(false);
            }

            event if self.keybinds.collapse.contains(event) => {
                self.state.stream.set_nodes_visibility(true);
            }

            _ => (),
        }
    }

    /// Process jq query and update the viewer state with the results.
    async fn refresh_view_with_query(
        &mut self,
        area: (u16, u16),
        input: String,
    ) -> (Option<GuideMessage>, Option<StyledGraphemes>) {
        match json::run_jaq(&input, &self.json) {
            Ok(ret) => {
                let mut guide = None;
                if ret.iter().all(|val| *val == Value::Null) {
                    guide = Some(GuideMessage::JqReturnedNull(input));

                    self.state.stream = JsonStream::new(self.json.iter());
                } else {
                    self.state.stream = JsonStream::new(ret.iter());
                }

                (guide, Some(self.state.create_graphemes(area.0, area.1)))
            }
            Err(e) => {
                self.state.stream = JsonStream::new(self.json.iter());

                (
                    Some(GuideMessage::JqFailed(e.to_string())),
                    Some(self.state.create_graphemes(area.0, area.1)),
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
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
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
    let stream = JsonStream::new(input_stream.iter());
    let state = jsonstream::State {
        stream,
        config: config.stream,
    };

    // Set state to Idle to prevent overwriting by spinner frames in terminal.
    {
        let mut ctx = shared_ctx.lock().await;
        ctx.state = State::Idle;
    }

    {
        let ctx = shared_ctx.lock().await;
        let area = ctx.area;
        drop(ctx);

        // TODO: error handling
        let _ = shared_renderer
            .update([(Index::JsonViewer, state.create_graphemes(area.0, area.1))])
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
    shared_viewer_state: SharedJsonViewer,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
    guide_action_tx: mpsc::Sender<GuideAction>,
    trigger: RenderTrigger,
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
    let area = {
        let ctx = shared_ctx.lock().await;
        ctx.area
    };

    let graphemes = {
        let mut viewer = shared_viewer_state.lock().await;
        viewer.handle_user_event(&event);
        viewer.state.create_graphemes(area.0, area.1)
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
            let ctx = shared_ctx.lock().await;
            let area = ctx.area;
            drop(ctx);

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
                maybe_resp.unwrap_or(StyledGraphemes::default()),
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
    guide_action_tx: mpsc::Sender<GuideAction>,
    shared_viewer_state: SharedJsonViewer,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
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
                                shared_viewer_state.clone(),
                                shared_renderer.clone(),
                                shared_ctx.clone(),
                                guide_action_tx.clone(),
                                RenderTrigger::UserAction(event),
                            )
                            .await;
                        }
                        ViewerAction::QueryChanged(query) => {
                            render(
                                shared_viewer_state.clone(),
                                shared_renderer.clone(),
                                shared_ctx.clone(),
                                guide_action_tx.clone(),
                                RenderTrigger::QueryChanged { query },
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
