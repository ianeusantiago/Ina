import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LANGUAGES } from "../i18n";
import { usePersistedState } from "../lib/usePersistedState";
import { applyTheme, parseTheme, THEME_KEY, THEMES } from "../lib/theme";

interface Props {
  language: string;
  languageLabel: string;
  onLanguage: (lng: (typeof LANGUAGES)[number]) => void;
}

/** Calls the native window; outside Tauri (plain browser) it is a no-op. */
function windowAction(action: "close" | "minimize" | "toggleMaximize") {
  try {
    const win = getCurrentWindow();
    win[action]().catch((e: unknown) => console.error(`window ${action} failed`, e));
  } catch {
    // Not running inside Tauri.
  }
}

const lights = [
  { action: "close", color: "bg-[#ff5f57]", glyph: "M3 3l4 4M7 3l-4 4" },
  { action: "minimize", color: "bg-[#febc2e]", glyph: "M2.5 5h5" },
  { action: "toggleMaximize", color: "bg-[#28c840]", glyph: "M3 7V3h4M7 3v4H3" },
] as const;

/** macOS-style unified title bar: traffic lights, centered title, appearance and language. */
export function TitleBar({ language, languageLabel, onLanguage }: Props) {
  const { t } = useTranslation();
  const [theme, setTheme] = usePersistedState(THEME_KEY, parseTheme);

  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const follow = () => applyTheme("system");
    media.addEventListener("change", follow);
    return () => media.removeEventListener("change", follow);
  }, [theme]);

  const labels = {
    close: t("app.window.close"),
    minimize: t("app.window.minimize"),
    toggleMaximize: t("app.window.maximize"),
  };

  return (
    <header
      data-tauri-drag-region
      onDoubleClick={(e) => e.target === e.currentTarget && windowAction("toggleMaximize")}
      className="relative flex h-12 shrink-0 select-none items-center gap-4 border-b border-line bg-sidebar px-4 backdrop-blur-2xl"
    >
      <div className="group flex items-center gap-2">
        {lights.map((l) => (
          <button
            key={l.action}
            aria-label={labels[l.action]}
            title={labels[l.action]}
            onClick={() => windowAction(l.action)}
            className={`flex size-3 items-center justify-center rounded-full ${l.color} shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.18)] focus-visible:outline-offset-2`}
          >
            <svg
              aria-hidden
              viewBox="0 0 10 10"
              className="size-2 opacity-0 transition group-hover:opacity-100"
            >
              <path d={l.glyph} stroke="rgb(0 0 0 / 0.55)" strokeWidth="1.3" fill="none" />
            </svg>
          </button>
        ))}
      </div>

      <span
        data-tauri-drag-region
        className="pointer-events-none absolute left-1/2 -translate-x-1/2 text-[13px] font-semibold text-fg"
      >
        {t("app.name")}
      </span>

      <span data-tauri-drag-region className="h-full flex-1" />

      <div
        role="radiogroup"
        aria-label={t("app.theme")}
        className="flex rounded-md bg-hover p-0.5 text-[11px] font-medium"
      >
        {THEMES.map((th) => (
          <button
            key={th}
            role="radio"
            aria-checked={theme === th}
            onClick={() => setTheme(th)}
            className={`rounded-[5px] px-2.5 py-0.5 transition ${
              theme === th
                ? "bg-panel text-fg shadow-[0_0_0_0.5px_var(--ina-separator),0_1px_2px_rgb(0_0_0/0.1)]"
                : "text-fg-2 hover:text-fg"
            }`}
          >
            {t(`app.themes.${th}`)}
          </button>
        ))}
      </div>

      <label className="flex items-center gap-2 text-[11px] font-medium text-muted">
        {languageLabel}
        <select
          className="rounded-md bg-panel px-1.5 py-0.5 text-[12px] text-fg shadow-[0_0_0_0.5px_var(--ina-separator),0_1px_1px_rgb(0_0_0/0.05)] outline-none"
          value={language}
          onChange={(e) => onLanguage(e.target.value as (typeof LANGUAGES)[number])}
        >
          {LANGUAGES.map((l) => (
            <option key={l} value={l}>
              {t(`app.languages.${l}`)}
            </option>
          ))}
        </select>
      </label>
    </header>
  );
}
