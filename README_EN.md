# LiveDownloader

LiveDownloader is a modern, visually stunning automated live stream recording   system designed for major streaming platforms. This repository contains the front-end UI project, which can run seamlessly as a standalone **web application** (communicating with remote servers or NAS backend APIs) or a native **desktop client** (powered by Tauri and Rust).

---

## 🚀 Core Modules

1. **Real-time Dashboard**:
   - Monitored Anchors List: Track room states (Living, Idle/Offline, Paused) in responsive tables or grid cards.
   - Quick Actions: Toggle stream recording status, pause/resume monitoring, delete rooms, view configs, or play active streams.
   - Fast Room Adding: Supports URL parsing for Bilibili, Douyin, Huya, Kuaishou, Douyu, Missevan, NetEase CC, Weibo, Taobao, AcFun, and Twitch.
2. **Recorded Videos**:
   - Fuzzy double filtering on file names and anchor aliases.
   - Unique "Anchor Filter Dropdown" to locate all clips belonging to a specific anchor immediately.
   - Built-in stream player (runs on a separate connection without affecting background recording).
3. **System Settings**:
   - Connection Config: Adjust remote API server base URLs and auth tokens.
   - Basic Parameters: Set default save paths (`./downloads`), media extensions, and poll cycles.
   - Global Proxies: Network rules for global platforms (e.g., Twitch stream crawling).
   - Instant Pushes: Supports DingTalk webhooks, Bark iOS notifications, Telegram status updates, and auto-upload of recording segments (up to 2GB).
   - Credentials Management (Cookies): Store platform cookies to unlock premium, high-definition streams (Source quality/1080p).
4. **Logs & Interactive CLI Console**:
   - Real-time backend system trace logging.
   - Integrated shell executor (ld CLI) allowing execution of core commands (e.g., `ls`, `add`, `push test`) with terminal response feedbacks.

---

## 🛠️ Technology Stack

- **Core**: React + Vite (JavaScript)
- **Styling**: Tailwind CSS v4 (configured via `@import "tailwindcss"` and `@theme` parameters)
- **Icons**: Lucide React
- **App Shell**: Tauri (Rust wrapper for desktop integration)

---

## 📦 Quick Start & Development

### 0. ffmpeg
Make sure you have [ffmpeg](https://www.ffmpeg.org/download.html) installed

### 1. Install Dependencies
Make sure you have [Node.js](https://nodejs.org/) installed, then run in project root directory:
```bash
npm install
```

### 2. Run Local Development Server
Launch Vite preview server:
```bash
npm run dev
```
Open your browser and navigate to `http://localhost:5173`.

### 3. Build Production Bundles
Compile and compress front-end static files:
```bash
npm run build
```
Production assets will be compiled to the `dist/` directory.

### 4. Run Tauri Desktop App (Rust toolchain required)
If you wish to compile or execute the desktop client:
```bash
# Run in dev mode
npm run tauri dev

# Package for desktop installation
npm run tauri build
```

## 📱 Android Local Deployment Guide (APK Frontend + Termux Backend)

If you want to achieve completely independent local stream recording on your Android device, you can use the **"Frontend packaged as standalone APK + Backend running locally inside Termux"** solution.

### 1. Build and Package Frontend as APK
1. Under the project root, initialize Capacitor to set up Android builds:
   ```bash
   npm install @capacitor/core @capacitor/cli
   npx cap init LiveDownloader com.livedownloader.app --web-dir=dist
   npm install @capacitor/android
   npx cap add android
   ```
2. Whenever frontend codes update, sync files and build APK with Android Studio:
   ```bash
   npm run build
   npx cap sync
   npx cap open android
   # Build, package and install Release APK from Android Studio
   ```
3. In the settings page of the installed Android App, configure the Remote API Address to point to local loopback: `http://127.0.0.1:10730`.

### 2. Setup Local Backend inside Termux
1. Install the latest version of [Termux (F-Droid Build)](https://f-droid.org/zh_CN/packages/com.termux/).
2. Start Termux shell, update repositories and install dependencies:
   ```bash
   pkg update && pkg upgrade -y
   pkg install ffmpeg rust git -y
   ```
3. Clone and compile the Rust backend executable directly on your phone:
   ```bash
   git clone <Backend-Git-Repository-URL>
   cd <backend-folder>
   cargo build --release
   ./target/release/livedownloader-backend --host 127.0.0.1 --port 10730
   ```

### 💡 Key Tuning & Optimizations
- **Shared Storage Mapping**: To ensure recorded videos can be accessed by default Android Movies galleries, run `termux-setup-storage` inside Termux to request storage permissions, then set your default video directory path in configurations to `/storage/emulated/0/Movies/LiveDownloader`.
- **Prevent App Background Killing**:
  1. Pull down the notifications menu and click **Acquire wakelock** inside the Termux notification banner to keep the CPU awake.
  2. Under system Settings > App Manager > Termux > Battery Management, check **"Unrestricted / Run in Background"**.

---

## 📁 Directory Structures

```text
├── src/
│   ├── components/       # UI components directory
│   │   ├── ui/           # Basic UI components (Card, Button, Table, Input, etc.)
│   │   ├── Sidebar.jsx   # Responsive side navigation drawer
│   │   ├── RoomSection.jsx    # Dashboard monitoring tables & form adding
│   │   ├── VideoSection.jsx   # Recorded videos list, filters, and stream plays
│   │   ├── SettingsSection.jsx# System setting fields (with sticky save footer)
│   │   ├── LogViewer.jsx # Run log output window & terminal CLI console
│   │   ├── ThemeSelector.jsx  # Presets, theme shuffle, and custom save slot
│   │   └── ModalOverlays.jsx  # Watch streams layer, raw cookie config modal
│   ├── lib/
│   │   ├── i18n.js       # Dynamic translation keys & translate function t()
│   │   └── utils.js      # Utility class consolidations (clsx & tailwind-merge)
│   ├── services/
│   │   └── api.js        # Backend API service bridges
│   ├── App.jsx           # Main controller, handles global states & theme systems
│   ├── index.css         # Styling entryway, defines custom theme variables
│   └── main.jsx          # Entry renderer
├── index.html            # Main HTML layout template
├── vite.config.js        # Vite configurations (alias mapping & plugin imports)
├── package.json          # Dependencies & execution scripts
└── components.json       # shadcn/ui components configuration file
```

---

## 🤝 Contribution

Contributions, pull requests, and feature suggestions are welcome!
When proposing code modifications, please verify that responsiveness and multi-theme design adaptations are well-preserved.
