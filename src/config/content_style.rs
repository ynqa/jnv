use crossterm::style::{Attribute, Attributes, Color, ContentStyle};
use serde::{Deserialize, Serialize};

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

pub mod content_style_serde {
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
