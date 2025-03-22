use std::collections::HashSet;

use crossterm::style::{Attribute, Attributes, Color, ContentStyle};
use derive_builder::Builder;
use duration_string::DurationString;
use promkit::style::StyleBuilder;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

#[derive(Serialize, Deserialize)]
struct ContentStyleDef {
    foreground: Option<Color>,
    background: Option<Color>,
    underline: Option<Color>,
    attributes: Option<Vec<Attribute>>,
}

impl From<&ContentStyle> for ContentStyleDef {
    fn from(style: &ContentStyle) -> Self {
        ContentStyleDef {
            foreground: style.foreground_color,
            background: style.background_color,
            underline: style.underline_color,
            attributes: if style.attributes.is_empty() {
                None
            } else {
                Some(
                    Attribute::iterator()
                        .filter(|x| style.attributes.has(*x))
                        .collect(),
                )
            },
        }
    }
}

impl From<ContentStyleDef> for ContentStyle {
    fn from(style_def: ContentStyleDef) -> Self {
        let mut style = ContentStyle::new();
        style.foreground_color = style_def.foreground;
        style.background_color = style_def.background;
        style.underline_color = style_def.underline;
        if let Some(attributes) = style_def.attributes {
            style.attributes = attributes
                .into_iter()
                .fold(Attributes::default(), |acc, x| acc | x);
        }
        style
    }
}

mod content_style_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(style: &ContentStyle, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let style_def = ContentStyleDef::from(style);
        style_def.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ContentStyle, D::Error>
    where
        D: Deserializer<'de>,
    {
        let style_def = ContentStyleDef::deserialize(deserializer)?;
        Ok(ContentStyle::from(style_def))
    }
}

mod duration_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&DurationString::from(*duration).to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(DurationString::deserialize(deserializer)?.into())
    }
}

mod option_content_style_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(style_opt: &Option<ContentStyle>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match style_opt {
            Some(style) => content_style_serde::serialize(style, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<ContentStyle>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<ContentStyleDef>::deserialize(deserializer)
            .map_or(Ok(None), |opt| Ok(opt.map(ContentStyle::from)))
    }
}

mod option_duration_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(duration_opt: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration_opt {
            Some(duration) => duration_serde::serialize(duration, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer).map_or(Ok(None), |opt| {
            Ok(opt.and_then(|s| DurationString::from_string(s).ok().map(|ds| ds.into())))
        })
    }
}

#[derive(Serialize, Deserialize, Builder)]
#[builder(derive(Serialize, Deserialize))]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(with = "option_duration_serde"))]
    pub query_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(with = "option_duration_serde"))]
    pub resize_debounce_duration: Duration,

    #[serde(with = "duration_serde")]
    #[builder_field_attr(serde(with = "option_duration_serde"))]
    pub spin_duration: Duration,

    pub search_result_chunk_size: usize,
    pub search_load_chunk_size: usize,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub active_item_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub inactive_item_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub inactive_char_style: ContentStyle,

    pub focus_prefix: String,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub focus_prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub focus_active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub focus_inactive_char_style: ContentStyle,

    pub defocus_prefix: String,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub defocus_prefix_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub defocus_active_char_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub defocus_inactive_char_style: ContentStyle,

    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub curly_brackets_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub square_brackets_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub key_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub string_value_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub number_value_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub boolean_value_style: ContentStyle,
    #[serde(with = "content_style_serde")]
    #[builder_field_attr(serde(with = "option_content_style_serde"))]
    pub null_value_style: ContentStyle,

    pub word_break_chars: HashSet<char>,

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

