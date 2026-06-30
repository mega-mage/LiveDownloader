use std::sync::Arc;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt};
use tracing::{info, error, debug};
use reqwest::Client;

/// A lightweight local HTTP proxy server that forwards requests with proper headers.
/// This is needed because browser-based players (HLS.js) cannot set custom Referer/User-Agent
/// headers, and CDN servers (bilibili, douyin etc.) reject requests without them.
///
/// Usage: Frontend requests `http://127.0.0.1:{port}/proxy?url={encoded_url}`
/// The proxy fetches the URL with correct headers and streams the response back.

pub struct StreamProxy;

impl StreamProxy {
    /// Start the proxy server accept loop with an existing TcpListener.
    pub fn start_with_listener(listener: TcpListener) {
        let client = Arc::new(
            Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .expect("Failed to build reqwest client for proxy")
        );

        tokio::spawn(async move {
            info!("Stream proxy accept loop running");
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        let client = client.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, client).await {
                                debug!("Proxy connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Proxy accept error: {}", e);
                    }
                }
            }
        });
    }
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    client: Arc<Client>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Read the HTTP request
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let request_str = String::from_utf8_lossy(&buf[..n]);

    // Parse the request line
    let first_line = request_str.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        send_error(&mut stream, 400, "Bad Request").await?;
        return Ok(());
    }

    let path = parts[1];

    // Handle serving local recorded video files
    if path.starts_with("/video") {
        let query_start = path.find('?').unwrap_or(path.len());
        let query = &path[query_start + 1..];
        
        let local_path_encoded = parse_query_param(query, "path").unwrap_or_default();
        let local_path_str = urlencoding::decode(&local_path_encoded).unwrap_or(std::borrow::Cow::Borrowed("")).to_string();
        let local_path = PathBuf::from(&local_path_str);
        
        if !local_path.exists() {
            send_error(&mut stream, 404, "File Not Found").await?;
            return Ok(());
        }
        
        let is_playlist = parse_query_param(query, "playlist").map(|v| v == "true").unwrap_or(false);
        let ext = local_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        
        if ext == "ts" && is_playlist {
            let duration = 36000.0;
            let m3u8_content = format!(
                "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:{}\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXTINF:{},\n/video?path={}&segment=true\n#EXT-X-ENDLIST\n",
                duration, duration, urlencoding::encode(&local_path_str)
            );
            
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n{}",
                m3u8_content.len(),
                m3u8_content
            );
            stream.write_all(response.as_bytes()).await?;
            return Ok(());
        }
        
        let mut file = match tokio::fs::File::open(&local_path).await {
            Ok(f) => f,
            Err(e) => {
                send_error(&mut stream, 500, &format!("File open error: {}", e)).await?;
                return Ok(());
            }
        };
        
        let metadata = file.metadata().await?;
        let file_len = metadata.len();
        
        let mut range_start = 0;
        let mut range_end = file_len - 1;
        let mut is_partial = false;
        
        for line in request_str.lines() {
            if line.to_lowercase().starts_with("range:") {
                if let Some(pos) = line.find("bytes=") {
                    let range_val = &line[pos + 6..].trim();
                    let parts: Vec<&str> = range_val.split('-').collect();
                    if !parts.is_empty() {
                        if let Ok(start) = parts[0].trim().parse::<u64>() {
                            range_start = start;
                            is_partial = true;
                        }
                        if parts.len() > 1 && !parts[1].trim().is_empty() {
                            if let Ok(end) = parts[1].trim().parse::<u64>() {
                                range_end = std::cmp::min(end, file_len - 1);
                            }
                        }
                    }
                }
            }
        }
        
        let content_type = match ext.as_str() {
            "mp4" => "video/mp4",
            "m4a" => "audio/mp4",
            "mp3" => "audio/mpeg",
            "ts" => "video/mp2t",
            "flv" => "video/x-flv",
            "mkv" => "video/x-matroska",
            _ => "application/octet-stream",
        };
        
        if is_partial {
            if file.seek(std::io::SeekFrom::Start(range_start)).await.is_err() {
                send_error(&mut stream, 500, "Seek Error").await?;
                return Ok(());
            }
            
            let chunk_size = range_end - range_start + 1;
            let header = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Type: {}\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
                content_type, range_start, range_end, file_len, chunk_size
            );
            stream.write_all(header.as_bytes()).await?;
            
            let mut file_stream = tokio::io::AsyncReadExt::take(file, chunk_size);
            tokio::io::copy(&mut file_stream, &mut stream).await?;
        } else {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
                content_type, file_len
            );
            stream.write_all(header.as_bytes()).await?;
            tokio::io::copy(&mut file, &mut stream).await?;
        }
        
        return Ok(());
    }

    // Handle CORS preflight
    if parts[0] == "OPTIONS" {
        let response = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Parse query parameters from path
    let (target_url, custom_referer) = if let Some(query_start) = path.find('?') {
        let query = &path[query_start + 1..];
        (parse_query_param(query, "url"), parse_query_param(query, "referer"))
    } else {
        (None, None)
    };

    let target_url = match target_url {
        Some(u) => u,
        None => {
            send_error(&mut stream, 400, "Missing 'url' parameter").await?;
            return Ok(());
        }
    };

    debug!("Proxying request to: {}", &target_url[..std::cmp::min(120, target_url.len())]);

    // Determine referer
    let referer = if let Some(ref r) = custom_referer {
        r.clone()
    } else if target_url.contains("bilivideo") || target_url.contains("bilibili") {
        "https://live.bilibili.com/".to_string()
    } else if target_url.contains("douyin") || target_url.contains("douyincdn") || target_url.contains("bytecdn") {
        "https://live.douyin.com/".to_string()
    } else {
        "".to_string()
    };

    // Make the proxied request with proper headers
    let mut req = client.get(&target_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36");

    if !referer.is_empty() {
        req = req.header("Referer", referer);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Proxy fetch error: {}", e);
            send_error(&mut stream, 502, &format!("Upstream error: {}", e)).await?;
            return Ok(());
        }
    };

    let status = resp.status().as_u16();
    let content_type = resp.headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap_or("application/octet-stream"))
        .unwrap_or("application/octet-stream")
        .to_string();

    // For m3u8 playlists, we need to rewrite internal URLs to also go through the proxy
    let is_m3u8 = content_type.contains("mpegurl") 
        || content_type.contains("m3u8") 
        || target_url.ends_with(".m3u8") 
        || target_url.contains(".m3u8?");

    if is_m3u8 {
        let body = resp.text().await.unwrap_or_default();
        let rewritten = rewrite_m3u8(&body, &target_url, custom_referer.as_deref());
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n{}",
            rewritten.len(),
            rewritten
        );
        stream.write_all(response.as_bytes()).await?;
    } else {
        // Stream binary data (ts segments, etc.)
        let content_length = resp.content_length();
        let mut header = format!(
            "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n",
            status, content_type
        );
        if let Some(len) = content_length {
            header.push_str(&format!("Content-Length: {}\r\n", len));
        }
        header.push_str("\r\n");
        stream.write_all(header.as_bytes()).await?;

        // Stream the body
        use futures_util::StreamExt;
        let mut byte_stream = resp.bytes_stream();
        while let Some(chunk) = byte_stream.next().await {
            match chunk {
                Ok(data) => {
                    if stream.write_all(&data).await.is_err() {
                        break; // Client disconnected
                    }
                }
                Err(e) => {
                    debug!("Stream chunk error: {}", e);
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Rewrite relative URLs in m3u8 playlists to point through our proxy
fn rewrite_m3u8(content: &str, base_url: &str, referer: Option<&str>) -> String {
    let base = if let Some(pos) = base_url.rfind('/') {
        &base_url[..pos + 1]
    } else {
        base_url
    };

    let mut result = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            // Check for URI= attributes in EXT tags (e.g., EXT-X-MAP)
            if trimmed.contains("URI=\"") {
                let rewritten_line = rewrite_uri_attribute(trimmed, base, referer);
                result.push_str(&rewritten_line);
            } else {
                result.push_str(trimmed);
            }
        } else {
            // This is a URL line (segment or sub-playlist)
            let full_url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                trimmed.to_string()
            } else {
                format!("{}{}", base, trimmed)
            };
            let encoded = urlencoding::encode(&full_url);
            let mut proxy_path = format!("/proxy?url={}", encoded);
            if let Some(ref_val) = referer {
                proxy_path.push_str(&format!("&referer={}", urlencoding::encode(ref_val)));
            }
            result.push_str(&proxy_path);
        }
        result.push('\n');
    }
    result
}

/// Rewrite URI="..." attributes inside EXT tags
fn rewrite_uri_attribute(line: &str, base: &str, referer: Option<&str>) -> String {
    if let Some(start) = line.find("URI=\"") {
        let uri_start = start + 5; // skip URI="
        if let Some(end) = line[uri_start..].find('"') {
            let uri = &line[uri_start..uri_start + end];
            let full_url = if uri.starts_with("http://") || uri.starts_with("https://") {
                uri.to_string()
            } else {
                format!("{}{}", base, uri)
            };
            let encoded = urlencoding::encode(&full_url);
            let mut new_uri = format!("/proxy?url={}", encoded);
            if let Some(ref_val) = referer {
                new_uri.push_str(&format!("&referer={}", urlencoding::encode(ref_val)));
            }
            return format!("{}URI=\"{}\"{}",
                &line[..start],
                new_uri,
                &line[uri_start + end + 1..]
            );
        }
    }
    line.to_string()
}

fn parse_query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == key {
                return Some(urlencoding::decode(v).unwrap_or_default().to_string());
            }
        }
    }
    None
}

async fn send_error(
    stream: &mut tokio::net::TcpStream,
    code: u16,
    msg: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let body = format!("{{\"error\": \"{}\"}}", msg);
    let response = format!(
        "HTTP/1.1 {} Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        code,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}
