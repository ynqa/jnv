use std::time::Duration;

use crossterm::style::{Color, ContentStyle};
use promkit::style::StyleBuilder;
use serde::Deserialize;
use serde_with::serde_as;
use serde_with::DurationMilliSeconds;

#[serde_as]
#[derive(Deserialize)]
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

    pub search_result_chunk_size: Option<usize>,
    pub search_load_chunk_size: Option<usize>,
}

pub struct Config {
    pub query_debounce_duration: Duration,
    pub resize_debounce_duration: Duration,
    pub active_item_style: Option<ContentStyle>,
    pub search_result_chunk_size: usize,
    pub search_load_chunk_size: usize,
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ConfigContentStyle {
    /// The foreground color.
    foreground_color: Option<Color>,
    /// The background color.
    background_color: Option<Color>,
    /// The underline color.
    underline_color: Option<Color>,
    // TODO: List of attributes.
    // pub attributes: Attributes,
}

impl Default for Config {
    fn default() -> Self {
        Self {
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
        }
    }
}

// Convert a ConfigContentStyle into a ContentStyle
impl TryFrom<ConfigContentStyle> for ContentStyle {
    type Error = anyhow::Error;

    fn try_from(config_content_style: ConfigContentStyle) -> Result<Self, Self::Error> {
        let mut style_builder = StyleBuilder::new();

        if let Some(foreground_color) = config_content_style.foreground_color {
            style_builder = style_builder.fgc(foreground_color);
        }

        if let Some(background_color) = config_content_style.background_color {
            style_builder = style_builder.bgc(background_color);
        }

        if let Some(underline_color) = config_content_style.underline_color {
            style_builder = style_builder.ulc(underline_color);
        }

        Ok(style_builder.build())
    }
}

pub fn load(filename: &str) -> anyhow::Result<Config> {
    // TODO: load defaults and then merge configuration file values
    let mut config = Config::default();
    let config_file: ConfigFile = toml::from_str(filename)?;

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

            [active_item_style]
            foreground_color = "green"
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
                foreground_color: Some(Color::Green),
                background_color: None,
                underline_color: None,
            })
        );
    }
}
