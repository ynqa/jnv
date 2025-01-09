use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, ContentStyle};
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

    #[serde(default)]
    pub active_item_style: Option<ConfigContentStyle>,

    pub move_to_tail: Option<KeyPress>,

    pub search_result_chunk_size: Option<usize>,
    pub search_load_chunk_size: Option<usize>,
    pub focus_prefix: Option<String>,
}

pub struct Config {
    pub query_debounce_duration: Duration,
    pub resize_debounce_duration: Duration,
    pub active_item_style: Option<ContentStyle>,
    pub search_result_chunk_size: usize,
    pub search_load_chunk_size: usize,
    pub move_to_tail: KeyEvent,
    pub focus_prefix: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigContentStyle {
    /// The foreground color.
    foreground: Option<Color>,
    /// The background color.
    background: Option<Color>,
    /// The underline color.
    underline: Option<Color>,
    // TODO: List of attributes.
    // pub attributes: Attributes,
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
            search_result_chunk_size: 100,
            query_debounce_duration: Duration::from_millis(600),
            resize_debounce_duration: Duration::from_millis(200),
            search_load_chunk_size: 50000,
            move_to_tail: KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
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

        Ok(style_builder.build())
    }
}

pub fn load(filename: &str) -> anyhow::Result<Config> {
    let mut config = Config::default();
    let config_file: ConfigFile = toml::from_str(&std::fs::read_to_string(filename)?)?;

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

    if let Some(search_result_chunk_size) = config_file.search_result_chunk_size {
        config.search_result_chunk_size = search_result_chunk_size;
    }

    if let Some(search_load_chunk_size) = config_file.search_load_chunk_size {
        config.search_load_chunk_size = search_load_chunk_size;
    }

    if let Some(move_to_tail) = config_file.move_to_tail {
        config.move_to_tail = move_to_tail.try_into()?;
    }

    if let Some(focus_prefix) = config_file.focus_prefix {
        config.focus_prefix = focus_prefix;
    }

    Ok(())
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

            [move_to_tail]
            key = { Char = "e" }
            modifiers = "CONTROL"
        "#;

        let config = toml::from_str::<ConfigFile>(toml).unwrap();

        assert_eq!(config.search_result_chunk_size, Some(10));
        assert_eq!(
            config.query_debounce_duration,
            Some(Duration::from_millis(1000))
        );
        assert_eq!(
            config.resize_debounce_duration,
            Some(Duration::from_millis(2000))
        );
        assert_eq!(config.search_load_chunk_size, Some(5));
        assert_eq!(
            config.active_item_style,
            Some(ConfigContentStyle {
                foreground: Some(Color::Green),
                background: None,
                underline: None,
            })
        );

        assert_eq!(
            config.move_to_tail,
            Some(KeyPress {
                key: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL
            })
        );

        assert_eq!(config.focus_prefix, Some("❯ ".to_string()));
    }
}
