use crate::error::LanternError;
use crate::paths;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type DbConn = Arc<Mutex<Connection>>;
const CURRENT_SCHEMA_VERSION: i32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub sort_order: i32,
    pub group_id: Option<String>,
    pub is_default: bool,
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
    pub sidebar_collapsed: bool,
    pub active_repo_id: Option<String>,
    pub collapsed_group_ids: Vec<String>,
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
            sidebar_collapsed: false,
            active_repo_id: None,
            collapsed_group_ids: Vec::new(),
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
            sidebar_collapsed INTEGER NOT NULL DEFAULT 0,
            active_repo_id TEXT REFERENCES repo(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS active_tab (
            repo_id TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
            session_id TEXT NOT NULL REFERENCES terminal_session(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        );
        ",
    )?;
    migrate_schema(conn)?;
    Ok(())
}

fn read_schema_version(conn: &Connection) -> Result<i32, LanternError> {
    let version = conn.query_row("SELECT MAX(version) FROM schema_version", [], |row| {
        row.get::<_, Option<i32>>(0)
    })?;
    Ok(version.unwrap_or(0))
}

fn table_has_column(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, LanternError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;

    for column in columns {
        if column? == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn ensure_schema_version(conn: &Connection) -> Result<i32, LanternError> {
    let version = read_schema_version(conn)?;
    if version > 0 {
        return Ok(version);
    }

    let inferred_version = if table_has_column(conn, "app_state", "collapsed_group_ids")? {
        CURRENT_SCHEMA_VERSION
    } else if table_has_column(conn, "repo", "group_id")? {
        3
    } else if table_has_column(conn, "app_state", "sidebar_collapsed")? {
        2
    } else {
        1
    };

    conn.execute(
        "INSERT INTO schema_version (version) VALUES (?1)",
        params![inferred_version],
    )?;

    Ok(inferred_version)
}

fn migrate_schema(conn: &Connection) -> Result<(), LanternError> {
    let version = ensure_schema_version(conn)?;

    if version < 2 {
        if !table_has_column(conn, "app_state", "sidebar_collapsed")? {
            conn.execute(
                "ALTER TABLE app_state ADD COLUMN sidebar_collapsed INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        conn.execute(
            "INSERT INTO schema_version (version) VALUES (2)",
            [],
        )?;
    }

    if version < 3 {
        if !table_has_column(conn, "repo", "group_id")? {
            conn.execute("ALTER TABLE repo ADD COLUMN group_id TEXT", [])?;
        }
        if !table_has_column(conn, "repo", "is_default")? {
            conn.execute(
                "ALTER TABLE repo ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        conn.execute(
            "INSERT OR REPLACE INTO schema_version (version) VALUES (3)",
            [],
        )?;
    }

    if version < 4 {
        if !table_has_column(conn, "app_state", "collapsed_group_ids")? {
            conn.execute(
                "ALTER TABLE app_state ADD COLUMN collapsed_group_ids TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }

        conn.execute(
            "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
            params![CURRENT_SCHEMA_VERSION],
        )?;
    }

    Ok(())
}

// ── Repo CRUD ──

pub fn add_repo(conn: &DbConn, path: &str) -> Result<Repo, LanternError> {
    add_repo_grouped(conn, path, None, false)
}

pub fn add_repo_grouped(
    conn: &DbConn,
    path: &str,
    group_id: Option<&str>,
    is_default: bool,
) -> Result<Repo, LanternError> {
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
        "INSERT INTO repo (id, name, path, sort_order, group_id, is_default) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, path, max_order + 1, group_id, is_default as i32],
    )?;

    Ok(Repo {
        id,
        name,
        path: path.to_string(),
        sort_order: max_order + 1,
        group_id: group_id.map(|s| s.to_string()),
        is_default,
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
    let mut stmt = db.prepare(
        "SELECT id, name, path, sort_order, group_id, is_default FROM repo ORDER BY COALESCE((SELECT MIN(r2.sort_order) FROM repo r2 WHERE r2.group_id = repo.group_id), sort_order), is_default DESC, sort_order",
    )?;
    let repos = stmt
        .query_map([], |row| {
            Ok(Repo {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                sort_order: row.get(3)?,
                group_id: row.get(4)?,
                is_default: row.get::<_, i32>(5)? != 0,
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

/// Find the group_id of any existing repo whose path matches one of the given paths.
pub fn find_group_id_by_paths(conn: &DbConn, paths: &[String]) -> Result<Option<String>, LanternError> {
    if paths.is_empty() {
        return Ok(None);
    }
    let db = conn.lock().unwrap();
    for path in paths {
        let result = db.query_row(
            "SELECT group_id FROM repo WHERE path = ?1 AND group_id IS NOT NULL",
            params![path],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(gid) => return Ok(Some(gid)),
            Err(rusqlite::Error::QueryReturnedNoRows) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(None)
}

/// Update a repo's group_id.
pub fn set_repo_group(conn: &DbConn, repo_id: &str, group_id: &str, is_default: bool) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "UPDATE repo SET group_id = ?1, is_default = ?2 WHERE id = ?3",
        params![group_id, is_default as i32, repo_id],
    )?;
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
    let collapsed_json = serde_json::to_string(&layout.collapsed_group_ids)
        .unwrap_or_else(|_| "[]".to_string());
    db.execute(
        "INSERT OR REPLACE INTO app_state (id, window_x, window_y, window_width, window_height, window_maximized, sidebar_width, sidebar_collapsed, active_repo_id, collapsed_group_ids)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            layout.window_x,
            layout.window_y,
            layout.window_width,
            layout.window_height,
            layout.window_maximized as i32,
            layout.sidebar_width,
            layout.sidebar_collapsed as i32,
            layout.active_repo_id,
            collapsed_json,
        ],
    )?;
    Ok(())
}

pub fn load_layout(conn: &DbConn) -> Result<Option<AppLayout>, LanternError> {
    let db = conn.lock().unwrap();
    let result = db.query_row(
        "SELECT window_x, window_y, window_width, window_height, window_maximized, sidebar_width, sidebar_collapsed, active_repo_id, collapsed_group_ids FROM app_state WHERE id = 1",
        [],
        |row| {
            let collapsed_json: String = row.get::<_, Option<String>>(8)?.unwrap_or_else(|| "[]".to_string());
            let collapsed_group_ids: Vec<String> = serde_json::from_str(&collapsed_json).unwrap_or_default();
            Ok(AppLayout {
                window_x: row.get(0)?,
                window_y: row.get(1)?,
                window_width: row.get(2)?,
                window_height: row.get(3)?,
                window_maximized: row.get::<_, i32>(4)? != 0,
                sidebar_width: row.get(5)?,
                sidebar_collapsed: row.get::<_, i32>(6)? != 0,
                active_repo_id: row.get(7)?,
                collapsed_group_ids,
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
            sidebar_collapsed: true,
            active_repo_id: None,
            collapsed_group_ids: vec!["g1".to_string(), "standalone-r1".to_string()],
        };
        save_layout(&conn, &layout).unwrap();
        let loaded = load_layout(&conn).unwrap().unwrap();
        assert_eq!(loaded.window_x, Some(100));
        assert_eq!(loaded.window_y, Some(200));
        assert_eq!(loaded.window_width, 1400);
        assert_eq!(loaded.window_height, 900);
        assert!(loaded.window_maximized);
        assert_eq!(loaded.sidebar_width, 300);
        assert!(loaded.sidebar_collapsed);
        assert_eq!(loaded.collapsed_group_ids, vec!["g1", "standalone-r1"]);
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
        // Verify default has empty collapsed_group_ids
        assert!(layout.collapsed_group_ids.is_empty());
        save_layout(&conn, &layout).unwrap();
        remove_repo(&conn, &repo.id).unwrap();
        let loaded = load_layout(&conn).unwrap().unwrap();
        // active_repo_id set to NULL via ON DELETE SET NULL
        assert!(loaded.active_repo_id.is_none());
    }

    #[test]
    fn test_migrate_layout_adds_sidebar_collapsed_column() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE app_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                window_x INTEGER,
                window_y INTEGER,
                window_width INTEGER DEFAULT 1200,
                window_height INTEGER DEFAULT 800,
                window_maximized INTEGER DEFAULT 0,
                sidebar_width INTEGER DEFAULT 250,
                active_repo_id TEXT
            );

            CREATE TABLE repo (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                sort_order INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE schema_version (
                version INTEGER PRIMARY KEY
            );

            INSERT INTO app_state (
                id,
                window_x,
                window_y,
                window_width,
                window_height,
                window_maximized,
                sidebar_width,
                active_repo_id
            ) VALUES (1, 10, 20, 1200, 800, 0, 260, NULL);

            INSERT INTO schema_version (version) VALUES (1);
            ",
        )
        .unwrap();

        create_tables(&conn).unwrap();
        assert!(table_has_column(&conn, "app_state", "sidebar_collapsed").unwrap());
        assert!(table_has_column(&conn, "repo", "group_id").unwrap());
        assert!(table_has_column(&conn, "repo", "is_default").unwrap());
        assert_eq!(read_schema_version(&conn).unwrap(), CURRENT_SCHEMA_VERSION);

        let conn = Arc::new(Mutex::new(conn));
        let layout = load_layout(&conn).unwrap().unwrap();
        assert!(!layout.sidebar_collapsed);
        assert_eq!(layout.sidebar_width, 260);
    }

    #[test]
    fn test_migration_v3_adds_group_columns() {
        // Simulate a v2 database (has sidebar_collapsed but no group_id)
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE repo (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE terminal_session (
                id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL REFERENCES repo(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                shell TEXT,
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE app_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                window_x INTEGER,
                window_y INTEGER,
                window_width INTEGER DEFAULT 1200,
                window_height INTEGER DEFAULT 800,
                window_maximized INTEGER DEFAULT 0,
                sidebar_width INTEGER DEFAULT 250,
                sidebar_collapsed INTEGER NOT NULL DEFAULT 0,
                active_repo_id TEXT
            );
            CREATE TABLE active_tab (
                repo_id TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
                session_id TEXT NOT NULL REFERENCES terminal_session(id) ON DELETE CASCADE
            );
            CREATE TABLE schema_version (
                version INTEGER PRIMARY KEY
            );
            INSERT INTO schema_version (version) VALUES (2);
            ",
        )
        .unwrap();

        // Insert a repo before migration
        conn.execute(
            "INSERT INTO repo (id, name, path, sort_order) VALUES ('r1', 'test', '/tmp/test', 0)",
            [],
        )
        .unwrap();

        create_tables(&conn).unwrap();
        assert!(table_has_column(&conn, "repo", "group_id").unwrap());
        assert!(table_has_column(&conn, "repo", "is_default").unwrap());
        assert_eq!(read_schema_version(&conn).unwrap(), CURRENT_SCHEMA_VERSION);

        // Existing repo should have NULL group_id and is_default=0
        let conn = Arc::new(Mutex::new(conn));
        let repos = list_repos(&conn).unwrap();
        assert_eq!(repos.len(), 1);
        assert!(repos[0].group_id.is_none());
        assert!(!repos[0].is_default);
    }

    #[test]
    fn test_add_repo_with_group() {
        let conn = test_db();
        let dir = tempfile::tempdir().unwrap();
        let gid = "test-group-id";
        let repo = add_repo_grouped(&conn, dir.path().to_str().unwrap(), Some(gid), true).unwrap();
        assert_eq!(repo.group_id.as_deref(), Some(gid));
        assert!(repo.is_default);
    }

    #[test]
    fn test_list_repos_groups_together() {
        let conn = test_db();
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let d3 = tempfile::tempdir().unwrap();
        let gid = "g1";
        let r_a = add_repo_grouped(&conn, d1.path().to_str().unwrap(), Some(gid), false).unwrap();
        let _r_b = add_repo(&conn, d2.path().to_str().unwrap()).unwrap();
        let r_c = add_repo_grouped(&conn, d3.path().to_str().unwrap(), Some(gid), false).unwrap();
        let repos = list_repos(&conn).unwrap();
        // A and C should be adjacent (both have group_id g1)
        let positions: Vec<usize> = repos
            .iter()
            .enumerate()
            .filter(|(_, r)| r.id == r_a.id || r.id == r_c.id)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(positions[1] - positions[0], 1);
    }

    #[test]
    fn test_list_repos_default_first_in_group() {
        let conn = test_db();
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let d3 = tempfile::tempdir().unwrap();
        let gid = "g1";
        let _r1 = add_repo_grouped(&conn, d1.path().to_str().unwrap(), Some(gid), false).unwrap();
        let _r2 = add_repo_grouped(&conn, d2.path().to_str().unwrap(), Some(gid), false).unwrap();
        let r3 = add_repo_grouped(&conn, d3.path().to_str().unwrap(), Some(gid), true).unwrap();
        let repos = list_repos(&conn).unwrap();
        let grouped: Vec<_> = repos.iter().filter(|r| r.group_id.as_deref() == Some(gid)).collect();
        assert_eq!(grouped[0].id, r3.id);
        assert!(grouped[0].is_default);
    }

    #[test]
    fn test_remove_repo_from_group_keeps_siblings() {
        let conn = test_db();
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let d3 = tempfile::tempdir().unwrap();
        let gid = "g1";
        let r1 = add_repo_grouped(&conn, d1.path().to_str().unwrap(), Some(gid), true).unwrap();
        let _r2 = add_repo_grouped(&conn, d2.path().to_str().unwrap(), Some(gid), false).unwrap();
        let _r3 = add_repo_grouped(&conn, d3.path().to_str().unwrap(), Some(gid), false).unwrap();
        remove_repo(&conn, &r1.id).unwrap();
        let repos = list_repos(&conn).unwrap();
        let grouped: Vec<_> = repos.iter().filter(|r| r.group_id.as_deref() == Some(gid)).collect();
        assert_eq!(grouped.len(), 2);
    }

    #[test]
    fn test_find_group_id_by_paths() {
        let conn = test_db();
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let gid = "g1";
        add_repo_grouped(&conn, d1.path().to_str().unwrap(), Some(gid), true).unwrap();
        let result = find_group_id_by_paths(
            &conn,
            &[d1.path().to_str().unwrap().to_string(), d2.path().to_str().unwrap().to_string()],
        )
        .unwrap();
        assert_eq!(result, Some(gid.to_string()));
    }

    #[test]
    fn test_find_group_id_by_paths_none() {
        let conn = test_db();
        let d1 = tempfile::tempdir().unwrap();
        add_repo(&conn, d1.path().to_str().unwrap()).unwrap();
        let result = find_group_id_by_paths(
            &conn,
            &[d1.path().to_str().unwrap().to_string()],
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_set_repo_group() {
        let conn = test_db();
        let repo = add_test_repo(&conn);
        assert!(repo.group_id.is_none());
        set_repo_group(&conn, &repo.id, "new-group", true).unwrap();
        let repos = list_repos(&conn).unwrap();
        let updated = repos.iter().find(|r| r.id == repo.id).unwrap();
        assert_eq!(updated.group_id.as_deref(), Some("new-group"));
        assert!(updated.is_default);
    }
}
