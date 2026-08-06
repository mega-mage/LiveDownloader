use crate::config::{AppConfig, LiveUrlConfig};
use crate::platforms::{PlatformManager, PlatformConfig, LiveStatus};
use crate::engine::recorder::Recorder;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn, debug};
use std::fs;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomStatus {
    pub url: String,
    pub title: String,
    pub anchor_name: String,
    pub status: String, // "Idle", "Living", "Error", "Paused"
    pub record_path: Option<String>,
    pub live_url: Option<String>, // Direct HLS or playable URL
    pub platform: String,
    #[serde(default)]
    pub split_mode: Option<String>,
    #[serde(default)]
    pub split_custom_secs: Option<u64>,
    #[serde(default)]
    pub current_auto_duration_secs: Option<u64>,
}

pub struct TaskManager {
    config_path: PathBuf,
    config: Arc<RwLock<AppConfig>>,
    active_tasks: HashMap<String, oneshot::Sender<()>>,
    pub room_statuses: Arc<RwLock<HashMap<String, RoomStatus>>>,
    is_paused: Arc<std::sync::atomic::AtomicBool>,
}

impl TaskManager {
    pub fn new<P: AsRef<Path>>(
        config_path: P,
        is_paused: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config = AppConfig::load_or_create(config_path.as_ref())?;

        // Scan and move leftover files from downloading to download dir
        scan_and_move_leftovers(config_path.as_ref(), &config.settings.save_path);

        let initial_statuses = load_room_statuses_from_file(config_path.as_ref());

        Ok(Self {
            config_path: config_path.as_ref().to_path_buf(),
            config: Arc::new(RwLock::new(config)),
            active_tasks: HashMap::new(),
            room_statuses: Arc::new(RwLock::new(initial_statuses)),
            is_paused,
        })
    }

    pub async fn run(&mut self, notify: Arc<tokio::sync::Notify>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting LiveDownloader Task Manager...");
        
        let platform_manager = Arc::new(PlatformManager::new());
        let recorder = Arc::new(Recorder::new());
        
        let mut last_config_md5 = String::new();
        let mut was_paused = false;
        
        loop {
            // Check if engine is paused
            let paused = self.is_paused.load(std::sync::atomic::Ordering::SeqCst);
            if paused {
                was_paused = true;
                self.stop_all_tasks().await;
                
                // Wait for changes or sleep 1s
                tokio::select! {
                    _ = sleep(Duration::from_secs(1)) => {}
                    _ = notify.notified() => {
                        debug!("Engine woke up from pause by notification");
                    }
                }
                continue;
            }

            let engine_just_resumed = was_paused;
            if engine_just_resumed {
                was_paused = false;
                info!("Engine resumed from pause. Restarting monitoring tasks...");
            }
            
            // Check if we need to reload configurations
            let current_md5 = get_file_md5(&self.config_path).unwrap_or_default();
            let reload_needed = current_md5 != last_config_md5 || engine_just_resumed;
            
            if reload_needed {
                info!("Configuration file changed or loaded for the first time. Reloading...");
                last_config_md5 = current_md5;
                
                if let Ok(new_config) = AppConfig::load_from_file(&self.config_path) {
                    let mut w_config = self.config.write().await;
                    *w_config = new_config;
                }
                
                let rooms = {
                    let r_config = self.config.read().await;
                    r_config.rooms.clone()
                };
                self.sync_tasks(rooms, platform_manager.clone(), recorder.clone()).await;
            }
            
            // Re-check files every 10 seconds or when notified of changes
            tokio::select! {
                _ = sleep(Duration::from_secs(10)) => {}
                _ = notify.notified() => {
                    debug!("Config change notification received, re-checking configuration immediately");
                }
            }
        }
    }

    async fn stop_all_tasks(&mut self) {
        let mut urls_to_stop = Vec::new();
        for url in self.active_tasks.keys() {
            urls_to_stop.push(url.clone());
        }
        for url in urls_to_stop {
            if let Some(stop_tx) = self.active_tasks.remove(&url) {
                let _ = stop_tx.send(());
            }
        }
        
        // Update all rooms' status to Paused
        {
            let mut map = self.room_statuses.write().await;
            for status in map.values_mut() {
                status.status = "Paused".to_string();
                status.live_url = None;
                status.record_path = None;
            }
        }
        save_room_statuses(&self.room_statuses).await;
    }

