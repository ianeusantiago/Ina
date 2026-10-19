import { useTranslation } from "react-i18next";

export const TEMPLATES = ["site", "api", "bug", "refactor"] as const;
export type Template = (typeof TEMPLATES)[number];

/**
 * Ready-made team requests. `where` picks the wording: "team" for a workspace that exists,
 * "home" for one Home creates (the text names the new workspace).
 */
export function TemplateChips({
  where,
  onPick,
  disabled,
}: {
  where: "team" | "home";
  onPick: (text: string) => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-wrap gap-1.5" role="group" aria-label={t("templates.label")}>
      {TEMPLATES.map((k) => (
        <button
          key={k}
          type="button"
          disabled={disabled}
          title={t(`templates.${k}.${where}`)}
          onClick={() => onPick(t(`templates.${k}.${where}`))}
          className="aero-pill px-2.5 py-1 text-[11.5px] font-medium text-fg-2 transition hover:text-fg disabled:opacity-40"
        >
          {t(`templates.${k}.name`)}
        </button>
      ))}
    </div>
  );
}
