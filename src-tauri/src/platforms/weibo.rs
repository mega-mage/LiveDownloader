use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use regex::Regex;
use crate::common::client::create_http_client;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct WeiboPlatform;

impl WeiboPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LivePlatform for WeiboPlatform {
    fn id(&self) -> &'static str {
        "weibo"
    }

    fn name(&self) -> &'static str {
        "微博直播"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("weibo.com")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("accept", reqwest::header::HeaderValue::from_static("application/json, text/plain, */*"));
        headers.insert("user-agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
        headers.insert("referer", reqwest::header::HeaderValue::from_static("https://weibo.com/"));
        
        // Add a default fallback cookie if none is provided to satisfy Weibo api checks
        let default_cookie = "XSRF-TOKEN=qAP-pIY5V4tO6blNOhA4IIOD; SUB=_2AkMRNMCwf8NxqwFRmfwWymPrbI9-zgzEieKnaDFrJRMxHRl-yT9kqmkhtRB6OrTuX5z9N_7qk9C3xxEmNR-8WLcyo2PM;";
        if let Some(ref cookies) = config.cookie {
            if !cookies.is_empty() {
                headers.insert("Cookie", reqwest::header::HeaderValue::from_str(cookies)?);
            } else {
                headers.insert("Cookie", reqwest::header::HeaderValue::from_static(default_cookie));
            }
        } else {
            headers.insert("Cookie", reqwest::header::HeaderValue::from_static(default_cookie));
        }

        let clean_url = url.split('?').next().ok_or("Empty URL")?;
        let mut room_id = String::new();

        // 1. Resolve room ID from URL format
        if clean_url.contains("show/") {
            room_id = clean_url.split("show/").nth(1).ok_or("Invalid Weibo live link structure")?.to_string();
        } else if clean_url.contains("/l/") {
            // Check direct live_id in query params
            if let Some(caps) = Regex::new(r"live_id=([^&]+)")?.captures(url) {
                room_id = caps.get(1).unwrap().as_str().to_string();
            }
        } else if clean_url.contains("/u/") {
            // Weibo user profile link. Query API to find active live_id
            let uid = clean_url.rsplit("/u/").next().ok_or("Cannot parse Weibo UID")?.to_string();
            let web_api = format!("https://weibo.com/ajax/statuses/mymblog?uid={}&page=1&feature=0", uid);
            
            let resp_str = client.get(&web_api)
                .headers(headers.clone())
                .send()
                .await?
                .text()
                .await?;

            let json_data: Value = serde_json::from_str(&resp_str)?;
            if let Some(list) = json_data["data"]["list"].as_array() {
                for item in list {
                    if item["page_info"]["object_type"].as_str() == Some("live") {
                        if let Some(obj_id) = item["page_info"]["object_id"].as_str() {
                            room_id = obj_id.to_string();
                            break;
                        }
                    }
                }
            }
        }

        if room_id.is_empty() {
            return Ok(LiveStatus::Idle);
        }

        // 2. Fetch live details
        let app_api = format!("https://weibo.com/l/pc/anchor/live?live_id={}", room_id);
        let live_resp_str = client.get(&app_api)
            .headers(headers)
            .send()
            .await?
            .text()
            .await?;

        let live_json: Value = serde_json::from_str(&live_resp_str)?;
        if live_json["data"].is_null() {
            return Ok(LiveStatus::Idle);
        }

        let anchor_name = live_json["data"]["user_info"]["name"].as_str()
            .unwrap_or("Unknown Weibo Anchor")
            .to_string();

        let live_status = live_json["data"]["item"]["status"].as_i64().unwrap_or(0) == 1;
        if !live_status {
            return Ok(LiveStatus::Idle);
        }

        let title = live_json["data"]["item"]["desc"].as_str()
            .unwrap_or("Weibo Live Stream")
            .to_string();

        let pull_urls = &live_json["data"]["item"]["stream_info"]["pull"];
        let m3u8_url = pull_urls["live_origin_hls_url"].as_str().map(|s| s.to_string());
        let flv_url = pull_urls["live_origin_flv_url"].as_str().map(|s| s.to_string());

        let record_url = flv_url.clone().or_else(|| m3u8_url.clone())
            .ok_or("No recordable stream found in Weibo response")?;

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://weibo.com/".to_string());

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
    async fn test_weibo_match_url() {
        let platform = WeiboPlatform::new();
        assert!(platform.match_url("https://weibo.com/u/5885340893"));
        assert!(platform.match_url("https://weibo.com/l/pc/anchor/live?live_id=12345"));
        assert!(!platform.match_url("https://live.bilibili.com/55"));
    }

    #[tokio::test]
    async fn test_weibo_fetch_status() {
        let platform = WeiboPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        // Use a test profile URL
        let result = platform.fetch_status("https://weibo.com/u/5885340893", &config).await;
        assert!(result.is_ok(), "Weibo fetch_status failed: {:?}", result.err());
        let status = result.unwrap();
        match status {
            LiveStatus::Idle => println!("Weibo room is offline"),
            LiveStatus::Living { title, anchor_name, .. } => {
                println!("Weibo room is living: {} by {}", title, anchor_name);
            }
            LiveStatus::Error(e) => println!("Weibo status returned error: {}", e),
        }
    }
}
