use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read},
    path::PathBuf,
    time::Duration,
};

use anyhow::{anyhow, Result};
use clap::Parser;
use crossterm::style::{Attribute, Attributes, Color};
use promkit::{
    jsonz::format::RowFormatter,
    listbox::{self, Listbox},
    style::StyleBuilder,
    text_editor,
};

mod editor;
use editor::Editor;
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
        short = 'e',
        long = "edit-mode",
        default_value = "insert",
        value_parser = edit_mode_validator,
        help = "Edit mode for the interface ('insert' or 'overwrite').",
        long_help = r#"
        Specifies the edit mode for the interface.
        Acceptable values are "insert" or "overwrite".
        - "insert" inserts a new input at the cursor's position.
        - "overwrite" mode replaces existing characters with new input at the cursor's position.
        "#,
    )]
    pub edit_mode: text_editor::Mode,

    #[arg(
        short = 'i',
        long = "indent",
        default_value = "2",
        help = "Number of spaces used for indentation in the visualized data.",
        long_help = "
        Affect the formatting of the displayed JSON,
        making it more readable by adjusting the indentation level.
        "
    )]
    pub indent: usize,

    #[arg(
        short = 'n',
        long = "no-hint",
        help = "Disables the display of hints.",
        long_help = "
        When this option is enabled, it prevents the display of
        hints that typically guide or offer suggestions to the user.
        "
    )]
    pub no_hint: bool,

    #[arg(
        long = "max-streams",
        help = "Maximum number of JSON streams to display",
        long_help = "
        Sets the maximum number of JSON streams to load and display.
        Limiting this value improves performance for large datasets.
        If not set, all streams will be displayed.
        "
    )]
    pub max_streams: Option<usize>,

    #[arg(
        long = "suggestions",
        default_value = "3",
        help = "Number of autocomplete suggestions to show",
        long_help = "
        Sets the number of autocomplete suggestions displayed during incremental search.
        Higher values show more suggestions but may occupy more screen space.
        Adjust this value based on your screen size and preference.
        "
    )]
    pub suggestions: usize,
}

fn edit_mode_validator(val: &str) -> Result<text_editor::Mode> {
    match val {
        "insert" | "" => Ok(text_editor::Mode::Insert),
        "overwrite" => Ok(text_editor::Mode::Overwrite),
        _ => Err(anyhow!("edit-mode must be 'insert' or 'overwrite'")),
    }
}

/// Parses the input based on the provided arguments.
///
/// This function reads input data from either a specified file or standard input.
/// If the `input` argument is `None`, or if it is a path
/// that equals "-", data is read from standard input.
/// Otherwise, the function attempts to open and
/// read from the file specified in the `input` argument.
fn parse_input(args: &Args) -> Result<String> {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let input = parse_input(&args)?;

    prompt::run(
        Box::leak(input.into_boxed_str()),
        Duration::from_millis(300),
        Duration::from_millis(600),
        Duration::from_millis(200),
        &mut JsonStreamProvider::new(
            RowFormatter {
                curly_brackets_style: StyleBuilder::new()
                    .attrs(Attributes::from(Attribute::Bold))
                    .build(),
                square_brackets_style: StyleBuilder::new()
                    .attrs(Attributes::from(Attribute::Bold))
                    .build(),
                key_style: StyleBuilder::new().fgc(Color::Cyan).build(),
                string_value_style: StyleBuilder::new().fgc(Color::Green).build(),
                number_value_style: StyleBuilder::new().build(),
                boolean_value_style: StyleBuilder::new().build(),
                null_value_style: StyleBuilder::new().fgc(Color::Grey).build(),
                active_item_attribute: Attribute::Bold,
                inactive_item_attribute: Attribute::Dim,
                indent: args.indent,
            },
            args.max_streams,
        ),
        text_editor::State {
            texteditor: Default::default(),
            history: Default::default(),
            prefix: String::from("❯❯ "),
            mask: Default::default(),
            prefix_style: StyleBuilder::new().fgc(Color::Blue).build(),
            active_char_style: StyleBuilder::new().bgc(Color::Magenta).build(),
            inactive_char_style: StyleBuilder::new().build(),
            edit_mode: args.edit_mode,
            word_break_chars: HashSet::from(['.', '|', '(', ')', '[', ']']),
            lines: Default::default(),
        },
        listbox::State {
            listbox: Listbox::from_displayable(Vec::<String>::new()),
            cursor: String::from("❯ "),
            active_item_style: Some(
                StyleBuilder::new()
                    .fgc(Color::Grey)
                    .bgc(Color::Yellow)
                    .build(),
            ),
            inactive_item_style: Some(StyleBuilder::new().fgc(Color::Grey).build()),
            lines: Some(args.suggestions),
        },
        100,
        50000,
        args.no_hint,
    )
    .await?;

    Ok(())
}
