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
use tokio::sync::RwLock;

mod query_editor;
use query_editor::QueryEditor;
mod config;
use config::Config;
mod guide;
mod json_viewer;
mod context;
mod stdout_redirect;
use stdout_redirect::StdoutRedirect;
mod completion;
mod prompt;
use completion::CompletionNavigator;
mod json;
mod utils;

use crate::{config::DEFAULT_CONFIG, context::SharedContext, prompt::Index};

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
        &input,
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
        renderer.clone(),
        ctx.clone(),
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

    // Wrap the query editor and completion navigator in Arc<Mutex<>> to allow shared mutable access across async tasks.
    let shared_query_editor = Arc::new(RwLock::new(query_editor));
    let shared_completion_navigator = Arc::new(RwLock::new(completion_navigator));

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

    // TODO: put all logics here.
    let maybe_output = prompt::run(
        ctx,
        renderer,
        shared_query_editor,
        shared_completion_navigator,
        config.no_hint,
        config.keybinds,
        args.write_to_stdout,
        debounce_query_tx,
        last_query_rx,
        query_debouncer,
        debounce_resize_tx,
        last_resize_rx,
        resize_debouncer,
        completion_loader_task,
        spinner_task,
        load_for_json_viewer,
    )
    .await;

    stdout_redirect.restore()?;
    let maybe_output = maybe_output?;

    if let Some(output) = maybe_output {
        let mut stdout = io::stdout();
        stdout.write_all(output.as_bytes())?;
        if !output.ends_with('\n') {
            stdout.write_all(b"\n")?;
        }
    }

    Ok(())
}
