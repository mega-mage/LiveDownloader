use crate::config::{AppConfig, LiveUrlConfig};
use crate::engine::manager::RoomStatus;
use std::collections::HashMap;
use std::path::Path;

#[cfg(target_os = "windows")]
mod win_console {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    pub fn attach() {
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn prompt_overwrite(platform: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            hWnd: *mut std::ffi::c_void,
            lpText: *const u16,
            lpCaption: *const u16,
            uType: u32,
        ) -> i32;
    }

    let text: Vec<u16> = OsStr::new(&format!(
        "平台 '{}' 的 Cookie 已经存在。是否覆盖？",
        platform
    ))
    .encode_wide()
    .chain(std::iter::once(0))
    .collect();

    let caption: Vec<u16> = OsStr::new("Cookie 覆盖确认")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        // MB_YESNO = 4, MB_ICONQUESTION = 32, IDYES = 6
        let result = MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            4 | 32,
        );
        result == 6
    }
}

#[cfg(not(target_os = "windows"))]
pub fn prompt_overwrite(platform: &str) -> bool {
    use std::io::Write;
    print!("平台 '{}' 的 Cookie 已经存在。是否覆盖？[Y/n]: ", platform);
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if let Ok(_) = std::io::stdin().read_line(&mut answer) {
        let answer = answer.trim().to_lowercase();
        answer == "y" || answer == "yes" || answer.is_empty()
    } else {
        false
    }
}

