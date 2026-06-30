use crate::platforms::douyin_sign;

use crate::platforms::{LivePlatform, LiveStatus, StreamUrls, PlatformConfig};
use crate::common::client::create_http_client;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, REFERER, USER_AGENT};
use serde_json::Value;
use std::collections::HashMap;
use url::form_urlencoded;

pub struct DouyinPlatform;

impl DouyinPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LivePlatform for DouyinPlatform {
    fn id(&self) -> &'static str {
        "douyin"
    }

    fn name(&self) -> &'static str {
        "抖音直播"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("live.douyin.com") && !url.contains("v.douyin.com") && !url.contains("/user/")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let web_rid = extract_web_rid(url)?;
        
        let client = create_http_client(config.proxy.as_deref(), 10)?;
        
        let ua = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.5845.97 Safari/537.36 Core/1.116.567.400 QQBrowser/19.7.6764.400";
        let default_cookie = "ttwid=1%7C2iDIYVmjzMcpZ20fcaFde0VghXAA3NaNXE_SLR68IyE%7C1761045455%7Cab35197d5cfb21df6cbb2fa7ef1c9262206b062c315b9d04da746d0b37dfbc7d";
        
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(ua));
        headers.insert(REFERER, HeaderValue::from_str(&format!("https://live.douyin.com/{}", web_rid))?);
        
        let cookie_str = config.cookie.as_deref().unwrap_or(default_cookie);
        headers.insert(COOKIE, HeaderValue::from_str(cookie_str)?);
        
        // 1. Construct parameters for enter API
        let params = [
            ("aid", "6383"),
            ("app_name", "douyin_web"),
            ("live_id", "1"),
            ("device_platform", "web"),
            ("language", "zh-CN"),
            ("browser_language", "zh-CN"),
            ("browser_platform", "Win32"),
            ("browser_name", "Chrome"),
            ("browser_version", "116.0.0.0"),
            ("web_rid", &web_rid),
            ("msToken", ""),
        ];
        
        let query_str = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params.iter())
            .finish();
            
        // 2. Generate a_bogus signature
        let a_bogus = douyin_sign::ab_sign(&query_str, ua);
        
        let enter_url = format!(
            "https://live.douyin.com/webcast/room/web/enter/?{}&a_bogus={}",
            query_str, a_bogus
        );
        
        // 3. Send request
        let resp = client.get(&enter_url)
            .headers(headers)
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        let data = &resp["data"];
        if data.is_null() || data["data"].is_null() || data["data"].as_array().map_or(true, |a| a.is_empty()) {
            return Ok(LiveStatus::Error("Douyin API returned empty room data (risk control triggered?)".to_string()));
        }
        
        let room_data = &data["data"][0];
        let anchor_name = data["user"]["nickname"].as_str()
            .unwrap_or("Unknown Anchor")
            .to_string();
            
        let status = room_data["status"].as_i64().unwrap_or(4);
        if status != 2 {
            // Anchor is not living
            return Ok(LiveStatus::Idle);
        }
        
        let title = room_data["title"].as_str().unwrap_or("Douyin Live Room").to_string();
        
        // 4. Resolve quality QN and select stream
        let stream_url_info = &room_data["stream_url"];
        let flv_url_dict = &stream_url_info["flv_pull_url"];
        let m3u8_url_dict = &stream_url_info["hls_pull_url_map"];
        
        // Collect stream URL pairs
        // Douyin typically provides: FULL_HD1, HD1, SD1, SD2 etc.
        // We will select stream based on quality configuration.
        // If config.quality is "原画" or empty, we will look for: FULL_HD1 or key with largest resolution
        let select_key = match config.quality.as_str() {
            "原画" => "FULL_HD1",
            "超清" => "HD1",
            "高清" => "SD1",
            "标清" => "SD2",
            "流畅" => "SD2",
            _ => "FULL_HD1",
        };
        
        // Extract FLV and M3U8 URLs
        let mut flv_url = flv_url_dict[select_key].as_str()
            .or_else(|| flv_url_dict.as_object().and_then(|obj| obj.values().next().and_then(|v| v.as_str())))
            .map(|s| s.to_string());
            
        let mut m3u8_url = m3u8_url_dict[select_key].as_str()
            .or_else(|| m3u8_url_dict.as_object().and_then(|obj| obj.values().next().and_then(|v| v.as_str())))
            .map(|s| s.to_string());
            
        // If there's an origin stream (higher quality), try to parse it (equivalent to lines 125-137 in spider.py)
        if let Some(sdk_data_str) = stream_url_info["live_core_sdk_data"]["pull_data"]["stream_data"].as_str() {
            if let Ok(sdk_data) = serde_json::from_str::<Value>(sdk_data_str) {
                if let Some(origin) = sdk_data["data"]["origin"]["main"].as_object() {
                    let vcodec = sdk_data["data"]["origin"]["main"]["sdk_params"]["VCodec"].as_str().unwrap_or("");
                    if let Some(hls) = origin.get("hls").and_then(|h| h.as_str()) {
                        m3u8_url = Some(format!("{}&codec={}", hls, vcodec));
                    }
                    if let Some(flv) = origin.get("flv").and_then(|f| f.as_str()) {
                        flv_url = Some(format!("{}&codec={}", flv, vcodec));
                    }
                }
            }
        }
        
        let record_url = m3u8_url.clone().or_else(|| flv_url.clone())
            .ok_or("No recordable stream URL found")?;
            
        // Prepare headers for ffmpeg recording
        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), ua.to_string());
        custom_headers.insert("Cookie".to_string(), cookie_str.to_string());
        
        Ok(LiveStatus::Living {
            title,
            anchor_name,
            stream_urls: StreamUrls {
                m3u8_url,
                flv_url,
                record_url,
                headers: Some(custom_headers),
            },
        })
    }
}

fn extract_web_rid(url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let clean_url = url.split('?').next().ok_or("Empty URL")?;
    let last_part = clean_url.rsplit('/').next().ok_or("Cannot extract web_rid from URL")?;
    Ok(last_part.to_string())
}
