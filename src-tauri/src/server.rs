use crate::AppState;
use crate::config::AppConfig;
use crate::engine::manager::RoomStatus;
use crate::RecordedVideo;

use axum::{
    Router,
    routing::{get, post, put, delete},
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
        let expected_trimmed = expected_token.trim();
        if !expected_trimmed.is_empty() {
            let auth_header = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let mut valid = false;

            if !auth_header.is_empty() {
                let token_from_header = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header).trim();
                if token_from_header == expected_trimmed {
                    valid = true;
                }
            }

            if !valid {
                let query_str = request.uri().query().unwrap_or("");
                let token_from_query = parse_query_param_simple(query_str, "token")
                    .or_else(|| parse_query_param_simple(query_str, "api_token"))
                    .unwrap_or_default();
                if token_from_query.trim() == expected_trimmed {
                    valid = true;
                }
            }

            if !valid {
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }

    Ok(next.run(request).await)
}

async fn api_get_rooms(state: AxumState<SharedState>) -> impl IntoResponse {
    let map = state.room_statuses.read().await;
    let mut result = Vec::new();

    if let Ok(config) = AppConfig::load_or_create(&state.config_toml_path) {
        if !config.rooms.is_empty() || map.is_empty() {
            for r in config.rooms {
                if r.is_commented {
                    let (anchor, title, platform, auto_dur) = if let Some(existing) = map.get(&r.url) {
                        (
                            if existing.anchor_name.is_empty() || existing.anchor_name == "未知主播" {
                                r.name.clone().unwrap_or_else(|| "未知主播".to_string())
                            } else {
                                existing.anchor_name.clone()
                            },
                            existing.title.clone(),
                            existing.platform.clone(),
                            existing.current_auto_duration_secs,
                        )
                    } else {
                        (
                            r.name.clone().unwrap_or_else(|| "未知主播".to_string()),
                            "".to_string(),
                            "".to_string(),
                            None,
                        )
                    };

                    result.push(RoomStatus {
                        url: r.url.clone(),
                        title,
                        anchor_name: anchor,
                        status: "Paused".to_string(),
                        record_path: None,
                        live_url: None,
                        platform,
                        split_mode: r.split_mode.clone(),
                        split_custom_secs: r.split_custom_secs,
                        current_auto_duration_secs: auto_dur,
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
            return Json(result);
        }
    }

    // Fallback: If config file read fails or returned 0 rooms while map has items, return memory statuses!
    for status in map.values() {
        result.push(status.clone());
    }
    Json(result)
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

#[derive(serde::Deserialize, Default)]
pub struct DeleteRoomQuery {
    pub url: Option<String>,
}

async fn api_delete_room(
    state: AxumState<SharedState>,
    axum::extract::Query(query): axum::extract::Query<DeleteRoomQuery>,
    body_bytes: axum::body::Bytes,
) -> impl IntoResponse {
    let target_url = if let Some(u) = query.url.filter(|s| !s.is_empty()) {
        u
    } else if !body_bytes.is_empty() {
        if let Ok(parsed) = serde_json::from_slice::<DeleteRoomQuery>(&body_bytes) {
            match parsed.url.filter(|s| !s.is_empty()) {
                Some(u) => u,
                None => return Err((StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string())),
            }
        } else {
            return Err((StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()));
        }
    } else {
        return Err((StatusCode::BAD_REQUEST, "Missing 'url' parameter".to_string()));
    };

    let mut config = match AppConfig::load_or_create(&state.config_toml_path) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    config.rooms.retain(|r| r.url != target_url);
    config.save_to_file(&state.config_toml_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    {
        let mut map = state.room_statuses.write().await;
        map.remove(&target_url);
    }
    state.change_notify.notify_one();
    Ok::<_, (StatusCode, String)>(Json(serde_json::json!({ "ok": true })))
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

    {
        let mut map = state.room_statuses.write().await;
        if body.paused {
            for status in map.values_mut() {
                status.status = "Paused".to_string();
                status.live_url = None;
                status.record_path = None;
            }
        } else {
            for status in map.values_mut() {
                if status.status == "Paused" {
                    status.status = "Idle".to_string();
                }
            }
        }
    }

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
    {
        let mut map = state.room_statuses.write().await;
        if body.paused {
            if let Some(status) = map.get_mut(&body.url) {
                status.status = "Paused".to_string();
                status.live_url = None;
                status.record_path = None;
            }
        } else if let Some(status) = map.get_mut(&body.url) {
            if status.status == "Paused" {
                status.status = "Idle".to_string();
            }
        }
    }
    state.change_notify.notify_one();
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
pub struct ExecuteCommandRequest {
    cmd: String,
}

async fn api_execute_command(
    state: AxumState<SharedState>,
    Json(body): Json<ExecuteCommandRequest>,
) -> impl IntoResponse {
    let output = crate::cli::execute_cli_str(&state.config_toml_path, &body.cmd);
    Json(serde_json::json!({ "output": output }))
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

#[derive(serde::Deserialize, Default)]
pub struct DeleteVideoQuery {
    pub path: Option<String>,
}

async fn api_delete_video(
    state: AxumState<SharedState>,
    axum::extract::Query(query): axum::extract::Query<DeleteVideoQuery>,
    body_bytes: axum::body::Bytes,
) -> impl IntoResponse {
    let target_path_str = if let Some(p) = query.path.filter(|s| !s.is_empty()) {
        p
    } else if !body_bytes.is_empty() {
        if let Ok(parsed) = serde_json::from_slice::<DeleteVideoQuery>(&body_bytes) {
            match parsed.path.filter(|s| !s.is_empty()) {
                Some(p) => p,
                None => return Err((StatusCode::BAD_REQUEST, "Missing 'path' parameter".to_string())),
            }
        } else {
            return Err((StatusCode::BAD_REQUEST, "Missing 'path' parameter".to_string()));
        }
    } else {
        return Err((StatusCode::BAD_REQUEST, "Missing 'path' parameter".to_string()));
    };

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
    let file_path = std::path::Path::new(&target_path_str);
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
    if let Ok(ipv6_addr) = format!("[::]:{}", port).parse::<SocketAddr>() {
        if let Ok(sock) = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP)) {
            let _ = sock.set_reuse_address(true);
            if sock.set_only_v6(false).is_ok() && sock.bind(&ipv6_addr.into()).is_ok() && sock.listen(1024).is_ok() {
                info!("Successfully bound dual-stack listener to [::]:{}", port);
                return Ok(sock.into());
            }
        }
    }

    // 2. Fallback to IPv4 wildcard 0.0.0.0
    if let Ok(ipv4_addr) = format!("0.0.0.0:{}", port).parse::<SocketAddr>() {
        if let Ok(socket) = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)) {
            let _ = socket.set_reuse_address(true);
            if socket.bind(&ipv4_addr.into()).is_ok() && socket.listen(1024).is_ok() {
                info!("IPv6 dual-stack bind failed, fell back to IPv4 wildcard 0.0.0.0:{}", port);
                return Ok(socket.into());
            }
        }
    }

    // 3. Fallback to IPv4 localhost 127.0.0.1
    let local_addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    let _ = socket.set_reuse_address(true);
    if let Err(e) = socket.bind(&local_addr.into()) {
        eprintln!("\n[错误] 端口 {} 绑定失败 ({})。", port, e);
        eprintln!("[原因] 端口 {} 属于 Windows 系统 Hyper-V / WSL2 动态保留排除端口段，或已被其他程序占用。", port);
        eprintln!("[解决方法] 请换用其他未被保留的端口重新运行，例如:");
        eprintln!("  cargo run --no-default-features --features server -- --port 10830\n");
        return Err(Box::new(e));
    }
    socket.listen(1024)?;
    info!("Wildcard binds failed, fell back to IPv4 localhost 127.0.0.1:{}", port);
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

    if target_url.contains("pull-flv-") || target_url.contains(".flv") {
        target_url = target_url
            .replace("http://", "https://")
            .replace("pull-flv-", "pull-hls-")
            .replace(".flv?", ".m3u8?")
            .replace(".flv", ".m3u8");
    }

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
        .http1_ignore_invalid_headers_in_responses(true)
        .http1_allow_obsolete_multiline_headers_in_responses(true)
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
            // --- Standard REST API Endpoints ---
            .route("/api/rooms", get(api_get_rooms).post(api_add_room).delete(api_delete_room))
            .route("/api/rooms/config", put(api_update_room_config))
            .route("/api/rooms/toggle", put(api_toggle_room_paused))
            .route("/api/config", get(api_get_config).put(api_save_config).post(api_save_config))
            .route("/api/cookies", post(api_save_cookie))
            .route("/api/engine/status", get(api_get_engine_status).put(api_toggle_engine))
            .route("/api/videos", get(api_get_recorded_videos).delete(api_delete_video))
            .route("/api/videos/download-link", post(api_get_download_link))
            .route("/api/videos/download", get(api_download_video))
            .route("/api/commands/execute", post(api_execute_command))
            .route("/api/logs", get(api_get_logs))
            .route("/api/proxy-port", get(api_get_proxy_port))

            // --- Legacy Backward-Compatibility Aliases ---
            .route("/api/room", post(api_add_room).delete(api_delete_room))
            .route("/api/room/config", post(api_update_room_config))
            .route("/api/room/toggle", post(api_toggle_room_paused))
            .route("/api/cookie", post(api_save_cookie))
            .route("/api/engine/toggle", post(api_toggle_engine))
            .route("/api/video", delete(api_delete_video))
            .route("/api/video/download-link", post(api_get_download_link))
            .route("/api/video/download", get(api_download_video))
            .route("/api/command", post(api_execute_command))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::{RwLock, Notify};
    use std::sync::atomic::AtomicBool;

    fn create_test_state() -> Arc<AppState> {
        let temp_dir = std::env::temp_dir().join(format!("ld_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let _ = std::fs::create_dir_all(&temp_dir);
        let config_toml_path = temp_dir.join("config.toml");
        
        let mut config = AppConfig::default();
        config.settings.save_path = temp_dir.join("downloads");
        let _ = config.save_to_file(&config_toml_path);

        Arc::new(AppState {
            room_statuses: Arc::new(RwLock::new(HashMap::new())),
            config_toml_path,
            is_paused: Arc::new(AtomicBool::new(false)),
            change_notify: Arc::new(Notify::new()),
            proxy_port: 10731,
        })
    }

    #[tokio::test]
    async fn test_rest_rooms_api() {
        let state = create_test_state();

        // 1. Add Room (POST /api/rooms)
        let req = AddRoomRequest {
            url: "https://live.bilibili.com/123456".to_string(),
            name: Some("TestAnchor".to_string()),
            quality: Some("原画".to_string()),
            split_mode: None,
            split_custom_secs: None,
        };
        let res = api_add_room(AxumState(state.clone()), Json(req)).await.into_response();
        assert_eq!(res.status(), StatusCode::OK);

        // 2. Get Rooms (GET /api/rooms)
        let rooms_res = api_get_rooms(AxumState(state.clone())).await.into_response();
        assert_eq!(rooms_res.status(), StatusCode::OK);

        // 3. Delete Room via Query (DELETE /api/rooms?url=...)
        let del_res = api_delete_room(
            AxumState(state.clone()),
            axum::extract::Query(DeleteRoomQuery {
                url: Some("https://live.bilibili.com/123456".to_string()),
            }),
            axum::body::Bytes::new(),
        ).await.into_response();
        assert_eq!(del_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rest_config_and_engine_api() {
        let state = create_test_state();

        // Get Config
        let cfg_res = api_get_config(AxumState(state.clone())).await.into_response();
        assert_eq!(cfg_res.status(), StatusCode::OK);

        // Toggle Engine Status (PUT /api/engine/status)
        let toggle_res = api_toggle_engine(
            AxumState(state.clone()),
            Json(ToggleEngineRequest { paused: true }),
        ).await.into_response();
        assert_eq!(toggle_res.status(), StatusCode::OK);

        let status_res = api_get_engine_status(AxumState(state.clone())).await.into_response();
        assert_eq!(status_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rest_cookie_api() {
        let state = create_test_state();
        let cookie_res = api_save_cookie(
            AxumState(state.clone()),
            Json(SaveCookieRequest {
                platform: "bilibili".to_string(),
                value: "SESSDATA=test_value".to_string(),
            }),
        ).await.into_response();
        assert_eq!(cookie_res.status(), StatusCode::OK);
    }
}