pub fn run_cli_commands(config_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(target_os = "windows")]
    win_console::attach();

    let args: Vec<String> = std::env::args().collect();
    let cmd = args[1].as_str();

    match cmd {
        "add" => {
            if args.len() >= 3 && args[2] == "cookies" {
                if args.len() < 5 {
                    println!("用法: ld add cookies <平台名称> <Cookie内容>");
                    return Ok(());
                }
                let platform_input = &args[3];
                let cookie_content = &args[4];

                let mapped_key = match platform_input.to_lowercase().as_str() {
                    "抖音" | "抖音cookie" | "douyin" | "dy" => Some("抖音cookie".to_string()),
                    "b站" | "b站cookie" | "bilibili" | "b" | "bz" => {
                        Some("b站cookie".to_string())
                    }
                    "虎牙" | "虎牙cookie" | "huya" | "hy" => Some("虎牙cookie".to_string()),
                    "快手" | "快手cookie" | "kuaishou" | "ks" => Some("快手cookie".to_string()),
                    "斗鱼" | "斗鱼cookie" | "douyu" | "dyu" => Some("斗鱼cookie".to_string()),
                    "猫耳" | "猫耳cookie" | "maoer" | "maoerfm" | "me" => {
                        Some("猫耳cookie".to_string())
                    }
                    "网易cc" | "网易cccookie" | "wangyicc" | "cc" => {
                        Some("网易cccookie".to_string())
                    }
                    "微博" | "微博cookie" | "weibo" | "wb" => Some("微博cookie".to_string()),
                    "淘宝" | "淘宝cookie" | "taobao" | "tb" => Some("淘宝cookie".to_string()),
                    "a站" | "a站cookie" | "acfun" | "ac" => Some("A站cookie".to_string()),
                    "twitch" | "twitchcookie" | "tc" => Some("Twitchcookie".to_string()),
                    _ => None,
                };

                let cookie_key = match mapped_key {
                    Some(key) => key,
                    None => {
                        println!("错误: 不支持的平台 '{}'！", platform_input);
                        println!(
                            "支持的平台包含: 抖音(douyin), b站(bilibili), 虎牙(huya), 快手(kuaishou), 斗鱼(douyu), 猫耳(maoer), 网易cc(cc), 微博(weibo), 淘宝(taobao), a站(acfun), twitch"
                        );
                        return Ok(());
                    }
                };

                let mut config = AppConfig::load_or_create(config_path)?;
                let exists = config
                    .cookies
                    .get(&cookie_key)
                    .map_or(false, |c| !c.is_empty());

                if exists {
                    if !prompt_overwrite(&cookie_key) {
                        println!("操作已取消。");
                        return Ok(());
                    }
                }

                config
                    .cookies
                    .insert(cookie_key.clone(), cookie_content.trim().to_string());
                config.save_to_file(config_path)?;
                println!("成功保存平台 '{}' 的 Cookie！", cookie_key);
            } else {
                if args.len() < 3 {
                    println!("用法: ld add <直播间地址> [名称] [画质]");
                    println!("      ld add cookies <平台名称> <Cookie内容>");
                    return Ok(());
                }
                let url = &args[2];
                let name = args.get(3).cloned();
                let quality = args.get(4).cloned();

                let mut config = AppConfig::load_or_create(config_path)?;
                let mut clean_url = url.trim().to_string();
                if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
                    clean_url = format!("https://{}", clean_url);
                }

                if config.rooms.iter().any(|r| r.url == clean_url) {
                    println!("错误: 该直播间地址已在监控列表中！");
                    return Ok(());
                }

                config.rooms.push(LiveUrlConfig {
                    url: clean_url.clone(),
                    name,
                    quality,
                    video_save_type: None,
                    is_commented: false,
                });

                config.save_to_file(config_path)?;
                println!("成功添加直播间: {}", clean_url);
            }
        }
        "ls" => {
            let config = AppConfig::load_or_create(config_path)?;
            let only_live = args.iter().any(|a| a == "-live" || a == "--live");

            let status_path = config_path.parent().unwrap().join("statuses.json");
            let statuses: HashMap<String, RoomStatus> = if status_path.exists() {
                if let Ok(content) = std::fs::read_to_string(status_path) {
                    serde_json::from_str(&content).unwrap_or_default()
                } else {
                    HashMap::new()
                }
            } else {
                HashMap::new()
            };

            println!(
                "{:<5} | {:<10} | {:<20} | {}",
                "序号", "状态", "主播名称", "直播间地址"
            );
            println!("{}", "-".repeat(80));

            let mut count = 0;
            for (idx, room) in config.rooms.iter().enumerate() {
                let live_status = statuses.get(&room.url);
                let status_str = if room.is_commented {
                    "Paused"
                } else {
                    live_status.map(|s| s.status.as_str()).unwrap_or("Idle")
                };

                if only_live && status_str != "Living" {
                    continue;
                }

                let anchor_name = room
                    .name
                    .clone()
                    .or_else(|| live_status.map(|s| s.anchor_name.clone()))
                    .unwrap_or_else(|| "未知".to_string());

                println!(
                    "{:<5} | {:<10} | {:<20} | {}",
                    idx, status_str, anchor_name, room.url
                );
                count += 1;
            }
            println!("{}", "-".repeat(80));
            println!("共 {} 个监控项目", count);
        }
        "del" => {
            if args.len() < 3 {
                println!("用法: ld del <序号 或 直播间地址>");
                return Ok(());
            }
            let target = &args[2];
            let mut config = AppConfig::load_or_create(config_path)?;

            let mut removed = false;
            if let Ok(idx) = target.parse::<usize>() {
                if idx < config.rooms.len() {
                    let removed_room = config.rooms.remove(idx);
                    println!("成功删除序号为 {} 的监控: {}", idx, removed_room.url);
                    removed = true;
                } else {
                    println!("错误: 序号 {} 超出范围！", idx);
                }
            } else {
                let mut clean_url = target.trim().to_string();
                if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
                    clean_url = format!("https://{}", clean_url);
                }

                let original_len = config.rooms.len();
                config.rooms.retain(|r| r.url != clean_url);
                if config.rooms.len() < original_len {
                    println!("成功删除监控地址: {}", clean_url);
                    removed = true;
                } else {
                    println!("错误: 未找到该监控地址！");
                }
            }

            if removed {
                config.save_to_file(config_path)?;
            }
        }
        "path" => {
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = add_to_path() {
                    println!("添加系统 PATH 失败: {}", e);
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                println!("path 命令目前仅支持 Windows 系统。");
            }
        }
        "push" => {
            if args.len() < 3 {
                println!("用法:");
                println!("  ld push ls                                     列出当前推送配置");
                println!(
                    "  ld push enable <dingtalk|bark|telegram|upload> 开启指定推送通道或切片自动上传"
                );
                println!(
                    "  ld push disable <dingtalk|bark|telegram|upload> 关闭指定推送通道或切片自动上传"
                );
                println!(
                    "  ld push set <dingtalk|bark|tg_token|tg_chat_id|tg_api_url|upload> <值> 设置参数"
                );
                println!("  ld push test                                   发送一条测试推送消息");
                return Ok(());
            }
            let sub_cmd = args[2].as_str();
            let mut config = AppConfig::load_or_create(config_path)?;

            match sub_cmd {
                "ls" => {
                    println!("--- 消息推送配置 ---");
                    println!("当前启用的通道: {:?}", config.push.push_channels);
                    println!(
                        "钉钉 API 地址: {}",
                        config.push.dingtalk_api.as_deref().unwrap_or("未配置")
                    );
                    println!(
                        "Bark API 地址: {}",
                        config.push.bark_api.as_deref().unwrap_or("未配置")
                    );
                    println!(
                        "Telegram Bot Token: {}",
                        config
                            .push
                            .tg_token
                            .as_deref()
                            .map(|t| if t.len() > 10 {
                                format!("{}...", &t[..8])
                            } else {
                                t.to_string()
                            })
                            .unwrap_or_else(|| "未配置".to_string())
                    );
                    println!(
                        "Telegram Chat ID  : {}",
                        config.push.tg_chat_id.as_deref().unwrap_or("未配置")
                    );
                    println!(
                        "Telegram API 地址 : {}",
                        config
                            .push
                            .tg_api_url
                            .as_deref()
                            .unwrap_or("https://api.telegram.org (默认)")
                    );
                    println!(
                        "Telegram 自动上传 : {}",
                        if config.push.tg_auto_upload {
                            "已开启"
                        } else {
                            "已关闭"
                        }
                    );
                }
                "enable" => {
                    if args.len() < 4 {
                        println!("错误: 请指定要开启的通道 (dingtalk, bark, telegram 或 upload)");
                        return Ok(());
                    }
                    let channel = args[3].to_lowercase();
                    if channel == "upload" || channel == "tg_auto_upload" {
                        config.push.tg_auto_upload = true;
                        config.save_to_file(config_path)?;
                        println!("成功开启 Telegram 视频切片自动上传功能。");
                        return Ok(());
                    }
                    if channel != "dingtalk" && channel != "bark" && channel != "telegram" {
                        println!(
                            "错误: 不支持的通道/参数 '{}'！仅支持 dingtalk, bark, telegram 或 upload。",
                            channel
                        );
                        return Ok(());
                    }
                    if !config.push.push_channels.contains(&channel) {
                        config.push.push_channels.push(channel.clone());
                        config.save_to_file(config_path)?;
                        println!("成功开启推送通道: {}", channel);
                    } else {
                        println!("推送通道 {} 已经是开启状态。", channel);
                    }
                }
                "disable" => {
                    if args.len() < 4 {
                        println!("错误: 请指定要关闭的通道 (dingtalk, bark, telegram 或 upload)");
                        return Ok(());
                    }
                    let channel = args[3].to_lowercase();
                    if channel == "upload" || channel == "tg_auto_upload" {
                        config.push.tg_auto_upload = false;
                        config.save_to_file(config_path)?;
                        println!("成功关闭 Telegram 视频切片自动上传功能。");
                        return Ok(());
                    }
                    let original_len = config.push.push_channels.len();
                    config.push.push_channels.retain(|c| c != &channel);
                    if config.push.push_channels.len() < original_len {
                        config.save_to_file(config_path)?;
                        println!("成功关闭推送通道: {}", channel);
                    } else {
                        println!("推送通道 {} 已经是关闭状态。", channel);
                    }
                }
                "set" => {
                    if args.len() < 5 {
                        println!(
                            "用法: ld push set <dingtalk|bark|tg_token|tg_chat_id|upload> <值>"
                        );
                        return Ok(());
                    }
                    let channel = args[3].to_lowercase();
                    let val = &args[4];

                    if channel == "dingtalk" {
                        config.push.dingtalk_api = Some(val.clone());
                        config.save_to_file(config_path)?;
                        println!("成功设置 钉钉 API 地址。");
                    } else if channel == "bark" {
                        config.push.bark_api = Some(val.clone());
                        config.save_to_file(config_path)?;
                        println!("成功设置 Bark API 地址。");
                    } else if channel == "tg_token"
                        || channel == "tg-token"
                        || channel == "tg_bot_token"
                        || channel == "token"
                    {
                        config.push.tg_token = Some(val.clone());
                        config.save_to_file(config_path)?;
                        println!("成功设置 Telegram Bot Token。");
                    } else if channel == "tg_chat_id"
                        || channel == "tg-chat-id"
                        || channel == "chat_id"
                        || channel == "chatid"
                    {
                        config.push.tg_chat_id = Some(val.clone());
                        config.save_to_file(config_path)?;
                        println!("成功设置 Telegram Chat ID。");
                    } else if channel == "tg_auto_upload" || channel == "upload" {
                        let is_true = val.to_lowercase() == "true"
                            || val == "1"
                            || val.to_lowercase() == "on"
                            || val.to_lowercase() == "yes";
                        config.push.tg_auto_upload = is_true;
                        config.save_to_file(config_path)?;
                        println!(
                            "成功设置 Telegram 视频切片自动上传状态为: {}",
                            if is_true { "开启" } else { "关闭" }
                        );
                    } else if channel == "tg_api_url"
                        || channel == "tg-api-url"
                        || channel == "api_url"
                        || channel == "api-url"
                        || channel == "tg_server"
                    {
                        if val == "default" || val == "none" || val == "null" || val == "" {
                            config.push.tg_api_url = None;
                            println!("成功清除自定义 Telegram API 地址，恢复为官方默认端点。");
                        } else {
                            config.push.tg_api_url = Some(val.clone());
                            println!("成功设置 自定义 Telegram API 地址 为: {}", val);
                        }
                        config.save_to_file(config_path)?;
                    } else {
                        println!("错误: 不支持的参数名 '{}'！", channel);
                    }
                }
                "test" => {
                    println!("正在向所有开启的通道发送测试推送消息...");
                    let notifier = crate::engine::notifier::Notifier::new();
                    let title = "LiveDownloader 测试推送";
                    let content = "这是一条来自 LiveDownloader 命令行工具的测试推送消息。如果您收到这条消息，说明您的推送配置工作正常！";

                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    rt.block_on(async {
                        notifier.notify(title, content, &config).await;
                    });
                    println!("发送完毕！请检查您的接收客户端。");
                }
                other => {
                    println!("未知子命令: {}", other);
                }
            }
        }
        "api_token" | "token" => {
            if args.len() < 3 {
                let config = AppConfig::load_or_create(config_path)?;
                let current = config.settings.api_token.as_deref().unwrap_or("未设置");
                let display = if current.len() > 12 && current != "未设置" {
                    format!("{}...{}", &current[..4], &current[current.len()-4..])
                } else {
                    current.to_string()
                };
                println!("当前 API Token: {}", display);
                println!("用法: ld api_token <新Token>");
                return Ok(());
            }
            let token = &args[2];
            let mut config = AppConfig::load_or_create(config_path)?;
            if token == "clear" || token == "none" || token == "" {
                config.settings.api_token = None;
                config.save_to_file(config_path)?;
                println!("已清除 API Token。");
            } else {
                config.settings.api_token = Some(token.trim().to_string());
                config.save_to_file(config_path)?;
                println!("成功设置 API Token！");
            }
        }
        "server" => {
            println!("要启动独立 Web 服务端模式，请使用以下命令编译并运行:");
            println!("  cargo build --no-default-features --features server");
            println!("  ./LiveDownloader --server");
            println!("或直接运行:");
            println!("  ./LiveDownloader --server --port 10730");
        }
        "help" | "-h" | "--help" => {
            print_cli_help();
        }
        other => {
            println!("未知命令: {}", other);
            print_cli_help();
        }
    }
    Ok(())
}