    async fn sync_tasks(
        &mut self,
        rooms: Vec<LiveUrlConfig>,
        platform_manager: Arc<PlatformManager>,
        recorder: Arc<Recorder>,
    ) {
        let mut current_urls = HashSet::new();
        
        for url_cfg in rooms {
            if url_cfg.is_commented {
                continue;
            }
            
            current_urls.insert(url_cfg.url.clone());
            
            if !self.active_tasks.contains_key(&url_cfg.url) {
                info!("Starting new monitor task for URL: {}", url_cfg.url);
                let (stop_tx, stop_rx) = oneshot::channel::<()>();
                
                let url = url_cfg.url.clone();
                let custom_quality = url_cfg.quality.clone();
                let custom_name = url_cfg.name.clone();
                let custom_format = url_cfg.video_save_type.clone();
                let config_cloned = self.config.clone();
                let config_path_cloned = self.config_path.clone();
                let pm_cloned = platform_manager.clone();
                let rec_cloned = recorder.clone();
                let statuses_cloned = self.room_statuses.clone();
                
                // Update/insert initial state for this room
                {
                    let mut map = self.room_statuses.write().await;
                    let handler_name = pm_cloned.find_handler(&url)
                        .map_or("Unknown", |h| h.name());
                    
                    let paused = self.is_paused.load(std::sync::atomic::Ordering::SeqCst);
                    let initial_status = if paused { "Paused" } else { "Idle" };

                    if let Some(existing) = map.get_mut(&url) {
                        if existing.status == "Paused" && !paused {
                            existing.status = "Idle".to_string();
                        }
                        if existing.anchor_name.is_empty() || existing.anchor_name == "Unknown" || existing.anchor_name == "未知主播" {
                            existing.anchor_name = custom_name.clone().unwrap_or_else(|| "未知主播".to_string());
                        }
                        if existing.platform.is_empty() || existing.platform == "Unknown" {
                            existing.platform = handler_name.to_string();
                        }
                    } else {
                        map.insert(url.clone(), RoomStatus {
                            url: url.clone(),
                            title: "".to_string(),
                            anchor_name: custom_name.clone().unwrap_or_else(|| "未知主播".to_string()),
                            status: initial_status.to_string(),
                            record_path: None,
                            live_url: None,
                            platform: handler_name.to_string(),
                            split_mode: url_cfg.split_mode.clone(),
                            split_custom_secs: url_cfg.split_custom_secs,
                            current_auto_duration_secs: None,
                        });
                    }
                }
                
                tokio::spawn(async move {
                    monitor_room_loop(
                        url,
                        custom_quality,
                        custom_name,
                        custom_format,
                        config_cloned,
                        config_path_cloned,
                        pm_cloned,
                        rec_cloned,
                        statuses_cloned,
                        stop_rx,
                    ).await;
                });
                
                self.active_tasks.insert(url_cfg.url, stop_tx);
            }
        }
        
        // Stop tasks that are no longer in the URL config list
        let mut to_remove = Vec::new();
        for url in self.active_tasks.keys() {
            if !current_urls.contains(url) {
                to_remove.push(url.clone());
            }
        }
        
        for url in to_remove {
            if let Some(stop_tx) = self.active_tasks.remove(&url) {
                info!("Stopping monitor task for URL: {}", url);
                let _ = stop_tx.send(());
                
                // Remove from state
                let mut map = self.room_statuses.write().await;
                map.remove(&url);
            }
        }
        save_room_statuses(&self.room_statuses).await;
    }
}

