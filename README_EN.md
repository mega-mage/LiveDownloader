# LiveDownloader

LiveDownloader is a modern, visually stunning automated live stream recording and scheduling management system designed for major streaming platforms. It runs seamlessly as a standalone **web application** (communicating with remote servers or NAS backend APIs) or a native **desktop client** (powered by Tauri v2 + Rust).

---

## 🚀 Core Features & Highlights

1. **Real-time Dashboard**:
   - **Live Monitoring**: Track room status (Living, Offline, Paused) in responsive tables or grid cards.
   - **Quick Actions**: Toggle stream recording status, pause/resume monitoring, delete rooms, view configs, or play active streams.
   - **Supported Platforms**: Bilibili, Douyin, Huya, Kuaishou, Douyu, Missevan, NetEase CC, Weibo, Taobao, AcFun, and Twitch.
2. **Real-time Live Stream Player (FFmpeg HLS Remux)**:
   - Innovative backend **/live** proxy endpoint that dynamically remuxes FLV/HLS/RTMP streams into standard HLS segments via FFmpeg.
   - Front-end HLS.js player integration that allows live watching **without interfering** with background recording.
   - Automatic session manager that cleanly terminates FFmpeg processes and temp files after 30s of inactivity.
3. **Recorded Video Management**:
   - Fuzzy double filtering on file names and anchor aliases, plus dedicated "Anchor Filter Dropdowns".
   - Signed temporary download links, inline web playback, and physical file deletion.
4. **System Settings**:
   - Connection Auth: Configure remote API server URLs and authentication tokens (supports CLI and API Token).
   - Basic Parameters: Default video save paths, formats (TS/MP4/MKV/FLV/MP3/M4A), segment splitting, and poll cycles.
   - Network & Notifications: Global network proxies, DingTalk webhooks, Bark iOS notifications, Telegram status updates, and auto-upload of recording segments (up to 2GB).
   - Credentials (Cookies): Store platform cookies to unlock premium, high-definition streams (Source/Blu-ray 1080p).
5. **Multi-Arch & Automated DevOps (CI/CD & Shell Automation)**:
   - **One-Click Server Install Script**: Supports Linux (x86_64, ARM64, ARMv7), auto-detects CPU architecture, and supports pre-compiled zero-build installs (fetches binaries directly from GitHub Releases).
   - **Caddy HTTPS Setup Script**: Installs Caddy, configures Let's Encrypt / ZeroSSL, and automatically applies `flush_interval -1` optimization for `/live/*` paths for zero-latency playback.
   - **GitHub Actions Multi-Arch Release**: Builds Windows, macOS (Universal), Linux GUI packages, and multi-arch Server binaries.

---

## 🛠️ Technology Stack

- **Core**: React 19 + Vite (JavaScript)
- **Styling**: Tailwind CSS v4
- **Icons**: Lucide React
- **App Shell**: Tauri v2 (Rust)
- **Backend Framework**: Axum + Tokio (Rust)

---

## 📦 Quick Start & Development

### 0. Prerequisites
Make sure [FFmpeg](https://www.ffmpeg.org/download.html) is installed on your system (required for backend recording and live stream remuxing).

### 1. Install & Build Frontend
```bash
# Install dependencies
npm install

# Launch development server (http://localhost:5173)
npm run dev

# Build production bundle (outputs to dist/)
npm run build
```

### 2. Desktop GUI (Tauri App)
```bash
# Run in dev mode
npm run tauri dev

# Build desktop packages (.msi / .dmg / .deb / .AppImage)
npm run tauri build
```

### 3. Web API Server Mode (Backend Server)
```bash
# Enter src-tauri directory and start local Web REST API server (default port 10730)
cd src-tauri
cargo run --no-default-features --features server -- --server --port 10730
```

---

## 🐧 Linux Server Deployment (Server Mode)

### 1. One-Click Installation Script (Recommended)
Run the setup script inside `src-tauri`. It will auto-detect your server CPU architecture (x86_64 / ARM64 / ARMv7), download the matching pre-compiled binary, and configure Systemd:

```bash
cd src-tauri
chmod +x install_server.sh
sudo ./install_server.sh
```

**Script Quick Commands**:
- Install service: `sudo ./install_server.sh install`
- Update service: `sudo ./install_server.sh update`
- Uninstall service: `sudo ./install_server.sh uninstall`

**Systemd Service Management**:
```bash
sudo systemctl start livedownloader    # Start
sudo systemctl status livedownloader   # Status
sudo systemctl enable livedownloader   # Auto-start on boot
```

### 2. Configure & Uninstall Caddy Automatic HTTPS
Run the reverse proxy scripts in the root directory:

- **Setup & Configure Caddy HTTPS**:
  ```bash
  chmod +x setup_caddy_https.sh
  sudo ./setup_caddy_https.sh live.yourdomain.com 10730
  ```
- **Uninstall & Clean Caddy HTTPS Configuration**:
  ```bash
  chmod +x uninstall_caddy_https.sh
  sudo ./uninstall_caddy_https.sh
  ```

### 3. API Token Security
- **CLI Management**:
  ```bash
  livedownloader token your_secret_token_123   # Set Token
  livedownloader token                         # View Token
  livedownloader token clear                   # Clear Token
  ```
- **Server Start Flag**:
  ```bash
  livedownloader --server --port 10730 --token your_secret_token_123
  ```

---

## 📱 Android Local Deployment (APK Frontend + Termux Backend)

To run LiveDownloader standalone on an Android phone, use the **Capacitor APK + Termux Backend** approach:

### 1. Build APK
```bash
npm install @capacitor/core @capacitor/cli @capacitor/android
npx cap init LiveDownloader com.livedownloader.app --web-dir=dist
npx cap add android
npm run build && npx cap sync && npx cap open android
```

### 2. Run Backend in Termux
Install Rust and FFmpeg inside [Termux](https://f-droid.org/zh_CN/packages/com.termux/):
```bash
pkg update && pkg install ffmpeg rust git -y
git clone https://github.com/mega-mage/LiveDownloader.git
cd LiveDownloader/src-tauri
cargo build --release --no-default-features --features server
./target/release/LiveDownloader --server --port 10730
```

---

## 📁 Directory Structure

```text
├── .github/workflows/      # GitHub Actions multi-arch Release CI/CD workflow
├── setup_caddy_https.sh    # Caddy HTTPS deployment & stream optimization script
├── uninstall_caddy_https.sh# Caddy HTTPS reverse proxy cleanup & uninstall script
├── src/                    # React frontend UI components & API service bridge
├── src-tauri/              # Rust backend core, platform parsers & install_server.sh script
├── dist/                   # Production frontend bundle
├── package.json            # Frontend dependencies & scripts
└── vite.config.js          # Vite configuration
```

---

## 🤝 Contribution

Contributions, pull requests, and feature suggestions are welcome! Please ensure responsive design and theme compatibility are maintained.
