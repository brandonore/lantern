use crate::error::LanternError;
use crate::models::{AppLayout, NativeSplitOrientation, NativeSplitState, Repo, TerminalSession};
use crate::paths;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub type DbConn = Arc<Mutex<Connection>>;
const CURRENT_SCHEMA_VERSION: i32 = 5;

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
            sort_order INTEGER NOT NULL DEFAULT 0,
            group_id TEXT,
            is_default INTEGER NOT NULL DEFAULT 0
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
            active_repo_id TEXT REFERENCES repo(id) ON DELETE SET NULL,
            collapsed_group_ids TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS active_tab (
            repo_id TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
            session_id TEXT NOT NULL REFERENCES terminal_session(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS native_terminal_split (
            repo_id TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
            visible_session_ids TEXT NOT NULL DEFAULT '[]',
            orientation TEXT NOT NULL DEFAULT 'horizontal',
            divider_position INTEGER,
            secondary_divider_position INTEGER,
            divider_positions TEXT NOT NULL DEFAULT '[]'
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

    let inferred_version = if table_has_column(conn, "native_terminal_split", "divider_positions")?
    {
        CURRENT_SCHEMA_VERSION
    } else if table_has_column(conn, "app_state", "collapsed_group_ids")? {
        4
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

    if version < 2 && !table_has_column(conn, "app_state", "sidebar_collapsed")? {
        conn.execute(
            "ALTER TABLE app_state ADD COLUMN sidebar_collapsed INTEGER NOT NULL DEFAULT 0",
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
    }

    if version < 4 && !table_has_column(conn, "app_state", "collapsed_group_ids")? {
        conn.execute(
            "ALTER TABLE app_state ADD COLUMN collapsed_group_ids TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }

    conn.execute(
        "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
        params![CURRENT_SCHEMA_VERSION],
    )?;

    if !table_has_column(conn, "native_terminal_split", "orientation")? {
        conn.execute(
            "ALTER TABLE native_terminal_split ADD COLUMN orientation TEXT NOT NULL DEFAULT 'horizontal'",
            [],
        )?;
    }

    if !table_has_column(conn, "native_terminal_split", "divider_position")? {
        conn.execute(
            "ALTER TABLE native_terminal_split ADD COLUMN divider_position INTEGER",
            [],
        )?;
    }

    if !table_has_column(conn, "native_terminal_split", "secondary_divider_position")? {
        conn.execute(
            "ALTER TABLE native_terminal_split ADD COLUMN secondary_divider_position INTEGER",
            [],
        )?;
    }

    if !table_has_column(conn, "native_terminal_split", "divider_positions")? {
        conn.execute(
            "ALTER TABLE native_terminal_split ADD COLUMN divider_positions TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }

    Ok(())
}

pub fn list_repos(conn: &DbConn) -> Result<Vec<Repo>, LanternError> {
    let db = conn.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT id, name, path, sort_order, group_id, is_default
         FROM repo
         ORDER BY
            COALESCE(
                (SELECT MIN(grouped_repo.sort_order)
                 FROM repo grouped_repo
                 WHERE grouped_repo.group_id = repo.group_id),
                sort_order
            ),
            is_default DESC,
            sort_order ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(Repo {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            sort_order: row.get(3)?,
            group_id: row.get(4)?,
            is_default: row.get::<_, i32>(5)? != 0,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

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
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());

    let db = conn.lock().unwrap();
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM repo WHERE path = ?1)",
        params![path],
        |row| row.get(0),
    )?;
    if exists {
        return Err(LanternError::RepoAlreadyExists(path.to_string()));
    }

    let sort_order: i32 = db.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM repo",
        [],
        |row| row.get(0),
    )?;
    let repo = Repo {
        id: Uuid::new_v4().to_string(),
        name,
        path: path.to_string(),
        sort_order,
        group_id: group_id.map(str::to_string),
        is_default,
    };

    db.execute(
        "INSERT INTO repo (id, name, path, sort_order, group_id, is_default)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            repo.id,
            repo.name,
            repo.path,
            repo.sort_order,
            repo.group_id,
            repo.is_default as i32,
        ],
    )?;

    Ok(repo)
}

pub fn remove_repo(conn: &DbConn, repo_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected_rows = db.execute("DELETE FROM repo WHERE id = ?1", params![repo_id])?;

    if affected_rows == 0 {
        return Err(LanternError::RepoNotFound(repo_id.to_string()));
    }

    Ok(())
}

pub fn reorder_repos(conn: &DbConn, repo_ids: &[String]) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    for (sort_order, repo_id) in repo_ids.iter().enumerate() {
        db.execute(
            "UPDATE repo SET sort_order = ?1 WHERE id = ?2",
            params![sort_order as i32, repo_id],
        )?;
    }

    Ok(())
}

pub fn find_group_id_by_paths(
    conn: &DbConn,
    paths: &[String],
) -> Result<Option<String>, LanternError> {
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
            Ok(group_id) => return Ok(Some(group_id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Ok(None)
}

pub fn set_repo_group(
    conn: &DbConn,
    repo_id: &str,
    group_id: &str,
    is_default: bool,
) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected_rows = db.execute(
        "UPDATE repo SET group_id = ?1, is_default = ?2 WHERE id = ?3",
        params![group_id, is_default as i32, repo_id],
    )?;

    if affected_rows == 0 {
        return Err(LanternError::RepoNotFound(repo_id.to_string()));
    }

    Ok(())
}

pub fn list_sessions(conn: &DbConn, repo_id: &str) -> Result<Vec<TerminalSession>, LanternError> {
    let db = conn.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT id, repo_id, title, shell, sort_order
         FROM terminal_session
         WHERE repo_id = ?1
         ORDER BY sort_order ASC, title ASC",
    )?;

    let rows = stmt.query_map(params![repo_id], |row| {
        Ok(TerminalSession {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            title: row.get(2)?,
            shell: row.get(3)?,
            sort_order: row.get(4)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn create_session(
    conn: &DbConn,
    repo_id: &str,
    title: &str,
    shell: Option<&str>,
) -> Result<TerminalSession, LanternError> {
    let db = conn.lock().unwrap();
    let repo_exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM repo WHERE id = ?1)",
        params![repo_id],
        |row| row.get(0),
    )?;

    if !repo_exists {
        return Err(LanternError::RepoNotFound(repo_id.to_string()));
    }

    let sort_order: i32 = db.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM terminal_session WHERE repo_id = ?1",
        params![repo_id],
        |row| row.get(0),
    )?;
    let session = TerminalSession {
        id: Uuid::new_v4().to_string(),
        repo_id: repo_id.to_string(),
        title: title.to_string(),
        shell: shell.map(str::to_string),
        sort_order,
    };

    db.execute(
        "INSERT INTO terminal_session (id, repo_id, title, shell, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            session.id,
            session.repo_id,
            session.title,
            session.shell,
            session.sort_order,
        ],
    )?;

    Ok(session)
}

pub fn close_session(conn: &DbConn, session_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected_rows = db.execute(
        "DELETE FROM terminal_session WHERE id = ?1",
        params![session_id],
    )?;

    if affected_rows == 0 {
        return Err(LanternError::SessionNotFound(session_id.to_string()));
    }

    Ok(())
}

pub fn rename_session(conn: &DbConn, session_id: &str, title: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected_rows = db.execute(
        "UPDATE terminal_session SET title = ?1 WHERE id = ?2",
        params![title, session_id],
    )?;

    if affected_rows == 0 {
        return Err(LanternError::SessionNotFound(session_id.to_string()));
    }

    Ok(())
}

pub fn reorder_sessions(
    conn: &DbConn,
    repo_id: &str,
    session_ids: &[String],
) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    for (sort_order, session_id) in session_ids.iter().enumerate() {
        db.execute(
            "UPDATE terminal_session SET sort_order = ?1 WHERE id = ?2 AND repo_id = ?3",
            params![sort_order as i32, session_id, repo_id],
        )?;
    }

    Ok(())
}

pub fn get_active_tab(conn: &DbConn, repo_id: &str) -> Result<Option<String>, LanternError> {
    let db = conn.lock().unwrap();
    db.query_row(
        "SELECT session_id FROM active_tab WHERE repo_id = ?1",
        params![repo_id],
        |row| row.get(0),
    )
    .map(Some)
    .or_else(|error| {
        if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
            Ok(None)
        } else {
            Err(error.into())
        }
    })
}

pub fn set_active_tab(conn: &DbConn, repo_id: &str, session_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO active_tab (repo_id, session_id) VALUES (?1, ?2)",
        params![repo_id, session_id],
    )?;
    Ok(())
}

pub fn save_layout(conn: &DbConn, layout: &AppLayout) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let collapsed_group_ids = serde_json::to_string(&layout.collapsed_group_ids)?;
    db.execute(
        "INSERT OR REPLACE INTO app_state (
            id, window_x, window_y, window_width, window_height, window_maximized,
            sidebar_width, sidebar_collapsed, active_repo_id, collapsed_group_ids
         ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            layout.window_x,
            layout.window_y,
            layout.window_width,
            layout.window_height,
            layout.window_maximized as i32,
            layout.sidebar_width,
            layout.sidebar_collapsed as i32,
            layout.active_repo_id,
            collapsed_group_ids,
        ],
    )?;
    Ok(())
}

