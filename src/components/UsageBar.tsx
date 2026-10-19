import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { readUsage, type Usage, type UsageReport, type UsageWindow } from "../lib/usage";

const POLL_MS = 60000;

/** Footer with plan usage (session and weekly windows) for Claude Code and Codex. */
export function UsageBar() {
  const { t } = useTranslation();
  const [report, setReport] = useState<UsageReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    const load = () =>
      readUsage()
        .then((r) => {
          if (!alive) return;
          setReport(r);
          setError(null);
        })
        .catch((e: unknown) => alive && setError(String(e)));
    void load();
    const timer = window.setInterval(() => void load(), POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);

  const codex = report?.codex;
  return (
    <footer
      aria-label={t("usage.title")}
      className="aero-glass m-2 mt-0 flex shrink-0 flex-wrap items-center gap-x-6 gap-y-2 rounded-2xl px-4 py-2 text-[11px]"
    >
      <Provider name="Claude Code" usage={report?.claude ?? null} empty={t("usage.claudeEmpty")} />
      <Provider
        name="Codex"
        usage={codex && "Ok" in codex ? codex.Ok : null}
        empty={codex && "Err" in codex ? t("usage.unavailable") : t("usage.loading")}
        title={codex && "Err" in codex ? codex.Err : undefined}
      />
      {error && <span className="text-danger">{error}</span>}
    </footer>
  );
}

function Provider({
  name,
  usage,
  empty,
  title,
}: {
  name: string;
  usage: Usage | null;
  empty: string;
  title?: string;
}) {
  return (
    <div className="flex items-center gap-3" title={title}>
      <span className="font-semibold text-fg">{name}</span>
      {usage && usage.windows.length > 0 ? (
        usage.windows.map((w) => <Meter key={w.minutes} w={w} />)
      ) : (
        <span className="text-muted">{empty}</span>
      )}
    </div>
  );
}

function Meter({ w }: { w: UsageWindow }) {
  const { t } = useTranslation();
  const pct = Math.min(100, Math.max(0, w.usedPercent));
  const label =
    w.minutes === 300
      ? t("usage.session")
      : w.minutes === 10080
        ? t("usage.weekly")
        : w.minutes % 1440 === 0
          ? t("usage.days", { count: w.minutes / 1440 })
          : t("usage.hours", { count: Math.round(w.minutes / 60) });
  const resets = w.resetsAt
    ? t("usage.resets", {
        when: new Date(w.resetsAt * 1000).toLocaleString(undefined, {
          weekday: "short",
          hour: "2-digit",
          minute: "2-digit",
        }),
      })
    : undefined;
  const fill =
    pct >= 90
      ? "from-[#ff8a80] to-[#d8342a]"
      : pct >= 70
        ? "from-[#ffd36b] to-[#e08a00]"
        : "from-[#9be86a] to-[#2a9d3a]";
  return (
    <div className="flex items-center gap-1.5" title={resets}>
      <span className="text-fg-2">{label}</span>
      <div
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(pct)}
        className="relative h-2.5 w-24 overflow-hidden rounded-full border border-[rgb(120_180_215/0.6)] bg-[rgb(255_255_255/0.5)] shadow-[inset_0_1px_2px_rgb(0_60_110/0.25)]"
      >
        <div
          className={`h-full rounded-full bg-gradient-to-b ${fill} transition-[width] duration-500`}
          style={{ width: `${pct}%` }}
        />
        <div className="pointer-events-none absolute inset-x-0 top-0 h-1/2 rounded-full bg-white/50" />
      </div>
      <span className="w-8 tabular-nums text-fg">{Math.round(pct)}%</span>
    </div>
  );
}
