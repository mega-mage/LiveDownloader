// src/services/api.js
// Unified API adapter: auto-detects Tauri desktop vs Web browser environment
// and routes calls accordingly.

const isTauri = () => !!window.__TAURI_INTERNALS__;

// === Web mode configuration (persisted in localStorage) ===
const STORAGE_KEY_API_BASE = "ld_api_base_url";
const STORAGE_KEY_API_TOKEN = "ld_api_token";

export function getApiBaseUrl() {
  return localStorage.getItem(STORAGE_KEY_API_BASE) || "";
}

export function setApiBaseUrl(url) {
  localStorage.setItem(STORAGE_KEY_API_BASE, url.replace(/\/+$/, ""));
}

export function getApiToken() {
  return localStorage.getItem(STORAGE_KEY_API_TOKEN) || "";
}

export function setApiToken(token) {
  localStorage.setItem(STORAGE_KEY_API_TOKEN, token);
}

export function isWebMode() {
  return !isTauri();
}

// === Internal HTTP fetch helper (Web mode) ===
async function apiFetch(path, options = {}) {
  const base = getApiBaseUrl();
  if (!base) {
    throw new Error("未配置远程服务器地址。请在设置中填写后端 API 地址。");
  }
  const url = `${base}${path}`;
  const token = getApiToken();
  const headers = {
    "Content-Type": "application/json",
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...options.headers,
  };
  const res = await fetch(url, { ...options, headers });
  if (!res.ok) {
    if (res.status === 401) {
      throw new Error("API 认证失败 (401)。请检查 API Token 是否正确。");
    }
    const text = await res.text().catch(() => res.statusText);
    throw new Error(`API 请求失败 (${res.status}): ${text}`);
  }
  return res.json();
}

// === Tauri invoke helper ===
let _invoke = null;
async function tauriInvoke(cmd, args = {}) {
  if (!_invoke) {
    const mod = await import("@tauri-apps/api/core");
    _invoke = mod.invoke;
  }
  return _invoke(cmd, args);
}

// ============================
//  Public API functions
// ============================

export async function getRooms() {
  if (isTauri()) return tauriInvoke("get_rooms");
  return apiFetch("/api/rooms");
}

export async function addRoom(url, name, quality) {
  if (isTauri()) return tauriInvoke("add_room", { url, name, quality });
  return apiFetch("/api/room", {
    method: "POST",
    body: JSON.stringify({ url, name, quality }),
  });
}

export async function deleteRoom(url) {
  if (isTauri()) return tauriInvoke("delete_room", { url });
  return apiFetch("/api/room", {
    method: "DELETE",
    body: JSON.stringify({ url }),
  });
}

export async function getConfig() {
  if (isTauri()) return tauriInvoke("get_config");
  return apiFetch("/api/config");
}

export async function saveConfig(newConfig) {
  if (isTauri()) return tauriInvoke("save_config", { newConfig });
  return apiFetch("/api/config", {
    method: "POST",
    body: JSON.stringify(newConfig),
  });
}

export async function getLogs() {
  if (isTauri()) return tauriInvoke("get_logs");
  return apiFetch("/api/logs");
}

export async function getProxyPort() {
  if (isTauri()) return tauriInvoke("get_proxy_port");
  return apiFetch("/api/proxy-port");
}

export async function toggleEngineStatus(paused) {
  if (isTauri()) return tauriInvoke("toggle_engine_status", { paused });
  return apiFetch("/api/engine/toggle", {
    method: "POST",
    body: JSON.stringify({ paused }),
  });
}

export async function getEngineStatus() {
  if (isTauri()) return tauriInvoke("get_engine_status");
  return apiFetch("/api/engine/status");
}

export async function getRecordedVideos() {
  if (isTauri()) return tauriInvoke("get_recorded_videos");
  return apiFetch("/api/videos");
}

export async function openRecordedFolder(path) {
  if (isTauri()) return tauriInvoke("open_recorded_folder", { path });
  // In web mode, this is a no-op (can't open local folders from browser)
  throw new Error("网页模式下无法打开本地文件夹。");
}

export async function toggleRoomPaused(url, paused) {
  if (isTauri()) return tauriInvoke("toggle_room_paused", { url, paused });
  return apiFetch("/api/room/toggle", {
    method: "POST",
    body: JSON.stringify({ url, paused }),
  });
}

export async function saveCookie(platform, value) {
  if (isTauri()) return tauriInvoke("save_cookie", { platform, value });
  return apiFetch("/api/cookie", {
    method: "POST",
    body: JSON.stringify({ platform, value }),
  });
}

export async function updateRoomConfig(url, name, quality, videoSaveType) {
  if (isTauri())
    return tauriInvoke("update_room_config", {
      url,
      name,
      quality,
      video_save_type: videoSaveType,
    });
  return apiFetch("/api/room/config", {
    method: "POST",
    body: JSON.stringify({
      url,
      name,
      quality,
      video_save_type: videoSaveType,
    }),
  });
}

export async function executeLdCommand(cmd) {
  if (isTauri()) return tauriInvoke("execute_ld_command", { cmd });
  const res = await apiFetch("/api/command", {
    method: "POST",
    body: JSON.stringify({ cmd }),
  });
  return res.output || "";
}

export async function getDownloadLink(path) {
  if (isTauri()) {
    throw new Error("Tauri mode doesn't need signed download links");
  }
  const res = await apiFetch("/api/video/download-link", {
    method: "POST",
    body: JSON.stringify({ path }),
  });
  // Prepend API base URL to the relative URL returned by the backend
  const base = getApiBaseUrl();
  return `${base}${res.url}`;
}

export async function deleteVideoFile(path) {
  if (isTauri()) return tauriInvoke("delete_video_file", { path });
  return apiFetch("/api/video", {
    method: "DELETE",
    body: JSON.stringify({ path }),
  });
}

// === Utility: build the stream proxy URL ===
export function getStreamProxyUrl(liveUrl, referer, proxyPort) {
  if (isTauri()) {
    return `http://127.0.0.1:${proxyPort}/proxy?url=${encodeURIComponent(liveUrl)}&referer=${encodeURIComponent(referer)}`;
  }
  // Web mode: route through the remote backend's proxy
  const base = getApiBaseUrl();
  if (!base) return liveUrl; // fallback
  // Extract the host:port from the base URL and use the proxy port from the API
  return `${base.replace(/:\d+$/, "")}:${proxyPort}/proxy?url=${encodeURIComponent(liveUrl)}&referer=${encodeURIComponent(referer)}`;
}
