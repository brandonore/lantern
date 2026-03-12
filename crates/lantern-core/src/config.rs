use crate::error::LanternError;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f64,
    #[serde(default = "default_terminal_latency_mode")]
    pub terminal_latency_mode: String,
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
    "nord-dark".to_string()
}

fn default_git_poll_interval() -> u64 {
    5
}

fn default_ui_scale() -> f64 {
    1.0
}

fn default_terminal_latency_mode() -> String {
    "low-latency".to_string()
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
            ui_scale: default_ui_scale(),
            terminal_latency_mode: default_terminal_latency_mode(),
        }
    }
}

impl UserConfig {
    pub fn load() -> Self {
        let path = paths::config_file();
        match fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|error| {
                eprintln!("Warning: invalid config TOML, using defaults: {error}");
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
            .map_err(|error| LanternError::Config(error.to_string()))?;
        fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_expected_values() {
        let config = UserConfig::default();
        assert_eq!(config.font_family, "JetBrains Mono");
        assert_eq!(config.font_size, 14);
        assert_eq!(config.scrollback_lines, 10000);
        assert_eq!(config.terminal_latency_mode, "low-latency");
    }
}
