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
#[builder(name = "ConfigFromFile")]
#[serde(deny_unknown_fields)]
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

    #[builder_field_attr(serde(default))]
    pub word_break_chars: HashSet<char>,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub active_item_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub inactive_item_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub inactive_char_style: ContentStyle,

    #[builder_field_attr(serde(default))]
    pub focus_prefix: String,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub focus_prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub focus_active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub focus_inactive_char_style: ContentStyle,

    #[builder_field_attr(serde(default))]
    pub defocus_prefix: String,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub defocus_prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub defocus_active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(default, with = "option_content_style_serde"))]
    pub defocus_inactive_char_style: ContentStyle,

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

impl Default for Config {
    fn default() -> Self {
        Self {
            focus_prefix: String::from("❯❯ "),
            active_item_style: StyleBuilder::new()
                .fgc(Color::Grey)
                .bgc(Color::Yellow)
                .build(),
            defocus_prefix: String::from("▼"),
            search_result_chunk_size: 100,
            query_debounce_duration: Duration::from_millis(600),
            resize_debounce_duration: Duration::from_millis(200),
            spin_duration: Duration::from_millis(300),
            word_break_chars: HashSet::from(['.', '|', '(', ')', '[', ']']),
            search_load_chunk_size: 50000,
            prefix_style: StyleBuilder::new().fgc(Color::Blue).build(),
            active_char_style: StyleBuilder::new().bgc(Color::Magenta).build(),
            inactive_char_style: StyleBuilder::new().build(),
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
            defocus_prefix_style: StyleBuilder::new()
                .fgc(Color::Blue)
                .attrs(Attributes::from(Attribute::Dim))
                .build(),
            defocus_active_char_style: StyleBuilder::new()
                .attrs(Attributes::from(Attribute::Dim))
                .build(),
            defocus_inactive_char_style: StyleBuilder::new()
                .attrs(Attributes::from(Attribute::Dim))
                .build(),
            focus_prefix_style: StyleBuilder::new().fgc(Color::Blue).build(),
            focus_active_char_style: StyleBuilder::new().bgc(Color::Magenta).build(),
            focus_inactive_char_style: StyleBuilder::new().build(),
            inactive_item_style: StyleBuilder::new().fgc(Color::Grey).build(),
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
        if let Some(val) = config.prefix_style {
            self.prefix_style = val;
        }
        if let Some(val) = config.active_char_style {
            self.active_char_style = val;
        }
        if let Some(val) = config.inactive_char_style {
            self.inactive_char_style = val;
        }
        if let Some(val) = config.focus_prefix {
            self.focus_prefix = val;
        }
        if let Some(val) = config.focus_prefix_style {
            self.focus_prefix_style = val;
        }
        if let Some(val) = config.focus_active_char_style {
            self.focus_active_char_style = val;
        }
        if let Some(val) = config.focus_inactive_char_style {
            self.focus_inactive_char_style = val;
        }
        if let Some(val) = config.defocus_prefix {
            self.defocus_prefix = val;
        }
        if let Some(val) = config.defocus_prefix_style {
            self.defocus_prefix_style = val;
        }
        if let Some(val) = config.defocus_active_char_style {
            self.defocus_active_char_style = val;
        }
        if let Some(val) = config.defocus_inactive_char_style {
            self.defocus_inactive_char_style = val;
        }
        if let Some(val) = config.curly_brackets_style {
            self.curly_brackets_style = val;
        }
        if let Some(val) = config.square_brackets_style {
            self.square_brackets_style = val;
        }
        if let Some(val) = config.key_style {
            self.key_style = val;
        }
        if let Some(val) = config.string_value_style {
            self.string_value_style = val;
        }
        if let Some(val) = config.number_value_style {
            self.number_value_style = val;
        }
        if let Some(val) = config.boolean_value_style {
            self.boolean_value_style = val;
        }
        if let Some(val) = config.null_value_style {
            self.null_value_style = val;
        }
        if let Some(val) = config.word_break_chars {
            self.word_break_chars = val;
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Builder)]
#[builder(derive(Serialize, Deserialize))]
#[builder(name = "KeybindsFromFile")]
#[serde(deny_unknown_fields)]
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
