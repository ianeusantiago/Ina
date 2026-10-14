import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";
import ptBR from "./locales/pt-BR.json";

export const LANGUAGES = ["pt-BR", "en"] as const;

export function detectLanguage(locale: string): (typeof LANGUAGES)[number] {
  return locale.toLowerCase().startsWith("pt") ? "pt-BR" : "en";
}

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, "pt-BR": { translation: ptBR } },
  lng: detectLanguage(navigator.language),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
