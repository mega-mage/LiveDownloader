import React, { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { executeLdCommand } from "../services/api.js";

export function XtermConsole() {
  const containerRef = useRef(null);
  const termRef = useRef(null);
  const fitAddonRef = useRef(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontSize: 13,
      fontFamily: 'Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
      theme: {
        background: "#090d16",
        foreground: "#e2e8f0",
        cursor: "#10b981",
        cursorAccent: "#000000",
        selectionBackground: "rgba(16, 185, 129, 0.3)",
        black: "#1e293b",
        red: "#f43f5e",
        green: "#10b981",
        yellow: "#f59e0b",
        blue: "#3b82f6",
        magenta: "#d946ef",
        cyan: "#06b6d4",
        white: "#f8fafc",
        brightBlack: "#475569",
        brightRed: "#fb7185",
        brightGreen: "#34d399",
        brightYellow: "#fbbf24",
        brightBlue: "#60a5fa",
        brightMagenta: "#e879f9",
        brightCyan: "#22d3ee",
        brightWhite: "#ffffff",
      },
      convertEol: true,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(containerRef.current);
    
    // Initial delay fit to ensure parent DOM layout is rendered
    setTimeout(() => {
      try {
        fitAddon.fit();
      } catch (e) {}
    }, 50);

    termRef.current = term;
    fitAddonRef.current = fitAddon;

    // Welcome banner
    term.writeln("\x1b[1;32m┌─────────────────────────────────────────────────────────────┐\x1b[0m");
    term.writeln("\x1b[1;32m│  LiveDownloader Interactive CLI Console (xterm.js)         │\x1b[0m");
    term.writeln("\x1b[1;32m└─────────────────────────────────────────────────────────────┘\x1b[0m");
    term.writeln("\x1b[90m提示: 输入指令 (如 help, rooms, config) 并按回车执行。输入 'clear' 清屏。\x1b[0m");
    term.writeln("");

    const prompt = () => {
      term.write("\x1b[1;36mld > \x1b[0m");
    };

    prompt();

    let inputBuffer = "";
    const history = [];
    let historyIndex = -1;

    const disposable = term.onData(async (data) => {
      // 1. Enter key (\r or \n)
      if (data === "\r" || data === "\n") {
        term.writeln("");
        const cmd = inputBuffer.trim();
        if (cmd) {
          history.push(cmd);
          historyIndex = history.length;

          if (cmd.toLowerCase() === "clear" || cmd.toLowerCase() === "cls") {
            term.clear();
          } else {
            try {
              const output = await executeLdCommand(cmd);
              if (output) {
                const formatted = output.replace(/\r?\n/g, "\r\n");
                term.writeln(formatted);
              }
            } catch (err) {
              const errMsg = err?.message || String(err);
              term.writeln(`\x1b[1;31m[错误] 指令执行失败: ${errMsg}\x1b[0m`);
            }
          }
        }
        inputBuffer = "";
        prompt();
        return;
      }

      // 2. Backspace (\x7f or \b)
      if (data === "\x7f" || data === "\b") {
        if (inputBuffer.length > 0) {
          inputBuffer = inputBuffer.slice(0, -1);
          term.write("\b \b");
        }
        return;
      }

      // 3. Ctrl + C (\x03)
      if (data === "\x03") {
        term.writeln("^C");
        inputBuffer = "";
        prompt();
        return;
      }

      // 4. Ctrl + L (\x0c)
      if (data === "\x0c") {
        term.clear();
        prompt();
        term.write(inputBuffer);
        return;
      }

      // 5. Arrow Keys (\x1b[A for UP, \x1b[B for DOWN)
      if (data === "\x1b[A") {
        // UP arrow
        if (history.length > 0 && historyIndex > 0) {
          historyIndex--;
          while (inputBuffer.length > 0) {
            inputBuffer = inputBuffer.slice(0, -1);
            term.write("\b \b");
          }
          inputBuffer = history[historyIndex];
          term.write(inputBuffer);
        }
        return;
      }

      if (data === "\x1b[B") {
        // DOWN arrow
        if (history.length > 0 && historyIndex < history.length - 1) {
          historyIndex++;
          while (inputBuffer.length > 0) {
            inputBuffer = inputBuffer.slice(0, -1);
            term.write("\b \b");
          }
          inputBuffer = history[historyIndex];
          term.write(inputBuffer);
        } else if (historyIndex === history.length - 1) {
          historyIndex = history.length;
          while (inputBuffer.length > 0) {
            inputBuffer = inputBuffer.slice(0, -1);
            term.write("\b \b");
          }
          inputBuffer = "";
        }
        return;
      }

      // 6. Normal printable character input
      if (data.length === 1 && data.charCodeAt(0) >= 32) {
        inputBuffer += data;
        term.write(data);
      }
    });

    // Resize listener with ResizeObserver
    const resizeObserver = new ResizeObserver(() => {
      try {
        fitAddon.fit();
      } catch (e) {}
    });

    if (containerRef.current) {
      resizeObserver.observe(containerRef.current);
    }

    return () => {
      disposable.dispose();
      resizeObserver.disconnect();
      term.dispose();
    };
  }, []);

  return (
    <div className="w-full h-full min-h-[180px] bg-[#090d16] overflow-hidden flex flex-col p-2">
      <div ref={containerRef} className="w-full flex-1 overflow-hidden" />
    </div>
  );
}
