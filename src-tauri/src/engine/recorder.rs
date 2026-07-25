use crate::config::AppConfig;
use crate::platforms::StreamUrls;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tracing::{info, warn, debug};
use chrono::Local;

pub struct RecordSession {
    child: Child,
    stop_tx: Option<oneshot::Sender<()>>,
    pub output_file_path: PathBuf,
    pub target_dir_path: PathBuf,
}

impl RecordSession {
    pub async fn wait_for_completion(&mut self) -> Result<std::process::ExitStatus, std::io::Error> {
        self.child.wait().await
    }

    pub async fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        
        // Try writing 'q' to stdin for a graceful exit
        if let Some(mut stdin) = self.child.stdin.take() {
            debug!("Sending 'q' to FFmpeg stdin for graceful stop");
            if let Err(e) = stdin.write_all(b"q\n").await {
                debug!("Failed to write 'q' to FFmpeg stdin: {}", e);
            }
            let _ = stdin.flush().await;
        }

        // Wait a bit or force kill
        let sleep = tokio::time::sleep(tokio::time::Duration::from_secs(3));
        tokio::pin!(sleep);

        tokio::select! {
            _ = self.child.wait() => {
                debug!("FFmpeg process exited gracefully");
            }
            _ = &mut sleep => {
                warn!("FFmpeg did not exit gracefully, killing process");
                let _ = self.child.kill().await;
            }
        }
    }
}

pub struct Recorder;

impl Recorder {
    pub fn new() -> Self {
        Self
    }

    /// Construct output directory and file path template according to AppConfig.
    /// Returns (dir_path, file_path, effective_extension).
    pub fn build_paths(
        config: &AppConfig,
        anchor_name: &str,
        title: &str,
        split_by_time: bool,
        custom_format: Option<&str>,
    ) -> (PathBuf, PathBuf, String) {
        let now = Local::now();
        let time_str = now.format("%Y-%m-%d_%H-%M-%S").to_string();
        
        // Clean up title/anchor names of forbidden characters
        let clean_anchor = sanitize_filename(anchor_name);
        let clean_title = sanitize_filename(title);
        
        let dir_path = config.settings.save_path.clone();
        
        // Base filename containing anchor, title (if enabled/available), and timestamp
        let filename_base = if config.settings.filename_by_title || (!clean_title.is_empty() && clean_title != "抖音直播间" && clean_title != "直播间") {
            format!("{}_{}_{}", clean_anchor, clean_title, time_str)
        } else {
            format!("{}_{}", clean_anchor, time_str)
        };
        
        // Check extension from custom_format first, fallback to video_save_type (defaults to ts)
        let raw_fmt = custom_format
            .map(|s| s.to_string())
            .unwrap_or_else(|| config.settings.video_save_type.clone())
            .to_lowercase();
        let ext = match raw_fmt.as_str() {
            "ts" | "mkv" | "flv" | "mp4" | "mp3" | "m4a" => raw_fmt.as_str(),
            "mp3音频" => "mp3",
            "m4a音频" => "m4a",
            _ => "ts",
        };
        let ext = ext.to_string(); // own the string to outlive raw_fmt
        
        let filename = if split_by_time {
            format!("{}_%03d.{}", filename_base, ext)
        } else {
            format!("{}.{}", filename_base, ext)
        };
        
        let file_path = dir_path.join(&filename);
        (dir_path, file_path, ext)
    }

