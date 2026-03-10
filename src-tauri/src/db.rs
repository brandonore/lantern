use crate::error::LanternError;
use crate::paths;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type DbConn = Arc<Mutex<Connection>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSession {
    pub id: String,
    pub repo_id: String,
    pub title: String,
    pub shell: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLayout {
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_width: i32,
    pub window_height: i32,
    pub window_maximized: bool,
    pub sidebar_width: i32,
    pub active_repo_id: Option<String>,
}

impl Default for AppLayout {
    fn default() -> Self {
        Self {
            window_x: None,
            window_y: None,
            window_width: 1200,
            window_height: 800,
            window_maximized: false,
            sidebar_width: 250,
            active_repo_id: None,
        }
    }
}

pub fn init_db(path: Option<PathBuf>) -> Result<DbConn, LanternError> {
    let db_path = path.unwrap_or_else(paths::db_file);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    create_tables(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

fn create_tables(conn: &Connection) -> Result<(), LanternError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS repo (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            sort_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS terminal_session (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL REFERENCES repo(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            shell TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS app_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            window_x INTEGER,
            window_y INTEGER,
            window_width INTEGER DEFAULT 1200,
            window_height INTEGER DEFAULT 800,
            window_maximized INTEGER DEFAULT 0,
            sidebar_width INTEGER DEFAULT 250,
            active_repo_id TEXT REFERENCES repo(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS active_tab (
            repo_id TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
            session_id TEXT NOT NULL REFERENCES terminal_session(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );
        INSERT OR IGNORE INTO schema_version VALUES (1);
        ",
    )?;
    Ok(())
}

// ── Repo CRUD ──

pub fn add_repo(conn: &DbConn, path: &str) -> Result<Repo, LanternError> {
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err(LanternError::PathNotFound(path.to_string()));
    }

    let name = path_buf
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    let id = uuid::Uuid::new_v4().to_string();
    let db = conn.lock().unwrap();

    // Check for duplicate
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM repo WHERE path = ?1)",
        params![path],
        |row| row.get(0),
    )?;
    if exists {
        return Err(LanternError::RepoAlreadyExists(path.to_string()));
    }

    let max_order: i32 = db
        .query_row("SELECT COALESCE(MAX(sort_order), -1) FROM repo", [], |row| {
            row.get(0)
        })?;

    db.execute(
        "INSERT INTO repo (id, name, path, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, path, max_order + 1],
    )?;

    Ok(Repo {
        id,
        name,
        path: path.to_string(),
        sort_order: max_order + 1,
    })
}

pub fn remove_repo(conn: &DbConn, id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected = db.execute("DELETE FROM repo WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(LanternError::RepoNotFound(id.to_string()));
    }
    Ok(())
}

pub fn list_repos(conn: &DbConn) -> Result<Vec<Repo>, LanternError> {
    let db = conn.lock().unwrap();
    let mut stmt = db.prepare("SELECT id, name, path, sort_order FROM repo ORDER BY sort_order")?;
    let repos = stmt
        .query_map([], |row| {
            Ok(Repo {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                sort_order: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(repos)
}

pub fn reorder_repos(conn: &DbConn, ids: &[String]) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    for (i, id) in ids.iter().enumerate() {
        db.execute(
            "UPDATE repo SET sort_order = ?1 WHERE id = ?2",
            params![i as i32, id],
        )?;
    }
    Ok(())
}

// ── Terminal Session CRUD ──

pub fn create_session(
    conn: &DbConn,
    repo_id: &str,
    title: &str,
    shell: Option<&str>,
) -> Result<TerminalSession, LanternError> {
    let db = conn.lock().unwrap();

    // Verify repo exists
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM repo WHERE id = ?1)",
        params![repo_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(LanternError::RepoNotFound(repo_id.to_string()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let max_order: i32 = db.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) FROM terminal_session WHERE repo_id = ?1",
        params![repo_id],
        |row| row.get(0),
    )?;

    db.execute(
        "INSERT INTO terminal_session (id, repo_id, title, shell, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, repo_id, title, shell, max_order + 1],
    )?;

    Ok(TerminalSession {
        id,
        repo_id: repo_id.to_string(),
        title: title.to_string(),
        shell: shell.map(|s| s.to_string()),
        sort_order: max_order + 1,
    })
}

pub fn list_sessions(conn: &DbConn, repo_id: &str) -> Result<Vec<TerminalSession>, LanternError> {
    let db = conn.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT id, repo_id, title, shell, sort_order FROM terminal_session WHERE repo_id = ?1 ORDER BY sort_order",
    )?;
    let sessions = stmt
        .query_map(params![repo_id], |row| {
            Ok(TerminalSession {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                title: row.get(2)?,
                shell: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sessions)
}

pub fn close_session(conn: &DbConn, session_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected = db.execute(
        "DELETE FROM terminal_session WHERE id = ?1",
        params![session_id],
    )?;
    if affected == 0 {
        return Err(LanternError::SessionNotFound(session_id.to_string()));
    }
    Ok(())
}

pub fn rename_session(conn: &DbConn, session_id: &str, title: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected = db.execute(
        "UPDATE terminal_session SET title = ?1 WHERE id = ?2",
        params![title, session_id],
    )?;
    if affected == 0 {
        return Err(LanternError::SessionNotFound(session_id.to_string()));
    }
    Ok(())
}

// ── Active Tab ──

pub fn set_active_tab(
    conn: &DbConn,
    repo_id: &str,
    session_id: &str,
) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO active_tab (repo_id, session_id) VALUES (?1, ?2)",
        params![repo_id, session_id],
    )?;
    Ok(())
}

pub fn get_active_tab(conn: &DbConn, repo_id: &str) -> Result<Option<String>, LanternError> {
    let db = conn.lock().unwrap();
    let result = db.query_row(
        "SELECT session_id FROM active_tab WHERE repo_id = ?1",
        params![repo_id],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// ── Layout ──

pub fn save_layout(conn: &DbConn, layout: &AppLayout) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO app_state (id, window_x, window_y, window_width, window_height, window_maximized, sidebar_width, active_repo_id)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            layout.window_x,
            layout.window_y,
            layout.window_width,
            layout.window_height,
            layout.window_maximized as i32,
            layout.sidebar_width,
            layout.active_repo_id,
        ],
    )?;
    Ok(())
}

pub fn load_layout(conn: &DbConn) -> Result<Option<AppLayout>, LanternError> {
    let db = conn.lock().unwrap();
    let result = db.query_row(
        "SELECT window_x, window_y, window_width, window_height, window_maximized, sidebar_width, active_repo_id FROM app_state WHERE id = 1",
        [],
        |row| {
            Ok(AppLayout {
                window_x: row.get(0)?,
                window_y: row.get(1)?,
                window_width: row.get(2)?,
                window_height: row.get(3)?,
                window_maximized: row.get::<_, i32>(4)? != 0,
                sidebar_width: row.get(5)?,
                active_repo_id: row.get(6)?,
            })
        },
    );
    match result {
        Ok(layout) => Ok(Some(layout)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> DbConn {
        init_db(Some(PathBuf::from(":memory:"))).unwrap()
    }

    fn add_test_repo(conn: &DbConn) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        add_repo(conn, dir.path().to_str().unwrap()).unwrap()
    }

    #[test]
    fn test_init_creates_tables() {
        let conn = test_db();
        let db = conn.lock().unwrap();
        let tables: Vec<String> = db
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(tables.contains(&"repo".to_string()));
        assert!(tables.contains(&"terminal_session".to_string()));
        assert!(tables.contains(&"app_state".to_string()));
        assert!(tables.contains(&"active_tab".to_string()));
        assert!(tables.contains(&"schema_version".to_string()));
    }

    #[test]
    fn test_add_repo() {
        let conn = test_db();
        let dir = tempfile::tempdir().unwrap();
        let repo = add_repo(&conn, dir.path().to_str().unwrap()).unwrap();
        assert!(!repo.id.is_empty());
        assert_eq!(repo.path, dir.path().to_str().unwrap());
    }

    #[test]
    fn test_add_duplicate_repo_errors() {
        let conn = test_db();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        add_repo(&conn, path).unwrap();
        let result = add_repo(&conn, path);
        assert!(matches!(result, Err(LanternError::RepoAlreadyExists(_))));
    }

    #[test]
    fn test_remove_repo_cascades_sessions() {
        let conn = test_db();
        let repo = add_test_repo(&conn);
        create_session(&conn, &repo.id, "Terminal 1", None).unwrap();
        create_session(&conn, &repo.id, "Terminal 2", None).unwrap();
        remove_repo(&conn, &repo.id).unwrap();
        let sessions = list_sessions(&conn, &repo.id).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_list_repos_ordered() {
        let conn = test_db();
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let d3 = tempfile::tempdir().unwrap();
        let r1 = add_repo(&conn, d1.path().to_str().unwrap()).unwrap();
        let r2 = add_repo(&conn, d2.path().to_str().unwrap()).unwrap();
        let r3 = add_repo(&conn, d3.path().to_str().unwrap()).unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos.len(), 3);
        assert_eq!(repos[0].id, r1.id);
        assert_eq!(repos[1].id, r2.id);
        assert_eq!(repos[2].id, r3.id);
    }

    #[test]
    fn test_reorder_repos() {
        let conn = test_db();
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let r1 = add_repo(&conn, d1.path().to_str().unwrap()).unwrap();
        let r2 = add_repo(&conn, d2.path().to_str().unwrap()).unwrap();
        reorder_repos(&conn, &[r2.id.clone(), r1.id.clone()]).unwrap();
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos[0].id, r2.id);
        assert_eq!(repos[1].id, r1.id);
    }

    #[test]
    fn test_repo_path_not_found() {
        let conn = test_db();
        let result = add_repo(&conn, "/nonexistent/path/12345");
        assert!(matches!(result, Err(LanternError::PathNotFound(_))));
    }

    #[test]
    fn test_create_session() {
        let conn = test_db();
        let repo = add_test_repo(&conn);
        let session = create_session(&conn, &repo.id, "Terminal 1", None).unwrap();
        assert!(!session.id.is_empty());
        assert_eq!(session.repo_id, repo.id);
        assert_eq!(session.title, "Terminal 1");
    }

    #[test]
    fn test_list_sessions_for_repo() {
        let conn = test_db();
        let repo = add_test_repo(&conn);
        create_session(&conn, &repo.id, "T1", None).unwrap();
        create_session(&conn, &repo.id, "T2", None).unwrap();
        create_session(&conn, &repo.id, "T3", None).unwrap();
        let sessions = list_sessions(&conn, &repo.id).unwrap();
        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0].title, "T1");
        assert_eq!(sessions[1].title, "T2");
        assert_eq!(sessions[2].title, "T3");
    }

    #[test]
    fn test_close_session_removes_from_db() {
        let conn = test_db();
        let repo = add_test_repo(&conn);
        let session = create_session(&conn, &repo.id, "T1", None).unwrap();
        close_session(&conn, &session.id).unwrap();
        let sessions = list_sessions(&conn, &repo.id).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_rename_session() {
        let conn = test_db();
        let repo = add_test_repo(&conn);
        let session = create_session(&conn, &repo.id, "Old Name", None).unwrap();
        rename_session(&conn, &session.id, "New Name").unwrap();
        let sessions = list_sessions(&conn, &repo.id).unwrap();
        assert_eq!(sessions[0].title, "New Name");
    }

    #[test]
    fn test_active_tab_persists() {
        let conn = test_db();
        let repo = add_test_repo(&conn);
        let session = create_session(&conn, &repo.id, "T1", None).unwrap();
        set_active_tab(&conn, &repo.id, &session.id).unwrap();
        let active = get_active_tab(&conn, &repo.id).unwrap();
        assert_eq!(active, Some(session.id));
    }

    #[test]
    fn test_save_and_load_layout() {
        let conn = test_db();
        let layout = AppLayout {
            window_x: Some(100),
            window_y: Some(200),
            window_width: 1400,
            window_height: 900,
            window_maximized: true,
            sidebar_width: 300,
            active_repo_id: None,
        };
        save_layout(&conn, &layout).unwrap();
        let loaded = load_layout(&conn).unwrap().unwrap();
        assert_eq!(loaded.window_x, Some(100));
        assert_eq!(loaded.window_y, Some(200));
        assert_eq!(loaded.window_width, 1400);
        assert_eq!(loaded.window_height, 900);
        assert!(loaded.window_maximized);
        assert_eq!(loaded.sidebar_width, 300);
    }

    #[test]
    fn test_load_layout_no_existing() {
        let conn = test_db();
        let layout = load_layout(&conn).unwrap();
        assert!(layout.is_none());
    }

    #[test]
    fn test_layout_survives_repo_delete() {
        let conn = test_db();
        let repo = add_test_repo(&conn);
        let layout = AppLayout {
            active_repo_id: Some(repo.id.clone()),
            ..Default::default()
        };
        save_layout(&conn, &layout).unwrap();
        remove_repo(&conn, &repo.id).unwrap();
        let loaded = load_layout(&conn).unwrap().unwrap();
        // active_repo_id set to NULL via ON DELETE SET NULL
        assert!(loaded.active_repo_id.is_none());
    }
}
