use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use regex::Regex;
use crate::common::client::create_http_client;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct KuaishouPlatform;

impl KuaishouPlatform {
    pub fn new() -> Self {
        Self
    }

    // Secondary fallback: Parse the Kuaishou desktop page HTML for stream details
    async fn fetch_via_html(&self, url: &str, client: &reqwest::Client, cookies_str: Option<&str>) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
        headers.insert("Accept-Language", reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9"));
        if let Some(cookies) = cookies_str {
            headers.insert("Cookie", reqwest::header::HeaderValue::from_str(cookies)?);
        }

        let html = client.get(url)
            .headers(headers)
            .send()
            .await?
            .text()
            .await?;

        // Extract window.__INITIAL_STATE__ JSON
        let re_state = Regex::new(r#"<script>window\.__INITIAL_STATE__=(.*?);\(function\(\)\{var s;"#)?;
        let caps = re_state.captures(&html).ok_or("Kuaishou INITIAL_STATE not found")?;
        let state_json_str = caps.get(1).ok_or("Kuaishou state capture empty")?.as_str();

        // Extract liveStream slice
        let re_livestream = Regex::new(r#"(\{"liveStream".*?),"gameInfo"#)?;
        let stream_caps = re_livestream.captures(state_json_str).ok_or("Kuaishou liveStream object not found")?;
        let livestream_json_str = format!("{}}}", stream_caps.get(1).ok_or("Kuaishou liveStream capture empty")?.as_str());

        let play_list: Value = serde_json::from_str(&livestream_json_str)?;

        if play_list.get("liveStream").is_none() || play_list["liveStream"].is_null() {
            return Ok(LiveStatus::Idle);
        }

        let anchor_name = play_list["author"]["name"].as_str()
            .unwrap_or("Unknown Kuaishou Anchor")
            .to_string();

        let live_stream = &play_list["liveStream"];
        let is_living = live_stream["isLiving"].as_bool().unwrap_or(false) || !live_stream["playUrls"].is_null();
        if !is_living {
            return Ok(LiveStatus::Idle);
        }

        let title = live_stream["caption"].as_str()
            .unwrap_or("Kuaishou Live Stream")
            .to_string();

        let mut flv_url = None;
        let mut m3u8_url = None;

        // Try to parse H264 stream URLs
        if let Some(representations) = live_stream["playUrls"]["h264"]["adaptationSet"]["representation"].as_array() {
            for rep in representations {
                let url_str = rep["url"].as_str().unwrap_or("");
                if !url_str.is_empty() {
                    if url_str.contains(".flv") {
                        flv_url = Some(url_str.to_string());
                    } else if url_str.contains(".m3u8") {
                        m3u8_url = Some(url_str.to_string());
                    }
                }
            }
        }

        let record_url = match flv_url.clone().or_else(|| m3u8_url.clone()) {
            Some(u) => u,
            None => return Ok(LiveStatus::Idle),
        };

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://live.kuaishou.com/".to_string());

        Ok(LiveStatus::Living {
            title,
            anchor_name,
            stream_urls: StreamUrls {
                m3u8_url: m3u8_url.clone().or_else(|| Some(record_url.clone())),
                flv_url,
                record_url,
                headers: Some(custom_headers),
            },
        })
    }
}

#[async_trait]
impl LivePlatform for KuaishouPlatform {
    fn id(&self) -> &'static str {
        "kuaishou"
    }

    fn name(&self) -> &'static str {
        "快手直播"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("kuaishou.com")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;
        let cookies_str = config.cookie.as_deref();

        // Standard headers for Kuaishou mobile API
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("ios/7.830 (ios 17.0; ; iPhone 15 (A2846/A3089/A3090/A3092))"));
        headers.insert("Accept-Language", reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.8,zh-TW;q=0.7"));
        headers.insert("content-type", reqwest::header::HeaderValue::from_static("application/json"));
        headers.insert("Referer", reqwest::header::HeaderValue::from_static("https://www.kuaishou.com/short-video/3x224rwabjmuc9y?fid=1712760877&cc=share_copylink&followRefer=151&shareMethod=TOKEN&docId=9&kpn=KUAISHOU&subBiz=BROWSE_SLIDE_PHOTO"));
        headers.insert("Cookie", reqwest::header::HeaderValue::from_static("did=web_e988652e11b545469633396abe85a89f; didv=1796004001000"));

        if let Some(cookies) = cookies_str {
            if !cookies.is_empty() {
                headers.insert("Cookie", reqwest::header::HeaderValue::from_str(cookies)?);
            }
        }

        // Try mobile API first. We extract author ID (eid) from URL like: kuaishou.com/u/xxxx
        let clean_url = url.split('?').next().ok_or("Empty URL")?;
        let parts: Vec<&str> = clean_url.split("/u/").collect();
        if parts.len() < 2 {
            // If no "/u/" found (e.g. short share URL or search URL), fallback to HTML parsing directly
            return self.fetch_via_html(url, &client, cookies_str).await;
        }

        let eid = parts[1].trim();
        let payload = serde_json::json!({
            "source": 5,
            "eid": eid,
            "shareMethod": "card",
            "clientType": "WEB_OUTSIDE_SHARE_H5"
        });

        let app_api = "https://livev.m.chenzhongtech.com/rest/k/live/byUser?kpn=GAME_ZONE&captchaToken=";
        let response = client.post(app_api)
            .headers(headers)
            .json(&payload)
            .send()
            .await?;

        let json_data: Value = match response.json().await {
            Ok(data) => data,
            Err(_) => {
                return self.fetch_via_html(url, &client, cookies_str).await;
            }
        };
        let live_stream = &json_data["liveStream"];

        if live_stream.is_null() || live_stream.get("user").is_none() {
            // Fallback to HTML if liveStream is null in API response
            return self.fetch_via_html(url, &client, cookies_str).await;
        }

        let anchor_name = live_stream["user"]["user_name"].as_str()
            .unwrap_or("Unknown Kuaishou Anchor")
            .to_string();

        let is_living = live_stream["living"].as_bool().unwrap_or(false);
        if !is_living {
            return Ok(LiveStatus::Idle);
        }

        let title = live_stream["caption"].as_str()
            .unwrap_or("Kuaishou Live Stream")
            .to_string();

        let backup_m3u8_url = live_stream["hlsPlayUrl"].as_str().map(|s| s.to_string());
        let backup_flv_url = live_stream["playUrls"][0]["url"].as_str().map(|s| s.to_string());

        let mut flv_url = backup_flv_url.clone();
        let mut m3u8_url = backup_m3u8_url.clone();

        // Get higher quality stream list if available
        if let Some(multi_res_urls) = live_stream["multiResolutionPlayUrls"][0]["urls"].as_array() {
            if !multi_res_urls.is_empty() {
                flv_url = multi_res_urls[0]["url"].as_str().map(|s| s.to_string());
            }
        }
        if let Some(multi_res_hls_urls) = live_stream["multiResolutionHlsPlayUrls"][0]["urls"].as_array() {
            if !multi_res_hls_urls.is_empty() {
                m3u8_url = multi_res_hls_urls[0]["url"].as_str().map(|s| s.to_string());
            }
        }

        let record_url = flv_url.clone().or_else(|| m3u8_url.clone())
            .ok_or("No recordable Kuaishou stream found in mobile API response")?;

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://live.kuaishou.com/".to_string());

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
    async fn test_kuaishou_match_url() {
        let platform = KuaishouPlatform::new();
        assert!(platform.match_url("https://live.kuaishou.com/u/3x33333"));
        assert!(platform.match_url("https://v.kuaishou.com/xxxx"));
        assert!(!platform.match_url("https://www.huya.com/lpl"));
    }

    #[tokio::test]
    async fn test_kuaishou_fetch_status() {
        let platform = KuaishouPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        // Use a test url
        let result = platform.fetch_status("https://live.kuaishou.com/u/3xtnuitaz2982eg", &config).await;
        assert!(result.is_ok(), "Kuaishou fetch_status failed: {:?}", result.err());
        let status = result.unwrap();
        match status {
            LiveStatus::Idle => println!("Kuaishou room is offline"),
            LiveStatus::Living { title, anchor_name, .. } => {
                println!("Kuaishou room is living: {} by {}", title, anchor_name);
            }
            LiveStatus::Error(e) => println!("Kuaishou status returned error: {}", e),
        }
    }
}
