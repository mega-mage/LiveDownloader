use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};


// Default value helpers for Serde
fn default_language() -> String {
    "zh_cn".to_string()
}
fn default_save_path() -> PathBuf {
    if std::path::Path::new("/data/data/com.termux").exists() || std::env::var("TERMUX_VERSION").is_ok() {
        if std::path::Path::new("/sdcard/Download").exists() {
            return PathBuf::from("/sdcard/Download");
        }
        if let Ok(home) = std::env::var("HOME") {
            let termux_dl = PathBuf::from(&home).join("storage/downloads");
            if termux_dl.exists() {
                return termux_dl;
            }
        }
        return PathBuf::from("/sdcard/Download");
    }
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        return PathBuf::from(home).join("downloads");
    }
    PathBuf::from("./downloads")
}
fn default_false() -> bool {
    false
}
fn default_video_save_type() -> String {
    "ts".to_string()
}
fn default_quality() -> String {
    "原画".to_string()
}
fn default_max_request() -> usize {
    3
}
fn default_delay() -> u64 {
    300
}
fn default_server_port() -> u16 {
    10730
}
fn default_split_mode() -> String {
    "time".to_string()
}
fn default_split_time_secs() -> u64 {
    1200
}
fn default_split_size_mb() -> u64 {
    1024
}
fn default_split_video_bitrate_kbps() -> u32 {
    8000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsConfig {
    #[serde(default = "default_language")]
    pub language: String,

    #[serde(default = "default_save_path")]
    pub save_path: PathBuf,

    #[serde(default = "default_false")]
    pub engine_paused: bool,

    #[serde(default = "default_false")]
    pub folder_by_author: bool,

    #[serde(default = "default_false")]
    pub folder_by_time: bool,

    #[serde(default = "default_false")]
    pub folder_by_title: bool,

    #[serde(default = "default_false")]
    pub filename_by_title: bool,

    #[serde(default = "default_video_save_type")]
    pub video_save_type: String,

    #[serde(default = "default_quality")]
    pub video_record_quality: String,

    #[serde(default = "default_false")]
    pub use_proxy: bool,

    pub proxy_addr: Option<String>,

    #[serde(default = "default_max_request")]
    pub max_request: usize,

    #[serde(default = "default_delay")]
    pub delay_default: u64,

    #[serde(default)]
    pub proxy_platforms: Vec<String>,

    pub api_token: Option<String>,

    #[serde(default = "default_server_port")]
    pub server_port: u16,

    #[serde(default = "default_split_mode")]
    pub split_mode: String,

    #[serde(default = "default_split_time_secs")]
    pub split_time_secs: u64,

    #[serde(default = "default_split_size_mb")]
    pub split_size_mb: u64,

    #[serde(default = "default_split_video_bitrate_kbps")]
    pub split_video_bitrate_kbps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PushConfig {
    #[serde(default)]
    pub push_channels: Vec<String>,
    pub dingtalk_api: Option<String>,
    pub bark_api: Option<String>,
    pub tg_token: Option<String>,
    pub tg_chat_id: Option<String>,
    #[serde(default = "default_false")]
    pub tg_auto_upload: bool,
    pub tg_api_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveUrlConfig {
    #[serde(default)]
    pub url: String,
    pub name: Option<String>,
    pub quality: Option<String>,
    #[serde(default)]
    pub video_save_type: Option<String>,
    #[serde(default = "default_false")]
    pub is_commented: bool,
    #[serde(default)]
    pub split_mode: Option<String>,
    #[serde(default)]
    pub split_custom_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub settings: SettingsConfig,

    #[serde(default)]
    pub cookies: HashMap<String, String>,

    #[serde(default)]
    pub push: PushConfig,

    #[serde(default)]
    pub rooms: Vec<LiveUrlConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            settings: SettingsConfig {
                language: default_language(),
                save_path: default_save_path(),
                engine_paused: false,
                folder_by_author: false,
                folder_by_time: false,
                folder_by_title: false,
                filename_by_title: false,
                video_save_type: default_video_save_type(),
                video_record_quality: default_quality(),
                use_proxy: false,
                proxy_addr: None,
                max_request: default_max_request(),
                delay_default: default_delay(),
                proxy_platforms: Vec::new(),
                api_token: None,
                server_port: default_server_port(),
                split_mode: default_split_mode(),
                split_time_secs: default_split_time_secs(),
                split_size_mb: default_split_size_mb(),
                split_video_bitrate_kbps: default_split_video_bitrate_kbps(),
            },
            cookies: HashMap::new(),
            push: PushConfig {
                push_channels: Vec::new(),
                dingtalk_api: None,
                bark_api: None,
                tg_token: None,
                tg_chat_id: None,
                tg_auto_upload: false,
                tg_api_url: None,
            },
            rooms: Vec::new(),
        }
    }
}

pub fn get_config_paths() -> (PathBuf, PathBuf) {
    // 1. Current working directory config.toml
    if std::path::Path::new("config.toml").exists() {
        if let Ok(p) = std::fs::canonicalize("config.toml") {
            return (p.clone(), p);
        }
        let p = PathBuf::from("./config.toml");
        return (p.clone(), p);
    }

    // 2. ~/.livedownloader/config.toml (User home directory)
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let home_config = PathBuf::from(home).join(".livedownloader").join("config.toml");
        if home_config.exists() {
            return (home_config.clone(), home_config);
        }
    }

    // 3. /var/lib/livedownloader/config.toml (Linux system daemon default)
    let var_lib_config = PathBuf::from("/var/lib/livedownloader/config.toml");
    if var_lib_config.exists() {
        return (var_lib_config.clone(), var_lib_config);
    }

    // 4. BaseDirs config directory (~/.config/LiveDownloader/config.toml or AppData/Roaming/LiveDownloader/config.toml)
    if let Some(base_dirs) = directories::BaseDirs::new() {
        let base_config = base_dirs.config_dir().join("LiveDownloader").join("config.toml");
        if base_config.exists() {
            return (base_config.clone(), base_config);
        }
    }

    // Default to ~/.livedownloader/config.toml if no existing config.toml file was found anywhere
    let home_dir = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));

    let config_dir = home_dir.join(".livedownloader");
    let _ = std::fs::create_dir_all(&config_dir);
    let target_config = config_dir.join("config.toml");
    (target_config.clone(), target_config)
}

