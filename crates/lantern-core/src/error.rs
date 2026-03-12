use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum LanternError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Repo already exists: {0}")]
    RepoAlreadyExists(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Repo not found: {0}")]
    RepoNotFound(String),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl Serialize for LanternError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
