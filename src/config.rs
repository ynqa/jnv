use std::collections::HashSet;

use crossterm::style::{Attribute, Attributes, Color, ContentStyle};
use promkit::style::StyleBuilder;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use tokio::time::Duration;

mod content_style_serde {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct ContentStyleDef {
        foreground: Option<Color>,
        background: Option<Color>,
        underline: Option<Color>,
        attributes: Option<Vec<String>>,
    }

    pub fn serialize<S>(style: &ContentStyle, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut style_def = ContentStyleDef {
            foreground: style.foreground_color,
            background: style.background_color,
            underline: style.underline_color,
            attributes: None,
        };

        if !style.attributes.is_empty() {
            let mut attr_vec = Vec::new();

            if style.attributes.has(Attribute::Bold) {
                attr_vec.push("Bold".to_string());
            }
            if style.attributes.has(Attribute::Dim) {
                attr_vec.push("Dim".to_string());
            }
            if style.attributes.has(Attribute::Italic) {
                attr_vec.push("Italic".to_string());
            }
            if style.attributes.has(Attribute::Underlined) {
                attr_vec.push("Underlined".to_string());
            }
            if style.attributes.has(Attribute::SlowBlink) {
                attr_vec.push("SlowBlink".to_string());
            }
            if style.attributes.has(Attribute::RapidBlink) {
                attr_vec.push("RapidBlink".to_string());
            }
            if style.attributes.has(Attribute::Reverse) {
                attr_vec.push("Reverse".to_string());
            }
            if style.attributes.has(Attribute::Hidden) {
                attr_vec.push("Hidden".to_string());
            }
            if style.attributes.has(Attribute::CrossedOut) {
                attr_vec.push("CrossedOut".to_string());
            }
            if style.attributes.has(Attribute::Fraktur) {
                attr_vec.push("Fraktur".to_string());
            }
            if style.attributes.has(Attribute::DoubleUnderlined) {
                attr_vec.push("DoubleUnderlined".to_string());
            }
            if style.attributes.has(Attribute::Undercurled) {
                attr_vec.push("Undercurled".to_string());
            }
            if style.attributes.has(Attribute::Underdotted) {
                attr_vec.push("Underdotted".to_string());
            }
            if style.attributes.has(Attribute::Underdashed) {
                attr_vec.push("Underdashed".to_string());
            }
            if style.attributes.has(Attribute::Framed) {
                attr_vec.push("Framed".to_string());
            }
            if style.attributes.has(Attribute::Encircled) {
                attr_vec.push("Encircled".to_string());
            }
            if style.attributes.has(Attribute::OverLined) {
                attr_vec.push("OverLined".to_string());
            }

            if !attr_vec.is_empty() {
                style_def.attributes = Some(attr_vec);
            }
        }

        style_def.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ContentStyle, D::Error>
    where
        D: Deserializer<'de>,
    {
        let style_def = ContentStyleDef::deserialize(deserializer)?;

        let mut style = ContentStyle::new();

        style.foreground_color = style_def.foreground;
        style.background_color = style_def.background;
        style.underline_color = style_def.underline;

        if let Some(attrs) = style_def.attributes {
            let mut attributes = Attributes::default();
            for attr_name in attrs {
                match attr_name.as_str() {
                    "Bold" => attributes = attributes | Attribute::Bold,
                    "Dim" => attributes = attributes | Attribute::Dim,
                    "Italic" => attributes = attributes | Attribute::Italic,
                    "Underlined" => attributes = attributes | Attribute::Underlined,
                    "SlowBlink" => attributes = attributes | Attribute::SlowBlink,
                    "RapidBlink" => attributes = attributes | Attribute::RapidBlink,
                    "Reverse" => attributes = attributes | Attribute::Reverse,
                    "Hidden" => attributes = attributes | Attribute::Hidden,
                    "CrossedOut" => attributes = attributes | Attribute::CrossedOut,
                    "Fraktur" => attributes = attributes | Attribute::Fraktur,
                    "DoubleUnderlined" => attributes = attributes | Attribute::DoubleUnderlined,
                    "Undercurled" => attributes = attributes | Attribute::Undercurled,
                    "Underdotted" => attributes = attributes | Attribute::Underdotted,
                    "Underdashed" => attributes = attributes | Attribute::Underdashed,
                    "Framed" => attributes = attributes | Attribute::Framed,
                    "Encircled" => attributes = attributes | Attribute::Encircled,
                    "OverLined" => attributes = attributes | Attribute::OverLined,
                    _ => {
                        return Err(serde::de::Error::custom(format!(
                            "Unknown attribute: {}",
                            attr_name
                        )))
                    }
                }
            }
            style.attributes = attributes;
        }

        Ok(style)
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Duration to debounce query events, in milliseconds.
    #[serde(default, rename = "query_debounce_duration_ms")]
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub query_debounce_duration: Duration,

    /// Duration to debounce resize events, in milliseconds.
    #[serde(default, rename = "resize_debounce_duration_ms")]
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub resize_debounce_duration: Duration,

    pub search_result_chunk_size: usize,
    pub search_load_chunk_size: usize,

    #[serde(with = "content_style_serde")]
    pub active_item_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    pub inactive_item_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    pub prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    pub active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    pub inactive_char_style: ContentStyle,

    pub focus_prefix: String,
    #[serde(with = "content_style_serde")]
    pub focus_prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    pub focus_active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    pub focus_inactive_char_style: ContentStyle,

    pub defocus_prefix: String,
    #[serde(with = "content_style_serde")]
    pub defocus_prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    pub defocus_active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    pub defocus_inactive_char_style: ContentStyle,

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

    pub word_break_chars: HashSet<char>,
    #[serde(default, rename = "spin_duration_ms")]
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub spin_duration: Duration,

    pub move_to_tail: crossterm::event::KeyEvent,
    pub move_to_head: crossterm::event::KeyEvent,
    pub backward: crossterm::event::KeyEvent,
    pub forward: crossterm::event::KeyEvent,
    pub completion: crossterm::event::KeyEvent,
    pub move_to_next_nearest: crossterm::event::KeyEvent,
    pub move_to_previous_nearest: crossterm::event::KeyEvent,
    pub erase: crossterm::event::KeyEvent,
    pub erase_all: crossterm::event::KeyEvent,
    pub erase_to_previous_nearest: crossterm::event::KeyEvent,
    pub erase_to_next_nearest: crossterm::event::KeyEvent,
    pub search_up: crossterm::event::KeyEvent,
}

impl Config {
    /// Overrides the current configuration with values from a string.
    ///
    /// This function parses the provided string and overrides the current configuration
    /// with the values from the parsed configuration.
    ///
    /// # Arguments
    ///
    /// * `content` - A string slice that holds the configuration content to be parsed.
    ///
    /// # Returns
    ///
    /// This function returns an `anyhow::Result<()>` which is `Ok(())` if the string is
    /// successfully parsed and the configuration is overridden, or an error if something
    /// goes wrong during the process.
    pub(crate) fn override_from_string(&mut self, content: &str) -> anyhow::Result<Self> {
        Ok(self)
    }
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
            // search_down: KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_config_deserialization() {
        let toml = r#"
            search_result_chunk_size = 10
            query_debounce_duration_ms = 1000
            resize_debounce_duration_ms = 2000
            search_load_chunk_size = 5
            focus_prefix = "❯ "

            [active_item_style]
            foreground = "green"

            [focus_active_char_style]
            background = "green"
            underline = "red"
            attributes = ["Bold", "Underlined"]

            [move_to_tail]
            key = { Char = "$" }
            modifiers = "CONTROL"
        "#;

        let config = Config::default().override_from_string(toml).unwrap();

        assert_eq!(config.search_result_chunk_size, 10);
        assert_eq!(config.query_debounce_duration, Duration::from_millis(1000));
        assert_eq!(config.resize_debounce_duration, Duration::from_millis(2000));
        assert_eq!(config.search_load_chunk_size, 5);
        assert_eq!(
            config.active_item_style,
            StyleBuilder::new().fgc(Color::Green).build(),
        );

        assert_eq!(
            config.move_to_tail,
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('$'),
                crossterm::event::KeyModifiers::CONTROL
            )
        );

        assert_eq!(config.focus_prefix, "❯ ".to_string());

        assert_eq!(
            config.focus_active_char_style,
            StyleBuilder::new()
                .bgc(Color::Green)
                .ulc(Color::Red)
                .attrs(Attributes::from(Attribute::Bold) | Attribute::Underlined)
                .build(),
        );
    }
}
