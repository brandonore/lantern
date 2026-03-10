use std::path::PathBuf;

/// Returns ~/.config/lantern/
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("lantern")
}

/// Returns ~/.config/lantern/config.toml
pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// Returns ~/.local/share/lantern/
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("lantern")
}

/// Returns ~/.local/share/lantern/lantern.db
pub fn db_file() -> PathBuf {
    data_dir().join("lantern.db")
}
