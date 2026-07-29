use crate::AppState;
use crate::config::AppConfig;
use crate::engine::manager::RoomStatus;
use crate::RecordedVideo;

use axum::{
    Router,
    routing::{get, post, delete},
    extract::State as AxumState,
    http::{StatusCode, HeaderMap},
    Json,
    response::IntoResponse,
    middleware::{self, Next},
};
use tower_http::cors::CorsLayer;
use std::sync::Arc;
use tracing::info;

type SharedState = Arc<AppState>;

fn parse_query_param_simple(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == key {
                return Some(urlencoding::decode(v).unwrap_or_default().to_string());
            }
        }
    }
    None
}

/// Auth middleware: checks Bearer token against config api_token (or query parameter token/api_token)
async fn auth_middleware(
    state: AxumState<SharedState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> impl IntoResponse {
    // Bypass auth for CORS preflight OPTIONS requests
    if request.method() == axum::http::Method::OPTIONS {
        return Ok(next.run(request).await);
    }

    let config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if let Some(ref expected_token) = config.settings.api_token {
        if !expected_token.trim().is_empty() {
            let auth_header = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let provided_token = if !auth_header.is_empty() {
                auth_header.strip_prefix("Bearer ").unwrap_or("").to_string()
            } else {
                let query_str = request.uri().query().unwrap_or("");
                parse_query_param_simple(query_str, "token")
                    .or_else(|| parse_query_param_simple(query_str, "api_token"))
                    .unwrap_or_default()
            };

            if provided_token != *expected_token {
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }

    Ok(next.run(request).await)
}

async fn api_get_rooms(state: AxumState<SharedState>) -> impl IntoResponse {
    let config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
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
                split_mode: r.split_mode.clone(),
                split_custom_secs: r.split_custom_secs,
                current_auto_duration_secs: None,
            });
        } else if let Some(status) = map.get(&r.url) {
            let mut status_clone = status.clone();
            status_clone.split_mode = r.split_mode.clone();
            status_clone.split_custom_secs = r.split_custom_secs;
            result.push(status_clone);
        } else {
            result.push(RoomStatus {
                url: r.url.clone(),
                title: "".to_string(),
                anchor_name: r.name.clone().unwrap_or_else(|| "未知主播".to_string()),
                status: "Idle".to_string(),
                record_path: None,
                live_url: None,
                platform: "".to_string(),
                split_mode: r.split_mode.clone(),
                split_custom_secs: r.split_custom_secs,
                current_auto_duration_secs: None,
            });
        }
    }
    Ok(Json(result))
}

#[derive(serde::Deserialize)]
pub struct AddRoomRequest {
    url: String,
    name: Option<String>,
    quality: Option<String>,
    split_mode: Option<String>,
    split_custom_secs: Option<u64>,
}

