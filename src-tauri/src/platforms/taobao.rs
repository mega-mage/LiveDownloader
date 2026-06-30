use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use regex::Regex;
use crate::common::client::create_http_client;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct TaobaoPlatform;

impl TaobaoPlatform {
    pub fn new() -> Self {
        Self
    }

    // Convert JSONP format like callback_name({...}) into raw JSON string
    fn jsonp_to_json(&self, jsonp: &str) -> Option<String> {
        let start = jsonp.find('(')?;
        let end = jsonp.rfind(')')?;
        if start < end {
            Some(jsonp[start + 1..end].to_string())
        } else {
            None
        }
    }

    // Extract cookie value by name from Cookie header string
    fn get_cookie_value(&self, cookie_str: &str, name: &str) -> Option<String> {
        let re = Regex::new(&format!(r"{}=([^;]+)", name)).ok()?;
        let caps = re.captures(cookie_str)?;
        Some(caps.get(1)?.as_str().trim().to_string())
    }
}

#[async_trait]
impl LivePlatform for TaobaoPlatform {
    fn id(&self) -> &'static str {
        "taobao"
    }

    fn name(&self) -> &'static str {
        "淘宝直播"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("taobao.com")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;

        let mut cookie_header = config.cookie.clone().unwrap_or_default();
        if cookie_header.is_empty() {
            // A placeholder is needed to trigger token extraction
            cookie_header = "_m_h5_tk=placeholder_12345; _m_h5_tk_enc=placeholder_enc;".to_string();
        }

        // 1. Resolve liveId from URL
        let mut live_id = String::new();
        if let Some(caps) = Regex::new(r"id=([^&]+)")?.captures(url) {
            live_id = caps.get(1).unwrap().as_str().to_string();
        } else {
            // Fetch page and find redirect
            let mut page_headers = reqwest::header::HeaderMap::new();
            page_headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
            page_headers.insert("Referer", reqwest::header::HeaderValue::from_static("https://huodong.m.taobao.com/"));
            page_headers.insert("Cookie", reqwest::header::HeaderValue::from_str(&cookie_header)?);

            let html = client.get(url)
                .headers(page_headers)
                .send()
                .await?
                .text()
                .await?;

            let re_redirect = Regex::new(r#"var url = '(.*?)';"#)?;
            if let Some(caps) = re_redirect.captures(&html) {
                let redirect_url = caps.get(1).unwrap().as_str();
                if let Some(id_caps) = Regex::new(r"id=([^&]+)")?.captures(redirect_url) {
                    live_id = id_caps.get(1).unwrap().as_str().to_string();
                }
            }
        }

        if live_id.is_empty() {
            return Ok(LiveStatus::Error("Failed to extract Taobao live ID from URL".to_string()));
        }

        // Taobao live uses two attempts. First attempt queries. If token is expired, we update cookies from Set-Cookie and retry.
        for attempt in 0..2 {
            let token = self.get_cookie_value(&cookie_header, "_m_h5_tk")
                .unwrap_or_else(|| "placeholder".to_string());
            let token_prefix = token.split('_').next().unwrap_or("placeholder");

            let t13 = chrono::Utc::now().timestamp_millis().to_string();
            let app_key = "12574478";
            let data_str = format!(r#"{{"liveId":"{}","creatorId":null}}"#, live_id);

            // Pre-sign format: token_prefix & timestamp & app_key & data
            let pre_sign = format!("{}&{}&{}&{}", token_prefix, t13, app_key, data_str);
            let sign = format!("{:x}", md5::compute(pre_sign));

            let api_url = format!(
                "https://h5api.m.taobao.com/h5/mtop.mediaplatform.live.livedetail/4.0/?jsv=2.7.0&appKey={}&t={}&sign={}&AntiFlood=true&AntiCreep=true&api=mtop.mediaplatform.live.livedetail&v=4.0&preventFallback=true&type=jsonp&dataType=jsonp&callback=mtopjsonp1&data={}",
                app_key, t13, sign, urlencoding::encode(&data_str)
            );

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
            headers.insert("Referer", reqwest::header::HeaderValue::from_static("https://huodong.m.taobao.com/"));
            headers.insert("Cookie", reqwest::header::HeaderValue::from_str(&cookie_header)?);

            let response = client.get(&api_url)
                .headers(headers)
                .send()
                .await?;

            // Extract cookies from response headers to update token if expired
            let mut new_token = None;
            let mut new_enc = None;
            for header in response.headers().get_all(reqwest::header::SET_COOKIE) {
                if let Ok(cookie_str) = header.to_str() {
                    if let Some(val) = self.get_cookie_value(cookie_str, "_m_h5_tk") {
                        new_token = Some(val);
                    }
                    if let Some(val) = self.get_cookie_value(cookie_str, "_m_h5_tk_enc") {
                        new_enc = Some(val);
                    }
                }
            }

            let resp_str = response.text().await?;
            let json_str = self.jsonp_to_json(&resp_str).ok_or("Invalid JSONP response from Taobao Live")?;
            let json_data: Value = serde_json::from_str(&json_str)?;

            let ret = &json_data["ret"];
            if let Some(ret_arr) = ret.as_array() {
                if ret_arr.iter().any(|v| v.as_str() == Some("SUCCESS::调用成功")) {
                    let anchor_name = json_data["data"]["broadCaster"]["accountName"].as_str()
                        .unwrap_or("Unknown Taobao Anchor")
                        .to_string();

                    let is_living = json_data["data"]["streamStatus"].as_str() == Some("1");
                    if !is_living {
                        return Ok(LiveStatus::Idle);
                    }

                    let title = json_data["data"]["title"].as_str()
                        .unwrap_or("Taobao Live Stream")
                        .to_string();

                    // Parse stream resolutions
                    let mut stream_urls_list = Vec::new();
                    if let Some(play_list) = json_data["data"]["liveUrlList"].as_array() {
                        for stream in play_list {
                            let url_str = stream["flvUrl"].as_str()
                                .or_else(|| stream["hlsUrl"].as_str())
                                .unwrap_or("");
                            let definition = stream["definition"].as_str()
                                .or_else(|| stream["newDefinition"].as_str())
                                .unwrap_or("md");

                            if !url_str.is_empty() {
                                let priority = match definition {
                                    "ud" => 4,
                                    "hd" => 3,
                                    "md" => 2,
                                    "ld" => 1,
                                    "lld" => 0,
                                    _ => -1,
                                };
                                stream_urls_list.push((priority, url_str.to_string()));
                            }
                        }
                    }

                    if stream_urls_list.is_empty() {
                        return Ok(LiveStatus::Idle);
                    }

                    // Sort by definition priority descending
                    stream_urls_list.sort_by(|a, b| b.0.cmp(&a.0));
                    let final_url = stream_urls_list[0].1.clone();

                    let mut custom_headers = HashMap::new();
                    custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
                    custom_headers.insert("Referer".to_string(), "https://huodong.m.taobao.com/".to_string());

                    let m3u8_url = if final_url.contains(".m3u8") { Some(final_url.clone()) } else { None };
                    let flv_url = if final_url.contains(".flv") { Some(final_url.clone()) } else { None };

                    return Ok(LiveStatus::Living {
                        title,
                        anchor_name,
                        stream_urls: StreamUrls {
                            m3u8_url: m3u8_url.or_else(|| Some(final_url.clone())),
                            flv_url,
                            record_url: final_url,
                            headers: Some(custom_headers),
                        },
                    });
                }
            }

            // If we have updated tokens, update cookie string and try one more time
            if let (Some(t), Some(enc)) = (new_token, new_enc) {
                cookie_header = format!("_m_h5_tk={}; _m_h5_tk_enc={};", t, enc);
            } else if attempt == 0 {
                // If it failed and we got no new cookies, there's no point retrying
                return Ok(LiveStatus::Error(format!("Taobao Live fetch failed: {:?}", ret)));
            }
        }

        Ok(LiveStatus::Error("Taobao Live failed: token invalid after retry".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_taobao_match_url() {
        let platform = TaobaoPlatform::new();
        assert!(platform.match_url("https://taobaolive.taobao.com/room?id=123456"));
        assert!(platform.match_url("https://h5.m.taobao.com/taobaolive/room.html?id=123456"));
        assert!(!platform.match_url("https://live.bilibili.com/55"));
    }

    #[tokio::test]
    async fn test_taobao_fetch_status() {
        let platform = TaobaoPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        // Use a placeholder room ID
        let result = platform.fetch_status("https://taobaolive.taobao.com/room?id=271941163940", &config).await;
        assert!(result.is_ok(), "Taobao fetch_status failed: {:?}", result.err());
        let status = result.unwrap();
        match status {
            LiveStatus::Idle => println!("Taobao room is offline"),
            LiveStatus::Living { title, anchor_name, .. } => {
                println!("Taobao room is living: {} by {}", title, anchor_name);
            }
            LiveStatus::Error(e) => println!("Taobao status returned error: {}", e),
        }
    }
}
