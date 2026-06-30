import React, { useState } from "react";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "./ui/card";
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from "./ui/table";
import { Input } from "./ui/input";
import { Select } from "./ui/select";
import { isWebMode, getStreamProxyUrl } from "@/services/api";
import { t } from "../lib/i18n.js";
import { 
  Tv, 
  PlusCircle, 
  Play, 
  PauseCircle, 
  Moon, 
  Sliders, 
  Trash2, 
  Search, 
  Check, 
  AlertCircle, 
  Info,
  Radio
} from "lucide-react";

export function RoomSection({
  activeTab,
  rooms,
  proxyPort,
  config,
  newUrl,
  setNewUrl,
  newAlias,
  setNewAlias,
  newQuality,
  setNewQuality,
  statusMsg,
  handleAddRoom,
  handleToggleRoomPaused,
  handleDeleteRoom,
  setRoomConfigModal,
  setActivePlayUrl,
  setActivePlayTitle,
  setActiveTab,
  lang
}) {
  const [searchQuery, setSearchQuery] = useState("");

  const filteredRooms = rooms.filter(room => {
    const query = searchQuery.toLowerCase().trim();
    if (!query) return true;
    return (
      (room.anchor_name || "").toLowerCase().includes(query) ||
      (room.title || "").toLowerCase().includes(query) ||
      (room.platform || "").toLowerCase().includes(query)
    );
  });

  if (activeTab === "dashboard") {
    if (rooms.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center text-center p-8 py-16 rounded-xl border border-dashed border-border bg-card/35 max-w-lg mx-auto mt-10 animate-slide-in">
          <div className="h-16 w-16 rounded-full bg-secondary flex items-center justify-center mb-5 text-muted-foreground">
            <Tv size={28} />
          </div>
          <h3 className="text-lg font-bold text-foreground">
            {lang === "zh" ? "当前无监控主播" : "No Monitored Anchors"}
          </h3>
          <p className="text-sm text-muted-foreground mt-2 max-w-sm">
            {lang === "zh" 
              ? "您还没有添加任何直播间进行录制监控。点击下方按钮添加第一个监控任务吧！" 
              : "You haven't added any live rooms for recording. Click the button below to add your first monitor task!"}
          </p>
          <Button className="mt-6 flex items-center gap-2" onClick={() => setActiveTab("add")}>
            <PlusCircle size={16} />
            <span>{t("add_room_btn", lang)}</span>
          </Button>
        </div>
      );
    }

    return (
      <div className="space-y-5 animate-slide-in">
        {/* Search bar and info header */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div className="relative max-w-xs w-full">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder={t("search_placeholder", lang)}
              className="pl-9"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>
          <span className="text-xs text-muted-foreground">
            {lang === "zh" 
              ? `共监控 ${rooms.length} 个主播，正在录制 ${rooms.filter(r => r.status === "Living").length} 个` 
              : `Monitoring ${rooms.length} rooms, ${rooms.filter(r => r.status === "Living").length} actively recording`}
          </span>
        </div>

        {/* --- Responsive Switch layouts --- */}

        {/* 1. Large viewport layout (Table view) */}
        <div className="hidden lg:block rounded-xl border border-border bg-card/30 overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[180px]">{t("tbl_anchor", lang)}</TableHead>
                <TableHead className="w-[100px]">{t("tbl_platform", lang)}</TableHead>
                <TableHead className="w-[280px]">{t("tbl_title", lang)}</TableHead>
                <TableHead className="w-[120px]">{t("tbl_status", lang)}</TableHead>
                <TableHead className="text-right w-[200px]">{t("tbl_actions", lang)}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredRooms.map((room) => (
                <TableRow key={room.url} className="group">
                  <TableCell className="font-semibold text-foreground flex items-center gap-2">
                    <div className="h-8 w-8 rounded-full bg-primary/10 border border-primary/25 text-primary flex items-center justify-center font-bold text-xs uppercase">
                      {(room.anchor_name || "?").charAt(0)}
                    </div>
                    <span>{room.anchor_name || (lang === "zh" ? "未知主播" : "Unknown Anchor")}</span>
                  </TableCell>
                  <TableCell>
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xxs font-medium bg-secondary text-foreground border border-border">
                      {room.platform}
                    </span>
                  </TableCell>
                  <TableCell className="max-w-[280px] truncate">
                    {room.status === "Living" ? (
                      <div className="flex flex-col">
                        <span className="text-xs font-medium text-foreground truncate">{room.title || (lang === "zh" ? "无标题" : "No Title")}</span>
                        <span className="text-[10px] text-muted-foreground font-mono mt-0.5 truncate" title={room.record_path}>
                          {room.record_path ? room.record_path.split(/[\\/]/).pop() : (lang === "zh" ? "未知" : "Unknown")}
                        </span>
                      </div>
                    ) : (
                      <span className="text-xs text-muted-foreground italic">{lang === "zh" ? "主播已离线 / 未开播" : "Offline / Idle"}</span>
                    )}
                  </TableCell>
                  <TableCell>
                    {room.status === "Living" ? (
                      <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-semibold bg-emerald-500/15 border border-emerald-500/30 text-emerald-400">
                        <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                        {lang === "zh" ? "录制中" : "Recording"}
                      </span>
                    ) : room.status === "Idle" ? (
                      <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-semibold bg-blue-500/10 border border-blue-500/20 text-blue-400">
                        {lang === "zh" ? "监听中" : "Listening"}
                      </span>
                    ) : room.status === "Paused" ? (
                      <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-semibold bg-amber-500/10 border border-amber-500/20 text-amber-400">
                        {lang === "zh" ? "已挂起" : "Paused"}
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-semibold bg-rose-500/10 border border-rose-500/20 text-rose-400">
                        {lang === "zh" ? "错误" : "Error"}
                      </span>
                    )}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex items-center justify-end gap-1.5">
                      {room.status === "Living" && room.live_url && (
                        <Button
                          size="sm"
                          variant="default"
                          className="h-8 bg-emerald-600 hover:bg-emerald-500 text-white font-medium text-xs px-2.5"
                          onClick={() => {
                            const proxiedUrl = proxyPort
                              ? getStreamProxyUrl(room.live_url, room.url, proxyPort)
                              : room.live_url;
                            setActivePlayUrl(proxiedUrl);
                            setActivePlayTitle(room.anchor_name);
                          }}
                        >
                          <Play size={12} className="mr-1 fill-white" />
                          {t("btn_watch", lang)}
                        </Button>
                      )}
                      {room.status === "Paused" ? (
                        <Button
                          size="sm"
                          variant="outline"
                          className="h-8 px-2.5 text-xs animate-pulse border-amber-500/40 hover:border-amber-500"
                          onClick={() => handleToggleRoomPaused(room.url, false)}
                        >
                          {t("btn_resume", lang)}
                        </Button>
                      ) : (
                        <Button
                          size="sm"
                          variant="outline"
                          className="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
                          onClick={() => handleToggleRoomPaused(room.url, true)}
                        >
                          {t("btn_pause", lang)}
                        </Button>
                      )}
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-8 px-2.5 text-xs animate-none"
                        onClick={() => {
                          const matchedRoom = config?.rooms?.find(r => r.url === room.url);
                          setRoomConfigModal({
                            show: true,
                            url: room.url,
                            anchorName: room.anchor_name,
                            name: matchedRoom?.name || "",
                            quality: matchedRoom?.quality || "",
                            videoSaveType: matchedRoom?.video_save_type || ""
                          });
                        }}
                      >
                        <Sliders size={12} />
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-8 px-2.5 text-xs text-destructive border-destructive/25 bg-destructive/5 hover:bg-destructive/15 animate-none"
                        onClick={() => handleDeleteRoom(room.url)}
                      >
                        <Trash2 size={12} />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>

        {/* 2. Small viewport layout (Cards view) */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:hidden gap-4">
          {filteredRooms.map((room) => (
            <Card key={room.url} className={cn(
              "border border-border/80 bg-card/45 backdrop-blur-md overflow-hidden transition-all duration-300",
              room.status === "Living" && "ring-1 ring-emerald-500/30"
            )}>
              <CardHeader className="flex flex-row items-center justify-between pb-3 p-5">
                <div className="flex items-center gap-3">
                  <div className="h-9 w-9 rounded-full bg-primary/10 border border-primary/20 text-primary flex items-center justify-center font-bold text-sm uppercase">
                    {(room.anchor_name || "?").charAt(0)}
                  </div>
                  <div className="flex flex-col">
                    <CardTitle className="text-sm font-bold text-foreground leading-none">{room.anchor_name || (lang === "zh" ? "未知主播" : "Unknown Anchor")}</CardTitle>
                    <span className="text-xxs text-muted-foreground mt-1">
                      {room.platform}
                    </span>
                  </div>
                </div>
                <div>
                  {room.status === "Living" ? (
                    <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 font-bold text-[10px] uppercase">
                      <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                      REC
                    </span>
                  ) : room.status === "Idle" ? (
                    <span className="inline-flex items-center px-2 py-0.5 rounded bg-blue-500/10 border border-blue-500/20 text-blue-400 text-[10px]">
                      {lang === "zh" ? "监听中" : "Listening"}
                    </span>
                  ) : room.status === "Paused" ? (
                    <span className="inline-flex items-center px-2 py-0.5 rounded bg-amber-500/10 border border-amber-500/20 text-amber-400 text-[10px]">
                      {lang === "zh" ? "挂起" : "Paused"}
                    </span>
                  ) : (
                    <span className="inline-flex items-center px-2 py-0.5 rounded bg-rose-500/10 border border-rose-500/20 text-rose-400 text-[10px]">
                      {lang === "zh" ? "错误" : "Error"}
                    </span>
                  )}
                </div>
              </CardHeader>
              <CardContent className="px-5 pb-4 pt-1 border-t border-border/40">
                {room.status === "Living" ? (
                  <div className="space-y-2 mt-3">
                    <div className="flex flex-col gap-0.5">
                      <span className="text-[10px] text-muted-foreground uppercase leading-none font-semibold">{lang === "zh" ? "直播标题" : "Live Title"}</span>
                      <span className="text-xs text-foreground font-medium truncate mt-1">{room.title || (lang === "zh" ? "无标题" : "No Title")}</span>
                    </div>
                    <div className="flex flex-col gap-0.5">
                      <span className="text-[10px] text-muted-foreground uppercase leading-none font-semibold">{lang === "zh" ? "录制切片" : "Segment File"}</span>
                      <span className="text-[10px] text-foreground font-mono mt-1 bg-secondary/55 p-1 px-1.5 rounded truncate" title={room.record_path}>
                        {room.record_path ? room.record_path.split(/[\\/]/).pop() : (lang === "zh" ? "暂无" : "None")}
                      </span>
                    </div>
                  </div>
                ) : room.status === "Paused" ? (
                  <div className="flex items-center gap-2 text-muted-foreground text-xs italic py-4">
                    <PauseCircle size={14} />
                    <span>{lang === "zh" ? "监控被暂停" : "Monitoring Paused"}</span>
                  </div>
                ) : (
                  <div className="flex items-center gap-2 text-muted-foreground text-xs italic py-4">
                    <Moon size={14} />
                    <span>{lang === "zh" ? "主播当前离线" : "Anchor is Offline"}</span>
                  </div>
                )}
              </CardContent>
              <CardFooter className="flex items-center justify-between p-4 px-5 border-t border-border/40 bg-secondary/20">
                <div className="flex items-center gap-1.5">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-8 px-2 text-xs"
                    onClick={() => {
                      const matchedRoom = config?.rooms?.find(r => r.url === room.url);
                      setRoomConfigModal({
                        show: true,
                        url: room.url,
                        anchorName: room.anchor_name,
                        name: matchedRoom?.name || "",
                        quality: matchedRoom?.quality || "",
                        videoSaveType: matchedRoom?.video_save_type || ""
                      });
                    }}
                  >
                    <Sliders size={12} className="mr-1" />
                    {t("btn_config", lang)}
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-8 px-2 text-xs text-destructive border-destructive/20 bg-destructive/5 hover:bg-destructive/15"
                    onClick={() => handleDeleteRoom(room.url)}
                  >
                    <Trash2 size={12} className="mr-1" />
                    {t("btn_delete", lang)}
                  </Button>
                </div>
                <div className="flex items-center gap-1.5">
                  {room.status === "Paused" ? (
                    <Button
                      size="sm"
                      variant="default"
                      className="h-8 px-3 text-xs bg-primary text-primary-foreground"
                      onClick={() => handleToggleRoomPaused(room.url, false)}
                    >
                      <Play size={12} className="mr-1 fill-white" />
                      {t("btn_resume", lang)}
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-8 px-3 text-xs"
                      onClick={() => handleToggleRoomPaused(room.url, true)}
                    >
                      {t("btn_pause", lang)}
                    </Button>
                  )}
                  {room.status === "Living" && room.live_url && (
                    <Button
                      size="sm"
                      variant="default"
                      className="h-8 px-3 text-xs bg-emerald-600 hover:bg-emerald-500 text-white font-medium"
                      onClick={() => {
                        const proxiedUrl = proxyPort
                          ? getStreamProxyUrl(room.live_url, room.url, proxyPort)
                          : room.live_url;
                        setActivePlayUrl(proxiedUrl);
                        setActivePlayTitle(room.anchor_name);
                      }}
                    >
                      <Play size={12} className="mr-1 fill-white" />
                      {t("btn_watch", lang)}
                    </Button>
                  )}
                </div>
              </CardFooter>
            </Card>
          ))}
        </div>
      </div>
    );
  }

  if (activeTab === "add") {
    return (
      <Card className="max-w-xl mx-auto border border-border bg-card/45 backdrop-blur-md shadow-lg animate-slide-in">
        <CardHeader className="p-6 pb-4 border-b border-border/50">
          <CardTitle className="text-base font-bold text-foreground">{t("add_room_btn", lang)}</CardTitle>
          <CardDescription className="text-xs text-muted-foreground mt-1">
            {lang === "zh" 
              ? "输入目标平台的直播间 URL 地址，系统将在后台全自动轮询检测开播状态并录制切片。" 
              : "Enter target livestream URLs. The backend engine polls to detect and record automatically."}
          </CardDescription>
        </CardHeader>
        <CardContent className="p-6">
          <form onSubmit={handleAddRoom} className="space-y-4">
            <div className="space-y-2">
              <label htmlFor="room-url" className="text-xs font-bold text-foreground/80">
                {t("live_room_url", lang)} *
              </label>
              <Input
                id="room-url"
                placeholder={t("live_room_url_placeholder", lang)}
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                required
              />
              <p className="text-[10px] text-muted-foreground">
                {t("live_room_url_tip", lang)}
              </p>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-2">
                <label htmlFor="room-alias" className="text-xs font-bold text-foreground/80">
                  {t("anchor_alias", lang)}
                </label>
                <Input
                  id="room-alias"
                  placeholder={t("anchor_alias_placeholder", lang)}
                  value={newAlias}
                  onChange={(e) => setNewAlias(e.target.value)}
                />
              </div>

              <div className="space-y-2">
                <label htmlFor="room-quality" className="text-xs font-bold text-foreground/80">
                  {lang === "zh" ? "指定录制画质" : "Specified Record Quality"}
                </label>
                <Select
                  id="room-quality"
                  value={newQuality}
                  onChange={(e) => setNewQuality(e.target.value)}
                >
                  <option value="原画">{lang === "zh" ? "原画 (最高画质)" : "Source (Max)"}</option>
                  <option value="超清">{lang === "zh" ? "超清" : "1080p / Ultra"}</option>
                  <option value="高清">{lang === "zh" ? "高清" : "720p / HD"}</option>
                  <option value="标清">{lang === "zh" ? "标清" : "480p / SD"}</option>
                  <option value="流畅">{lang === "zh" ? "流畅 (节省硬盘)" : "Speed (Save Space)"}</option>
                </Select>
              </div>
            </div>

            {statusMsg.text && (
              <div className={cn(
                "p-3 rounded-lg border flex items-start gap-2.5 text-xs transition-all animate-slide-in mt-3",
                statusMsg.type === "success" && "bg-emerald-500/10 border-emerald-500/30 text-emerald-400",
                statusMsg.type === "error" && "bg-rose-500/10 border-rose-500/30 text-rose-400",
                statusMsg.type === "info" && "bg-blue-500/10 border-blue-500/20 text-blue-400"
              )}>
                {statusMsg.type === "success" && <Check size={16} className="shrink-0 mt-0.5" />}
                {statusMsg.type === "error" && <AlertCircle size={16} className="shrink-0 mt-0.5" />}
                {statusMsg.type === "info" && <Info size={16} className="shrink-0 mt-0.5" />}
                <span>{statusMsg.text}</span>
              </div>
            )}

            <Button type="submit" className="w-full flex items-center justify-center gap-2 mt-6 py-2.5 font-bold">
              <PlusCircle size={16} />
              <span>{t("add_to_monitor", lang)}</span>
            </Button>
          </form>
        </CardContent>
      </Card>
    );
  }

  return null;
}
