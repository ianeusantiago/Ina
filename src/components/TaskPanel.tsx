import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { teamTasks, type AgentSpec, type TaskStatus, type TaskView } from "../lib/team";

const POLL_MS = 5000;

const tone: Record<TaskStatus, string> = {
  pending: "text-muted",
  running: "text-sky-300",
  done: "text-emerald-300",
  failed: "text-red-400",
  blocked: "text-amber-300",
  cancelled: "text-muted",
};

/** SQLite `datetime('now')` is UTC without a zone marker. */
const localTime = (utc: string) => new Date(`${utc.replace(" ", "T")}Z`).toLocaleString();

/** Polls the team's task registry (status comes from the backend: process + result.json). */
export function useTeamTasks(planId: number) {
  const [tasks, setTasks] = useState<TaskView[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setTasks(await teamTasks(planId));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [planId]);

  useEffect(() => {
    const first = setTimeout(() => void refresh(), 0);
    const timer = setInterval(() => void refresh(), POLL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [refresh]);

  return { tasks, error, refresh };
}

export function TaskPanel({
  agents,
  tasks,
  error,
  onRefresh,
  autoStart,
  onAutoStart,
}: {
  agents: AgentSpec[];
  tasks: TaskView[];
  error: string | null;
  onRefresh: () => void;
  autoStart: boolean;
  onAutoStart: (on: boolean) => void;
}) {
  const { t } = useTranslation();
  const byId = new Map(agents.map((a) => [a.id, a]));

  return (
    <section className="flex flex-col gap-1 rounded border border-line bg-panel px-3 py-2 text-xs">
      <div className="flex items-center gap-2">
        <span className="font-semibold uppercase tracking-wide text-muted">{t("tasks.title")}</span>
        <button
          className="rounded border border-line px-2 py-0.5 hover:bg-line"
          onClick={onRefresh}
        >
          {t("tasks.refresh")}
        </button>
        <label className="flex items-center gap-1 text-muted">
          <input
            type="checkbox"
            checked={autoStart}
            onChange={(e) => onAutoStart(e.target.checked)}
          />
          {t("tasks.autoStart")}
        </label>
      </div>
      {error && (
        <p role="alert" className="break-words text-red-400">
          {error}
        </p>
      )}
      {tasks.length === 0 && !error && <p className="text-muted">{t("tasks.none")}</p>}
      <ul className="flex flex-col gap-1">
        {tasks.map((task) => {
          const spec = byId.get(task.agentId);
          return (
            <li key={task.agentId} className="flex flex-col">
              <div className="flex flex-wrap items-baseline gap-2">
                <span className="text-zinc-100">{spec?.name ?? task.agentId}</span>
                <span className={tone[task.status]}>{t(`tasks.status.${task.status}`)}</span>
                <span className="text-muted">{localTime(task.updatedAt)}</span>
                {task.summary && <span className="text-zinc-300">{task.summary}</span>}
                {task.detail && task.detail !== task.summary && (
                  <span className="text-muted">· {task.detail}</span>
                )}
              </div>
              {task.resultError && (
                <p className="text-amber-300">
                  {t("tasks.invalidResult", { error: task.resultError })}
                </p>
              )}
              {spec && spec.acceptance.length > 0 && (
                <details className="text-muted">
                  <summary className="cursor-pointer">
                    {t("tasks.criteria", { count: spec.acceptance.length })}
                  </summary>
                  <ul className="ml-4 list-disc">
                    {spec.acceptance.map((c) => (
                      <li key={c}>{c}</li>
                    ))}
                  </ul>
                </details>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
