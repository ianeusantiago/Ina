import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import type { TaskView } from "../lib/team";

/** Stage reached and overall % (agents' work is 75%, merging the last 25%). */
export function progress(tasks: TaskView[], agents: number, merged: boolean) {
  const done = tasks.filter((x) => x.status === "done").length;
  const started = tasks.some((x) => x.status !== "pending");
  const problem = tasks.some((x) => x.status === "failed" || x.status === "blocked");
  const allDone = agents > 0 && done === agents;
  const stage = merged ? 3 : allDone ? 2 : started ? 1 : 0;
  const pct = Math.round((agents > 0 ? (done / agents) * 75 : 0) + (merged ? 25 : 0));
  return { done, stage, pct, problem };
}

const STEPS = ["assembled", "working", "verified", "merged"] as const;

interface TeamUsage {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  sessions: number;
}

const USAGE_POLL_MS = 30_000;

/** Tokens the team's Claude sessions used so far (refreshed while the team is open). */
function useTeamUsage(planId: number) {
  const [usage, setUsage] = useState<TeamUsage | null>(null);
  useEffect(() => {
    let alive = true;
    const load = () =>
      invoke<TeamUsage>("team_usage", { planId })
        .then((u) => alive && setUsage(u))
        .catch((e: unknown) => console.error("[usage]", e));
    void load();
    const timer = setInterval(() => void load(), USAGE_POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [planId]);
  return usage;
}

/** Simple timeline of the team: assembled → working (x/n) → verified → merged. */
export function TeamProgress({
  planId,
  tasks,
  agents,
  merged,
}: {
  planId: number;
  tasks: TaskView[];
  agents: number;
  merged: boolean;
}) {
  const { t, i18n } = useTranslation();
  const { done, stage, pct, problem } = progress(tasks, agents, merged);
  const usage = useTeamUsage(planId);
  const fmt = (n: number) =>
    new Intl.NumberFormat(i18n.language, { notation: "compact", maximumFractionDigits: 1 }).format(
      n,
    );
  return (
    <section
      aria-label={t("progress.label")}
      className="aero-card flex items-center gap-4 px-4 py-2.5 text-[12px]"
    >
      <ol className="flex min-w-0 flex-1 items-center gap-1.5">
        {STEPS.map((s, i) => {
          const reached = i <= stage;
          const current = i === stage && !merged;
          return (
            <li key={s} className="flex min-w-0 flex-1 items-center gap-1.5">
              <span
                aria-hidden
                className={`flex size-5 shrink-0 items-center justify-center rounded-full text-[10px] font-bold ${
                  reached ? "aero-btn text-white" : "bg-hover text-muted"
                } ${current && problem ? "ring-2 ring-warning" : ""}`}
              >
                <span className="relative">{i < stage || merged ? "✓" : i + 1}</span>
              </span>
              <span
                className={`truncate ${reached ? "font-semibold text-fg" : "text-muted"}`}
                aria-current={current ? "step" : undefined}
              >
                {s === "working"
                  ? t("progress.working", { done, total: agents })
                  : t(`progress.${s}`)}
              </span>
              {i < STEPS.length - 1 && (
                <span
                  aria-hidden
                  className={`h-0.5 min-w-3 flex-1 rounded-full ${i < stage ? "bg-accent" : "bg-line"}`}
                />
              )}
            </li>
          );
        })}
      </ol>
      <div className="flex w-36 shrink-0 items-center gap-2">
        <div className="h-2 flex-1 overflow-hidden rounded-full bg-hover">
          <div
            className="h-full rounded-full bg-gradient-to-r from-[#7ad3ff] to-accent transition-all"
            style={{ width: `${pct}%` }}
          />
        </div>
        <span className="w-9 text-right font-semibold text-fg">{pct}%</span>
      </div>
      {problem && <span className="shrink-0 text-warning">{t("progress.problem")}</span>}
      {usage && usage.sessions > 0 && (
        <span
          className="shrink-0 text-muted"
          title={t("progress.usageDetail", {
            input: fmt(usage.input),
            output: fmt(usage.output),
            cacheRead: fmt(usage.cacheRead),
            cacheWrite: fmt(usage.cacheWrite),
            sessions: usage.sessions,
          })}
        >
          {t("progress.usage", {
            tokens: fmt(usage.input + usage.output + usage.cacheRead + usage.cacheWrite),
          })}
        </span>
      )}
    </section>
  );
}
