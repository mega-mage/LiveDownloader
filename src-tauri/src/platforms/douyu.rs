use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use regex::Regex;
use crate::common::client::create_http_client;
use crate::common::js_engine::JsEngine;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct DouyuPlatform;

impl DouyuPlatform {
    pub fn new() -> Self {
        Self
    }

    // Extract the signature parameters using our JS engine
    async fn get_sign_params(
        &self,
        rid: &str,
        client: &reqwest::Client,
        headers: &reqwest::header::HeaderMap,
    ) -> Result<HashMap<String, String>, Box<dyn std::error::Error + Send + Sync>> {
        let room_url = format!("https://www.douyu.com/{}", rid);
        let html = client.get(&room_url)
            .headers(headers.clone())
            .send()
            .await?
            .text()
            .await?;

        // 1. Find the ub98484234 function
        let re_js = Regex::new(r#"(vdwdae325w_64we[\s\S]*function ub98484234[\s\S]*?)function"#)?;
        let caps = re_js.captures(&html).ok_or("Douyu signature function ub98484234 not found")?;
        let js_code = caps.get(1).ok_or("Douyu signature capture empty")?.as_str();

        // Replace eval block to extract the internal sign JS logic
        let re_eval = Regex::new(r#"eval.*?;\s*}"#)?;
        let func_ub9 = re_eval.replace(js_code, "strc;}").to_string();

        let mut engine = JsEngine::new();
        engine.load_code(&func_ub9)?;
        let res = engine.call_function("ub98484234", &[])?;

        let t10 = chrono::Utc::now().timestamp().to_string();
        let did = "10000000000000000000000000003306";

        // Find v parameter inside res
        let re_v = Regex::new(r#"v=(\d+)"#)?;
        let v_caps = re_v.captures(&res).ok_or("v parameter not found in ub98484234 result")?;
        let v = v_caps.get(1).unwrap().as_str();

        // Calculate rb = md5(rid + did + t10 + v)
        let rb_src = format!("{}{}{}{}", rid, did, t10, v);
        let rb = format!("{:x}", md5::compute(rb_src));

        // Format sign function for execution in JS engine
        let mut func_sign = res.replace("return rt;});", "return rt;}");
        func_sign = func_sign.replace("return rt;})", "return rt;}");
        func_sign = func_sign.replace("(function (", "function sign(");
        func_sign = func_sign.replace("CryptoJS.MD5(cb).toString()", &format!("\"{}\"", rb));

        let mut engine2 = JsEngine::new();
        engine2.load_code(&func_sign)?;
        let params_str = engine2.call_function("sign", &[rid.to_string(), did.to_string(), t10.to_string()])?;

        // Parse query string parameters like key1=val1&key2=val2
        let mut params = HashMap::new();
        for pair in params_str.split('&') {
            let mut kv = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                params.insert(k.to_string(), v.to_string());
            }
        }

        // Add additional standard API payload parameters
        params.insert("ver".to_string(), "22011191".to_string());
        params.insert("rid".to_string(), rid.to_string());
        params.insert("rate".to_string(), "-1".to_string()); // Default stream rate

        Ok(params)
    }
}

#[async_trait]
impl LivePlatform for DouyuPlatform {
    fn id(&self) -> &'static str {
        "douyu"
    }

    fn name(&self) -> &'static str {
        "斗鱼直播"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("douyu.com")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
        headers.insert("Referer", reqwest::header::HeaderValue::from_static("https://m.douyu.com/"));
        
        if let Some(ref cookies) = config.cookie {
            if !cookies.is_empty() {
                headers.insert("Cookie", reqwest::header::HeaderValue::from_str(cookies)?);
            }
        }

        // Extract raw room ID (rid)
        let clean_url = url.split('?').next().ok_or("Empty URL")?;
        let mut rid = if let Some(caps) = Regex::new(r"rid=([^&]+)")?.captures(url) {
            caps.get(1).unwrap().as_str().to_string()
        } else {
            clean_url.rsplit('/').next().ok_or("Cannot parse room ID from URL")?.to_string()
        };

        // If rid has alphabetic characters (alias), we must query the mobile page context to find the numeric rid
        let has_alpha = rid.chars().any(|c| c.is_alphabetic());
        if has_alpha {
            let mut mobile_headers = headers.clone();
            mobile_headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15"));
            
            let m_url = format!("https://m.douyu.com/{}", rid);
            let html = client.get(&m_url)
                .headers(mobile_headers)
                .send()
                .await?
                .text()
                .await?;

            let re_ctx = Regex::new(r#"<script id="vike_pageContext" type="application/json">(.*?)</script>"#)?;
            if let Some(caps) = re_ctx.captures(&html) {
                let ctx_str = caps.get(1).unwrap().as_str();
                let ctx_json: Value = serde_json::from_str(ctx_str)?;
                if let Some(num_rid) = ctx_json["pageProps"]["room"]["roomInfo"]["roomInfo"]["rid"].as_i64() {
                    rid = num_rid.to_string();
                } else if let Some(num_rid_str) = ctx_json["pageProps"]["room"]["roomInfo"]["roomInfo"]["rid"].as_str() {
                    rid = num_rid_str.to_string();
                }
            } else {
                return Ok(LiveStatus::Error("Failed to extract numeric room ID from Douyu mobile context".to_string()));
            }
        }

        // Query the betard info endpoint to check status
        let info_url = format!("https://www.douyu.com/betard/{}", rid);
        let info_resp_str = client.get(&info_url)
            .headers(headers.clone())
            .send()
            .await?
            .text()
            .await?;

        let info_json: Value = serde_json::from_str(&info_resp_str)?;
        let anchor_name = info_json["room"]["nickname"].as_str()
            .unwrap_or("Unknown Douyu Anchor")
            .to_string();

        let is_video_loop = info_json["room"]["videoLoop"].as_i64().unwrap_or(1);
        let show_status = info_json["room"]["show_status"].as_i64().unwrap_or(0);

        let is_live = is_video_loop == 0 && show_status == 1;
        if !is_live {
            return Ok(LiveStatus::Idle);
        }

        let title = info_json["room"]["room_name"].as_str()
            .unwrap_or("Douyu Live Stream")
            .replace("&amp;", "&")
            .replace("&nbsp;", " ");

        // Retrieve signature parameters and request getH5Play
        let sign_params = self.get_sign_params(&rid, &client, &headers).await?;
        let play_api = format!("https://www.douyu.com/lapi/live/getH5Play/{}", rid);

        let play_resp = client.post(&play_api)
            .headers(headers)
            .form(&sign_params)
            .send()
            .await?;

        if !play_resp.status().is_success() {
            return Ok(LiveStatus::Error("Douyu getH5Play API request failed".to_string()));
        }

        let play_json: Value = play_resp.json().await?;
        if play_json["error"].as_i64().unwrap_or(-1) != 0 {
            return Ok(LiveStatus::Error(format!("Douyu getH5Play returned error: {}", play_json["msg"])));
        }

        let rtmp_url = play_json["data"]["rtmp_url"].as_str().ok_or("rtmp_url missing in Douyu stream data")?;
        let rtmp_live = play_json["data"]["rtmp_live"].as_str().ok_or("rtmp_live missing in Douyu stream data")?;

        let flv_url = format!("{}/{}", rtmp_url, rtmp_live);
        
        // Douyu usually includes a backup HLS stream in play_json, or we can use the FLV stream directly
        let m3u8_url = play_json["data"]["hls_url"].as_str().map(|s| format!("{}/{}", rtmp_url, s));

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://www.douyu.com/".to_string());

        Ok(LiveStatus::Living {
            title,
            anchor_name,
            stream_urls: StreamUrls {
                m3u8_url: m3u8_url.or_else(|| Some(flv_url.clone())),
                flv_url: Some(flv_url.clone()),
                record_url: flv_url,
                headers: Some(custom_headers),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_douyu_match_url() {
        let platform = DouyuPlatform::new();
        assert!(platform.match_url("https://www.douyu.com/288016"));
        assert!(platform.match_url("https://douyu.com/lpl"));
        assert!(!platform.match_url("https://www.huya.com/lpl"));
    }

    #[tokio::test]
    async fn test_douyu_fetch_status() {
        let platform = DouyuPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        // Use a test room id
        let result = platform.fetch_status("https://www.douyu.com/lpl", &config).await;
        assert!(result.is_ok(), "Douyu fetch_status failed: {:?}", result.err());
        let status = result.unwrap();
        match status {
            LiveStatus::Idle => println!("Douyu room lpl is offline"),
            LiveStatus::Living { title, anchor_name, .. } => {
                println!("Douyu room lpl is living: {} by {}", title, anchor_name);
            }
            LiveStatus::Error(e) => println!("Douyu status returned error: {}", e),
        }
    }
}
