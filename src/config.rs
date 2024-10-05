use std::path::Path;

use anyhow::{anyhow, Result};
use serde_derive::{Deserialize, Serialize};

use promkit::{
    crossterm::{
        event::Event,
        style::{Attribute, Attributes, Color, ContentStyle},
    },
    json::{self, JsonNode, JsonPathSegment, JsonStream},
    listbox,
    pane::Pane,
    serde_json,
    snapshot::Snapshot,
    style::StyleBuilder,
    suggest::Suggest,
    switch::ActiveKeySwitcher,
    text, text_editor, PaneFactory, Prompt, PromptSignal,
};

mod content_style_serde {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct ContentStyleDef {
        foreground_color: Option<String>,
        background_color: Option<String>,
        underline_color: Option<String>,
        attributes: Vec<String>,
    }

    pub fn serialize<S>(style: &ContentStyle, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let def = ContentStyleDef {
            foreground_color: style.foreground_color.map(|c| format!("{:?}", c)),
            background_color: style.background_color.map(|c| format!("{:?}", c)),
            underline_color: style.underline_color.map(|c| format!("{:?}", c)),
            attributes: style
                .attributes
                .iter()
                .map(|a| format!("{:?}", a))
                .collect(),
        };
        def.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ContentStyle, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let def = ContentStyleDef::deserialize(deserializer)?;
        Ok(ContentStyle {
            foreground_color: def.foreground_color.and_then(|s| s.parse().ok()),
            background_color: def.background_color.and_then(|s| s.parse().ok()),
            underline_color: def.underline_color.and_then(|s| s.parse().ok()),
            attributes: Attributes::from_iter(def.attributes.iter().filter_map(|s| s.parse().ok())),
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct JsonTheme {
    /// Style for {}.
    #[serde(with = "content_style_serde")]
    pub curly_brackets_style: ContentStyle,
    /// Style for [].
    #[serde(with = "content_style_serde")]
    pub square_brackets_style: ContentStyle,
    /// Style for "key".
    pub key_style: ContentStyle,
    /// Style for string values.
    pub string_value_style: ContentStyle,
    /// Style for number values.
    pub number_value_style: ContentStyle,
    /// Style for boolean values.
    pub boolean_value_style: ContentStyle,
    /// Style for null values.
    pub null_value_style: ContentStyle,

    /// Attribute for the selected line.
    pub active_item_attribute: Attribute,
    /// Attribute for unselected lines.
    pub inactive_item_attribute: Attribute,

    /// Number of lines available for rendering.
    pub lines: Option<usize>,

    /// The number of spaces used for indentation in the rendered JSON structure.
    /// This value multiplies with the indentation level of a JSON element to determine
    /// the total indentation space. For example, an `indent` value of 4 means each
    /// indentation level will be 4 spaces wide.
    pub indent: usize,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub json_theme: JsonTheme,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        confy::load_path(&path).map_err(|e| {
            anyhow!(
                "Failed to load config from {}: {}",
                path.as_ref().display(),
                e
            )
        })
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        confy::store_path(&path, self).map_err(|e| {
            anyhow!(
                "Failed to save config to {}: {}",
                path.as_ref().display(),
                e
            )
        })
    }
}
