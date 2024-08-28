use serde::Deserialize;
use std::collections::HashMap;
use std::{fs, io};

#[derive(Deserialize)]
pub struct FeatureConfig {
    pub strict: bool,
}

#[derive(Deserialize, Clone)]
pub struct GlobalConfig {
    pub concurrency: usize,
    pub clean: bool,
    pub clear_terminal: bool,
}

#[derive(Deserialize)]
pub struct Config {
    pub global: GlobalConfig,
    pub features: HashMap<String, FeatureConfig>,
}

impl Config {
    pub fn new(file_path: &str) -> io::Result<Self> {
        let contents = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => {
                return Err(io::Error::new(io::ErrorKind::NotFound, "File not found"));
            }
        };

        let data: Self = match toml::from_str(&contents) {
            Ok(d) => d,
            Err(err) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid TOML: {}", err),
                ));
            }
        };

        Ok(data)
    }
}