async fn monitor_room_loop(
    url: String,
    custom_quality: Option<String>,
    custom_name: Option<String>,
    custom_format: Option<String>,
    config: Arc<RwLock<AppConfig>>,
    config_path: PathBuf,
    platform_manager: Arc<PlatformManager>,
    recorder: Arc<Recorder>,
    statuses: Arc<RwLock<HashMap<String, RoomStatus>>>,
    mut stop_rx: oneshot::Receiver<()>,
) {
    let handler = match platform_manager.find_handler(&url) {
        Some(h) => h,
        None => {
            error!("No platform handler found for URL: {}", url);
            return;
        }
    };
    
    info!("Room task started for [{}] on platform [{}]", url, handler.name());
    
    loop {
        // Check cancellation
        if stop_rx.try_recv().is_ok() {
            info!("Cancellation signal received. Exiting task loop for [{}]", url);
            break;
        }
        
        // Retrieve current configuration
        let (delay_secs, pc) = {
            let r_config = config.read().await;
            let platform_cookie = r_config.cookies.get(handler.id())
                .cloned()
                .or_else(|| r_config.cookies.get("douyin").cloned())
                .or_else(|| r_config.cookies.get("抖音").cloned())
                .or_else(|| r_config.cookies.get("抖音cookie").cloned())
                .or_else(|| r_config.cookies.get("douyin_cookie").cloned())
                .or_else(|| {
                    let key = match handler.id() {
                        "douyin" => "抖音cookie",
                        "bilibili" => "b站cookie",
                        "huya" => "虎牙cookie",
                        "kuaishou" => "快手cookie",
                        "douyu" => "斗鱼cookie",
                        "maoerfm" => "猫耳cookie",
                        "netease_cc" => "网易cccookie",
                        "weibo" => "微博cookie",
                        "taobao" => "淘宝cookie",
                        "acfun" => "A站cookie",
                        "twitch" => "Twitchcookie",
                        _ => "",
                    };
                    r_config.cookies.get(key).cloned()
                });
            let extra = HashMap::new();
            let proxy_to_use = if r_config.settings.use_proxy {
                r_config.settings.proxy_addr.clone()
            } else {
                None
            };
            let pc = PlatformConfig {
                cookie: platform_cookie,
                proxy: proxy_to_use,
                quality: custom_quality.clone().unwrap_or_else(|| r_config.settings.video_record_quality.clone()),
                extra,
            };
            (r_config.settings.delay_default, pc)
        };
        
        match handler.fetch_status(&url, &pc).await {
            Ok(LiveStatus::Living { title, anchor_name, stream_urls }) => {
                let display_name = custom_name.as_deref().unwrap_or(&anchor_name);
                info!("Anchor [{}] is LIVING: '{}'", display_name, title);
                
                let app_config = {
                    let r = config.read().await;
                    r.clone()
                };
                
                // Send online notification
                let notifier = crate::engine::notifier::Notifier::new();
                let push_title = format!("{} 开播啦！", display_name);
                let push_content = format!(
                    "主播: {}\n标题: {}\n平台: {}\n时间: {}",
                    display_name,
                    title,
                    handler.name(),
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                );
                notifier.notify(&push_title, &push_content, &app_config).await;
                
                let (room_split_mode, room_split_custom_secs, current_auto_duration) = {
                    let r = config.read().await;
                    let room_cfg = r.rooms.iter().find(|rm| rm.url == url);
                    let split_m = room_cfg.and_then(|rm| rm.split_mode.clone());
                    let split_c = room_cfg.and_then(|rm| rm.split_custom_secs);

                    let st_map = statuses.read().await;
                    let auto_dur = st_map.get(&url).and_then(|st| st.current_auto_duration_secs);
                    (split_m, split_c, auto_dur)
                };

                // Update shared status state
                {
                    let mut map = statuses.write().await;
                    map.insert(url.clone(), RoomStatus {
                        url: url.clone(),
                        title: title.clone(),
                        anchor_name: display_name.to_string(),
                        status: "Living".to_string(),
                        record_path: Some(format!("./downloads/{}/...", display_name)),
                        live_url: stream_urls.m3u8_url.clone().or_else(|| {
                            let rec = stream_urls.record_url.clone();
                            if rec.contains(".flv") {
                                Some(rec.replace("pull-flv-", "pull-hls-").replace(".flv?", ".m3u8?").replace(".flv", ".m3u8"))
                            } else {
                                Some(rec)
                            }
                        }),
                        platform: handler.name().to_string(),
                        split_mode: room_split_mode.clone(),
                        split_custom_secs: room_split_custom_secs,
                        current_auto_duration_secs: current_auto_duration,
                    });
                }
                save_room_statuses(&statuses).await;
                
                // Start record session
                match recorder.start_record(
                    display_name,
                    &title,
                    &stream_urls,
                    &app_config,
                    &config_path,
                    custom_format.as_deref(),
                    room_split_mode.as_deref(),
                    room_split_custom_secs,
                    current_auto_duration,
                ).await {
                    Ok(mut session) => {
                        info!("Recording started for [{}], output file: {:?}", display_name, session.output_file_path);
                        
                        {
                            let mut map = statuses.write().await;
                            if let Some(room) = map.get_mut(&url) {
                                room.record_path = Some(session.output_file_path.to_string_lossy().to_string());
                            }
                        }
                        save_room_statuses(&statuses).await;

                        // Segment monitoring task for real-time file moving and Telegram auto-upload
                        let output_template = session.output_file_path.clone();
                        let target_dir_path = session.target_dir_path.clone();
                        let app_config_cloned = app_config.clone();
                        let display_name_str = display_name.to_string();
                        let notifier_cloned = crate::engine::notifier::Notifier::new();
                        let url_str = url.clone();
                        let statuses_cloned = statuses.clone();
                        let room_split_mode_str = room_split_mode.clone().unwrap_or_else(|| "auto".to_string());
                        
                        let (poll_stop_tx, mut poll_stop_rx) = tokio::sync::watch::channel(false);
                        
                        let segment_handle = tokio::spawn(async move {
                            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
                            let mut processed_files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
                            
                            loop {
                                tokio::select! {
                                    _ = interval.tick() => {
                                        let completed = find_completed_segments(&output_template, true);
                                        for file_path in completed {
                                            if processed_files.contains(&file_path) {
                                                continue;
                                            }
                                            processed_files.insert(file_path.clone());

                                            if app_config_cloned.push.tg_auto_upload {
                                                let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                                let caption = format!("【自动上传切片】\n主播: {}\n文件: {}", display_name_str, file_name);
                                                if let Err(e) = notifier_cloned.upload_file_to_telegram(&file_path, &caption, &app_config_cloned).await {
                                                    error!("Failed to upload segment {:?} to Telegram: {}", file_path, e);
                                                }
                                            }
                                            if let Some(filename) = file_path.file_name() {
                                                let _ = std::fs::create_dir_all(&target_dir_path);
                                                let dest = target_dir_path.join(filename);
                                                let actual_bytes = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
                                                info!("Real-time segment move: Moving completed segment from downloading to final dir: {:?}", dest);
                                                if let Err(e) = std::fs::rename(&file_path, &dest) {
                                                    debug!("Rename failed for segment, falling back to copy/remove: {}", e);
                                                    if let Err(err) = std::fs::copy(&file_path, &dest).and_then(|_| std::fs::remove_file(&file_path)) {
                                                        error!("Failed to move completed segment {:?} to {:?}: {}", file_path, dest, err);
                                                    }
                                                }

                                                // Dynamic Auto-Adjustment Logic for "auto" split mode
                                                if room_split_mode_str.to_lowercase() == "auto" {
                                                    let actual_mb = actual_bytes as f64 / (1024.0 * 1024.0);
                                                    let target_mb = app_config_cloned.settings.split_size_mb.max(10) as f64;
                                                    let target_kbps = app_config_cloned.settings.split_video_bitrate_kbps.max(500) as f64;
                                                    let calculated_secs = ((target_mb * 1024.0 * 8.0) / target_kbps).round() as u64;
                                                    let initial_default = calculated_secs.clamp(180, 14400);
                                                    
                                                    if actual_mb > 1.0 {
                                                        let mut map = statuses_cloned.write().await;
                                                        if let Some(room) = map.get_mut(&url_str) {
                                                            let current_secs = room.current_auto_duration_secs.unwrap_or(initial_default);
                                                            let is_in_target_range = actual_mb >= target_mb * 0.90 && actual_mb <= target_mb * 1.10;
                                                            
                                                            if is_in_target_range {
                                                                info!("Auto split calculation for [{}]: Segment size {:.2} MB is within target ({:.0} MB ±10%). Duration remains {}s.", display_name_str, actual_mb, target_mb, current_secs);
                                                            } else {
                                                                let new_secs = ((current_secs as f64) * (target_mb / actual_mb)).round() as u64;
                                                                let clamped_secs = new_secs.clamp(180, 10800); // 3 mins to 3 hours
                                                                info!("Auto split calculation for [{}]: 1st segment was {:.2} MB in {}s. Adjusting next segment duration to {}s (~{:.1} mins) to target {:.0} MB.", display_name_str, actual_mb, current_secs, clamped_secs, clamped_secs as f64 / 60.0, target_mb);
                                                                room.current_auto_duration_secs = Some(clamped_secs);
                                                            }
                                                        }
                                                        save_room_statuses(&statuses_cloned).await;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    _ = poll_stop_rx.changed() => {
                                        if *poll_stop_rx.borrow() {
                                            break;
                                        }
                                    }
                                }
                            }
                            
                            // One final check after FFmpeg exits
                            let completed = find_completed_segments(&output_template, false);
                            for file_path in completed {
                                if processed_files.contains(&file_path) {
                                    continue;
                                }
                                processed_files.insert(file_path.clone());

                                if app_config_cloned.push.tg_auto_upload {
                                    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                    let caption = format!("【自动上传切片】\n主播: {}\n文件: {}", display_name_str, file_name);
                                    if let Err(e) = notifier_cloned.upload_file_to_telegram(&file_path, &caption, &app_config_cloned).await {
                                        error!("Failed to upload final segment {:?} to Telegram: {}", file_path, e);
                                    }
                                }
                                if let Some(filename) = file_path.file_name() {
                                    let _ = std::fs::create_dir_all(&target_dir_path);
                                    let dest = target_dir_path.join(filename);
                                    info!("Finalizing download: Moving final segment from downloading to final dir: {:?}", dest);
                                    if let Err(e) = std::fs::rename(&file_path, &dest) {
                                        debug!("Rename failed for final segment, falling back to copy/remove: {}", e);
                                        if let Err(err) = std::fs::copy(&file_path, &dest).and_then(|_| std::fs::remove_file(&file_path)) {
                                            error!("Failed to move final segment {:?} to {:?}: {}", file_path, dest, err);
                                        }
                                    }
                                }
                            }
                        });

                        let mut should_stop_loop = false;

                        tokio::select! {
                            res = session.wait_for_completion() => {
                                match res {
                                    Ok(status) => {
                                        info!("Recording finished for [{}] with status: {:?}", display_name, status);
                                    }
                                    Err(e) => {
                                        error!("Error during recording for [{}]: {}", display_name, e);
                                    }
                                }
                            }
                            _ = &mut stop_rx => {
                                info!("Stop signal received during recording of [{}]. Terminating recorder...", display_name);
                                session.stop().await;
                                should_stop_loop = true;
                            }
                        }

                        // Stop the segment monitoring loop and wait for final uploads/moves
                        let _ = poll_stop_tx.send(true);
                        let _ = segment_handle.await;
                        // Note: segment_handle's final check already moves all remaining files
                        // from downloading dir to target dir, so no additional move needed.
                        
                        // Send offline notification
                        let push_title = format!("{} 直播已录制结束/停止", display_name);
                        let push_content = format!(
                            "主播: {}\n平台: {}\n时间: {}",
                            display_name,
                            handler.name(),
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                        );
                        notifier.notify(&push_title, &push_content, &app_config).await;

                        if should_stop_loop {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to start recording for [{}]: {}", display_name, e);
                    }
                }

                // Recording stopped, update state back to Idle
                {
                    let mut map = statuses.write().await;
                    if let Some(room) = map.get_mut(&url) {
                        room.status = "Idle".to_string();
                        room.record_path = None;
                        room.live_url = None;
                    } else {
                        map.insert(url.clone(), RoomStatus {
                            url: url.clone(),
                            title: "".to_string(),
                            anchor_name: display_name.to_string(),
                            status: "Idle".to_string(),
                            record_path: None,
                            live_url: None,
                            platform: handler.name().to_string(),
                            split_mode: room_split_mode,
                            split_custom_secs: room_split_custom_secs,
                            current_auto_duration_secs: current_auto_duration,
                        });
                    }
                }
                save_room_statuses(&statuses).await;
            }
            Ok(LiveStatus::Idle) => {
                debug!("Room [{}] is currently offline/idle.", url);
                {
                    let mut map = statuses.write().await;
                    if let Some(room) = map.get_mut(&url) {
                        room.status = "Idle".to_string();
                        room.record_path = None;
                        room.live_url = None;
                    }
                }
                save_room_statuses(&statuses).await;
            }
            Ok(LiveStatus::Error(e)) => {
                warn!("Error fetching status for [{}]: {}", url, e);
                {
                    let mut map = statuses.write().await;
                    if let Some(room) = map.get_mut(&url) {
                        room.status = "Error".to_string();
                    }
                }
                save_room_statuses(&statuses).await;
            }
            Err(e) => {
                error!("Network/API error fetching status for [{}]: {}", url, e);
                {
                    let mut map = statuses.write().await;
                    if let Some(room) = map.get_mut(&url) {
                        room.status = "Error".to_string();
                    }
                }
                save_room_statuses(&statuses).await;
            }
        }
        
        tokio::select! {
            _ = sleep(Duration::from_secs(delay_secs)) => {}
            _ = &mut stop_rx => {
                info!("Stop signal received during poll interval for [{}]. Exiting.", url);
                break;
            }
        }
    }
}

async fn save_room_statuses(statuses: &Arc<RwLock<HashMap<String, RoomStatus>>>) {
    let (config_path, _) = crate::config::get_config_paths();
    if let Some(parent) = config_path.parent() {
        let status_path = parent.join("statuses.json");
        let tmp_status_path = parent.join("statuses.json.tmp");
        let statuses_map = statuses.read().await;
        if let Ok(json_str) = serde_json::to_string_pretty(&*statuses_map) {
            if fs::write(&tmp_status_path, json_str).is_ok() {
                let _ = fs::rename(&tmp_status_path, &status_path);
            }
        }
    }
}

fn load_room_statuses_from_file(config_path: &Path) -> HashMap<String, RoomStatus> {
    if let Some(parent) = config_path.parent() {
        let status_path = parent.join("statuses.json");
        if status_path.exists() {
            if let Ok(content) = fs::read_to_string(&status_path) {
                if let Ok(mut map) = serde_json::from_str::<HashMap<String, RoomStatus>>(&content) {
                    for status in map.values_mut() {
                        if status.status == "Living" {
                            status.status = "Idle".to_string();
                            status.live_url = None;
                            status.record_path = None;
                        }
                        if let Some(ref mut live_u) = status.live_url {
                            if live_u.contains("pull-flv-") || live_u.contains(".flv") {
                                *live_u = live_u
                                    .replace("pull-flv-", "pull-hls-")
                                    .replace(".flv?", ".m3u8?")
                                    .replace(".flv", ".m3u8");
                            }
                        }
                    }
                    return map;
                }
            }
        }
    }
    HashMap::new()
}

fn get_file_md5<P: AsRef<Path>>(path: P) -> Result<String, std::io::Error> {
    let content = fs::read(path)?;
    let digest = md5::compute(content);
    Ok(format!("{:x}", digest))
}

fn find_completed_segments(
    output_template: &Path,
    is_ffmpeg_active: bool,
) -> Vec<PathBuf> {
    let parent_dir = match output_template.parent() {
        Some(p) => p,
        None => return Vec::new(),
    };
    
    let file_name_template = match output_template.file_name().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return Vec::new(),
    };
    
    let parts: Vec<&str> = file_name_template.split("%03d").collect();
    if parts.len() < 2 {
        if !is_ffmpeg_active && output_template.exists() {
            return vec![output_template.to_path_buf()];
        }
        return Vec::new();
    }
    
    let prefix = parts[0];
    let suffix = parts[1];
    
    let mut files = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(parent_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with(prefix) && name.ends_with(suffix) {
                        let seq_str = &name[prefix.len()..(name.len() - suffix.len())];
                        if let Ok(seq) = seq_str.parse::<u32>() {
                            files.push((seq, path));
                        }
                    }
                }
            }
        }
    }
    
    files.sort_by_key(|(seq, _)| *seq);
    
    let mut completed = Vec::new();
    if !files.is_empty() {
        if is_ffmpeg_active {
            for i in 0..(files.len() - 1) {
                completed.push(files[i].1.clone());
            }
        } else {
            for item in files {
                completed.push(item.1);
            }
        }
    }
    
    completed
}



pub fn scan_and_move_leftovers(config_toml_path: &Path, save_path: &Path) {
    let downloading_dir = crate::config::get_downloading_dir(config_toml_path);
    if !downloading_dir.exists() {
        return;
    }

    if let Err(e) = std::fs::create_dir_all(save_path) {
        debug!("Failed to ensure save_path directory exists: {}", e);
        return;
    }

    if let Ok(entries) = std::fs::read_dir(&downloading_dir) {
        for entry in entries.flatten() {
            let src = entry.path();
            if src.is_file() {
                // Check if file is currently open/locked by FFmpeg or another process
                if std::fs::OpenOptions::new().write(true).open(&src).is_err() {
                    debug!("Startup cleaner: File {:?} is currently in use by another process, skipping for now", src);
                    continue;
                }

                if let Some(name) = entry.file_name().to_str() {
                    let dest = save_path.join(name);
                    info!("Startup cleaner: Moving leftover file from downloading to download dir: {:?}", dest);
                    if let Err(e) = std::fs::rename(&src, &dest) {
                        debug!("Rename failed for leftover file, falling back to copy/remove: {}", e);
                        if let Err(err) = std::fs::copy(&src, &dest).and_then(|_| std::fs::remove_file(&src)) {
                            warn!("Startup cleaner: Could not move leftover file {:?} to {:?}: {}", src, dest, err);
                        }
                    }
                }
            }
        }
    }
}