impl Config {
    /// Overrides the current configuration with values from a string.
    pub(crate) fn override_from_string(mut self, content: &str) -> anyhow::Result<Self> {
        let builder: ConfigBuilder = toml::from_str(content)?;
        // TODO: This is awful verbose. Can we do better?
        if let Some(val) = builder.query_debounce_duration {
            self.query_debounce_duration = val;
        }
        if let Some(val) = builder.resize_debounce_duration {
            self.resize_debounce_duration = val;
        }
        if let Some(val) = builder.spin_duration {
            self.spin_duration = val;
        }
        if let Some(val) = builder.search_result_chunk_size {
            self.search_result_chunk_size = val;
        }
        if let Some(val) = builder.search_load_chunk_size {
            self.search_load_chunk_size = val;
        }
        if let Some(val) = builder.active_item_style {
            self.active_item_style = val;
        }
        if let Some(val) = builder.inactive_item_style {
            self.inactive_item_style = val;
        }
        if let Some(val) = builder.prefix_style {
            self.prefix_style = val;
        }
        if let Some(val) = builder.active_char_style {
            self.active_char_style = val;
        }
        if let Some(val) = builder.inactive_char_style {
            self.inactive_char_style = val;
        }
        if let Some(val) = builder.focus_prefix {
            self.focus_prefix = val;
        }
        if let Some(val) = builder.focus_prefix_style {
            self.focus_prefix_style = val;
        }
        if let Some(val) = builder.focus_active_char_style {
            self.focus_active_char_style = val;
        }
        if let Some(val) = builder.focus_inactive_char_style {
            self.focus_inactive_char_style = val;
        }
        if let Some(val) = builder.defocus_prefix {
            self.defocus_prefix = val;
        }
        if let Some(val) = builder.defocus_prefix_style {
            self.defocus_prefix_style = val;
        }
        if let Some(val) = builder.defocus_active_char_style {
            self.defocus_active_char_style = val;
        }
        if let Some(val) = builder.defocus_inactive_char_style {
            self.defocus_inactive_char_style = val;
        }
        if let Some(val) = builder.curly_brackets_style {
            self.curly_brackets_style = val;
        }
        if let Some(val) = builder.square_brackets_style {
            self.square_brackets_style = val;
        }
        if let Some(val) = builder.key_style {
            self.key_style = val;
        }
        if let Some(val) = builder.string_value_style {
            self.string_value_style = val;
        }
        if let Some(val) = builder.number_value_style {
            self.number_value_style = val;
        }
        if let Some(val) = builder.boolean_value_style {
            self.boolean_value_style = val;
        }
        if let Some(val) = builder.null_value_style {
            self.null_value_style = val;
        }
        if let Some(val) = builder.word_break_chars {
            self.word_break_chars = val;
        }
        if let Some(val) = builder.move_to_tail {
            self.move_to_tail = val;
        }
        if let Some(val) = builder.move_to_head {
            self.move_to_head = val;
        }
        if let Some(val) = builder.backward {
            self.backward = val;
        }
        if let Some(val) = builder.forward {
            self.forward = val;
        }
        if let Some(val) = builder.completion {
            self.completion = val;
        }
        if let Some(val) = builder.move_to_next_nearest {
            self.move_to_next_nearest = val;
        }
        if let Some(val) = builder.move_to_previous_nearest {
            self.move_to_previous_nearest = val;
        }
        if let Some(val) = builder.erase {
            self.erase = val;
        }
        if let Some(val) = builder.erase_all {
            self.erase_all = val;
        }
        if let Some(val) = builder.erase_to_previous_nearest {
            self.erase_to_previous_nearest = val;
        }
        if let Some(val) = builder.erase_to_next_nearest {
            self.erase_to_next_nearest = val;
        }
        if let Some(val) = builder.search_up {
            self.search_up = val;
        }

        Ok(self)
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

        let config = Config::default();
        let config = config.override_from_string(toml).unwrap();

        assert_eq!(config.search_result_chunk_size, 10);
        assert_eq!(config.query_debounce_duration, Duration::from_millis(1000));
        assert_eq!(config.resize_debounce_duration, Duration::from_secs(2));
        assert_eq!(config.spin_duration, Duration::from_millis(500));
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
