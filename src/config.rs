use std::collections::HashSet;

use crossterm::{
    event::KeyEvent,
    style::{Attribute, Attributes, Color, ContentStyle},
};
use derive_builder::Builder;
use promkit::style::StyleBuilder;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

mod core;
use core::{
    content_style_serde, duration_serde, key_event_serde, option_content_style_serde,
    option_duration_serde, option_key_event_serde,
};

#[derive(Serialize, Deserialize, Builder)]
#[builder(derive(Serialize, Deserialize))]
#[builder(name = "ConfigFromFile")]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(with = "option_duration_serde"))]
    #[builder_field_attr(serde(default))]
    pub query_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(with = "option_duration_serde"))]
    #[builder_field_attr(serde(default))]
    pub resize_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(with = "option_duration_serde"))]
    #[builder_field_attr(serde(default))]
    pub spin_duration: Duration,

    #[builder_field_attr(serde(default))]
    pub search_result_chunk_size: usize,

    #[builder_field_attr(serde(default))]
    pub search_load_chunk_size: usize,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub active_item_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub inactive_item_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub inactive_char_style: ContentStyle,

    #[builder_field_attr(serde(default))]
    pub focus_prefix: String,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub focus_prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub focus_active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub focus_inactive_char_style: ContentStyle,

    #[builder_field_attr(serde(default))]
    pub defocus_prefix: String,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub defocus_prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub defocus_active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub defocus_inactive_char_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub curly_brackets_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub square_brackets_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub key_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub string_value_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub number_value_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub boolean_value_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    #[builder_field_attr(serde(default))]
    pub null_value_style: ContentStyle,

    #[builder_field_attr(serde(default))]
    pub word_break_chars: HashSet<char>,

    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub move_to_tail: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub move_to_head: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub backward: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub forward: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub completion: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub move_to_next_nearest: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub move_to_previous_nearest: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub erase: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub erase_all: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub erase_to_previous_nearest: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub erase_to_next_nearest: KeyEvent,
    #[serde(with = "key_event_serde")]
    #[builder_field_attr(serde(with = "option_key_event_serde"))]
    #[builder_field_attr(serde(default))]
    pub search_up: KeyEvent,
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
            search_load_chunk_size: 50000,
            move_to_tail: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('e'),
                crossterm::event::KeyModifiers::CONTROL,
            ),
            move_to_head: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('a'),
                crossterm::event::KeyModifiers::CONTROL,
            ),
            spin_duration: Duration::from_millis(300),
            word_break_chars: HashSet::from(['.', '|', '(', ')', '[', ']']),
            backward: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Left,
                crossterm::event::KeyModifiers::NONE,
            ),
            forward: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Right,
                crossterm::event::KeyModifiers::NONE,
            ),
            completion: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Tab,
                crossterm::event::KeyModifiers::NONE,
            ),
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
            move_to_next_nearest: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('f'),
                crossterm::event::KeyModifiers::ALT,
            ),
            move_to_previous_nearest: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('b'),
                crossterm::event::KeyModifiers::ALT,
            ),
            erase: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Backspace,
                crossterm::event::KeyModifiers::NONE,
            ),
            erase_all: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('u'),
                crossterm::event::KeyModifiers::CONTROL,
            ),
            erase_to_previous_nearest: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('w'),
                crossterm::event::KeyModifiers::CONTROL,
            ),
            erase_to_next_nearest: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('d'),
                crossterm::event::KeyModifiers::CONTROL,
            ),
            search_up: crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Up,
                crossterm::event::KeyModifiers::NONE,
            ),
        }
    }
}

impl ConfigFromFile {
    /// Load the config from a string.
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
        if let Some(val) = config.move_to_tail {
            self.move_to_tail = val;
        }
        if let Some(val) = config.move_to_head {
            self.move_to_head = val;
        }
        if let Some(val) = config.backward {
            self.backward = val;
        }
        if let Some(val) = config.forward {
            self.forward = val;
        }
        if let Some(val) = config.completion {
            self.completion = val;
        }
        if let Some(val) = config.move_to_next_nearest {
            self.move_to_next_nearest = val;
        }
        if let Some(val) = config.move_to_previous_nearest {
            self.move_to_previous_nearest = val;
        }
        if let Some(val) = config.erase {
            self.erase = val;
        }
        if let Some(val) = config.erase_all {
            self.erase_all = val;
        }
        if let Some(val) = config.erase_to_previous_nearest {
            self.erase_to_previous_nearest = val;
        }
        if let Some(val) = config.erase_to_next_nearest {
            self.erase_to_next_nearest = val;
        }
        if let Some(val) = config.search_up {
            self.search_up = val;
        }
    }
}

#[cfg(test)]
mod tests {
    mod load_from {
        use super::super::*;
        use crossterm::event::{KeyCode, KeyModifiers};

        #[test]
        fn test() {
            let toml = r#"
                search_result_chunk_size = 10
                query_debounce_duration = "1000ms"
                resize_debounce_duration = "2s"
                search_load_chunk_size = 5
                focus_prefix = "❯ "
                spin_duration = "500ms"

                [active_item_style]
                foreground = "green"

                [focus_active_char_style]
                background = "green"
                underline = "red"
                attributes = ["Bold", "Underlined"]

                [move_to_tail]
                code = { Char = "$" }
                modifiers = "CONTROL"
            "#;

            let config = ConfigFromFile::load_from(toml).unwrap();

            assert_eq!(config.search_result_chunk_size, Some(10));
            assert_eq!(
                config.query_debounce_duration,
                Some(Duration::from_millis(1000))
            );
            assert_eq!(
                config.resize_debounce_duration,
                Some(Duration::from_secs(2))
            );
            assert_eq!(config.spin_duration, Some(Duration::from_millis(500)));
            assert_eq!(config.search_load_chunk_size, Some(5));
            assert_eq!(
                config.active_item_style,
                Some(StyleBuilder::new().fgc(Color::Green).build()),
            );

            assert_eq!(
                config.move_to_tail,
                Some(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::CONTROL))
            );

            assert_eq!(config.focus_prefix, Some("❯ ".to_string()));

            assert_eq!(
                config.focus_active_char_style,
                Some(
                    StyleBuilder::new()
                        .bgc(Color::Green)
                        .ulc(Color::Red)
                        .attrs(Attributes::from(Attribute::Bold) | Attribute::Underlined)
                        .build()
                ),
            );

            // Check the part of the config that was not set in the toml
            assert_eq!(config.backward, None);
            assert_eq!(config.forward, None);
            assert_eq!(config.completion, None);
        }

        #[test]
        fn test_with_empty() {
            let toml = "";
            let config = ConfigFromFile::load_from(toml).unwrap();

            assert_eq!(config.search_result_chunk_size, None);
            assert_eq!(config.query_debounce_duration, None);
            assert_eq!(config.resize_debounce_duration, None);
            assert_eq!(config.spin_duration, None);
            assert_eq!(config.search_load_chunk_size, None);
            assert_eq!(config.active_item_style, None);
            assert_eq!(config.inactive_item_style, None);
            assert_eq!(config.focus_prefix, None);
            assert_eq!(config.focus_active_char_style, None);
            assert_eq!(config.move_to_tail, None);
            assert_eq!(config.move_to_head, None);
        }
    }

    mod patch_with {
        use super::super::*;

        #[test]
        fn test() {
            let mut config = Config::default();
            let config_from_file = ConfigFromFile {
                focus_prefix: Some(":)".to_string()),
                ..Default::default()
            };
            config.patch_with(config_from_file);
            assert_eq!(config.focus_prefix, ":)".to_string());
        }
    }
}
