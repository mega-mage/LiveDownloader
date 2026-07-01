# LiveDownloader 后端 (Tauri & Axum Server)

本项目是 LiveDownloader 的 Rust 后端核心。它采用双模式（Dual-Mode）架构设计，既能作为 **Tauri 桌面端** 的底层逻辑，也能独立作为 **高性能 Web API 服务端**（基于 Axum 与 Tokio）部署于 NAS、本地 Termux 容器或远程 Linux 服务器。

---

## 🚀 核心架构与功能

1. **多流平台抓取与解析**：
   - 动态调用内置 JS 引擎（`boa_engine`）或正则解析，实时追踪并获取 哔哩哔哩、抖音、虎牙、快手、斗鱼、猫耳FM、网易CC、微博、淘宝、AcFun 及 Twitch 等平台的直播间开播状态与拉流地址。
2. **多线程并发调度**：
   - 基于 `tokio` 异步运行时，每个监控主播对应一个独立的异步工作流，按设定的轮询周期进行监测。
   - 自动检测主播开播状态，一旦开播立即动态孵化子进程调用 `ffmpeg` 开展视频切片下载。
3. **HTTP RESTful API & 跨域支持 (CORS)**：
   - 基于 `axum` 提供灵活的 Web API 路由，支持跨域访问。
   - 提供安全凭证校验（`api_token`），保障控制信道的私密性。
4. **实时日志追踪 & 交互式命令行 (ld CLI)**：
   - 实现了双向输出：既能向前端推送控制台日志，也能通过交互式命令行执行管理命令。

---

## 🛠️ 技术栈与依赖库

- **异步核心**：Tokio (异步 I/O 及多任务调度)
- **网络客户端**：Reqwest (异步 HTTP 请求，支持 SOCKS 代理及 Cookie 载入)
- **配置系统**：rust-ini & toml (多格式本地配置文件存取)
- **序列化**：Serde & serde_json (数据报文序列化传递)
- **Web API 框架**：Axum (路由、中间件与中间态拦截)
- **客户端容器**：Tauri v2 (桌面级窗口运行外壳)

---

## 📦 编译与运行指南

本仓库支持通过 Cargo Feature 进行条件编译，从而剔除不需要的依赖（例如在无界面的 Linux 服务器上剔除 GUI 部分）：

### 1. Tauri 桌面 GUI 模式 (默认运行)
若想开发并运行带窗口的桌面端，执行：
```bash
cargo run --features gui
```

### 2. Standalone API 服务端模式 (常用于 NAS/Linux/Termux)
若仅需要运行后台 Web API，请剔除 `gui` 依赖并编译 `server` 特性：
```bash
# 本地调试运行
cargo run --no-default-features --features server

# 编译生成生产模式下的独立静态二进制文件
cargo build --release --no-default-features --features server
```
*编译完成后，可在 `target/release/` 目录下找到可执行程序。*

---

## 📁 目录结构

```text
├── src/
│   ├── bin/              # 辅助工具及可执行入口
│   ├── common/           # 公共基础库（日志格式化、工具类等）
│   ├── engine/           # 录制引擎调度核心，管理 ffmpeg 进程及异步轮询
│   ├── platforms/        # 各大直播平台解析规则实现
│   ├── stream/           # 视频流解析及下载写入器
│   ├── main.rs           # 程序入口，在此决定拉起 GUI 还是 Axum API 服务
│   ├── server.rs         # 核心 API 服务路由及请求控制器 (Axum 实现)
│   ├── cli.rs            # ld 交互式命令行处理器
│   ├── config.rs         # 本地配置文件读写接口 (LiveDownloader.ini)
│   └── commands.rs       # 业务逻辑服务层 (供 Tauri 或 API 统一调用)
├── Cargo.toml            # 后端编译依赖及 Feature 定义
├── build.rs              # 针对 Tauri 桌面端的编译构建脚本
└── tauri.conf.json       # Tauri 窗口与安全权限设定文件
```

---

## 📱 安卓 Termux 手机部署说明

由于 Rust 在 ARM 架构上的优越表现，本后端可以直接运行在安卓手机的 **Termux** 终端内：
1. 在 Termux 内安装 Rust 和编译依赖环境：
   ```bash
   pkg update && pkg install clang rust make openssl -y
   ```
2. 检出代码并执行服务端条件编译：
   ```bash
   cargo build --release --no-default-features --features server
   ```
3. 运行 `./target/release/LiveDownloader --server --port 10730`，配合编译打包出的安卓前端 APK 实现完全无损的手机本地直播录制。
