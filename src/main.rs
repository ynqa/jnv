use std::{
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
    sync::Arc,
};

use anyhow::anyhow;
use clap::Parser;
use promkit_widgets::{
    core::{
        crossterm,
        grapheme::StyledGraphemes,
        render::{Renderer, SharedRenderer},
    },
    listbox::{self, Listbox},
    spinner::{self, Spinner},
    text_editor::{self, TextEditor},
};
use tokio::sync::{mpsc, RwLock};

mod query_editor;
use query_editor::QueryEditor;
mod config;
use config::Config;
mod context;
mod guide;
mod json_viewer;
mod stdout_redirect;
use stdout_redirect::StdoutRedirect;
mod completion;
mod event_dispatcher;
mod runtime_tasks;
use completion::CompletionNavigator;
mod json;
mod utils;

use crate::{
    completion::CompletionAction,
    config::DEFAULT_CONFIG,
    context::{Index, SharedContext},
    guide::GuideAction,
    query_editor::QueryEditorAction,
};

/// JSON navigator and interactive filter leveraging jq
#[derive(Parser)]
#[command(
    name = "jnv",
    version,
    help_template = "
{about}

Usage: {usage}

Examples:
- Read from a file:
        {bin} data.json

- Read from standard input:
        cat data.json | {bin}

Arguments:
{positionals}

Options:
{options}
"
)]
pub struct Args {
    /// Optional path to a JSON file.
    /// If not provided or if "-" is specified,
    /// reads from standard input.
    pub input: Option<PathBuf>,

    #[arg(short = 'c', long = "config", help = "Path to the configuration file.")]
    pub config_file: Option<PathBuf>,

    #[arg(
        long = "default-filter",
        help = "Default jq filter to apply to the input data",
        long_help = "
        Sets the default jq filter to apply to the input data.
        The filter is applied when the interface is first loaded.
        "
    )]
    default_filter: Option<String>,

    #[arg(
        long = "write-to-stdout",
        help = "Write the current JSON result to stdout when exiting"
    )]
    write_to_stdout: bool,
}

/// Parses the input based on the provided arguments.
///
/// This function reads input data from either a specified file or standard input.
/// If the `input` argument is `None`, or if it is a path
/// that equals "-", data is read from standard input.
/// Otherwise, the function attempts to open and
/// read from the file specified in the `input` argument.
fn parse_input(args: &Args) -> anyhow::Result<String> {
    let mut ret = String::new();

    match &args.input {
        None => {
            io::stdin().read_to_string(&mut ret)?;
        }
        Some(path) => {
            if path == &PathBuf::from("-") {
                io::stdin().read_to_string(&mut ret)?;
            } else {
                File::open(path)?.read_to_string(&mut ret)?;
            }
        }
    }

    Ok(ret)
}

/// Ensures the configuration file exists, creating it with default settings if it doesn't
///
/// If the file already exists, returns Ok.
/// If the file doesn't exist, writes the default configuration in TOML format.
/// Returns an error if file creation fails.
fn ensure_file_exists(path: &PathBuf) -> anyhow::Result<()> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| anyhow!("Failed to create directory: {e}"))?;
    }

    std::fs::File::create(path)?.write_all(DEFAULT_CONFIG.as_bytes())?;

    Ok(())
}

/// Determines the configuration file path with the following precedence:
/// 1. The provided `config_path` argument, if it exists.
/// 2. The default configuration file path in the user's configuration directory.
///
/// If the configuration file does not exist, it will be created.
/// Returns an error if the file creation fails.
fn determine_config_file(config_path: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    // If a custom path is provided
    if let Some(path) = config_path {
        ensure_file_exists(&path)?;
        return Ok(path);
    }

    // Use the default path
    let default_path = dirs::config_dir()
        .ok_or_else(|| anyhow!("Failed to determine the configuration directory"))?
        // TODO: need versions...?
        .join("jnv")
        .join("config.toml");

    ensure_file_exists(&default_path)?;
    Ok(default_path)
}

/// A guard that ensures terminal state is restored when dropped.
struct TerminalCleanupGuard;

