import { useTranslation } from "react-i18next";

export const SPLIT_MIN = 10;
export const SPLIT_MAX = 70;

/** Height of the task/message area, in % of the team view. */
export function parseSplit(v: string | null): number {
  const n = Number(v);
  return v !== null && n >= SPLIT_MIN && n <= SPLIT_MAX ? n : 40;
}

/** Drag handle that resizes the area above it as a % of `container`'s height. */
export function Splitter({
  container,
  value,
  onChange,
}: {
  container: React.RefObject<HTMLDivElement | null>;
  value: number;
  onChange: (pct: number) => void;
}) {
  const { t } = useTranslation();
  const set = (pct: number) => onChange(Math.round(Math.min(SPLIT_MAX, Math.max(SPLIT_MIN, pct))));
  const drag = (e: React.PointerEvent<HTMLDivElement>) => {
    const box = container.current?.getBoundingClientRect();
    if (!box) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => set(((ev.clientY - box.top) / box.height) * 100);
    const target = e.currentTarget;
    const up = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
      target.removeEventListener("pointercancel", up);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
    // A drag the system interrupts must not leave its listeners behind.
    target.addEventListener("pointercancel", up);
  };
  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label={t("team.resize")}
      aria-valuenow={value}
      aria-valuemin={SPLIT_MIN}
      aria-valuemax={SPLIT_MAX}
      tabIndex={0}
      className="h-1.5 shrink-0 cursor-row-resize rounded bg-line/50 hover:bg-accent/60 focus:bg-accent/60 focus:outline-none"
      onPointerDown={drag}
      onKeyDown={(e) => {
        if (e.key === "ArrowUp") set(value - 5);
        if (e.key === "ArrowDown") set(value + 5);
      }}
    />
  );
}
