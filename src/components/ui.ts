/** Shared control styles (macOS + Frutiger Aero look; colors come from the theme tokens in index.css). */
export const smallButton =
  "aero-pill px-3 py-1 font-medium text-fg transition hover:brightness-105 active:scale-[0.98] disabled:opacity-40";
export const primaryButton =
  "aero-btn px-4 py-1 font-semibold transition active:scale-[0.98] disabled:opacity-40";
export const ghostButton =
  "rounded-md px-2 py-1 text-fg-2 transition hover:bg-hover hover:text-fg disabled:opacity-40";
export const panel = "aero-glass flex flex-col gap-1.5 rounded-2xl px-4 py-3 text-xs";
export const panelTitle = "text-[11px] font-semibold text-muted";
export const errorText = "break-words text-danger";
export const field =
  "rounded-md bg-panel text-fg shadow-[0_0_0_0.5px_var(--ina-separator)] outline-none transition focus:shadow-[0_0_0_3px_color-mix(in_srgb,var(--ina-accent)_35%,transparent)]";
