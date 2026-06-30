use crate::AppState;
use crate::config::{AppConfig, LiveUrlConfig};
use crate::engine::manager::RoomStatus;
use crate::RecordedVideo;

use std::fs;
use std::path::PathBuf;
use tauri::State;
use tracing::{info, debug};

#[tauri::command]
pub async fn get_rooms(state: State<'_, AppState>) -> Result<Vec<RoomStatus>, String> {
    let config = AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;
    let map = state.room_statuses.read().await;

    let mut result = Vec::new();
    for r in config.rooms {
        if r.is_commented {
            result.push(RoomStatus {
                url: r.url.clone(),
                title: "".to_string(),
                anchor_name: r.name.clone().unwrap_or_else(|| "未知主播".to_string()),
                status: "Paused".to_string(),
                record_path: None,
                live_url: None,
                platform: "".to_string(),
            });
        } else {
            if let Some(status) = map.get(&r.url) {
                result.push(status.clone());
            } else {
                result.push(RoomStatus {
                    url: r.url.clone(),
                    title: "".to_string(),
                    anchor_name: r.name.clone().unwrap_or_else(|| "未知主播".to_string()),
                    status: "Idle".to_string(),
                    record_path: None,
                    live_url: None,
                    platform: "".to_string(),
                });
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn add_room(
    url: String,
    name: Option<String>,
    quality: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config =
        AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;
    let mut clean_url = url.trim().to_string();
    if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
        clean_url = format!("https://{}", clean_url);
    }

    if config.rooms.iter().any(|r| r.url == clean_url) {
        return Err("该直播间地址已在监控列表中".to_string());
    }

    config.rooms.push(LiveUrlConfig {
        url: clean_url,
        name: if name.as_ref().map_or(true, |n| n.is_empty()) {
            None
        } else {
            name
        },
        quality: if quality.as_ref().map_or(true, |q| q.is_empty()) {
            None
        } else {
            quality
        },
        video_save_type: None,
        is_commented: false,
    });

    config
        .save_to_file(&state.config_toml_path)
        .map_err(|e| e.to_string())?;
    state.change_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn delete_room(url: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut config =
        AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;
    config.rooms.retain(|r| r.url != url);
    config
        .save_to_file(&state.config_toml_path)
        .map_err(|e| e.to_string())?;

    {
        let mut map = state.room_statuses.write().await;
        map.remove(&url);
    }

    state.change_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn update_room_config(
    url: String,
    name: Option<String>,
    quality: Option<String>,
    video_save_type: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!(
        "update_room_config called. URL: '{}', Name: {:?}, Quality: {:?}, Format: {:?}",
        url, name, quality, video_save_type
    );
    let mut config =
        AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;

    if let Some(room) = config.rooms.iter_mut().find(|r| r.url == url) {
        room.name = if name.as_ref().map_or(true, |n| n.trim().is_empty()) {
            None
        } else {
            name.map(|s| s.trim().to_string())
        };
        room.quality = if quality.as_ref().map_or(true, |q| q.trim().is_empty()) {
            None
        } else {
            quality.map(|s| s.trim().to_string())
        };
        room.video_save_type = if video_save_type
            .as_ref()
            .map_or(true, |f| f.trim().is_empty())
        {
            None
        } else {
            video_save_type.map(|s| s.trim().to_string())
        };

        config
            .save_to_file(&state.config_toml_path)
            .map_err(|e| e.to_string())?;
        state.change_notify.notify_one();
        Ok(())
    } else {
        Err("找不到该直播间监控配置".to_string())
    }
}

#[tauri::command]
pub async fn toggle_room_paused(
    url: String,
    paused: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config =
        AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;
    if let Some(room) = config.rooms.iter_mut().find(|r| r.url == url) {
        room.is_commented = paused;
    } else {
        return Err("未找到该直播间".to_string());
    }
    config
        .save_to_file(&state.config_toml_path)
        .map_err(|e| e.to_string())?;

    if paused {
        let mut map = state.room_statuses.write().await;
        map.remove(&url);
    }

    state.change_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;
    debug!(
        "get_config called. Loaded cookies keys: {:?}",
        config.cookies.keys().collect::<Vec<_>>()
    );
    Ok(config)
}

#[tauri::command]
pub async fn save_config(new_config: AppConfig, state: State<'_, AppState>) -> Result<(), String> {
    info!(
        "save_config called. Cookies keys to save: {:?}",
        new_config.cookies.keys().collect::<Vec<_>>()
    );
    for (k, v) in &new_config.cookies {
        info!("  Cookie for platform '{}' length: {}", k, v.len());
    }
    new_config
        .save_to_file(&state.config_toml_path)
        .map_err(|e| e.to_string())?;
    state.change_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn save_cookie(
    platform: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!(
        "save_cookie called. Platform: '{}', Value length: {}",
        platform,
        value.len()
    );
    let mut config =
        AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;
    config.cookies.insert(platform, value);
    config
        .save_to_file(&state.config_toml_path)
        .map_err(|e| e.to_string())?;
    state.change_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn get_logs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let log_path = state.config_toml_path.parent().unwrap().join("app.log");
    if !log_path.exists() {
        return Ok(vec!["No logs available yet.".to_string()]);
    }
    let content = fs::read_to_string(&log_path).map_err(|e| e.to_string())?;
    let all_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let len = all_lines.len();
    let start = if len > 150 { len - 150 } else { 0 };
    let lines = all_lines[start..].to_vec();
    Ok(lines)
}

#[tauri::command]
pub async fn get_proxy_port(state: State<'_, AppState>) -> Result<u16, String> {
    Ok(state.proxy_port)
}

#[tauri::command]
pub async fn toggle_engine_status(paused: bool, state: State<'_, AppState>) -> Result<(), String> {
    state
        .is_paused
        .store(paused, std::sync::atomic::Ordering::SeqCst);
    state.change_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn get_engine_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.is_paused.load(std::sync::atomic::Ordering::SeqCst))
}

#[tauri::command]
pub async fn get_recorded_videos(state: State<'_, AppState>) -> Result<Vec<RecordedVideo>, String> {
    let config = AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;
    let save_path = config.settings.save_path;
    if !save_path.exists() {
        return Ok(Vec::new());
    }

    let mut videos = Vec::new();
    let mut dirs_to_visit = vec![save_path.clone()];
    let allowed_exts = vec!["ts", "mp4", "mkv", "flv", "mp3", "m4a"];

    while let Some(dir) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path != save_path.join("downloading") {
                        dirs_to_visit.push(path);
                    }
                } else if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if allowed_exts.contains(&ext.to_lowercase().as_str()) {
                            let name = path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            let modified = entry
                                .metadata()
                                .and_then(|m| m.modified())
                                .map(|t| {
                                    let datetime: chrono::DateTime<chrono::Local> = t.into();
                                    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                                })
                                .unwrap_or_default();

                            let anchor = if let Some(parent) = path.parent() {
                                if parent != save_path {
                                    parent
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("Unknown")
                                        .to_string()
                                    } else {
                                        "Unknown".to_string()
                                    }
                            } else {
                                "Unknown".to_string()
                            };

                            videos.push(RecordedVideo {
                                name,
                                path: path.to_string_lossy().to_string(),
                                size,
                                modified,
                                anchor,
                            });
                        }
                    }
                }
            }
        }
    }
    videos.sort_by(|a, b| b.modified.cmp(&a.modified));
    debug!(
        "get_recorded_videos called. save_path={:?}, found {} videos",
        save_path,
        videos.len()
    );
    Ok(videos)
}

