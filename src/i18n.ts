import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";
import ptBR from "./locales/pt-BR.json";

export const LANGUAGES = ["pt-BR", "en"] as const;

export function detectLanguage(locale: string): (typeof LANGUAGES)[number] {
  return locale.toLowerCase().startsWith("pt") ? "pt-BR" : "en";
}

const STORAGE_KEY = "ina.language";

/** Saved choice, if any; storage may be unavailable (private mode, blocked). */
function savedLanguage(): (typeof LANGUAGES)[number] | null {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return LANGUAGES.find((l) => l === v) ?? null;
  } catch {
    return null;
  }
}

export function setLanguage(lng: (typeof LANGUAGES)[number]) {
  void i18n.changeLanguage(lng);
  try {
    localStorage.setItem(STORAGE_KEY, lng);
  } catch {
    // Not persisted; the choice still applies to this session.
  }
}

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, "pt-BR": { translation: ptBR } },
  lng: savedLanguage() ?? detectLanguage(navigator.language),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
