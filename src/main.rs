use std::{
    fs::File,
    io::{self, IsTerminal, Read, Write},
    path::PathBuf,
};

use anyhow::anyhow;
use clap::Parser;
use config::Config;
use promkit_widgets::{
    listbox::{self, Listbox},
    text_editor::{self, TextEditor},
};

#[cfg(unix)]
use std::os::fd::OwnedFd;
#[cfg(unix)]
use rustix::{
    io::dup,
    stdio::{dup2_stdout, stdout},
};

mod editor;
use editor::Editor;
mod config;
mod json;
use json::JsonStreamProvider;
mod processor;
use processor::{
    init::ViewInitializer, monitor::ContextMonitor, spinner::SpinnerSpawner, Context, Processor,
    ViewProvider, Visualizer,
};
mod prompt;
mod search;
use search::{IncrementalSearcher, SearchProvider};

use crate::config::DEFAULT_CONFIG;

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

struct StdoutRedirect {
    #[cfg(unix)]
    saved_stdout: Option<OwnedFd>,
}

impl StdoutRedirect {
    fn for_tui(write_to_stdout: bool) -> anyhow::Result<Self> {
        if !write_to_stdout || io::stdout().is_terminal() {
            return Ok(Self {
                #[cfg(unix)]
                saved_stdout: None,
            });
        }

        #[cfg(unix)]
        {
            let tty = File::options()
                .read(true)
                .write(true)
                .open("/dev/tty")
                .map_err(|e| anyhow!("Failed to open /dev/tty for TUI rendering: {e}"))?;

            let saved_fd = dup(stdout()).map_err(|e| anyhow!("Failed to duplicate stdout: {e}"))?;
            dup2_stdout(&tty).map_err(|e| anyhow!("Failed to redirect stdout to /dev/tty: {e}"))?;

            Ok(Self {
                saved_stdout: Some(saved_fd),
            })
        }

        #[cfg(not(unix))]
        {
            Err(anyhow!(
                "`--write-to-stdout` with piped stdout is not supported on this platform"
            ))
        }
    }

    fn restore(&mut self) -> anyhow::Result<()> {
        #[cfg(unix)]
        if let Some(saved_stdout) = self.saved_stdout.take() {
            dup2_stdout(&saved_stdout).map_err(|e| anyhow!("Failed to restore stdout: {e}"))?;
        }

        Ok(())
    }
}

impl Drop for StdoutRedirect {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let input = parse_input(&args)?;

    let config = determine_config_file(args.config_file)
        .and_then(|config_file| {
            std::fs::read_to_string(&config_file)
                .map_err(|e| anyhow!("Failed to read configuration file: {e}"))
        })
        .and_then(|content| Config::load_from(&content))
        .unwrap_or_else(|_e| {
            Config::load_from(DEFAULT_CONFIG).expect("Failed to load default configuration")
        });

    let listbox_state = listbox::State {
        listbox: Listbox::default(),
        config: config.completion.listbox.clone(),
    };

    let searcher =
        IncrementalSearcher::new(listbox_state, config.completion.search_result_chunk_size);

    let text_editor_state = text_editor::State {
        texteditor: if let Some(filter) = args.default_filter {
            TextEditor::new(filter)
        } else {
            Default::default()
        },
        history: Default::default(),
        config: config.editor.on_focus.clone(),
    };

    let provider =
        &mut JsonStreamProvider::new(config.json.stream.clone(), config.json.max_streams);

    let item = Box::leak(input.into_boxed_str());

    let loading_suggestions_task =
        searcher.spawn_load_task(provider, item, config.completion.search_load_chunk_size);

    // TODO: re-consider put editor_task of prompt::run into Editor construction time.
    // Overall, there are several cases where it would be sufficient to
    // launch a background thread during construction.
    let editor = Editor::new(
        text_editor_state,
        searcher,
        config.editor.on_focus,
        config.editor.on_defocus,
        // TODO: remove clones
        config.keybinds.on_editor.clone(),
    );

    let mut stdout_redirect = StdoutRedirect::for_tui(args.write_to_stdout)?;

    // TODO: put all logics here.
    let maybe_output = prompt::run(
        item,
        config.reactivity_control,
        provider,
        editor,
        loading_suggestions_task,
        config.no_hint,
        config.keybinds,
        args.write_to_stdout,
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
