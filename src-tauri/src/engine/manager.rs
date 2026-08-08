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

pub fn perform_concat_merge(
    anchor_dir: &Path,
    segment_files: &[PathBuf],
    target_dir: &Path,
    output_filename: &str,
) -> Result<PathBuf, String> {
    if segment_files.is_empty() {
        return Err("No segment files to merge".to_string());
    }

    std::fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
    let dest_path = target_dir.join(output_filename);

    let ffmpeg_path = crate::engine::recorder::get_ffmpeg_path();
    let ext = dest_path.extension().and_then(|s| s.to_str()).unwrap_or("ts").to_lowercase();

    if segment_files.len() == 1 {
        let src = &segment_files[0];
        let src_ext = src.extension().and_then(|s| s.to_str()).unwrap_or("ts").to_lowercase();

        if src_ext == ext {
            if let Err(e) = std::fs::rename(src, &dest_path) {
                debug!("Rename failed, falling back to copy/remove: {}", e);
                std::fs::copy(src, &dest_path).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(src);
            }
            return Ok(dest_path);
        } else {
            let mut cmd = std::process::Command::new(&ffmpeg_path);
            let mut args = vec![
                "-y".to_string(),
                "-i".to_string(), src.to_string_lossy().to_string(),
                "-c".to_string(), "copy".to_string(),
            ];
            if ext == "mp4" {
                args.push("-movflags".to_string());
                args.push("+faststart".to_string());
            }
            args.push(dest_path.to_string_lossy().to_string());
            cmd.args(&args);
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000);
            }
            let output = cmd.output().map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(format!("FFmpeg remux failed with status: {:?}", output.status));
            }
            let _ = std::fs::remove_file(src);
            return Ok(dest_path);
        }
    }

    let concat_list_path = anchor_dir.join(format!("concat_{}.txt", chrono::Local::now().format("%H%M%S_%f")));
    let mut concat_content = String::new();
    for file in segment_files {
        if let Some(filename) = file.file_name().and_then(|s| s.to_str()) {
            concat_content.push_str(&format!("file '{}'\n", filename.replace("'", "'\\''")));
        }
    }
    std::fs::write(&concat_list_path, concat_content).map_err(|e| e.to_string())?;

    let mut cmd = std::process::Command::new(&ffmpeg_path);
    let mut args = vec![
        "-y".to_string(),
        "-f".to_string(), "concat".to_string(),
        "-safe".to_string(), "0".to_string(),
        "-i".to_string(), concat_list_path.to_string_lossy().to_string(),
        "-c".to_string(), "copy".to_string(),
    ];
    if ext == "mp4" {
        args.push("-movflags".to_string());
        args.push("+faststart".to_string());
    }
    args.push(dest_path.to_string_lossy().to_string());
    cmd.args(&args);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let output = cmd.output().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&concat_list_path);

    if !output.status.success() {
        return Err(format!("FFmpeg concat failed with status: {:?}", output.status));
    }

    for file in segment_files {
        let _ = std::fs::remove_file(file);
    }

    Ok(dest_path)
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
            let config_md5 = get_file_md5(&self.config_path).unwrap_or_default();
            let rooms_path = self.config_path.with_file_name("rooms.toml");
            let rooms_md5 = get_file_md5(&rooms_path).unwrap_or_default();
            let current_md5 = format!("{}:{}", config_md5, rooms_md5);
            let reload_needed = current_md5 != last_config_md5 || engine_just_resumed;
            
            if reload_needed {
                info!("Configuration file changed or loaded for the first time. Reloading... (config_md5={}, rooms_md5={})", config_md5, rooms_md5);
                last_config_md5 = current_md5;
                
                match AppConfig::load_from_file(&self.config_path) {
                    Ok(new_config) => {
                        let rooms_count = new_config.rooms.len();
                        let commented_count = new_config.rooms.iter().filter(|r| r.is_commented).count();
                        debug!("[CONFIG_RELOAD] Loaded config: {} total rooms, {} commented, {} active", rooms_count, commented_count, rooms_count - commented_count);
                        for (i, r) in new_config.rooms.iter().enumerate() {
                            debug!("[CONFIG_RELOAD]   room[{}]: url={}, name={:?}, commented={}", i, r.url, r.name, r.is_commented);
                        }
                        let mut w_config = self.config.write().await;
                        *w_config = new_config;
                    }
                    Err(e) => {
                        error!("[CONFIG_RELOAD] Failed to load config from {:?}: {}", self.config_path, e);
                    }
                }
                
                let rooms = {
                    let r_config = self.config.read().await;
                    r_config.rooms.clone()
                };
                
                // Log current state before sync
                {
                    let map = self.room_statuses.read().await;
                    debug!("[SYNC_BEFORE] Status map has {} entries, active_tasks has {} entries", map.len(), self.active_tasks.len());
                    for (url, status) in map.iter() {
                        debug!("[SYNC_BEFORE]   status_map: url={}, status={}, anchor={}", url, status.status, status.anchor_name);
                    }
                    for url in self.active_tasks.keys() {
                        debug!("[SYNC_BEFORE]   active_task: url={}", url);
                    }
                }
                
                self.sync_tasks(rooms, platform_manager.clone(), recorder.clone()).await;
                
                // Log state after sync
                {
                    let map = self.room_statuses.read().await;
                    debug!("[SYNC_AFTER] Status map has {} entries, active_tasks has {} entries", map.len(), self.active_tasks.len());
                    for (url, status) in map.iter() {
                        debug!("[SYNC_AFTER]   status_map: url={}, status={}, anchor={}", url, status.status, status.anchor_name);
                    }
                }
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
        
        debug!("[SYNC_TASKS] Starting sync with {} rooms from config", rooms.len());
        
        for url_cfg in rooms {
            if url_cfg.is_commented {
                debug!("[SYNC_TASKS] Skipping commented room: {}", url_cfg.url);
                continue;
            }
            
            current_urls.insert(url_cfg.url.clone());
            
            if !self.active_tasks.contains_key(&url_cfg.url) {
                info!("[SYNC_TASKS] Starting new monitor task for URL: {}", url_cfg.url);
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
                        debug!("[SYNC_TASKS] Room {} already in status map (status={}), updating", url, existing.status);
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
                        debug!("[SYNC_TASKS] Inserting new room {} into status map with status={}", url, initial_status);
                        map.insert(url.clone(), RoomStatus {
                            url: url.clone(),
                            title: "".to_string(),
                            anchor_name: custom_name.clone().unwrap_or_else(|| "未知主播".to_string()),
                            status: initial_status.to_string(),
                            record_path: None,
                            live_url: None,
                            platform: handler_name.to_string(),
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
            } else {
                debug!("[SYNC_TASKS] Room {} already has active task, skipping", url_cfg.url);
            }
        }
        
        // Stop tasks that are no longer in the URL config list
        let mut to_remove = Vec::new();
        for url in self.active_tasks.keys() {
            if !current_urls.contains(url) {
                to_remove.push(url.clone());
            }
        }
        
        if !to_remove.is_empty() {
            warn!("[SYNC_TASKS] About to REMOVE {} rooms from status map that are no longer in config: {:?}", to_remove.len(), to_remove);
            debug!("[SYNC_TASKS] current_urls from config ({} items): {:?}", current_urls.len(), current_urls);
        }
        
        for url in to_remove {
            if let Some(stop_tx) = self.active_tasks.remove(&url) {
                warn!("[SYNC_TASKS] REMOVING room and stopping monitor task for URL: {}", url);
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
    
    info!("Starting monitoring loop for room: [{}] ({})", handler.name(), url);
    
    loop {
        // Check cancellation
        if stop_rx.try_recv().is_ok() {
            info!("Cancellation signal received. Exiting task loop for [{}]", url);
            break;
        }
        
        // Retrieve current configuration
        let (delay_secs, pc) = {
            let r_config = config.read().await;
            let platform_cookie = r_config.get_cookie_for_platform(handler.id());
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
                        live_url: stream_urls.m3u8_url.clone().or_else(|| {
                            let rec = stream_urls.record_url.clone();
                            if rec.contains(".flv") {
                                Some(rec.replace("pull-flv-", "pull-hls-").replace(".flv?", ".m3u8?").replace(".flv", ".m3u8"))
                            } else {
                                Some(rec)
                            }
                        }),
                        platform: handler.name().to_string(),
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

                        // Segment monitoring task for size-threshold concat merging and Telegram auto-upload
                        let output_template = session.output_file_path.clone();
                        let target_dir_path = session.target_dir_path.clone();
                        let filename_base = session.filename_base.clone();
                        let effective_ext = session.effective_ext.clone();
                        let app_config_cloned = app_config.clone();
                        let display_name_str = display_name.to_string();
                        let notifier_cloned = crate::engine::notifier::Notifier::new();
                        let url_str = url.clone();
                        let statuses_cloned = statuses.clone();
                        
                        let (poll_stop_tx, mut poll_stop_rx) = tokio::sync::watch::channel(false);
                        
                        let segment_handle = tokio::spawn(async move {
                            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
                            let mut processed_files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
                            let mut part_index = 1;

                            let anchor_dir = match output_template.parent() {
                                Some(p) => p.to_path_buf(),
                                None => return,
                            };
                            let target_mb = app_config_cloned.settings.split_size_mb;
                            
                            loop {
                                tokio::select! {
                                    _ = interval.tick() => {
                                        let completed = find_completed_segments(&output_template, true);
                                        let unmerged: Vec<PathBuf> = completed.into_iter().filter(|f| !processed_files.contains(f)).collect();

                                        let unmerged_bytes: u64 = unmerged.iter().map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0)).sum();
                                        let unmerged_mb = unmerged_bytes as f64 / (1024.0 * 1024.0);

                                        if target_mb > 0 && unmerged_mb >= target_mb as f64 && !unmerged.is_empty() {
                                            let part_filename = format!("{}_part{}.{}", filename_base, part_index, effective_ext);
                                            info!("Split size limit ({} MB) reached for [{}]. Merging {} segments ({:.2} MB) into {}", target_mb, display_name_str, unmerged.len(), unmerged_mb, part_filename);

                                            match perform_concat_merge(&anchor_dir, &unmerged, &target_dir_path, &part_filename) {
                                                Ok(merged_dest) => {
                                                    for f in &unmerged {
                                                        processed_files.insert(f.clone());
                                                    }
                                                    part_index += 1;

                                                    if app_config_cloned.push.tg_auto_upload {
                                                        let caption = format!("【自动上传切片】\n主播: {}\n文件: {}", display_name_str, part_filename);
                                                        if let Err(e) = notifier_cloned.upload_file_to_telegram(&merged_dest, &caption, &app_config_cloned).await {
                                                            error!("Failed to upload merged part {:?} to Telegram: {}", merged_dest, e);
                                                        }
                                                    }

                                                    let mut map = statuses_cloned.write().await;
                                                    if let Some(room) = map.get_mut(&url_str) {
                                                        room.record_path = Some(merged_dest.to_string_lossy().to_string());
                                                    }
                                                    save_room_statuses(&statuses_cloned).await;
                                                }
                                                Err(e) => {
                                                    error!("Failed to merge segments for [{}]: {}", display_name_str, e);
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
                            let unmerged: Vec<PathBuf> = completed.into_iter().filter(|f| !processed_files.contains(f)).collect();

                            if !unmerged.is_empty() {
                                let final_filename = if part_index == 1 {
                                    format!("{}.{}", filename_base, effective_ext)
                                } else {
                                    format!("{}_part{}.{}", filename_base, part_index, effective_ext)
                                };
                                info!("Finalizing download for [{}]: Merging {} remaining segments into {}", display_name_str, unmerged.len(), final_filename);

                                match perform_concat_merge(&anchor_dir, &unmerged, &target_dir_path, &final_filename) {
                                    Ok(merged_dest) => {
                                        if app_config_cloned.push.tg_auto_upload {
                                            let caption = format!("【自动上传切片】\n主播: {}\n文件: {}", display_name_str, final_filename);
                                            if let Err(e) = notifier_cloned.upload_file_to_telegram(&merged_dest, &caption, &app_config_cloned).await {
                                                error!("Failed to upload final segment {:?} to Telegram: {}", merged_dest, e);
                                            }
                                        }

                                        let mut map = statuses_cloned.write().await;
                                        if let Some(room) = map.get_mut(&url_str) {
                                            room.record_path = Some(merged_dest.to_string_lossy().to_string());
                                        }
                                        save_room_statuses(&statuses_cloned).await;
                                    }
                                    Err(e) => {
                                        error!("Failed to merge final segments for [{}]: {}", display_name_str, e);
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
                    let map_len = map.len();
                    if let Some(room) = map.get_mut(&url) {
                        debug!("[ROOM_LIFECYCLE] Recording finished for [{}], setting status from '{}' -> 'Idle' (map has {} entries)", url, room.status, map_len);
                        room.status = "Idle".to_string();
                        room.record_path = None;
                        room.live_url = None;
                    } else {
                        warn!("[ROOM_LIFECYCLE] Room [{}] was NOT in status_map after recording finished (map has {} entries)! Re-inserting.", url, map_len);
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
                }
                save_room_statuses(&statuses).await;
            }
            Ok(LiveStatus::Idle) => {
                debug!("Room [{}] is currently offline/idle.", url);
                {
                    let mut map = statuses.write().await;
                    if let Some(room) = map.get_mut(&url) {
                        if room.status != "Idle" {
                            debug!("[ROOM_LIFECYCLE] Room [{}] status changing '{}' -> 'Idle'", url, room.status);
                        }
                        room.status = "Idle".to_string();
                        room.record_path = None;
                        room.live_url = None;
                    } else {
                        warn!("[ROOM_LIFECYCLE] Room [{}] is Idle but NOT in status_map (map has {} entries)!", url, map.len());
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
        let count = statuses_map.len();
        debug!("[SAVE_STATUSES] Saving {} room statuses to {:?}", count, status_path);
        if count == 0 {
            warn!("[SAVE_STATUSES] WARNING: Saving EMPTY status map to statuses.json!");
        }
        if let Ok(json_str) = serde_json::to_string_pretty(&*statuses_map) {
            if fs::write(&tmp_status_path, &json_str).is_ok() {
                if let Err(e) = fs::rename(&tmp_status_path, &status_path) {
                    error!("[SAVE_STATUSES] Failed to rename tmp status file: {}", e);
                }
            } else {
                error!("[SAVE_STATUSES] Failed to write tmp status file {:?}", tmp_status_path);
            }
        } else {
            error!("[SAVE_STATUSES] Failed to serialize status map to JSON");
        }
    }
}

fn load_room_statuses_from_file(config_path: &Path) -> HashMap<String, RoomStatus> {
    if let Some(parent) = config_path.parent() {
        let status_path = parent.join("statuses.json");
        if status_path.exists() {
            match fs::read_to_string(&status_path) {
                Ok(content) => {
                    debug!("[LOAD_STATUSES] Read statuses.json ({} bytes)", content.len());
                    match serde_json::from_str::<HashMap<String, RoomStatus>>(&content) {
                        Ok(mut map) => {
                            info!("[LOAD_STATUSES] Loaded {} room statuses from statuses.json", map.len());
                            for (url, status) in map.iter_mut() {
                                debug!("[LOAD_STATUSES]   room: url={}, status={}, anchor={}", url, status.status, status.anchor_name);
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
                        Err(e) => {
                            error!("[LOAD_STATUSES] Failed to parse statuses.json: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("[LOAD_STATUSES] Failed to read statuses.json: {}", e);
                }
            }
        } else {
            info!("[LOAD_STATUSES] statuses.json does not exist at {:?}", status_path);
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

    if let Ok(anchor_entries) = std::fs::read_dir(&downloading_dir) {
        for anchor_entry in anchor_entries.flatten() {
            let anchor_path = anchor_entry.path();
            if anchor_path.is_dir() {
                if let Ok(files) = std::fs::read_dir(&anchor_path) {
                    let mut unlocked_files: Vec<PathBuf> = Vec::new();
                    for file_entry in files.flatten() {
                        let src = file_entry.path();
                        if src.is_file() {
                            if std::fs::OpenOptions::new().write(true).open(&src).is_ok() {
                                unlocked_files.push(src);
                            } else {
                                debug!("Startup cleaner: File {:?} is locked, skipping", src);
                            }
                        }
                    }
                    if !unlocked_files.is_empty() {
                        unlocked_files.sort();
                        let anchor_name = anchor_entry.file_name().to_string_lossy().to_string();
                        let now_str = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                        let out_name = format!("{}_leftover_{}.ts", anchor_name, now_str);
                        info!("Startup cleaner: Merging {} leftover segment files for anchor [{}] into {}", unlocked_files.len(), anchor_name, out_name);
                        let _ = perform_concat_merge(&anchor_path, &unlocked_files, save_path, &out_name);
                    }
                }
            } else if anchor_path.is_file() {
                if std::fs::OpenOptions::new().write(true).open(&anchor_path).is_ok() {
                    if let Some(name) = anchor_entry.file_name().to_str() {
                        let dest = save_path.join(name);
                        info!("Startup cleaner: Moving leftover file to save_path: {:?}", dest);
                        if let Err(_e) = std::fs::rename(&anchor_path, &dest) {
                            if let Err(err) = std::fs::copy(&anchor_path, &dest).and_then(|_| std::fs::remove_file(&anchor_path)) {
                                warn!("Startup cleaner move failed: {}", err);
                            }
                        }
                    }
                }
            }
        }
    }
}
