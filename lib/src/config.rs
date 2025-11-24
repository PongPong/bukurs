use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::Path;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Custom user-agent string for HTTP requests
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            user_agent: default_user_agent(),
        }
    }
}

fn default_user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/605.1.15 (KHTML, like Gecko) \
     Version/18.5 Safari/605.1.15"
        .to_string()
}

impl Config {
    /// Load configuration from a file path
    pub fn load_from_path(path: &Path) -> Result<Self, Box<dyn Error>> {
        let contents = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    /// Load configuration from default location (~/.config/bukurs/config.yml)
    /// Falls back to default config if file doesn't exist
    pub fn load() -> Self {
        let config_path = crate::utils::get_config_dir().join("config.yml");

        if config_path.exists() {
            match Self::load_from_path(&config_path) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load config from {:?}: {}",
                        config_path, e
                    );
                    eprintln!("Using default configuration");
                    Self::default()
                }
            }
        } else {
            Self::default()
        }
    }

    /// Save configuration to a file path
    pub fn save_to_path(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)?;
        Ok(())
    }

    /// Save configuration to default location
    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let config_path = crate::utils::get_config_dir().join("config.yml");
        self.save_to_path(&config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.user_agent.contains("Mozilla"));
    }

    #[test]
    fn test_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path();

        let original = Config {
            user_agent: "Custom User Agent".to_string(),
        };

        original.save_to_path(config_path).unwrap();
        let loaded = Config::load_from_path(config_path).unwrap();

        assert_eq!(original.user_agent, loaded.user_agent);
    }

    #[test]
    fn test_load_invalid_yaml() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path();

        fs::write(config_path, "invalid: yaml: content:").unwrap();

        let result = Config::load_from_path(config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_partial_config() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_path = temp_file.path();

        // Write YAML with only some fields (user_agent missing)
        fs::write(config_path, "# Empty config\n").unwrap();

        let config = Config::load_from_path(config_path).unwrap();
        // Should use default for missing field
        assert_eq!(config.user_agent, default_user_agent());
    }
}
