import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import i18n from "../i18n";
import { listTeams, ORCHESTRATOR_ID, type TaskStatus, type TaskView } from "./team";
import { TEAM_CHANGED, type TeamChanged } from "./usePolled";
import type { Workspace } from "./workspaces";
import { postReportToHome } from "./report";

/** Statuses worth a desktop notification. */
const NOTIFY: ReadonlySet<TaskStatus> = new Set(["done", "failed", "blocked"]);

/** Registry timestamps are SQLite UTC ("YYYY-MM-DD HH:MM:SS"). */
export const updatedMs = (t: TaskView) => Date.parse(`${t.updatedAt.replace(" ", "T")}Z`);

/**
 * Agents whose status became one worth notifying. A task seen for the first time counts only
 * if it changed after `since` (the app start), so old results never flood at startup while a
 * team that finished between two looks (or across a reload) is still announced.
 */
export function transitions(
  seen: Map<string, TaskStatus>,
  planId: number,
  tasks: TaskView[],
  since: number,
): { agentId: string; status: TaskStatus }[] {
  const out: { agentId: string; status: TaskStatus }[] = [];
  for (const task of tasks) {
    const key = `${planId}:${task.agentId}`;
    const before = seen.get(key);
    seen.set(key, task.status);
    const fresh = before === undefined ? updatedMs(task) >= since : before !== task.status;
    if (fresh && NOTIFY.has(task.status)) out.push({ agentId: task.agentId, status: task.status });
  }
  return out;
}

/** How recent a finished team must be to get its report posted to Home. */
const REPORT_WINDOW_MS = 10 * 60_000;

let allowed: Promise<boolean> | null = null;

/** Shows a Windows notification (asks for permission once). Failures are only logged. */
export async function notify(title: string, body: string) {
  try {
    allowed ??= isPermissionGranted().then(
      async (ok) => ok || (await requestPermission()) === "granted",
    );
    if (await allowed) sendNotification({ title, body });
  } catch (e) {
    console.error("[notify]", e);
  }
}

/** Notifies when any team's agent finishes, fails or gets blocked (from the backend tick). */
export function useTeamNotifications(workspaces: Workspace[]) {
  const seen = useRef(new Map<string, TaskStatus>());
  const [since] = useState(() => Date.now());
  const names = useRef(new Map<string, string>());
  const summaries = useRef(new Map<number, string>());
  const workspacesRef = useRef(workspaces);
  useEffect(() => {
    workspacesRef.current = workspaces;
  }, [workspaces]);

  useEffect(() => {
    const nameOf = async (planId: number, agentId: string) => {
      const key = `${planId}:${agentId}`;
      if (!names.current.has(key)) {
        // Unknown team (approved since the last look): refresh names from every workspace.
        for (const w of workspacesRef.current) {
          for (const team of await listTeams(w.id).catch(() => [])) {
            names.current.set(`${team.id}:${ORCHESTRATOR_ID}`, i18n.t("team.orchestrator"));
            summaries.current.set(team.id, team.plan.summary);
            for (const a of team.plan.agents) names.current.set(`${team.id}:${a.id}`, a.name);
          }
        }
      }
      return names.current.get(key) ?? agentId;
    };
    let stop: (() => void) | null = null;
    let gone = false;
    listen<TeamChanged>(TEAM_CHANGED, (e) => {
      const { planId, tasks } = e.payload;
      const changed = transitions(seen.current, planId, tasks, since);
      // Report once per team (postReportToHome dedupes); also catches a team that finished just
      // before a reload or restart cut the previous report request short.
      if (
        tasks.length > 0 &&
        tasks.every((x) => x.status === "done") &&
        Math.max(...tasks.map(updatedMs)) >= since - REPORT_WINDOW_MS
      ) {
        void nameOf(planId, ORCHESTRATOR_ID).then(() =>
          postReportToHome(planId, summaries.current.get(planId) ?? `#${planId}`),
        );
      }
      for (const { agentId, status } of changed) {
        void nameOf(planId, agentId).then((name) =>
          notify(i18n.t(`notify.${status}.title`, { name }), i18n.t(`notify.${status}.body`)),
        );
      }
    })
      .then((unlisten) => (gone ? unlisten() : (stop = unlisten)))
      .catch((e: unknown) => console.error("[notify]", e));
    return () => {
      gone = true;
      stop?.();
    };
  }, [since]);
}
