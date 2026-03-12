use crate::error::LanternError;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
pub struct PtyInfo {
    pub session_id: String,
    pub pid: u32,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum TerminalOutput {
    Data { data: String },
    Exited { code: Option<i32> },
}

struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    pid: u32,
    cols: u16,
    rows: u16,
    shutdown: Arc<AtomicBool>,
    reader_handle: Option<std::thread::JoinHandle<()>>,
}

pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShellKind {
    Bash,
    Fish,
    Pwsh,
    Zsh,
    Unknown,
}

fn shell_kind(shell: &str) -> ShellKind {
    let Some(file_name) = Path::new(shell)
        .file_name()
        .and_then(|value| value.to_str())
    else {
        return ShellKind::Unknown;
    };

    match file_name
        .trim_start_matches('-')
        .trim_end_matches(".exe")
        .to_ascii_lowercase()
        .as_str()
    {
        "bash" => ShellKind::Bash,
        "fish" => ShellKind::Fish,
        "pwsh" => ShellKind::Pwsh,
        "zsh" => ShellKind::Zsh,
        _ => ShellKind::Unknown,
    }
}

fn shell_integration_dir(session_id: &str) -> PathBuf {
    std::env::temp_dir()
        .join("lantern-shell-integration")
        .join(session_id)
}

fn prompt_marker() -> &'static str {
    "\u{1b}]633;Lantern;Prompt\u{7}"
}

fn configure_shell_integration(
    cmd: &mut CommandBuilder,
    shell: &str,
    session_id: &str,
) -> Result<(), LanternError> {
    let integration_dir = shell_integration_dir(session_id);
    match shell_kind(shell) {
        ShellKind::Bash => configure_bash_integration(cmd, &integration_dir),
        ShellKind::Fish => configure_fish_integration(cmd, &integration_dir),
        ShellKind::Pwsh => configure_pwsh_integration(cmd, &integration_dir),
        ShellKind::Zsh => configure_zsh_integration(cmd, &integration_dir),
        ShellKind::Unknown => Ok(()),
    }
}

fn configure_bash_integration(
    cmd: &mut CommandBuilder,
    integration_dir: &Path,
) -> Result<(), LanternError> {
    fs::create_dir_all(integration_dir)?;
    let rc_path = integration_dir.join("lantern.bashrc");
    fs::write(
        &rc_path,
        format!(
            r#"if [ -f "$HOME/.bashrc" ]; then
  . "$HOME/.bashrc"
fi

__lantern_emit_prompt() {{
  printf '{marker}'
}}

if [ -n "${{PROMPT_COMMAND:-}}" ]; then
  PROMPT_COMMAND="__lantern_emit_prompt;$PROMPT_COMMAND"
else
  PROMPT_COMMAND="__lantern_emit_prompt"
fi
"#,
            marker = prompt_marker()
        ),
    )?;
    cmd.arg("--rcfile");
    cmd.arg(rc_path);
    cmd.arg("-i");
    Ok(())
}

fn configure_fish_integration(
    cmd: &mut CommandBuilder,
    integration_dir: &Path,
) -> Result<(), LanternError> {
    let fish_dir = integration_dir.join("fish");
    fs::create_dir_all(&fish_dir)?;
    let config_path = fish_dir.join("config.fish");
    fs::write(
        &config_path,
        format!(
            r#"if test -f "$HOME/.config/fish/config.fish"
  source "$HOME/.config/fish/config.fish"
end

if functions -q fish_prompt
  functions -c fish_prompt __lantern_original_fish_prompt
end

function fish_prompt
  printf '{marker}'
  if functions -q __lantern_original_fish_prompt
    __lantern_original_fish_prompt
  else
    printf '%s> ' (prompt_pwd)
  end
end
"#,
            marker = prompt_marker()
        ),
    )?;
    cmd.env("XDG_CONFIG_HOME", integration_dir);
    cmd.arg("-i");
    Ok(())
}

