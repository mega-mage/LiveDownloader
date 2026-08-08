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
fn default_split_size_mb() -> u64 {
    1024
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

    #[serde(default = "default_split_size_mb")]
    pub split_size_mb: u64,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomsConfig {
    #[serde(default)]
    pub rooms: Vec<LiveUrlConfig>,
}

impl RoomsConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(toml_str) => {
                    if toml_str.trim().is_empty() {
                        tracing::warn!("[ROOMS_LOAD] rooms.toml at {:?} exists but is EMPTY ({} bytes raw)", path, toml_str.len());
                        return RoomsConfig::default();
                    }
                    match toml::from_str::<RoomsConfig>(&toml_str) {
                        Ok(cfg) => {
                            tracing::debug!("[ROOMS_LOAD] Loaded {} rooms from {:?}", cfg.rooms.len(), path);
                            if cfg.rooms.is_empty() {
                                tracing::warn!("[ROOMS_LOAD] rooms.toml parsed successfully but contains 0 rooms!");
                            }
                            return cfg;
                        }
                        Err(e) => {
                            tracing::error!("[ROOMS_LOAD] Failed to parse rooms.toml at {:?}: {}", path, e);
                            tracing::debug!("[ROOMS_LOAD] rooms.toml content ({} bytes): {:?}", toml_str.len(), &toml_str[..toml_str.len().min(500)]);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("[ROOMS_LOAD] Failed to read rooms.toml at {:?}: {}", path, e);
                }
            }
        } else {
            tracing::info!("[ROOMS_LOAD] rooms.toml does not exist at {:?}, returning empty", path);
        }
        RoomsConfig::default()
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = path.as_ref();
        let toml_str = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, toml_str)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub settings: SettingsConfig,

    #[serde(default)]
    pub cookies: HashMap<String, String>,

    #[serde(default)]
    pub push: PushConfig,

    #[serde(skip_serializing, default)]
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
                split_size_mb: default_split_size_mb(),
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

pub fn get_user_home_dir() -> PathBuf {
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        let trimmed = sudo_user.trim();
        if !trimmed.is_empty() && trimmed != "root" {
            #[cfg(target_os = "macos")]
            let user_home = PathBuf::from("/Users").join(trimmed);
            #[cfg(not(target_os = "macos"))]
            let user_home = PathBuf::from("/home").join(trimmed);

            if user_home.exists() {
                return user_home;
            }
        }
    }

    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn get_config_paths() -> (PathBuf, PathBuf) {
    let home_dir = get_user_home_dir();
    let config_dir = home_dir.join(".livedownloader");
    let _ = std::fs::create_dir_all(&config_dir);
    let target_config = config_dir.join("config.toml");
    let target_rooms = config_dir.join("rooms.toml");
    (target_config, target_rooms)
}

pub fn get_downloading_dir(config_toml_path: &Path) -> PathBuf {
    let config_dir = config_toml_path
        .parent()
        .unwrap_or_else(|| Path::new("./config"));
    let downloading_dir = config_dir.join("downloading");
    if let Err(e) = std::fs::create_dir_all(&downloading_dir) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            tracing::warn!("Permission denied creating {:?}, falling back to temp downloading directory", downloading_dir);
            let fallback = std::env::temp_dir().join("livedownloader").join("downloading");
            let _ = std::fs::create_dir_all(&fallback);
            return fallback;
        }
    }
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

        // Always load rooms from rooms.toml in the same directory
        let rooms_path = path.with_file_name("rooms.toml");
        let rooms_cfg = RoomsConfig::load_from_file(&rooms_path);
        config.rooms = rooms_cfg.rooms;

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

        // Exclusively save rooms to rooms.toml
        let rooms_path = path.with_file_name("rooms.toml");
        let rooms_cfg = RoomsConfig {
            rooms: self.rooms.clone(),
        };
        rooms_cfg.save_to_file(&rooms_path)?;

        Ok(())
    }
    pub fn get_cookie_for_platform(&self, platform_id: &str) -> Option<String> {
        let id_lower = platform_id.to_lowercase();
        let keys_to_try: &[&str] = match id_lower.as_str() {
            "douyin" => &["douyin", "抖音", "抖音cookie", "douyin_cookie"],
            "bilibili" => &["bilibili", "b站", "b站cookie", "bilibili_cookie"],
            "huya" => &["huya", "虎牙", "虎牙cookie", "huya_cookie"],
            "kuaishou" => &["kuaishou", "快手", "快手cookie", "kuaishou_cookie"],
            "douyu" => &["douyu", "斗鱼", "斗鱼cookie", "douyu_cookie"],
            "maoerfm" => &["maoerfm", "猫耳", "猫耳cookie", "maoer_cookie"],
            "netease_cc" => &["netease_cc", "网易cc", "网易cccookie", "netease_cookie"],
            "weibo" => &["weibo", "微博", "微博cookie", "weibo_cookie"],
            "taobao" => &["taobao", "淘宝", "淘宝cookie", "taobao_cookie"],
            "acfun" => &["acfun", "a站", "A站cookie", "acfun_cookie"],
            "twitch" => &["twitch", "Twitchcookie", "twitch_cookie"],
            _ => &[platform_id],
        };

        for key in keys_to_try {
            if let Some(val) = self.cookies.get(*key) {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        if let Some(val) = self.cookies.get(platform_id) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        None
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
    let trimmed = val.trim();
    if trimmed.is_empty() || trimmed.starts_with("enc_v1:") {
        return trimmed.to_string();
    }
    let key = b"LiveDownloaderSecureSalt123!_CookieKey";
    let input = trimmed.as_bytes();
    let mut encrypted = Vec::with_capacity(input.len());
    for (i, &byte) in input.iter().enumerate() {
        encrypted.push(byte ^ key[i % key.len()]);
    }
    format!("enc_v1:{}", base64_encode(&encrypted))
}

fn decrypt_cookie(val: &str) -> String {
    let mut cur = val.trim().to_string();
    while cur.starts_with("enc_v1:") {
        let encrypted_base64 = &cur[7..];
        let decoded = match base64_decode(encrypted_base64) {
            Some(d) => d,
            None => break,
        };
        let key = b"LiveDownloaderSecureSalt123!_CookieKey";
        let mut decrypted = Vec::with_capacity(decoded.len());
        for (i, &byte) in decoded.iter().enumerate() {
            decrypted.push(byte ^ key[i % key.len()]);
        }
        match String::from_utf8(decrypted) {
            Ok(s) => cur = s,
            Err(_) => break,
        }
    }
    cur
}
