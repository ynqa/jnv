use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub search_result_chunk_size: usize,
    #[serde(alias = "query_debounce_duration_ms")]
    pub query_debounce_duration: Duration,
    #[serde(alias = "resize_debounce_duration_ms")]
    pub resize_debounce_duration: Duration,
    pub search_load_chunk_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            search_result_chunk_size: 100,
            query_debounce_duration: Duration::from_millis(600),
            resize_debounce_duration: Duration::from_millis(200),
            search_load_chunk_size: 50000,
        }
    }
}

pub fn load(config_file: &str) -> anyhow::Result<Config> {
    let config = std::fs::read_to_string(config_file)?;
    let config: Config = toml::from_str(&config)?;
    Ok(config)
}
