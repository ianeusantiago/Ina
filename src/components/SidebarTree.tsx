import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  archiveTeam,
  listTeams,
  ORCHESTRATOR_ID,
  type PlanDraft,
  type TaskStatus,
} from "../lib/team";
import { TEAMS_CHANGED, usePolled } from "../lib/usePolled";
import { useTeamTasks } from "./TaskPanel";
import { primaryButton } from "./ui";

/* The sidebar tree: a workspace's teams and their members, with live status. */

/** A team member opened from the sidebar. */
export interface Focus {
  workspaceId: number;
  planId: number;
  agentId: string;
}

const DOT: Record<TaskStatus, string> = {
  pending: "bg-muted/50",
  running: "bg-success animate-pulse",
  verifying: "bg-info animate-pulse",
  done: "bg-success",
  failed: "bg-danger",
  blocked: "bg-warning",
  cancelled: "bg-muted",
};

const NO_TEAMS: PlanDraft[] = [];

export function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      aria-hidden
      viewBox="0 0 10 10"
      className={`size-2.5 transition-transform ${open ? "rotate-90" : ""}`}
    >
      <path
        d="M3.5 2 6.5 5l-3 3"
        stroke="currentColor"
        strokeWidth="1.5"
        fill="none"
        strokeLinecap="round"
      />
    </svg>
  );
}

