pub mod bilibili;
pub mod douyin;
pub mod douyin_sign;
pub mod huya;
pub mod kuaishou;
pub mod douyu;
pub mod maoerfm;
pub mod netease_cc;
pub mod weibo;
pub mod taobao;
pub mod acfun;
pub mod twitch;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum LiveStatus {
    Idle,
    Living {
        title: String,
        anchor_name: String,
        stream_urls: StreamUrls,
    },
    Error(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StreamUrls {
    pub m3u8_url: Option<String>,
    pub flv_url: Option<String>,
    pub record_url: String,
    pub headers: Option<HashMap<String, String>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub cookie: Option<String>,
    pub proxy: Option<String>,
    pub quality: String,
    pub extra: HashMap<String, String>,
}

#[async_trait]
pub trait LivePlatform: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn match_url(&self, url: &str) -> bool;
    async fn fetch_status(
        &self,
        url: &str,
        config: &PlatformConfig,
    ) -> Result<LiveStatus, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct PlatformManager {
    platforms: Vec<Arc<dyn LivePlatform>>,
}

impl PlatformManager {
    pub fn new() -> Self {
        Self {
            platforms: vec![
                Arc::new(bilibili::BilibiliPlatform::new()),
                Arc::new(douyin::DouyinPlatform::new()),
                Arc::new(huya::HuyaPlatform::new()),
                Arc::new(kuaishou::KuaishouPlatform::new()),
                Arc::new(douyu::DouyuPlatform::new()),
                Arc::new(maoerfm::MaoerfmPlatform::new()),
                Arc::new(netease_cc::NeteaseCcPlatform::new()),
                Arc::new(weibo::WeiboPlatform::new()),
                Arc::new(taobao::TaobaoPlatform::new()),
                Arc::new(acfun::AcfunPlatform::new()),
                Arc::new(twitch::TwitchPlatform::new()),
            ],
        }
    }

    pub fn find_handler(&self, url: &str) -> Option<Arc<dyn LivePlatform>> {
        self.platforms.iter()
            .find(|p| p.match_url(url))
            .cloned()
    }
}
