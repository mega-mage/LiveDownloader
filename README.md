
# LiveDownloader

LiveDownloader 是一款专为各大直播平台设计的高颜值、现代化自动录制与调度管理系统。支持以独立的**网页浏览器模式**（与远程服务器或 NAS 上的 API 进行通信）以及**桌面客户端模式**（结合 Rust 编写的 Tauri 底层）无缝运行。


---

## 🚀 核心功能模块

1. **实时工作台 (Dashboard)**：
   - 动态监控列表：卡片与表格展示所有主播的开播状态（正在直播 / 离线 / 暂停监控）。
   - 快捷控制：支持一键修改单个主播画质格式、暂停/启用监控、跳转实时观战，或直接删除。
   - 快速添加监控：支持解析 哔哩哔哩、抖音、虎牙、快手、斗鱼、猫耳FM、网易CC、微博、淘宝、AcFun 及 Twitch 等直播地址。
2. **已录视频切片 (Recorded Videos)**：
   - 支持根据主播名称、文件名称进行模糊双重过滤。
   - 独创“主播专属过滤”下拉列表，快速定位所选主播的视频归档。
   - 內置独立通道拉流播放器，观战不干扰后台录制进程。
3. **系统参数设置 (System Settings)**：
   - 通信凭证：配置网页端与 NAS/远程服务器后端的 API 认证地址。
   - 基础设置：视频默认保存路径（ downloads）、录制封装格式、循环检测开播间隔。
   - 境外代理：一键开启代理，确保 Twitch 等境外平台流畅解析与下载。
   - 消息推送：支持钉钉机器人推送、Bark iOS 服务、Telegram 消息警告及录制切片自动上传（支持最大 2GB）。
   - 登录凭证管理 (Cookies)：对哔哩哔哩、抖音等平台导入授权 Cookies，以解锁超清或原画高码率录制画质。
4. **追踪日志与控制台 (Logs & CLI Console)**：
   - 实时日志输出查看。
   - 搭载命令行控制端命令执行系统（ld cli），直接在前台键入后台核心指令（如 `ls`, `add`, `push test` 等）并回显终端反馈。

---

## 🛠️ 技术栈

- **前端核心**：React + Vite (JavaScript)
- **样式系统**：Tailwind CSS v4 (基于 `@import "tailwindcss"` 及 `@theme` 变量系统配置)
- **图标集**：Lucide React
- **跨平台壳**：Tauri (Rust)

---

## 📦 快速开始与开发指南

### 0. ffmpeg
确保你安装 [ffmpeg](https://www.ffmpeg.org/download.html) 服务

### 1. 克隆与依赖安装
确保你已安装 [Node.js](https://nodejs.org/) 环境，在项目根目录下执行：
```bash
npm install
```

### 2. 启动本地开发服务器
运行 Vite 调试服务器：
```bash
npm run dev
```
启动后可在浏览器中访问 `http://localhost:5173`。

### 3. 构建生产资源包
编译并打包前端静态资源：
```bash
npm run build
```
打包产物将输出在 `dist/` 文件夹中。

### 4. 运行 Tauri 桌面端 (需要配置 Rust 开发环境)
如果你需要编译或运行桌面客户端：
```bash
# 开发模式
npm run tauri dev

# 编译打包桌面安装包
npm run tauri build
```

## 📱 安卓本地部署指南 (APK 前端 + Termux 后端)

如果你想在安卓手机上实现完全独立的本地直播录制，可以采用 **“前端打包成 APK 安装包 + 后端利用 Termux 在本地后台运行”** 的闭环方案。

### 1. 前端打包为安卓 App
1. 在项目根目录下，利用 **Capacitor** 初始化安卓环境：
   ```bash
   npm install @capacitor/core @capacitor/cli
   npx cap init LiveDownloader com.livedownloader.app --web-dir=dist
   npm install @capacitor/android
   npx cap add android
   ```
2. 每次前端资源构建完成后，同步代码并使用 Android Studio 生成 APK 安装包：
   ```bash
   npm run build
   npx cap sync
   npx cap open android
   # 在 Android Studio 中编译生成并安装 Release APK
   ```
3. 在安装好的 App 设置中，将远程 API 地址设定为本地回环地址 `http://127.0.0.1:10730`。

### 2. 手机本地 Termux 部署后端
1. 在手机上安装最新版 [Termux (F-Droid 发行版)](https://f-droid.org/zh_CN/packages/com.termux/)。
2. 打开 Termux 终端，更新源并安装依赖库：
   ```bash
   pkg update && pkg upgrade -y
   pkg install ffmpeg rust git -y
   ```
3. 拉取后端代码并直接在手机本地进行编译运行：
   ```bash
   git clone <后端项目 Git 仓库地址>
   cd <后端目录>
   cargo build --release
   ./target/release/livedownloader-backend --host 127.0.0.1 --port 10730
   ```

### 💡 关键避坑与调优
- **外部存储映射**：为了让录制出的视频保存在手机的常规相册/电影文件夹中，必须在 Termux 内运行 `termux-setup-storage` 获取存储权限，并将后端配置中的视频默认保存路径设为 `/storage/emulated/0/Movies/LiveDownloader`。
- **防止后台冻结**：
  1. 下拉 Termux 状态栏通知，点击 **Acquire wakelock** 锁定 CPU 不休眠。
  2. 在手机系统设置中，将 Termux 的省电策略设为 **“无限制 / 允许后台高耗电”**。

---

## 📁 目录结构

```text
├── src/
│   ├── components/       # UI 视图组件目录
│   │   ├── ui/           # 通用基础基础组件 (Card, Button, Table, Input 等)
│   │   ├── Sidebar.jsx   # 响应式侧边导航栏 (含移动端抽屉)
│   │   ├── RoomSection.jsx    # 工作台监控列表及添加表单
│   │   ├── VideoSection.jsx   # 视频管理、专属过滤与网页放映
│   │   ├── SettingsSection.jsx# 系统基础、网络、消息、Cookies 配置 (粘性按钮)
│   │   ├── LogViewer.jsx # 追踪日志与 CLI 控制终端
│   │   ├── ThemeSelector.jsx  # 预设主题、Shuffle 随机、自定义保存皮肤栏
│   │   └── ModalOverlays.jsx  # 播放器浮层、Cookie 导入与主播设置弹窗
│   ├── lib/
│   │   ├── i18n.js       # 多语言字典文件及 `t()` 解析工具
│   │   └── utils.js      # clsx 与 tailwind-merge 通用配置
│   ├── services/
│   │   └── api.js        # 后端接口对接及连接判定
│   ├── App.jsx           # 主控制台视图，管理全局状态与主题应用
│   ├── index.css         # 全局样式，包含各预设主题 CSS 变量定义
│   └── main.jsx          # 入口渲染文件
├── index.html            # 主页模板
├── vite.config.js        # Vite 打包插件及别名配置
├── package.json          # 依赖及指令脚本
└── components.json       # shadcn/ui 配置文件
```

---

## 🤝 贡献与反馈

欢迎提交 PR 或 Issue 来改善 LiveDownloader UI 的设计与功能！
在修改代码时，请保持现有组件的适配性与多主题的配色兼容。
