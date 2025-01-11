use std::collections::HashSet;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Attribute, Attributes, Color, ContentStyle};
use promkit::style::StyleBuilder;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DurationMilliSeconds;

#[serde_as]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    /// Duration to debounce query events, in milliseconds.
    #[serde(default, alias = "query_debounce_duration_ms")]
    #[serde_as(as = "Option<DurationMilliSeconds<u64>>")]
    pub query_debounce_duration: Option<Duration>,

    /// Duration to debounce resize events, in milliseconds.
    #[serde(default, alias = "resize_debounce_duration_ms")]
    #[serde_as(as = "Option<DurationMilliSeconds<u64>>")]
    pub resize_debounce_duration: Option<Duration>,

    pub search_result_chunk_size: Option<usize>,
    pub search_load_chunk_size: Option<usize>,

    pub active_item_style: Option<ConfigContentStyle>,
    pub inactive_item_style: Option<ConfigContentStyle>,

    pub focus_prefix: Option<String>,
    pub focus_prefix_style: Option<ConfigContentStyle>,
    pub focus_active_char_style: Option<ConfigContentStyle>,
    pub focus_inactive_char_style: Option<ConfigContentStyle>,

    pub defocus_prefix: Option<String>,
    pub defocus_prefix_style: Option<ConfigContentStyle>,
    pub defocus_active_char_style: Option<ConfigContentStyle>,
    pub defocus_inactive_char_style: Option<ConfigContentStyle>,

    pub word_break_chars: Option<Vec<char>>,
    pub spin_duration: Option<Duration>,

    pub move_to_tail: Option<KeyPress>,
    pub move_to_head: Option<KeyPress>,
    pub backward: Option<KeyPress>,
    pub forward: Option<KeyPress>,
    pub completion: Option<KeyPress>,
    pub move_to_next_nearest: Option<KeyPress>,
    pub move_to_previous_nearest: Option<KeyPress>,
    pub erase: Option<KeyPress>,
    pub erase_all: Option<KeyPress>,
    pub erase_to_previous_nearest: Option<KeyPress>,
    pub erase_to_next_nearest: Option<KeyPress>,
    pub search_up: Option<KeyPress>,
}

pub struct Config {
    pub query_debounce_duration: Duration,
    pub resize_debounce_duration: Duration,

    pub search_result_chunk_size: usize,
    pub search_load_chunk_size: usize,

    pub active_item_style: Option<ContentStyle>,
    pub inactive_item_style: Option<ContentStyle>,

    pub defocus_prefix: String,
    pub defocus_prefix_style: ContentStyle,
    pub defocus_active_char_style: ContentStyle,
    pub defocus_inactive_char_style: ContentStyle,

    pub focus_prefix: String,
    pub focus_prefix_style: ContentStyle,
    pub focus_active_char_style: ContentStyle,
    pub focus_inactive_char_style: ContentStyle,

    pub spin_duration: Duration,
    pub word_break_chars: std::collections::HashSet<char>,

    pub move_to_tail: KeyEvent,
    pub move_to_head: KeyEvent,
    pub move_to_next_nearest: KeyEvent,
    pub move_to_previous_nearest: KeyEvent,
    pub backward: KeyEvent,
    pub forward: KeyEvent,
    pub completion: KeyEvent,
    pub erase: KeyEvent,
    pub erase_all: KeyEvent,
    pub erase_to_previous_nearest: KeyEvent,
    pub erase_to_next_nearest: KeyEvent,
    pub search_up: KeyEvent,
    // pub search_down: KeyEvent,
}

pub fn load_file(filename: &str) -> anyhow::Result<Config> {
    load_string(&std::fs::read_to_string(filename)?)
}

fn load_string(content: &str) -> anyhow::Result<Config> {
    let mut config = Config::default();
    let config_file: ConfigFile = toml::from_str(content)?;

    merge(&mut config, config_file)?;
    Ok(config)
}

