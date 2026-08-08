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
#[allow(dead_code)]
pub fn prompt_overwrite(platform: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "user32")]
    unsafe extern "system" {
        #[allow(dead_code)]
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
    let cmd_line = args[1..].join(" ");
    let output = execute_cli_str(config_path, &cmd_line);
    println!("{}", output);
    Ok(())
}

pub fn split_arguments(command_line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in command_line.trim().chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ' ' | '\u{3000}' if !in_quotes => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

pub fn execute_cli_str(config_path: &Path, command_line: &str) -> String {
    let raw_args = split_arguments(command_line);
    if raw_args.is_empty() {
        return "错误: 指令为空。".to_string();
    }

    let args: Vec<String> = if raw_args[0].eq_ignore_ascii_case("ld") || raw_args[0].eq_ignore_ascii_case("ld.exe") {
        raw_args.into_iter().skip(1).collect()
    } else {
        raw_args
    };

    if args.is_empty() {
        return get_cli_help();
    }

    let cmd = args[0].to_lowercase();
    match cmd.as_str() {
        "add" => handle_cli_add(config_path, &args),
        "ls" | "list" => handle_cli_ls(config_path, &args),
        "del" | "delete" | "rm" => handle_cli_del(config_path, &args),
        "push" => handle_cli_push(config_path, &args),
        "api_token" | "token" => handle_cli_token(config_path, &args),
        "path" => {
            #[cfg(target_os = "windows")]
            {
                match add_to_path() {
                    Ok(_) => "已成功执行添加 PATH 逻辑。".to_string(),
                    Err(e) => format!("添加系统 PATH 失败: {}", e),
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                "path 命令仅支持 Windows 系统。".to_string()
            }
        }
        "server" => "要启动独立 Web 服务端模式，请运行:\n  cargo run --no-default-features --features server -- --port 10830".to_string(),
        "stop" | "shutdown" | "exit" | "quit" => {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                std::process::exit(0);
            });
            "正在停止 LiveDownloader 后端服务，进程即将在 500ms 后退出...".to_string()
        }
        "help" | "-h" | "--help" => get_cli_help(),
        other => format!("未知命令: '{}'。输入 'help' 查看可用的指令列表。", other),
    }
}

fn handle_cli_add(config_path: &Path, args: &[String]) -> String {
    if args.len() >= 2 && (args[1] == "cookies" || args[1] == "cookie") {
        if args.len() < 4 {
            return "用法: ld add cookies <平台名称> <Cookie内容>".to_string();
        }
        let platform_input = &args[2];
        let cookie_content = &args[3];

        let mapped_key = match platform_input.to_lowercase().as_str() {
            "抖音" | "抖音cookie" | "douyin" | "dy" => Some("抖音cookie".to_string()),
            "b站" | "b站cookie" | "bilibili" | "b" | "bz" => Some("b站cookie".to_string()),
            "虎牙" | "虎牙cookie" | "huya" | "hy" => Some("虎牙cookie".to_string()),
            "快手" | "快手cookie" | "kuaishou" | "ks" => Some("快手cookie".to_string()),
            "斗鱼" | "斗鱼cookie" | "douyu" | "dyu" => Some("斗鱼cookie".to_string()),
            "猫耳" | "猫耳cookie" | "maoer" | "maoerfm" | "me" => Some("猫耳cookie".to_string()),
            "网易cc" | "网易cccookie" | "wangyicc" | "cc" => Some("网易cccookie".to_string()),
            "微博" | "微博cookie" | "weibo" | "wb" => Some("微博cookie".to_string()),
            "淘宝" | "淘宝cookie" | "taobao" | "tb" => Some("淘宝cookie".to_string()),
            "a站" | "a站cookie" | "acfun" | "ac" => Some("A站cookie".to_string()),
            "twitch" | "twitchcookie" | "tc" => Some("Twitchcookie".to_string()),
            _ => None,
        };

        let cookie_key = match mapped_key {
            Some(key) => key,
            None => {
                return format!("错误: 不支持的平台 '{}'！支持的平台: 抖音(douyin), b站(bilibili), 虎牙(huya), 快手(kuaishou), 斗鱼(douyu), 猫耳(maoer), 网易cc(cc), 微博(weibo), 淘宝(taobao), a站(acfun), twitch", platform_input);
            }
        };

        let mut config = match AppConfig::load_or_create(config_path) {
            Ok(c) => c,
            Err(e) => return format!("错误: 读取配置失败: {}", e),
        };
        config.cookies.insert(cookie_key.clone(), cookie_content.trim().to_string());
        if let Err(e) = config.save_to_file(config_path) {
            return format!("错误: 保存配置失败: {}", e);
        }
        format!("成功保存平台 '{}' 的 Cookie！", cookie_key)
    } else {
        if args.len() < 2 {
            return "用法: ld add <直播间地址> [名称] [画质]".to_string();
        }
        let url = &args[1];
        let name = args.get(2).cloned();
        let quality = args.get(3).cloned();

        let mut config = match AppConfig::load_or_create(config_path) {
            Ok(c) => c,
            Err(e) => return format!("错误: 读取配置失败: {}", e),
        };
        let mut clean_url = url.trim().to_string();
        if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
            clean_url = format!("https://{}", clean_url);
        }

        if config.rooms.iter().any(|r| r.url == clean_url) {
            return "错误: 该直播间地址已在监控列表中！".to_string();
        }

        config.rooms.push(LiveUrlConfig {
            url: clean_url.clone(),
            name,
            quality,
            video_save_type: None,
            is_commented: false,
        });

        if let Err(e) = config.save_to_file(config_path) {
            return format!("错误: 保存配置失败: {}", e);
        }
        format!("成功添加直播间监控: {}", clean_url)
    }
}

