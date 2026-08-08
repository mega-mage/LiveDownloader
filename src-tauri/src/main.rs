#![cfg_attr(all(not(debug_assertions), feature = "gui"), windows_subsystem = "windows")]
#![allow(linker_messages)]

pub mod common;
mod config;
mod engine;
mod platforms;
mod stream;
#[cfg(feature = "gui")]
mod commands;
mod cli;
#[cfg(feature = "server")]
mod server;

use config::{AppConfig, get_config_paths};
use engine::manager::{RoomStatus, TaskManager};
use stream::proxy::StreamProxy;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub struct AppState {
    pub room_statuses: Arc<tokio::sync::RwLock<HashMap<String, RoomStatus>>>,
    pub config_toml_path: PathBuf,
    pub proxy_port: u16,
    pub change_notify: Arc<tokio::sync::Notify>,
    pub is_paused: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordedVideo {
    name: String,
    path: String,
    size: u64,
    modified: String,
    anchor: String,
}

fn adjust_current_dir() {
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.ends_with("src-tauri") {
            if let Some(parent) = cwd.parent() {
                let _ = std::env::set_current_dir(parent);
            }
        }
    }
}

fn init_logging(config_toml_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config_dir = config_toml_path.parent().unwrap();
    let _ = fs::create_dir_all(config_dir);
    let log_path = config_dir.join("app.log");
    if log_path.exists() && fs::metadata(&log_path)?.len() > 10 * 1024 * 1024 {
        let _ = fs::remove_file(&log_path);
    }
    let log_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&log_path)?;

    let file_layer = fmt::layer().with_ansi(false).with_writer(log_file);
    let _ = tracing_subscriber::registry()
        .with(fmt::layer().with_ansi(true))
        .with(file_layer)
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,LiveDownloader=debug")))
        .try_init();

    Ok(())
}

/// Shared initialization: setup TaskManager, proxy, and shared state
fn init_core(config_toml_path: &Path) -> Result<(
    TaskManager,
    std::net::TcpListener,
    u16,
    Arc<tokio::sync::Notify>,
    Arc<std::sync::atomic::AtomicBool>,
    Arc<tokio::sync::RwLock<HashMap<String, RoomStatus>>>,
), Box<dyn std::error::Error + Send + Sync>> {
    let initial_paused = AppConfig::load_or_create(config_toml_path)
        .map(|c| c.settings.engine_paused)
        .unwrap_or(false);
    let is_paused = Arc::new(std::sync::atomic::AtomicBool::new(initial_paused));
    let manager = TaskManager::new(config_toml_path, is_paused.clone())?;
    let room_statuses = manager.room_statuses.clone();

    let proxy_listener = std::net::TcpListener::bind("0.0.0.0:0")
        .or_else(|_| std::net::TcpListener::bind("127.0.0.1:0"))?;
    let proxy_port = proxy_listener.local_addr()?.port();
    proxy_listener.set_nonblocking(true)?;
    info!("Stream proxy will listen on port {}", proxy_port);

    let change_notify = Arc::new(tokio::sync::Notify::new());

    Ok((manager, proxy_listener, proxy_port, change_notify, is_paused, room_statuses))
}

// ========== GUI Mode (Tauri) ==========
#[cfg(feature = "gui")]
fn run_gui(config_toml_path: PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::Manager;

    let (manager, proxy_listener, proxy_port, change_notify, is_paused, room_statuses) =
        init_core(&config_toml_path)?;

    let manager_cell = std::sync::Mutex::new(Some(manager));
    let proxy_listener_cell = std::sync::Mutex::new(Some(proxy_listener));
    let task_notify = change_notify.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            room_statuses,
            config_toml_path,
            proxy_port,
            change_notify,
            is_paused,
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_rooms,
            commands::add_room,
            commands::delete_room,
            commands::get_config,
            commands::save_config,
            commands::get_logs,
            commands::get_proxy_port,
            commands::toggle_engine_status,
            commands::get_engine_status,
            commands::get_recorded_videos,
            commands::open_recorded_folder,
            commands::toggle_room_paused,
            commands::save_cookie,
            commands::update_room_config,
            commands::execute_ld_command,
            commands::delete_video_file
        ])
        .setup(move |app| {
            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "完全退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                        if button == MouseButton::Left && button_state == MouseButtonState::Down {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;

            if let Some(mut mgr) = manager_cell.lock().unwrap().take() {
                let notify = task_notify.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = mgr.run(notify).await {
                        error!("Error running Task Manager loop: {}", e);
                    }
                });
                info!("TaskManager background loop spawned successfully.");
            }

            if let Some(std_listener) = proxy_listener_cell.lock().unwrap().take() {
                tauri::async_runtime::spawn(async move {
                    let tokio_listener = tokio::net::TcpListener::from_std(std_listener)
                        .expect("Failed to convert std TcpListener to tokio TcpListener");
                    StreamProxy::start_with_listener(tokio_listener);
                });
                info!("Stream proxy server started.");
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                info!("Main window closed. Hiding to system tray, downloads will continue in background.");
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}

