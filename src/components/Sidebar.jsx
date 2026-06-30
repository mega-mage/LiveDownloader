import React, { useState } from "react";
import { cn } from "@/lib/utils";
import { ThemeSelector } from "./ThemeSelector";
import { isWebMode, getApiBaseUrl } from "@/services/api";
import { t } from "../lib/i18n.js";
import { 
  LayoutDashboard, 
  UserPlus, 
  Video, 
  Settings, 
  Terminal, 
  Menu, 
  X, 
  Radio, 
  Server,
  MonitorCheck
} from "lucide-react";

const getNavItems = (lang) => [
  { id: "dashboard", label: t("tab_dashboard", lang), icon: LayoutDashboard, desc: lang === "zh" ? "实时监控开播状态与录制输出详情" : "Monitor live streams and check recording outputs" },
  { id: "add", label: t("tab_add", lang), icon: UserPlus, desc: lang === "zh" ? "将新的主播直播间加入监控列表" : "Add new anchors to background monitoring list" },
  { id: "videos", label: t("tab_videos", lang), icon: Video, desc: lang === "zh" ? "浏览与重播已录制的直播回放" : "Browse and play recorded video segments" },
  { id: "settings", label: t("tab_settings", lang), icon: Settings, desc: lang === "zh" ? "管理保存路径、网络代理及抓取 Cookie" : "Manage folder paths, proxies, and crawler cookies" },
  { id: "logs", label: t("tab_logs", lang), icon: Terminal, desc: lang === "zh" ? "从后台引擎实时获取控制台日志信息" : "Fetch and trace engine logs in real time" }
];

export function Sidebar({ activeTab, onChangeTab, currentTheme, onChangeTheme, onShuffle, onSaveCustom, hasSavedCustom, isEnginePaused, lang, toggleLanguage }) {
  const [isMobileOpen, setIsMobileOpen] = useState(false);
  const isWeb = isWebMode();
  const apiBase = getApiBaseUrl();

  const handleNavClick = (id) => {
    onChangeTab(id);
    setIsMobileOpen(false);
  };

  const navItems = getNavItems(lang);

  const SidebarContent = () => (
    <div className="flex flex-col h-full bg-card text-card-foreground">
      {/* Brand Logo & Header */}
      <div className="flex items-center gap-3 px-6 py-5 border-b border-border">
        <div className="relative flex items-center justify-center h-10 w-10 rounded-xl bg-primary shadow-lg shadow-primary/20">
          <Radio className="text-white animate-pulse" size={20} />
          <span className="absolute -top-1 -right-1 flex h-3 w-3">
            <span className={cn(
              "animate-ping absolute inline-flex h-full w-full rounded-full opacity-75",
              isEnginePaused ? "bg-amber-400" : "bg-emerald-400"
            )}></span>
            <span className={cn(
              "relative inline-flex rounded-full h-3 w-3",
              isEnginePaused ? "bg-amber-500" : "bg-emerald-500"
            )}></span>
          </span>
        </div>
        <div className="flex flex-col">
          <h1 className="font-bold text-base leading-none tracking-tight">LiveDownloader</h1>
        </div>
      </div>

      {/* Connection Status Indicator */}
      <div className="px-4 py-3 mx-4 my-3 rounded-lg bg-secondary/40 border border-border/60 flex items-center gap-2.5">
        <div className="flex items-center justify-center h-7 w-7 rounded-md bg-card border border-border text-primary">
          {isWeb ? <Server size={14} /> : <MonitorCheck size={14} />}
        </div>
        <div className="flex flex-col min-w-0">
          <span className="text-xxs text-muted-foreground leading-none">{t("run_mode", lang)}</span>
          <span className="text-xs font-semibold truncate mt-0.5">
            {isWeb ? `${t("web_mode", lang)} (API: ${apiBase ? apiBase.replace(/https?:\/\//, '') : t("api_not_configured", lang)})` : `${t("desktop_mode", lang)} (Tauri)`}
          </span>
        </div>
      </div>

      {/* Navigation Links */}
      <nav className="flex-1 px-3 py-3 space-y-1 overflow-y-auto">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => handleNavClick(item.id)}
              className={cn(
                "flex items-center gap-3 w-full px-4 py-3 rounded-lg text-sm font-medium transition-all duration-200 cursor-pointer",
                isActive 
                  ? "bg-primary text-primary-foreground shadow-md shadow-primary/10" 
                  : "hover:bg-secondary/65 text-muted-foreground hover:text-foreground"
              )}
            >
              <Icon size={18} className={cn("transition-transform duration-300 group-hover:scale-110", isActive ? "text-white" : "text-muted-foreground")} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      {/* Theme Toggler at Bottom */}
      <ThemeSelector currentTheme={currentTheme} onChangeTheme={onChangeTheme} onShuffle={onShuffle} onSaveCustom={onSaveCustom} hasSavedCustom={hasSavedCustom} lang={lang} />
    </div>
  );

  return (
    <>
      {/* === Desktop Sidebar view === */}
      <aside className="hidden md:flex flex-col w-[260px] h-full border-r border-border bg-card shrink-0">
        <SidebarContent />
      </aside>

      {/* === Mobile Header view === */}
      <header className="flex md:hidden items-center justify-between px-4 h-14 border-b border-border bg-card w-full fixed top-0 left-0 z-40">
        <div className="flex items-center gap-2">
          <div className="h-8 w-8 rounded-lg bg-primary flex items-center justify-center text-white">
            <Radio size={16} className="animate-pulse" />
          </div>
          <span className="font-bold text-sm tracking-tight">LiveDownloader</span>
        </div>
        <div className="flex items-center gap-2.5">
          {/* Mobile Language Switcher */}
          <button 
            onClick={toggleLanguage}
            className="h-8 px-2.5 rounded-md border border-border flex items-center justify-center text-xxs font-bold text-foreground hover:bg-secondary cursor-pointer"
            title={lang === "zh" ? "Switch to English" : "切换为中文"}
          >
            {lang === "zh" ? "EN" : "中"}
          </button>
          <button
            onClick={() => setIsMobileOpen(true)}
            className={cn(
              "h-9 w-9 rounded-md border border-border flex items-center justify-center text-foreground hover:bg-secondary cursor-pointer transition-all",
              !isWeb && "mr-48"
            )}
          >
            <Menu size={18} />
          </button>
        </div>
      </header>

      {/* === Mobile Drawer Sheet overlay === */}
      {isMobileOpen && (
        <div className="md:hidden fixed inset-0 z-50 flex">
          {/* Overlay Background */}
          <div 
            className="fixed inset-0 bg-black/60 backdrop-blur-xs transition-opacity duration-300"
            onClick={() => setIsMobileOpen(false)}
          />
          {/* Slider Menu Body */}
          <div className="relative flex flex-col w-[280px] max-w-sm h-full bg-card border-r border-border animate-slide-in shadow-2xl z-50">
            {/* Close Button */}
            <button
              onClick={() => setIsMobileOpen(false)}
              className="absolute top-4 right-4 h-8 w-8 rounded-full bg-secondary flex items-center justify-center text-foreground cursor-pointer hover:opacity-80"
            >
              <X size={16} />
            </button>
            <SidebarContent />
          </div>
        </div>
      )}
    </>
  );
}