async fn api_add_room(
    state: AxumState<SharedState>,
    Json(body): Json<AddRoomRequest>,
) -> impl IntoResponse {
    let mut config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    let mut clean_url = body.url.trim().to_string();
    if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
        clean_url = format!("https://{}", clean_url);
    }
    if config.rooms.iter().any(|r| r.url == clean_url) {
        return Err((StatusCode::CONFLICT, "该直播间地址已在监控列表中".to_string()));
    }
    config.rooms.push(crate::config::LiveUrlConfig {
        url: clean_url,
        name: body.name.filter(|n| !n.is_empty()),
        quality: body.quality.filter(|q| !q.is_empty()),
        video_save_type: None,
        is_commented: false,
        split_mode: body.split_mode.filter(|s| !s.trim().is_empty()),
        split_custom_secs: body.split_custom_secs,
    });
    config.save_to_file(&state.config_toml_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.change_notify.notify_one();
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
pub struct DeleteRoomRequest {
    url: String,
}

async fn api_delete_room(
    state: AxumState<SharedState>,
    Json(body): Json<DeleteRoomRequest>,
) -> impl IntoResponse {
    let mut config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    config.rooms.retain(|r| r.url != body.url);
    config.save_to_file(&state.config_toml_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    {
        let mut map = state.room_statuses.write().await;
        map.remove(&body.url);
    }
    state.change_notify.notify_one();
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn api_get_config(state: AxumState<SharedState>) -> impl IntoResponse {
    match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(config) => Ok(Json(config)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn api_save_config(
    state: AxumState<SharedState>,
    Json(new_config): Json<AppConfig>,
) -> impl IntoResponse {
    new_config.save_to_file(&state.config_toml_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.change_notify.notify_one();
    Ok::<_, (StatusCode, String)>(Json(serde_json::json!({ "ok": true })))
}

async fn api_get_logs(state: AxumState<SharedState>) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let log_path = state.config_toml_path.parent().unwrap().join("app.log");
    if !log_path.exists() {
        return Ok(Json(vec!["No logs available yet.".to_string()]));
    }
    let content = std::fs::read_to_string(&log_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let all_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let len = all_lines.len();
    let start = if len > 150 { len - 150 } else { 0 };
    Ok(Json(all_lines[start..].to_vec()))
}

async fn api_get_proxy_port(state: AxumState<SharedState>) -> impl IntoResponse {
    Json(state.proxy_port)
}

#[derive(serde::Deserialize)]
pub struct ToggleEngineRequest {
    paused: bool,
}

async fn api_toggle_engine(
    state: AxumState<SharedState>,
    Json(body): Json<ToggleEngineRequest>,
) -> impl IntoResponse {
    state.is_paused.store(body.paused, std::sync::atomic::Ordering::SeqCst);
    state.change_notify.notify_one();
    Json(serde_json::json!({ "ok": true }))
}

async fn api_get_engine_status(state: AxumState<SharedState>) -> impl IntoResponse {
    Json(state.is_paused.load(std::sync::atomic::Ordering::SeqCst))
}

async fn api_get_recorded_videos(state: AxumState<SharedState>) -> Result<Json<Vec<RecordedVideo>>, (StatusCode, String)> {
    let config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    let save_path = config.settings.save_path;
    if !save_path.exists() {
        let _ = std::fs::create_dir_all(&save_path);
    }
    let mut videos = Vec::new();
    let mut dirs_to_visit = vec![save_path.clone()];
    let allowed_exts = vec!["ts", "mp4", "mkv", "flv", "mp3", "m4a"];
    while let Some(dir) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs_to_visit.push(path);
                } else if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if allowed_exts.contains(&ext.to_lowercase().as_str()) {
                            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            let modified = entry.metadata().and_then(|m| m.modified()).map(|t| {
                                let datetime: chrono::DateTime<chrono::Local> = t.into();
                                datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                            }).unwrap_or_default();
                            let anchor = if let Some(parent) = path.parent() {
                                if parent != save_path {
                                    parent.file_name().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string()
                                } else {
                                     crate::common::utils::parse_anchor_from_filename(&name)
                                 }
                             } else {
                                 crate::common::utils::parse_anchor_from_filename(&name)
                            };
                            videos.push(RecordedVideo { name, path: path.to_string_lossy().to_string(), size, modified, anchor });
                        }
                    }
                }
            }
        }
    }
    videos.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(Json(videos))
}

#[derive(serde::Deserialize)]
pub struct SaveCookieRequest {
    platform: String,
    value: String,
}

