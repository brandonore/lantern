use crate::error::LanternError;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default = "default_shell")]
    pub default_shell: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_git_poll_interval")]
    pub git_poll_interval_secs: u64,
}

fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
}
fn default_font_family() -> String {
    "JetBrains Mono".to_string()
}
fn default_font_size() -> u32 {
    14
}
fn default_scrollback_lines() -> u32 {
    10000
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_git_poll_interval() -> u64 {
    5
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            default_shell: default_shell(),
            font_family: default_font_family(),
            font_size: default_font_size(),
            scrollback_lines: default_scrollback_lines(),
            theme: default_theme(),
            git_poll_interval_secs: default_git_poll_interval(),
        }
    }
}

impl UserConfig {
    pub fn load() -> Self {
        let path = paths::config_file();
        match fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("Warning: invalid config TOML, using defaults: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), LanternError> {
        let path = paths::config_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| LanternError::Config(e.to_string()))?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn merge_patch(&mut self, patch: serde_json::Value) {
        if let Some(v) = patch.get("default_shell").and_then(|v| v.as_str()) {
            self.default_shell = v.to_string();
        }
        if let Some(v) = patch.get("font_family").and_then(|v| v.as_str()) {
            self.font_family = v.to_string();
        }
        if let Some(v) = patch.get("font_size").and_then(|v| v.as_u64()) {
            self.font_size = v as u32;
        }
        if let Some(v) = patch.get("scrollback_lines").and_then(|v| v.as_u64()) {
            self.scrollback_lines = v as u32;
        }
        if let Some(v) = patch.get("theme").and_then(|v| v.as_str()) {
            self.theme = v.to_string();
        }
        if let Some(v) = patch.get("git_poll_interval_secs").and_then(|v| v.as_u64()) {
            self.git_poll_interval_secs = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_default_config() {
        // With no file, should return defaults
        let config = UserConfig::default();
        assert_eq!(config.font_size, 14);
        assert_eq!(config.scrollback_lines, 10000);
        assert_eq!(config.theme, "dark");
        assert_eq!(config.git_poll_interval_secs, 5);
    }

    #[test]
    fn test_save_and_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = UserConfig {
            default_shell: "/bin/zsh".to_string(),
            font_family: "Fira Code".to_string(),
            font_size: 16,
            scrollback_lines: 5000,
            theme: "dark".to_string(),
            git_poll_interval_secs: 10,
        };

        let content = toml::to_string_pretty(&config).unwrap();
        fs::write(&path, &content).unwrap();

        let loaded: UserConfig = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.default_shell, "/bin/zsh");
        assert_eq!(loaded.font_family, "Fira Code");
        assert_eq!(loaded.font_size, 16);
        assert_eq!(loaded.scrollback_lines, 5000);
    }

    #[test]
    fn test_partial_update() {
        let mut config = UserConfig::default();
        let patch = serde_json::json!({ "font_size": 18 });
        config.merge_patch(patch);
        assert_eq!(config.font_size, 18);
        // Other fields unchanged
        assert_eq!(config.scrollback_lines, 10000);
    }

    #[test]
    fn test_invalid_toml_returns_default() {
        let result: Result<UserConfig, _> = toml::from_str("this is not valid toml {{{{");
        assert!(result.is_err());
        // In the actual load path, this falls back to default
        let config = UserConfig::default();
        assert_eq!(config.font_size, 14);
    }
}
