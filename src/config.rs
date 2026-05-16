use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_indent")]
    pub indent: usize,
    #[serde(default = "default_max_line_length")]
    pub max_line_length: usize,
    #[serde(default = "default_max_attribute_length")]
    pub max_attribute_length: usize,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub custom_blocks: Vec<String>,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default = "default_max_blank_lines")]
    pub max_blank_lines: usize,
}

fn default_indent() -> usize {
    4
}
fn default_max_line_length() -> usize {
    120
}
fn default_max_attribute_length() -> usize {
    70
}
fn default_profile() -> String {
    "html".to_string()
}
fn default_max_blank_lines() -> usize {
    1
}

impl Default for Config {
    fn default() -> Self {
        Self {
            indent: 4,
            max_line_length: 120,
            max_attribute_length: 70,
            ignore: Vec::new(),
            include: Vec::new(),
            custom_blocks: Vec::new(),
            profile: "html".to_string(),
            max_blank_lines: 1,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        // Try to find .djlintrc or pyproject.toml in current directory
        if let Ok(config) = Self::from_file(".djlintrc") {
            return config;
        }
        if let Ok(config) = Self::from_pyproject("pyproject.toml") {
            return config;
        }
        Self::default()
    }

    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    fn from_pyproject<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let value: serde_json::Value = toml::from_str(&content)?;

        // djlint config in pyproject.toml can be under [tool.djlint] or [tool.djlintr]
        if let Some(tool) = value.get("tool") {
            if let Some(djlint) = tool.get("djlintr").or_else(|| tool.get("djlint")) {
                let config: Config = serde_json::from_value(djlint.clone())?;
                return Ok(config);
            }
        }
        anyhow::bail!("No [tool.djlint] or [tool.djlintr] section in pyproject.toml")
    }
}