pub fn get_downloading_dir(config_toml_path: &Path) -> PathBuf {
    let config_dir = config_toml_path
        .parent()
        .unwrap_or_else(|| Path::new("./config"));
    let downloading_dir = config_dir.join("downloading");
    let _ = std::fs::create_dir_all(&downloading_dir);
    downloading_dir
}

impl AppConfig {
    pub fn load_or_create<P: AsRef<Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = path.as_ref();
        if !path.exists() {
            let config = AppConfig::default();
            config.save_to_file(path)?;
            Ok(config)
        } else {
            Self::load_from_file(path)
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = path.as_ref();
        let toml_str = std::fs::read_to_string(path)?;
        if toml_str.trim().is_empty() {
            return Err("Configuration file is empty or currently being updated".into());
        }
        let mut config: AppConfig = toml::from_str(&toml_str)?;

        if config.settings.save_path.as_os_str().is_empty() {
            config.settings.save_path = default_save_path();
        }

        // Decrypt all cookies loaded from config.toml
        for value in config.cookies.values_mut() {
            *value = decrypt_cookie(value);
        }

        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = path.as_ref();
        let mut config_to_save = self.clone();

        // Encrypt all cookies before saving to config.toml
        for value in config_to_save.cookies.values_mut() {
            *value = encrypt_cookie(value);
        }

        let toml_str = toml::to_string_pretty(&config_to_save)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, toml_str)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

// Custom secure XOR + Base64 encryption/obfuscation helpers for cookies
const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut result = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < input.len() {
        let chunk = &input[i..std::cmp::min(i + 3, input.len())];
        let mut b = 0u32;
        for (j, &byte) in chunk.iter().enumerate() {
            b |= (byte as u32) << (16 - j * 8);
        }
        
        result.push(BASE64_CHARS[(b >> 18 & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[(b >> 12 & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(BASE64_CHARS[(b >> 6 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(BASE64_CHARS[(b & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        i += 3;
    }
    result
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let input = input.trim_end_matches('=');
    let mut result = Vec::with_capacity(input.len() * 3 / 4);
    let mut b = 0u32;
    let mut bits = 0;
    
    for c in input.chars() {
        let val = if c >= 'A' && c <= 'Z' {
            c as u32 - 'A' as u32
        } else if c >= 'a' && c <= 'z' {
            c as u32 - 'a' as u32 + 26
        } else if c >= '0' && c <= '9' {
            c as u32 - '0' as u32 + 52
        } else if c == '+' {
            62
        } else if c == '/' {
            63
        } else {
            return None;
        };
        
        b = (b << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((b >> bits) as u8);
        }
    }
    Some(result)
}

fn encrypt_cookie(val: &str) -> String {
    if val.is_empty() {
        return String::new();
    }
    let key = b"LiveDownloaderSecureSalt123!_CookieKey";
    let input = val.as_bytes();
    let mut encrypted = Vec::with_capacity(input.len());
    for (i, &byte) in input.iter().enumerate() {
        encrypted.push(byte ^ key[i % key.len()]);
    }
    format!("enc_v1:{}", base64_encode(&encrypted))
}

fn decrypt_cookie(val: &str) -> String {
    if !val.starts_with("enc_v1:") {
        return val.to_string();
    }
    let encrypted_base64 = &val[7..];
    let decoded = match base64_decode(encrypted_base64) {
        Some(d) => d,
        None => return val.to_string(),
    };
    let key = b"LiveDownloaderSecureSalt123!_CookieKey";
    let mut decrypted = Vec::with_capacity(decoded.len());
    for (i, &byte) in decoded.iter().enumerate() {
        decrypted.push(byte ^ key[i % key.len()]);
    }
    String::from_utf8(decrypted).unwrap_or_else(|_| val.to_string())
}
