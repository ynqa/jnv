use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    style::{Attribute, Attributes, Color, ContentStyle},
};
use duration_string::DurationString;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

#[derive(Serialize, Deserialize)]
struct KeyEventDef {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl From<&KeyEvent> for KeyEventDef {
    fn from(key_event: &KeyEvent) -> Self {
        KeyEventDef {
            code: key_event.code,
            modifiers: key_event.modifiers,
        }
    }
}

impl From<KeyEventDef> for KeyEvent {
    fn from(key_event_def: KeyEventDef) -> Self {
        KeyEvent::new(key_event_def.code, key_event_def.modifiers)
    }
}

pub mod key_events_serde {
    use super::*;
    use serde::{Deserializer, Serializer};
    use std::collections::HashSet;

    pub fn serialize<S>(key_events: &HashSet<KeyEvent>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let key_event_defs: Vec<KeyEventDef> = key_events.iter().map(KeyEventDef::from).collect();
        key_event_defs.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashSet<KeyEvent>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let key_event_defs = Vec::<KeyEventDef>::deserialize(deserializer)?;
        Ok(key_event_defs.into_iter().map(KeyEvent::from).collect())
    }
}

pub mod option_key_events_serde {
    use super::*;
    use serde::{Deserializer, Serializer};
    use std::collections::HashSet;

    pub fn serialize<S>(
        key_events_opt: &Option<HashSet<KeyEvent>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match key_events_opt {
            Some(key_events) => {
                let key_event_defs: Vec<KeyEventDef> =
                    key_events.iter().map(KeyEventDef::from).collect();
                key_event_defs.serialize(serializer)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<HashSet<KeyEvent>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<Vec<KeyEventDef>>::deserialize(deserializer).map(|opt| {
            opt.map(|key_event_defs| key_event_defs.into_iter().map(KeyEvent::from).collect())
        })
    }
}

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

pub mod duration_serde {
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

pub mod option_content_style_serde {
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

pub mod option_duration_serde {
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
