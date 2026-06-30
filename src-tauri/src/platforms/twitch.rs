use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use crate::common::client::create_http_client;
use crate::platforms::{LivePlatform, LiveStatus, PlatformConfig, StreamUrls};

pub struct TwitchPlatform;

impl TwitchPlatform {
    pub fn new() -> Self {
        Self
    }

    // Call ChannelShell GQL query to retrieve display name and status
    async fn get_room_info(
        &self,
        client: &reqwest::Client,
        uid: &str,
        token: &str,
        cookies: Option<&str>,
    ) -> Result<(String, bool), Box<dyn std::error::Error + Send + Sync>> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
        headers.insert("Referer", reqwest::header::HeaderValue::from_static("https://www.twitch.tv/"));
        headers.insert("Client-Id", reqwest::header::HeaderValue::from_static("kimne78kx3ncx6brgo4mv6wki5h1ko"));
        headers.insert("Client-Integrity", reqwest::header::HeaderValue::from_str(token)?);
        headers.insert("Content-Type", reqwest::header::HeaderValue::from_static("application/json"));

        if let Some(c) = cookies {
            headers.insert("Cookie", reqwest::header::HeaderValue::from_str(c)?);
        }

        let payload = serde_json::json!([
            {
                "operationName": "ChannelShell",
                "variables": {
                    "login": uid
                },
                "extensions": {
                    "persistedQuery": {
                        "version": 1,
                        "sha256Hash": "580ab410bcd0c1ad194224957ae2241e5d252b2c5173d8e0cce9d32d5bb14efe"
                    }
                }
            }
        ]);

        let resp = client.post("https://gql.twitch.tv/gql")
            .headers(headers)
            .json(&payload)
            .send()
            .await?;

        let json_data: Value = resp.json().await?;
        let user_data = &json_data[0]["data"]["userOrError"];
        
        if user_data.is_null() || user_data.get("login").is_none() {
            return Err("User not found on Twitch".into());
        }

        let display_name = user_data["displayName"].as_str().unwrap_or(uid);
        let login = user_data["login"].as_str().unwrap_or(uid);
        let nickname = format!("{}-{}", display_name, login);

        let is_live = !user_data["stream"].is_null();

        Ok((nickname, is_live))
    }
}

#[async_trait]
impl LivePlatform for TwitchPlatform {
    fn id(&self) -> &'static str {
        "twitch"
    }

    fn name(&self) -> &'static str {
        "Twitch"
    }

    fn match_url(&self, url: &str) -> bool {
        url.contains("twitch.tv")
    }

    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(config.proxy.as_deref(), 10)?;
        let cookies_str = config.cookie.as_deref();

        let clean_url = url.split('?').next().ok_or("Empty URL")?;
        let clean_url = clean_url.trim_end_matches('/');
        let uid = clean_url.rsplit('/').next().ok_or("Cannot parse Twitch login ID from URL")?.to_string();

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("User-Agent", reqwest::header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
        headers.insert("Referer", reqwest::header::HeaderValue::from_static("https://www.twitch.tv/"));
        headers.insert("Client-ID", reqwest::header::HeaderValue::from_static("kimne78kx3ncx6brgo4mv6wki5h1ko"));
        headers.insert("Content-Type", reqwest::header::HeaderValue::from_static("application/json"));

        if let Some(c) = cookies_str {
            headers.insert("Cookie", reqwest::header::HeaderValue::from_str(c)?);
        }

        // 1. Fetch playback access token
        let payload = serde_json::json!({
            "operationName": "PlaybackAccessToken_Template",
            "query": "query PlaybackAccessToken_Template($login: String!, $isLive: Boolean!, $playerType: String!) {  streamPlaybackAccessToken(channelName: $login, params: {platform: \"web\", playerBackend: \"mediaplayer\", playerType: $playerType}) @include(if: $isLive) {    value    signature   authorization { isForbidden forbiddenReasonCode }   __typename  } }",
            "variables": {
                "isLive": true,
                "login": uid,
                "playerType": "embed"
            }
        });

        let token_resp = client.post("https://gql.twitch.tv/gql")
            .headers(headers)
            .json(&payload)
            .send()
            .await?;

        let token_json: Value = token_resp.json().await?;
        let token_data = &token_json["data"]["streamPlaybackAccessToken"];
        if token_data.is_null() {
            return Ok(LiveStatus::Idle);
        }

        let token = token_data["value"].as_str().ok_or("Token value missing in Twitch response")?;
        let signature = token_data["signature"].as_str().ok_or("Signature missing in Twitch response")?;

        // 2. Fetch room info for live check & nickname
        let (anchor_name, is_live) = match self.get_room_info(&client, &uid, token, cookies_str).await {
            Ok(res) => res,
            Err(_) => return Ok(LiveStatus::Idle),
        };

        if !is_live {
            return Ok(LiveStatus::Idle);
        }

        // 3. Assemble HLS playlist URL
        let m3u8_url = format!(
            "https://usher.ttvnw.net/api/channel/hls/{}.m3u8?acmb=e30%3D&allow_source=true&browser_family=firefox&browser_version=124.0&cdm=wv&fast_bread=true&os_name=Windows&os_version=NT%252010.0&p=3553732&platform=web&play_session_id=064bc3ff1722b6f53b0b5b8c01e46ca5&player_backend=mediaplayer&player_version=1.28.0-rc.1&playlist_include_framerate=true&reassignments_supported=true&sig={}&token={}&transcode_mode=cbr_v1",
            uid, signature, urlencoding::encode(token)
        );

        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string());
        custom_headers.insert("Referer".to_string(), "https://www.twitch.tv/".to_string());

        Ok(LiveStatus::Living {
            title: format!("{}'s Twitch Stream", display_name_only(&anchor_name)),
            anchor_name,
            stream_urls: StreamUrls {
                m3u8_url: Some(m3u8_url.clone()),
                flv_url: None,
                record_url: m3u8_url,
                headers: Some(custom_headers),
            },
        })
    }
}

// Helper to extract display name from "DisplayName-login" format
fn display_name_only(full_name: &str) -> &str {
    full_name.split('-').next().unwrap_or(full_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_twitch_match_url() {
        let platform = TwitchPlatform::new();
        assert!(platform.match_url("https://www.twitch.tv/ninja"));
        assert!(platform.match_url("https://twitch.tv/shroud"));
        assert!(!platform.match_url("https://live.bilibili.com/55"));
    }

    #[tokio::test]
    async fn test_twitch_fetch_status() {
        let platform = TwitchPlatform::new();
        let config = PlatformConfig {
            cookie: None,
            // Twitch requires VPN/Proxy inside China. If we are offline/blocked, this test will print warning.
            proxy: None,
            quality: "origin".to_string(),
            extra: HashMap::new(),
        };
        let result = platform.fetch_status("https://www.twitch.tv/ninja", &config).await;
        match result {
            Ok(status) => match status {
                LiveStatus::Idle => println!("Twitch room ninja is offline"),
                LiveStatus::Living { title, anchor_name, .. } => {
                    println!("Twitch room ninja is living: {} by {}", title, anchor_name);
                }
                LiveStatus::Error(e) => println!("Twitch status returned error: {}", e),
            },
            Err(e) => println!("Twitch fetch_status failed (possible proxy error): {:?}", e),
        }
    }
}
// Note: test_twitch_fetch_status should not fail the test suite even if blocked.
// We intercept errors inside tests.
