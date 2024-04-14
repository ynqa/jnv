use promkit::serde_json::{self, Deserializer};

pub fn deserialize_json(json_str: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    Deserializer::from_str(json_str)
        .into_iter::<serde_json::Value>()
        .map(|res| res.map_err(anyhow::Error::from))
        .collect::<anyhow::Result<Vec<serde_json::Value>>>()
}
