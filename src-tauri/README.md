# LiveDownloader 后端 (Rust / Axum / Stream Proxy)

本项目是 LiveDownloader 的核心 Rust 后端。它采用双模式（Dual-Mode）架构设计，既能作为 **Tauri 桌面客户端** 的底层逻辑，也能独立作为 **高性能 Web API 服务端**（基于 Axum 与 Tokio）部署于 Linux 服务器、NAS 或安卓手机 Termux 容器中。

---

## 🚀 核心架构与功能

1. **多流平台抓取与解析**：
   - 实时解析 哔哩哔哩、抖音、虎牙、快手、斗鱼、猫耳FM、网易CC、微博、淘宝、AcFun 及 Twitch 等平台的直播间开播状态与拉流地址。
2. **多线程并发调度与录制**：
   - 基于 `tokio` 异步运行时，每个监控主播对应一个独立的异步工作流。
   - 监测到开播后立即孵化子进程调用 `ffmpeg` 进行全自动切片与录制。
3. **实时直播观战 (FFmpeg HLS Remux Proxy)**：
   - **Stream Proxy `/live` 端点**：在后端通过 FFmpeg 将 FLV/HLS/RTMP 等任意格式直播流实时 remux 为标准 HLS 分片。
   - 在 Windows 上采用 `CREATE_NO_WINDOW` 静默启动 FFmpeg，不弹黑框。
   - 自动会话管理，播放停止 30 秒后自动终止 FFmpeg 进程并清理临时分片目录。
4. **HTTP RESTful API & 跨域支持 (CORS)**：
   - 基于 `axum` 提供 Web API，支持跨域访问与安全凭证校验 (`api_token`)。
5. **交互式命令行与 Token 管理 (ld CLI)**：
   - 支持 `livedownloader token [Token|clear]` 实时查看、更新或清除 API 认证密钥。
   - 支持启动参数 `--token <Token>` 在服务拉起时自动写入配置。

---

## 🛠️ 自动化部署与多架构支持

### 1. 一键服务端安装脚本 (`install_server.sh`)
位于 `src-tauri/install_server.sh`，具备以下能力：
- **CPU 架构自动识别**：支持 `x86_64` (amd64)、`aarch64` (ARM64 / Cortex-A53/A72)、`armv7` (Cortex-A7 32位)。
- **免本地编译**：优先检查本地或自动向 GitHub Release 拉取对应 CPU 架构的预编译文件；若无法下载则回退至本地 `cargo build`。
- **Systemd 集成**：自动安装为 `livedownloader.service` 服务并配置开机自启。

### 2. Caddy HTTPS 反向代理脚本 (`setup_caddy_https.sh`)
位于项目根目录下，自动安装 Caddy 并申请 SSL 证书。针对 `/live/*` 和 `/proxy/*` 端点配置 `flush_interval -1`，消除直播流传输缓冲延迟。

---

## 📦 编译与运行指南

### 1. Tauri 桌面 GUI 模式 (默认)
```bash
cargo run --features gui
```

### 2. Standalone Web API 服务端模式
剔除 GUI 依赖并编译 `server` 特性：
```bash
# 本地开发调试
cargo run --no-default-features --features server

# 生产模式编译（输出到 target/release/LiveDownloader）
cargo build --release --no-default-features --features server

# 启动服务
./target/release/LiveDownloader --server --port 10730
```

### 3. CLI 命令列举
```bash
livedownloader add <直播间地址> [名称] [画质]  # 添加监控
livedownloader add cookies <平台> <Cookie>   # 添加平台 Cookie
livedownloader ls [-live]                   # 列出监控列表
livedownloader del <序号|地址>               # 删除监控
livedownloader token [新Token|clear]         # 管理 API Token
livedownloader push test                    # 测试消息推送
```

---

## 📁 目录结构

```text
├── src/
│   ├── engine/           # 录制引擎调度核心，管理 ffmpeg 进程及异步轮询
│   ├── platforms/        # 各大直播平台解析规则实现
│   ├── stream/           # Stream Proxy (包含 /proxy 转发与 /live FFmpeg HLS Remux)
│   ├── main.rs           # 程序入口，在此决定拉起 GUI 还是 Axum API 服务
│   ├── server.rs         # 核心 API 服务路由及请求控制器 (Axum 实现)
│   ├── cli.rs            # ld 命令行工具处理器 (含 Token 管理)
│   ├── config.rs         # 本地配置文件读写接口 (config.toml)
│   └── commands.rs       # 业务逻辑服务层 (供 Tauri 或 API 统一调用)
├── install_server.sh     # 多架构服务器部署脚本
├── Cargo.toml            # 编译依赖及 Feature 定义
└── tauri.conf.json       # Tauri 窗口与配置
```
