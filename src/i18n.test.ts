import { describe, expect, it } from "vitest";
import { detectLanguage } from "./i18n";
import en from "./locales/en.json";
import ptBR from "./locales/pt-BR.json";

const keys = (o: object, p = ""): string[] =>
  Object.entries(o).flatMap(([k, v]) =>
    typeof v === "object" && v !== null ? keys(v as object, `${p}${k}.`) : [`${p}${k}`],
  );

describe("i18n", () => {
  it("detects language from OS locale", () => {
    expect(detectLanguage("pt-BR")).toBe("pt-BR");
    expect(detectLanguage("pt-PT")).toBe("pt-BR");
    expect(detectLanguage("en-US")).toBe("en");
    expect(detectLanguage("de-DE")).toBe("en");
  });

  it("locales have identical keys", () => {
    expect(keys(ptBR).sort()).toEqual(keys(en).sort());
  });
});