impl Drop for TerminalCleanupGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::cursor::Show,
            crossterm::event::DisableMouseCapture
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load input data
    let input = parse_input(&args)?;
    let input: &'static str = Box::leak(input.into_boxed_str());

    // Load configuration
    let config = determine_config_file(args.config_file)
        .and_then(|config_file| {
            std::fs::read_to_string(&config_file)
                .map_err(|e| anyhow!("Failed to read configuration file: {e}"))
        })
        .and_then(|content| Config::load_from(&content))
        .unwrap_or_else(|_e| {
            Config::load_from(DEFAULT_CONFIG).expect("Failed to load default configuration")
        });

    // Set up terminal
    crossterm::terminal::enable_raw_mode()?;
    let _terminal_cleanup_guard = TerminalCleanupGuard;
    crossterm::execute!(io::stdout(), crossterm::cursor::Hide)?;

    // Spawn the completion loader task, which will asynchronously load suggestions based on the input data.
    let (shared_suggestions, completion_loader_task) = completion::spawn_initialize(
        input,
        config.json.max_streams,
        config.completion.search_load_chunk_size,
    );

    // Initialize the completion navigator with shared suggestions and configuration.
    let completion_navigator = CompletionNavigator::new(
        shared_suggestions,
        listbox::State {
            listbox: Listbox::default(),
            config: config.completion.listbox,
        },
        config.completion.search_result_chunk_size,
    );

    // Initialize the query editor with the default filter, configuration, and keybindings.
    let query_editor = QueryEditor::new(
        text_editor::State {
            texteditor: if let Some(ref filter) = args.default_filter {
                TextEditor::new(filter)
            } else {
                Default::default()
            },
            history: Default::default(),
            config: config.editor.on_focus.clone(),
        },
        config.editor.on_focus,
        config.editor.on_defocus,
        // TODO: remove clones
        config.keybinds.on_editor.clone(),
    );

    // Redirects stdout to prevent interference with TUI interface.
    let mut stdout_redirect = StdoutRedirect::try_new_for_tui(args.write_to_stdout)?;

    // Get terminal size for rendering purposes.
    let terminal_size = crossterm::terminal::size()?;

    // Initialize the shared renderer with graphemes for each UI component.
    let renderer = SharedRenderer::new(
        Renderer::try_new_with_graphemes(
            [
                (
                    Index::QueryEditor,
                    query_editor.create_graphemes(terminal_size.0, terminal_size.1),
                ),
                (Index::Guide, StyledGraphemes::default()),
                (Index::Completion, StyledGraphemes::default()),
                (Index::JsonViewer, StyledGraphemes::default()),
            ]
            .into_iter(),
            true,
        )
        .await?,
    );

    // Initialize the shared context with the terminal size,
    // which can be used by various components for rendering and state management.
    let ctx = SharedContext::new(terminal_size);

    // Load input data into JSON viewer, initializing it with the provided configuration and keybindings.
    let load_for_json_viewer = json_viewer::initialize(
        input,
        config.json,
        config.keybinds.on_json_viewer.clone(),
        ctx.clone(),
        renderer.clone(),
    );

    // Spawn the spinner task, which will display a loading spinner in JSON viewer while processing is ongoing.
    let spinner_task = tokio::spawn({
        let shared_renderer = renderer.clone();
        let ctx = ctx.clone();
        async move {
            let spinner = Spinner::default().duration(config.reactivity_control.spin_duration);
            let _ = spinner::run(&spinner, ctx, Index::JsonViewer, shared_renderer).await;
        }
    });

    // Set up the debouncer for the query editor input, which will manage the timing of query updates
    // to prevent excessive processing while the user is typing.
    let (debounce_query_tx, last_query_rx, query_debouncer) =
        utils::setup_debouncer::<String>(config.reactivity_control.query_debounce_duration);

    // If a default filter is provided via command-line arguments, send it to the query debouncer
    // to initialize the interface with that filter applied.
    if let Some(default_filter) = args.default_filter {
        debounce_query_tx.send(default_filter).await?;
    }

    // Set up the debouncer for terminal resize events, which will manage the timing of resize handling
    // to prevent excessive re-rendering while the terminal is being resized.
    let (debounce_resize_tx, last_resize_rx, resize_debouncer) =
        utils::setup_debouncer::<(u16, u16)>(config.reactivity_control.resize_debounce_duration);

    // Create channels for communication between the main event loop and various components
    // (query editor, completion navigator, JSON viewer, and guide).
    let (editor_action_tx, editor_action_rx) = mpsc::channel::<QueryEditorAction>(1);
    let (completion_action_tx, completion_action_rx) = mpsc::channel::<CompletionAction>(1);
    let (json_viewer_action_tx, json_viewer_action_rx) =
        mpsc::channel::<json_viewer::ViewerAction>(8);
    let (guide_action_tx, guide_action_rx) = mpsc::channel::<GuideAction>(8);

    // Spawn the terminal event dispatcher task, which will listen for user input and terminal events,
    // and forward them to the appropriate channels for handling by the main event loop and components.
    let event_dispacher_task = event_dispatcher::spawn_terminal_event_dispatch_task(
        ctx.clone(),
        config.keybinds.clone(),
        debounce_resize_tx,
        editor_action_tx.clone(),
        completion_action_tx.clone(),
        json_viewer_action_tx.clone(),
        guide_action_tx.clone(),
    );

    // Spawn a task to forward query changes from the debouncer to the JSON viewer, ensuring that
    // the viewer updates in response to user input in the query editor.
    let query_change_forward_task = runtime_tasks::spawn_query_change_forward_task(
        last_query_rx,
        json_viewer_action_tx.clone(),
    );

    // Spawn the guide task, which will manage the display of hints and guidance
    // to the user based on their interactions with the interface.
    let guide_task = guide::start_guide_task(
        guide_action_rx,
        renderer.clone(),
        ctx.clone(),
        config.no_hint,
    );

    // Wrap the query editor and completion navigator in Arc<Mutex<>> to allow shared mutable access across async tasks.
    let shared_query_editor = Arc::new(RwLock::new(query_editor));
    let shared_completion_navigator = Arc::new(RwLock::new(completion_navigator));

    // Spawn the query editor task, which will handle user input in the query editor and update the interface accordingly.
    let query_editor_task = query_editor::start_query_editor_task(
        editor_action_rx,
        ctx.clone(),
        shared_query_editor.clone(),
        renderer.clone(),
        completion_action_tx.clone(),
        debounce_query_tx.clone(),
        guide_action_tx.clone(),
    );

    // Spawn the completion task, which will handle user input in the completion navigator and update the interface accordingly.
    let completion_navigator_task = completion::start_completion_task(
        completion_action_rx,
        ctx.clone(),
        shared_completion_navigator.clone(),
        renderer.clone(),
        editor_action_tx.clone(),
        guide_action_tx.clone(),
        config.keybinds.on_editor.on_completion.clone(),
    );

    // Await JSON viewer bootstrap to complete, which will initialize the viewer with the input data and configuration.
    let shared_json_viewer = load_for_json_viewer.await?;
    // Spawn the JSON viewer processor task, which will handle updates to the JSON viewer based on user input and query changes.
    let json_viewer_task = json_viewer::start_viewer_task(
        json_viewer_action_rx,
        ctx.clone(),
        shared_json_viewer.clone(),
        renderer.clone(),
        guide_action_tx.clone(),
    );

    // Spawn the resize render task, which will listen for terminal resize events and trigger re-rendering of the UI components accordingly.
    let resize_render_task = runtime_tasks::spawn_resize_render_task(
        last_resize_rx,
        ctx.clone(),
        renderer.clone(),
        shared_query_editor.clone(),
        shared_completion_navigator.clone(),
        shared_json_viewer.clone(),
        guide_action_tx.clone(),
    );

    let maybe_output_result: anyhow::Result<Option<String>> = match event_dispacher_task.await {
        Ok(Ok(())) if args.write_to_stdout => {
            let runtime = shared_json_viewer.lock().await;
            Ok(Some(runtime.formatted_content()))
        }
        Ok(Ok(())) => Ok(None),
        // `event_dispacher_task` itself joined successfully, but the task body returned an application error.
        Ok(Err(err)) => Err(err),
        // The join operation failed (e.g. panic/cancel) before the task body could return its own result.
        Err(err) => Err(err.into()),
    };

    spinner_task.abort();
    query_debouncer.abort();
    resize_debouncer.abort();
    completion_loader_task.abort();
    query_change_forward_task.abort();
    resize_render_task.abort();
    guide_task.abort();
    query_editor_task.abort();
    completion_navigator_task.abort();
    json_viewer_task.abort();

    // Restore terminal state and write output to stdout if the option is enabled.
    stdout_redirect.restore()?;

    // If the user has enabled the option to write the current JSON result to stdout on exit, output it now.
    if let Some(output) = maybe_output_result? {
        let mut stdout = io::stdout();
        stdout.write_all(output.as_bytes())?;
        if !output.ends_with('\n') {
            stdout.write_all(b"\n")?;
        }
    }

    Ok(())
}
