import React, { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { t } from "../lib/i18n.js";
import { Terminal, Send, Activity, ChevronRight } from "lucide-react";

// Helper to parse Rust tracing log lines
const simplifyLog = (logLine) => {
  try {
    const parts = logLine.split(" ");
    if (parts.length >= 3) {
      // Trace log format typically: 2026-06-29T12:00:00.123456Z INFO module: Message
      const timePart = parts[0];
      const levelPart = parts[1];
      const timeOnly = timePart.includes("T") ? timePart.split("T")[1].substring(0, 8) : "";
      
      const messageStartIdx = logLine.indexOf(levelPart) + levelPart.length;
      const rawMessage = logLine.substring(messageStartIdx).trim();
      
      // Remove module paths from message for readability (e.g. "LiveDownloader::engine::manager:")
      let cleanMessage = rawMessage;
      const colonIdx = rawMessage.indexOf(":");
      if (colonIdx > 0 && colonIdx < 50) {
        cleanMessage = rawMessage.substring(colonIdx + 1).trim();
      }

      return {
        time: timeOnly,
        level: levelPart,
        message: cleanMessage
      };
    }
  } catch (e) {
    // Fallback
  }
  return { time: "", level: "INFO", message: logLine };
};

export function LogViewer({
  activeTab,
  logs,
  terminalLogs,
  cmdInput,
  setCmdInput,
  handleRunCommand,
  lang
}) {
  const consoleWrapperRef = useRef(null);
  const terminalWrapperRef = useRef(null);

  // Auto-scroll to bottom of logs when they update
  useEffect(() => {
    if (consoleWrapperRef.current) {
      consoleWrapperRef.current.scrollTop = consoleWrapperRef.current.scrollHeight;
    }
  }, [logs]);

  // Auto-scroll to bottom of command terminal when output updates
  useEffect(() => {
    if (terminalWrapperRef.current) {
      terminalWrapperRef.current.scrollTop = terminalWrapperRef.current.scrollHeight;
    }
  }, [terminalLogs]);

  if (activeTab !== "logs") return null;

  return (
    <div className="flex flex-col flex-1 h-[calc(100vh-8rem)] md:h-[calc(100vh-5rem)] border border-border bg-card/45 backdrop-blur-md rounded-xl overflow-hidden animate-slide-in">
      {/* 1. RUNNING LOGS (60% Height) */}
      <div className="h-[60%] flex flex-col border-b border-border">
        {/* Logs Header */}
        <div className="flex items-center gap-2 px-5 py-3 border-b border-border/50 bg-secondary/15">
          <Terminal size={14} className="text-primary" />
          <h3 className="text-xs font-bold text-foreground">{t("logs_title", lang)}</h3>
          <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse"></span>
          <span className="text-[10px] text-muted-foreground">Streaming</span>
        </div>
        
        {/* Logs Terminal Body */}
        <div 
          ref={consoleWrapperRef}
          className="flex-1 overflow-y-auto p-4 px-5 space-y-1.5 font-mono text-[11px] leading-relaxed bg-black/35"
        >
          {logs.length === 0 ? (
            <div className="text-muted-foreground italic">{t("waiting_for_logs", lang)}</div>
          ) : (
            logs.map((log, index) => {
              const parsed = simplifyLog(log);
              const isError = parsed.level === "ERROR";
              const isWarn = parsed.level === "WARN";
              
              return (
                <div 
                  key={index}
                  className={cn(
                    "flex items-start gap-3 transition-colors hover:bg-white/5 py-0.5 px-1.5 rounded",
                    isError && "text-rose-400 bg-rose-500/5",
                    isWarn && "text-amber-400 bg-amber-500/5"
                  )}
                >
                  <span className="w-8 shrink-0 text-right text-muted-foreground select-none">{index + 1}</span>
                  <span className="text-gray-500 shrink-0">[{parsed.time || "LOG"}]</span>
                  <span className={cn(
                    "w-12 shrink-0 font-bold",
                    isError ? "text-rose-500" : isWarn ? "text-amber-500" : "text-purple-400"
                  )}>
                    {parsed.level}
                  </span>
                  <span className="break-all flex-1 text-foreground/90">{parsed.message}</span>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* 2. INTERACTIVE TERMINAL (40% Height) */}
      <div className="h-[40%] flex flex-col bg-black/45">
        {/* Terminal Header */}
        <div className="flex items-center gap-2 px-5 py-2.5 border-b border-border/40 bg-black/20">
          <ChevronRight size={14} className="text-primary" />
          <h3 className="text-xs font-bold text-primary-foreground">{t("cli_console_title", lang)}</h3>
          <span className="text-[10px] text-muted-foreground">{t("cli_console_sub", lang)}</span>
        </div>

        {/* Terminal logs list */}
        <div 
          ref={terminalWrapperRef}
          className="flex-1 overflow-y-auto p-4 px-5 space-y-1 font-mono text-xs text-foreground/80 bg-black/20"
        >
          {terminalLogs.map((log, index) => {
            const isInput = log.startsWith("ld >");
            const isError = log.startsWith("错误") || log.startsWith("Error");
            return (
              <div 
                key={index} 
                className={cn(
                  "break-all whitespace-pre-wrap leading-relaxed",
                  isInput && "text-primary font-bold",
                  isError && "text-rose-400 bg-rose-500/5 px-1 rounded"
                )}
              >
                {log}
              </div>
            );
          })}
        </div>

        {/* Terminal Command Input Form */}
        <form 
          onSubmit={handleRunCommand}
          className="flex items-center gap-3 px-5 py-3 border-t border-border bg-black/35"
        >
          <span className="font-mono font-bold text-primary select-none text-sm shrink-0">ld &gt;</span>
          <input
            type="text"
            value={cmdInput}
            onChange={(e) => setCmdInput(e.target.value)}
            placeholder={t("cli_input_placeholder", lang)}
            className="flex-1 bg-transparent border-none outline-none text-foreground/90 font-mono text-xs p-0 focus:ring-0"
          />
          <Button 
            type="submit"
            size="sm"
            variant="secondary"
            className="h-7 text-xxs font-mono shrink-0 cursor-pointer"
          >
            <Send size={10} className="mr-1" />
            {t("btn_execute", lang)}
          </Button>
        </form>
      </div>
    </div>
  );
}
