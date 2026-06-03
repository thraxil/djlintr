use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_indent", deserialize_with = "deserialize_usize")]
    pub indent: usize,
    #[serde(
        default = "default_max_line_length",
        deserialize_with = "deserialize_usize"
    )]
    pub max_line_length: usize,
    #[serde(
        default = "default_max_attribute_length",
        deserialize_with = "deserialize_usize"
    )]
    pub max_attribute_length: usize,
    #[serde(default, deserialize_with = "deserialize_vec_string")]
    pub ignore: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_vec_string")]
    pub include: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_vec_string")]
    pub custom_blocks: Vec<String>,
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(
        default = "default_max_blank_lines",
        deserialize_with = "deserialize_usize"
    )]
    pub max_blank_lines: usize,
    #[serde(default)]
    pub close_void_tags: bool,
    #[serde(default)]
    pub require_closed_blocks: bool,
    #[serde(default)]
    pub use_gitignore: bool,
    #[serde(default)]
    pub better_attribute_parsing: bool,
}

fn deserialize_usize<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IntOrString {
        Int(usize),
        String(String),
    }

    match IntOrString::deserialize(deserializer)? {
        IntOrString::Int(i) => Ok(i),
        IntOrString::String(s) => s.parse::<usize>().map_err(serde::de::Error::custom),
    }
}

fn deserialize_vec_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum VecOrString {
        Vec(Vec<String>),
        String(String),
    }

    match VecOrString::deserialize(deserializer)? {
        VecOrString::Vec(v) => Ok(v),
        VecOrString::String(s) => Ok(s
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()),
    }
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
            close_void_tags: false,
            require_closed_blocks: false,
            use_gitignore: false,
            better_attribute_parsing: false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_djlintrc_comma_separated() {
        let djlintrc = r#"{
            "custom_blocks": "component,endcomponent",
            "max_attribute_length": "40",
            "ignore": "H014,H013"
        }"#;
        let path = ".djlintrc_test_json";
        fs::write(path, djlintrc).unwrap();

        let config = Config::from_file(path);
        fs::remove_file(path).unwrap();

        assert!(config.is_ok(), "Config loading failed: {:?}", config.err());
        let config = config.unwrap();
        assert_eq!(config.custom_blocks, vec!["component", "endcomponent"]);
        assert_eq!(config.max_attribute_length, 40);
        assert_eq!(config.ignore, vec!["H014", "H013"]);
    }

    #[test]
    fn test_load_pyproject_toml() {
        let pyproject = r#"
[tool.djlint]
ignore = "H014,H013"
max_attribute_length = 40
"#;
        let path = "pyproject.toml_test";
        fs::write(path, pyproject).unwrap();

        let config = Config::from_pyproject(path);
        fs::remove_file(path).unwrap();

        assert!(config.is_ok(), "Config loading failed: {:?}", config.err());
        let config = config.unwrap();
        assert_eq!(config.ignore, vec!["H014", "H013"]);
        assert_eq!(config.max_attribute_length, 40);
    }
}
