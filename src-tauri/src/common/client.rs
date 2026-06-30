use reqwest::{Client, Proxy};
use std::time::Duration;

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[cfg(target_os = "windows")]
fn get_windows_system_proxy() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(settings) = hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings") {
        let enable: u32 = settings.get_value("ProxyEnable").unwrap_or(0);
        if enable == 1 {
            if let Ok(server) = settings.get_value::<String, _>("ProxyServer") {
                let server = server.trim();
                if !server.is_empty() {
                    if server.contains(';') {
                        for part in server.split(';') {
                            let part = part.trim();
                            if part.starts_with("http=") {
                                return Some(part["http=".len()..].to_string());
                            } else if part.starts_with("https=") {
                                return Some(part["https=".len()..].to_string());
                            }
                        }
                    } else {
                        return Some(server.to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn create_http_client(
    proxy_addr: Option<&str>,
    timeout_secs: u64,
) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    let mut builder = Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .timeout(Duration::from_secs(timeout_secs))
        .danger_accept_invalid_certs(true);

    #[cfg(target_os = "windows")]
    let effective_proxy = {
        let mut proxy = proxy_addr.map(|s| s.to_string());
        if proxy.is_none() || proxy.as_ref().unwrap().trim().is_empty() {
            proxy = get_windows_system_proxy();
        }
        proxy
    };

    #[cfg(not(target_os = "windows"))]
    let effective_proxy = proxy_addr.map(|s| s.to_string());

    if let Some(addr) = effective_proxy {
        let addr = addr.trim();
        if !addr.is_empty() {
            // Support http, https, socks5 protocols
            let proxy = if addr.starts_with("socks5://") || addr.starts_with("http://") || addr.starts_with("https://") {
                Proxy::all(addr)?
            } else {
                // Default to http proxy
                Proxy::all(format!("http://{}", addr))?
            };
            builder = builder.proxy(proxy);
        }
    }

    let client = builder.build()?;
    Ok(client)
}
