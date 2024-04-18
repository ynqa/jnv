use std::{
    fs::File,
    io::{self, Read},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use clap::Parser;

use promkit::text_editor;

mod jnv;
use jnv::Jnv;

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
    pub expand_depth: Option<usize>,

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

fn main() -> Result<()> {
    let args = Args::parse();
    let mut prompt = Jnv::try_new(
        parse_input(&args)?,
        args.expand_depth,
        args.no_hint,
        args.edit_mode,
        args.indent,
        args.suggestion_list_length,
    )?;
    let _ = prompt.run()?;
    Ok(())
}