pub fn load_layout(conn: &DbConn) -> Result<Option<AppLayout>, LanternError> {
    let db = conn.lock().unwrap();
    db.query_row(
        "SELECT window_x, window_y, window_width, window_height, window_maximized,
                sidebar_width, sidebar_collapsed, active_repo_id, collapsed_group_ids
         FROM app_state
         WHERE id = 1",
        [],
        |row| {
            let collapsed_group_ids: String = row.get(8)?;
            let collapsed_group_ids =
                serde_json::from_str(&collapsed_group_ids).unwrap_or_else(|_| Vec::new());

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
    )
    .map(Some)
    .or_else(|error| {
        if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
            Ok(None)
        } else {
            Err(error.into())
        }
    })
}

pub fn load_native_split_state(
    conn: &DbConn,
) -> Result<HashMap<String, NativeSplitState>, LanternError> {
    let db = conn.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT repo_id, visible_session_ids, orientation, divider_position, secondary_divider_position, divider_positions
         FROM native_terminal_split
         ORDER BY repo_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let repo_id: String = row.get(0)?;
        let visible_session_ids: String = row.get(1)?;
        let orientation: String = row.get(2)?;
        let divider_position: Option<i32> = row.get(3)?;
        let secondary_divider_position: Option<i32> = row.get(4)?;
        let divider_positions: String = row.get(5)?;
        Ok((
            repo_id,
            visible_session_ids,
            orientation,
            divider_position,
            secondary_divider_position,
            divider_positions,
        ))
    })?;

    let mut split_state = HashMap::new();
    for row in rows {
        let (
            repo_id,
            visible_session_ids,
            orientation,
            divider_position,
            secondary_divider_position,
            divider_positions,
        ) = row?;
        let mut divider_positions =
            serde_json::from_str::<Vec<i32>>(&divider_positions).unwrap_or_else(|_| Vec::new());
        if divider_positions.is_empty() {
            if let Some(divider_position) = divider_position {
                divider_positions.push(divider_position);
            }
            if let Some(secondary_divider_position) = secondary_divider_position {
                divider_positions.push(secondary_divider_position);
            }
        }
        split_state.insert(
            repo_id,
            NativeSplitState {
                visible_session_ids: serde_json::from_str(&visible_session_ids)
                    .unwrap_or_else(|_| Vec::new()),
                orientation: parse_native_split_orientation(&orientation),
                divider_positions,
            },
        );
    }

    Ok(split_state)
}