fn merge(config: &mut Config, config_file: ConfigFile) -> anyhow::Result<()> {
    if let Some(query_debounce_duration) = config_file.query_debounce_duration {
        config.query_debounce_duration = query_debounce_duration;
    }

    if let Some(resize_debounce_duration) = config_file.resize_debounce_duration {
        config.resize_debounce_duration = resize_debounce_duration;
    }

    if let Some(active_item_style) = config_file.active_item_style {
        config.active_item_style = Some(active_item_style.try_into()?);
    }

    if let Some(inactive_item_style) = config_file.inactive_item_style {
        config.inactive_item_style = Some(inactive_item_style.try_into()?);
    }

    if let Some(search_result_chunk_size) = config_file.search_result_chunk_size {
        config.search_result_chunk_size = search_result_chunk_size;
    }

    if let Some(search_load_chunk_size) = config_file.search_load_chunk_size {
        config.search_load_chunk_size = search_load_chunk_size;
    }

    if let Some(focus_prefix) = config_file.focus_prefix {
        config.focus_prefix = focus_prefix;
    }

    if let Some(focus_prefix_style) = config_file.focus_prefix_style {
        config.focus_prefix_style = focus_prefix_style.try_into()?;
    }

    if let Some(focus_active_char_style) = config_file.focus_active_char_style {
        config.focus_active_char_style = focus_active_char_style.try_into()?;
    }

    if let Some(focus_inactive_char_style) = config_file.focus_inactive_char_style {
        config.focus_inactive_char_style = focus_inactive_char_style.try_into()?;
    }

    if let Some(defocus_prefix) = config_file.defocus_prefix {
        config.defocus_prefix = defocus_prefix;
    }

    if let Some(defocus_prefix_style) = config_file.defocus_prefix_style {
        config.defocus_prefix_style = defocus_prefix_style.try_into()?;
    }

    if let Some(defocus_active_char_style) = config_file.defocus_active_char_style {
        config.defocus_active_char_style = defocus_active_char_style.try_into()?;
    }

    if let Some(defocus_inactive_char_style) = config_file.defocus_inactive_char_style {
        config.defocus_inactive_char_style = defocus_inactive_char_style.try_into()?;
    }

    if let Some(spin_duration) = config_file.spin_duration {
        config.spin_duration = spin_duration;
    }

    if let Some(word_break_chars) = config_file.word_break_chars {
        config.word_break_chars = word_break_chars.into_iter().collect();
    }

    if let Some(backward) = config_file.backward {
        config.backward = backward.try_into()?;
    }

    if let Some(forward) = config_file.forward {
        config.forward = forward.try_into()?;
    }

    if let Some(move_to_tail) = config_file.move_to_tail {
        config.move_to_tail = move_to_tail.try_into()?;
    }

    if let Some(move_to_head) = config_file.move_to_head {
        config.move_to_head = move_to_head.try_into()?;
    }

    if let Some(completion) = config_file.completion {
        config.completion = completion.try_into()?;
    }

    if let Some(move_to_next_nearest) = config_file.move_to_next_nearest {
        config.move_to_next_nearest = move_to_next_nearest.try_into()?;
    }

    if let Some(move_to_previous_nearest) = config_file.move_to_previous_nearest {
        config.move_to_previous_nearest = move_to_previous_nearest.try_into()?;
    }

    if let Some(erase) = config_file.erase {
        config.erase = erase.try_into()?;
    }

    if let Some(erase_all) = config_file.erase_all {
        config.erase_all = erase_all.try_into()?;
    }

    if let Some(erase_to_previous_nearest) = config_file.erase_to_previous_nearest {
        config.erase_to_previous_nearest = erase_to_previous_nearest.try_into()?;
    }

    if let Some(erase_to_next_nearest) = config_file.erase_to_next_nearest {
        config.erase_to_next_nearest = erase_to_next_nearest.try_into()?;
    }

    if let Some(search_up) = config_file.search_up {
        config.search_up = search_up.try_into()?;
    }

    Ok(())
}

#[derive(Default, Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigContentStyle {
    /// The foreground color.
    foreground: Option<Color>,
    /// The background color.
    background: Option<Color>,
    /// The underline color.
    underline: Option<Color>,
    /// The attributes like bold, italic, etc.
    attributes: Option<Vec<Attribute>>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyPress {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            focus_prefix: String::from("❯❯ "),
            active_item_style: Some(
                StyleBuilder::new()
                    .fgc(Color::Grey)
                    .bgc(Color::Yellow)
                    .build(),
            ),
            defocus_prefix: String::from("▼"),
            search_result_chunk_size: 100,
            query_debounce_duration: Duration::from_millis(600),
            resize_debounce_duration: Duration::from_millis(200),
            search_load_chunk_size: 50000,
            move_to_tail: KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
            move_to_head: KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            spin_duration: Duration::from_millis(300),
            word_break_chars: HashSet::from(['.', '|', '(', ')', '[', ']']),
            backward: KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            forward: KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            completion: KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
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
            inactive_item_style: Some(StyleBuilder::new().fgc(Color::Grey).build()),
            move_to_next_nearest: KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT),
            move_to_previous_nearest: KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT),
            erase: KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            erase_all: KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            erase_to_previous_nearest: KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
            erase_to_next_nearest: KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            search_up: KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            // search_down: KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        }
    }
}

impl TryFrom<KeyPress> for KeyEvent {
    type Error = anyhow::Error;

    fn try_from(keybind: KeyPress) -> Result<Self, Self::Error> {
        Ok(KeyEvent::new(keybind.key, keybind.modifiers))
    }
}

// Convert a ConfigContentStyle into a ContentStyle
impl TryFrom<ConfigContentStyle> for ContentStyle {
    type Error = anyhow::Error;

    fn try_from(config_content_style: ConfigContentStyle) -> Result<Self, Self::Error> {
        let mut style_builder = StyleBuilder::new();

        if let Some(foreground_color) = config_content_style.foreground {
            style_builder = style_builder.fgc(foreground_color);
        }

        if let Some(background_color) = config_content_style.background {
            style_builder = style_builder.bgc(background_color);
        }

        if let Some(underline_color) = config_content_style.underline {
            style_builder = style_builder.ulc(underline_color);
        }

        if let Some(attributes) = config_content_style.attributes {
            style_builder = style_builder.attrs(
                attributes
                    .into_iter()
                    .fold(Attributes::default(), |acc, x| acc | x),
            );
        }

        Ok(style_builder.build())
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

        let config = load_string(toml).unwrap();

        assert_eq!(config.search_result_chunk_size, 10);
        assert_eq!(config.query_debounce_duration, Duration::from_millis(1000));
        assert_eq!(config.resize_debounce_duration, Duration::from_millis(2000));
        assert_eq!(config.search_load_chunk_size, 5);
        assert_eq!(
            config.active_item_style,
            Some(StyleBuilder::new().fgc(Color::Green).build()),
        );

        assert_eq!(
            config.move_to_tail,
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::CONTROL)
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
