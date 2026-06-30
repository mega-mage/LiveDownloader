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

    /// Construct output directory and file path template according to AppConfig
    pub fn build_paths(
        config: &AppConfig,
        anchor_name: &str,
        title: &str,
        split_by_time: bool,
        custom_format: Option<&str>,
    ) -> (PathBuf, PathBuf) {
        let now = Local::now();
        let date_folder = now.format("%Y-%m-%d").to_string();
        let time_str = now.format("%Y-%m-%d_%H-%M-%S").to_string();
        
        // Clean up title/anchor names of forbidden characters
        let clean_anchor = sanitize_filename(anchor_name);
        let clean_title = sanitize_filename(title);
        
        let mut dir_path = config.settings.save_path.clone();
        
        if config.settings.folder_by_author {
            dir_path.push(&clean_anchor);
        }
        if config.settings.folder_by_time {
            dir_path.push(&date_folder);
        }
        if config.settings.folder_by_title {
            dir_path.push(&clean_title);
        }
        
        // Base filename
        let filename_base = if config.settings.filename_by_title {
            format!("{}_{}_{}", clean_anchor, clean_title, time_str)
        } else {
            format!("{}_{}", clean_anchor, time_str)
        };
        
        // Check extension from custom_format first, fallback to video_save_type (defaults to ts)
        let ext = custom_format.map(|s| s.to_string())
            .unwrap_or_else(|| config.settings.video_save_type.clone())
            .to_lowercase();
        let ext = match ext.as_str() {
            "ts" | "mkv" | "flv" | "mp4" | "mp3" | "m4a" => ext.as_str(),
            "mp3音频" => "mp3",
            "m4a音频" => "m4a",
            _ => "ts",
        };
        
        let filename = if split_by_time {
            format!("{}_%03d.{}", filename_base, ext)
        } else {
            format!("{}.{}", filename_base, ext)
        };
        
        let file_path = dir_path.join(&filename);
        (dir_path, file_path)
    }

    /// Start a recording session using FFmpeg
    pub fn start_record(
        &self,
        anchor_name: &str,
        title: &str,
        stream_urls: &StreamUrls,
        config: &AppConfig,
        custom_format: Option<&str>,
    ) -> Result<RecordSession, Box<dyn std::error::Error + Send + Sync>> {
        // Read segment config
        // Default to split if configured. Let's assume we can fetch split settings from extra configs or check python logic
        // We will read "分段录制是否开启" and "视频分段时间(秒)" from config section
        // Note: For now, we can check if config has custom fields, or assume config.ini defaults.
        // Let's add simple parsing for these in recorder.
        let split_by_time = true;
        
        let (dir_path, file_path) = Self::build_paths(config, anchor_name, title, split_by_time, custom_format);

        // Create downloading directory
        let downloading_dir = config.settings.save_path.join("downloading");
        std::fs::create_dir_all(&downloading_dir)?;

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
        args.push("-re".to_string());
        
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
        
        // Output format settings
        let ext = config.settings.video_save_type.to_lowercase();
        let ext = match ext.as_str() {
            "ts" | "mkv" | "flv" | "mp4" | "mp3" | "m4a" => ext.as_str(),
            "mp3音频" => "mp3",
            "m4a音频" => "m4a",
            _ => "ts",
        };
        
        match ext {
            "mp3" => {
                args.push("-map".to_string());
                args.push("0:a".to_string());
                args.push("-c:a".to_string());
                args.push("libmp3lame".to_string());
                args.push("-ab".to_string());
                args.push("320k".to_string());
                
                if split_by_time {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push("1200".to_string());
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
                
                if split_by_time {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push("1200".to_string());
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
                
                if split_by_time {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push("1200".to_string());
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
                
                if split_by_time {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push("1200".to_string());
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
                
                if split_by_time {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push("1200".to_string());
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
                
                if split_by_time {
                    args.push("-f".to_string());
                    args.push("segment".to_string());
                    args.push("-segment_time".to_string());
                    args.push("1200".to_string());
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