pub fn save_native_split_state(
    conn: &DbConn,
    repo_id: &str,
    split_state: &NativeSplitState,
) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO native_terminal_split (
            repo_id,
            visible_session_ids,
            orientation,
            divider_position,
            secondary_divider_position,
            divider_positions
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            repo_id,
            serde_json::to_string(&split_state.visible_session_ids)?,
            native_split_orientation_value(split_state.orientation),
            split_state.divider_positions.first().copied(),
            split_state.divider_positions.get(1).copied(),
            serde_json::to_string(&split_state.divider_positions)?,
        ],
    )?;
    Ok(())
}

pub fn delete_native_split_state(conn: &DbConn, repo_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "DELETE FROM native_terminal_split WHERE repo_id = ?1",
        params![repo_id],
    )?;
    Ok(())
}

fn parse_native_split_orientation(value: &str) -> NativeSplitOrientation {
    match value {
        "vertical" => NativeSplitOrientation::Vertical,
        _ => NativeSplitOrientation::Horizontal,
    }
}

fn native_split_orientation_value(value: NativeSplitOrientation) -> &'static str {
    match value {
        NativeSplitOrientation::Horizontal => "horizontal",
        NativeSplitOrientation::Vertical => "vertical",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn insert_repo(conn: &DbConn, repo_id: &str) {
        let db = conn.lock().unwrap();
        db.execute(
            "INSERT INTO repo (id, name, path, sort_order, group_id, is_default)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                repo_id,
                format!("Repo {repo_id}"),
                format!("/tmp/{repo_id}"),
                0,
                Option::<String>::None,
                0
            ],
        )
        .unwrap();
    }

    fn add_temp_repo(conn: &DbConn, label: &str) -> Repo {
        let dir = tempdir().unwrap();
        let path = dir.keep().join(label);
        std::fs::create_dir_all(&path).unwrap();
        add_repo(conn, path.to_str().unwrap()).unwrap()
    }

    #[test]
    fn layout_roundtrip_preserves_sidebar_state() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        insert_repo(&conn, "repo-1");
        let layout = AppLayout {
            sidebar_width: 320,
            sidebar_collapsed: true,
            active_repo_id: Some("repo-1".to_string()),
            collapsed_group_ids: vec!["group-1".to_string()],
            ..AppLayout::default()
        };

        save_layout(&conn, &layout).unwrap();
        let loaded = load_layout(&conn).unwrap().unwrap();

        assert_eq!(loaded.sidebar_width, 320);
        assert!(loaded.sidebar_collapsed);
        assert_eq!(loaded.active_repo_id.as_deref(), Some("repo-1"));
        assert_eq!(loaded.collapsed_group_ids, vec!["group-1".to_string()]);
    }

    #[test]
    fn create_and_close_session_updates_repo_sessions() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        insert_repo(&conn, "repo-1");

        let session = create_session(&conn, "repo-1", "Terminal 1", Some("/bin/zsh")).unwrap();
        assert_eq!(session.repo_id, "repo-1");
        assert_eq!(session.title, "Terminal 1");
        assert_eq!(session.shell.as_deref(), Some("/bin/zsh"));
        assert_eq!(session.sort_order, 0);

        let sessions = list_sessions(&conn, "repo-1").unwrap();
        assert_eq!(sessions, vec![session.clone()]);

        close_session(&conn, session.id.as_str()).unwrap();

        assert!(list_sessions(&conn, "repo-1").unwrap().is_empty());
    }

    #[test]
    fn add_repo_validates_path_and_prevents_duplicates() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        let repo_path = dir.path().join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        let repo = add_repo(&conn, repo_path.to_str().unwrap()).unwrap();
        assert_eq!(repo.name, "repo");

        let duplicate = add_repo(&conn, repo_path.to_str().unwrap());
        assert!(matches!(duplicate, Err(LanternError::RepoAlreadyExists(_))));

        let missing = add_repo(&conn, dir.path().join("missing").to_str().unwrap());
        assert!(matches!(missing, Err(LanternError::PathNotFound(_))));
    }

    #[test]
    fn remove_repo_cascades_sessions() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        let repo = add_temp_repo(&conn, "repo");
        let session = create_session(&conn, repo.id.as_str(), "Terminal 1", None).unwrap();

        remove_repo(&conn, repo.id.as_str()).unwrap();

        assert!(list_repos(&conn).unwrap().is_empty());
        assert!(list_sessions(&conn, repo.id.as_str()).unwrap().is_empty());
        assert!(matches!(
            close_session(&conn, session.id.as_str()),
            Err(LanternError::SessionNotFound(_))
        ));
    }

    #[test]
    fn rename_session_updates_saved_title() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        let repo = add_temp_repo(&conn, "repo");
        let session = create_session(&conn, repo.id.as_str(), "Terminal 1", None).unwrap();

        rename_session(&conn, session.id.as_str(), "Logs").unwrap();

        let renamed_session = list_sessions(&conn, repo.id.as_str())
            .unwrap()
            .into_iter()
            .find(|saved_session| saved_session.id == session.id)
            .unwrap();
        assert_eq!(renamed_session.title, "Logs");
    }

    #[test]
    fn reorder_sessions_updates_terminal_tab_order() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        let repo = add_temp_repo(&conn, "repo");
        let first = create_session(&conn, repo.id.as_str(), "Terminal 1", None).unwrap();
        let second = create_session(&conn, repo.id.as_str(), "Terminal 2", None).unwrap();
        let third = create_session(&conn, repo.id.as_str(), "Terminal 3", None).unwrap();

        reorder_sessions(
            &conn,
            repo.id.as_str(),
            &[third.id.clone(), first.id.clone(), second.id.clone()],
        )
        .unwrap();

        let ordered_session_ids = list_sessions(&conn, repo.id.as_str())
            .unwrap()
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        assert_eq!(ordered_session_ids, vec![third.id, first.id, second.id]);
    }

    #[test]
    fn grouped_repos_are_listed_together_with_default_first() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        let standalone = add_temp_repo(&conn, "standalone");
        let main = add_repo_grouped(
            &conn,
            tempdir().unwrap().keep().to_str().unwrap(),
            Some("group-1"),
            true,
        )
        .unwrap();
        let sibling = add_repo_grouped(
            &conn,
            tempdir().unwrap().keep().to_str().unwrap(),
            Some("group-1"),
            false,
        )
        .unwrap();

        let repos = list_repos(&conn).unwrap();
        let grouped = repos
            .iter()
            .filter(|repo| repo.group_id.as_deref() == Some("group-1"))
            .collect::<Vec<_>>();

        assert_eq!(repos.len(), 3);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].id, main.id);
        assert_eq!(grouped[1].id, sibling.id);
        assert!(repos.iter().any(|repo| repo.id == standalone.id));
    }

    #[test]
    fn find_group_id_and_set_repo_group_roundtrip() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        let grouped_path = tempdir().unwrap().keep();
        let grouped_repo =
            add_repo_grouped(&conn, grouped_path.to_str().unwrap(), Some("group-1"), true).unwrap();
        let standalone_repo = add_temp_repo(&conn, "standalone");

        let found_group_id = find_group_id_by_paths(&conn, &[grouped_repo.path.clone()]).unwrap();
        assert_eq!(found_group_id.as_deref(), Some("group-1"));

        set_repo_group(&conn, standalone_repo.id.as_str(), "group-2", false).unwrap();

        let updated_repo = list_repos(&conn)
            .unwrap()
            .into_iter()
            .find(|repo| repo.id == standalone_repo.id)
            .unwrap();
        assert_eq!(updated_repo.group_id.as_deref(), Some("group-2"));
        assert!(!updated_repo.is_default);
    }

    #[test]
    fn native_split_state_roundtrips_and_deletes() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();
        insert_repo(&conn, "repo-1");

        save_native_split_state(
            &conn,
            "repo-1",
            &NativeSplitState {
                visible_session_ids: vec![
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string(),
                ],
                orientation: NativeSplitOrientation::Vertical,
                divider_positions: vec![420, 260],
            },
        )
        .unwrap();

        let split_state = load_native_split_state(&conn).unwrap();
        assert_eq!(
            split_state.get("repo-1"),
            Some(&NativeSplitState {
                visible_session_ids: vec![
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string(),
                ],
                orientation: NativeSplitOrientation::Vertical,
                divider_positions: vec![420, 260],
            })
        );

        delete_native_split_state(&conn, "repo-1").unwrap();

        assert!(load_native_split_state(&conn)
            .unwrap()
            .get("repo-1")
            .is_none());
    }

    #[test]
    fn native_split_state_migrates_legacy_divider_columns() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lantern.db");
        let legacy_conn = Connection::open(&path).unwrap();
        legacy_conn
            .execute_batch(
                "
                PRAGMA foreign_keys=ON;

                CREATE TABLE repo (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    path TEXT NOT NULL UNIQUE,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    group_id TEXT,
                    is_default INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE native_terminal_split (
                    repo_id TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
                    visible_session_ids TEXT NOT NULL DEFAULT '[]',
                    orientation TEXT NOT NULL DEFAULT 'horizontal',
                    divider_position INTEGER,
                    secondary_divider_position INTEGER
                );

                CREATE TABLE schema_version (
                    version INTEGER PRIMARY KEY
                );
                ",
            )
            .unwrap();
        legacy_conn
            .execute(
                "INSERT INTO repo (id, name, path, sort_order, group_id, is_default)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "repo-1",
                    "repo-1",
                    "/tmp/repo-1",
                    0,
                    Option::<String>::None,
                    0
                ],
            )
            .unwrap();
        legacy_conn
            .execute(
                "INSERT INTO native_terminal_split (
                    repo_id,
                    visible_session_ids,
                    orientation,
                    divider_position,
                    secondary_divider_position
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "repo-1",
                    serde_json::to_string(&vec!["tab-1", "tab-2", "tab-3"]).unwrap(),
                    "vertical",
                    420,
                    260,
                ],
            )
            .unwrap();
        legacy_conn
            .execute("INSERT INTO schema_version (version) VALUES (4)", [])
            .unwrap();
        drop(legacy_conn);

        let conn = init_db(Some(path)).unwrap();
        let split_state = load_native_split_state(&conn).unwrap();
        assert_eq!(
            split_state.get("repo-1"),
            Some(&NativeSplitState {
                visible_session_ids: vec![
                    "tab-1".to_string(),
                    "tab-2".to_string(),
                    "tab-3".to_string(),
                ],
                orientation: NativeSplitOrientation::Vertical,
                divider_positions: vec![420, 260],
            })
        );
    }
}
