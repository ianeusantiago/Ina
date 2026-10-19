import { useEffect, useRef } from "react";
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

/** Statuses worth a desktop notification. */
const NOTIFY: ReadonlySet<TaskStatus> = new Set(["done", "failed", "blocked"]);

/**
 * Agents whose status just became one worth notifying. The first sighting of a task only
 * records it (no flood of notifications for old results at startup).
 */
export function transitions(
  seen: Map<string, TaskStatus>,
  planId: number,
  tasks: TaskView[],
): { agentId: string; status: TaskStatus }[] {
  const out: { agentId: string; status: TaskStatus }[] = [];
  for (const task of tasks) {
    const key = `${planId}:${task.agentId}`;
    const before = seen.get(key);
    seen.set(key, task.status);
    if (before !== undefined && before !== task.status && NOTIFY.has(task.status)) {
      out.push({ agentId: task.agentId, status: task.status });
    }
  }
  return out;
}

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
  const names = useRef(new Map<string, string>());
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
            for (const a of team.plan.agents) names.current.set(`${team.id}:${a.id}`, a.name);
          }
        }
      }
      return names.current.get(key) ?? agentId;
    };
    let stop: (() => void) | null = null;
    let gone = false;
    listen<TeamChanged>(TEAM_CHANGED, (e) => {
      for (const { agentId, status } of transitions(
        seen.current,
        e.payload.planId,
        e.payload.tasks,
      )) {
        void nameOf(e.payload.planId, agentId).then((name) =>
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
  }, []);
}
