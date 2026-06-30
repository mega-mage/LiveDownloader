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
        scan_and_move_leftovers(&config.settings.save_path);

        Ok(Self {
            config_path: config_path.as_ref().to_path_buf(),
            config: Arc::new(RwLock::new(config)),
            active_tasks: HashMap::new(),
            room_statuses: Arc::new(RwLock::new(HashMap::new())),
            is_paused,
        })
    }

    pub async fn run(&mut self, notify: Arc<tokio::sync::Notify>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting LiveDownloader Task Manager...");
        
        let platform_manager = Arc::new(PlatformManager::new());
        let recorder = Arc::new(Recorder::new());
        
        let mut last_config_md5 = String::new();
        
        loop {
            // Check if engine is paused
            let paused = self.is_paused.load(std::sync::atomic::Ordering::SeqCst);
            if paused {
                if !self.active_tasks.is_empty() {
                    info!("Engine is paused. Stopping all active monitoring tasks...");
                    self.stop_all_tasks().await;
                }
                
                // Wait for changes or sleep 1s
                tokio::select! {
                    _ = sleep(Duration::from_secs(1)) => {}
                    _ = notify.notified() => {
                        debug!("Engine woke up from pause by notification");
                    }
                }
                continue;
            }
            
            // Check if we need to reload configurations
            let current_md5 = get_file_md5(&self.config_path).unwrap_or_default();
            let reload_needed = current_md5 != last_config_md5;
            
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
                let pm_cloned = platform_manager.clone();
                let rec_cloned = recorder.clone();
                let statuses_cloned = self.room_statuses.clone();
                
                // Insert initial state for this new room
                {
                    let mut map = self.room_statuses.write().await;
                    let handler_name = pm_cloned.find_handler(&url)
                        .map_or("Unknown", |h| h.name());
                    
                    let paused = self.is_paused.load(std::sync::atomic::Ordering::SeqCst);
                    let initial_status = if paused { "Paused" } else { "Idle" };
                    
                    map.insert(url.clone(), RoomStatus {
                        url: url.clone(),
                        title: "".to_string(),
                        anchor_name: custom_name.clone().unwrap_or_else(|| "Unknown".to_string()),
                        status: initial_status.to_string(),
                        record_path: None,
                        live_url: None,
                        platform: handler_name.to_string(),
                    });
                }
                
                tokio::spawn(async move {
                    monitor_room_loop(
                        url,
                        custom_quality,
                        custom_name,
                        custom_format,
                        config_cloned,
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
                
                // Update shared status state
                {
                    let mut map = statuses.write().await;
                    map.insert(url.clone(), RoomStatus {
                        url: url.clone(),
                        title: title.clone(),
                        anchor_name: display_name.to_string(),
                        status: "Living".to_string(),
                        record_path: Some(format!("./downloads/{}/...", display_name)),
                        live_url: stream_urls.m3u8_url.clone().or_else(|| Some(stream_urls.record_url.clone())),
                        platform: handler.name().to_string(),
                    });
                }
                save_room_statuses(&statuses).await;
                
                // Start record session
                match recorder.start_record(display_name, &title, &stream_urls, &app_config, custom_format.as_deref()) {
                    Ok(mut session) => {
                        info!("Recording started for [{}], output file: {:?}", display_name, session.output_file_path);
                        
                        {
                            let mut map = statuses.write().await;
                            if let Some(room) = map.get_mut(&url) {
                                room.record_path = Some(session.output_file_path.to_string_lossy().to_string());
                            }
                        }
                        save_room_statuses(&statuses).await;

                        // Telegram automatic upload task for completed segments
                        let output_template = session.output_file_path.clone();
                        let app_config_cloned = app_config.clone();
                        let display_name_str = display_name.to_string();
                        let notifier_cloned = crate::engine::notifier::Notifier::new();
                        
                        let (poll_stop_tx, mut poll_stop_rx) = tokio::sync::watch::channel(false);
                        
                        let upload_handle = tokio::spawn(async move {
                            if !app_config_cloned.push.tg_auto_upload {
                                return;
                            }
                            let mut uploaded_files = std::collections::HashSet::new();
                            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
                            
                            loop {
                                tokio::select! {
                                    _ = interval.tick() => {
                                        let completed = find_completed_segments(&output_template, true);
                                        for file_path in completed {
                                            if !uploaded_files.contains(&file_path) {
                                                let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                                let caption = format!("【自动上传切片】\n主播: {}\n文件: {}", display_name_str, file_name);
                                                if let Err(e) = notifier_cloned.upload_file_to_telegram(&file_path, &caption, &app_config_cloned).await {
                                                    error!("Failed to upload segment {:?} to Telegram: {}", file_path, e);
                                                } else {
                                                    uploaded_files.insert(file_path);
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
                                if !uploaded_files.contains(&file_path) {
                                    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                    let caption = format!("【自动上传切片】\n主播: {}\n文件: {}", display_name_str, file_name);
                                    if let Err(e) = notifier_cloned.upload_file_to_telegram(&file_path, &caption, &app_config_cloned).await {
                                        error!("Failed to upload final segment {:?} to Telegram: {}", file_path, e);
                                    } else {
                                        uploaded_files.insert(file_path);
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

                        // Stop the telegram upload loop and wait for final uploads
                        let _ = poll_stop_tx.send(true);
                        let _ = upload_handle.await;

                        // Move files from downloading directory to final target directory
                        move_session_files_to_dest(&session.output_file_path, &session.target_dir_path);
                        
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
                    map.insert(url.clone(), RoomStatus {
                        url: url.clone(),
                        title: "".to_string(),
                        anchor_name: display_name.to_string(),
                        status: "Idle".to_string(),
                        record_path: None,
                        live_url: None,
                        platform: handler.name().to_string(),
                    });
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
        let statuses_map = statuses.read().await;
        if let Ok(json_str) = serde_json::to_string_pretty(&*statuses_map) {
            let _ = fs::write(status_path, json_str);
        }
    }
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

fn find_session_files(downloading_dir: &Path, file_path_template: &Path) -> Vec<PathBuf> {
    let mut matched = Vec::new();
    let template_name = match file_path_template.file_name().and_then(|s| s.to_str()) {
        Some(n) => n,
        None => return matched,
    };

    if template_name.contains("%03d") {
        let parts: Vec<&str> = template_name.split("%03d").collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            if let Ok(entries) = std::fs::read_dir(downloading_dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with(prefix) && name.ends_with(suffix) {
                            matched.push(entry.path());
                        }
                    }
                }
            }
        }
    } else {
        let path = downloading_dir.join(template_name);
        if path.exists() {
            matched.push(path);
        }
    }
    matched
}

fn move_session_files_to_dest(downloading_file_template: &Path, dest_dir: &Path) {
    let downloading_dir = match downloading_file_template.parent() {
        Some(p) => p,
        None => return,
    };
    let matched_files = find_session_files(downloading_dir, downloading_file_template);

    if !matched_files.is_empty() {
        let _ = std::fs::create_dir_all(dest_dir);
        for src in matched_files {
            if let Some(filename) = src.file_name() {
                let dest = dest_dir.join(filename);
                info!("Finalizing download: Moving file from downloading to final dir: {:?}", dest);
                if let Err(e) = std::fs::rename(&src, &dest) {
                    debug!("Rename failed, falling back to copy/remove: {}", e);
                    if let Err(err) = std::fs::copy(&src, &dest).and_then(|_| std::fs::remove_file(&src)) {
                        error!("Failed to move finalized file {:?} to {:?}: {}", src, dest, err);
                    }
                }
            }
        }
    }
}

pub fn scan_and_move_leftovers(save_path: &Path) {
    let downloading_dir = save_path.join("downloading");
    if !downloading_dir.exists() {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(&downloading_dir) {
        for entry in entries.flatten() {
            let src = entry.path();
            if src.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    let dest = save_path.join(name);
                    info!("Startup cleaner: Moving leftover file from downloading to download dir: {:?}", dest);
                    if let Err(e) = std::fs::rename(&src, &dest) {
                        debug!("Rename failed for leftover file, falling back to copy/remove: {}", e);
                        if let Err(err) = std::fs::copy(&src, &dest).and_then(|_| std::fs::remove_file(&src)) {
                            error!("Failed to move leftover file {:?} to {:?}: {}", src, dest, err);
                        }
                    }
                }
            }
        }
    }
}
