use promkit_widgets::text_editor::Mode;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod text_editor_mode_serde {
    use super::*;

    pub fn serialize<S>(mode: &Mode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mode_str = match mode {
            Mode::Insert => "Insert",
            Mode::Overwrite => "Overwrite",
            // Add other variants if they exist
        };
        mode_str.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Mode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mode_str = String::deserialize(deserializer)?;
        match mode_str.as_str() {
            "Insert" => Ok(Mode::Insert),
            "Overwrite" => Ok(Mode::Overwrite),
            // Add other variants if they exist
            _ => Err(serde::de::Error::custom(format!(
                "Unknown Mode variant: {}",
                mode_str
            ))),
        }
    }
}
