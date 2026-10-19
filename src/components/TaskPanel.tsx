import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { panel, panelTitle, errorText } from "./ui";

import { usePolled, type TeamChanged } from "../lib/usePolled";
import { teamTasks, type AgentSpec, type TaskStatus, type TaskView } from "../lib/team";

const tone: Record<TaskStatus, string> = {
  pending: "text-muted",
  running: "text-info",
  verifying: "text-info",
  done: "text-success",
  failed: "text-danger",
  blocked: "text-warning",
  cancelled: "text-muted",
};

/** SQLite `datetime('now')` is UTC without a zone marker. */
const localTime = (utc: string) => new Date(`${utc.replace(" ", "T")}Z`).toLocaleString();

const NONE: TaskView[] = [];
const eventTasks = (e: TeamChanged) => e.tasks;

/** Polls the team's task registry (status comes from the backend: process + result.json). */
export function useTeamTasks(planId: number) {
  const load = useCallback(() => teamTasks(planId), [planId]);
  const { data: tasks, error, run } = usePolled(load, NONE, planId, eventTasks);
  return { tasks, error, refresh: () => run() };
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
    <section className={panel}>
      <div className="flex items-center gap-2">
        <span className={panelTitle}>{t("tasks.title")}</span>
        <button
          className="rounded-md bg-panel px-2 py-1 font-medium shadow-[0_0_0_0.5px_var(--ina-separator),0_1px_1px_rgb(0_0_0/0.05)] transition hover:bg-hover active:scale-[0.98]"
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
        <p role="alert" className={errorText}>
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
                <span className="text-fg">{spec?.name ?? task.agentId}</span>
                <span className={tone[task.status]}>{t(`tasks.status.${task.status}`)}</span>
                <span className="text-muted">{localTime(task.updatedAt)}</span>
                {task.summary && <span className="text-fg-2">{task.summary}</span>}
                {task.detail && task.detail !== task.summary && (
                  <span className="text-muted">· {task.detail}</span>
                )}
              </div>
              {task.resultError && (
                <p className="text-warning">
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