// ========== Server Mode (Axum) ==========
#[cfg(feature = "server")]
fn run_server(config_toml_path: PathBuf, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let (mut manager, proxy_listener, proxy_port, change_notify, is_paused, room_statuses) =
            init_core(&config_toml_path)?;

        let task_notify = change_notify.clone();

        // Spawn TaskManager
        tokio::spawn(async move {
            if let Err(e) = manager.run(task_notify).await {
                error!("Error running Task Manager loop: {}", e);
            }
        });
        info!("TaskManager background loop spawned successfully.");

        // Spawn stream proxy
        let tokio_listener = tokio::net::TcpListener::from_std(proxy_listener)?;
        StreamProxy::start_with_listener(tokio_listener);
        info!("Stream proxy server started on port {}.", proxy_port);

        let state = Arc::new(AppState {
            room_statuses,
            config_toml_path,
            proxy_port,
            change_notify,
            is_paused,
        });

        server::start_server(state, port).await
    })
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    adjust_current_dir();

    let (config_toml_path, _) = get_config_paths();

    let args: Vec<String> = std::env::args().collect();

    let has_cli_subcommand = args.iter().skip(1).any(|a| {
        !a.starts_with("--server")
            && !a.starts_with("--port")
            && !a.starts_with("--token")
            && !a.starts_with("--api-token")
            && a.parse::<u16>().is_err()
    });

    let is_server_mode = args.iter().any(|a| a == "--server")
        || (!has_cli_subcommand && !cfg!(feature = "gui") && cfg!(feature = "server"));

    let _port = args.iter().position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or_else(|| {
            AppConfig::load_or_create(&config_toml_path)
                .map(|c| c.settings.server_port)
                .unwrap_or(10730)
        });

    // Handle --token / --api-token command-line flag if provided
    if let Some(pos) = args.iter().position(|a| a == "--token" || a == "--api-token") {
        if let Some(token_val) = args.get(pos + 1) {
            let mut config = AppConfig::load_or_create(&config_toml_path)?;
            if token_val == "clear" || token_val == "none" {
                config.settings.api_token = None;
                println!("已成功清除 API Token 配置。");
            } else {
                config.settings.api_token = Some(token_val.trim().to_string());
                println!("已成功保存 API Token: {}", token_val.trim());
            }
            config.save_to_file(&config_toml_path)?;
            if !is_server_mode {
                return Ok(());
            }
        }
    }

    // CLI execution: if CLI subcommand (like help, ls, rooms, add) is supplied without --server flag, execute CLI and exit
    if has_cli_subcommand && !args.iter().any(|a| a == "--server") {
        if let Err(e) = cli::run_cli_commands(&config_toml_path) {
            eprintln!("CLI execution error: {}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    if is_server_mode {
        init_logging(&config_toml_path)?;
        info!("Starting LiveDownloader in Web Server mode...");

        #[cfg(feature = "server")]
        {
            return run_server(config_toml_path, _port);
        }

        #[cfg(not(feature = "server"))]
        {
            eprintln!("错误: 当前二进制未启用 'server' 特性。请使用以下命令重新编译:");
            eprintln!("  cargo build --no-default-features --features server");
            std::process::exit(1);
        }
    }

    // GUI Mode
    init_logging(&config_toml_path)?;
    info!("Starting LiveDownloader Tauri GUI Wrapper...");

    #[cfg(feature = "gui")]
    {
        return run_gui(config_toml_path);
    }

    #[cfg(not(feature = "gui"))]
    {
        eprintln!("错误: 当前二进制未启用 'gui' 特性，且未指定 --server 模式。");
        eprintln!("请使用 --server 启动 Web 服务端模式，或使用 gui 特性重新编译。");
        std::process::exit(1);
    }
}