async fn api_save_cookie(
    state: AxumState<SharedState>,
    Json(body): Json<SaveCookieRequest>,
) -> impl IntoResponse {
    let mut config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    config.cookies.insert(body.platform, body.value);
    config.save_to_file(&state.config_toml_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.change_notify.notify_one();
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
pub struct UpdateRoomConfigRequest {
    url: String,
    name: Option<String>,
    quality: Option<String>,
    video_save_type: Option<String>,
    split_mode: Option<String>,
    split_custom_secs: Option<u64>,
}

async fn api_update_room_config(
    state: AxumState<SharedState>,
    Json(body): Json<UpdateRoomConfigRequest>,
) -> impl IntoResponse {
    let mut config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    if let Some(room) = config.rooms.iter_mut().find(|r| r.url == body.url) {
        room.name = body.name.filter(|n| !n.trim().is_empty()).map(|s| s.trim().to_string());
        room.quality = body.quality.filter(|q| !q.trim().is_empty()).map(|s| s.trim().to_string());
        room.video_save_type = body.video_save_type.filter(|f| !f.trim().is_empty()).map(|s| s.trim().to_string());
        room.split_mode = body.split_mode.filter(|s| !s.trim().is_empty()).map(|s| s.trim().to_string());
        room.split_custom_secs = body.split_custom_secs;
        config.save_to_file(&state.config_toml_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        state.change_notify.notify_one();
        Ok(Json(serde_json::json!({ "ok": true })))
    } else {
        Err((StatusCode::NOT_FOUND, "找不到该直播间监控配置".to_string()))
    }
}

#[derive(serde::Deserialize)]
pub struct ToggleRoomPausedRequest {
    url: String,
    paused: bool,
}

async fn api_toggle_room_paused(
    state: AxumState<SharedState>,
    Json(body): Json<ToggleRoomPausedRequest>,
) -> impl IntoResponse {
    let mut config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    if let Some(room) = config.rooms.iter_mut().find(|r| r.url == body.url) {
        room.is_commented = body.paused;
    } else {
        return Err((StatusCode::NOT_FOUND, "未找到该直播间".to_string()));
    }
    config.save_to_file(&state.config_toml_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if body.paused {
        let mut map = state.room_statuses.write().await;
        map.remove(&body.url);
    }
    state.change_notify.notify_one();
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
pub struct ExecuteCommandRequest {
    cmd: String,
}

async fn api_execute_command(
    Json(body): Json<ExecuteCommandRequest>,
) -> impl IntoResponse {
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    let parsed_args = split_arguments(&body.cmd);
    if parsed_args.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "指令为空".to_string()));
    }
    let output = std::process::Command::new(current_exe)
        .args(&parsed_args)
        .output()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let mut result = stdout;
    if !stderr.is_empty() {
        if !result.is_empty() { result.push('\n'); }
        result.push_str(&stderr);
    }
    Ok(Json(serde_json::json!({ "output": result })))
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
            '"' => in_quotes = !in_quotes,
            ' ' | '\u{3000}' if !in_quotes => {
                if !current.is_empty() { args.push(current.clone()); current.clear(); }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() { args.push(current); }
    args
}

#[derive(serde::Deserialize)]
pub struct DownloadLinkRequest {
    path: String,
}

async fn api_get_download_link(
    state: AxumState<SharedState>,
    Json(body): Json<DownloadLinkRequest>,
) -> impl IntoResponse {
    let config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    // Directory traversal check
    let _ = std::fs::create_dir_all(&config.settings.save_path);
    let save_path = match std::fs::canonicalize(&config.settings.save_path) {
        Ok(p) => p,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to canonicalize save path: {}", e))),
    };
    let file_path = std::path::Path::new(&body.path);
    let target_path = match std::fs::canonicalize(file_path) {
        Ok(p) => p,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Invalid file path: {}", e))),
    };
    if !target_path.starts_with(&save_path) || !target_path.is_file() {
        return Err((StatusCode::FORBIDDEN, "Access denied: directory traversal blocked".to_string()));
    }

    let current_time = chrono::Utc::now().timestamp();
    let expires = current_time + 86400; // 24 hours

    let api_token = config.settings.api_token.as_deref().unwrap_or("default_salt");
    let sign_data = format!("{}{}{}", body.path, expires, api_token);
    let sig = format!("{:x}", md5::compute(sign_data));

    let relative_url = format!(
        "/api/video/download?path={}&expires={}&sig={}",
        urlencoding::encode(&body.path),
        expires,
        sig
    );

    Ok::<_, (StatusCode, String)>(Json(serde_json::json!({ "url": relative_url })))
}

#[derive(serde::Deserialize)]
pub struct DownloadVideoQuery {
    path: String,
    expires: i64,
    sig: String,
}

async fn api_download_video(
    state: AxumState<SharedState>,
    axum::extract::Query(query): axum::extract::Query<DownloadVideoQuery>,
) -> impl IntoResponse {
    let config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // 1. Verify expiration
    let current_time = chrono::Utc::now().timestamp();
    if current_time > query.expires {
        return (StatusCode::GONE, "Download link has expired".to_string()).into_response();
    }

    // 2. Verify signature
    let api_token = config.settings.api_token.as_deref().unwrap_or("default_salt");
    let sign_data = format!("{}{}{}", query.path, query.expires, api_token);
    let expected_sig = format!("{:x}", md5::compute(sign_data));
    if query.sig != expected_sig {
        return (StatusCode::UNAUTHORIZED, "Invalid signature".to_string()).into_response();
    }

    // 3. Verify directory traversal
    let _ = std::fs::create_dir_all(&config.settings.save_path);
    let save_path = match std::fs::canonicalize(&config.settings.save_path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to canonicalize save path: {}", e)).into_response(),
    };
    let file_path = std::path::Path::new(&query.path);
    let target_path = match std::fs::canonicalize(file_path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::NOT_FOUND, format!("File not found: {}", e)).into_response(),
    };
    if !target_path.starts_with(&save_path) || !target_path.is_file() {
        return (StatusCode::FORBIDDEN, "Access denied: directory traversal blocked".to_string()).into_response();
    }

    // 4. Stream the file
    let file = match tokio::fs::File::open(&target_path).await {
        Ok(f) => f,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to open file: {}", e)).into_response(),
    };

    let filename = target_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download.ts")
        .to_string();

    let stream = tokio_util::io::ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        axum::http::HeaderValue::from_static("*"),
    );

    let content_disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        filename,
        urlencoding::encode(&filename)
    );
    if let Ok(value) = axum::http::HeaderValue::from_str(&content_disposition) {
        headers.insert(axum::http::header::CONTENT_DISPOSITION, value);
    }

    (headers, body).into_response()
}

