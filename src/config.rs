use std::collections::HashSet;

use crossterm::{
    event::{KeyCode, KeyModifiers},
    style::{Attribute, Attributes, Color, ContentStyle},
};
use promkit::style::StyleBuilder;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

mod content_style;
use content_style::content_style_serde;
mod duration;
use duration::duration_serde;
pub mod event;
use event::{EventDefSet, KeyEventDef};

#[derive(Serialize, Deserialize)]
pub(crate) struct EditorConfig {
    pub theme_on_focus: EditorTheme,
    pub theme_on_defocus: EditorTheme,
    pub word_break_chars: HashSet<char>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct EditorTheme {
    pub prefix: String,

    #[serde(with = "content_style_serde")]
    pub prefix_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub active_char_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub inactive_char_style: ContentStyle,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme_on_focus: EditorTheme {
                prefix: String::from("❯❯ "),
                prefix_style: StyleBuilder::new().fgc(Color::Blue).build(),
                active_char_style: StyleBuilder::new().bgc(Color::Magenta).build(),
                inactive_char_style: StyleBuilder::new().build(),
            },
            theme_on_defocus: EditorTheme {
                prefix: String::from("▼"),
                prefix_style: StyleBuilder::new()
                    .fgc(Color::Blue)
                    .attrs(Attributes::from(Attribute::Dim))
                    .build(),
                active_char_style: StyleBuilder::new()
                    .attrs(Attributes::from(Attribute::Dim))
                    .build(),
                inactive_char_style: StyleBuilder::new()
                    .attrs(Attributes::from(Attribute::Dim))
                    .build(),
            },
            word_break_chars: HashSet::from(['.', '|', '(', ')', '[', ']']),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct JsonTheme {
    pub indent: usize,

    #[serde(with = "content_style_serde")]
    pub curly_brackets_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub square_brackets_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub key_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub string_value_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub number_value_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub boolean_value_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub null_value_style: ContentStyle,
}

impl Default for JsonTheme {
    fn default() -> Self {
        Self {
            indent: 2,
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
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct CompletionConfig {
    pub lines: Option<usize>,

    pub search_result_chunk_size: usize,

    pub search_load_chunk_size: usize,

    #[serde(with = "content_style_serde")]
    pub active_item_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub inactive_item_style: ContentStyle,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            lines: Some(3),
            search_result_chunk_size: 100,
            search_load_chunk_size: 50000,
            active_item_style: StyleBuilder::new()
                .fgc(Color::Grey)
                .bgc(Color::Yellow)
                .build(),
            inactive_item_style: StyleBuilder::new().fgc(Color::Grey).build(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Keybinds {
    pub move_to_tail: EventDefSet,
    pub backward: EventDefSet,
    pub forward: EventDefSet,
    pub completion: EventDefSet,
    pub move_to_head: EventDefSet,
    pub move_to_previous_nearest: EventDefSet,
    pub move_to_next_nearest: EventDefSet,
    pub erase: EventDefSet,
    pub erase_all: EventDefSet,
    pub erase_to_previous_nearest: EventDefSet,
    pub erase_to_next_nearest: EventDefSet,
    pub search_up: EventDefSet,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            move_to_tail: EventDefSet::from(KeyEventDef::new(
                KeyCode::Char('e'),
                KeyModifiers::CONTROL,
            )),
            move_to_head: EventDefSet::from(KeyEventDef::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL,
            )),
            backward: EventDefSet::from(KeyEventDef::new(KeyCode::Left, KeyModifiers::NONE)),
            forward: EventDefSet::from(KeyEventDef::new(KeyCode::Right, KeyModifiers::NONE)),
            completion: EventDefSet::from(KeyEventDef::new(KeyCode::Tab, KeyModifiers::NONE)),
            move_to_next_nearest: EventDefSet::from(KeyEventDef::new(
                KeyCode::Char('f'),
                KeyModifiers::ALT,
            )),
            move_to_previous_nearest: EventDefSet::from(KeyEventDef::new(
                KeyCode::Char('b'),
                KeyModifiers::ALT,
            )),
            erase: EventDefSet::from(KeyEventDef::new(KeyCode::Backspace, KeyModifiers::NONE)),
            erase_all: EventDefSet::from(KeyEventDef::new(
                KeyCode::Char('u'),
                KeyModifiers::CONTROL,
            )),
            erase_to_previous_nearest: EventDefSet::from(KeyEventDef::new(
                KeyCode::Char('w'),
                KeyModifiers::CONTROL,
            )),
            erase_to_next_nearest: EventDefSet::from(KeyEventDef::new(
                KeyCode::Char('d'),
                KeyModifiers::ALT,
            )),
            search_up: EventDefSet::from(KeyEventDef::new(KeyCode::Up, KeyModifiers::NONE)),
        }
    }
}

/// Note that the config struct and the `.toml` configuration file are
/// managed separately because the current toml crate
/// does not readily support the following features:
///
/// - Preserve docstrings as comments in the `.toml` file
///   - https://github.com/toml-rs/toml/issues/376
/// - Output inline tables
///   - https://github.com/toml-rs/toml/issues/592
#[derive(Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(with = "duration_serde")]
    pub query_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    pub resize_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    pub spin_duration: Duration,

    pub editor: EditorConfig,
    pub json: JsonTheme,
    pub completion: CompletionConfig,
    pub keybinds: Keybinds,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            query_debounce_duration: Duration::from_millis(600),
            resize_debounce_duration: Duration::from_millis(200),
            spin_duration: Duration::from_millis(300),
            editor: EditorConfig::default(),
            json: JsonTheme::default(),
            completion: CompletionConfig::default(),
            keybinds: Keybinds::default(),
        }
    }
}

impl Config {
    pub fn load_from(content: &str) -> anyhow::Result<Self> {
        toml::from_str(content).map_err(Into::into)
    }
}
