use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub search_result_chunk_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            search_result_chunk_size: 100,
        }
    }
}

//. config_fil is a TOML file that contains the configuration for the application.
pub fn load(config_file: &str) -> anyhow::Result<Config> {
    let config = std::fs::read_to_string(config_file)?;
    let config: Config = toml::from_str(&config)?;
    Ok(config)
}
