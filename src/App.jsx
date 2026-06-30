import { useState, useEffect, useRef } from "react";
import {
  getRooms, addRoom, deleteRoom, getConfig, saveConfig, getLogs,
  getProxyPort, toggleEngineStatus, getEngineStatus, getRecordedVideos,
  openRecordedFolder, toggleRoomPaused, saveCookie, updateRoomConfig,
  executeLdCommand, getStreamProxyUrl, isWebMode,
  getApiBaseUrl, setApiBaseUrl, getApiToken, setApiToken, getDownloadLink, deleteVideoFile
} from "./services/api.js";

import { Sidebar } from "./components/Sidebar";
import { RoomSection } from "./components/RoomSection";
import { VideoSection } from "./components/VideoSection";
import { SettingsSection } from "./components/SettingsSection";
import { LogViewer } from "./components/LogViewer";
import { ModalOverlays } from "./components/ModalOverlays";
import { Button } from "./components/ui/button";
import { cn } from "./lib/utils";
import { t } from "./lib/i18n.js";


// Conditionally import Tauri APIs only when running in Tauri desktop mode
const isTauri = () => !!window.__TAURI_INTERNALS__;
let getCurrentWebviewWindow, PhysicalPosition;
if (isTauri()) {
  import("@tauri-apps/api/webviewWindow").then(m => { getCurrentWebviewWindow = m.getCurrentWebviewWindow; });
  import("@tauri-apps/api/dpi").then(m => { PhysicalPosition = m.PhysicalPosition; });
}

import {
  Minus,
  Square,
  X,
  GripHorizontal,
  PlayCircle,
  PauseCircle
} from "lucide-react";

import "./App.css";

