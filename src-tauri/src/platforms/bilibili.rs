use crate::platforms::{LivePlatform, LiveStatus, StreamUrls, PlatformConfig};
use crate::common::client::create_http_client;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, ORIGIN, REFERER};
use serde_json::Value;
use std::collections::HashMap;

pub struct BilibiliPlatform;

impl BilibiliPlatform {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LivePlatform for BilibiliPlatform {
    fn id(&self) -> &'static str {
        "bilibili"
    }

    fn name(&self) -> &'static str {
        "B站直播"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("live.bilibili.com")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let room_id = extract_room_id(url)?;
        
        let client = create_http_client(config.proxy.as_deref(), 10)?;
        
        // 1. Get room init info (to check if live and get uid)
        let init_url = format!("https://api.live.bilibili.com/room/v1/Room/room_init?id={}", room_id);
        let mut headers = HeaderMap::new();
        if let Some(ref cookie) = config.cookie {
            headers.insert(COOKIE, HeaderValue::from_str(cookie)?);
        }
        
        let init_resp = client.get(&init_url)
            .headers(headers.clone())
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        if init_resp["code"].as_i64().unwrap_or(-1) != 0 {
            return Ok(LiveStatus::Error(format!("Room init API failed: {}", init_resp["msg"])));
        }
        
        let live_status = init_resp["data"]["live_status"].as_i64().unwrap_or(0) == 1;
        if !live_status {
            return Ok(LiveStatus::Idle);
        }
        
        let uid = init_resp["data"]["uid"].as_i64().ok_or("uid not found")?;
        
        // 2. Get master info (anchor name)
        let master_url = format!("https://api.live.bilibili.com/live_user/v1/Master/info?uid={}", uid);
        let master_resp = client.get(&master_url)
            .headers(headers.clone())
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        let anchor_name = master_resp["data"]["info"]["uname"].as_str()
            .unwrap_or("Unknown Anchor")
            .to_string();
            
        // 3. Get room detail (title) using getH5InfoByRoom
        let h5_url = format!("https://api.live.bilibili.com/xlive/web-room/v1/index/getH5InfoByRoom?room_id={}", room_id);
        let mut h5_headers = headers.clone();
        h5_headers.insert(ORIGIN, HeaderValue::from_static("https://live.bilibili.com"));
        h5_headers.insert(REFERER, HeaderValue::from_str(&format!("https://live.bilibili.com/{}", room_id))?);
        
        let h5_resp = client.get(&h5_url)
            .headers(h5_headers)
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        let title = h5_resp["data"]["room_info"]["title"].as_str()
            .unwrap_or("Bilibili Live Room")
            .to_string();
            
        // 4. Resolve quality QN
        // qn: 10000=原画, 400=蓝光, 250=超清, 150=高清, 80=流畅
        let qn = match config.quality.as_str() {
            "原画" => "10000",
            "超清" => "250",
            "高清" => "150",
            "标清" => "80",
            "流畅" => "80",
            _ => "10000",
        };
        
        // 5. Get play URL
        let play_url_api = format!(
            "https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo?room_id={}&protocol=0,1&format=0,1,2&codec=0,1&qn={}&platform=web&ptype=8&dolby=5&panorama=1&hdr_type=0,1",
            room_id, qn
        );
        
        let play_resp = client.get(&play_url_api)
            .headers(headers)
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        if play_resp["code"].as_i64().unwrap_or(-1) != 0 {
            return Ok(LiveStatus::Error(format!("Play URL API failed: {}", play_resp["message"])));
        }
        
        let playurl_info = &play_resp["data"]["playurl_info"];
        let streams = playurl_info["playurl"]["stream"].as_array()
            .ok_or("playurl stream array missing")?;
            
        if streams.is_empty() {
            return Ok(LiveStatus::Error("No playurl streams returned by Bilibili API".to_string()));
        }
        
        let mut record_url = None;
        let mut m3u8_url = None;
        let mut flv_url = None;
        
        // Loop through all streams to find HLS vs FLV/TS
        for stream in streams {
            let protocol_name = stream["protocol_name"].as_str().unwrap_or("");
            if let Some(formats) = stream["format"].as_array() {
                for format in formats {
                    let format_name = format["format_name"].as_str().unwrap_or("");
                    if let Some(codecs) = format["codec"].as_array() {
                        for codec in codecs {
                            let base_url = match codec["base_url"].as_str() {
                                Some(b) => b,
                                None => continue,
                            };
                            let url_info = &codec["url_info"][0];
                            let host = match url_info["host"].as_str() {
                                Some(h) => h,
                                None => continue,
                            };
                            let extra = match url_info["extra"].as_str() {
                                Some(e) => e,
                                None => continue,
                            };
                            
                            let full_url = format!("{}{}{}", host, base_url, extra);
                            
                            // Check if HLS (m3u8)
                            if protocol_name == "http_hls" || format_name == "hls" || base_url.contains(".m3u8") {
                                if m3u8_url.is_none() {
                                    m3u8_url = Some(full_url.clone());
                                }
                            } else if format_name == "flv" || base_url.contains(".flv") {
                                if flv_url.is_none() {
                                    flv_url = Some(full_url.clone());
                                }
                            }
                            
                            // Set default record_url if none set yet (first codec of first stream)
                            if record_url.is_none() {
                                record_url = Some(full_url);
                            }
                        }
                    }
                }
            }
        }
        
        let final_record_url = record_url.ok_or("No recordable stream URL found")?;
        
        // Set standard headers for bilibili recording
        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), crate::common::client::DEFAULT_USER_AGENT.to_string());
        custom_headers.insert("Referer".to_string(), format!("https://live.bilibili.com/{}", room_id));
        
        Ok(LiveStatus::Living {
            title,
            anchor_name,
            stream_urls: StreamUrls {
                m3u8_url: m3u8_url.or_else(|| flv_url.clone()).or_else(|| Some(final_record_url.clone())),
                flv_url,
                record_url: final_record_url,
                headers: Some(custom_headers),
            },
        })
    }
}

fn extract_room_id(url: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let clean_url = url.split('?').next().ok_or("Empty URL")?;
    let last_part = clean_url.rsplit('/').next().ok_or("Cannot extract room id from URL")?;
    let room_id = last_part.parse::<u64>()?;
    Ok(room_id)
}
