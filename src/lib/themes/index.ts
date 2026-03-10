export interface ThemeColors {
  bgPrimary: string;
  bgSecondary: string;
  bgTertiary: string;
  bgElevated: string;
  border: string;
  borderFocus: string;
  textPrimary: string;
  textSecondary: string;
  textTertiary: string;
  accent: string;
  accentDim: string;
  accentBg: string;
  statusClean: string;
  statusDirty: string;
  statusError: string;
}

export interface TerminalColors {
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
  selectionBackground: string;
}

export interface ThemeVariant {
  colors: ThemeColors;
  terminal: TerminalColors;
}

export interface ThemeFamily {
  id: string;
  name: string;
  dark: ThemeVariant;
  light: ThemeVariant;
}

import { nord } from "./themes/nord";
import { tokyoNight } from "./themes/tokyo-night";
import { catppuccin } from "./themes/catppuccin";
import { dracula } from "./themes/dracula";
import { one } from "./themes/one";
import { solarized } from "./themes/solarized";
import { rosePine } from "./themes/rose-pine";
import { gruvbox } from "./themes/gruvbox";
import { github } from "./themes/github";
import { kanagawa } from "./themes/kanagawa";

const families: ThemeFamily[] = [
  nord,
  tokyoNight,
  catppuccin,
  dracula,
  one,
  solarized,
  rosePine,
  gruvbox,
  github,
  kanagawa,
];

const familyMap = new Map(families.map((f) => [f.id, f]));

export function getAllFamilies(): ThemeFamily[] {
  return families;
}

export function getThemeFamily(id: string): string {
  const dash = id.lastIndexOf("-");
  return dash > 0 ? id.slice(0, dash) : id;
}

export function getThemeMode(id: string): "dark" | "light" {
  return id.endsWith("-light") ? "light" : "dark";
}

export function toggleMode(id: string): string {
  const family = getThemeFamily(id);
  const mode = getThemeMode(id);
  return `${family}-${mode === "dark" ? "light" : "dark"}`;
}

export function getTheme(id: string): ThemeVariant {
  const familyId = getThemeFamily(id);
  const mode = getThemeMode(id);
  const family = familyMap.get(familyId) ?? nord;
  return family[mode];
}

const colorVarMap: Record<keyof ThemeColors, string> = {
  bgPrimary: "--bg-primary",
  bgSecondary: "--bg-secondary",
  bgTertiary: "--bg-tertiary",
  bgElevated: "--bg-elevated",
  border: "--border",
  borderFocus: "--border-focus",
  textPrimary: "--text-primary",
  textSecondary: "--text-secondary",
  textTertiary: "--text-tertiary",
  accent: "--accent",
  accentDim: "--accent-dim",
  accentBg: "--accent-bg",
  statusClean: "--status-clean",
  statusDirty: "--status-dirty",
  statusError: "--status-error",
};

export function applyTheme(variant: ThemeVariant, mode?: "dark" | "light"): void {
  const root = document.documentElement;
  for (const [key, cssVar] of Object.entries(colorVarMap)) {
    root.style.setProperty(cssVar, variant.colors[key as keyof ThemeColors]);
  }
  // Tell the browser which color scheme to use for native controls (select, scrollbars, etc.)
  const scheme = mode ?? (isColorDark(variant.colors.bgPrimary) ? "dark" : "light");
  root.style.setProperty("color-scheme", scheme);
}

function isColorDark(hex: string): boolean {
  const c = hex.replace("#", "");
  const r = parseInt(c.slice(0, 2), 16);
  const g = parseInt(c.slice(2, 4), 16);
  const b = parseInt(c.slice(4, 6), 16);
  return (r * 299 + g * 587 + b * 114) / 1000 < 128;
}

export function applyUiScale(scale: number): void {
  document.documentElement.style.setProperty('--ui-scale', String(scale));
}