fn print_cli_help() {
    println!("LiveDownloader 命令行工具");
    println!("用法:");
    println!("  ld <命令> [参数...]");
    println!();
    println!("命令列表:");
    println!("  add <地址> [名称] [画质]   添加要录制的直播间监控");
    println!("  add cookies <平台> <值>   添加或更新对应平台的 Cookie 凭证");
    println!("  ls [-live]                列出所有监控的直播间 (加 -live 仅列出正在录制的)");
    println!("  del <序号 或 地址>         删除指定的直播间监控");
    println!("  push <ls|enable|disable|set|test> [参数...]  消息推送服务配置与测试");
    println!("  path                      将当前程序所在目录加入系统环境变量 PATH 中");
    println!("  api_token [Token]          查看或设置 Web API 认证 Token");
    println!("  server                    显示独立 Web 服务端部署说明");
    println!("  help                      显示帮助信息");
}

#[cfg(target_os = "windows")]
fn add_to_path() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use winreg::RegKey;
    use winreg::enums::*;

    let current_exe = std::env::current_exe()?;
    let exe_dir = current_exe.parent().ok_or("No parent directory")?;
    let exe_dir_str = exe_dir.to_string_lossy().to_string();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
    let current_path: String = env_key.get_value("Path")?;

    let paths: Vec<&str> = current_path.split(';').collect();
    if paths
        .iter()
        .any(|p| p.trim().eq_ignore_ascii_case(&exe_dir_str))
    {
        println!("当前目录已在系统 PATH 中，无需重复添加。");
        return Ok(());
    }

    let separator = if current_path.ends_with(';') || current_path.is_empty() {
        ""
    } else {
        ";"
    };
    let new_path = format!("{}{}{}", current_path, separator, exe_dir_str);
    env_key.set_value("Path", &new_path)?;

    println!(
        "已成功将目录 [{}] 添加到用户的 PATH 环境变量！",
        exe_dir_str
    );
    println!("注意: 您可能需要重新启动您的命令行窗口以应用该更改。");

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendMessageTimeoutW(
            hWnd: *mut std::ffi::c_void,
            Msg: u32,
            wParam: usize,
            lParam: *const u16,
            fuFlags: u32,
            uTimeout: u32,
            lpdwResult: *mut usize,
        ) -> isize;
    }

    let msg = 0x001A; // WM_SETTINGCHANGE
    let param = "Environment\0".encode_utf16().collect::<Vec<u16>>();
    let mut result = 0;
    unsafe {
        SendMessageTimeoutW(
            std::ptr::null_mut(),
            msg,
            0,
            param.as_ptr(),
            0x0002, // SMTO_ABORTIFHUNG
            5000,
            &mut result,
        );
    }

    Ok(())
}