fn handle_cli_ls(config_path: &Path, args: &[String]) -> String {
    let config = match AppConfig::load_or_create(config_path) {
        Ok(c) => c,
        Err(e) => return format!("错误: 读取配置失败: {}", e),
    };
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

    let mut out = String::new();
    out.push_str(&format!("{:<5} | {:<10} | {:<20} | {}\n", "序号", "状态", "主播名称", "直播间地址"));
    out.push_str(&format!("{}\n", "-".repeat(75)));

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

        out.push_str(&format!("{:<5} | {:<10} | {:<20} | {}\n", idx, status_str, anchor_name, room.url));
        count += 1;
    }
    out.push_str(&format!("{}\n", "-".repeat(75)));
    out.push_str(&format!("共 {} 个监控项目", count));
    out
}

fn handle_cli_del(config_path: &Path, args: &[String]) -> String {
    if args.len() < 2 {
        return "用法: ld del <序号 或 直播间地址>".to_string();
    }
    let target = &args[1];
    let mut config = match AppConfig::load_or_create(config_path) {
        Ok(c) => c,
        Err(e) => return format!("错误: 读取配置失败: {}", e),
    };

    if let Ok(idx) = target.parse::<usize>() {
        if idx < config.rooms.len() {
            let removed_room = config.rooms.remove(idx);
            let _ = config.save_to_file(config_path);
            format!("成功删除序号为 {} 的监控: {}", idx, removed_room.url)
        } else {
            format!("错误: 序号 {} 超出范围！当前监控项共 {} 项。", idx, config.rooms.len())
        }
    } else {
        let mut clean_url = target.trim().to_string();
        if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
            clean_url = format!("https://{}", clean_url);
        }

        let original_len = config.rooms.len();
        config.rooms.retain(|r| r.url != clean_url);
        if config.rooms.len() < original_len {
            let _ = config.save_to_file(config_path);
            format!("成功删除监控地址: {}", clean_url)
        } else {
            format!("错误: 未找到监控地址 '{}'", clean_url)
        }
    }
}

