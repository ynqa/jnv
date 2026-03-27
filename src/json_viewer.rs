use std::{future::Future, sync::Arc};

use promkit_widgets::{
    core::{
        crossterm::{event::Event, terminal},
        grapheme::StyledGraphemes,
        render::SharedRenderer,
        Widget,
    },
    jsonstream::{self, JsonStream},
    serde_json::{self, Value},
    spinner,
    status::{self, Severity},
};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::{
    config::{JsonConfig, JsonViewerKeybinds},
    json,
    prompt::Index,
};

#[derive(PartialEq)]
/// Represent the current state of the JSON viewer,
/// which can be used to control rendering behavior
/// and manage concurrent tasks like query processing and spinner animation.
pub enum State {
    /// The viewer is idle and ready for user interactions or query processing.
    Idle,
    /// The viewer is currently loading the JSON stream, which may involve deserialization
    Loading,
    /// The viewer is actively processing a jq query, which may involve executing the query
    /// and updating the view with the results.
    Processing,
}

struct Context {
    /// The current state of the processor, which can be Idle, Loading, or Processing.
    state: State,
    /// The current size of the terminal area.
    area: (u16, u16),
    /// The current task being executed, if any.
    current_task: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct SharedContext(Arc<Mutex<Context>>);

impl SharedContext {
    pub fn try_default() -> anyhow::Result<Self> {
        let area = terminal::size()?;
        Ok(Self(Arc::new(Mutex::new(Context {
            state: State::Idle,
            area,
            current_task: None,
        }))))
    }

    async fn lock(&self) -> tokio::sync::MutexGuard<'_, Context> {
        self.0.lock().await
    }
}

impl spinner::State for SharedContext {
    fn is_idle(&self) -> impl Future<Output = bool> + Send {
        let shared = self.0.clone();
        async move {
            let context = shared.lock().await;
            context.state == State::Idle
        }
    }
}

/// Represent the trigger for rendering views.
pub enum RenderTrigger {
    /// User actions such as key presses
    UserAction(Event),
    /// Query changes such as new jq filter input
    QueryChanged { query: String },
    /// Terminal resize events
    AreaResized { area: (u16, u16), query: String },
}

/// JSON viewer that maintains the state of JSON stream
/// and handles user interactions and query processing.
pub struct JsonViewer {
    state: jsonstream::State,
    json: Vec<serde_json::Value>,
    keybinds: JsonViewerKeybinds,
}

/// Initialize the JSON viewer with the given input, configuration, keybinds, and shared context.
pub async fn initialize(
    input: &'static str,
    config: JsonConfig,
    keybinds: JsonViewerKeybinds,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
) -> anyhow::Result<JsonViewer> {
    // Set state to Loading to prevent overwriting by spinner frames in terminal.
    {
        let mut shared_ctx = shared_ctx.lock().await;
        if let Some(task) = shared_ctx.current_task.take() {
            task.abort();
        }
        shared_ctx.state = State::Loading;
    }

    let input_stream = json::deserialize(input, config.max_streams)?;
    let stream = JsonStream::new(input_stream.iter());
    let state = jsonstream::State {
        stream,
        config: config.stream,
    };

    // Set state to Idle to prevent overwriting by spinner frames in terminal.
    {
        let mut shared_ctx = shared_ctx.lock().await;
        shared_ctx.state = State::Idle;
    }

    {
        let shared_ctx = shared_ctx.lock().await;
        let area = shared_ctx.area;
        drop(shared_ctx);

        // TODO: error handling
        let _ = shared_renderer
            .update([(Index::Processor, state.create_graphemes(area.0, area.1))])
            .render()
            .await;
    }

    Ok(JsonViewer {
        json: input_stream,
        state,
        keybinds,
    })
}

pub async fn render(
    shared_viewer_state: Arc<Mutex<JsonViewer>>,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
    trigger: RenderTrigger,
) {
    match trigger {
        RenderTrigger::UserAction(event) => {
            handle_user_action(shared_viewer_state, shared_renderer, shared_ctx, event).await;
        }
        RenderTrigger::QueryChanged { query } => {
            handle_query_changed(shared_viewer_state, shared_renderer, shared_ctx, query).await;
        }
        RenderTrigger::AreaResized { area, query } => {
            handle_area_resized(
                shared_viewer_state,
                shared_renderer,
                shared_ctx,
                area,
                query,
            )
            .await;
        }
    }
}