    /// Start a recording session using FFmpeg
    pub async fn start_record(
        &self,
        anchor_name: &str,
        title: &str,
        stream_urls: &StreamUrls,
        config: &AppConfig,
        config_toml_path: &std::path::Path,
        custom_format: Option<&str>,
    ) -> Result<RecordSession, Box<dyn std::error::Error + Send + Sync>> {
        // Read split config from AppConfig
        let split_mode = config.settings.split_mode.to_lowercase();
        let enable_split = split_mode != "none" && split_mode != "false" && split_mode != "off";

        // Extract headers from stream URLs (passed by platform plugin)
        let mut user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string();
        let mut custom_headers_str = String::new();
        
        if let Some(ref headers) = stream_urls.headers {
            for (key, val) in headers {
                if key.to_lowercase() == "user-agent" {
                    user_agent = val.clone();
                } else {
                    custom_headers_str.push_str(&format!("{}: {}\r\n", key, val));
                }
            }
        }

        let proxy_addr_str = if config.settings.use_proxy {
            config.settings.proxy_addr.as_deref()
        } else {
            None
        };

        let (dir_path, file_path, effective_ext) = Self::build_paths(config, anchor_name, title, enable_split, custom_format);

        let segment_time_str = match split_mode.as_str() {
            "size" => {
                let size_mb = config.settings.split_size_mb.max(10);
                let is_audio_format = matches!(custom_format, Some("mp3") | Some("m4a")) || effective_ext == "mp3" || effective_ext == "m4a";
                let fallback_bitrate = if is_audio_format {
                    256
                } else {
                    (config.settings.split_video_bitrate_kbps as u64).max(100)
                };

                let effective_bitrate_kbps = match Self::probe_stream_bitrate(&stream_urls.record_url, &user_agent, &custom_headers_str, proxy_addr_str).await {
                    Some(probed) => {
                        info!("Auto-detected stream bitrate for [{}]: {} kbps (Target segment size: {} MB)", anchor_name, probed, size_mb);
                        probed
                    }
                    None => {
                        debug!("Bitrate probing for [{}] yielded no result, using configured fallback bitrate: {} kbps", anchor_name, fallback_bitrate);
                        fallback_bitrate
                    }
                };

                let calc_secs = (size_mb * 8388608) / (effective_bitrate_kbps * 1000);
                calc_secs.max(10).to_string()
            }
            _ => { // "time" or default
                config.settings.split_time_secs.max(10).to_string()
            }
        };

        // Create downloading directory inside config directory (e.g. ~/.config/LiveDownloader/downloading)
        let downloading_dir = crate::config::get_downloading_dir(config_toml_path);

        let filename_str = file_path.file_name().ok_or("Invalid filename")?.to_str().ok_or("Invalid filename encoding")?;
        let downloading_file_path = downloading_dir.join(filename_str);

        // Create target directory if it doesn't exist
        std::fs::create_dir_all(&dir_path)?;
        
        let mut args = vec![
            "-y".to_string(),
            "-v".to_string(), "verbose".to_string(),
            "-rw_timeout".to_string(), "15000000".to_string(),
            "-loglevel".to_string(), "error".to_string(),
            "-hide_banner".to_string(),
        ];
        
        // Add http proxy if configured and requested for this platform (or globally)
        if config.settings.use_proxy {
            if let Some(ref proxy) = config.settings.proxy_addr {
                args.push("-http_proxy".to_string());
                args.push(proxy.clone());
            }
        }

        args.push("-user_agent".to_string());
        args.push(user_agent);
        
        if !custom_headers_str.is_empty() {
            args.push("-headers".to_string());
            args.push(custom_headers_str);
        }
        
        // Input settings
        args.push("-protocol_whitelist".to_string());
        args.push("rtmp,crypto,file,http,https,tcp,tls,udp,rtp,httpproxy".to_string());
        args.push("-thread_queue_size".to_string());
        args.push("1024".to_string());
        args.push("-analyzeduration".to_string());
        args.push("20000000".to_string());
        args.push("-probesize".to_string());
        args.push("20000000".to_string());
        args.push("-fflags".to_string());
        args.push("+discardcorrupt".to_string());
        
        // Real stream input URL
        args.push("-i".to_string());
        args.push(stream_urls.record_url.clone());
        
        // Reconnect and queue settings
        args.push("-bufsize".to_string());
        args.push("15000k".to_string());
        args.push("-sn".to_string());
        args.push("-dn".to_string());
        args.push("-reconnect_delay_max".to_string());
        args.push("60".to_string());
        args.push("-reconnect_streamed".to_string());
        args.push("-reconnect_at_eof".to_string());
        args.push("-max_muxing_queue_size".to_string());
        args.push("2048".to_string());
        args.push("-correct_ts_overflow".to_string());
        args.push("1".to_string());
        args.push("-avoid_negative_ts".to_string());
        args.push("1".to_string());
        
        // Output format: use effective_ext resolved by build_paths (respects custom_format)
        let ext = effective_ext.as_str();
        
        match ext {
            "mp3" => {
                args.push("-map".to_string());
                args.push("0:a".to_string());
                args.push("-c:a".to_string());
                args.push("libmp3lame".to_string());
                args.push("-ab".to_string());
                args.push("320k".to_string());
                
                if enable_split {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push(segment_time_str.clone());
                    args.push("-reset_timestamps".to_string());
                    args.push("1".to_string());
                    args.push("-segment_format".to_string());
                    args.push("mp3".to_string());
                } else {
                    args.push("-f".to_string());
                    args.push("mp3".to_string());
                }
            }
            "m4a" => {
                args.push("-map".to_string());
                args.push("0:a".to_string());
                args.push("-c:a".to_string());
                args.push("aac".to_string());
                args.push("-bsf:a".to_string());
                args.push("aac_adtstoasc".to_string());
                args.push("-ab".to_string());
                args.push("320k".to_string());
                
                if enable_split {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push(segment_time_str.clone());
                    args.push("-reset_timestamps".to_string());
                    args.push("1".to_string());
                    args.push("-segment_format".to_string());
                    args.push("ipod".to_string());
                } else {
                    args.push("-f".to_string());
                    args.push("ipod".to_string());
                }
            }
            "mp4" => {
                args.push("-map".to_string());
                args.push("0".to_string());
                args.push("-c:v".to_string());
                args.push("copy".to_string());
                args.push("-c:a".to_string());
                args.push("copy".to_string());
                args.push("-bsf:a".to_string());
                args.push("aac_adtstoasc".to_string());
                
                if enable_split {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push(segment_time_str.clone());
                    args.push("-reset_timestamps".to_string());
                    args.push("1".to_string());
                    args.push("-segment_format".to_string());
                    args.push("mp4".to_string());
                } else {
                    args.push("-movflags".to_string());
                    args.push("+faststart".to_string());
                    args.push("-f".to_string());
                    args.push("mp4".to_string());
                }
            }
            "flv" => {
                args.push("-map".to_string());
                args.push("0".to_string());
                args.push("-c:v".to_string());
                args.push("copy".to_string());
                args.push("-c:a".to_string());
                args.push("copy".to_string());
                args.push("-bsf:a".to_string());
                args.push("aac_adtstoasc".to_string());
                
                if enable_split {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push(segment_time_str.clone());
                    args.push("-reset_timestamps".to_string());
                    args.push("1".to_string());
                    args.push("-segment_format".to_string());
                    args.push("flv".to_string());
                } else {
                    args.push("-f".to_string());
                    args.push("flv".to_string());
                }
            }
            "mkv" => {
                args.push("-map".to_string());
                args.push("0".to_string());
                args.push("-c:v".to_string());
                args.push("copy".to_string());
                args.push("-c:a".to_string());
                args.push("copy".to_string());
                args.push("-bsf:a".to_string());
                args.push("aac_adtstoasc".to_string());
                
                if enable_split {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push(segment_time_str.clone());
                    args.push("-reset_timestamps".to_string());
                    args.push("1".to_string());
                    args.push("-segment_format".to_string());
                    args.push("matroska".to_string());
                } else {
                    args.push("-f".to_string());
                    args.push("matroska".to_string());
                }
            }
            _ => { // ts
                args.push("-map".to_string());
                args.push("0".to_string());
                args.push("-c:v".to_string());
                args.push("copy".to_string());
                args.push("-c:a".to_string());
                args.push("copy".to_string());
                args.push("-bsf:a".to_string());
                args.push("aac_adtstoasc".to_string());
                
                if enable_split {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push(segment_time_str.clone());
                    args.push("-reset_timestamps".to_string());
                    args.push("1".to_string());
                    args.push("-segment_format".to_string());
                    args.push("mpegts".to_string());
                } else {
                    args.push("-f".to_string());
                    args.push("mpegts".to_string());
                }
            }
        }
        
        // Output file
        let output_str = downloading_file_path.to_string_lossy().to_string();
        args.push(output_str);

        let ffmpeg_path = get_ffmpeg_path();
        info!("Spawning FFmpeg at {:?} for {}. Args: {:?}", ffmpeg_path, anchor_name, args);

        let mut cmd = Command::new(ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let child = cmd.spawn()?;

        let (stop_tx, _stop_rx) = oneshot::channel();

        Ok(RecordSession {
            child,
            stop_tx: Some(stop_tx),
            output_file_path: downloading_file_path,
            target_dir_path: dir_path,
        })
    }

    /// Probe real-time stream bitrate (in kbps) using M3U8 tags or ffprobe
    pub async fn probe_stream_bitrate(
        stream_url: &str,
        user_agent: &str,
        custom_headers: &str,
        proxy_addr: Option<&str>,
    ) -> Option<u64> {
        // 1. Try parsing M3U8 playlist
        if stream_url.contains(".m3u8") {
            if let Ok(client) = reqwest::Client::builder().user_agent(user_agent).build() {
                if let Ok(res) = client.get(stream_url).send().await {
                    if let Ok(text) = res.text().await {
                        // 1a. Master Playlist: parse BANDWIDTH= attribute
                        for line in text.lines() {
                            if line.contains("BANDWIDTH=") {
                                if let Some(idx) = line.find("BANDWIDTH=") {
                                    let sub = &line[idx + 10..];
                                    let end = sub.find(|c: char| !c.is_ascii_digit()).unwrap_or(sub.len());
                                    if let Ok(bps) = sub[..end].parse::<u64>() {
                                        if bps > 16_000 {
                                            return Some(bps / 1000);
                                        }
                                    }
                                }
                            }
                        }

                        // 1b. Media Playlist: parse #EXTINF chunk duration & fetch Content-Length
                        let lines: Vec<&str> = text.lines().collect();
                        for i in 0..lines.len() {
                            if lines[i].starts_with("#EXTINF:") {
                                let dur_str = &lines[i][8..];
                                let dur_val = dur_str.split(',').next().and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
                                if dur_val > 0.5 && i + 1 < lines.len() {
                                    let seg_url_raw = lines[i + 1].trim();
                                    if !seg_url_raw.is_empty() && !seg_url_raw.starts_with('#') {
                                        let seg_url = if seg_url_raw.starts_with("http") {
                                            seg_url_raw.to_string()
                                        } else if let Ok(base) = url::Url::parse(stream_url) {
                                            base.join(seg_url_raw).map(|u| u.to_string()).unwrap_or_default()
                                        } else {
                                            String::new()
                                        };

                                        if !seg_url.is_empty() {
                                            if let Ok(head_res) = client.head(&seg_url).send().await {
                                                if let Some(cl) = head_res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok()) {
                                                    let bps = ((cl as f64 * 8.0) / dur_val) as u64;
                                                    if bps > 16_000 {
                                                        return Some(bps / 1000);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Use ffprobe command with a short 4-second timeout
        let ffprobe_path = get_ffprobe_path();
        let mut cmd = Command::new(ffprobe_path);
        cmd.arg("-v").arg("quiet")
           .arg("-print_format").arg("json")
           .arg("-show_format")
           .arg("-show_streams")
           .arg("-analyze_duration").arg("2000000")
           .arg("-probesize").arg("2000000")
           .arg("-user_agent").arg(user_agent);

        if !custom_headers.is_empty() {
            cmd.arg("-headers").arg(custom_headers);
        }
        if let Some(proxy) = proxy_addr {
            cmd.arg("-http_proxy").arg(proxy);
        }

        cmd.arg(stream_url);

        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        if let Ok(Ok(output)) = tokio::time::timeout(tokio::time::Duration::from_secs(4), cmd.output()).await {
            if output.status.success() {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(bitrate_str) = json.pointer("/format/bit_rate").and_then(|v| v.as_str()) {
                        if let Ok(bps) = bitrate_str.parse::<u64>() {
                            if bps > 16_000 {
                                return Some(bps / 1000);
                            }
                        }
                    }

                    // Check if bit_rate can be calculated from size and duration
                    if let (Some(size_str), Some(dur_str)) = (
                        json.pointer("/format/size").and_then(|v| v.as_str()),
                        json.pointer("/format/duration").and_then(|v| v.as_str()),
                    ) {
                        if let (Ok(size), Ok(dur)) = (size_str.parse::<u64>(), dur_str.parse::<f64>()) {
                            if dur > 0.1 {
                                let bps = ((size as f64 * 8.0) / dur) as u64;
                                if bps > 16_000 {
                                    return Some(bps / 1000);
                                }
                            }
                        }
                    }

                    let mut has_video = false;
                    let mut total_bps: u64 = 0;
                    if let Some(streams) = json.pointer("/streams").and_then(|v| v.as_array()) {
                        for s in streams {
                            if s.get("codec_type").and_then(|v| v.as_str()) == Some("video") {
                                has_video = true;
                            }
                            if let Some(br_str) = s.get("bit_rate").and_then(|v| v.as_str()) {
                                if let Ok(bps) = br_str.parse::<u64>() {
                                    total_bps += bps;
                                }
                            }
                        }
                    }

                    if total_bps > 16_000 {
                        return Some(total_bps / 1000);
                    }

                    // If audio-only stream (no video stream), return 256 kbps audio bitrate!
                    if !has_video {
                        info!("Stream probed as audio-only, using 256 kbps audio bitrate");
                        return Some(256);
                    }
                }
            }
        }

        None
    }
}

/// Sanitize filename by removing invalid OS characters
fn sanitize_filename(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' || c == ' ' {
            s.push(c);
        } else {
            // Replace emojis/special characters with empty or underscore
            s.push('_');
        }
    }
    // Remove consecutive underscores
    let mut result = String::new();
    let mut last_was_under = false;
    for c in s.chars() {
        if c == '_' {
            if !last_was_under {
                result.push(c);
                last_was_under = true;
            }
        } else {
            result.push(c);
            last_was_under = false;
        }
    }
    result.trim_matches(|c| c == '_' || c == ' ').to_string()
}

/// Retrieve the custom local FFmpeg path or fallback to system path
fn get_ffmpeg_path() -> PathBuf {
    // 1. Check in the same directory as the running executable
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let local_ffmpeg = exe_dir.join(if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" });
            if local_ffmpeg.exists() {
                return local_ffmpeg;
            }
        }
    }

    // 2. Check in the current working directory
    let cwd_ffmpeg = PathBuf::from(if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" });
    if cwd_ffmpeg.exists() {
        return cwd_ffmpeg;
    }

    // 3. Fallback to system PATH
    PathBuf::from("ffmpeg")
}

/// Retrieve the custom local FFprobe path or fallback to system path
fn get_ffprobe_path() -> PathBuf {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let local_ffprobe = exe_dir.join(if cfg!(target_os = "windows") { "ffprobe.exe" } else { "ffprobe" });
            if local_ffprobe.exists() {
                return local_ffprobe;
            }
        }
    }

    let cwd_ffprobe = PathBuf::from(if cfg!(target_os = "windows") { "ffprobe.exe" } else { "ffprobe" });
    if cwd_ffprobe.exists() {
        return cwd_ffprobe;
    }

    PathBuf::from("ffprobe")
}
