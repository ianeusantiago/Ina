import { useId, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

export const TEMPLATES = ["site", "api", "bug", "refactor"] as const;
export type Template = (typeof TEMPLATES)[number];

/** Glassy Aqua tones per template: [light, deep]. */
const TONES: Record<Template, [string, string]> = {
  site: ["#8fe3ff", "#1a8fe0"],
  api: ["#b9f59a", "#3aa83a"],
  bug: ["#ffb3a8", "#e0483a"],
  refactor: ["#ffe9a0", "#e6a100"],
};

/** Small Frutiger Aero glyph: gradient body, soft top gloss, thin white rim. */
function AeroIcon({ kind }: { kind: Template }) {
  const id = useId();
  const [light, deep] = TONES[kind];
  const body = `url(#${id}b)`;
  const shapes: Record<Template, ReactNode> = {
    // Globe: sphere with meridian and parallels.
    site: (
      <>
        <circle cx="8" cy="8" r="6.2" fill={body} />
        <g fill="none" stroke="#fff" strokeOpacity="0.85" strokeWidth="0.9">
          <ellipse cx="8" cy="8" rx="2.6" ry="6.2" />
          <path d="M1.9 6.2h12.2M1.9 9.8h12.2" />
        </g>
      </>
    ),
    // Two chain links: the API and its tests connected.
    api: (
      <g fill="none" strokeWidth="2.4" strokeLinecap="round">
        <rect
          x="1.4"
          y="5.4"
          width="7.8"
          height="5.2"
          rx="2.6"
          stroke={body}
          transform="rotate(-35 5.3 8)"
        />
        <rect
          x="6.8"
          y="5.4"
          width="7.8"
          height="5.2"
          rx="2.6"
          stroke={body}
          transform="rotate(-35 10.7 8)"
        />
      </g>
    ),
    // Ladybug: shell with a center line and spots.
    bug: (
      <>
        <circle cx="8" cy="4.2" r="1.9" fill="#3b3f47" />
        <ellipse cx="8" cy="9.2" rx="5.2" ry="5.2" fill={body} />
        <path d="M8 4.6v9.6" stroke="#3b3f47" strokeWidth="0.9" />
        <g fill="#3b3f47">
          <circle cx="5.6" cy="8.2" r="0.95" />
          <circle cx="10.4" cy="8.2" r="0.95" />
          <circle cx="6.2" cy="11.4" r="0.8" />
          <circle cx="9.8" cy="11.4" r="0.8" />
        </g>
      </>
    ),
    // Sparkle: code made shiny and clean.
    refactor: (
      <>
        <path d="M8 1.2 9.6 6.4 14.8 8 9.6 9.6 8 14.8 6.4 9.6 1.2 8 6.4 6.4z" fill={body} />
        <path d="M13 1.6l.5 1.4 1.4.5-1.4.5-.5 1.4-.5-1.4-1.4-.5 1.4-.5z" fill={deep} />
      </>
    ),
  };
  return (
    <svg aria-hidden viewBox="0 0 16 16" className="size-3.5 shrink-0">
      <defs>
        <linearGradient id={`${id}b`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={light} />
          <stop offset="1" stopColor={deep} />
        </linearGradient>
        <linearGradient id={`${id}g`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0.9" />
          <stop offset="1" stopColor="#fff" stopOpacity="0" />
        </linearGradient>
      </defs>
      <g stroke="#fff" strokeOpacity="0.6" strokeWidth="0.4">
        {shapes[kind]}
      </g>
      {/* Aqua gloss over the top half. */}
      <ellipse cx="8" cy="4.6" rx="4.6" ry="2.2" fill={`url(#${id}g)`} opacity="0.55" />
    </svg>
  );
}

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
          className="aero-pill flex items-center gap-1.5 px-2.5 py-1 text-[11.5px] font-medium text-fg-2 transition hover:text-fg disabled:opacity-40"
        >
          <AeroIcon kind={k} />
          {t(`templates.${k}.name`)}
        </button>
      ))}
    </div>
  );
}
