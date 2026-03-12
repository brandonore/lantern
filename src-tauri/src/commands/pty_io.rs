use crate::db;
use crate::error::LanternError;
use crate::git;
use crate::pty::TerminalOutput;
use crate::state::AppState;
#[cfg(debug_assertions)]
use std::time::Instant;
use tauri::{
    http::HeaderMap,
    ipc::{Channel, InvokeBody, Request},
    State,
};

const TERMINAL_SESSION_HEADER: &str = "x-lantern-session-id";
const TERMINAL_INPUT_SEQ_HEADER: &str = "x-lantern-input-seq";

fn parse_required_header(headers: &HeaderMap, name: &str) -> Result<String, LanternError> {
    let value = headers
        .get(name)
        .ok_or_else(|| LanternError::InvalidInput(format!("missing {name} header")))?;

    value
        .to_str()
        .map(|value| value.to_string())
        .map_err(|_| LanternError::InvalidInput(format!("invalid {name} header")))
}

fn parse_optional_u64_header(headers: &HeaderMap, name: &str) -> Result<Option<u64>, LanternError> {
    headers
        .get(name)
        .map(|value| {
            value
                .to_str()
                .map_err(|_| LanternError::InvalidInput(format!("invalid {name} header")))
                .and_then(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| LanternError::InvalidInput(format!("invalid {name} header")))
                })
        })
        .transpose()
}

fn parse_raw_write_request<'a>(
    headers: &HeaderMap,
    body: &'a InvokeBody,
) -> Result<(String, Option<u64>, &'a [u8]), LanternError> {
    let session_id = parse_required_header(headers, TERMINAL_SESSION_HEADER)?;
    let seq = parse_optional_u64_header(headers, TERMINAL_INPUT_SEQ_HEADER)?;

    match body {
        InvokeBody::Raw(bytes) => Ok((session_id, seq, bytes.as_slice())),
        InvokeBody::Json(_) => Err(LanternError::InvalidInput(
            "terminal_write_raw requires a raw bytes payload".into(),
        )),
    }
}

#[tauri::command]
pub fn terminal_write(
    session_id: String,
    data: Vec<u8>,
    state: State<AppState>,
) -> Result<(), LanternError> {
    state.pty_manager.write(&session_id, &data)
}

#[tauri::command]
pub fn terminal_write_raw(
    request: Request<'_>,
    state: State<AppState>,
) -> Result<(), LanternError> {
    #[cfg(debug_assertions)]
    let command_started = Instant::now();
    let (session_id, seq, data) = parse_raw_write_request(request.headers(), request.body())?;
    #[cfg(debug_assertions)]
    let parse_elapsed = command_started.elapsed();
    #[cfg(debug_assertions)]
    let write_started = Instant::now();

    state.pty_manager.write(&session_id, data)?;

    #[cfg(debug_assertions)]
    if let Some(seq) = seq {
        let write_elapsed = write_started.elapsed();
        let total_elapsed = command_started.elapsed();
        eprintln!(
            "[lantern][pty-write] session={} seq={} bytes={} parse_ms={:.2} write_ms={:.2} total_ms={:.2}",
            session_id,
            seq,
            data.len(),
            parse_elapsed.as_secs_f64() * 1000.0,
            write_elapsed.as_secs_f64() * 1000.0,
            total_elapsed.as_secs_f64() * 1000.0,
        );
    }

    Ok(())
}

#[tauri::command]
pub fn terminal_resize(
    session_id: String,
    cols: u16,
    rows: u16,
    state: State<AppState>,
) -> Result<(), LanternError> {
    state.pty_manager.resize(&session_id, cols, rows)
}

#[tauri::command]
pub fn terminal_subscribe(
    session_id: String,
    channel: Channel<TerminalOutput>,
    state: State<AppState>,
) -> Result<(), LanternError> {
    // Get the session info to find shell and cwd
    let repos = db::list_repos(&state.db)?;
    let sessions_all: Vec<db::TerminalSession> = repos
        .iter()
        .flat_map(|r| db::list_sessions(&state.db, &r.id).unwrap_or_default())
        .collect();

    let session = sessions_all
        .iter()
        .find(|s| s.id == session_id)
        .ok_or_else(|| LanternError::SessionNotFound(session_id.clone()))?;

    let repo = repos
        .iter()
        .find(|r| r.id == session.repo_id)
        .ok_or_else(|| LanternError::RepoNotFound(session.repo_id.clone()))?;

    let config = state.config.lock().unwrap();
    let shell = session
        .shell
        .clone()
        .unwrap_or_else(|| config.default_shell.clone());
    drop(config);

    // Only spawn if not already running
    if !state.pty_manager.session_exists(&session_id) {
        state.pty_manager.spawn(
            &session_id,
            &shell,
            &repo.path,
            80,
            24,
            Box::new(move |output| {
                let _ = channel.send(output);
            }),
        )?;
    }

    Ok(())
}

#[tauri::command]
pub fn terminal_get_foreground_process(
    session_id: String,
    state: State<AppState>,
) -> Result<Option<git::ProcessInfo>, LanternError> {
    match state.pty_manager.get_pid(&session_id) {
        Some(pid) => Ok(git::get_foreground_process(pid)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::http::{
        header::{HeaderName, HeaderValue},
        HeaderMap,
    };

    #[test]
    fn parse_raw_write_request_reads_headers_and_body() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(TERMINAL_SESSION_HEADER),
            HeaderValue::from_static("session-1"),
        );
        headers.insert(
            HeaderName::from_static(TERMINAL_INPUT_SEQ_HEADER),
            HeaderValue::from_static("42"),
        );

        let body = InvokeBody::Raw(vec![97, 98, 99]);
        let (session_id, seq, bytes) = parse_raw_write_request(&headers, &body).unwrap();

        assert_eq!(session_id, "session-1");
        assert_eq!(seq, Some(42));
        assert_eq!(bytes, &[97, 98, 99]);
    }

    #[test]
    fn parse_raw_write_request_rejects_missing_session_header() {
        let headers = HeaderMap::new();
        let body = InvokeBody::Raw(vec![97]);

        let error = parse_raw_write_request(&headers, &body).unwrap_err();
        assert!(matches!(error, LanternError::InvalidInput(_)));
        assert!(error.to_string().contains("missing x-lantern-session-id"));
    }

    #[test]
    fn parse_raw_write_request_rejects_json_payload() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(TERMINAL_SESSION_HEADER),
            HeaderValue::from_static("session-1"),
        );

        let body = InvokeBody::Json(serde_json::json!({ "data": [97] }));
        let error = parse_raw_write_request(&headers, &body).unwrap_err();

        assert!(matches!(error, LanternError::InvalidInput(_)));
        assert!(error.to_string().contains("raw bytes payload"));
    }

    #[test]
    fn parse_raw_write_request_rejects_invalid_seq_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(TERMINAL_SESSION_HEADER),
            HeaderValue::from_static("session-1"),
        );
        headers.insert(
            HeaderName::from_static(TERMINAL_INPUT_SEQ_HEADER),
            HeaderValue::from_static("invalid"),
        );

        let body = InvokeBody::Raw(vec![97]);
        let error = parse_raw_write_request(&headers, &body).unwrap_err();

        assert!(matches!(error, LanternError::InvalidInput(_)));
        assert!(error.to_string().contains("invalid x-lantern-input-seq"));
    }
}
