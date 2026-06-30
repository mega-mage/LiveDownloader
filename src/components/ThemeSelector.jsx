import React from "react";
import { cn } from "@/lib/utils";
import { Palette, Shuffle, Save } from "lucide-react";
import { t } from "../lib/i18n.js";

const THEMES = [
  { id: "dark", bgClass: "bg-slate-950", borderClass: "border-blue-500", dotClass: "bg-blue-500" },
  { id: "cyberpunk", bgClass: "bg-[#0a0314]", borderClass: "border-[#ff0055]", dotClass: "bg-[#00ffcc]" },
  { id: "sakura", bgClass: "bg-[#fff5f7]", borderClass: "border-[#ff8da1]", dotClass: "bg-[#d01c51]" },
  { id: "light", bgClass: "bg-slate-100", borderClass: "border-slate-800", dotClass: "bg-slate-800" },
  { id: "forest", bgClass: "bg-[#07120e]", borderClass: "border-[#10b981]", dotClass: "bg-[#84cc16]" }
];

const getThemeName = (id, lang) => {
  const map = {
    dark: { zh: "暗黑极客", en: "Sleek Dark" },
    cyberpunk: { zh: "赛博朋克", en: "Cyberpunk" },
    sakura: { zh: "粉嫩樱花", en: "Sakura" },
    light: { zh: "极简石蓝", en: "Slate Light" },
    forest: { zh: "森林之息", en: "Forest" }
  };
  return map[id]?.[lang === "zh" ? "zh" : "en"] || map[id]?.zh || id;
};

export function ThemeSelector({ currentTheme, onChangeTheme, onShuffle, onSaveCustom, hasSavedCustom, lang }) {
  return (
    <div className="flex flex-col gap-2 p-4 border-t border-border mt-auto">
      <div className="flex items-center gap-2 text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1">
        <Palette size={14} />
        <span>{t("sys_theme", lang)}</span>
      </div>

      {/* Theme Dots Grid: 5 presets + 1 saved custom */}
      <div className="grid grid-cols-6 gap-1.5">
        {THEMES.map((theme) => {
          const isActive = currentTheme === theme.id;
          const themeTitle = getThemeName(theme.id, lang);
          return (
            <button
              key={theme.id}
              onClick={() => onChangeTheme(theme.id)}
              title={themeTitle}
              className={cn(
                "group relative flex h-10 w-full items-center justify-center rounded-lg border-2 transition-all duration-300 cursor-pointer hover:scale-105 active:scale-95 shadow-sm",
                theme.bgClass,
                isActive ? theme.borderClass : "border-transparent opacity-65 hover:opacity-100"
              )}
            >
              <span className={cn("h-3 w-3 rounded-full shadow-inner transition-transform duration-300 group-hover:scale-110", theme.dotClass)} />
              {isActive && (
                <span className="absolute -bottom-1 -right-1 flex h-2 w-2">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-75"></span>
                  <span className="relative inline-flex rounded-full h-2 w-2 bg-primary"></span>
                </span>
              )}
            </button>
          );
        })}

        {/* 6th dot: Saved Custom Theme */}
        <button
          onClick={() => { if (hasSavedCustom) onChangeTheme("savedCustom"); }}
          title={t("custom_theme", lang)}
          disabled={!hasSavedCustom}
          className={cn(
            "group relative flex h-10 w-full items-center justify-center rounded-lg border-2 transition-all duration-300 shadow-sm",
            hasSavedCustom ? "cursor-pointer hover:scale-105 active:scale-95" : "cursor-not-allowed opacity-30",
            currentTheme === "savedCustom" ? "border-primary" : "border-transparent opacity-65 hover:opacity-100",
            !hasSavedCustom && "!opacity-30"
          )}
          style={hasSavedCustom
            ? { background: "linear-gradient(135deg, #8b5cf6 0%, #ec4899 50%, #f59e0b 100%)" }
            : { background: "var(--secondary)" }
          }
        >
          {hasSavedCustom ? (
            <span className="h-3 w-3 rounded-full bg-white/80 shadow-inner transition-transform duration-300 group-hover:scale-110" />
          ) : (
            <span className="h-3 w-3 rounded-full border border-dashed border-muted-foreground/50" />
          )}
          {currentTheme === "savedCustom" && (
            <span className="absolute -bottom-1 -right-1 flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-primary"></span>
            </span>
          )}
        </button>
      </div>

      {/* Shuffle & Save buttons row */}
      <div className="flex items-center gap-1.5 mt-1.5">
        {/* Shuffle Button */}
        <button
          onClick={onShuffle}
          className={cn(
            "flex-1 flex items-center justify-center gap-1.5 h-9 rounded-lg border text-xs font-semibold transition-all duration-300 cursor-pointer",
            "hover:scale-[1.02] active:scale-[0.98]",
            currentTheme === "custom"
              ? "border-primary/50 bg-primary/10 text-primary shadow-sm shadow-primary/10"
              : "border-border bg-secondary/40 text-muted-foreground hover:text-foreground hover:border-primary/30 hover:bg-secondary/70"
          )}
          title={t("shuffle_theme", lang)}
        >
          <Shuffle size={13} />
          <span>{t("shuffle_theme", lang)}</span>
        </button>

        {/* Save Custom Theme Button */}
        <button
          onClick={onSaveCustom}
          disabled={currentTheme !== "custom"}
          className={cn(
            "flex items-center justify-center gap-1.5 h-9 px-3 rounded-lg border text-xs font-semibold transition-all duration-300",
            currentTheme === "custom"
              ? "border-primary/50 bg-primary/10 text-primary cursor-pointer hover:scale-[1.02] active:scale-[0.98] hover:bg-primary/20"
              : "border-border bg-secondary/30 text-muted-foreground/40 cursor-not-allowed"
          )}
          title={t("save_theme", lang)}
        >
          <Save size={13} />
          <span>{t("save_theme", lang)}</span>
        </button>
      </div>
    </div>
  );
}