#[derive(serde::Deserialize)]
pub struct DeleteVideoRequest {
    path: String,
}

async fn api_delete_video(
    state: AxumState<SharedState>,
    Json(body): Json<DeleteVideoRequest>,
) -> impl IntoResponse {
    let config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    // Directory traversal security check
    let _ = std::fs::create_dir_all(&config.settings.save_path);
    let save_path = match std::fs::canonicalize(&config.settings.save_path) {
        Ok(p) => p,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to canonicalize save path: {}", e))),
    };
    let file_path = std::path::Path::new(&body.path);
    let target_path = match std::fs::canonicalize(file_path) {
        Ok(p) => p,
        Err(e) => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", e))),
    };

    if !target_path.starts_with(&save_path) || !target_path.is_file() {
        return Err((StatusCode::FORBIDDEN, "Access denied: directory traversal blocked".to_string()));
    }

    std::fs::remove_file(&target_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to delete file: {}", e)))?;

    Ok::<_, (StatusCode, String)>(Json(serde_json::json!({ "ok": true })))
}

fn bind_listener(port: u16) -> Result<std::net::TcpListener, Box<dyn std::error::Error + Send + Sync>> {
    use socket2::{Socket, Domain, Type, Protocol};
    use std::net::SocketAddr;

    // 1. Try to bind to IPv6 wildcard [::] with only_v6=false (dual-stack)
    let ipv6_addr: SocketAddr = format!("[::]:{}", port).parse()?;
    let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP));
    
    if let Ok(sock) = socket {
        let _ = sock.set_reuse_address(true);
        if sock.set_only_v6(false).is_ok() && sock.bind(&ipv6_addr.into()).is_ok() && sock.listen(1024).is_ok() {
            info!("Successfully bound dual-stack listener to [::]:{}", port);
            return Ok(sock.into());
        }
    }

    // 2. Fallback to IPv4 wildcard 0.0.0.0
    let ipv4_addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    let _ = socket.set_reuse_address(true);
    socket.bind(&ipv4_addr.into())?;
    socket.listen(1024)?;
    info!("IPv6 dual-stack bind failed, fell back to IPv4 wildcard 0.0.0.0:{}", port);
    Ok(socket.into())
}

#[derive(serde::Deserialize)]
pub struct ProxyQuery {
    pub url: String,
    pub referer: Option<String>,
    pub token: Option<String>,
}

