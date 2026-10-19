import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { LANGUAGES } from "../i18n";
import { usePersistedState } from "../lib/usePersistedState";
import { applyTheme, parseTheme, THEME_KEY, THEMES } from "../lib/theme";

interface Props {
  language: string;
  languageLabel: string;
  onLanguage: (lng: (typeof LANGUAGES)[number]) => void;
}

/** macOS-style toolbar: centered title, appearance and language (window controls stay native). */
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

  return (
    <header className="relative flex h-12 shrink-0 select-none items-center gap-4 border-b border-line bg-sidebar px-4 backdrop-blur-2xl">
      <span className="pointer-events-none absolute left-1/2 -translate-x-1/2 text-[13px] font-semibold text-fg">
        {t("app.name")}
      </span>

      <span className="flex-1" />

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
