import React, { useState } from "react";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "./ui/card";
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from "./ui/table";
import { Input } from "./ui/input";
import { Select } from "./ui/select";
import { isWebMode, getDownloadLink } from "@/services/api";
import { t } from "../lib/i18n.js";
import { 
  FolderOpen, 
  Search, 
  Download, 
  Trash2, 
  Play, 
  Folder,
  FileVideo
} from "lucide-react";

const formatBytes = (bytes) => {
  if (!bytes || bytes === 0) return "0 Bytes";
  const k = 1024;
  const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
};

export function VideoSection({
  recordedVideos,
  proxyPort,
  handleOpenFolder,
  handleDeleteVideo,
  setActivePlayUrl,
  setActivePlayTitle,
  showAlert,
  lang
}) {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedAnchor, setSelectedAnchor] = useState("all");
  const isWeb = isWebMode();

  // Extract unique anchors that have recorded videos
  const uniqueAnchors = Array.from(
    new Set(recordedVideos.map(vid => vid.anchor).filter(Boolean))
  ).sort();

  const filteredVideos = recordedVideos.filter(vid => {
    const matchesAnchor = selectedAnchor === "all" || vid.anchor === selectedAnchor;
    const query = searchQuery.toLowerCase().trim();
    const matchesQuery = !query || 
      (vid.name || "").toLowerCase().includes(query) ||
      (vid.anchor || "").toLowerCase().includes(query);
    return matchesAnchor && matchesQuery;
  });

  return (
    <div className="space-y-5 animate-slide-in">
      {/* Header with Search & Dropdown Filter */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <h2 className="text-lg font-bold text-foreground shrink-0">{t("recorded_videos_title", lang)}</h2>
        
        {/* Search controls container */}
        <div className="flex flex-col sm:flex-row items-center gap-2 max-w-md w-full">
          {/* Direct Anchor Dropdown Filter */}
          <Select
            value={selectedAnchor}
            onChange={(e) => setSelectedAnchor(e.target.value)}
            className="w-full sm:w-[150px] shrink-0 font-medium text-xs h-9"
          >
            <option value="all">{lang === "zh" ? "全部主播" : "All Anchors"}</option>
            {uniqueAnchors.map(name => (
              <option key={name} value={name}>{name}</option>
            ))}
          </Select>
          
          {/* Text input search */}
          <div className="relative flex-1 w-full">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder={t("search_video_placeholder", lang)}
              className="pl-9 w-full h-9 text-xs"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>
        </div>
      </div>

      {filteredVideos.length === 0 ? (
        <div className="flex flex-col items-center justify-center text-center p-8 py-16 rounded-xl border border-dashed border-border bg-card/35 max-w-lg mx-auto mt-10">
          <div className="h-16 w-16 rounded-full bg-secondary flex items-center justify-center mb-5 text-muted-foreground">
            <FolderOpen size={28} />
          </div>
          <h3 className="text-lg font-bold text-foreground">
            {lang === "zh" ? "无符合条件的视频回放" : "No matching video playbacks found"}
          </h3>
          <p className="text-sm text-muted-foreground mt-2 max-w-xs">
            {t("no_recorded_videos", lang)}
          </p>
        </div>
      ) : (
        <>
          {/* 1. Large viewport layout (Table view) */}
          <div className="hidden lg:block rounded-xl border border-border bg-card/30 overflow-hidden">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[120px]">{t("tbl_video_anchor", lang)}</TableHead>
                  <TableHead className="w-[380px]">{t("tbl_video_name", lang)}</TableHead>
                  <TableHead className="w-[100px]">{t("tbl_video_size", lang)}</TableHead>
                  <TableHead className="w-[180px]">{t("tbl_video_date", lang)}</TableHead>
                  <TableHead className="text-right w-[180px]">{t("tbl_actions", lang)}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredVideos.map((vid, idx) => (
                  <TableRow key={idx}>
                    <TableCell>
                      <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xxs font-semibold bg-primary/10 border border-primary/20 text-primary uppercase">
                        {vid.anchor}
                      </span>
                    </TableCell>
                    <TableCell className="max-w-[380px] font-medium text-foreground truncate" title={vid.name}>
                      {vid.name}
                    </TableCell>
                    <TableCell className="text-muted-foreground text-xs font-mono">
                      {formatBytes(vid.size)}
                    </TableCell>
                    <TableCell className="text-muted-foreground text-xs">
                      {vid.modified}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1.5">
                        {isWeb ? (
                          <>
                            <Button
                              size="sm"
                              className="h-8 px-2.5 bg-primary text-primary-foreground font-semibold text-xs animate-none"
                              title={lang === "zh" ? "下载视频到本地" : "Download video to local"}
                              onClick={async () => {
                                try {
                                  const url = await getDownloadLink(vid.path);
                                  const a = document.createElement("a");
                                  a.href = url;
                                  a.download = vid.name;
                                  document.body.appendChild(a);
                                  a.click();
                                  document.body.removeChild(a);
                                } catch (err) {
                                  showAlert(lang === "zh" ? "获取下载链接失败" : "Failed to get download link", err.message || err, "error");
                                }
                              }}
                            >
                              <Download size={12} className="mr-1" />
                              {t("btn_download", lang)}
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              className="h-8 px-2.5 text-xs text-destructive border-destructive/25 bg-destructive/5 hover:bg-destructive/15 animate-none"
                              onClick={() => handleDeleteVideo(vid.path, vid.name)}
                            >
                              <Trash2 size={12} />
                            </Button>
                          </>
                        ) : (
                          <>
                            <Button
                              size="sm"
                              className="h-8 px-2.5 text-xs animate-none"
                              title={lang === "zh" ? "在程序内观看放映" : "Play in local player"}
                              onClick={() => {
                                const isTs = vid.name.toLowerCase().endsWith(".ts");
                                const videoUrl = proxyPort
                                  ? (isTs
                                    ? `http://127.0.0.1:${proxyPort}/video?playlist=true&path=${encodeURIComponent(vid.path)}`
                                    : `http://127.0.0.1:${proxyPort}/video?path=${encodeURIComponent(vid.path)}`)
                                  : "";
                                if (videoUrl) {
                                  setActivePlayUrl(videoUrl);
                                  setActivePlayTitle(vid.name);
                                }
                              }}
                            >
                              <Play size={12} className="mr-1 fill-white" />
                              {t("btn_play", lang)}
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              className="h-8 px-2.5 text-xs animate-none"
                              title={lang === "zh" ? "打开所在文件夹" : "Open containing folder"}
                              onClick={() => handleOpenFolder(vid.path)}
                            >
                              <Folder size={12} className="mr-1" />
                              {lang === "zh" ? "定位" : "Locate"}
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              className="h-8 px-2.5 text-xs text-destructive border-destructive/25 bg-destructive/5 hover:bg-destructive/15 animate-none"
                              onClick={() => handleDeleteVideo(vid.path, vid.name)}
                            >
                              <Trash2 size={12} />
                            </Button>
                          </>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>

          {/* 2. Small viewport layout (Cards view) */}
          <div className="grid grid-cols-1 md:grid-cols-2 lg:hidden gap-4">
            {filteredVideos.map((vid, idx) => (
              <Card key={idx} className="border border-border/80 bg-card/45 backdrop-blur-md overflow-hidden animate-none">
                <CardHeader className="flex flex-row items-center justify-between pb-2 p-5 border-b border-border/30 bg-secondary/15">
                  <div className="flex items-center gap-2">
                    <FileVideo size={16} className="text-primary" />
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xxs font-bold bg-primary/10 border border-primary/20 text-primary uppercase">
                      {vid.anchor}
                    </span>
                  </div>
                  <span className="text-xs font-mono text-muted-foreground">
                    {formatBytes(vid.size)}
                  </span>
                </CardHeader>
                <CardContent className="p-5 py-4 space-y-2">
                  <div className="flex flex-col gap-0.5">
                    <span className="text-[10px] text-muted-foreground uppercase font-bold">{t("tbl_video_name", lang)}</span>
                    <span className="text-xs text-foreground font-semibold line-clamp-2 mt-1" title={vid.name}>
                      {vid.name}
                    </span>
                  </div>
                  <div className="flex flex-col gap-0.5">
                    <span className="text-[10px] text-muted-foreground uppercase font-bold">{t("tbl_video_date", lang)}</span>
                    <span className="text-xs text-foreground/80 font-medium mt-1">
                      {vid.modified}
                    </span>
                  </div>
                </CardContent>
                <CardFooter className="flex items-center justify-end gap-1.5 p-4 border-t border-border/40 bg-secondary/10">
                  {isWeb ? (
                    <>
                      <Button
                        size="sm"
                        className="h-8 px-3 bg-primary text-primary-foreground font-bold text-xs"
                        onClick={async () => {
                          try {
                            const url = await getDownloadLink(vid.path);
                            const a = document.createElement("a");
                            a.href = url;
                            a.download = vid.name;
                            document.body.appendChild(a);
                            a.click();
                            document.body.removeChild(a);
                          } catch (err) {
                            showAlert(lang === "zh" ? "获取下载链接失败" : "Failed to get download link", err.message || err, "error");
                          }
                        }}
                      >
                        <Download size={12} className="mr-1" />
                        {t("btn_download", lang)}
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-8 px-3 text-xs text-destructive border-destructive/25 bg-destructive/5 hover:bg-destructive/15"
                        onClick={() => handleDeleteVideo(vid.path, vid.name)}
                      >
                        <Trash2 size={12} className="mr-1" />
                        {t("btn_delete", lang)}
                      </Button>
                    </>
                  ) : (
                    <>
                      <Button
                        size="sm"
                        className="h-8 px-3 text-xs"
                        onClick={() => {
                          const isTs = vid.name.toLowerCase().endsWith(".ts");
                          const videoUrl = proxyPort
                            ? (isTs
                              ? `http://127.0.0.1:${proxyPort}/video?playlist=true&path=${encodeURIComponent(vid.path)}`
                              : `http://127.0.0.1:${proxyPort}/video?path=${encodeURIComponent(vid.path)}`)
                            : "";
                          if (videoUrl) {
                            setActivePlayUrl(videoUrl);
                            setActivePlayTitle(vid.name);
                          }
                        }}
                      >
                        <Play size={12} className="mr-1 fill-white" />
                        {t("btn_play", lang)}
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-8 px-3 text-xs"
                        onClick={() => handleOpenFolder(vid.path)}
                      >
                        <Folder size={12} className="mr-1" />
                        {lang === "zh" ? "定位" : "Locate"}
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-8 px-3 text-xs text-destructive border-destructive/25 bg-destructive/5 hover:bg-destructive/15"
                        onClick={() => handleDeleteVideo(vid.path, vid.name)}
                      >
                        <Trash2 size={12} className="mr-1" />
                        {t("btn_delete", lang)}
                      </Button>
                    </>
                  )}
                </CardFooter>
              </Card>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
