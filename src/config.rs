use std::collections::HashSet;

use crossterm::{
    event::{KeyCode, KeyModifiers},
    style::{Attribute, Attributes, Color, ContentStyle},
};
use derive_builder::Builder;
use promkit::style::StyleBuilder;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

mod content_style;
use content_style::{content_style_serde, option_content_style_serde};
mod duration;
use duration::{duration_serde, option_duration_serde};
pub mod event;
use event::{EventDefSet, KeyEventDef};

#[derive(Serialize, Deserialize, Builder)]
#[builder(derive(Serialize, Deserialize))]
#[builder(name = "EditorConfigFromFile")]
pub(crate) struct EditorConfig {
    pub theme_on_focus: EditorTheme,
    pub theme_on_defocus: EditorTheme,

    #[builder_field_attr(serde(default))]
    pub word_break_chars: HashSet<char>,
}

#[derive(Clone, Serialize, Deserialize, Builder)]
#[builder(derive(Serialize, Deserialize))]
#[builder(name = "EditorThemeFromFile")]
pub(crate) struct EditorTheme {
    #[builder_field_attr(serde(default))]
    pub prefix: String,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub prefix_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub active_char_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
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

impl EditorConfigFromFile {
    /// Load the config from a string.
    pub fn load_from(content: &str) -> anyhow::Result<Self> {
        toml::from_str(content).map_err(Into::into)
    }
}

impl EditorConfig {
    pub fn patch_with(&mut self, config: EditorConfigFromFile) {
        // TODO: This is awful verbose. Can we do better?
        if let Some(theme_on_focus) = config.theme_on_focus {
            self.theme_on_focus = theme_on_focus;
        }
        if let Some(theme_on_defocus) = config.theme_on_defocus {
            self.theme_on_defocus = theme_on_defocus;
        }
        if let Some(val) = config.word_break_chars {
            self.word_break_chars = val;
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Builder)]
#[builder(derive(Serialize, Deserialize))]
#[builder(name = "JsonThemeFromFile")]
pub(crate) struct JsonTheme {
    pub indent: usize,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub curly_brackets_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub square_brackets_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub key_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub string_value_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub number_value_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub boolean_value_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
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

impl JsonThemeFromFile {
    /// Load the config from a string.
    pub fn load_from(content: &str) -> anyhow::Result<Self> {
        toml::from_str(content).map_err(Into::into)
    }
}

impl JsonTheme {
    pub fn patch_with(&mut self, theme: JsonThemeFromFile) {
        // TODO: This is awful verbose. Can we do better?
        if let Some(val) = theme.indent {
            self.indent = val;
        }
        if let Some(val) = theme.curly_brackets_style {
            self.curly_brackets_style = val;
        }
        if let Some(val) = theme.square_brackets_style {
            self.square_brackets_style = val;
        }
        if let Some(val) = theme.key_style {
            self.key_style = val;
        }
        if let Some(val) = theme.string_value_style {
            self.string_value_style = val;
        }
        if let Some(val) = theme.number_value_style {
            self.number_value_style = val;
        }
        if let Some(val) = theme.boolean_value_style {
            self.boolean_value_style = val;
        }
        if let Some(val) = theme.null_value_style {
            self.null_value_style = val;
        }
    }
}

#[derive(Serialize, Deserialize, Builder)]
#[builder(derive(Serialize, Deserialize))]
#[builder(name = "ConfigFromFile")]
pub(crate) struct Config {
    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(default, with = "option_duration_serde"))]
    pub query_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(default, with = "option_duration_serde"))]
    pub resize_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(default, with = "option_duration_serde"))]
    pub spin_duration: Duration,

    #[builder_field_attr(serde(default))]
    pub search_result_chunk_size: usize,

    #[builder_field_attr(serde(default))]
    pub search_load_chunk_size: usize,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub active_item_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub inactive_item_style: ContentStyle,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_item_style: StyleBuilder::new()
                .fgc(Color::Grey)
                .bgc(Color::Yellow)
                .build(),
            inactive_item_style: StyleBuilder::new().fgc(Color::Grey).build(),
            search_result_chunk_size: 100,
            query_debounce_duration: Duration::from_millis(600),
            resize_debounce_duration: Duration::from_millis(200),
            spin_duration: Duration::from_millis(300),
            search_load_chunk_size: 50000,
        }
    }
}

impl ConfigFromFile {
    pub fn load_from(content: &str) -> anyhow::Result<Self> {
        toml::from_str(content).map_err(Into::into)
    }
}

impl Config {
    pub fn patch_with(&mut self, config: ConfigFromFile) {
        // TODO: This is awful verbose. Can we do better?
        if let Some(val) = config.query_debounce_duration {
            self.query_debounce_duration = val;
        }
        if let Some(val) = config.resize_debounce_duration {
            self.resize_debounce_duration = val;
        }
        if let Some(val) = config.spin_duration {
            self.spin_duration = val;
        }
        if let Some(val) = config.search_result_chunk_size {
            self.search_result_chunk_size = val;
        }
        if let Some(val) = config.search_load_chunk_size {
            self.search_load_chunk_size = val;
        }
        if let Some(val) = config.active_item_style {
            self.active_item_style = val;
        }
        if let Some(val) = config.inactive_item_style {
            self.inactive_item_style = val;
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Builder)]
#[builder(derive(Serialize, Deserialize))]
#[builder(name = "KeybindsFromFile")]
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

impl KeybindsFromFile {
    /// Load the config from a string.
    pub fn load_from(content: &str) -> anyhow::Result<Self> {
        toml::from_str(content).map_err(Into::into)
    }
}

impl Keybinds {
    pub fn patch_with(&mut self, keybinds: KeybindsFromFile) {
        // TODO: This is awful verbose. Can we do better?
        if let Some(val) = keybinds.move_to_tail {
            self.move_to_tail = val;
        }
        if let Some(val) = keybinds.move_to_head {
            self.move_to_head = val;
        }
        if let Some(val) = keybinds.backward {
            self.backward = val;
        }
        if let Some(val) = keybinds.forward {
            self.forward = val;
        }
        if let Some(val) = keybinds.completion {
            self.completion = val;
        }
        if let Some(val) = keybinds.move_to_next_nearest {
            self.move_to_next_nearest = val;
        }
        if let Some(val) = keybinds.move_to_previous_nearest {
            self.move_to_previous_nearest = val;
        }
        if let Some(val) = keybinds.erase {
            self.erase = val;
        }
        if let Some(val) = keybinds.erase_all {
            self.erase_all = val;
        }
        if let Some(val) = keybinds.erase_to_previous_nearest {
            self.erase_to_previous_nearest = val;
        }
        if let Some(val) = keybinds.erase_to_next_nearest {
            self.erase_to_next_nearest = val;
        }
        if let Some(val) = keybinds.search_up {
            self.search_up = val;
        }
    }
}
