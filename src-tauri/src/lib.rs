pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod git;
pub mod paths;
pub mod pty;
pub mod state;

use state::AppState;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let user_config = config::UserConfig::load();
    let db = db::init_db(None).expect("Failed to initialize database");
    let pty_manager = pty::PtyManager::new();

    // Load existing layout for initial state
    let saved_layout = db::load_layout(&db).ok().flatten();
    let initial_sidebar_width = saved_layout
        .as_ref()
        .map(|l| l.sidebar_width)
        .unwrap_or(250);
    let initial_sidebar_collapsed = saved_layout
        .as_ref()
        .map(|l| l.sidebar_collapsed)
        .unwrap_or(false);
    let initial_active_repo = saved_layout.as_ref().and_then(|l| l.active_repo_id.clone());
    let initial_collapsed_groups = saved_layout
        .as_ref()
        .map(|l| l.collapsed_group_ids.clone())
        .unwrap_or_default();

    let git_poll_interval = Arc::new(AtomicU64::new(user_config.git_poll_interval_secs));

    let app_state = AppState {
        pty_manager,
        db: db.clone(),
        config: Mutex::new(user_config.clone()),
        sidebar_width: Mutex::new(initial_sidebar_width),
        sidebar_collapsed: Mutex::new(initial_sidebar_collapsed),
        active_repo_id: Mutex::new(initial_active_repo),
        collapsed_group_ids: Mutex::new(initial_collapsed_groups),
        git_poll_interval: git_poll_interval.clone(),
    };

    // Start git polling thread
    let db_for_git = db.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::repo::repo_add,
            commands::repo::repo_remove,
            commands::repo::repo_list,
            commands::repo::repo_reorder,
            commands::repo::repo_get_all_git_info,
            commands::repo::repo_detect_worktrees,
            commands::repo::repo_add_with_worktrees,
            commands::terminal::terminal_create,
            commands::terminal::terminal_list,
            commands::terminal::terminal_close,
            commands::terminal::terminal_rename,
            commands::terminal::terminal_set_active,
            commands::terminal::terminal_get_active,
            commands::pty_io::terminal_write,
            commands::pty_io::terminal_write_raw,
            commands::pty_io::terminal_resize,
            commands::pty_io::terminal_subscribe,
            commands::pty_io::terminal_get_foreground_process,
            commands::config::config_get,
            commands::config::config_update,
            commands::layout::state_save_layout,
            commands::layout::state_load_layout,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            // Start git polling in background
            let poll_interval = git_poll_interval.clone();
            std::thread::Builder::new()
                .name("git-poller".to_string())
                .spawn(move || {
                    loop {
                        let interval = poll_interval.load(Ordering::Relaxed);
                        if interval == 0 {
                            // Polling disabled, sleep and check again
                            std::thread::sleep(std::time::Duration::from_secs(1));
                            continue;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(interval));
                        let repos = match db::list_repos(&db_for_git) {
                            Ok(r) => r,
                            Err(_) => continue,
                        };
                        let infos: Vec<(String, git::GitInfo)> = repos
                            .iter()
                            .map(|r| (r.id.clone(), git::git_info_for_path(&r.path)))
                            .collect();
                        let _ = handle.emit("git-status-update", &infos);
                    }
                })
                .ok();
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state: tauri::State<AppState> = window.state();
                // Save layout with actual sidebar width and active repo
                if let Ok(pos) = window.outer_position() {
                    if let Ok(size) = window.outer_size() {
                        let maximized = window.is_maximized().unwrap_or(false);
                        let sidebar_width = *state.sidebar_width.lock().unwrap();
                        let sidebar_collapsed = *state.sidebar_collapsed.lock().unwrap();
                        let active_repo_id = state.active_repo_id.lock().unwrap().clone();
                        let collapsed_group_ids = state.collapsed_group_ids.lock().unwrap().clone();
                        let layout = db::AppLayout {
                            window_x: Some(pos.x),
                            window_y: Some(pos.y),
                            window_width: size.width as i32,
                            window_height: size.height as i32,
                            window_maximized: maximized,
                            sidebar_width,
                            sidebar_collapsed,
                            active_repo_id,
                            collapsed_group_ids,
                        };
                        let _ = db::save_layout(&state.db, &layout);
                    }
                }
                // Kill all PTYs
                state.pty_manager.close_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
