use std::collections::HashSet;
use std::time::Duration;

use crossterm::style::{Attribute, Attributes, Color};
use promkit::style::StyleBuilder;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DurationMilliSeconds;

/// Loads a configuration file and parses its contents into a Config struct.
///
/// This function reads the contents of the specified file and parses it into a Config struct.
/// It returns a Result containing the Config struct if successful, or an error if the file
/// cannot be read or parsed.
///
/// # Arguments
///
/// * `filename` - A string slice that holds the name of the file to be loaded.
///
/// # Returns
///
/// This function returns an `anyhow::Result<Config>` which is `Ok(Config)` if the file is
/// successfully read and parsed, or an error if something goes wrong during the process.
pub(crate) fn load_file(filename: &str) -> anyhow::Result<Config> {
    load_string(&std::fs::read_to_string(filename)?)
}

fn load_string(content: &str) -> anyhow::Result<Config> {
    let mut config = Config::default();
    let config_file: ConfigFile = toml::from_str(content)?;

    merge(&mut config, config_file)?;
    Ok(config)
}

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

    pub active_item_style: Option<ContentStyle>,
    pub inactive_item_style: Option<ContentStyle>,

    pub prefix_style: Option<ContentStyle>,
    pub active_char_style: Option<ContentStyle>,
    pub inactive_char_style: Option<ContentStyle>,

    pub focus_prefix: Option<String>,
    pub focus_prefix_style: Option<ContentStyle>,
    pub focus_active_char_style: Option<ContentStyle>,
    pub focus_inactive_char_style: Option<ContentStyle>,

    pub defocus_prefix: Option<String>,
    pub defocus_prefix_style: Option<ContentStyle>,
    pub defocus_active_char_style: Option<ContentStyle>,
    pub defocus_inactive_char_style: Option<ContentStyle>,

    pub curly_brackets_style: Option<ContentStyle>,
    pub square_brackets_style: Option<ContentStyle>,
    pub key_style: Option<ContentStyle>,
    pub string_value_style: Option<ContentStyle>,
    pub number_value_style: Option<ContentStyle>,
    pub boolean_value_style: Option<ContentStyle>,
    pub null_value_style: Option<ContentStyle>,

    pub word_break_chars: Option<Vec<char>>,
    pub spin_duration: Option<Duration>,

    pub move_to_tail: Option<KeyEvent>,
    pub move_to_head: Option<KeyEvent>,
    pub backward: Option<KeyEvent>,
    pub forward: Option<KeyEvent>,
    pub completion: Option<KeyEvent>,
    pub move_to_next_nearest: Option<KeyEvent>,
    pub move_to_previous_nearest: Option<KeyEvent>,
    pub erase: Option<KeyEvent>,
    pub erase_all: Option<KeyEvent>,
    pub erase_to_previous_nearest: Option<KeyEvent>,
    pub erase_to_next_nearest: Option<KeyEvent>,
    pub search_up: Option<KeyEvent>,
}

pub(crate) struct Config {
    pub query_debounce_duration: Duration,
    pub resize_debounce_duration: Duration,

    pub search_result_chunk_size: usize,
    pub search_load_chunk_size: usize,

    pub prefix_style: crossterm::style::ContentStyle,
    pub active_char_style: crossterm::style::ContentStyle,
    pub inactive_char_style: crossterm::style::ContentStyle,
    pub active_item_style: Option<crossterm::style::ContentStyle>,
    pub inactive_item_style: Option<crossterm::style::ContentStyle>,

    pub curly_brackets_style: crossterm::style::ContentStyle,
    pub square_brackets_style: crossterm::style::ContentStyle,
    pub key_style: crossterm::style::ContentStyle,
    pub string_value_style: crossterm::style::ContentStyle,
    pub number_value_style: crossterm::style::ContentStyle,
    pub boolean_value_style: crossterm::style::ContentStyle,
    pub null_value_style: crossterm::style::ContentStyle,

    pub defocus_prefix: String,
    pub defocus_prefix_style: crossterm::style::ContentStyle,
    pub defocus_active_char_style: crossterm::style::ContentStyle,
    pub defocus_inactive_char_style: crossterm::style::ContentStyle,