fn handle_cli_push(config_path: &Path, args: &[String]) -> String {
    if args.len() < 2 {
        let mut help = String::new();
        help.push_str("用法:\n");
        help.push_str("  push ls                                     列出当前推送配置\n");
        help.push_str("  push enable <dingtalk|bark|telegram|upload> 开启指定推送通道\n");
        help.push_str("  push disable <dingtalk|bark|telegram|upload> 关闭指定推送通道\n");
        help.push_str("  push set <dingtalk|bark|tg_token|tg_chat_id|upload> <值> 设置参数");
        return help;
    }

    let sub_cmd = args[1].as_str();
    let mut config = match AppConfig::load_or_create(config_path) {
        Ok(c) => c,
        Err(e) => return format!("错误: 读取配置失败: {}", e),
    };

    match sub_cmd {
        "ls" => {
            let mut out = String::new();
            out.push_str("--- 消息推送配置 ---\n");
            out.push_str(&format!("当前启用的通道: {:?}\n", config.push.push_channels));
            out.push_str(&format!("钉钉 API 地址: {}\n", config.push.dingtalk_api.as_deref().unwrap_or("未配置")));
            out.push_str(&format!("Bark API 地址: {}\n", config.push.bark_api.as_deref().unwrap_or("未配置")));
            out.push_str(&format!("Telegram Chat ID: {}\n", config.push.tg_chat_id.as_deref().unwrap_or("未配置")));
            out.push_str(&format!("Telegram 自动上传: {}", if config.push.tg_auto_upload { "已开启" } else { "已关闭" }));
            out
        }
        "enable" => {
            if args.len() < 3 {
                return "错误: 请指定要开启的通道 (dingtalk, bark, telegram 或 upload)".to_string();
            }
            let channel = args[2].to_lowercase();
            if channel == "upload" || channel == "tg_auto_upload" {
                config.push.tg_auto_upload = true;
                let _ = config.save_to_file(config_path);
                return "成功开启 Telegram 视频切片自动上传功能。".to_string();
            }
            if !config.push.push_channels.contains(&channel) {
                config.push.push_channels.push(channel.clone());
                let _ = config.save_to_file(config_path);
                format!("成功开启推送通道: {}", channel)
            } else {
                format!("推送通道 {} 已经是开启状态。", channel)
            }
        }
        "disable" => {
            if args.len() < 3 {
                return "错误: 请指定要关闭的通道 (dingtalk, bark, telegram 或 upload)".to_string();
            }
            let channel = args[2].to_lowercase();
            if channel == "upload" || channel == "tg_auto_upload" {
                config.push.tg_auto_upload = false;
                let _ = config.save_to_file(config_path);
                return "成功关闭 Telegram 视频切片自动上传功能。".to_string();
            }
            let original_len = config.push.push_channels.len();
            config.push.push_channels.retain(|c| c != &channel);
            if config.push.push_channels.len() < original_len {
                let _ = config.save_to_file(config_path);
                format!("成功关闭推送通道: {}", channel)
            } else {
                format!("推送通道 {} 已经是关闭状态。", channel)
            }
        }
        "set" => {
            if args.len() < 4 {
                return "用法: push set <dingtalk|bark|tg_token|tg_chat_id|upload> <值>".to_string();
            }
            let channel = args[2].to_lowercase();
            let val = &args[3];

            if channel == "dingtalk" {
                config.push.dingtalk_api = Some(val.clone());
                let _ = config.save_to_file(config_path);
                "成功设置 钉钉 API 地址。".to_string()
            } else if channel == "bark" {
                config.push.bark_api = Some(val.clone());
                let _ = config.save_to_file(config_path);
                "成功设置 Bark API 地址。".to_string()
            } else if channel == "tg_token" || channel == "token" {
                config.push.tg_token = Some(val.clone());
                let _ = config.save_to_file(config_path);
                "成功设置 Telegram Bot Token。".to_string()
            } else if channel == "tg_chat_id" || channel == "chat_id" {
                config.push.tg_chat_id = Some(val.clone());
                let _ = config.save_to_file(config_path);
                "成功设置 Telegram Chat ID。".to_string()
            } else {
                format!("错误: 不支持的推送配置项 '{}'", channel)
            }
        }
        other => format!("未知 push 命令: '{}'", other),
    }
}

fn handle_cli_token(config_path: &Path, args: &[String]) -> String {
    if args.len() < 2 {
        let config = AppConfig::load_or_create(config_path).unwrap_or_default();
        let current = config.settings.api_token.as_deref().unwrap_or("未设置");
        format!("当前 API Token: {}\n用法: token <新Token|clear>", current)
    } else {
        let token = &args[1];
        let mut config = match AppConfig::load_or_create(config_path) {
            Ok(c) => c,
            Err(e) => return format!("错误: 读取配置失败: {}", e),
        };
        if token == "clear" || token == "none" || token.is_empty() {
            config.settings.api_token = None;
            let _ = config.save_to_file(config_path);
            "已成功清除 API Token。".to_string()
        } else {
            config.settings.api_token = Some(token.trim().to_string());
            let _ = config.save_to_file(config_path);
            format!("成功设置 API Token 为: {}", token.trim())
        }
    }
}

fn get_cli_help() -> String {
    let mut h = String::new();
    h.push_str("LiveDownloader 交互式控制台指令帮助:\n");
    h.push_str("  ls [-live]                列出所有监控的直播间 (加 -live 仅显示直播中)\n");
    h.push_str("  add <地址> [名称] [画质]   添加要监控录制的直播间\n");
    h.push_str("  add cookies <平台> <值>   配置/覆盖对应平台的 Cookie 凭证\n");
    h.push_str("  del <序号 或 地址>         删除指定的直播间监控\n");
    h.push_str("  token [Token]              查看或设置 API 认证 Token\n");
    h.push_str("  push <ls|enable|disable>   消息推送通道查看与配置\n");
    h.push_str("  stop / shutdown            安全停止并退出后端服务进程\n");
    h.push_str("  clear                      清空终端屏幕\n");
    h.push_str("  help                       显示本帮助信息");
    h
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
