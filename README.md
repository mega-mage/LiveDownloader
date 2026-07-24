# LiveDownloader

LiveDownloader 是一款专为各大直播平台设计的高颜值、现代化自动录制与调度管理系统。支持以独立的**网页浏览器模式**（与远程服务器或 NAS 上的 API 进行通信）以及**桌面客户端模式**（基于 Tauri v2 + Rust）无缝运行。

---

## 🚀 核心功能与亮点

1. **实时工作台 (Dashboard)**：
   - **动态监控**：卡片与表格展示所有主播的开播状态（正在直播 / 离线 / 暂停监控）。
   - **快捷控制**：支持一键修改单个主播画质格式、暂停/启用监控、跳转实时观战或直接删除。
   - **平台支持**：支持解析 哔哩哔哩、抖音、虎牙、快手、斗鱼、猫耳FM、网易CC、微博、淘宝、AcFun 及 Twitch 等平台。
2. **实时直播观战 (FFmpeg HLS Remux)**：
   - 独创后端 **/live** 代理转码端点，通过 FFmpeg 将 FLV/HLS/RTMP 等任意格式直播流实时 remux 为标准 HLS 分片。
   - 前端集成 HLS.js 播放器，观战**完全不干扰**后台录制的进行。
   - 自动会话管理，播放停止 30 秒后自动静默清理 FFmpeg 进程与临时分片。
3. **已录视频切片管理 (Recorded Videos)**：
   - 支持根据主播名称、文件名称进行模糊双重过滤与“主播专属过滤”下拉选单。
   - 提供安全带签名的临时 HTTP 下载链接，支持网页在线放映与文件物理删除。
4. **系统参数设置 (System Settings)**：
   - 通信凭证：配置网页端与 NAS/远程服务器后端的 API 认证地址（支持命令行与 API Token 认证）。
   - 基础设置：视频默认保存路径、录制封装格式 (TS/MP4/MKV/FLV/MP3/M4A)、分段切片与轮询间隔。
   - 代理与推送：支持全局网络代理，支持钉钉机器人、Bark iOS 服务、Telegram 消息通知及录制切片自动上传（支持最大 2GB）。
   - 登录凭证管理 (Cookies)：导入 platform Cookies 解锁原画/蓝光超清码率。
5. **多架构与自动化运维 (CI/CD & Shell Automation)**：
   - **一键服务端部署脚本**：支持 Linux (x86_64, ARM64, ARMv7)，自动识别 CPU 架构，支持免本地编译（自动从 GitHub Release 拉取二进制）。
   - **Caddy HTTPS 部署脚本**：一键安装 Caddy 并配置自动 SSL 证书，针对 `/live/*` 实时流路径自动优化 `flush_interval -1` 实现零延迟播放。
   - **GitHub Actions 全平台 Release**：提供 Windows、macOS (Universal)、Linux GUI 安装包以及多架构 Server 静态二进制文件。

---

## 🛠️ 技术栈

- **前端核心**：React 19 + Vite (JavaScript)
- **样式系统**：Tailwind CSS v4
- **图标集**：Lucide React
- **跨平台壳**：Tauri v2 (Rust)
- **后端框架**：Axum + Tokio (Rust)

---

## 📦 快速开始与开发指南

### 0. 环境准备
确保系统已安装 [FFmpeg](https://www.ffmpeg.org/download.html)（后端录制与实时转码播放依赖 FFmpeg）。

### 1. 前端开发与构建
```bash
# 安装依赖
npm install

# 启动本地开发服务器 (http://localhost:5173)
npm run dev

# 构建生产资源包 (输出到 dist/)
npm run build
```

### 2. 桌面客户端 (Tauri GUI)
```bash
# 开发模式
npm run tauri dev

# 编译打包桌面安装包 (.msi / .dmg / .deb / .AppImage)
npm run tauri build
```

---

## 🐧 Linux 服务器部署 (Server 模式)

### 1. 快捷脚本一键安装 (推荐)
进入 `src-tauri` 目录执行部署脚本，脚本会自动检测 CPU 架构 (x86_64 / ARM64 / ARMv7) 并优先拉取预编译二进制，自动配置 Systemd 服务：

```bash
cd src-tauri
chmod +x install_server.sh
sudo ./install_server.sh
```

**命令行快捷指令**：
- 安装服务：`sudo ./install_server.sh install`
- 更新服务：`sudo ./install_server.sh update`
- 卸载服务：`sudo ./install_server.sh uninstall`

**Systemd 服务管理**：
```bash
sudo systemctl start livedownloader    # 启动
sudo systemctl status livedownloader   # 状态
sudo systemctl enable livedownloader   # 开机自启
```

### 2. 配置 Caddy 自动 HTTPS 反向代理
在服务器上直接运行根目录下的脚本：
```bash
chmod +x setup_caddy_https.sh
sudo ./setup_caddy_https.sh live.yourdomain.com 10730
```

### 3. API Token 安全管理
- **命令行查看/修改 Token**：
  ```bash
  livedownloader token your_secret_token_123   # 设置 Token
  livedownloader token                         # 查看 Token
  livedownloader token clear                   # 清除 Token
  ```
- **服务启动时带 Token**：
  ```bash
  livedownloader --server --port 10730 --token your_secret_token_123
  ```

---

## 📱 安卓本地部署指南 (APK 前端 + Termux 后端)

若需要在安卓手机上独立运行，可使用 **“Capacitor 打包 APK + Termux 运行后端”** 方案：

### 1. 打包 APK
```bash
npm install @capacitor/core @capacitor/cli @capacitor/android
npx cap init LiveDownloader com.livedownloader.app --web-dir=dist
npx cap add android
npm run build && npx cap sync && npx cap open android
```

### 2. Termux 运行后端
在 [Termux](https://f-droid.org/zh_CN/packages/com.termux/) 中安装 Rust 与 FFmpeg 后运行：
```bash
pkg update && pkg install ffmpeg rust git -y
git clone https://github.com/mega-mage/LiveDownloader.git
cd LiveDownloader/src-tauri
cargo build --release --no-default-features --features server
./target/release/LiveDownloader --server --port 10730
```

---

## 📁 目录结构

```text
├── .github/workflows/   # GitHub Actions 多架构 Release CI/CD 工作流
├── setup_caddy_https.sh # Caddy HTTPS 自动部署与流优化脚本
├── src/                 # React 前端组件与 API 服务层
├── src-tauri/           # Rust 后端引擎、平台解析与 install_server.sh 部署脚本
├── dist/                # 前端打包产物
├── package.json         # 前端依赖与脚本
└── vite.config.js       # Vite 打包配置
```

---

## 🤝 贡献与反馈

欢迎提交 Issue 或 Pull Request！如需新增平台解析或优化 UI，请保持现有的响应式设计与多主题兼容。
