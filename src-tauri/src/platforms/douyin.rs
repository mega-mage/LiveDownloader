use crate::platforms::douyin_sign;

use crate::platforms::{LivePlatform, LiveStatus, StreamUrls, PlatformConfig};
use crate::common::client::create_http_client;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, REFERER, USER_AGENT, ACCEPT, ACCEPT_LANGUAGE};
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
        url.contains("douyin.com") || url.contains("douyincdn.com") || url.contains("pull-")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";
        
        let user_cookie = config.cookie.as_deref().unwrap_or("").trim();
        let cookie_str = if !user_cookie.is_empty() {
            user_cookie.to_string()
        } else {
            get_fresh_ttwid(&client, ua).await.unwrap_or_default()
        };

        // 0. Support direct FLV / M3U8 stream CDN URLs (e.g. pull-flv-l1.douyincdn.com)
        if url.contains(".flv") || url.contains(".m3u8") || url.contains("douyincdn.com") || url.contains("pull-") {
            let record_url = clean_stream_url(url);
            let is_m3u8 = record_url.contains(".m3u8");
            let mut custom_headers = HashMap::new();
            custom_headers.insert("User-Agent".to_string(), ua.to_string());
            if !cookie_str.is_empty() {
                custom_headers.insert("Cookie".to_string(), cookie_str.clone());
            }
            return Ok(LiveStatus::Living {
                title: "抖音直播直连".to_string(),
                anchor_name: "抖音主播".to_string(),
                stream_urls: StreamUrls {
                    m3u8_url: if is_m3u8 { Some(record_url.clone()) } else { None },
                    flv_url: if !is_m3u8 { Some(record_url.clone()) } else { None },
                    record_url,
                    headers: Some(custom_headers),
                },
            });
        }

        // 0.1 Support short links (v.douyin.com), follow redirect to extract real room URL
        let real_url = if url.contains("v.douyin.com") {
            if let Ok(resp) = client.get(url).send().await {
                resp.url().to_string()
            } else {
                url.to_string()
            }
        } else {
            url.to_string()
        };

        let web_rid = extract_web_rid(&real_url)?;

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(ua));
        headers.insert(REFERER, HeaderValue::from_str(&format!("https://live.douyin.com/{}", web_rid))?);
        if !cookie_str.is_empty() {
            headers.insert(COOKIE, HeaderValue::from_str(&cookie_str)?);
        }
        headers.insert(ACCEPT, HeaderValue::from_static("application/json, text/plain, */*"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"));

        // 1. Try enter API request
        let params = [
            ("aid", "6383"),
            ("app_name", "douyin_web"),
            ("live_id", "1"),
            ("device_platform", "web"),
            ("language", "zh-CN"),
            ("browser_language", "zh-CN"),
            ("browser_platform", "Win32"),
            ("browser_name", "Chrome"),
            ("browser_version", "122.0.0.0"),
            ("web_rid", &web_rid),
            ("room_id", &web_rid),
            ("msToken", ""),
        ];
        
        let query_str = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params.iter())
            .finish();
            
        let a_bogus = douyin_sign::ab_sign(&query_str, ua);
        
        let enter_url = format!(
            "https://live.douyin.com/webcast/room/web/enter/?{}&a_bogus={}",
            query_str, a_bogus
        );

        if let Ok(resp) = client.get(&enter_url).headers(headers.clone()).send().await {
            if let Ok(body_text) = resp.text().await {
                if let Ok(json_val) = serde_json::from_str::<Value>(&body_text) {
                    let data = &json_val["data"];
                    if !data.is_null() && !data["data"].is_null() && data["data"].as_array().map_or(false, |a| !a.is_empty()) {
                        let room_data = &data["data"][0];
                        let anchor_name = data["user"]["nickname"].as_str()
                            .unwrap_or_else(|| room_data.pointer("/owner/nickname").and_then(|v| v.as_str()).unwrap_or("抖音主播"))
                            .to_string();
                        return parse_douyin_room_data(room_data, &anchor_name, &config.quality, ua, &cookie_str);
                    }
                }
            }
        }

        // 2. Fetch room HTML page directly with cookie & decode HTML entities
        let room_page_url = format!("https://live.douyin.com/{}", web_rid);
        let mut html_headers = headers.clone();
        html_headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));

        if let Ok(page_resp) = client.get(&room_page_url).headers(html_headers).send().await {
            if let Ok(html_text) = page_resp.text().await {
                if let Some(json_val) = extract_douyin_html_data(&html_text) {
                    if let Some((room_data, anchor_name)) = find_room_in_html_json(&json_val) {
                        return parse_douyin_room_data(&room_data, &anchor_name, &config.quality, ua, &cookie_str);
                    }
                }
            }
        }

        Ok(LiveStatus::Idle)
    }
}

