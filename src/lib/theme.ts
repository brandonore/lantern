import type { ITheme } from "@xterm/xterm";
import type { TerminalColors } from "./themes/index";

export function getTerminalTheme(colors?: TerminalColors): ITheme {
  if (colors) {
    return {
      background: undefined,
      foreground: undefined,
      cursor: undefined,
      cursorAccent: undefined,
      selectionBackground: colors.selectionBackground,
      selectionForeground: undefined,
      black: colors.black,
      red: colors.red,
      green: colors.green,
      yellow: colors.yellow,
      blue: colors.blue,
      magenta: colors.magenta,
      cyan: colors.cyan,
      white: colors.white,
      brightBlack: colors.brightBlack,
      brightRed: colors.brightRed,
      brightGreen: colors.brightGreen,
      brightYellow: colors.brightYellow,
      brightBlue: colors.brightBlue,
      brightMagenta: colors.brightMagenta,
      brightCyan: colors.brightCyan,
      brightWhite: colors.brightWhite,
    };
  }

  const style = getComputedStyle(document.documentElement);
  const get = (prop: string) => style.getPropertyValue(prop).trim();

  return {
    background: get("--bg-primary") || "#2e3440",
    foreground: get("--text-primary") || "#eceff4",
    cursor: get("--accent") || "#88c0d0",
    cursorAccent: get("--bg-primary") || "#2e3440",
    selectionBackground: "rgba(136, 192, 208, 0.2)",
    selectionForeground: undefined,
    black: "#3b4252",
    red: "#bf616a",
    green: "#a3be8c",
    yellow: "#ebcb8b",
    blue: "#81a1c1",
    magenta: "#b48ead",
    cyan: "#88c0d0",
    white: "#e5e9f0",
    brightBlack: "#4c566a",
    brightRed: "#bf616a",
    brightGreen: "#a3be8c",
    brightYellow: "#ebcb8b",
    brightBlue: "#81a1c1",
    brightMagenta: "#b48ead",
    brightCyan: "#8fbcbb",
    brightWhite: "#eceff4",
  };
}