/** One team: the orchestrator and its agents, with live task status. */
function TeamMembers({
  team,
  workspaceId,
  focus,
  onFocus,
  defaultOpen,
  filter,
  onDone,
  onArchive,
}: {
  team: PlanDraft;
  workspaceId: number;
  focus: Focus | null;
  onFocus: (f: Focus) => void;
  /** Newest team starts open; older ones collapsed (some workspaces have many). */
  defaultOpen: boolean;
  /** Lower-case text to match against the team and member names ("" shows all). */
  filter: string;
  onDone: (teamId: number, done: boolean) => void;
  onArchive: (teamId: number) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(defaultOpen);
  const holdsFocus = focus?.planId === team.id;
  const { tasks } = useTeamTasks(team.id);
  const status = new Map(tasks.map((x) => [x.agentId, x.status]));
  const done = tasks.length > 0 && tasks.every((x) => x.status === "done");
  useEffect(() => onDone(team.id, done), [onDone, team.id, done]);
  const [confirming, setConfirming] = useState(false);
  const members = [
    { id: ORCHESTRATOR_ID, name: t("team.orchestrator"), hint: team.plan.summary },
    ...team.plan.agents.map((a) => ({ id: a.id, name: a.name, hint: a.role })),
  ];
  const matches =
    !filter ||
    team.plan.summary.toLowerCase().includes(filter) ||
    members.some((m) => m.name.toLowerCase().includes(filter));
  if (!matches) return null;
  return (
    <li className="group/team">
      {confirming && (
        <div role="alert" className="mx-1 my-1 rounded-md bg-hover px-2 py-1.5 text-[11px] text-fg">
          <p>{t("archive.confirmOne")}</p>
          <div className="mt-1 flex gap-1.5">
            <button className={`text-[11px] ${primaryButton}`} onClick={() => onArchive(team.id)}>
              {t("archive.yes")}
            </button>
            <button
              className="text-[11px] text-muted hover:text-fg"
              onClick={() => setConfirming(false)}
            >
              {t("archive.no")}
            </button>
          </div>
        </div>
      )}
      <div className="flex items-center">
        <button
          className="flex w-full min-w-0 items-center gap-1.5 rounded-md px-1.5 pt-1.5 pb-0.5 text-left text-[11px] text-muted hover:text-fg"
          title={team.plan.summary}
          aria-expanded={open || holdsFocus}
          onClick={() => setOpen((o) => !o)}
        >
          <Chevron open={open || holdsFocus} />
          <span className="truncate">{team.plan.summary}</span>
          <span className="ml-auto shrink-0">{members.length}</span>
        </button>
        <button
          className="shrink-0 rounded px-1 pt-1 text-muted opacity-0 transition group-hover/team:opacity-100 hover:text-fg focus:opacity-100"
          aria-label={t("archive.one", { name: team.plan.summary })}
          title={t("archive.one", { name: team.plan.summary })}
          onClick={() => setConfirming(true)}
        >
          <svg aria-hidden viewBox="0 0 16 16" className="size-3">
            <path
              d="M2 4h12M3 4v8.5A1.5 1.5 0 0 0 4.5 14h7a1.5 1.5 0 0 0 1.5-1.5V4M6 7.5h4M1.5 2h13v2h-13z"
              stroke="currentColor"
              strokeWidth="1.3"
              fill="none"
            />
          </svg>
        </button>
      </div>
      {(open || holdsFocus) && (
        <ul>
          {members.map((m) => {
            const active = focus?.planId === team.id && focus.agentId === m.id;
            const st = status.get(m.id);
            const lead = m.id === ORCHESTRATOR_ID;
            return (
              <li key={m.id}>
                <button
                  className={`flex w-full min-w-0 items-center gap-2 rounded-md py-1 pr-2 pl-2.5 text-left text-[12.5px] transition ${
                    active ? "bg-accent text-white" : "text-fg hover:bg-hover"
                  }`}
                  title={m.hint}
                  aria-current={active}
                  onClick={() => onFocus({ workspaceId, planId: team.id, agentId: m.id })}
                >
                  {lead ? (
                    <svg aria-hidden viewBox="0 0 16 16" className="size-3.5 shrink-0 opacity-80">
                      <path
                        fill="currentColor"
                        d="M8 1.5 9.6 6.4 14.5 8l-4.9 1.6L8 14.5 6.4 9.6 1.5 8l4.9-1.6z"
                      />
                    </svg>
                  ) : (
                    <span
                      aria-hidden
                      className={`mx-1 size-1.5 shrink-0 rounded-full ${
                        active ? "bg-white" : st ? DOT[st] : "bg-muted/50"
                      }`}
                    />
                  )}
                  <span className={`truncate ${lead ? "font-semibold" : ""}`}>{m.name}</span>
                  {st && !active && (
                    <span className="ml-auto shrink-0 text-[10.5px] text-muted">
                      {t(`tasks.status.${st}`)}
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </li>
  );
}

export function WorkspaceTeams(props: {
  workspaceId: number;
  focus: Focus | null;
  onFocus: (f: Focus) => void;
  filter: string;
}) {
  const { t } = useTranslation();
  const [done, setDone] = useState<ReadonlySet<number>>(() => new Set());
  const [confirmAll, setConfirmAll] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const onDone = useCallback(
    (id: number, isDone: boolean) =>
      setDone((cur) => {
        if (cur.has(id) === isDone) return cur;
        const next = new Set(cur);
        if (isDone) next.add(id);
        else next.delete(id);
        return next;
      }),
    [],
  );
  const load = useCallback(() => listTeams(props.workspaceId), [props.workspaceId]);
  // No single team to filter on (-1): loads now, on the fallback interval and when teams change.
  const { data: teams, run } = usePolled(load, NO_TEAMS, -1);
  useEffect(() => {
    const reload = () => void run();
    window.addEventListener(TEAMS_CHANGED, reload);
    return () => window.removeEventListener(TEAMS_CHANGED, reload);
  }, [run]);
  const archive = async (ids: number[]) => {
    setError(null);
    try {
      for (const id of ids) await archiveTeam(id);
    } catch (e) {
      setError(String(e));
    }
    setConfirmAll(false);
    window.dispatchEvent(new Event(TEAMS_CHANGED));
  };
  if (teams.length === 0) {
    return <p className="py-1 pr-2 pl-9 text-[11.5px] text-muted">{t("workspaces.noTeams")}</p>;
  }
  const finished = teams.filter((x) => done.has(x.id)).map((x) => x.id);
  return (
    <div className="mt-0.5 ml-3 border-l border-line pl-1.5">
      {finished.length > 0 &&
        (confirmAll ? (
          <div
            role="alert"
            className="mx-1 my-1 rounded-md bg-hover px-2 py-1.5 text-[11px] text-fg"
          >
            <p>{t("archive.confirmAll", { count: finished.length })}</p>
            <div className="mt-1 flex gap-1.5">
              <button
                className={`text-[11px] ${primaryButton}`}
                onClick={() => void archive(finished)}
              >
                {t("archive.yes")}
              </button>
              <button
                className="text-[11px] text-muted hover:text-fg"
                onClick={() => setConfirmAll(false)}
              >
                {t("archive.no")}
              </button>
            </div>
          </div>
        ) : (
          <button
            className="px-1.5 pt-1 text-[11px] text-accent hover:underline"
            onClick={() => setConfirmAll(true)}
          >
            {t("archive.all", { count: finished.length })}
          </button>
        ))}
      {error && <p className="px-1.5 text-[11px] text-danger">{error}</p>}
      <ul>
        {teams.map((team, i) => (
          <TeamMembers
            key={team.id}
            team={team}
            defaultOpen={i === 0}
            onDone={onDone}
            onArchive={(id) => void archive([id])}
            {...props}
          />
        ))}
      </ul>
    </div>
  );
}
