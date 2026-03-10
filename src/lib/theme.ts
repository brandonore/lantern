import type { ITheme } from "@xterm/xterm";

export function getTerminalTheme(): ITheme {
  const style = getComputedStyle(document.documentElement);
  const get = (prop: string) => style.getPropertyValue(prop).trim();

  return {
    background: get("--bg-primary") || "#0a0f0d",
    foreground: get("--text-primary") || "#d4e8dc",
    cursor: get("--accent") || "#3ecf8e",
    cursorAccent: get("--bg-primary") || "#0a0f0d",
    selectionBackground: "rgba(62, 207, 142, 0.2)",
    selectionForeground: undefined,
    black: "#1a1a2e",
    red: "#e06c75",
    green: "#3ecf8e",
    yellow: "#f0c674",
    blue: "#61afef",
    magenta: "#c678dd",
    cyan: "#56b6c2",
    white: "#d4e8dc",
    brightBlack: "#4a6b5a",
    brightRed: "#e06c75",
    brightGreen: "#3ecf8e",
    brightYellow: "#f0c674",
    brightBlue: "#61afef",
    brightMagenta: "#c678dd",
    brightCyan: "#56b6c2",
    brightWhite: "#ffffff",
  };
}
