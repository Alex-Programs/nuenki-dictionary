use serde::{Deserialize, Serialize};
use toml;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub listen_address: String,
    pub listen_port: u16,
    pub loki_url: String,
    pub loki_job: String,
    pub metrics_bind: String,
    pub dump_path: String,
}

impl Config {
    pub fn from_file(file: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(file)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn to_file(&self, file: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string(self)?;
        std::fs::write(file, content)?;
        Ok(())
    }
}