#[tauri::command]
pub async fn open_recorded_folder(path: String) -> Result<(), String> {
    let mut file_path = PathBuf::from(path);

    // Resolve relative path to absolute using current directory
    if file_path.is_relative() {
        if let Ok(current_dir) = std::env::current_dir() {
            file_path = current_dir.join(file_path);
        }
    }

    // Normalize slashes for Windows Explorer
    #[cfg(target_os = "windows")]
    let file_path = {
        let path_str = file_path.to_string_lossy().replace('/', "\\");
        PathBuf::from(path_str)
    };

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let folder_path = if file_path.is_dir() {
            file_path
        } else {
            file_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        };
        opener::open(folder_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn split_arguments(command_line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    let cmd_trimmed = command_line.trim();
    let cmd_to_parse = if cmd_trimmed.starts_with("ld ") {
        &cmd_trimmed[3..]
    } else if cmd_trimmed.starts_with("ld.exe ") {
        &cmd_trimmed[7..]
    } else {
        cmd_trimmed
    };

    for c in cmd_to_parse.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '　' if !in_quotes => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

#[tauri::command]
pub async fn execute_ld_command(cmd: String) -> Result<String, String> {
    info!("execute_ld_command called with: {}", cmd);
    let parsed_args = split_arguments(&cmd);
    if parsed_args.is_empty() {
        return Err("指令为空".to_string());
    }

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;

    let mut command = std::process::Command::new(current_exe);
    command.args(&parsed_args);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = command.output().map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let mut result = stdout;
    if !stderr.is_empty() {
        if !result.is_empty() {
            result.push_str("\n");
        }
        result.push_str(&stderr);
    }

    Ok(result)
}

#[tauri::command]
pub async fn delete_video_file(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let config = AppConfig::load_or_create(&state.config_toml_path).map_err(|e| e.to_string())?;

    // Directory traversal security check
    let save_path = std::fs::canonicalize(&config.settings.save_path)
        .map_err(|e| format!("Failed to canonicalize save path: {}", e))?;
    let file_path = std::path::Path::new(&path);
    let target_path = std::fs::canonicalize(file_path)
        .map_err(|e| format!("Invalid file path: {}", e))?;

    if !target_path.starts_with(&save_path) || !target_path.is_file() {
        return Err("Access denied: directory traversal blocked".to_string());
    }

    std::fs::remove_file(&target_path)
        .map_err(|e| format!("Failed to delete file: {}", e))?;

    Ok(())
}
