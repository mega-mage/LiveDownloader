use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use crate::common::client::create_http_client;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct AcfunPlatform;

impl AcfunPlatform {
    pub fn new() -> Self {
        Self
    }

    // Generate random 16 character alphanumeric string for did using timestamp hash
    fn generate_did(&self) -> String {
        let t = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(123456789);
        let hash = format!("{:x}", md5::compute(t.to_string()));
        format!("web_{}", &hash[0..16])
    }

    // Call app visitor login to retrieve sign params
    async fn get_sign_params(
        &self,
        client: &reqwest::Client,
        did: &str,
        cookies: Option<&str>,
    ) -> Result<(i64, String), Box<dyn std::error::Error + Send + Sync>> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("referer", reqwest::header::HeaderValue::from_static("https://live.acfun.cn/"));
        headers.insert("user-agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
        
        let cookie_val = if let Some(c) = cookies {
            format!("_did={}; {}", did, c)
        } else {
            format!("_did={};", did)
        };
        headers.insert("Cookie", reqwest::header::HeaderValue::from_str(&cookie_val)?);

        let mut form_data = HashMap::new();
        form_data.insert("sid", "acfun.api.visitor");

        let api = "https://id.app.acfun.cn/rest/app/visitor/login";
        let resp = client.post(api)
            .headers(headers)
            .form(&form_data)
            .send()
            .await?;

        let json_data: Value = resp.json().await?;
        let user_id = json_data["userId"].as_i64().ok_or("userId missing in AcFun login response")?;
        let visitor_st = json_data["acfun.api.visitor_st"].as_str()
            .ok_or("visitor_st missing in AcFun login response")?
            .to_string();

        Ok((user_id, visitor_st))
    }
}

#[async_trait]
impl LivePlatform for AcfunPlatform {
    fn id(&self) -> &'static str {
        "acfun"
    }

    fn name(&self) -> &'static str {
        "AcFun"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("acfun.cn")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;
        let cookies_str = config.cookie.as_deref();

        let clean_url = url.split('?').next().ok_or("Empty URL")?;
        let author_id = clean_url.rsplit('/').next().ok_or("Cannot parse author ID from URL")?.to_string();

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("referer", reqwest::header::HeaderValue::from_str(&format!("https://live.acfun.cn/live/{}", author_id))?);
        headers.insert("user-agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));

        if let Some(c) = cookies_str {
            headers.insert("Cookie", reqwest::header::HeaderValue::from_str(c)?);
        }

        let user_info_api = format!("https://live.acfun.cn/rest/pc-direct/user/userInfo?userId={}", author_id);
        let info_resp = client.get(&user_info_api)
            .headers(headers.clone())
            .send()
            .await?;

        if !info_resp.status().is_success() {
            return Ok(LiveStatus::Idle);
        }

        let info_json: Value = info_resp.json().await?;
        let profile = &info_json["profile"];
        if profile.is_null() {
            return Ok(LiveStatus::Idle);
        }

        let anchor_name = profile["name"].as_str().unwrap_or("Unknown AcFun Anchor").to_string();
        let is_living = !profile["liveId"].is_null();

        if !is_living {
            return Ok(LiveStatus::Idle);
        }

        // Retrieve visitor token details
        let did = self.generate_did();
        let (user_id, visitor_st) = self.get_sign_params(&client, &did, cookies_str).await?;

        // Query the startPlay API
        let play_url_api = format!(
            "https://api.kuaishouzt.com/rest/zt/live/web/startPlay?subBiz=mainApp&kpn=ACFUN_APP&kpf=PC_WEB&userId={}&did={}&acfun.api.visitor_st={}",
            user_id, did, visitor_st
        );

        let mut play_form = HashMap::new();
        play_form.insert("authorId", author_id.as_str());
        play_form.insert("pullStreamType", "FLV");

        let play_resp = client.post(&play_url_api)
            .headers(headers)
            .form(&play_form)
            .send()
            .await?;

        let play_json: Value = play_resp.json().await?;
        if play_json["result"].as_i64().unwrap_or(-1) != 1 {
            return Ok(LiveStatus::Error(format!("AcFun startPlay returned error: {}", play_json["error_msg"])));
        }

        let title = play_json["data"]["caption"].as_str().unwrap_or("AcFun Live Stream").to_string();
        let video_play_res_str = play_json["data"]["videoPlayRes"].as_str()
            .ok_or("videoPlayRes missing in AcFun response")?;

        let video_play_res: Value = serde_json::from_str(video_play_res_str)?;
        let streams = video_play_res["liveAdaptiveManifest"][0]["adaptationSet"]["representation"].as_array()
            .ok_or("stream representations missing in AcFun play response")?;

        if streams.is_empty() {
            return Ok(LiveStatus::Idle);
        }

        // Sort streams by bitrate descending
        let mut play_url_list = Vec::new();
        for s in streams {
            let bitrate = s["bitrate"].as_i64().unwrap_or(0);
            let play_url = s["url"].as_str().unwrap_or("");
            if !play_url.is_empty() {
                play_url_list.push((bitrate, play_url.to_string()));
            }
        }

        if play_url_list.is_empty() {
            return Ok(LiveStatus::Idle);
        }

        play_url_list.sort_by(|a, b| b.0.cmp(&a.0));
        let final_url = play_url_list[0].1.clone();

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://live.acfun.cn/".to_string());

        let m3u8_url = if final_url.contains(".m3u8") { Some(final_url.clone()) } else { None };
        let flv_url = if final_url.contains(".flv") { Some(final_url.clone()) } else { None };

        Ok(LiveStatus::Living {
            title,
            anchor_name,
            stream_urls: StreamUrls {
                m3u8_url: m3u8_url.or_else(|| Some(final_url.clone())),
                flv_url,
                record_url: final_url,
                headers: Some(custom_headers),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acfun_match_url() {
        let platform = AcfunPlatform::new();
        assert!(platform.match_url("https://live.acfun.cn/live/17912421"));
        assert!(!platform.match_url("https://live.bilibili.com/55"));
    }

    #[tokio::test]
    async fn test_acfun_fetch_status() {
        let platform = AcfunPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        // Use a test room ID on AcFun
        let result = platform.fetch_status("https://live.acfun.cn/live/17912421", &config).await;
        assert!(result.is_ok(), "AcFun fetch_status failed: {:?}", result.err());
        let status = result.unwrap();
        match status {
            LiveStatus::Idle => println!("AcFun room is offline"),
            LiveStatus::Living { title, anchor_name, .. } => {
                println!("AcFun room is living: {} by {}", title, anchor_name);
            }
            LiveStatus::Error(e) => println!("AcFun status returned error: {}", e),
        }
    }
}
