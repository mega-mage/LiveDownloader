use crate::common::client::create_http_client;
use crate::config::AppConfig;
use serde_json::json;
use tracing::{error, info, warn};

pub struct Notifier;

impl Notifier {
    pub fn new() -> Self {
        Self
    }

    pub async fn upload_file_to_telegram(
        &self,
        file_path: &std::path::Path,
        caption: &str,
        config: &AppConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !config.push.push_channels.contains(&"telegram".to_string()) {
            return Ok(());
        }

        let token = match &config.push.tg_token {
            Some(t) => t,
            None => return Err("Telegram push is enabled but Bot Token is missing".into()),
        };
        let chat_id = match &config.push.tg_chat_id {
            Some(c) => c,
            None => return Err("Telegram push is enabled but Chat ID is missing".into()),
        };

        let metadata = std::fs::metadata(file_path)?;
        let file_size = metadata.len();

        let has_custom_server = config
            .push
            .tg_api_url
            .as_ref()
            .map_or(false, |url| !url.trim().is_empty());
        let max_size = if has_custom_server {
            2000 * 1024 * 1024 // 2GB for self-hosted Telegram API server
        } else {
            50 * 1024 * 1024 // 50MB default limit
        };

        if file_size > max_size {
            let limit_str = if has_custom_server { "2GB" } else { "50MB" };
            warn!(
                "Segment file {:?} size ({} bytes) exceeds Telegram Bot {} limit. Skipping upload.",
                file_path, file_size, limit_str
            );
            return Ok(());
        }

        info!("Uploading segment to Telegram: {:?}", file_path);

        let file_bytes = tokio::fs::read(file_path).await?;
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("segment.ts")
            .to_string();

        let client = create_http_client(None, 60)?; // 60 seconds timeout for large uploads

        let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

        let form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.clone())
            .text("caption", caption.to_string())
            .part("document", part);

        let base_api_url = config
            .push
            .tg_api_url
            .as_deref()
            .unwrap_or("https://api.telegram.org");

        let url = format!(
            "{}/bot{}/sendDocument",
            base_api_url.trim_end_matches('/'),
            token
        );
        let resp = client.post(&url).multipart(form).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Telegram sendDocument failed with status {}: {}",
                status, err_text
            )
            .into());
        }

        info!("Successfully uploaded segment {:?} to Telegram", file_path);
        Ok(())
    }

    /// Unified entry to send notification to all enabled channels
    pub async fn notify(&self, title: &str, content: &str, config: &AppConfig) {
        if config.push.push_channels.is_empty() {
            return;
        }

        info!(
            "Sending notifications to channels: {:?}",
            config.push.push_channels
        );

        for channel in &config.push.push_channels {
            match channel.as_str() {
                "dingtalk" => {
                    if let Some(ref api_url) = config.push.dingtalk_api {
                        if let Err(e) = self.send_dingtalk(api_url, content).await {
                            error!("DingTalk notification error: {}", e);
                        }
                    } else {
                        error!("DingTalk was enabled but 'dingtalk_api' is missing in config");
                    }
                }
                "bark" => {
                    if let Some(ref api_url) = config.push.bark_api {
                        if let Err(e) = self.send_bark(api_url, title, content).await {
                            error!("Bark notification error: {}", e);
                        }
                    } else {
                        error!("Bark was enabled but 'bark_api' is missing in config");
                    }
                }
                "telegram" => {
                    if let (Some(token), Some(chat_id)) =
                        (&config.push.tg_token, &config.push.tg_chat_id)
                    {
                        if let Err(e) = self
                            .send_telegram(
                                token,
                                chat_id,
                                title,
                                content,
                                config.push.tg_api_url.as_deref(),
                            )
                            .await
                        {
                            error!("Telegram notification error: {}", e);
                        }
                    } else {
                        error!(
                            "Telegram was enabled but 'tg_token' or 'tg_chat_id' is missing in config"
                        );
                    }
                }
                other => {
                    error!("Unsupported notification channel: {}", other);
                }
            }
        }
    }

    async fn send_telegram(
        &self,
        token: &str,
        chat_id: &str,
        title: &str,
        content: &str,
        api_url: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(None, 10)?;
        let base_api_url = api_url.unwrap_or("https://api.telegram.org");
        let url = format!(
            "{}/bot{}/sendMessage",
            base_api_url.trim_end_matches('/'),
            token
        );
        let full_text = format!("【{}】\n{}", title, content);

        let payload = json!({
            "chat_id": chat_id,
            "text": full_text
        });

        let resp = client.post(&url).json(&payload).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Telegram server returned HTTP status: {} - {}",
                status, error_text
            )
            .into());
        }

        info!("Successfully sent Telegram push notification");
        Ok(())
    }

    async fn send_dingtalk(
        &self,
        api_url: &str,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(None, 10)?;
        let payload = json!({
            "msgtype": "text",
            "text": {
                "content": content
            }
        });

        let resp = client.post(api_url).json(&payload).send().await?;

        if !resp.status().is_success() {
            return Err(format!("DingTalk server returned HTTP status: {}", resp.status()).into());
        }

        info!("Successfully sent DingTalk push notification");
        Ok(())
    }

    async fn send_bark(
        &self,
        api_url: &str,
        title: &str,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = create_http_client(None, 10)?;
        // Format of Bark API is usually: https://api.day.app/yourkey/title/content
        // Or via POST JSON payload
        let payload = json!({
            "title": title,
            "body": content
        });

        let resp = client.post(api_url).json(&payload).send().await?;

        if !resp.status().is_success() {
            return Err(format!("Bark server returned HTTP status: {}", resp.status()).into());
        }

        info!("Successfully sent Bark push notification");
        Ok(())
    }
}
