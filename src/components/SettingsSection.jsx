import React from "react";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "./ui/card";
import { Input } from "./ui/input";
import { Select } from "./ui/select";
import { isWebMode, getApiBaseUrl, setApiBaseUrl, getApiToken, setApiToken } from "@/services/api";
import { t } from "../lib/i18n.js";
import { 
  Server, 
  Settings, 
  Database, 
  Cookie, 
  Send,
  Sliders,
  Sparkles,
  Link2
} from "lucide-react";

const getPlatformName = (name, lang) => {
  if (lang === "zh") return name;
  const map = {
    "抖音直播": "Douyin Live",
    "哔哩哔哩": "Bilibili Live",
    "虎牙直播": "Huya Live",
    "快手直播": "Kuaishou Live",
    "斗鱼直播": "Douyu Live",
    "猫耳FM": "Missevan FM",
    "网易CC": "NetEase CC",
    "微博直播": "Weibo Live",
    "淘宝直播": "Taobao Live",
    "AcFun": "AcFun Live",
    "Twitch直播": "Twitch Live"
  };
  return map[name] || name;
};

export function SettingsSection({
  activeTab,
  remoteApiBase,
  setRemoteApiBase,
  remoteApiToken,
  setRemoteApiToken,
  savePath,
  setSavePath,
  saveFormat,
  setSaveFormat,
  pollInterval,
  setPollInterval,
  useProxy,
  setUseProxy,
  proxyAddr,
  setProxyAddr,
  pushChannels,
  setPushChannels,
  dingtalkApi,
  setDingtalkApi,
  barkApi,
  setBarkApi,
  tgToken,
  setTgToken,
  tgChatId,
  setTgChatId,
  tgApiUrl,
  setTgApiUrl,
  tgAutoUpload,
  setTgAutoUpload,
  cookies,
  setCookieModal,
  handleSaveConfig,
  showAlert,
  lang
}) {
  const isWeb = isWebMode();

  if (activeTab !== "settings") return null;

  return (
    <div className="space-y-6 max-w-4xl mx-auto pb-10 animate-slide-in">
      <form onSubmit={handleSaveConfig} className="space-y-6">
        
        {/* 1. Connection Config (Only in Web browser mode) */}
        {isWeb && (
          <Card className="border border-border bg-card/45 backdrop-blur-md shadow-md">
            <CardHeader className="p-5 border-b border-border/50">
              <CardTitle className="text-sm font-bold text-foreground flex items-center gap-2">
                <Server size={16} className="text-primary" />
                <span>{t("remote_connection_title", lang)}</span>
              </CardTitle>
              <CardDescription className="text-xs text-muted-foreground mt-1">
                {t("remote_connection_desc", lang)}
              </CardDescription>
            </CardHeader>
            <CardContent className="p-5 space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div className="space-y-2">
                  <label className="text-xs font-bold text-foreground/80">{t("api_server_address", lang)}</label>
                  <Input
                    placeholder={t("api_server_placeholder", lang)}
                    value={remoteApiBase}
                    onChange={(e) => setRemoteApiBase(e.target.value)}
                    onBlur={() => setApiBaseUrl(remoteApiBase)}
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-bold text-foreground/80">{t("api_token", lang)}</label>
                  <Input
                    type="password"
                    placeholder={t("api_token_placeholder", lang)}
                    value={remoteApiToken}
                    onChange={(e) => setRemoteApiToken(e.target.value)}
                    onBlur={() => setApiToken(remoteApiToken)}
                  />
                </div>
              </div>
            </CardContent>
            <CardFooter className="p-4 px-5 border-t border-border/40 bg-secondary/15 justify-end">
              <Button
                type="button"
                className="h-8 text-xs font-semibold"
                onClick={() => {
                  setApiBaseUrl(remoteApiBase);
                  setApiToken(remoteApiToken);
                  showAlert(
                    lang === "zh" ? "连接配置已更新" : "Connection Config Updated",
                    lang === "zh" ? "API 连接配置已保存，正在重新载入..." : "API credentials updated, reloading...",
                    "success"
                  );
                  setTimeout(() => window.location.reload(), 1200);
                }}
              >
                <Link2 size={12} className="mr-1.5" />
                {t("apply_reconnect", lang)}
              </Button>
            </CardFooter>
          </Card>
        )}

        {/* 2. Recording Basic Configurations */}
        <Card className="border border-border bg-card/45 backdrop-blur-md shadow-md">
          <CardHeader className="p-5 border-b border-border/50">
            <CardTitle className="text-sm font-bold text-foreground flex items-center gap-2">
              <Sliders size={16} className="text-primary" />
              <span>{t("recording_base_title", lang)}</span>
            </CardTitle>
            <CardDescription className="text-xs text-muted-foreground mt-1">
              {t("recording_base_desc", lang)}
            </CardDescription>
          </CardHeader>
          <CardContent className="p-5 space-y-4">
            <div className="space-y-2">
              <label className="text-xs font-bold text-foreground/80">{t("save_path", lang)}</label>
              <Input
                value={savePath}
                onChange={(e) => setSavePath(e.target.value)}
                placeholder={t("save_path_placeholder", lang)}
              />
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-xs font-bold text-foreground/80">{t("save_format", lang)}</label>
                <Select value={saveFormat} onChange={(e) => setSaveFormat(e.target.value)}>
                  <option value="ts">{lang === "zh" ? "MPEG-TS (.ts) - 最稳定" : "MPEG-TS (.ts) - Recommended"}</option>
                  <option value="mp4">MP4 (.mp4)</option>
                  <option value="flv">FLV (.flv)</option>
                  <option value="mkv">Matroska (.mkv)</option>
                  <option value="mp3">{lang === "zh" ? "MP3 音频 (.mp3)" : "MP3 Audio (.mp3)"}</option>
                  <option value="m4a">{lang === "zh" ? "M4A 音频 (.m4a)" : "M4A Audio (.m4a)"}</option>
                </Select>
              </div>
              <div className="space-y-2">
                <label className="text-xs font-bold text-foreground/80">{t("poll_interval", lang)}</label>
                <Input
                  type="number"
                  value={pollInterval}
                  onChange={(e) => setPollInterval(e.target.value)}
                  placeholder={t("poll_interval_placeholder", lang)}
                />
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 3. Proxy Configurations */}
        <Card className="border border-border bg-card/45 backdrop-blur-md shadow-md">
          <CardHeader className="p-5 border-b border-border/50">
            <CardTitle className="text-sm font-bold text-foreground flex items-center gap-2">
              <Sparkles size={16} className="text-primary" />
              <span>{t("proxy_config_title", lang)}</span>
            </CardTitle>
            <CardDescription className="text-xs text-muted-foreground mt-1">
              {t("proxy_config_desc", lang)}
            </CardDescription>
          </CardHeader>
          <CardContent className="p-5">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-xs font-bold text-foreground/80">{t("enable_proxy", lang)}</label>
                <Select value={useProxy} onChange={(e) => setUseProxy(e.target.value)}>
                  <option value="是">{t("proxy_yes", lang)}</option>
                  <option value="否">{t("proxy_no", lang)}</option>
                </Select>
              </div>
              <div className="space-y-2">
                <label className="text-xs font-bold text-foreground/80">{t("proxy_address", lang)}</label>
                <Input
                  value={proxyAddr}
                  onChange={(e) => setProxyAddr(e.target.value)}
                  placeholder={t("proxy_address_placeholder", lang)}
                />
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 4. Notification Push Configurations */}
        <Card className="border border-border bg-card/45 backdrop-blur-md shadow-md">
          <CardHeader className="p-5 border-b border-border/50">
            <CardTitle className="text-sm font-bold text-foreground flex items-center gap-2">
              <Send size={16} className="text-primary" />
              <span>{t("push_title", lang)}</span>
            </CardTitle>
            <CardDescription className="text-xs text-muted-foreground mt-1">
              {t("push_desc", lang)}
            </CardDescription>
          </CardHeader>
          <CardContent className="p-5 space-y-5">
            {/* Dingtalk option */}
            <div className="flex flex-col sm:flex-row sm:items-center gap-4 py-2 border-b border-border/30">
              <div className="flex items-center gap-2.5 min-w-[200px]">
                <input
                  type="checkbox"
                  id="enable-dingtalk"
                  checked={pushChannels.includes("dingtalk")}
                  onChange={(e) => {
                    if (e.target.checked) setPushChannels(prev => [...prev, "dingtalk"]);
                    else setPushChannels(prev => prev.filter(c => c !== "dingtalk"));
                  }}
                  className="h-4 w-4 rounded border-border text-primary focus:ring-primary cursor-pointer"
                />
                <label htmlFor="enable-dingtalk" className="text-xs font-bold text-foreground/90 cursor-pointer">
                  {t("enable_dingtalk", lang)}
                </label>
              </div>
              <div className="flex-1">
                <Input
                  placeholder={t("dingtalk_placeholder", lang)}
                  value={dingtalkApi}
                  onChange={(e) => setDingtalkApi(e.target.value)}
                  disabled={!pushChannels.includes("dingtalk")}
                />
              </div>
            </div>

            {/* Bark option */}
            <div className="flex flex-col sm:flex-row sm:items-center gap-4 py-2 border-b border-border/30">
              <div className="flex items-center gap-2.5 min-w-[200px]">
                <input
                  type="checkbox"
                  id="enable-bark"
                  checked={pushChannels.includes("bark")}
                  onChange={(e) => {
                    if (e.target.checked) setPushChannels(prev => [...prev, "bark"]);
                    else setPushChannels(prev => prev.filter(c => c !== "bark"));
                  }}
                  className="h-4 w-4 rounded border-border text-primary focus:ring-primary cursor-pointer"
                />
                <label htmlFor="enable-bark" className="text-xs font-bold text-foreground/90 cursor-pointer">
                  {t("enable_bark", lang)}
                </label>
              </div>
              <div className="flex-1">
                <Input
                  placeholder={t("bark_placeholder", lang)}
                  value={barkApi}
                  onChange={(e) => setBarkApi(e.target.value)}
                  disabled={!pushChannels.includes("bark")}
                />
              </div>
            </div>

            {/* Telegram option */}
            <div className="space-y-4 py-2">
              <div className="flex items-center gap-2.5">
                <input
                  type="checkbox"
                  id="enable-telegram"
                  checked={pushChannels.includes("telegram")}
                  onChange={(e) => {
                    if (e.target.checked) setPushChannels(prev => [...prev, "telegram"]);
                    else setPushChannels(prev => prev.filter(c => c !== "telegram"));
                  }}
                  className="h-4 w-4 rounded border-border text-primary focus:ring-primary cursor-pointer"
                />
                <label htmlFor="enable-telegram" className="text-xs font-bold text-foreground/90 cursor-pointer">
                  {t("enable_telegram", lang)}
                </label>
              </div>

              {pushChannels.includes("telegram") && (
                <div className="pl-6 space-y-3 animate-slide-in">
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <div className="space-y-1">
                      <span className="text-[10px] font-bold text-muted-foreground uppercase">Bot Token</span>
                      <Input
                        type="password"
                        placeholder="123456789:ABCdef..."
                        value={tgToken}
                        onChange={(e) => setTgToken(e.target.value)}
                      />
                    </div>
                    <div className="space-y-1">
                      <span className="text-[10px] font-bold text-muted-foreground uppercase">Chat ID</span>
                      <Input
                        placeholder="例如: -100123456789"
                        value={tgChatId}
                        onChange={(e) => setTgChatId(e.target.value)}
                      />
                    </div>
                  </div>
                  <div className="space-y-1">
                    <span className="text-[10px] font-bold text-muted-foreground uppercase">
                      {lang === "zh" ? "自建 API 服务反代地址 (可选)" : "Custom Telegram Proxy Base (Optional)"}
                    </span>
                    <Input
                      placeholder={t("tg_api_url_placeholder", lang)}
                      value={tgApiUrl}
                      onChange={(e) => setTgApiUrl(e.target.value)}
                    />
                  </div>
                  <div className="flex items-center gap-2 pt-1">
                    <input
                      type="checkbox"
                      id="enable-tg-upload"
                      checked={tgAutoUpload}
                      onChange={(e) => setTgAutoUpload(e.target.checked)}
                      className="h-3.5 w-3.5 rounded border-border cursor-pointer"
                    />
                    <label htmlFor="enable-tg-upload" className="text-xs text-muted-foreground cursor-pointer select-none">
                      {t("tg_auto_upload", lang)} {tgApiUrl.trim() ? t("tg_auto_upload_desc_proxy", lang) : t("tg_auto_upload_desc_default", lang)}
                    </label>
                  </div>
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        {/* 5. Platform Credentials (Cookies) */}
        <Card className="border border-border bg-card/45 backdrop-blur-md shadow-md animate-none">
          <CardHeader className="p-5 border-b border-border/50">
            <CardTitle className="text-sm font-bold text-foreground flex items-center gap-2">
              <Cookie size={16} className="text-primary" />
              <span>{t("cookie_title", lang)}</span>
            </CardTitle>
            <CardDescription className="text-xs text-muted-foreground mt-1">
              {t("cookie_desc", lang)}
            </CardDescription>
          </CardHeader>
          <CardContent className="p-5">
            <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-3">
              {[
                { key: "抖音cookie", name: "抖音直播" },
                { key: "b站cookie", name: "哔哩哔哩" },
                { key: "虎牙cookie", name: "虎牙直播" },
                { key: "快手cookie", name: "快手直播" },
                { key: "斗鱼cookie", name: "斗鱼直播" },
                { key: "猫耳cookie", name: "猫耳FM" },
                { key: "网易cccookie", name: "网易CC" },
                { key: "微博cookie", name: "微博直播" },
                { key: "淘宝cookie", name: "淘宝直播" },
                { key: "A站cookie", name: "AcFun" },
                { key: "Twitchcookie", name: "Twitch直播" }
              ].map((plat) => {
                const hasCookie = !!(cookies[plat.key] && cookies[plat.key].trim());
                const platName = getPlatformName(plat.name, lang);
                return (
                  <div 
                    key={plat.key} 
                    className={cn(
                      "flex items-center justify-between p-3 rounded-lg border text-xs transition-all animate-none",
                      hasCookie ? "border-emerald-500/20 bg-emerald-500/5 text-emerald-300" : "border-border bg-secondary/15 text-muted-foreground"
                    )}
                  >
                    <div className="flex flex-col min-w-0">
                      <span className="font-semibold text-foreground truncate">{platName}</span>
                      <span className="text-[10px] mt-0.5 opacity-80">
                        {hasCookie ? t("cookie_imported", lang) : t("cookie_missing", lang)}
                      </span>
                    </div>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      className="h-7 text-[10px] px-2 animate-none"
                      onClick={() => setCookieModal({
                        show: true,
                        platformKey: plat.key,
                        platformName: platName,
                        value: cookies[plat.key] || ""
                      })}
                    >
                      {hasCookie ? t("btn_modify", lang) : t("btn_import", lang)}
                    </Button>
                  </div>
                );
              })}
            </div>
          </CardContent>
        </Card>

        {/* Save Configurations Button Footer */}
        <div className="sticky bottom-0 -mx-5 mt-6 p-4 bg-card/90 backdrop-blur-md border-t border-border z-20 flex justify-end shadow-lg shadow-black/5">
          <Button type="submit" className="h-10 font-bold px-6 shadow-md shadow-primary/20 cursor-pointer animate-none">
            <Database size={16} className="mr-2" />
            {t("save_global_config", lang)}
          </Button>
        </div>

      </form>
    </div>
  );
}
