use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{error, info};

// Default value helpers for Serde
fn default_language() -> String {
    "zh_cn".to_string()
}
fn default_save_path() -> PathBuf {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsConfig {
    #[serde(default = "default_language")]
    pub language: String,

    #[serde(default = "default_save_path")]
    pub save_path: PathBuf,

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
    pub url: String,
    pub name: Option<String>,
    pub quality: Option<String>,
    #[serde(default)]
    pub video_save_type: Option<String>,
    #[serde(default = "default_false")]
    pub is_commented: bool,
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
    let config_dir = if let Some(proj_dirs) =
        directories::ProjectDirs::from("com", "LiveDownloader", "LiveDownloader")
    {
        proj_dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from("./config")
    };

    let config_path = config_dir.join("config.toml");
    // We return config_path for both main config and url config since they are merged now.
    (config_path.clone(), config_path)
}

pub fn migrate_old_config(old_ini_path: &Path, new_toml_path: &Path) {
    if old_ini_path.exists() && !new_toml_path.exists() {
        info!("Migrating old config.ini to standard config.toml...");

        // 1. Try to load config.ini
        if let Ok(old_config) = AppConfig::load_from_ini(old_ini_path) {
            let mut rooms = Vec::new();

            // 2. Try to load URL_config.ini
            let old_urls_path = old_ini_path.parent().unwrap().join("URL_config.ini");
            if old_urls_path.exists() {
                if let Ok(urls) = AppConfig::load_urls_from_ini(&old_urls_path) {
                    rooms = urls;
                }
            }

            let mut final_config = old_config;
            final_config.rooms = rooms;

            // 3. Save to new standard config.toml
            if let Err(e) = final_config.save_to_file(new_toml_path) {
                error!(
                    "Failed to save migrated config to {:?}: {}",
                    new_toml_path, e
                );
            } else {
                info!("Successfully migrated config.ini and URL_config.ini to config.toml!");
                // Rename old configuration files to avoid repeating migration
                let _ = std::fs::rename(old_ini_path, old_ini_path.with_extension("ini.bak"));
                if old_urls_path.exists() {
                    let _ =
                        std::fs::rename(&old_urls_path, old_urls_path.with_extension("ini.bak"));
                }
            }
        }
    }
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
        let mut config: AppConfig = toml::from_str(&toml_str)?;

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
        std::fs::write(path, toml_str)?;
        Ok(())
    }

    // Helper functions for parsing old INI configuration files
    fn load_from_ini<P: AsRef<Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let conf = ini::Ini::load_from_file(path)?;
        let section_settings = conf
            .section(Some("录制设置"))
            .ok_or("Missing [录制设置] section")?;

        let language = section_settings
            .get("language(zh_cn/en)")
            .unwrap_or("zh_cn")
            .to_string();
        let save_path_str = section_settings
            .get("直播保存路径(不填则默认)")
            .unwrap_or("")
            .trim();
        let save_path = if save_path_str.is_empty() {
            PathBuf::from("./downloads")
        } else {
            PathBuf::from(save_path_str)
        };

        let parse_yes_no = |val: Option<&str>| -> bool {
            val.map(|v| v.trim() == "是" || v.trim().to_lowercase() == "yes")
                .unwrap_or(false)
        };

        let folder_by_author = parse_yes_no(section_settings.get("保存文件夹是否以作者区分"));
        let folder_by_time = parse_yes_no(section_settings.get("保存文件夹是否以时间区分"));
        let folder_by_title = parse_yes_no(section_settings.get("保存文件夹是否以标题区分"));
        let filename_by_title = parse_yes_no(section_settings.get("保存文件名是否包含标题"));
        let use_proxy = parse_yes_no(section_settings.get("是否使用代理ip(是/否)"));

        let video_save_type = section_settings
            .get("视频保存格式ts|mkv|flv|mp4|mp3音频|m4a音频")
            .unwrap_or("ts")
            .to_string();
        let video_record_quality = section_settings
            .get("原画|超清|高清|标清|流畅")
            .unwrap_or("原画")
            .to_string();

        let proxy_addr = section_settings.get("代理地址").and_then(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        });

        let max_request = section_settings
            .get("同一时间访问网络的线程数")
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(3);

        let delay_default = section_settings
            .get("循环时间(秒)")
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(300);

        let proxy_platforms = section_settings
            .get("使用代理录制的平台(逗号分隔)")
            .map(|s| {
                s.split(',')
                    .map(|p| p.trim().to_lowercase())
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        let mut cookies = HashMap::new();
        if let Some(section_cookies) = conf.section(Some("Cookie")) {
            for (key, val) in section_cookies.iter() {
                if !val.trim().is_empty() {
                    cookies.insert(key.to_string(), val.to_string());
                }
            }
        }

        let mut push_channels = Vec::new();
        let mut dingtalk_api = None;
        let mut bark_api = None;
        if let Some(section_push) = conf.section(Some("推送配置")) {
            if let Some(channels_str) = section_push.get("直播状态推送渠道") {
                push_channels = channels_str
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            dingtalk_api = section_push
                .get("钉钉推送接口链接")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            bark_api = section_push
                .get("bark推送接口链接")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }

        Ok(AppConfig {
            settings: SettingsConfig {
                language,
                save_path,
                folder_by_author,
                folder_by_time,
                folder_by_title,
                filename_by_title,
                video_save_type,
                video_record_quality,
                use_proxy,
                proxy_addr,
                max_request,
                delay_default,
                proxy_platforms,
                api_token: None,
                server_port: default_server_port(),
            },
            cookies,
            push: PushConfig {
                push_channels,
                dingtalk_api,
                bark_api,
                tg_token: None,
                tg_chat_id: None,
                tg_auto_upload: false,
                tg_api_url: None,
            },
            rooms: Vec::new(),
        })
    }

    fn load_urls_from_ini<P: AsRef<Path>>(
        path: P,
    ) -> Result<Vec<LiveUrlConfig>, Box<dyn std::error::Error + Send + Sync>> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut urls = Vec::new();

        for line_res in reader.lines() {
            let line_raw = line_res?;
            let origin_line = line_raw.trim();
            if origin_line.is_empty() {
                continue;
            }

            let is_commented = origin_line.starts_with('#');
            let mut line = if is_commented {
                origin_line.trim_start_matches('#').trim().to_string()
            } else {
                origin_line.to_string()
            };

            if let Some(pos) = line.find("主播: ") {
                line = line[..pos].trim().to_string();
            }

            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = if line.contains(',') {
                line.split(',').map(|s| s.trim()).collect()
            } else if line.contains('，') {
                line.split('，').map(|s| s.trim()).collect()
            } else {
                vec![&line]
            };

            let mut url = String::new();
            let mut name = None;
            let mut quality = None;

            if parts.len() == 1 {
                url = parts[0].to_string();
            } else if parts.len() == 2 {
                let first = parts[0];
                let second = parts[1];

                if first.contains("://")
                    || first.contains("douyin.com")
                    || first.contains("bilibili.com")
                {
                    url = first.to_string();
                    name = Some(second.to_string());
                } else {
                    quality = Some(first.to_string());
                    url = second.to_string();
                }
            } else if parts.len() >= 3 {
                quality = Some(parts[0].to_string());
                url = parts[1].to_string();
                name = Some(parts[2].to_string());
            }

            if !url.starts_with("http://") && !url.starts_with("https://") {
                url = format!("https://{}", url);
            }

            urls.push(LiveUrlConfig {
                url,
                name: if name.as_ref().map_or(true, |n| n.is_empty()) {
                    None
                } else {
                    name
                },
                quality: if quality.as_ref().map_or(true, |q| q.is_empty()) {
                    None
                } else {
                    quality
                },
                video_save_type: None,
                is_commented,
            });
        }

        Ok(urls)
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
