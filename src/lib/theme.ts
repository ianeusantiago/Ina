import type { ITheme } from "@xterm/xterm";

export const THEMES = ["light", "dark", "system"] as const;
export type ThemeChoice = (typeof THEMES)[number];

export const THEME_KEY = "ina.theme";

/** Light unless the user picked otherwise. */
export function parseTheme(stored: string | null): ThemeChoice {
  return THEMES.find((t) => t === stored) ?? "light";
}

function systemDark() {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
}

export function resolveTheme(choice: ThemeChoice): "light" | "dark" {
  return choice === "system" ? (systemDark() ? "dark" : "light") : choice;
}

export function applyTheme(choice: ThemeChoice) {
  document.documentElement.dataset.theme = resolveTheme(choice);
}

/** Terminal colors matching the current `data-theme` (ANSI tuned for readability on each background). */
export function terminalTheme(): ITheme {
  const dark = document.documentElement.dataset.theme === "dark";
  return dark
    ? {
        background: "#242426",
        foreground: "#f5f5f7",
        cursor: "#0a84ff",
        selectionBackground: "#0a84ff55",
        black: "#3a3a3c",
        red: "#ff6961",
        green: "#30d158",
        yellow: "#ffd60a",
        blue: "#64a8ff",
        magenta: "#da8fff",
        cyan: "#70d7ff",
        white: "#e5e5ea",
        brightBlack: "#8e8e93",
        brightWhite: "#ffffff",
      }
    : {
        background: "#ffffff",
        foreground: "#1d1d1f",
        cursor: "#007aff",
        selectionBackground: "#007aff33",
        black: "#1d1d1f",
        red: "#d70015",
        green: "#248a3d",
        yellow: "#a05a00",
        blue: "#0040dd",
        magenta: "#ad44ab",
        cyan: "#0071a4",
        white: "#6e6e73",
        brightBlack: "#86868b",
        brightRed: "#e5352b",
        brightGreen: "#1f9d4c",
        brightYellow: "#b25000",
        brightBlue: "#007aff",
        brightMagenta: "#bf5af2",
        brightCyan: "#0088b4",
        brightWhite: "#3a3a3c",
      };
}
