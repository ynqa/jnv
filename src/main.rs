use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use clap::Parser;

use promkit::{
    crossterm::style::{Attribute, Attributes, Color},
    listbox,
    serde_json::{self, Deserializer},
    style::StyleBuilder,
    text, text_editor,
};

mod jnv;
use jnv::{Jnv, JsonTheme};
mod trie;

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
        short = 'd',
        long = "expand-depth",
        default_value = "3",
        help = "Initial depth to which JSON nodes are expanded in the visualization.",
        long_help = "
        Specifies the initial depth to which JSON nodes are expanded in the visualization.
        Note: Increasing this depth can significantly slow down the display for large datasets.
        "
    )]
    pub json_expand_depth: Option<usize>,

    #[arg(
        short = 's',
        long = "limit-length",
        default_value = "50",
        help = "Limit length of JSON array in the visualization.",
        long_help = "
        Specifies the limit length to load JSON array in the visualization.
        Note: Increasing this length can significantly slow down the display for large datasets.
        "
    )]
    pub json_limit_length: Option<usize>,

    #[arg(
        short = 'l',
        long = "suggestion-list-length",
        default_value = "3",
        help = "Number of suggestions visible in the list.",
        long_help = "
        Controls the number of suggestions displayed in the list,
        aiding users in making selections more efficiently.
        "
    )]
    pub suggestion_list_length: usize,
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

/// Deserializes a JSON string into a vector of `serde_json::Value`.
///
/// This function takes a JSON string as input and attempts to parse it into a vector
/// of `serde_json::Value`, which represents any valid JSON value (e.g., object, array, string, number).
/// It leverages `serde_json::Deserializer` to parse the string and collect the results.
///
/// # Arguments
/// * `json_str` - A string slice that holds the JSON data to be deserialized.
///
/// # Returns
/// An `anyhow::Result` wrapping a vector of `serde_json::Value`. On success, it contains the parsed
/// JSON data. On failure, it contains an error detailing what went wrong during parsing.
fn deserialize_json(
    json_str: &str,
    limit_length: Option<usize>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let deserializer = Deserializer::from_str(json_str).into_iter::<serde_json::Value>();
    let results = match limit_length {
        Some(l) => deserializer.take(l).collect::<Result<Vec<_>, _>>(),
        None => deserializer.collect::<Result<Vec<_>, _>>(),
    };
    results.map_err(anyhow::Error::from)
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input = parse_input(&args)?;
    let input_stream = deserialize_json(&input, args.json_limit_length)?;

    let filter_editor = text_editor::State {
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
    };

    let hint_message = text::State {
        text: Default::default(),
        style: StyleBuilder::new()
            .fgc(Color::Green)
            .attrs(Attributes::from(Attribute::Bold))
            .build(),
    };

    let suggestions = listbox::State {
        listbox: listbox::Listbox::from_iter(Vec::<String>::new()),
        cursor: String::from("❯ "),
        active_item_style: Some(
            StyleBuilder::new()
                .fgc(Color::Grey)
                .bgc(Color::Yellow)
                .build(),
        ),
        inactive_item_style: Some(StyleBuilder::new().fgc(Color::Grey).build()),
        lines: Some(args.suggestion_list_length),
    };

    let json_theme = JsonTheme {
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
        lines: Default::default(),
        indent: args.indent,
    };

    let mut prompt = Jnv::try_new(
        input_stream,
        filter_editor,
        hint_message,
        suggestions,
        json_theme,
        args.json_expand_depth,
        args.no_hint,
    )?;
    let _ = prompt.run()?;
    Ok(())
}
