use crate::error::LanternError;
use crate::models::{
    AppLayout, NativeSplitOrientation, NativeSplitState, Repo, TerminalSession, TerminalTab,
};
use crate::paths;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub type DbConn = Arc<Mutex<Connection>>;
const CURRENT_SCHEMA_VERSION: i32 = 7;

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

        CREATE TABLE IF NOT EXISTS terminal_tab (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL REFERENCES repo(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            active_session_id TEXT REFERENCES terminal_session(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS terminal_session (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL REFERENCES repo(id) ON DELETE CASCADE,
            tab_id TEXT NOT NULL REFERENCES terminal_tab(id) ON DELETE CASCADE,
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
            tab_id TEXT NOT NULL REFERENCES terminal_tab(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS native_terminal_split (
            tab_id TEXT PRIMARY KEY REFERENCES terminal_tab(id) ON DELETE CASCADE,
            visible_session_ids TEXT NOT NULL DEFAULT '[]',
            orientation TEXT NOT NULL DEFAULT 'horizontal',
            divider_position INTEGER,
            secondary_divider_position INTEGER,
            divider_positions TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS hidden_repo_path (
            path TEXT PRIMARY KEY
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

    let inferred_version = if table_has_column(conn, "terminal_session", "tab_id")?
        && table_has_column(conn, "active_tab", "tab_id")?
    {
        CURRENT_SCHEMA_VERSION
    } else if table_has_column(conn, "native_terminal_split", "divider_positions")? {
        6
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

    if version < 7
        || !table_has_column(conn, "terminal_session", "tab_id")?
        || !table_has_column(conn, "active_tab", "tab_id")?
    {
        migrate_tabs_and_split_state(conn)?;
    }

    conn.execute(
        "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
        params![CURRENT_SCHEMA_VERSION],
    )?;

    Ok(())
}

#[derive(Clone)]
struct LegacySessionRecord {
    id: String,
    repo_id: String,
    title: String,
    shell: Option<String>,
    sort_order: i32,
}

#[derive(Clone)]
struct LegacySplitRecord {
    visible_session_ids: Vec<String>,
    orientation: NativeSplitOrientation,
    divider_positions: Vec<i32>,
}

fn migrate_tabs_and_split_state(conn: &Connection) -> Result<(), LanternError> {
    let repo_ids = load_legacy_repo_ids(conn)?;
    let sessions = load_legacy_sessions(conn)?;
    let active_tab_by_repo_id = load_legacy_active_tab_map(conn)?;
    let split_state_by_repo_id = load_legacy_split_map(conn)?;

    conn.execute_batch(
        "
        PRAGMA foreign_keys=OFF;
        BEGIN IMMEDIATE;

        DELETE FROM terminal_tab;

        CREATE TABLE terminal_session_v2 (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL REFERENCES repo(id) ON DELETE CASCADE,
            tab_id TEXT NOT NULL REFERENCES terminal_tab(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            shell TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE active_tab_v2 (
            repo_id TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
            tab_id TEXT NOT NULL REFERENCES terminal_tab(id) ON DELETE CASCADE
        );

        CREATE TABLE native_terminal_split_v2 (
            tab_id TEXT PRIMARY KEY REFERENCES terminal_tab(id) ON DELETE CASCADE,
            visible_session_ids TEXT NOT NULL DEFAULT '[]',
            orientation TEXT NOT NULL DEFAULT 'horizontal',
            divider_position INTEGER,
            secondary_divider_position INTEGER,
            divider_positions TEXT NOT NULL DEFAULT '[]'
        );
        ",
    )?;

    for repo_id in repo_ids {
        migrate_repo_tabs(
            conn,
            repo_id.as_str(),
            sessions
                .get(repo_id.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            active_tab_by_repo_id.get(repo_id.as_str()).map(String::as_str),
            split_state_by_repo_id.get(repo_id.as_str()),
        )?;
    }

    conn.execute_batch(
        "
        DROP TABLE active_tab;
        ALTER TABLE active_tab_v2 RENAME TO active_tab;

        DROP TABLE native_terminal_split;
        ALTER TABLE native_terminal_split_v2 RENAME TO native_terminal_split;

        DROP TABLE terminal_session;
        ALTER TABLE terminal_session_v2 RENAME TO terminal_session;

        COMMIT;
        PRAGMA foreign_keys=ON;
        ",
    )?;

    Ok(())
}

fn load_legacy_repo_ids(conn: &Connection) -> Result<Vec<String>, LanternError> {
    let mut stmt = conn.prepare("SELECT id FROM repo ORDER BY sort_order ASC, name ASC")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn load_legacy_sessions(
    conn: &Connection,
) -> Result<HashMap<String, Vec<LegacySessionRecord>>, LanternError> {
    let mut stmt = conn.prepare(
        "SELECT id, repo_id, title, shell, sort_order
         FROM terminal_session
         ORDER BY repo_id ASC, sort_order ASC, title ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(LegacySessionRecord {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            title: row.get(2)?,
            shell: row.get(3)?,
            sort_order: row.get(4)?,
        })
    })?;

    let mut sessions_by_repo_id = HashMap::new();
    for row in rows {
        let session = row?;
        sessions_by_repo_id
            .entry(session.repo_id.clone())
            .or_insert_with(Vec::new)
            .push(session);
    }
    Ok(sessions_by_repo_id)
}

fn load_legacy_active_tab_map(conn: &Connection) -> Result<HashMap<String, String>, LanternError> {
    let active_column = if table_has_column(conn, "active_tab", "tab_id")? {
        "tab_id"
    } else {
        "session_id"
    };
    let query = format!("SELECT repo_id, {active_column} FROM active_tab ORDER BY repo_id ASC");
    let mut stmt = conn.prepare(query.as_str())?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    let mut values = HashMap::new();
    for row in rows {
        let (repo_id, active_id) = row?;
        values.insert(repo_id, active_id);
    }
    Ok(values)
}

fn load_legacy_split_map(conn: &Connection) -> Result<HashMap<String, LegacySplitRecord>, LanternError> {
    if !table_has_column(conn, "native_terminal_split", "visible_session_ids")? {
        return Ok(HashMap::new());
    }

    let key_column = if table_has_column(conn, "native_terminal_split", "repo_id")? {
        "repo_id"
    } else {
        "tab_id"
    };
    let query = format!(
        "SELECT {key_column}, visible_session_ids, orientation, divider_position, secondary_divider_position, divider_positions
         FROM native_terminal_split
         ORDER BY {key_column} ASC"
    );
    let mut stmt = conn.prepare(query.as_str())?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<i32>>(3)?,
            row.get::<_, Option<i32>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let mut split_state = HashMap::new();
    for row in rows {
        let (
            key,
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
            key,
            LegacySplitRecord {
                visible_session_ids: serde_json::from_str(&visible_session_ids)
                    .unwrap_or_else(|_| Vec::new()),
                orientation: parse_native_split_orientation(&orientation),
                divider_positions,
            },
        );
    }
    Ok(split_state)
}

fn migrate_repo_tabs(
    conn: &Connection,
    repo_id: &str,
    sessions: &[LegacySessionRecord],
    persisted_active_session_id: Option<&str>,
    legacy_split: Option<&LegacySplitRecord>,
) -> Result<(), LanternError> {
    if sessions.is_empty() {
        return Ok(());
    }

    let session_by_id = sessions
        .iter()
        .map(|session| (session.id.as_str(), session))
        .collect::<HashMap<_, _>>();
    let split_session_ids = legacy_split
        .map(|split| {
            split
                .visible_session_ids
                .iter()
                .filter(|session_id| session_by_id.contains_key(session_id.as_str()))
                .fold(Vec::new(), |mut ids, session_id| {
                    if !ids.iter().any(|existing| existing == session_id) {
                        ids.push(session_id.clone());
                    }
                    ids
                })
        })
        .unwrap_or_default();

    let mut migrated_tabs = Vec::new();

    if !split_session_ids.is_empty() {
        let split_active_session_id = persisted_active_session_id
            .filter(|session_id| split_session_ids.iter().any(|id| id == *session_id))
            .map(str::to_string)
            .or_else(|| split_session_ids.first().cloned());
        let split_sort_order = split_session_ids
            .iter()
            .filter_map(|session_id| session_by_id.get(session_id.as_str()).map(|session| session.sort_order))
            .min()
            .unwrap_or(0);
        let split_title = split_active_session_id
            .as_deref()
            .and_then(|session_id| session_by_id.get(session_id).map(|session| session.title.clone()))
            .unwrap_or_else(|| "Terminal".to_string());
        let tab_id = Uuid::new_v4().to_string();

        insert_terminal_tab(
            conn,
            &TerminalTab {
                id: tab_id.clone(),
                repo_id: repo_id.to_string(),
                title: split_title,
                sort_order: split_sort_order,
                active_session_id: split_active_session_id.clone(),
            },
        )?;

        for (sort_order, session_id) in split_session_ids.iter().enumerate() {
            let session = session_by_id[session_id.as_str()];
            insert_terminal_session(conn, session, tab_id.as_str(), sort_order as i32)?;
        }

        if let Some(legacy_split) = legacy_split {
            insert_native_split_state(
                conn,
                tab_id.as_str(),
                &NativeSplitState {
                    visible_session_ids: split_session_ids.clone(),
                    orientation: legacy_split.orientation,
                    divider_positions: legacy_split.divider_positions.clone(),
                },
            )?;
        }

        migrated_tabs.push((tab_id, split_sort_order));
    }

    for session in sessions {
        if split_session_ids.iter().any(|session_id| session_id == &session.id) {
            continue;
        }

        let tab_id = Uuid::new_v4().to_string();
        insert_terminal_tab(
            conn,
            &TerminalTab {
                id: tab_id.clone(),
                repo_id: repo_id.to_string(),
                title: session.title.clone(),
                sort_order: session.sort_order,
                active_session_id: Some(session.id.clone()),
            },
        )?;
        insert_terminal_session(conn, session, tab_id.as_str(), 0)?;
        migrated_tabs.push((tab_id, session.sort_order));
    }

    migrated_tabs.sort_by(|left, right| left.1.cmp(&right.1));

    let active_tab_id = persisted_active_session_id
        .and_then(|active_session_id| {
            conn.query_row(
                "SELECT tab_id FROM terminal_session_v2 WHERE id = ?1",
                params![active_session_id],
                |row| row.get::<_, String>(0),
            )
            .ok()
        })
        .or_else(|| migrated_tabs.first().map(|(tab_id, _)| tab_id.clone()));

    if let Some(active_tab_id) = active_tab_id {
        conn.execute(
            "INSERT INTO active_tab_v2 (repo_id, tab_id) VALUES (?1, ?2)",
            params![repo_id, active_tab_id],
        )?;
    }

    Ok(())
}

fn insert_terminal_tab(conn: &Connection, tab: &TerminalTab) -> Result<(), LanternError> {
    conn.execute(
        "INSERT INTO terminal_tab (id, repo_id, title, sort_order, active_session_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            tab.id,
            tab.repo_id,
            tab.title,
            tab.sort_order,
            tab.active_session_id,
        ],
    )?;
    Ok(())
}

fn insert_terminal_session(
    conn: &Connection,
    session: &LegacySessionRecord,
    tab_id: &str,
    sort_order: i32,
) -> Result<(), LanternError> {
    conn.execute(
        "INSERT INTO terminal_session_v2 (id, repo_id, tab_id, title, shell, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            session.id,
            session.repo_id,
            tab_id,
            session.title,
            session.shell,
            sort_order,
        ],
    )?;
    Ok(())
}

fn insert_native_split_state(
    conn: &Connection,
    tab_id: &str,
    split_state: &NativeSplitState,
) -> Result<(), LanternError> {
    conn.execute(
        "INSERT INTO native_terminal_split_v2 (
            tab_id,
            visible_session_ids,
            orientation,
            divider_position,
            secondary_divider_position,
            divider_positions
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            tab_id,
            serde_json::to_string(&split_state.visible_session_ids)?,
            native_split_orientation_value(split_state.orientation),
            split_state.divider_positions.first().copied(),
            split_state.divider_positions.get(1).copied(),
            serde_json::to_string(&split_state.divider_positions)?,
        ],
    )?;
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

pub fn find_repo_id_by_path(conn: &DbConn, path: &str) -> Result<Option<String>, LanternError> {
    let db = conn.lock().unwrap();
    match db.query_row(
        "SELECT id FROM repo WHERE path = ?1",
        params![path],
        |row| row.get(0),
    ) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
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

pub fn list_tabs(conn: &DbConn, repo_id: &str) -> Result<Vec<TerminalTab>, LanternError> {
    let db = conn.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT id, repo_id, title, sort_order, active_session_id
         FROM terminal_tab
         WHERE repo_id = ?1
         ORDER BY sort_order ASC, title ASC",
    )?;

    let rows = stmt.query_map(params![repo_id], |row| {
        Ok(TerminalTab {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            title: row.get(2)?,
            sort_order: row.get(3)?,
            active_session_id: row.get(4)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn create_tab(conn: &DbConn, repo_id: &str, title: &str) -> Result<TerminalTab, LanternError> {
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
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM terminal_tab WHERE repo_id = ?1",
        params![repo_id],
        |row| row.get(0),
    )?;
    let tab = TerminalTab {
        id: Uuid::new_v4().to_string(),
        repo_id: repo_id.to_string(),
        title: title.to_string(),
        sort_order,
        active_session_id: None,
    };

    db.execute(
        "INSERT INTO terminal_tab (id, repo_id, title, sort_order, active_session_id)
         VALUES (?1, ?2, ?3, ?4, NULL)",
        params![tab.id, tab.repo_id, tab.title, tab.sort_order],
    )?;

    Ok(tab)
}

pub fn close_tab(conn: &DbConn, tab_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected_rows = db.execute("DELETE FROM terminal_tab WHERE id = ?1", params![tab_id])?;

    if affected_rows == 0 {
        return Err(LanternError::TabNotFound(tab_id.to_string()));
    }

    Ok(())
}

pub fn rename_tab(conn: &DbConn, tab_id: &str, title: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected_rows = db.execute(
        "UPDATE terminal_tab SET title = ?1 WHERE id = ?2",
        params![title, tab_id],
    )?;

    if affected_rows == 0 {
        return Err(LanternError::TabNotFound(tab_id.to_string()));
    }

    Ok(())
}

pub fn reorder_tabs(conn: &DbConn, repo_id: &str, tab_ids: &[String]) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    for (sort_order, tab_id) in tab_ids.iter().enumerate() {
        db.execute(
            "UPDATE terminal_tab SET sort_order = ?1 WHERE id = ?2 AND repo_id = ?3",
            params![sort_order as i32, tab_id, repo_id],
        )?;
    }

    Ok(())
}

pub fn list_sessions(conn: &DbConn, tab_id: &str) -> Result<Vec<TerminalSession>, LanternError> {
    let db = conn.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT id, repo_id, tab_id, title, shell, sort_order
         FROM terminal_session
         WHERE tab_id = ?1
         ORDER BY sort_order ASC, title ASC",
    )?;

    let rows = stmt.query_map(params![tab_id], |row| {
        Ok(TerminalSession {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            tab_id: row.get(2)?,
            title: row.get(3)?,
            shell: row.get(4)?,
            sort_order: row.get(5)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn create_session(
    conn: &DbConn,
    tab_id: &str,
    title: &str,
    shell: Option<&str>,
) -> Result<TerminalSession, LanternError> {
    let db = conn.lock().unwrap();
    let repo_id: String = db
        .query_row(
            "SELECT repo_id FROM terminal_tab WHERE id = ?1",
            params![tab_id],
            |row| row.get(0),
        )
        .map_err(|error| {
            if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                LanternError::TabNotFound(tab_id.to_string())
            } else {
                LanternError::from(error)
            }
        })?;

    let tab_exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM terminal_tab WHERE id = ?1)",
        params![tab_id],
        |row| row.get(0),
    )?;

    if !tab_exists {
        return Err(LanternError::TabNotFound(tab_id.to_string()));
    }

    let sort_order: i32 = db.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM terminal_session WHERE tab_id = ?1",
        params![tab_id],
        |row| row.get(0),
    )?;
    let session = TerminalSession {
        id: Uuid::new_v4().to_string(),
        repo_id,
        tab_id: tab_id.to_string(),
        title: title.to_string(),
        shell: shell.map(str::to_string),
        sort_order,
    };

    db.execute(
        "INSERT INTO terminal_session (id, repo_id, tab_id, title, shell, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            session.id,
            session.repo_id,
            session.tab_id,
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
    tab_id: &str,
    session_ids: &[String],
) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    for (sort_order, session_id) in session_ids.iter().enumerate() {
        db.execute(
            "UPDATE terminal_session SET sort_order = ?1 WHERE id = ?2 AND tab_id = ?3",
            params![sort_order as i32, session_id, tab_id],
        )?;
    }

    Ok(())
}

pub fn get_active_tab(conn: &DbConn, repo_id: &str) -> Result<Option<String>, LanternError> {
    let db = conn.lock().unwrap();
    db.query_row(
        "SELECT tab_id FROM active_tab WHERE repo_id = ?1",
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

pub fn set_active_tab(conn: &DbConn, repo_id: &str, tab_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO active_tab (repo_id, tab_id) VALUES (?1, ?2)",
        params![repo_id, tab_id],
    )?;
    Ok(())
}

pub fn get_active_session(conn: &DbConn, tab_id: &str) -> Result<Option<String>, LanternError> {
    let db = conn.lock().unwrap();
    db.query_row(
        "SELECT active_session_id FROM terminal_tab WHERE id = ?1",
        params![tab_id],
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

pub fn set_active_session(conn: &DbConn, tab_id: &str, session_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    let affected_rows = db.execute(
        "UPDATE terminal_tab SET active_session_id = ?1 WHERE id = ?2",
        params![session_id, tab_id],
    )?;

    if affected_rows == 0 {
        return Err(LanternError::TabNotFound(tab_id.to_string()));
    }

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
        "SELECT tab_id, visible_session_ids, orientation, divider_position, secondary_divider_position, divider_positions
         FROM native_terminal_split
         ORDER BY tab_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let tab_id: String = row.get(0)?;
        let visible_session_ids: String = row.get(1)?;
        let orientation: String = row.get(2)?;
        let divider_position: Option<i32> = row.get(3)?;
        let secondary_divider_position: Option<i32> = row.get(4)?;
        let divider_positions: String = row.get(5)?;
        Ok((
            tab_id,
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
            tab_id,
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
            tab_id,
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
    tab_id: &str,
    split_state: &NativeSplitState,
) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO native_terminal_split (
            tab_id,
            visible_session_ids,
            orientation,
            divider_position,
            secondary_divider_position,
            divider_positions
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            tab_id,
            serde_json::to_string(&split_state.visible_session_ids)?,
            native_split_orientation_value(split_state.orientation),
            split_state.divider_positions.first().copied(),
            split_state.divider_positions.get(1).copied(),
            serde_json::to_string(&split_state.divider_positions)?,
        ],
    )?;
    Ok(())
}

pub fn delete_native_split_state(conn: &DbConn, tab_id: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "DELETE FROM native_terminal_split WHERE tab_id = ?1",
        params![tab_id],
    )?;
    Ok(())
}

pub fn hide_repo_path(conn: &DbConn, path: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "INSERT OR IGNORE INTO hidden_repo_path (path) VALUES (?1)",
        params![path],
    )?;
    Ok(())
}

pub fn unhide_repo_path(conn: &DbConn, path: &str) -> Result<(), LanternError> {
    let db = conn.lock().unwrap();
    db.execute(
        "DELETE FROM hidden_repo_path WHERE path = ?1",
        params![path],
    )?;
    Ok(())
}

pub fn list_hidden_paths(conn: &DbConn) -> Result<HashSet<String>, LanternError> {
    let db = conn.lock().unwrap();
    let mut stmt = db.prepare("SELECT path FROM hidden_repo_path")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(Into::into)
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

    fn insert_legacy_session(
        conn: &Connection,
        session_id: &str,
        repo_id: &str,
        title: &str,
        sort_order: i32,
    ) {
        conn.execute(
            "INSERT INTO terminal_session (id, repo_id, title, shell, sort_order)
             VALUES (?1, ?2, ?3, NULL, ?4)",
            params![session_id, repo_id, title, sort_order],
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
        let tab = create_tab(&conn, "repo-1", "Terminal 1").unwrap();

        let session =
            create_session(&conn, tab.id.as_str(), "Terminal 1", Some("/bin/zsh")).unwrap();
        assert_eq!(session.repo_id, "repo-1");
        assert_eq!(session.tab_id, tab.id);
        assert_eq!(session.title, "Terminal 1");
        assert_eq!(session.shell.as_deref(), Some("/bin/zsh"));
        assert_eq!(session.sort_order, 0);

        let sessions = list_sessions(&conn, tab.id.as_str()).unwrap();
        assert_eq!(sessions, vec![session.clone()]);

        close_session(&conn, session.id.as_str()).unwrap();

        assert!(list_sessions(&conn, tab.id.as_str()).unwrap().is_empty());
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
        let tab = create_tab(&conn, repo.id.as_str(), "Terminal 1").unwrap();
        let session = create_session(&conn, tab.id.as_str(), "Terminal 1", None).unwrap();

        remove_repo(&conn, repo.id.as_str()).unwrap();

        assert!(list_repos(&conn).unwrap().is_empty());
        assert!(list_tabs(&conn, repo.id.as_str()).unwrap().is_empty());
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
        let tab = create_tab(&conn, repo.id.as_str(), "Terminal 1").unwrap();
        let session = create_session(&conn, tab.id.as_str(), "Terminal 1", None).unwrap();

        rename_session(&conn, session.id.as_str(), "Logs").unwrap();

        let renamed_session = list_sessions(&conn, tab.id.as_str())
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
        let tab = create_tab(&conn, repo.id.as_str(), "Terminal 1").unwrap();
        let first = create_session(&conn, tab.id.as_str(), "Terminal 1", None).unwrap();
        let second = create_session(&conn, tab.id.as_str(), "Terminal 2", None).unwrap();
        let third = create_session(&conn, tab.id.as_str(), "Terminal 3", None).unwrap();

        reorder_sessions(
            &conn,
            tab.id.as_str(),
            &[third.id.clone(), first.id.clone(), second.id.clone()],
        )
        .unwrap();

        let ordered_session_ids = list_sessions(&conn, tab.id.as_str())
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
        let tab = create_tab(&conn, "repo-1", "Terminal 1").unwrap();

        save_native_split_state(
            &conn,
            tab.id.as_str(),
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
            split_state.get(tab.id.as_str()),
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

        delete_native_split_state(&conn, tab.id.as_str()).unwrap();

        assert!(load_native_split_state(&conn)
            .unwrap()
            .get(tab.id.as_str())
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

                CREATE TABLE terminal_session (
                    id TEXT PRIMARY KEY,
                    repo_id TEXT NOT NULL REFERENCES repo(id) ON DELETE CASCADE,
                    title TEXT NOT NULL,
                    shell TEXT,
                    sort_order INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE active_tab (
                    repo_id TEXT PRIMARY KEY REFERENCES repo(id) ON DELETE CASCADE,
                    session_id TEXT NOT NULL REFERENCES terminal_session(id) ON DELETE CASCADE
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
        insert_legacy_session(&legacy_conn, "tab-1", "repo-1", "Terminal 1", 0);
        insert_legacy_session(&legacy_conn, "tab-2", "repo-1", "Terminal 2", 1);
        insert_legacy_session(&legacy_conn, "tab-3", "repo-1", "Terminal 3", 2);
        legacy_conn
            .execute(
                "INSERT INTO active_tab (repo_id, session_id) VALUES (?1, ?2)",
                params!["repo-1", "tab-1"],
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
        assert_eq!(split_state.len(), 1);
        assert_eq!(
            split_state.values().next(),
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

    #[test]
    fn hidden_repo_path_hides_and_unhides() {
        let dir = tempdir().unwrap();
        let conn = init_db(Some(dir.path().join("lantern.db"))).unwrap();

        hide_repo_path(&conn, "/tmp/hidden-repo").unwrap();
        hide_repo_path(&conn, "/tmp/another-hidden").unwrap();

        let hidden = list_hidden_paths(&conn).unwrap();
        assert!(hidden.contains("/tmp/hidden-repo"));
        assert!(hidden.contains("/tmp/another-hidden"));
        assert_eq!(hidden.len(), 2);

        // Duplicate insert is ignored
        hide_repo_path(&conn, "/tmp/hidden-repo").unwrap();
        assert_eq!(list_hidden_paths(&conn).unwrap().len(), 2);

        unhide_repo_path(&conn, "/tmp/hidden-repo").unwrap();
        let hidden = list_hidden_paths(&conn).unwrap();
        assert!(!hidden.contains("/tmp/hidden-repo"));
        assert!(hidden.contains("/tmp/another-hidden"));
        assert_eq!(hidden.len(), 1);
    }
}
