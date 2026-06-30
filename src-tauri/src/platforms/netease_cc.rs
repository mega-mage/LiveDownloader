use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use regex::Regex;
use crate::common::client::create_http_client;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct NeteaseCcPlatform;

impl NeteaseCcPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LivePlatform for NeteaseCcPlatform {
    fn id(&self) -> &'static str {
        "netease_cc"
    }

    fn name(&self) -> &'static str {
        "网易CC直播"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("cc.163.com")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("accept", reqwest::header::HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8"));
        headers.insert("referer", reqwest::header::HeaderValue::from_static("https://cc.163.com/"));
        headers.insert("user-agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));

        if let Some(ref cookies) = config.cookie {
            if !cookies.is_empty() {
                headers.insert("Cookie", reqwest::header::HeaderValue::from_str(cookies)?);
            }
        }

        // Standardize URL: ensure trailing slash
        let clean_url = url.split('?').next().ok_or("Empty URL")?;
        let formatted_url = if clean_url.ends_with('/') {
            clean_url.to_string()
        } else {
            format!("{}/", clean_url)
        };

        let html = client.get(&formatted_url)
            .headers(headers)
            .send()
            .await?
            .text()
            .await?;

        // Extract NEXT_DATA script block
        let re_data = Regex::new(r#"<script id="__NEXT_DATA__"\s+type="application/json"\s*[^>]*>(.*?)</script></body>"#)?;
        let caps = re_data.captures(&html).ok_or("NetEase CC NEXT_DATA not found in page HTML")?;
        let json_str = caps.get(1).ok_or("NetEase CC JSON extract empty")?.as_str();

        let json_data: Value = serde_json::from_str(json_str)?;
        let room_data = &json_data["props"]["pageProps"]["roomInfoInitData"];
        let live_data = &room_data["live"];

        if live_data.is_null() {
            return Ok(LiveStatus::Idle);
        }

        let live_status = live_data["status"].as_i64().unwrap_or(0) == 1;
        let anchor_name = live_data["nickname"].as_str()
            .or_else(|| room_data["nickname"].as_str())
            .unwrap_or("Unknown CC Anchor")
            .to_string();

        if !live_status {
            return Ok(LiveStatus::Idle);
        }

        let title = live_data["title"].as_str().unwrap_or("NetEase CC Live").to_string();
        let m3u8_url = live_data["sharefile"].as_str().map(|s| s.to_string());
        let mut flv_url = None;

        // Try to extract resolution streams
        if let Some(resolutions) = live_data["quickplay"]["resolution"].as_object() {
            // Find a stream based on priority: blueray > ultra > high > standard
            let order = vec!["blueray", "ultra", "high", "standard"];
            for &q in &order {
                if let Some(res_obj) = resolutions.get(q) {
                    if let Some(cdn_list) = res_obj["cdn"].as_object() {
                        if let Some((_, cdn_url_val)) = cdn_list.iter().next() {
                            if let Some(url_str) = cdn_url_val.as_str() {
                                flv_url = Some(url_str.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }

        let record_url = flv_url.clone().or_else(|| m3u8_url.clone())
            .ok_or("No recordable stream found in NetEase CC response")?;

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://cc.163.com/".to_string());

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
    async fn test_netease_cc_match_url() {
        let platform = NeteaseCcPlatform::new();
        assert!(platform.match_url("https://cc.163.com/361433"));
        assert!(!platform.match_url("https://live.bilibili.com/55"));
    }

    #[tokio::test]
    async fn test_netease_cc_fetch_status() {
        let platform = NeteaseCcPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        // Use a test room ID on CC
        let result = platform.fetch_status("https://cc.163.com/361433", &config).await;
        assert!(result.is_ok(), "NetEase CC fetch_status failed: {:?}", result.err());
        let status = result.unwrap();
        match status {
            LiveStatus::Idle => println!("NetEase CC room is offline"),
            LiveStatus::Living { title, anchor_name, .. } => {
                println!("NetEase CC room is living: {} by {}", title, anchor_name);
            }
            LiveStatus::Error(e) => println!("NetEase CC status returned error: {}", e),
        }
    }
}
