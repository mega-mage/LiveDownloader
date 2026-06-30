use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use regex::Regex;
use crate::common::client::create_http_client;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct HuyaPlatform;

impl HuyaPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LivePlatform for HuyaPlatform {
    fn id(&self) -> &'static str {
        "huya"
    }

    fn name(&self) -> &'static str {
        "虎牙直播"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("huya.com")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;

        // Standard headers mimicking WeChat Mini Program request
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("ios/7.830 (ios 17.0; ; iPhone 15 (A2846/A3089/A3090/A3092))"));
        headers.insert("xweb_xhr", reqwest::header::HeaderValue::from_static("1"));
        headers.insert("referer", reqwest::header::HeaderValue::from_static("https://servicewechat.com/wx74767bf0b684f7d3/301/page-frame.html"));
        headers.insert("accept-language", reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9"));

        if let Some(ref cookies) = config.cookie {
            if !cookies.is_empty() {
                headers.insert("Cookie", reqwest::header::HeaderValue::from_str(cookies)?);
            }
        }

        // Extract raw room ID from URL path (e.g. huya.com/12345 or huya.com/lpl)
        let clean_url = url.split('?').next().ok_or("Empty URL")?;
        let mut room_id = clean_url.rsplit('/').next().ok_or("Cannot parse room ID from URL")?.to_string();

        // If room_id has alphabetic characters, we must query the page HTML to find the numeric room ID
        let has_alpha = room_id.chars().any(|c| c.is_alphabetic());
        if has_alpha {
            let mut page_headers = reqwest::header::HeaderMap::new();
            page_headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
            
            let html = client.get(clean_url)
                .headers(page_headers)
                .send()
                .await?
                .text()
                .await?;
                
            let re = Regex::new(r#"ProfileRoom":(.*?),"sPrivateHost"#)?;
            if let Some(caps) = re.captures(&html) {
                if let Some(m) = caps.get(1) {
                    room_id = m.as_str().to_string();
                }
            } else {
                return Ok(LiveStatus::Error("Failed to extract numeric room ID from Huya page. Please try using a numeric URL.".to_string()));
            }
        }

        // Query the profileRoom WeChat mini-program cache endpoint
        let api_url = format!("https://mp.huya.com/cache.php?m=Live&do=profileRoom&roomid={}&showSecret=1", room_id);
        let resp_str = client.get(&api_url)
            .headers(headers)
            .send()
            .await?
            .text()
            .await?;

        let json_data: Value = serde_json::from_str(&resp_str)?;

        let anchor_name = json_data["data"]["profileInfo"]["nick"].as_str()
            .unwrap_or("Unknown Anchor")
            .to_string();

        let live_status = json_data["data"]["realLiveStatus"].as_str().unwrap_or("OFF");
        if live_status != "ON" {
            return Ok(LiveStatus::Idle);
        }

        let title = json_data["data"]["liveData"]["introduction"].as_str()
            .unwrap_or("Huya Live Stream")
            .to_string();

        let stream_info_list = json_data["data"]["stream"]["baseSteamInfoList"].as_array()
            .ok_or("stream info list missing in Huya API response")?;

        if stream_info_list.is_empty() {
            return Ok(LiveStatus::Idle);
        }

        // Extract available HLS and FLV urls and rank by CDN priority
        let mut play_urls = Vec::new();
        for stream in stream_info_list {
            let cdn_type = stream["sCdnType"].as_str().unwrap_or("");
            let stream_name = stream["sStreamName"].as_str().unwrap_or("");
            let s_flv_url = stream["sFlvUrl"].as_str().unwrap_or("");
            let flv_anti_code = stream["sFlvAntiCode"].as_str().unwrap_or("");
            let s_hls_url = stream["sHlsUrl"].as_str().unwrap_or("");
            let hls_anti_code = stream["sHlsAntiCode"].as_str().unwrap_or("");

            let m3u8_url = format!("{}/{}.m3u8?{}", s_hls_url, stream_name, hls_anti_code);
            let flv_url = format!("{}/{}.flv?{}", s_flv_url, stream_name, flv_anti_code);

            play_urls.push((cdn_type.to_string(), m3u8_url, flv_url));
        }

        // CDNs priority: Tencent (TX), Huawei (HW), Wangsu (HS), Alibaba (AL)
        let priority_order = vec!["TX", "HW", "HS", "AL"];
        let mut selected_flv_url = None;
        let mut selected_m3u8_url = None;
        let mut selected_cdn = None;

        for &target_cdn in &priority_order {
            for (cdn, m3u8, flv) in &play_urls {
                if cdn == target_cdn {
                    selected_flv_url = Some(flv.clone());
                    selected_m3u8_url = Some(m3u8.clone());
                    selected_cdn = Some(cdn.clone());
                    break;
                }
            }
            if selected_flv_url.is_some() {
                break;
            }
        }

        // Fallback to first if priority CDN not found
        let (final_cdn, mut final_m3u8_url, mut final_flv_url) = if let (Some(cdn), Some(m3u8), Some(flv)) = (selected_cdn, selected_m3u8_url, selected_flv_url) {
            (cdn, m3u8, flv)
        } else {
            let (cdn, m3u8, flv) = &play_urls[0];
            (cdn.clone(), m3u8.clone(), flv.clone())
        };

        // Ensure use HTTPS
        if final_m3u8_url.starts_with("http://") {
            final_m3u8_url = final_m3u8_url.replace("http://", "https://");
        }
        if final_flv_url.starts_with("http://") {
            final_flv_url = final_flv_url.replace("http://", "https://");
        }

        // If it's Tencent CDN, perform extra parameters replacement as original spider.py
        if final_cdn == "TX" {
            final_flv_url = final_flv_url.replace("&ctype=tars_mp", "&ctype=huya_webh5").replace("&fs=bhct", "&fs=bgct");
            final_m3u8_url = final_m3u8_url.replace("&ctype=tars_mp", "&ctype=huya_webh5").replace("&fs=bhct", "&fs=bgct");
        }

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://www.huya.com/".to_string());

        Ok(LiveStatus::Living {
            title,
            anchor_name,
            stream_urls: StreamUrls {
                m3u8_url: Some(final_m3u8_url.clone()),
                flv_url: Some(final_flv_url.clone()),
                record_url: final_flv_url,
                headers: Some(custom_headers),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_huya_match_url() {
        let platform = HuyaPlatform::new();
        assert!(platform.match_url("https://www.huya.com/11342411"));
        assert!(platform.match_url("https://huya.com/lpl"));
        assert!(!platform.match_url("https://live.bilibili.com/55"));
    }

    #[tokio::test]
    async fn test_huya_fetch_status() {
        let platform = HuyaPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        // Use a popular room like LPL which is stable
        let result = platform.fetch_status("https://www.huya.com/lpl", &config).await;
        assert!(result.is_ok(), "Huya fetch_status failed: {:?}", result.err());
        let status = result.unwrap();
        match status {
            LiveStatus::Idle => println!("Huya room lpl is offline"),
            LiveStatus::Living { title, anchor_name, .. } => {
                println!("Huya room lpl is living: {} by {}", title, anchor_name);
            }
            LiveStatus::Error(e) => println!("Huya status returned error: {}", e),
        }
    }
}
