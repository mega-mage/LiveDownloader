import React from "react";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Select } from "./ui/select";
import { t } from "../lib/i18n.js";
import { 
  Volume2, 
  StopCircle, 
  Check, 
  AlertCircle, 
  Info, 
  HelpCircle,
  X 
} from "lucide-react";

export function ModalOverlays({
  activePlayUrl,
  activePlayTitle,
  setActivePlayUrl,
  setActivePlayTitle,
  videoRef,
  hlsRef,
  modal,
  closeModal,
  cookieModal,
  setCookieModal,
  saveCookie,
  getConfig,
  setCookies,
  roomConfigModal,
  setRoomConfigModal,
  updateRoomConfig,
  setRooms,
  getRooms,
  setConfig,
  qualityDefault,
  saveFormat,
  showAlert,
  lang
}) {
  return (
    <>
      {/* 1. Floating HLS Video Player Modal */}
      {activePlayUrl && (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4 md:p-6">
          <div className="relative w-full max-w-4xl bg-card border border-border rounded-xl shadow-2xl overflow-hidden animate-slide-in">
            {/* Player Header */}
            <div className="flex items-center justify-between px-5 py-4 border-b border-border bg-secondary/15">
              <div className="flex items-center gap-2 text-foreground font-semibold text-sm">
                <Volume2 size={16} className="text-primary animate-bounce" />
                <span>{t("live_play", lang)}: <span className="text-primary font-bold">{activePlayTitle}</span></span>
              </div>
              <Button
                variant="outline"
                size="sm"
                className="h-8 text-xs text-destructive border-destructive/25 bg-destructive/5 hover:bg-destructive/15 animate-none"
                onClick={() => {
                  if (hlsRef.current) {
                    hlsRef.current.destroy();
                    hlsRef.current = null;
                  }
                  setActivePlayUrl(null);
                  setActivePlayTitle("");
                }}
              >
                <StopCircle size={14} className="mr-1.5" />
                {t("close_player", lang)}
              </Button>
            </div>

            {/* Video Content */}
            <div className="aspect-video bg-black flex items-center justify-center">
              <video
                ref={videoRef}
                controls
                autoPlay
                className="w-full h-full max-h-[70vh] object-contain"
              />
            </div>

            {/* Player Footer */}
            <div className="px-5 py-3.5 border-t border-border bg-secondary/10 flex items-center gap-2">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse shrink-0"></span>
              <p className="text-[10px] text-muted-foreground">
                {t("player_tip", lang)}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* 2. Alert / Confirm Modal Dialog */}
      {modal.show && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-xs z-50 flex items-center justify-center p-4 animate-none">
          <div className="w-full max-w-sm bg-card border border-border rounded-xl shadow-2xl overflow-hidden animate-slide-in">
            <div className="flex items-center justify-between px-5 py-3 border-b border-border bg-secondary/15 animate-none">
              <div className="flex items-center gap-2 animate-none">
                <span className={cn(
                  "h-2 w-2 rounded-full animate-none",
                  modal.type === "success" && "bg-emerald-500",
                  modal.type === "error" && "bg-rose-500",
                  modal.type === "confirm" && "bg-amber-500",
                  modal.type === "info" && "bg-blue-500"
                )}></span>
                <h3 className="text-xs font-bold text-foreground">{modal.title}</h3>
              </div>
              <button 
                onClick={closeModal}
                className="text-muted-foreground hover:text-foreground cursor-pointer transition-colors"
              >
                <X size={14} />
              </button>
            </div>
            <div className="p-5 text-sm text-muted-foreground leading-relaxed">
              {modal.message}
            </div>
            <div className="flex items-center justify-end gap-2 p-3 px-5 border-t border-border bg-secondary/10">
              {modal.type === "confirm" && (
                <Button 
                  variant="outline" 
                  size="sm" 
                  className="h-8 text-xs" 
                  onClick={modal.onCancel}
                >
                  {t("btn_cancel", lang)}
                </Button>
              )}
              <Button 
                size="sm" 
                className="h-8 text-xs font-semibold px-4" 
                onClick={modal.onConfirm}
              >
                {t("btn_ok", lang)}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* 3. Platform Cookies Input Modal */}
      {cookieModal.show && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-xs z-50 flex items-center justify-center p-4">
          <div className="w-full max-w-md bg-card border border-border rounded-xl shadow-2xl overflow-hidden animate-slide-in">
            <div className="flex items-center justify-between px-5 py-4 border-b border-border bg-secondary/15">
              <div className="flex items-center gap-2">
                <span className="h-2 w-2 rounded-full bg-primary animate-pulse"></span>
                <h3 className="text-sm font-bold text-foreground">{cookieModal.platformName} {t("cookie_config_title", lang)}</h3>
              </div>
              <button 
                onClick={() => setCookieModal(prev => ({ ...prev, show: false }))}
                className="text-muted-foreground hover:text-foreground cursor-pointer transition-colors"
              >
                <X size={14} />
              </button>
            </div>
            <div className="p-5 space-y-4">
              <p className="text-xs text-muted-foreground leading-normal">
                {t("cookie_config_tip", lang)}
              </p>
              <textarea
                className="w-full p-3 rounded-lg border border-border bg-black/20 text-foreground font-mono text-xs outline-none focus:border-primary resize-y h-32"
                placeholder={t("cookie_config_placeholder", lang)}
                value={cookieModal.value}
                onChange={(e) => setCookieModal(prev => ({ ...prev, value: e.target.value }))}
              />
            </div>
            <div className="flex items-center justify-end gap-2 p-3 px-5 border-t border-border bg-secondary/10">
              <Button 
                variant="outline" 
                size="sm" 
                className="h-8 text-xs" 
                onClick={() => setCookieModal(prev => ({ ...prev, show: false }))}
              >
                {t("btn_cancel", lang)}
              </Button>
              <Button 
                size="sm" 
                className="h-8 text-xs font-semibold px-4 animate-none" 
                onClick={async () => {
                  try {
                    await saveCookie(cookieModal.platformKey, cookieModal.value.trim());
                    const res = await getConfig();
                    if (res.cookies) {
                      setCookies(res.cookies);
                    }
                    showAlert(
                      lang === "zh" ? "保存成功" : "Save Successful",
                      lang === "zh" ? `已成功保存 ${cookieModal.platformName} 的 Cookie 凭证！` : `Saved cookies for ${cookieModal.platformName} successfully!`,
                      "success"
                    );
                  } catch (err) {
                    showAlert(
                      lang === "zh" ? "保存失败" : "Save Failed",
                      `${lang === "zh" ? "无法保存 Cookie" : "Failed to save Cookie"}: ${err}`,
                      "error"
                    );
                  }
                  setCookieModal(prev => ({ ...prev, show: false }));
                }}
              >
                {t("btn_save_cookie", lang)}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* 4. Monitored Room Overrides Config Modal */}
      {roomConfigModal.show && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-xs z-50 flex items-center justify-center p-4 animate-none">
          <div className="w-full max-w-sm bg-card border border-border rounded-xl shadow-2xl overflow-hidden animate-slide-in">
            <div className="flex items-center justify-between px-5 py-4 border-b border-border bg-secondary/15 animate-none">
              <div className="flex items-center gap-2 animate-none">
                <span className="h-2 w-2 rounded-full bg-primary animate-pulse"></span>
                <h3 className="text-sm font-bold text-foreground">{t("modify_room_config_title", lang)}</h3>
              </div>
              <button 
                onClick={() => setRoomConfigModal(prev => ({ ...prev, show: false }))}
                className="text-muted-foreground hover:text-foreground cursor-pointer transition-colors"
              >
                <X size={14} />
              </button>
            </div>
            <div className="p-5 space-y-4">
              <div className="text-[10px] text-muted-foreground break-all bg-secondary/25 p-2 rounded">
                {lang === "zh" ? "网址" : "URL"}: <span className="font-mono text-foreground">{roomConfigModal.url}</span>
              </div>
              <div className="space-y-1.5">
                <label className="text-xs font-bold text-foreground/80">{t("custom_alias", lang)}</label>
                <Input
                  placeholder={t("custom_alias_placeholder", lang)}
                  value={roomConfigModal.name}
                  onChange={(e) => setRoomConfigModal(prev => ({ ...prev, name: e.target.value }))}
                />
              </div>
              <div className="space-y-1.5">
                <label className="text-xs font-bold text-foreground/80">{t("specified_quality", lang)}</label>
                <Select
                  value={roomConfigModal.quality}
                  onChange={(e) => setRoomConfigModal(prev => ({ ...prev, quality: e.target.value }))}
                >
                  <option value="">{t("follow_global", lang)} ({qualityDefault})</option>
                  <option value="原画">{lang === "zh" ? "原画 (最高画质)" : "Source (Max)"}</option>
                  <option value="超清">{lang === "zh" ? "超清" : "1080p / Ultra"}</option>
                  <option value="高清">{lang === "zh" ? "高清" : "720p / HD"}</option>
                  <option value="标清">{lang === "zh" ? "标清" : "480p / SD"}</option>
                  <option value="流畅">{lang === "zh" ? "流畅" : "Speed"}</option>
                </Select>
              </div>
              <div className="space-y-1.5">
                <label className="text-xs font-bold text-foreground/80">{t("save_video_format", lang)}</label>
                <Select
                  value={roomConfigModal.videoSaveType}
                  onChange={(e) => setRoomConfigModal(prev => ({ ...prev, videoSaveType: e.target.value }))}
                >
                  <option value="">{lang === "zh" ? `跟随全局配置 (${saveFormat})` : `Follow Global (${saveFormat})`}</option>
                  <option value="ts">MPEG-TS (.ts)</option>
                  <option value="mp4">MP4 (.mp4)</option>
                  <option value="mkv">Matroska (.mkv)</option>
                  <option value="flv">FLV (.flv)</option>
                  <option value="mp3音频">{lang === "zh" ? "MP3 音频 (.mp3)" : "MP3 Audio (.mp3)"}</option>
                  <option value="m4a音频">{lang === "zh" ? "M4A 音频 (.m4a)" : "M4A Audio (.m4a)"}</option>
                </Select>
              </div>
            </div>
            <div className="flex items-center justify-end gap-2 p-3 px-5 border-t border-border bg-secondary/10">
              <Button 
                variant="outline" 
                size="sm" 
                className="h-8 text-xs animate-none" 
                onClick={() => setRoomConfigModal(prev => ({ ...prev, show: false }))}
              >
                {t("btn_cancel", lang)}
              </Button>
              <Button 
                size="sm" 
                className="h-8 text-xs font-semibold px-4 animate-none" 
                onClick={async () => {
                  try {
                    await updateRoomConfig(
                      roomConfigModal.url,
                      roomConfigModal.name.trim() || null,
                      roomConfigModal.quality || null,
                      roomConfigModal.videoSaveType || null
                    );
                    const res = await getConfig();
                    setConfig(res);
                    const roomsRes = await getRooms();
                    setRooms(roomsRes);
                    showAlert(
                      lang === "zh" ? "修改成功" : "Updated Successfully",
                      lang === "zh" ? "自定义监控配置更新成功！" : "Custom room configurations updated!",
                      "success"
                    );
                  } catch (err) {
                    showAlert(
                      lang === "zh" ? "修改失败" : "Update Failed",
                      `${lang === "zh" ? "无法修改配置" : "Failed to update room configurations"}: ${err}`,
                      "error"
                    );
                  }
                  setRoomConfigModal(prev => ({ ...prev, show: false }));
                }}
              >
                {t("btn_save_config", lang)}
              </Button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