pub async fn get_fresh_ttwid(client: &reqwest::Client, ua: &str) -> Option<String> {
    let body = serde_json::json!({
        "region": "cn",
        "aid": 1768,
        "needFp": "true",
        "service": "www.douyin.com",
        "clientInfo": {}
    });

    if let Ok(resp) = client.post("https://ttwid.bytedance.com/ttwid/union/register/")
        .header(USER_AGENT, ua)
        .json(&body)
        .send()
        .await
    {
        for header in resp.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(s) = header.to_str() {
                if s.contains("ttwid=") {
                    for p in s.split(';') {
                        let p_trimmed = p.trim();
                        if p_trimmed.starts_with("ttwid=") {
                            return Some(p_trimmed.to_string());
                        }
                    }
                }
            }
        }
    }

    if let Ok(resp) = client.get("https://live.douyin.com/").header(USER_AGENT, ua).send().await {
        for header in resp.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(s) = header.to_str() {
                if s.contains("ttwid=") {
                    for p in s.split(';') {
                        let p_trimmed = p.trim();
                        if p_trimmed.starts_with("ttwid=") {
                            return Some(p_trimmed.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
     .replace("&amp;", "&")
     .replace("&lt;", "<")
     .replace("&gt;", ">")
     .replace("&#34;", "\"")
     .replace("&#39;", "'")
}

fn clean_stream_url(u: &str) -> String {
    u.replace("&amp;", "&")
     .replace("\\u0026", "&")
     .replace("&quot;", "")
}

fn parse_douyin_room_data(
    room_data: &Value,
    anchor_name: &str,
    quality_setting: &str,
    ua: &str,
    cookie_str: &str,
) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
    let status = room_data["status"].as_i64().unwrap_or(4);
    if status != 2 {
        return Ok(LiveStatus::Idle);
    }

    let raw_title = room_data["title"].as_str().unwrap_or("抖音直播间");
    let title = if raw_title.is_empty() { "抖音直播间".to_string() } else { raw_title.to_string() };

    let raw_anchor = if !anchor_name.is_empty() && anchor_name != "Unknown Anchor" {
        anchor_name
    } else {
        room_data.pointer("/owner/nickname").and_then(|v| v.as_str()).unwrap_or("抖音主播")
    };
    let anchor_name_str = if raw_anchor.is_empty() { "抖音主播".to_string() } else { raw_anchor.to_string() };

    let stream_url_info = &room_data["stream_url"];
    let flv_url_dict = &stream_url_info["flv_pull_url"];
    let m3u8_url_dict = &stream_url_info["hls_pull_url_map"];

    let select_key = match quality_setting {
        "原画" => "FULL_HD1",
        "超清" => "HD1",
        "高清" => "SD1",
        "标清" => "SD2",
        "流畅" => "SD2",
        _ => "FULL_HD1",
    };

    let mut flv_url = flv_url_dict[select_key].as_str()
        .or_else(|| flv_url_dict.as_object().and_then(|obj| obj.values().next().and_then(|v| v.as_str())))
        .map(|s| clean_stream_url(s));

    let mut m3u8_url = m3u8_url_dict[select_key].as_str()
        .or_else(|| m3u8_url_dict.as_object().and_then(|obj| obj.values().next().and_then(|v| v.as_str())))
        .map(|s| clean_stream_url(s));

    if let Some(sdk_data_str) = stream_url_info["live_core_sdk_data"]["pull_data"]["stream_data"].as_str() {
        let clean_sdk_str = clean_stream_url(sdk_data_str);
        if let Ok(sdk_data) = serde_json::from_str::<Value>(&clean_sdk_str) {
            if let Some(origin) = sdk_data["data"]["origin"]["main"].as_object() {
                let vcodec = sdk_data["data"]["origin"]["main"]["sdk_params"]["VCodec"].as_str().unwrap_or("");
                if let Some(hls) = origin.get("hls").and_then(|h| h.as_str()) {
                    m3u8_url = Some(clean_stream_url(&format!("{}&codec={}", hls, vcodec)));
                }
                if let Some(flv) = origin.get("flv").and_then(|f| f.as_str()) {
                    flv_url = Some(clean_stream_url(&format!("{}&codec={}", flv, vcodec)));
                }
            }
        }
    }

    let record_url = flv_url.clone().or_else(|| m3u8_url.clone())
        .ok_or("No recordable stream URL found")?;

    let mut custom_headers = HashMap::new();
    custom_headers.insert("User-Agent".to_string(), ua.to_string());
    if !cookie_str.is_empty() {
        custom_headers.insert("Cookie".to_string(), cookie_str.to_string());
    }

    Ok(LiveStatus::Living {
        title,
        anchor_name: anchor_name_str,
        stream_urls: StreamUrls {
            m3u8_url,
            flv_url,
            record_url,
            headers: Some(custom_headers),
        },
    })
}

fn extract_douyin_html_data(html: &str) -> Option<Value> {
    let clean_text = html.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");

    // 1. Try RENDER_DATA
    if let Some(start) = html.find(r#"id="RENDER_DATA""#) {
        if let Some(content_start) = html[start..].find('>') {
            let real_start = start + content_start + 1;
            if let Some(end) = html[real_start..].find("</script>") {
                let raw_content = html[real_start..real_start + end].trim();
                let decoded_url = urlencoding::decode(raw_content).unwrap_or(std::borrow::Cow::Borrowed(raw_content)).to_string();
                let unescaped = decode_html_entities(&decoded_url);
                if let Ok(val) = serde_json::from_str::<Value>(&unescaped) {
                    return Some(val);
                }
            }
        }
    }

    // 2. Try __UNIVERSAL_DATA_FOR_REHYDRATION__
    if let Some(start) = html.find(r#"id="__UNIVERSAL_DATA_FOR_REHYDRATION__""#) {
        if let Some(content_start) = html[start..].find('>') {
            let real_start = start + content_start + 1;
            if let Some(end) = html[real_start..].find("</script>") {
                let raw_content = html[real_start..real_start + end].trim();
                let decoded_url = urlencoding::decode(raw_content).unwrap_or(std::borrow::Cow::Borrowed(raw_content)).to_string();
                let unescaped = decode_html_entities(&decoded_url);
                if let Ok(val) = serde_json::from_str::<Value>(&unescaped) {
                    return Some(val);
                }
            }
        }
    }

    // 3. Fallback for Next.js SSR pace_f scripts: Extract FLV stream and real Title & Anchor Nickname
    let flv_re = regex::Regex::new(r#"(https?://[^\s"'<>]+\.flv[^\s"'<>]*)"#).ok()?;
    if let Some(cap) = flv_re.captures(&clean_text) {
        let flv_url = clean_stream_url(&cap[1]);
        let (extracted_title, extracted_anchor) = parse_douyin_html_meta(html);

        let mut room = serde_json::Map::new();
        room.insert("status".to_string(), serde_json::json!(2));
        room.insert("title".to_string(), serde_json::json!(extracted_title));
        
        let mut owner = serde_json::Map::new();
        owner.insert("nickname".to_string(), serde_json::json!(extracted_anchor));
        room.insert("owner".to_string(), Value::Object(owner));

        let mut stream = serde_json::Map::new();
        let mut flv_dict = serde_json::Map::new();
        flv_dict.insert("FULL_HD1".to_string(), serde_json::json!(flv_url));
        stream.insert("flv_pull_url".to_string(), Value::Object(flv_dict));
        stream.insert("hls_pull_url_map".to_string(), Value::Object(serde_json::Map::new()));
        room.insert("stream_url".to_string(), Value::Object(stream));
        return Some(Value::Object(room));
    }

    None
}

fn parse_douyin_html_meta(html: &str) -> (String, String) {
    let clean_text = html.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");

    // 1. Extract Anchor Nickname
    let nick_re = regex::Regex::new(r#""nickname"\s*:\s*"([^"]+)""#).unwrap();
    let mut anchor_name = "抖音主播".to_string();
    for cap in nick_re.captures_iter(&clean_text) {
        let name = cap[1].trim();
        if !name.is_empty() && name != "$undefined" && name != "抖音主播" {
            anchor_name = name.to_string();
            break;
        }
    }

    // 2. Extract Room Title
    let mut title = "抖音直播间".to_string();
    let title_status_re = regex::Regex::new(r#""title"\s*:\s*"([^"]+)"[^{}]*?"status"\s*:\s*2"#).unwrap();
    if let Some(cap) = title_status_re.captures(&clean_text) {
        title = cap[1].trim().to_string();
    } else {
        let status_title_re = regex::Regex::new(r#""status"\s*:\s*2[^{}]*?"title"\s*:\s*"([^"]+)""#).unwrap();
        if let Some(cap) = status_title_re.captures(&clean_text) {
            title = cap[1].trim().to_string();
        } else {
            if let Some(t1) = clean_text.find("<title") {
                if let Some(t_start) = clean_text[t1..].find('>') {
                    if let Some(t2) = clean_text[t1 + t_start..].find("</title>") {
                        let raw_t = &clean_text[t1 + t_start + 1..t1 + t_start + t2];
                        let clean_t = raw_t.split('-').next().unwrap_or(raw_t).trim();
                        if !clean_t.is_empty() && clean_t != "抖音直播" {
                            title = clean_t.to_string();
                        }
                    }
                }
            }
        }
    }

    (title, anchor_name)
}

fn find_room_in_html_json(val: &Value) -> Option<(Value, String)> {
    if let Some(room) = val.pointer("/__DEFAULT_SCOPE__/webapp.room.detail/data/0") {
        if !room.is_null() && room.get("stream_url").is_some() {
            let nickname = val.pointer("/__DEFAULT_SCOPE__/webapp.room.detail/user/nickname")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| room.pointer("/owner/nickname").and_then(|v| v.as_str()).unwrap_or("Unknown Anchor"))
                .to_string();
            return Some((room.clone(), nickname));
        }
    }

    if let Some(room_info) = val.pointer("/appContext/states/roomStore/roomInfo") {
        if let Some(room) = room_info.get("room") {
            if !room.is_null() && room.get("stream_url").is_some() {
                let nickname = room_info.pointer("/anchor/nickname")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| room.pointer("/owner/nickname").and_then(|v| v.as_str()).unwrap_or("Unknown Anchor"))
                    .to_string();
                return Some((room.clone(), nickname));
            }
        }
    }

    search_room_object(val)
}

fn search_room_object(val: &Value) -> Option<(Value, String)> {
    match val {
        Value::Object(map) => {
            if map.contains_key("stream_url") && (map.contains_key("status") || map.contains_key("title")) {
                let nickname = map.get("owner")
                    .and_then(|o| o.get("nickname"))
                    .and_then(|n| n.as_str())
                    .or_else(|| map.get("user").and_then(|u| u.get("nickname")).and_then(|n| n.as_str()))
                    .unwrap_or("抖音主播")
                    .to_string();
                return Some((val.clone(), nickname));
            }
            for v in map.values() {
                if let Some(res) = search_room_object(v) {
                    return Some(res);
                }
            }
        }
        Value::Array(arr) => {
            for v in arr {
                if let Some(res) = search_room_object(v) {
                    return Some(res);
                }
            }
        }
        _ => {}
    }
    None
}

fn extract_web_rid(url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let clean_url = url.split('?').next().ok_or("Empty URL")?;
    let last_part = clean_url.trim_end_matches('/').rsplit('/').next().ok_or("Cannot extract web_rid from URL")?;
    Ok(last_part.to_string())
}
