import { useRef, type KeyboardEvent } from "react";

const STYLES = {
  /** Glossy Aqua pill (team views). */
  pill: {
    group: "aero-pill flex shrink-0 p-0.5 text-[12px] font-medium",
    item: "rounded-full px-3 py-1 transition",
    on: "aero-btn",
  },
  /** Flat tab, like a system toolbar control (title bar). */
  tab: {
    group: "flex rounded-md bg-hover p-0.5 text-[11px] font-medium",
    item: "rounded-[5px] px-2.5 py-0.5 transition",
    on: "bg-panel text-fg shadow-[0_0_0_0.5px_var(--ina-separator),0_1px_2px_rgb(0_0_0/0.1)]",
  },
} as const;

/**
 * One-of-several choice as an accessible radio group: only the selected option is in the tab
 * order, and the arrow keys (plus Home/End) move the selection.
 */
export function Segmented<T extends string>({
  options,
  value,
  onChange,
  label,
  optionLabel,
  variant,
}: {
  options: readonly T[];
  value: T;
  onChange: (value: T) => void;
  label: string;
  optionLabel: (value: T) => string;
  variant: keyof typeof STYLES;
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const style = STYLES[variant];
  const move = (e: KeyboardEvent, i: number) => {
    const last = options.length - 1;
    const next =
      e.key === "ArrowRight" || e.key === "ArrowDown"
        ? (i + 1) % options.length
        : e.key === "ArrowLeft" || e.key === "ArrowUp"
          ? (i + last) % options.length
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? last
              : null;
    const option = next === null ? undefined : options[next];
    if (next === null || option === undefined) return;
    e.preventDefault();
    onChange(option);
    refs.current[next]?.focus();
  };
  return (
    <div role="radiogroup" aria-label={label} className={style.group}>
      {options.map((o, i) => {
        const on = o === value;
        return (
          <button
            key={o}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={on}
            tabIndex={on ? 0 : -1}
            onClick={() => onChange(o)}
            onKeyDown={(e) => move(e, i)}
            className={`${style.item} ${on ? style.on : "text-fg-2 hover:text-fg"}`}
          >
            <span className="relative">{optionLabel(o)}</span>
          </button>
        );
      })}
    </div>
  );
}