function App() {
  // Titlebar drag logic for custom frameless windows
  const dragRef = useRef({
    isDragging: false,
    startX: 0,
    startY: 0,
    startWinX: 0,
    startWinY: 0,
    scaleFactor: 1,
    rafId: null,
    currentX: 0,
    currentY: 0,
  });

  const handleMinimize = () => {
    if (getCurrentWebviewWindow) getCurrentWebviewWindow().minimize();
  };

  const handleMaximize = async () => {
    if (!getCurrentWebviewWindow) return;
    const win = getCurrentWebviewWindow();
    if (await win.isMaximized()) {
      win.unmaximize();
    } else {
      win.maximize();
    }
  };

  const handleClose = () => {
    if (getCurrentWebviewWindow) getCurrentWebviewWindow().close();
  };

  const handleDragStart = async (e) => {
    if (e.button !== 0 || !getCurrentWebviewWindow) return;
    e.preventDefault();

    const win = getCurrentWebviewWindow();
    const pos = await win.outerPosition();
    const sf = await win.scaleFactor();

    dragRef.current = {
      isDragging: true,
      startX: e.screenX,
      startY: e.screenY,
      startWinX: pos.x,
      startWinY: pos.y,
      scaleFactor: sf,
      currentX: pos.x,
      currentY: pos.y,
      rafId: null,
    };

    document.addEventListener("mousemove", handleDragMove);
    document.addEventListener("mouseup", handleDragEnd);
  };

  const handleDragMove = (e) => {
    if (!dragRef.current.isDragging) return;

    const deltaX = e.screenX - dragRef.current.startX;
    const deltaY = e.screenY - dragRef.current.startY;

    dragRef.current.currentX = dragRef.current.startWinX + Math.round(deltaX * dragRef.current.scaleFactor);
    dragRef.current.currentY = dragRef.current.startWinY + Math.round(deltaY * dragRef.current.scaleFactor);

    if (!dragRef.current.rafId) {
      dragRef.current.rafId = requestAnimationFrame(async () => {
        if (!dragRef.current.isDragging) {
          dragRef.current.rafId = null;
          return;
        }
        try {
          if (getCurrentWebviewWindow && PhysicalPosition) {
            const win = getCurrentWebviewWindow();
            await win.setPosition(new PhysicalPosition(dragRef.current.currentX, dragRef.current.currentY));
          }
        } catch (err) {
          console.error(`ERROR in setPosition: ${err.message || err}`);
        }
        dragRef.current.rafId = null;
      });
    }
  };

  const handleDragEnd = () => {
    dragRef.current.isDragging = false;
    if (dragRef.current.rafId) {
      cancelAnimationFrame(dragRef.current.rafId);
      dragRef.current.rafId = null;
    }
    document.removeEventListener("mousemove", handleDragMove);
    document.removeEventListener("mouseup", handleDragEnd);
  };

  useEffect(() => {
    return () => {
      document.removeEventListener("mousemove", handleDragMove);
      document.removeEventListener("mouseup", handleDragEnd);
    };
  }, []);

  // Theme Skin state loading
  const [theme, setTheme] = useState(() => localStorage.getItem("theme") || "dark");
  const [themeVersion, setThemeVersion] = useState(0);

  // Apply custom theme vars to document root element
  const applyCustomThemeVars = (vars) => {
    const el = document.documentElement;
    el.className = "";
    el.style.cssText = "";
    Object.entries(vars).forEach(([key, value]) => {
      if (key === "font") {
        el.style.fontFamily = value;
      } else if (key.startsWith("--")) {
        el.style.setProperty(key, value);
      }
    });
  };

  useEffect(() => {
    const el = document.documentElement;
    if (theme === "custom") {
      const customVars = JSON.parse(localStorage.getItem("customThemeVars") || "null");
      if (customVars) {
        applyCustomThemeVars(customVars);
      }
    } else if (theme === "savedCustom") {
      const savedVars = JSON.parse(localStorage.getItem("savedCustomThemeVars") || "null");
      if (savedVars) {
        applyCustomThemeVars(savedVars);
      }
    } else {
      el.style.cssText = "";
      const appliedClass = theme === "dark" ? "" : `theme-${theme}`;
      el.className = appliedClass;
    }
    localStorage.setItem("theme", theme);
  }, [theme, themeVersion]);

  // Multi-dimensional random theme generator
  const handleShuffleTheme = () => {
    const isDark = Math.random() > 0.35;
    const primaryHue = Math.floor(Math.random() * 360);
    const accentHue = (primaryHue + 90 + Math.floor(Math.random() * 180)) % 360;

    const radii = ["0px", "0.25rem", "0.375rem", "0.5rem", "0.75rem", "1rem"];
    const radius = radii[Math.floor(Math.random() * radii.length)];

    const fonts = [
      "'Inter', system-ui, sans-serif",
      "'Outfit', system-ui, sans-serif",
      "'DM Sans', system-ui, sans-serif",
      "'Space Grotesk', system-ui, sans-serif",
      "'Nunito', system-ui, sans-serif",
      "'Poppins', system-ui, sans-serif",
    ];
    const font = fonts[Math.floor(Math.random() * fonts.length)];

    let vars;
    if (isDark) {
      const bgS = 8 + Math.floor(Math.random() * 15);
      const bgL = 3 + Math.floor(Math.random() * 6);
      const fgL = 92 + Math.floor(Math.random() * 6);
      const pS = 65 + Math.floor(Math.random() * 30);
      const pL = 50 + Math.floor(Math.random() * 15);
      const aS = 60 + Math.floor(Math.random() * 35);
      const aL = 55 + Math.floor(Math.random() * 15);
      const surfaceAlpha = (0.5 + Math.random() * 0.4).toFixed(2);
      const borderAlpha = (0.06 + Math.random() * 0.12).toFixed(2);

      vars = {
        "--background": `hsl(${primaryHue}, ${bgS}%, ${bgL}%)`,
        "--foreground": `hsl(${primaryHue}, 5%, ${fgL}%)`,
        "--card": `hsla(${primaryHue}, ${bgS + 3}%, ${bgL + 5 + Math.floor(Math.random() * 5)}%, ${surfaceAlpha})`,
        "--card-foreground": `hsl(${primaryHue}, 5%, ${fgL}%)`,
        "--popover": `hsl(${primaryHue}, ${bgS}%, ${bgL + 2}%)`,
        "--popover-foreground": `hsl(${primaryHue}, 5%, ${fgL}%)`,
        "--primary": `hsl(${primaryHue}, ${pS}%, ${pL}%)`,
        "--primary-foreground": "#ffffff",
        "--secondary": `hsl(${primaryHue}, ${10 + Math.floor(Math.random() * 10)}%, ${12 + Math.floor(Math.random() * 8)}%)`,
        "--secondary-foreground": `hsl(${primaryHue}, 5%, ${fgL}%)`,
        "--muted": `hsl(${primaryHue}, ${10 + Math.floor(Math.random() * 10)}%, ${12 + Math.floor(Math.random() * 8)}%)`,
        "--muted-foreground": `hsl(${primaryHue}, ${15 + Math.floor(Math.random() * 20)}%, ${55 + Math.floor(Math.random() * 15)}%)`,
        "--accent": `hsl(${accentHue}, ${aS}%, ${aL}%)`,
        "--accent-foreground": "#ffffff",
        "--destructive": "#ef4444",
        "--destructive-foreground": "#ffffff",
        "--border": `hsla(${primaryHue}, 15%, 100%, ${borderAlpha})`,
        "--input": `hsla(${primaryHue}, 15%, 100%, ${(parseFloat(borderAlpha) + 0.04).toFixed(2)})`,
        "--ring": `hsl(${primaryHue}, ${pS}%, ${pL}%)`,
        "--radius": radius,
        "--bg-gradient": `linear-gradient(135deg, hsl(${primaryHue}, 10%, ${Math.max(bgL - 2, 1)}%) 0%, hsl(${primaryHue}, 15%, ${bgL + 5}%) 100%)`,
        "--accent-gradient": `linear-gradient(90deg, hsl(${primaryHue}, ${pS}%, ${pL}%) 0%, hsl(${accentHue}, ${aS}%, ${aL}%) 100%)`,
        "--accent-glow": `hsla(${accentHue}, ${aS}%, ${aL}%, 0.25)`,
        "font": font
      };
    } else {
      const bgS = 8 + Math.floor(Math.random() * 20);
      const bgL = 96 + Math.floor(Math.random() * 3);
      const fgS = 15 + Math.floor(Math.random() * 30);
      const fgL = 8 + Math.floor(Math.random() * 10);
      const pS = 55 + Math.floor(Math.random() * 40);
      const pL = 40 + Math.floor(Math.random() * 15);
      const aS = 55 + Math.floor(Math.random() * 40);
      const aL = 45 + Math.floor(Math.random() * 15);

      vars = {
        "--background": `hsl(${primaryHue}, ${bgS}%, ${bgL}%)`,
        "--foreground": `hsl(${primaryHue}, ${fgS}%, ${fgL}%)`,
        "--card": `hsla(0, 0%, 100%, ${(0.85 + Math.random() * 0.15).toFixed(2)})`,
        "--card-foreground": `hsl(${primaryHue}, ${fgS}%, ${fgL}%)`,
        "--popover": "#ffffff",
        "--popover-foreground": `hsl(${primaryHue}, ${fgS}%, ${fgL}%)`,
        "--primary": `hsl(${primaryHue}, ${pS}%, ${pL}%)`,
        "--primary-foreground": "#ffffff",
        "--secondary": `hsl(${primaryHue}, ${10 + Math.floor(Math.random() * 20)}%, ${92 + Math.floor(Math.random() * 4)}%)`,
        "--secondary-foreground": `hsl(${primaryHue}, ${fgS}%, ${fgL}%)`,
        "--muted": `hsl(${primaryHue}, ${10 + Math.floor(Math.random() * 15)}%, ${92 + Math.floor(Math.random() * 4)}%)`,
        "--muted-foreground": `hsl(${primaryHue}, ${10 + Math.floor(Math.random() * 15)}%, ${35 + Math.floor(Math.random() * 15)}%)`,
        "--accent": `hsl(${accentHue}, ${aS}%, ${aL}%)`,
        "--accent-foreground": "#ffffff",
        "--destructive": "#ef4444",
        "--destructive-foreground": "#ffffff",
        "--border": `hsl(${primaryHue}, ${10 + Math.floor(Math.random() * 15)}%, ${85 + Math.floor(Math.random() * 8)}%)`,
        "--input": `hsl(${primaryHue}, ${10 + Math.floor(Math.random() * 15)}%, ${78 + Math.floor(Math.random() * 10)}%)`,
        "--ring": `hsl(${primaryHue}, ${pS}%, ${pL}%)`,
        "--radius": radius,
        "--bg-gradient": `linear-gradient(135deg, hsl(${primaryHue}, 12%, ${bgL - 2}%) 0%, hsl(${primaryHue}, 15%, ${bgL - 8}%) 100%)`,
        "--accent-gradient": `linear-gradient(90deg, hsl(${primaryHue}, ${pS}%, ${pL}%) 0%, hsl(${accentHue}, ${aS}%, ${aL}%) 100%)`,
        "--accent-glow": `hsla(${accentHue}, ${aS}%, ${aL}%, 0.15)`,
        "font": font
      };
    }

    localStorage.setItem("customThemeVars", JSON.stringify(vars));
    applyCustomThemeVars(vars);
    setTheme("custom");
    setThemeVersion(v => v + 1);
    localStorage.setItem("theme", "custom");
  };

  // Saved custom theme state & handler
  const [hasSavedCustom, setHasSavedCustom] = useState(() => !!localStorage.getItem("savedCustomThemeVars"));

  const handleSaveCustomTheme = () => {
    const vars = localStorage.getItem("customThemeVars");
    if (vars) {
      localStorage.setItem("savedCustomThemeVars", vars);
      setHasSavedCustom(true);
      setTheme("savedCustom");
      setThemeVersion(v => v + 1);
      localStorage.setItem("theme", "savedCustom");
    }
  };

  // Language state loading
  const [lang, setLang] = useState(() => localStorage.getItem("lang") || "zh");

  const toggleLanguage = () => {
    const nextLang = lang === "zh" ? "en" : "zh";
    setLang(nextLang);
    localStorage.setItem("lang", nextLang);
  };

  // Unified global states
  const [activeTab, setActiveTab] = useState("dashboard");
  const [rooms, setRooms] = useState([]);
  const [logs, setLogs] = useState([]);
  const [config, setConfig] = useState(null);

  // Floating Player state
  const [activePlayUrl, setActivePlayUrl] = useState(null);
  const [activePlayTitle, setActivePlayTitle] = useState("");
  const videoRef = useRef(null);
  const hlsRef = useRef(null);

  // Form states
  const [newUrl, setNewUrl] = useState("");
  const [newAlias, setNewAlias] = useState("");
  const [newQuality, setNewQuality] = useState("原画");
  const [statusMsg, setStatusMsg] = useState({ type: "", text: "" });

  // Proxy port for stream playback
  const [proxyPort, setProxyPort] = useState(null);

  // Web mode remote connection settings
  const [remoteApiBase, setRemoteApiBase] = useState(getApiBaseUrl());
  const [remoteApiToken, setRemoteApiToken] = useState(getApiToken());
  const [showConnectionSettings, setShowConnectionSettings] = useState(false);

  // Engine Pause Status
  const [isEnginePaused, setIsEnginePaused] = useState(true);

  // Recorded Videos state
  const [recordedVideos, setRecordedVideos] = useState([]);
  const [videoSearch, setVideoSearch] = useState("");

  // Config form states
  const [savePath, setSavePath] = useState("");
  const [saveFormat, setSaveFormat] = useState("ts");
  const [qualityDefault, setQualityDefault] = useState("原画");
  const [useProxy, setUseProxy] = useState("否");
  const [proxyAddr, setProxyAddr] = useState("");
  const [pollInterval, setPollInterval] = useState("60");
  const [cookies, setCookies] = useState({});
  const [pushChannels, setPushChannels] = useState([]);
  const [dingtalkApi, setDingtalkApi] = useState("");
  const [barkApi, setBarkApi] = useState("");
  const [tgToken, setTgToken] = useState("");
  const [tgChatId, setTgChatId] = useState("");
  const [tgAutoUpload, setTgAutoUpload] = useState(false);
  const [tgApiUrl, setTgApiUrl] = useState("");
  const [cmdInput, setCmdInput] = useState("");
  const [terminalLogs, setTerminalLogs] = useState(["欢迎使用 ld 交互控制台。输入 ld 指令后按回车执行。"]);

  // Overlay Modals
  const [modal, setModal] = useState({
    show: false,
    title: "",
    message: "",
    type: "info",
    onConfirm: null,
    onCancel: null
  });

  const [cookieModal, setCookieModal] = useState({
    show: false,
    platformKey: "",
    platformName: "",
    value: ""
  });

  const [roomConfigModal, setRoomConfigModal] = useState({
    show: false,
    url: "",
    anchorName: "",
    name: "",
    quality: "",
    videoSaveType: ""
  });

  const showAlert = (title, message, type = "info") => {
    setModal({
      show: true,
      title,
      message,
      type,
      onConfirm: () => closeModal(),
      onCancel: null
    });
  };

  const showConfirm = (title, message, onConfirm, onCancel = null) => {
    setModal({
      show: true,
      title,
      message,
      type: "confirm",
      onConfirm: () => {
        onConfirm();
        closeModal();
      },
      onCancel: () => {
        if (onCancel) onCancel();
        closeModal();
      }
    });
  };

  const closeModal = () => {
    setModal(prev => ({ ...prev, show: false }));
  };

  // Run ld Command Console submit handler
  const handleRunCommand = async (e) => {
    if (e) e.preventDefault();
    const trimmed = cmdInput.trim();
    if (!trimmed) return;

    setTerminalLogs(prev => [...prev, `ld > ${trimmed}`]);
    setCmdInput("");

    try {
      const output = await executeLdCommand(trimmed);
      const outputLines = output.split("\n");
      setTerminalLogs(prev => [...prev, ...outputLines]);
    } catch (err) {
      setTerminalLogs(prev => [...prev, `错误: 指令执行失败: ${err}`]);
    }
  };

  // Save config action
  const handleSaveConfig = async (e) => {
    if (e) e.preventDefault();
    if (!config) return;

    const updatedConfig = { ...config };
    updatedConfig.settings = {
      ...updatedConfig.settings,
      save_path: savePath,
      video_save_type: saveFormat,
      video_record_quality: qualityDefault,
      use_proxy: useProxy === "是",
      proxy_addr: proxyAddr.trim() || null,
      delay_default: parseInt(pollInterval, 10) || 300,
    };

    const cleanCookies = {};
    if (cookies) {
      Object.keys(cookies).forEach(key => {
        if (cookies[key]) {
          cleanCookies[key] = cookies[key].trim().replace(/\r?\n|\r/g, "");
        } else {
          cleanCookies[key] = "";
        }
      });
    }
    updatedConfig.cookies = cleanCookies;
    updatedConfig.push = {
      push_channels: pushChannels,
      dingtalk_api: dingtalkApi.trim() || null,
      bark_api: barkApi.trim() || null,
      tg_token: tgToken.trim() || null,
      tg_chat_id: tgChatId.trim() || null,
      tg_auto_upload: tgAutoUpload,
      tg_api_url: tgApiUrl.trim() || null,
    };

    try {
      await saveConfig(updatedConfig);
      setConfig(updatedConfig);
      showAlert("保存成功", "全局配置保存成功！部分修改可能需要重启后端引擎才能完全生效。", "success");
    } catch (err) {
      showAlert("保存失败", `保存配置失败: ${err}`, "error");
    }
  };

  // Poll proxy port on mount
  useEffect(() => {
    if (isWebMode() && !getApiBaseUrl()) {
      setActiveTab("settings");
      return;
    }
    const fetchProxyPort = async () => {
      try {
        const port = await getProxyPort();
        setProxyPort(port);
      } catch (err) {
        console.error("Error fetching proxy port:", err);
      }
    };
    fetchProxyPort();
  }, []);

  // Poll monitored rooms and core engine status
  useEffect(() => {
    if (isWebMode() && !getApiBaseUrl()) return;
    const fetchRoomsAndStatus = async () => {
      try {
        const res = await getRooms();
        setRooms(res);
        const paused = await getEngineStatus();
        setIsEnginePaused(paused);
      } catch (err) {
        console.error("Error polling backend details:", err);
      }
    };

    fetchRoomsAndStatus();
    const interval = setInterval(fetchRoomsAndStatus, 3000);
    return () => clearInterval(interval);
  }, []);

  // Poll runtime logs
  useEffect(() => {
    if (activeTab !== "logs" || (isWebMode() && !getApiBaseUrl())) return;

    const fetchLogs = async () => {
      try {
        const res = await getLogs();
        setLogs(res);
      } catch (err) {
        console.error("Error fetching logs: ", err);
      }
    };

    fetchLogs();
    const interval = setInterval(fetchLogs, 2000);
    return () => clearInterval(interval);
  }, [activeTab]);

  // Poll recorded videos
  useEffect(() => {
    if (activeTab !== "videos" || (isWebMode() && !getApiBaseUrl())) return;

    const fetchRecordedVideos = async () => {
      try {
        const res = await getRecordedVideos();
        setRecordedVideos(res);
      } catch (err) {
        console.error("Error fetching recorded videos:", err);
      }
    };
    fetchRecordedVideos();
    const interval = setInterval(fetchRecordedVideos, 4000);
    return () => clearInterval(interval);
  }, [activeTab]);

  // Load config data
  useEffect(() => {
    if (activeTab !== "settings" || (isWebMode() && !getApiBaseUrl())) return;

    const loadConfig = async () => {
      try {
        const res = await getConfig();
        setConfig(res);
        if (res.settings) {
          setSavePath(res.settings.save_path || "");
          setSaveFormat(res.settings.video_save_type || "ts");
          setQualityDefault(res.settings.video_record_quality || "原画");
          setUseProxy(res.settings.use_proxy ? "是" : "否");
          setProxyAddr(res.settings.proxy_addr || "");
          setPollInterval(res.settings.delay_default ? res.settings.delay_default.toString() : "300");
        }
        if (res.cookies) {
          setCookies(res.cookies);
        }
        if (res.push) {
          setPushChannels(res.push.push_channels || []);
          setDingtalkApi(res.push.dingtalk_api || "");
          setBarkApi(res.push.bark_api || "");
          setTgToken(res.push.tg_token || "");
          setTgChatId(res.push.tg_chat_id || "");
          setTgAutoUpload(!!res.push.tg_auto_upload);
          setTgApiUrl(res.push.tg_api_url || "");
        }
      } catch (err) {
        console.error("Error loading config: ", err);
      }
    };
    loadConfig();
  }, [activeTab]);

  // Initialize Player when activePlayUrl changes
  useEffect(() => {
    if (!activePlayUrl || !videoRef.current) return;

    if (hlsRef.current) {
      hlsRef.current.destroy();
      hlsRef.current = null;
    }

    const video = videoRef.current;
    const isHls = activePlayUrl.includes("playlist=true") || activePlayUrl.includes(".m3u8");

    if (!isHls) {
      video.src = activePlayUrl;
      video.play().catch(e => console.log("Direct playback blocked or failed", e));
    } else {
      import("hls.js").then((M) => {
        const HlsClass = M.default;
        if (!activePlayUrl || !videoRef.current) return;
        if (hlsRef.current) return;

        if (video.canPlayType("application/vnd.apple.mpegurl")) {
          video.src = activePlayUrl;
        } else if (HlsClass.isSupported()) {
          const hls = new HlsClass({
            maxMaxBufferLength: 10,
            enableWorker: true,
            lowLatencyMode: true
          });
          hlsRef.current = hls;
          hls.loadSource(activePlayUrl);
          hls.attachMedia(video);
          hls.on(HlsClass.Events.MANIFEST_PARSED, () => {
            video.play().catch(e => console.log("HLS auto-play blocked or failed", e));
          });
          hls.on(HlsClass.Events.ERROR, (event, data) => {
            if (data.fatal) {
              switch (data.type) {
                case HlsClass.ErrorTypes.NETWORK_ERROR:
                  hls.startLoad();
                  break;
                case HlsClass.ErrorTypes.MEDIA_ERROR:
                  hls.recoverMediaError();
                  break;
                default:
                  hls.destroy();
                  break;
              }
            }
          });
        }
      }).catch(err => {
        console.error("Failed to load hls.js dynamically:", err);
      });
    }

    return () => {
      if (hlsRef.current) {
        hlsRef.current.destroy();
        hlsRef.current = null;
      }
    };
  }, [activePlayUrl]);

  // Toggle engine play status
  const handleToggleEngine = async () => {
    const nextState = !isEnginePaused;
    try {
      await toggleEngineStatus(nextState);
      setIsEnginePaused(nextState);
      if (nextState) {
        showAlert("监控暂停", "后台监控任务已全部挂起挂起，不再轮询检测直播状态。", "info");
      } else {
        showAlert("监控运行", "后台监控已恢复运行，开始轮询检测主播开播状态！", "success");
      }
    } catch (err) {
      showAlert("操作失败", `切换监控状态失败: ${err}`, "error");
    }
  };

  // Add room submit handler
  const handleAddRoom = async (e) => {
    e.preventDefault();
    if (!newUrl.trim()) return;
    setStatusMsg({ type: "info", text: "正在提交主播监控地址..." });
    try {
      await addRoom(newUrl.trim(), newAlias.trim() || null, newQuality || null);
      setStatusMsg({ type: "success", text: "成功添加主播到监控调度列表！" });
      setNewUrl("");
      setNewAlias("");
      const res = await getRooms();
      setRooms(res);
      setTimeout(() => setStatusMsg({ type: "", text: "" }), 3000);
    } catch (err) {
      setStatusMsg({ type: "error", text: `添加监控任务失败: ${err}` });
    }
  };

  // Delete room confirmation handler
  const handleDeleteRoom = async (url) => {
    showConfirm(
      "移除确认",
      `确定要从监控队列中删除此直播间 URL "${url}" 吗？`,
      async () => {
        try {
          await deleteRoom(url);
          setRooms(prev => prev.filter(r => r.url !== url));
          showAlert("移除成功", "已成功删除该直播间监控任务！", "success");
        } catch (err) {
          showAlert("移除失败", `移除监控任务失败: ${err}`, "error");
        }
      }
    );
  };

  // Delete video confirmation handler
  const handleDeleteVideo = async (path, name) => {
    showConfirm(
      "删除视频确认",
      `确定要永久删除已录视频 "${name}" 吗？此操作将从磁盘上彻底删除该文件，且不可恢复！`,
      async () => {
        try {
          await deleteVideoFile(path);
          setRecordedVideos(prev => prev.filter(v => v.path !== path));
          showAlert("删除成功", `已成功删除视频切片文件！`, "success");
        } catch (err) {
          showAlert("删除失败", `无法删除文件: ${err}`, "error");
        }
      }
    );
  };

  // Toggle room paused monitoring status
  const handleToggleRoomPaused = async (url, paused) => {
    try {
      await toggleRoomPaused(url, paused);
      setRooms(prev => prev.map(r => r.url === url ? { ...r, status: paused ? "Paused" : "Idle" } : r));
    } catch (err) {
      showAlert("操作失败", `无法切换直播监控状态: ${err}`, "error");
    }
  };

  // Open folder path (Tauri mode only)
  const handleOpenFolder = async (path) => {
    try {
      await openRecordedFolder(path);
    } catch (err) {
      showAlert("打开失败", `打开文件夹失败: ${err}`, "error");
    }
  };

  const livingRooms = rooms.filter(r => r.status === "Living");

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground transition-all duration-300">
      
      {/* 1. Custom Titlebar for Tauri Frameless Window */}
      {!isWebMode() && (
        <div className="fixed top-0 right-0 h-10 flex items-center justify-end z-50 select-none pr-2 bg-transparent">
          <div className="flex items-center h-full">
            {/* Grab handle */}
            <div
              className="flex items-center justify-center w-10 h-full text-muted-foreground hover:text-foreground cursor-grab active:cursor-grabbing"
              onMouseDown={handleDragStart}
              title={lang === "zh" ? "拖动窗口" : "Drag Window"}
            >
              <GripHorizontal size={14} />
            </div>
            {/* Language Switcher */}
            <button 
              className="flex items-center justify-center px-3.5 h-full text-xs font-bold text-muted-foreground hover:bg-secondary hover:text-foreground transition-all cursor-pointer mr-1"
              onClick={toggleLanguage} 
              title={lang === "zh" ? "Switch to English" : "切换为中文"}
            >
              <span>{lang === "zh" ? "EN" : "中"}</span>
            </button>
            <button 
              className="flex items-center justify-center w-10 h-full text-muted-foreground hover:bg-secondary hover:text-foreground transition-all cursor-pointer"
              onClick={handleMinimize} 
              title={lang === "zh" ? "最小化" : "Minimize"}
            >
              <Minus size={14} />
            </button>
            <button 
              className="flex items-center justify-center w-10 h-full text-muted-foreground hover:bg-secondary hover:text-foreground transition-all cursor-pointer"
              onClick={handleMaximize} 
              title={lang === "zh" ? "最大化" : "Maximize"}
            >
              <Square size={10} />
            </button>
            <button 
              className="flex items-center justify-center w-10 h-full text-muted-foreground hover:bg-rose-500 hover:text-white transition-all cursor-pointer"
              onClick={handleClose} 
              title={lang === "zh" ? "关闭" : "Close"}
            >
              <X size={14} />
            </button>
          </div>
        </div>
      )}

      {/* 2. Responsive Side Navigation menu */}
      <Sidebar
        activeTab={activeTab}
        onChangeTab={setActiveTab}
        currentTheme={theme}
        onChangeTheme={setTheme}
        onShuffle={handleShuffleTheme}
        onSaveCustom={handleSaveCustomTheme}
        hasSavedCustom={hasSavedCustom}
        isEnginePaused={isEnginePaused}
        lang={lang}
        toggleLanguage={toggleLanguage}
      />

      {/* 3. Main Console Viewport */}
      <main className="flex-1 flex flex-col h-full overflow-hidden bg-background relative px-4 md:px-8 pt-20 pb-6 md:py-6">
        
        {/* Header Title section */}
        <header className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-5 border-b border-border/60 mb-5">
          <div className="space-y-1">
            <h1 className="text-xl font-bold tracking-tight text-foreground">
              {activeTab === "dashboard" && t("realtime_console", lang)}
              {activeTab === "add" && t("add_room_btn", lang)}
              {activeTab === "videos" && t("recorded_videos_title", lang)}
              {activeTab === "settings" && t("recording_base_title", lang)}
              {activeTab === "logs" && t("logs_title", lang)}
            </h1>
            <p className="text-xs text-muted-foreground">
              {activeTab === "dashboard" && t("dashboard_desc", lang)}
              {activeTab === "add" && t("add_room_desc", lang)}
              {activeTab === "videos" && t("videos_desc", lang)}
              {activeTab === "settings" && t("settings_desc", lang)}
              {activeTab === "logs" && t("logs_desc", lang)}
            </p>
          </div>

          {/* Engine global controllers & Counters */}
          <div className="flex items-center gap-2">
            <Button
              variant={isEnginePaused ? "default" : "outline"}
              className={cn(
                "h-9 px-4 font-bold text-xs shrink-0 flex items-center gap-1.5 shadow-sm transition-all duration-300",
                isEnginePaused 
                  ? "bg-amber-600 hover:bg-amber-500 text-white border-transparent"
                  : "border-border text-muted-foreground hover:text-foreground"
              )}
              onClick={handleToggleEngine}
              title={isEnginePaused ? (lang === "zh" ? "点击启动后台轮询监控" : "Click to start polling monitor") : (lang === "zh" ? "点击挂起监控引擎" : "Click to pause monitor engine")}
            >
              {isEnginePaused ? <PlayCircle size={14} /> : <PauseCircle size={14} />}
              <span>{isEnginePaused ? t("start_engine", lang) : t("pause_engine", lang)}</span>
            </Button>

            <div className="hidden sm:flex items-center gap-1 bg-secondary/55 p-1 px-2.5 rounded-lg border border-border/80 text-xs shrink-0 h-9">
              <span className="h-2 w-2 rounded-full bg-emerald-500 animate-pulse"></span>
              <span className="text-muted-foreground font-medium ml-1">{t("active_recording", lang)}</span>
              <span className="font-bold text-foreground ml-1.5">{livingRooms.length}</span>
            </div>

            <div className="hidden sm:flex items-center gap-1 bg-secondary/55 p-1 px-2.5 rounded-lg border border-border/80 text-xs shrink-0 h-9">
              <span className="h-2 w-2 rounded-full bg-blue-500"></span>
              <span className="text-muted-foreground font-medium ml-1">{t("monitored_rooms", lang)}</span>
              <span className="font-bold text-foreground ml-1.5">{rooms.length}</span>
            </div>
          </div>
        </header>

        {/* Dynamic Inner Tab pages */}
        <div className="flex-1 overflow-y-auto">
          {activeTab === "dashboard" && (
            <RoomSection
              activeTab="dashboard"
              rooms={rooms}
              proxyPort={proxyPort}
              config={config}
              newUrl={newUrl}
              setNewUrl={setNewUrl}
              newAlias={newAlias}
              setNewAlias={setNewAlias}
              newQuality={newQuality}
              setNewQuality={setNewQuality}
              statusMsg={statusMsg}
              handleAddRoom={handleAddRoom}
              handleToggleRoomPaused={handleToggleRoomPaused}
              handleDeleteRoom={handleDeleteRoom}
              setRoomConfigModal={setRoomConfigModal}
              setActivePlayUrl={setActivePlayUrl}
              setActivePlayTitle={setActivePlayTitle}
              setActiveTab={setActiveTab}
              lang={lang}
            />
          )}

          {activeTab === "add" && (
            <RoomSection
              activeTab="add"
              rooms={rooms}
              proxyPort={proxyPort}
              config={config}
              newUrl={newUrl}
              setNewUrl={setNewUrl}
              newAlias={newAlias}
              setNewAlias={setNewAlias}
              newQuality={newQuality}
              setNewQuality={setNewQuality}
              statusMsg={statusMsg}
              handleAddRoom={handleAddRoom}
              handleToggleRoomPaused={handleToggleRoomPaused}
              handleDeleteRoom={handleDeleteRoom}
              setRoomConfigModal={setRoomConfigModal}
              setActivePlayUrl={setActivePlayUrl}
              setActivePlayTitle={setActivePlayTitle}
              setActiveTab={setActiveTab}
              lang={lang}
            />
          )}

          {activeTab === "videos" && (
            <VideoSection
              recordedVideos={recordedVideos}
              proxyPort={proxyPort}
              handleOpenFolder={handleOpenFolder}
              handleDeleteVideo={handleDeleteVideo}
              setActivePlayUrl={setActivePlayUrl}
              setActivePlayTitle={setActivePlayTitle}
              showAlert={showAlert}
              lang={lang}
            />
          )}

          {activeTab === "settings" && (
            <SettingsSection
              activeTab="settings"
              remoteApiBase={remoteApiBase}
              setRemoteApiBase={setRemoteApiBase}
              remoteApiToken={remoteApiToken}
              setRemoteApiToken={setRemoteApiToken}
              savePath={savePath}
              setSavePath={setSavePath}
              saveFormat={saveFormat}
              setSaveFormat={setSaveFormat}
              pollInterval={pollInterval}
              setPollInterval={setPollInterval}
              useProxy={useProxy}
              setUseProxy={setUseProxy}
              proxyAddr={proxyAddr}
              setProxyAddr={setProxyAddr}
              pushChannels={pushChannels}
              setPushChannels={setPushChannels}
              dingtalkApi={dingtalkApi}
              setDingtalkApi={setDingtalkApi}
              barkApi={barkApi}
              setBarkApi={setBarkApi}
              tgToken={tgToken}
              setTgToken={setTgToken}
              tgChatId={tgChatId}
              setTgChatId={setTgChatId}
              tgApiUrl={tgApiUrl}
              setTgApiUrl={setTgApiUrl}
              tgAutoUpload={tgAutoUpload}
              setTgAutoUpload={setTgAutoUpload}
              cookies={cookies}
              setCookieModal={setCookieModal}
              handleSaveConfig={handleSaveConfig}
              showAlert={showAlert}
              lang={lang}
            />
          )}

          {activeTab === "logs" && (
            <LogViewer
              activeTab="logs"
              logs={logs}
              terminalLogs={terminalLogs}
              cmdInput={cmdInput}
              setCmdInput={setCmdInput}
              handleRunCommand={handleRunCommand}
              lang={lang}
            />
          )}
        </div>
      </main>

      {/* 4. Unified Overlays Modals */}
      <ModalOverlays
        activePlayUrl={activePlayUrl}
        activePlayTitle={activePlayTitle}
        setActivePlayUrl={setActivePlayUrl}
        setActivePlayTitle={setActivePlayTitle}
        videoRef={videoRef}
        hlsRef={hlsRef}
        modal={modal}
        closeModal={closeModal}
        cookieModal={cookieModal}
        setCookieModal={setCookieModal}
        saveCookie={saveCookie}
        getConfig={getConfig}
        setCookies={setCookies}
        roomConfigModal={roomConfigModal}
        setRoomConfigModal={setRoomConfigModal}
        updateRoomConfig={updateRoomConfig}
        setRooms={setRooms}
        getRooms={getRooms}
        setConfig={setConfig}
        qualityDefault={qualityDefault}
        saveFormat={saveFormat}
        showAlert={showAlert}
        lang={lang}
      />
    </div>
  );
}

export default App;
