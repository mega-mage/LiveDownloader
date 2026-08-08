import React from "react";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "./ui/card";
import { Input } from "./ui/input";
import { Select } from "./ui/select";
import { isWebMode, getApiBaseUrl, setApiBaseUrl, getApiToken, setApiToken, getConfig, saveConfig } from "@/services/api";
import { t } from "../lib/i18n.js";
import { 
  Server, 
  Settings, 
  Database, 
  Cookie, 
  Send,
  Sliders,
  Sparkles,
  Link2,
  Scissors
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
  pollInterval,
  setPollInterval,
  useProxy,
  setUseProxy,
  proxyAddr,
  setProxyAddr,
  splitSizeMb,
  setSplitSizeMb,
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
  showAlert,
  lang
}) {
  const isWeb = isWebMode();

  if (activeTab !== "settings") return null;

  const handleSaveSection = async (sectionKey) => {
    let baseConfig = {};
    try {
      baseConfig = (await getConfig()) || {};
    } catch (err) {
      console.error("Failed to fetch existing config:", err);
    }

    const numVal = (v, def) => {
      const parsed = parseInt(v, 10);
      return isNaN(parsed) ? def : parsed;
    };

    const updatedConfig = { ...baseConfig };
    updatedConfig.settings = { ...updatedConfig.settings };

    let successMsg = "";
    if (sectionKey === "basic") {
      updatedConfig.settings.save_path = savePath;
      updatedConfig.settings.delay_default = numVal(pollInterval, 300);
      successMsg = lang === "zh" ? "录制与保存基本配置保存成功！" : "Basic recording settings saved successfully!";
    } else if (sectionKey === "segment") {
      updatedConfig.settings.split_size_mb = numVal(splitSizeMb, 1024);
      successMsg = lang === "zh" ? "录制分段切片设置保存成功！" : "Segment settings saved successfully!";
    } else if (sectionKey === "proxy") {
      updatedConfig.settings.use_proxy = useProxy === "是";
      updatedConfig.settings.proxy_addr = proxyAddr.trim() || null;
      successMsg = lang === "zh" ? "网络代理配置保存成功！" : "Proxy settings saved successfully!";
    } else if (sectionKey === "push") {
      updatedConfig.push = {
        push_channels: pushChannels,
        dingtalk_api: dingtalkApi.trim() || null,
        bark_api: barkApi.trim() || null,
        tg_token: tgToken.trim() || null,
        tg_chat_id: tgChatId.trim() || null,
        tg_auto_upload: tgAutoUpload,
        tg_api_url: tgApiUrl.trim() || null,
      };
      successMsg = lang === "zh" ? "开播消息推送与上传配置保存成功！" : "Push notification settings saved successfully!";
    }

    try {
      await saveConfig(updatedConfig);
      showAlert(lang === "zh" ? "保存成功" : "Saved", successMsg, "success");
    } catch (err) {
      showAlert(lang === "zh" ? "保存失败" : "Error", `保存配置失败: ${err}`, "error");
    }
  };

  return (
    <div className="space-y-6 max-w-4xl mx-auto pb-10 animate-slide-in">
      <div className="space-y-6">
        
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
                  {window.location.protocol === "https:" && remoteApiBase.toLowerCase().startsWith("http:") && !remoteApiBase.toLowerCase().startsWith("https:") && (
                    <p className="text-xxs text-rose-500 mt-1 leading-normal font-semibold">
                      {lang === "zh" 
                        ? "⚠️ 混合内容限制：网页当前处于安全加密的 HTTPS，但填写的 API 为非加密 HTTP。浏览器将会拦截此网络请求。请使用 HTTPS 地址，或通过不带 S 的 HTTP 网页访问。" 
                        : "⚠️ Mixed Content: Page is loaded over secure HTTPS, but requested API is HTTP. Browser will block this request. Please use HTTPS or load the page over HTTP."
                      }
                    </p>
                  )}
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
          <CardFooter className="p-4 px-5 border-t border-border/40 bg-secondary/15 justify-end">
            <Button
              type="button"
              className="h-8 text-xs font-semibold"
              onClick={() => handleSaveSection("basic")}
            >
              <Database size={12} className="mr-1.5" />
              {lang === "zh" ? "保存基本配置" : "Save Basic Settings"}
            </Button>
          </CardFooter>
        </Card>

        {/* 2.5 Segment / Split Configurations */}
        <Card className="border border-border bg-card/45 backdrop-blur-md shadow-md">
          <CardHeader className="p-5 border-b border-border/50">
            <CardTitle className="text-sm font-bold text-foreground flex items-center gap-2">
              <Scissors size={16} className="text-primary" />
              <span>{lang === "zh" ? "录制分段切片设置" : "Recording Segment Settings"}</span>
            </CardTitle>
            <CardDescription className="text-xs text-muted-foreground mt-1">
              {lang === "zh"
                ? "配置直播录制时的分段切片与自动合并输出体积"
                : "Configure segment auto-concatenation size limit for recordings"}
            </CardDescription>
          </CardHeader>
          <CardContent className="p-5 space-y-4">
            <div className="space-y-2 max-w-md">
              <label className="text-xs font-bold text-foreground/80">
                {lang === "zh" ? "分段切片目标体积 (MB)" : "Target Segment File Size (MB)"}
              </label>
              <Input
                type="number"
                value={splitSizeMb}
                onChange={(e) => setSplitSizeMb(e.target.value)}
                placeholder="1024"
              />
              <p className="text-xxs text-emerald-600 dark:text-emerald-400 font-medium leading-relaxed">
                {lang === "zh"
                  ? "✨ 录制时默认每 10 分钟生成切片落地于 downloading/[主播名]/ 独立目录中。当已闭合切片总体积达到设定的 MB 值（如 1024 MB = 1GB），或主播下播时，系统会自动无损合并输出至保存目录。设为 0 表示整场直播合并为单文件。"
                  : "✨ Segments are written every 10 mins into downloading/[Streamer]/ subfolder. When accumulated closed segments reach this MB target (e.g. 1024 MB), or when stream ends, they auto-concat into final output directory. Set to 0 to output single file per live session."}
              </p>
            </div>
          </CardContent>
          <CardFooter className="p-4 px-5 border-t border-border/40 bg-secondary/15 justify-end">
            <Button
              type="button"
              className="h-8 text-xs font-semibold"
              onClick={() => handleSaveSection("segment")}
            >
              <Database size={12} className="mr-1.5" />
              {lang === "zh" ? "保存切片设置" : "Save Segment Settings"}
            </Button>
          </CardFooter>
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
          <CardFooter className="p-4 px-5 border-t border-border/40 bg-secondary/15 justify-end">
            <Button
              type="button"
              className="h-8 text-xs font-semibold"
              onClick={() => handleSaveSection("proxy")}
            >
              <Database size={12} className="mr-1.5" />
              {lang === "zh" ? "保存代理配置" : "Save Proxy Settings"}
            </Button>
          </CardFooter>
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
          <CardFooter className="p-4 px-5 border-t border-border/40 bg-secondary/15 justify-end">
            <Button
              type="button"
              className="h-8 text-xs font-semibold"
              onClick={() => handleSaveSection("push")}
            >
              <Database size={12} className="mr-1.5" />
              {lang === "zh" ? "保存推送配置" : "Save Push Settings"}
            </Button>
          </CardFooter>
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

      </div>
    </div>
  );
}
