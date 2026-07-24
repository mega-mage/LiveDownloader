use reqwest::header::{HeaderMap, HeaderValue, COOKIE, REFERER, USER_AGENT, ACCEPT, ACCEPT_LANGUAGE};

#[tokio::main]
async fn main() {
    let web_rid = "578883147650";
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .build().unwrap();

    let mut ttwid = String::new();
    let body = serde_json::json!({
        "region": "cn",
        "aid": 1768,
        "needFp": "true",
        "service": "www.douyin.com",
        "clientInfo": {}
    });

    if let Ok(resp) = client.post("https://ttwid.bytedance.com/ttwid/union/register/")
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
                            ttwid = p_trimmed.to_string();
                        }
                    }
                }
            }
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36"));
    headers.insert(REFERER, HeaderValue::from_str(&format!("https://live.douyin.com/{}", web_rid)).unwrap());
    if !ttwid.is_empty() {
        headers.insert(COOKIE, HeaderValue::from_str(&ttwid).unwrap());
    }
    headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9"));

    let page_url = format!("https://live.douyin.com/{}", web_rid);
    let resp = client.get(&page_url).headers(headers.clone()).send().await.unwrap();
    let text = resp.text().await.unwrap();

    let (title, nickname) = parse_douyin_html_meta(&text);
    println!("[TEST EXTRACTION] Title: '{}', Anchor: '{}'", title, nickname);
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

    // 2. Extract Room Title (find title inside room JSON block containing status: 2)
    let mut title = "抖音直播间".to_string();
    
    // Pattern A: "title":"...", ... "status":2
    let title_status_re = regex::Regex::new(r#""title"\s*:\s*"([^"]+)"[^{}]*?"status"\s*:\s*2"#).unwrap();
    if let Some(cap) = title_status_re.captures(&clean_text) {
        title = cap[1].trim().to_string();
    } else {
        // Pattern B: "status":2, ... "title":"..."
        let status_title_re = regex::Regex::new(r#""status"\s*:\s*2[^{}]*?"title"\s*:\s*"([^"]+)""#).unwrap();
        if let Some(cap) = status_title_re.captures(&clean_text) {
            title = cap[1].trim().to_string();
        } else {
            // Pattern C: HTML title tag (e.g. <title>xxx的抖音直播间 - 抖音直播</title>)
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