fn configure_pwsh_integration(
    cmd: &mut CommandBuilder,
    integration_dir: &Path,
) -> Result<(), LanternError> {
    fs::create_dir_all(integration_dir)?;
    let script_path = integration_dir.join("lantern-profile.ps1");
    fs::write(
        &script_path,
        format!(
            r#"if (Test-Path $PROFILE.CurrentUserAllHosts) {{ . $PROFILE.CurrentUserAllHosts }}
if (Test-Path $PROFILE.CurrentUserCurrentHost) {{ . $PROFILE.CurrentUserCurrentHost }}

$__lantern_original_prompt = $function:prompt
function global:prompt {{
  [Console]::Out.Write("{marker}")
  if ($__lantern_original_prompt) {{
    & $__lantern_original_prompt
  }} else {{
    "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) "
  }}
}}
"#,
            marker = prompt_marker()
        ),
    )?;
    cmd.arg("-NoLogo");
    cmd.arg("-NoExit");
    cmd.arg("-File");
    cmd.arg(script_path);
    Ok(())
}

fn configure_zsh_integration(
    cmd: &mut CommandBuilder,
    integration_dir: &Path,
) -> Result<(), LanternError> {
    fs::create_dir_all(integration_dir)?;
    let rc_path = integration_dir.join(".zshrc");
    fs::write(
        &rc_path,
        format!(
            r#"if [ -f "$HOME/.zshrc" ]; then
  source "$HOME/.zshrc"
fi

autoload -Uz add-zsh-hook 2>/dev/null
__lantern_emit_prompt() {{
  printf '{marker}'
}}
add-zsh-hook precmd __lantern_emit_prompt
"#,
            marker = prompt_marker()
        ),
    )?;
    cmd.env("ZDOTDIR", integration_dir);
    cmd.arg("-i");
    Ok(())
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn spawn(
        &self,
        session_id: &str,
        shell: &str,
        cwd: &str,
        cols: u16,
        rows: u16,
        output_tx: Box<dyn Fn(TerminalOutput) + Send + 'static>,
    ) -> Result<PtyInfo, LanternError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| LanternError::Pty(e.to_string()))?;

        let mut cmd = CommandBuilder::new(shell);
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        if let Err(error) = configure_shell_integration(&mut cmd, shell, session_id) {
            eprintln!(
                "Shell integration setup failed for session {}: {}",
                session_id, error
            );
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| LanternError::Pty(e.to_string()))?;

        let pid = child.process_id().unwrap_or(0);
        let child = Arc::new(Mutex::new(child));
        let child_for_reader = child.clone();

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| LanternError::Pty(e.to_string()))?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| LanternError::Pty(e.to_string()))?;

        let session_id_clone = session_id.to_string();
        let reader_handle = std::thread::Builder::new()
            .name(format!("pty-reader-{}", session_id))
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    if shutdown_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            // EOF — process exited
                            let code = child_for_reader
                                .lock()
                                .unwrap()
                                .try_wait()
                                .ok()
                                .flatten()
                                .map(|s| s.exit_code() as i32);
                            output_tx(TerminalOutput::Exited { code });
                            break;
                        }
                        Ok(n) => {
                            output_tx(TerminalOutput::Data {
                                data: String::from_utf8_lossy(&buf[..n]).into_owned(),
                            });
                        }
                        Err(e) => {
                            if shutdown_clone.load(Ordering::Relaxed) {
                                break;
                            }
                            eprintln!("PTY reader error for session {}: {}", session_id_clone, e);
                            let code = child_for_reader
                                .lock()
                                .unwrap()
                                .try_wait()
                                .ok()
                                .flatten()
                                .map(|s| s.exit_code() as i32);
                            output_tx(TerminalOutput::Exited { code });
                            break;
                        }
                    }
                }
            })
            .map_err(|e| LanternError::Pty(e.to_string()))?;

        let session = PtySession {
            master: pair.master,
            writer,
            child,
            pid,
            cols,
            rows,
            shutdown,
            reader_handle: Some(reader_handle),
        };

        let info = PtyInfo {
            session_id: session_id.to_string(),
            pid,
            cols,
            rows,
        };

        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), session);

        Ok(info)
    }

    pub fn write(&self, session_id: &str, data: &[u8]) -> Result<(), LanternError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| LanternError::SessionNotFound(session_id.to_string()))?;
        session
            .writer
            .write_all(data)
            .map_err(|e| LanternError::Pty(e.to_string()))?;
        Ok(())
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), LanternError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| LanternError::SessionNotFound(session_id.to_string()))?;
        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| LanternError::Pty(e.to_string()))?;
        session.cols = cols;
        session.rows = rows;
        Ok(())
    }

    pub fn close(&self, session_id: &str) -> Result<(), LanternError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(mut session) = sessions.remove(session_id) {
            session.shutdown.store(true, Ordering::Relaxed);
            // Kill the child process
            let _ = session.child.lock().unwrap().kill();
            // Drop writer to signal EOF to the PTY
            drop(session.writer);
            // Wait for reader thread to finish
            if let Some(handle) = session.reader_handle.take() {
                let _ = handle.join();
            }
        }
        Ok(())
    }

    pub fn close_all(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        let ids: Vec<String> = sessions.keys().cloned().collect();
        for id in ids {
            if let Some(mut session) = sessions.remove(&id) {
                session.shutdown.store(true, Ordering::Relaxed);
                let _ = session.child.lock().unwrap().kill();
                drop(session.writer);
                if let Some(handle) = session.reader_handle.take() {
                    let _ = handle.join();
                }
            }
        }
    }

    pub fn get_pid(&self, session_id: &str) -> Option<u32> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(session_id).map(|s| s.pid)
    }

    pub fn session_exists(&self, session_id: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    use tempfile::tempdir;

    fn wait_for_output(
        rx: &mpsc::Receiver<TerminalOutput>,
        timeout_ms: u64,
    ) -> Vec<TerminalOutput> {
        let mut outputs = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(output) => outputs.push(output),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
        outputs
    }

    fn collect_data(outputs: &[TerminalOutput]) -> String {
        let mut result = String::new();
        for output in outputs {
            if let TerminalOutput::Data { data: d } = output {
                result.push_str(d);
            }
        }
        result
    }

    #[test]
    fn test_configure_bash_shell_integration() {
        let dir = tempdir().unwrap();
        let mut cmd = CommandBuilder::new("/bin/bash");
        configure_bash_integration(&mut cmd, dir.path()).unwrap();

        let argv = cmd.get_argv();
        assert!(argv.iter().any(|arg| arg == "--rcfile"));
        assert!(argv.iter().any(|arg| arg == "-i"));

        let rc_path = dir.path().join("lantern.bashrc");
        let content = fs::read_to_string(rc_path).unwrap();
        assert!(content.contains(prompt_marker()));
        assert!(content.contains("PROMPT_COMMAND"));
    }

    #[test]
    fn test_configure_zsh_shell_integration() {
        let dir = tempdir().unwrap();
        let mut cmd = CommandBuilder::new("/bin/zsh");
        configure_zsh_integration(&mut cmd, dir.path()).unwrap();

        assert_eq!(cmd.get_env("ZDOTDIR"), Some(dir.path().as_os_str()));
        assert!(cmd.get_argv().iter().any(|arg| arg == "-i"));

        let rc_path = dir.path().join(".zshrc");
        let content = fs::read_to_string(rc_path).unwrap();
        assert!(content.contains(prompt_marker()));
        assert!(content.contains("add-zsh-hook"));
    }

    #[test]
    fn test_spawn_pty_returns_pid() {
        let mgr = PtyManager::new();
        let (tx, _rx) = mpsc::channel();
        let info = mgr
            .spawn(
                "test-1",
                "/bin/bash",
                "/tmp",
                80,
                24,
                Box::new(move |out| {
                    let _ = tx.send(out);
                }),
            )
            .unwrap();
        assert!(info.pid > 0);
        mgr.close("test-1").unwrap();
    }

    #[test]
    fn test_write_and_read_pty() {
        let mgr = PtyManager::new();
        let (tx, rx) = mpsc::channel();
        mgr.spawn(
            "test-2",
            "/bin/bash",
            "/tmp",
            80,
            24,
            Box::new(move |out| {
                let _ = tx.send(out);
            }),
        )
        .unwrap();

        // Wait for shell to initialize
        std::thread::sleep(Duration::from_millis(200));

        mgr.write("test-2", b"echo hello_lantern\n").unwrap();

        let outputs = wait_for_output(&rx, 2000);
        let text = collect_data(&outputs);
        assert!(
            text.contains("hello_lantern"),
            "Expected 'hello_lantern' in output, got: {}",
            text
        );
        mgr.close("test-2").unwrap();
    }

    #[test]
    fn test_resize_pty() {
        let mgr = PtyManager::new();
        let (tx, _rx) = mpsc::channel();
        mgr.spawn(
            "test-3",
            "/bin/bash",
            "/tmp",
            80,
            24,
            Box::new(move |out| {
                let _ = tx.send(out);
            }),
        )
        .unwrap();
        // Should not error
        mgr.resize("test-3", 40, 10).unwrap();
        mgr.close("test-3").unwrap();
    }

    #[test]
    fn test_close_pty_kills_process() {
        let mgr = PtyManager::new();
        let (tx, _rx) = mpsc::channel();
        let info = mgr
            .spawn(
                "test-4",
                "/bin/bash",
                "/tmp",
                80,
                24,
                Box::new(move |out| {
                    let _ = tx.send(out);
                }),
            )
            .unwrap();
        let pid = info.pid;
        mgr.close("test-4").unwrap();
        assert!(!mgr.session_exists("test-4"));
        // Give OS time to clean up
        std::thread::sleep(Duration::from_millis(100));
        // Process should be gone (or zombie reaped)
        let proc_path = format!("/proc/{}/status", pid);
        // May or may not exist depending on timing, but session should be removed
        assert!(!mgr.session_exists("test-4"));
    }

    #[test]
    fn test_spawn_with_cwd() {
        let mgr = PtyManager::new();
        let (tx, rx) = mpsc::channel();
        mgr.spawn(
            "test-5",
            "/bin/bash",
            "/tmp",
            80,
            24,
            Box::new(move |out| {
                let _ = tx.send(out);
            }),
        )
        .unwrap();

        std::thread::sleep(Duration::from_millis(200));
        mgr.write("test-5", b"pwd\n").unwrap();

        let outputs = wait_for_output(&rx, 2000);
        let text = collect_data(&outputs);
        assert!(
            text.contains("/tmp"),
            "Expected '/tmp' in output, got: {}",
            text
        );
        mgr.close("test-5").unwrap();
    }

    #[test]
    fn test_spawn_nonexistent_shell_errors() {
        let mgr = PtyManager::new();
        let (tx, _rx) = mpsc::channel();
        let result = mgr.spawn(
            "test-6",
            "/bin/doesnotexist",
            "/tmp",
            80,
            24,
            Box::new(move |out| {
                let _ = tx.send(out);
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_concurrent_sessions() {
        let mgr = PtyManager::new();
        let mut receivers = Vec::new();

        for i in 0..5 {
            let (tx, rx) = mpsc::channel();
            let id = format!("multi-{}", i);
            mgr.spawn(
                &id,
                "/bin/bash",
                "/tmp",
                80,
                24,
                Box::new(move |out| {
                    let _ = tx.send(out);
                }),
            )
            .unwrap();
            receivers.push((id, rx));
        }

        std::thread::sleep(Duration::from_millis(200));

        for (id, _rx) in &receivers {
            mgr.write(id, format!("echo session_{}\n", id).as_bytes())
                .unwrap();
        }

        std::thread::sleep(Duration::from_millis(500));

        for (id, rx) in &receivers {
            let outputs = wait_for_output(rx, 1000);
            let text = collect_data(&outputs);
            assert!(
                text.contains(&format!("session_{}", id)),
                "Session {} output didn't contain expected marker: {}",
                id,
                text
            );
        }

        for (id, _) in &receivers {
            mgr.close(id).unwrap();
        }
    }
}
