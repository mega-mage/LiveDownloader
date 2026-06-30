# LiveDownloader (自动录制调度系统)

LiveDownloader 是一款专为各大直播平台设计的高颜值、现代化自动录制与调度管理系统。本仓库为该项目的前端 UI 仓，支持以独立的**网页浏览器模式**（与远程服务器或 NAS 上的 API 进行通信）以及**桌面客户端模式**（结合 Rust 编写的 Tauri 底层）无缝运行。

---

## 🎨 界面美学与特色

- **玻璃拟态（Glassmorphism）与微交互**：全面采用现代化设计的半透明磨砂玻璃卡片布局、渐变光效阴影以及流畅的动画过渡。
- **中英文国际化 (i18n)**：全站词条完美适配中文与英文，并在顶部/侧边导航栏提供一键切换，偏好语种可持久化保存于本地。
- **响应式跨端适配**：完美适配大屏电脑、平板以及手机竖屏视口，在窄屏下自动切换为侧滑菜单抽屉与卡片瀑布流布局，右上角控制栏防重叠防遮挡。
- **自定义随机主题 (Theme Shuffle)**：
  - 拥有 5 种精美预设主题：暗黑极客 (Dark)、赛博朋克 (Cyberpunk)、粉嫩樱花 (Sakura)、极简石蓝 (Light)、森林之息 (Forest)。
  - 拥有类似 `shadcn/ui` 的随机主题生成功能：支持从**基准明暗模式、主次色相色调、圆角弧度、Google 字体库、玻璃背景透光度**等多个独立维度进行多维无缝抽取。
  - 支持**第 6 个自定义主题保存插槽**，可一键保存并持久化复原你所随机到的完美风格。

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