async fn handle_user_action(
    shared_viewer_state: Arc<Mutex<JsonViewer>>,
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
        viewer.operate(&event);
        viewer.state.create_graphemes(area.0, area.1)
    };

    // TODO: error handling
    let _ = shared_renderer
        .update([(Index::Processor, graphemes)])
        .render()
        .await;
}

async fn handle_query_changed(
    shared_viewer_state: Arc<Mutex<JsonViewer>>,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
    query: String,
) {
    {
        let mut shared_state = shared_ctx.lock().await;
        if let Some(task) = shared_state.current_task.take() {
            task.abort();
        }
    }

    let process_task = spawn_query_processing_task(
        shared_viewer_state.clone(),
        shared_ctx.clone(),
        shared_renderer,
        query,
    );

    // Store the new processing task handle in shared context
    // to allow future cancellation if needed.
    {
        let mut shared_state = shared_ctx.lock().await;
        shared_state.current_task = Some(process_task);
    }
}

async fn handle_area_resized(
    shared_viewer_state: Arc<Mutex<JsonViewer>>,
    shared_renderer: SharedRenderer<Index>,
    shared_ctx: SharedContext,
    area: (u16, u16),
    query: String,
) {
    {
        let mut ctx = shared_ctx.lock().await;

        // Update the terminal area in shared context for accurate rendering.
        ctx.area = area;

        // Abort any ongoing processing task to prevent race conditions
        // and ensure the new render reflects the latest terminal size.
        if let Some(task) = ctx.current_task.take() {
            task.abort();
        }
    }

    let process_task = spawn_query_processing_task(
        shared_viewer_state.clone(),
        shared_ctx.clone(),
        shared_renderer,
        query,
    );

    // Store the new processing task handle in shared context
    // to allow future cancellation if needed.
    {
        let mut shared_state = shared_ctx.lock().await;
        shared_state.current_task = Some(process_task);
    }
}

fn spawn_query_processing_task(
    shared_viewer_state: Arc<Mutex<JsonViewer>>,
    shared_ctx: SharedContext,
    shared_renderer: SharedRenderer<Index>,
    query: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        {
            let mut shared_state = shared_ctx.lock().await;
            shared_state.state = State::Processing;
        }

        let (maybe_guide, maybe_resp) = {
            let shared_state = shared_ctx.lock().await;
            let area = shared_state.area;
            drop(shared_state);

            let mut runtime = shared_viewer_state.lock().await;
            runtime.create_panes_from_query(area, query).await
        };

        // Set state to Idle to prevent overwriting by spinner frames in terminal.
        {
            let mut shared_state = shared_ctx.lock().await;
            shared_state.state = State::Idle;
        }

        // TODO: error handling
        let _ = shared_renderer
            .update([
                (
                    Index::Guide,
                    maybe_guide.unwrap_or(StyledGraphemes::default()),
                ),
                (
                    Index::Processor,
                    maybe_resp.unwrap_or(StyledGraphemes::default()),
                ),
            ])
            .render()
            .await;
    })
}

impl JsonViewer {
    /// Get the formatted content of current JSON stream.
    pub fn formatted_content(&self) -> String {
        self.state.config.format_raw_json(self.state.stream.rows())
    }

    fn operate(&mut self, event: &Event) {
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

    async fn create_panes_from_query(
        &mut self,
        area: (u16, u16),
        input: String,
    ) -> (Option<StyledGraphemes>, Option<StyledGraphemes>) {
        match json::run_jaq(&input, &self.json) {
            Ok(ret) => {
                let mut guide = None;
                if ret.iter().all(|val| *val == Value::Null) {
                    guide = Some(
                        status::State::new(
                            format!(
                                "jq returned 'null', which may indicate a typo or incorrect filter: `{input}`"
                            ),
                            Severity::Warning,
                        )
                        .create_graphemes(area.0, area.1),
                    );

                    self.state.stream = JsonStream::new(self.json.iter());
                } else {
                    self.state.stream = JsonStream::new(ret.iter());
                }

                (guide, Some(self.state.create_graphemes(area.0, area.1)))
            }
            Err(e) => {
                self.state.stream = JsonStream::new(self.json.iter());

                (
                    Some(
                        status::State::new(format!("jq failed: `{e}`"), Severity::Error)
                            .create_graphemes(area.0, area.1),
                    ),
                    Some(self.state.create_graphemes(area.0, area.1)),
                )
            }
        }
    }
}
