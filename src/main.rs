use std::{
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
};

use anyhow::anyhow;
use clap::Parser;
use config::Config;
use crossterm::style::Attribute;
use promkit::{
    jsonz::format::RowFormatter,
    listbox::{self, Listbox},
    text_editor,
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
mod render;
use render::{PaneIndex, Renderer, EMPTY_PANE};
mod search;
use search::{IncrementalSearcher, SearchProvider};

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

    #[arg(
        short = 'c',
        long = "config",
        help = "Path to the configuration file.",
        long_help = "
        Specifies the path to the configuration file.
        "
    )]
    pub config_file: Option<PathBuf>,
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
fn ensure_file_exists(path: &PathBuf, default_config: &Config) -> anyhow::Result<()> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow!("Failed to create directory: {}", e))?;
    }

    std::fs::File::create(path)?.write_all(toml::to_string_pretty(default_config)?.as_bytes())?;

    Ok(())
}

/// Determines the configuration file path with the following precedence:
/// 1. The provided `config_path` argument, if it exists.
/// 2. The default configuration file path in the user's configuration directory.
///
/// If the configuration file does not exist, it will be created.
/// Returns an error if the file creation fails.
fn determine_config_file(
    config_path: Option<PathBuf>,
    default_config: &Config,
) -> anyhow::Result<PathBuf> {
    // If a custom path is provided
    if let Some(path) = config_path {
        ensure_file_exists(&path, default_config)?;
        return Ok(path);
    }

    // Use the default path
    let default_path = dirs::config_dir()
        .ok_or_else(|| anyhow!("Failed to determine the configuration directory"))?
        .join("jnv")
        .join("config.toml");

    ensure_file_exists(&default_path, default_config)?;
    Ok(default_path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let input = parse_input(&args)?;

    let mut config = Config::default();
    if let Ok(config_file) = determine_config_file(args.config_file, &config) {
        // Note that the configuration file absolutely exists.
        let content = std::fs::read_to_string(&config_file)
            .map_err(|e| anyhow!("Failed to read configuration file: {}", e))?;
        config = Config::load_from(&content)
            .map_err(|e| anyhow!("Failed to deserialize configuration file: {}", e))?;
    }

    let listbox_state = listbox::State {
        listbox: Listbox::default(),
        cursor: config.completion.cursor,
        active_item_style: Some(config.completion.active_item_style),
        inactive_item_style: Some(config.completion.inactive_item_style),
        lines: config.completion.lines,
    };

    let searcher =
        IncrementalSearcher::new(listbox_state, config.completion.search_result_chunk_size);

    let text_editor_state = text_editor::State {
        texteditor: Default::default(),
        history: Default::default(),
        prefix: config.editor.theme_on_focus.prefix.clone(),
        mask: Default::default(),
        prefix_style: config.editor.theme_on_focus.prefix_style,
        active_char_style: config.editor.theme_on_focus.active_char_style,
        inactive_char_style: config.editor.theme_on_focus.inactive_char_style,
        edit_mode: config.editor.mode,
        word_break_chars: config.editor.word_break_chars,
        lines: Default::default(),
    };

    let provider = &mut JsonStreamProvider::new(
        RowFormatter {
            curly_brackets_style: config.json.theme.curly_brackets_style,
            square_brackets_style: config.json.theme.square_brackets_style,
            key_style: config.json.theme.key_style,
            string_value_style: config.json.theme.string_value_style,
            number_value_style: config.json.theme.number_value_style,
            boolean_value_style: config.json.theme.boolean_value_style,
            null_value_style: config.json.theme.null_value_style,
            active_item_attribute: Attribute::Bold,
            inactive_item_attribute: Attribute::Dim,
            indent: config.json.theme.indent,
        },
        config.json.max_streams,
    );

    let item = Box::leak(input.into_boxed_str());

    let loading_suggestions_task =
        searcher.spawn_load_task(provider, item, config.completion.search_load_chunk_size);

    let editor = Editor::new(
        text_editor_state,
        searcher,
        config.editor.theme_on_focus,
        config.editor.theme_on_defocus,
        // TODO: remove clones
        config.keybinds.on_editor.clone(),
        config.keybinds.on_completion.clone(),
    );

    prompt::run(
        item,
        config.reactivity_control,
        provider,
        editor,
        loading_suggestions_task,
        config.no_hint,
        config.keybinds,
    )
    .await?;

    Ok(())
}
