use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use crate::common::client::create_http_client;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct MaoerfmPlatform;

impl MaoerfmPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LivePlatform for MaoerfmPlatform {
    fn id(&self) -> &'static str {
        "maoerfm"
    }

    fn name(&self) -> &'static str {
        "猫耳FM"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("fm.missevan.com")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;
        
        let clean_url = url.split('?').next().ok_or("Empty URL")?;
        let room_id = clean_url.rsplit('/').next().ok_or("Cannot parse room ID from URL")?.to_string();

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("accept", reqwest::header::HeaderValue::from_static("application/json, text/plain, */*"));
        headers.insert("referer", reqwest::header::HeaderValue::from_str(&format!("https://fm.missevan.com/live/{}", room_id))?);
        headers.insert("user-agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));

        if let Some(ref cookies) = config.cookie {
            if !cookies.is_empty() {
                headers.insert("Cookie", reqwest::header::HeaderValue::from_str(cookies)?);
            }
        }

        let api_url = format!("https://fm.missevan.com/api/v2/live/{}", room_id);
        let resp_str = client.get(&api_url)
            .headers(headers)
            .send()
            .await?
            .text()
            .await?;

        let json_data: Value = serde_json::from_str(&resp_str)?;

        let creator = &json_data["info"]["creator"];
        let anchor_name = creator["username"].as_str().unwrap_or("Unknown Maoer Anchor").to_string();

        let room_info = &json_data["info"]["room"];
        if room_info.is_null() {
            return Ok(LiveStatus::Idle);
        }

        let live_status = room_info["status"]["broadcasting"].as_bool()
            .or_else(|| room_info["status"]["broadcasting"].as_i64().map(|v| v == 1))
            .unwrap_or(false);

        if !live_status {
            return Ok(LiveStatus::Idle);
        }

        let title = room_info["name"].as_str().unwrap_or("Maoer FM Live").to_string();
        
        let channel = &room_info["channel"];
        let m3u8_url = channel["hls_pull_url"].as_str().map(|s| s.to_string());
        let flv_url = channel["flv_pull_url"].as_str().map(|s| s.to_string());

        let record_url = flv_url.clone().or_else(|| m3u8_url.clone())
            .ok_or("No recordable stream found in Maoer FM response")?;

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://fm.missevan.com/".to_string());

        Ok(LiveStatus::Living {
            title,
            anchor_name,
            stream_urls: StreamUrls {
                m3u8_url: m3u8_url.or_else(|| Some(record_url.clone())),
                flv_url,
                record_url,
                headers: Some(custom_headers),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_maoerfm_match_url() {
        let platform = MaoerfmPlatform::new();
        assert!(platform.match_url("https://fm.missevan.com/live/868895007"));
        assert!(!platform.match_url("https://live.bilibili.com/55"));
    }

    #[tokio::test]
    async fn test_maoerfm_fetch_status() {
        let platform = MaoerfmPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        // Use a test room ID from the python project referer
        let result = platform.fetch_status("https://fm.missevan.com/live/868895007", &config).await;
        assert!(result.is_ok(), "Maoer FM fetch_status failed: {:?}", result.err());
        let status = result.unwrap();
        match status {
            LiveStatus::Idle => println!("Maoer FM room is offline"),
            LiveStatus::Living { title, anchor_name, .. } => {
                println!("Maoer FM room is living: {} by {}", title, anchor_name);
            }
            LiveStatus::Error(e) => println!("Maoer FM status returned error: {}", e),
        }
    }
}