    pub focus_prefix: String,
    pub focus_prefix_style: crossterm::style::ContentStyle,
    pub focus_active_char_style: crossterm::style::ContentStyle,
    pub focus_inactive_char_style: crossterm::style::ContentStyle,

    pub spin_duration: Duration,
    pub word_break_chars: std::collections::HashSet<char>,

    pub move_to_tail: crossterm::event::KeyEvent,
    pub move_to_head: crossterm::event::KeyEvent,
    pub move_to_next_nearest: crossterm::event::KeyEvent,
    pub move_to_previous_nearest: crossterm::event::KeyEvent,
    pub backward: crossterm::event::KeyEvent,
    pub forward: crossterm::event::KeyEvent,
    pub completion: crossterm::event::KeyEvent,
    pub erase: crossterm::event::KeyEvent,
    pub erase_all: crossterm::event::KeyEvent,
    pub erase_to_previous_nearest: crossterm::event::KeyEvent,
    pub erase_to_next_nearest: crossterm::event::KeyEvent,
    pub search_up: crossterm::event::KeyEvent,
    // pub search_down: KeyEvent, TODO: Vec of KeyEvent
}

/// Merge the ConfigFile into the Config
///
/// This function is used to merge the ConfigFile into the Config. It will only update the fields
/// that are present in the ConfigFile. If a field is not present in the ConfigFile, the Config will
/// keep its default value.
///
/// # Arguments
///
/// * `config` - A mutable reference to the Config struct that will be updated.
/// * `config_file` - The ConfigFile struct containing the new configuration values.
///
/// # Returns
///
/// This function returns an `anyhow::Result<()>` which is `Ok(())` if the merge is successful,
/// or an error if something goes wrong during the merge process.
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

    if let Some(prefix_style) = config_file.prefix_style {
        config.prefix_style = prefix_style.try_into()?;
    }

    if let Some(active_char_style) = config_file.active_char_style {
        config.active_char_style = active_char_style.try_into()?;
    }

    if let Some(inactive_char_style) = config_file.inactive_char_style {
        config.inactive_char_style = inactive_char_style.try_into()?;
    }

    if let Some(curly_brackets_style) = config_file.curly_brackets_style {
        config.curly_brackets_style = curly_brackets_style.try_into()?;
    }

    if let Some(square_brackets_style) = config_file.square_brackets_style {
        config.square_brackets_style = square_brackets_style.try_into()?;
    }

    if let Some(key_style) = config_file.key_style {
        config.key_style = key_style.try_into()?;
    }

    if let Some(string_value_style) = config_file.string_value_style {
        config.string_value_style = string_value_style.try_into()?;
    }

    if let Some(number_value_style) = config_file.number_value_style {
        config.number_value_style = number_value_style.try_into()?;
    }

    if let Some(boolean_value_style) = config_file.boolean_value_style {
        config.boolean_value_style = boolean_value_style.try_into()?;
    }

    if let Some(null_value_style) = config_file.null_value_style {
        config.null_value_style = null_value_style.try_into()?;
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

/// A Deserializable struct that represents a ContentStyle in the ConfigFile
#[derive(Default, Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentStyle {
    foreground: Option<Color>,
    background: Option<Color>,
    underline: Option<Color>,
    attributes: Option<Vec<Attribute>>,
}

/// A Deserializable struct that represents a KeyPress in the ConfigFile
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KeyEvent {
    pub key: crossterm::event::KeyCode,
    pub modifiers: crossterm::event::KeyModifiers,
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
            inactive_item_style: Some(StyleBuilder::new().fgc(Color::Grey).build()),
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

impl TryFrom<KeyEvent> for crossterm::event::KeyEvent {
    type Error = anyhow::Error;

    fn try_from(keybind: KeyEvent) -> Result<Self, Self::Error> {
        Ok(crossterm::event::KeyEvent::new(
            keybind.key,
            keybind.modifiers,
        ))
    }
}

// Convert a ConfigContentStyle into a ContentStyle
impl TryFrom<ContentStyle> for crossterm::style::ContentStyle {
    type Error = anyhow::Error;

    fn try_from(config_content_style: ContentStyle) -> Result<Self, Self::Error> {
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