async fn api_proxy(
    axum::extract::Query(query): axum::extract::Query<ProxyQuery>,
) -> impl IntoResponse {
    let mut target_url = query.url;
    let custom_referer = query.referer;

    if target_url.starts_with("http://") && (
        target_url.contains("douyincdn") || target_url.contains("bytecdn") ||
        target_url.contains("amemv") || target_url.contains("iesdouyin") ||
        target_url.contains("pstatp") || target_url.contains("bilivideo")
    ) {
        target_url = target_url.replace("http://", "https://");
    }

    let referer = if let Some(ref r) = custom_referer {
        r.clone()
    } else if target_url.contains("bilivideo") || target_url.contains("bilibili") {
        "https://live.bilibili.com/".to_string()
    } else if target_url.contains("douyin") || target_url.contains("douyincdn") || target_url.contains("bytecdn") || target_url.contains("amemv") || target_url.contains("iesdouyin") || target_url.contains("pstatp") {
        "https://live.douyin.com/".to_string()
    } else {
        "".to_string()
    };

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .pool_max_idle_per_host(10)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let mut req = client.get(&target_url)
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .header(reqwest::header::ACCEPT, "*/*")
        .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8");

    if !referer.is_empty() {
        let sanitized = referer.chars().filter(|c| c.is_ascii() && !c.is_ascii_control()).collect::<String>();
        let trimmed = sanitized.trim();
        if let Ok(val) = reqwest::header::HeaderValue::from_str(trimmed) {
            req = req.header(reqwest::header::REFERER, val);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let content_type = resp.headers()
                .get("content-type")
                .map(|v| v.to_str().unwrap_or("application/octet-stream"))
                .unwrap_or("application/octet-stream")
                .to_string();

            let is_m3u8 = content_type.contains("mpegurl") 
                || content_type.contains("m3u8") 
                || target_url.ends_with(".m3u8") 
                || target_url.contains(".m3u8?");

            if is_m3u8 {
                let body = resp.text().await.unwrap_or_default();
                let base_parsed = url::Url::parse(&target_url).ok();
                let base_fallback = if let Some(pos) = target_url.rfind('/') {
                    &target_url[..pos + 1]
                } else {
                    &target_url
                };

                let mut result = String::new();
                for line in body.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        result.push_str(trimmed);
                    } else {
                        let full_url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                            trimmed.to_string()
                        } else if let Some(ref base_u) = base_parsed {
                            base_u.join(trimmed).map(|u| u.to_string()).unwrap_or_else(|_| format!("{}{}", base_fallback, trimmed))
                        } else {
                            format!("{}{}", base_fallback, trimmed)
                        };
                        let encoded = urlencoding::encode(&full_url);
                        let mut proxy_path = format!("/proxy?url={}", encoded);
                        if let Some(ref ref_val) = custom_referer {
                            proxy_path.push_str(&format!("&referer={}", urlencoding::encode(ref_val)));
                        }
                        if let Some(ref tok_val) = query.token {
                            proxy_path.push_str(&format!("&token={}", urlencoding::encode(tok_val)));
                        }
                        result.push_str(&proxy_path);
                    }
                    result.push('\n');
                }

                (
                    status,
                    [
                        ("Content-Type", "application/vnd.apple.mpegurl"),
                        ("Access-Control-Allow-Origin", "*"),
                        ("Access-Control-Allow-Headers", "*"),
                    ],
                    axum::body::Body::from(result),
                ).into_response()
            } else {
                let stream = resp.bytes_stream();
                (
                    status,
                    [
                        ("Content-Type", content_type.as_str()),
                        ("Access-Control-Allow-Origin", "*"),
                        ("Access-Control-Allow-Headers", "*"),
                    ],
                    axum::body::Body::from_stream(stream),
                ).into_response()
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            [("Content-Type", "application/json")],
            axum::body::Body::from(format!("{{\"error\": \"{}\"}}", e)),
        ).into_response(),
    }
}

pub async fn start_server(state: Arc<AppState>, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cors = CorsLayer::permissive();

    let app = Router::new()
        .merge(Router::new()
            .route("/proxy", get(api_proxy))
            .route("/api/video/download", get(api_download_video))
            .route("/api/rooms", get(api_get_rooms))
            .route("/api/room", post(api_add_room))
            .route("/api/room", delete(api_delete_room))
            .route("/api/config", get(api_get_config))
            .route("/api/config", post(api_save_config))
            .route("/api/logs", get(api_get_logs))
            .route("/api/proxy-port", get(api_get_proxy_port))
            .route("/api/engine/toggle", post(api_toggle_engine))
            .route("/api/engine/status", get(api_get_engine_status))
            .route("/api/videos", get(api_get_recorded_videos))
            .route("/api/cookie", post(api_save_cookie))
            .route("/api/room/config", post(api_update_room_config))
            .route("/api/room/toggle", post(api_toggle_room_paused))
            .route("/api/command", post(api_execute_command))
            .route("/api/video/download-link", post(api_get_download_link))
            .route("/api/video", delete(api_delete_video))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        )
        .layer(cors)
        .with_state(state);

    let std_listener = bind_listener(port)?;
    std_listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(std_listener)?;
    
    info!("LiveDownloader Web API server starting on dual-stack IPv4/IPv6 port {}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
