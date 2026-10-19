import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type { Workspace } from "../lib/workspaces";
import { listTeams, ORCHESTRATOR_ID, type PlanDraft, type TaskStatus } from "../lib/team";
import { TEAMS_CHANGED, usePolled } from "../lib/usePolled";
import { useTeamTasks } from "./TaskPanel";
import { field, primaryButton } from "./ui";

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

function Chevron({ open }: { open: boolean }) {
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
}: {
  team: PlanDraft;
  workspaceId: number;
  focus: Focus | null;
  onFocus: (f: Focus) => void;
  /** Newest team starts open; older ones collapsed (some workspaces have many). */
  defaultOpen: boolean;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(defaultOpen);
  const holdsFocus = focus?.planId === team.id;
  const { tasks } = useTeamTasks(team.id);
  const status = new Map(tasks.map((x) => [x.agentId, x.status]));
  const members = [
    { id: ORCHESTRATOR_ID, name: t("team.orchestrator"), hint: team.plan.summary },
    ...team.plan.agents.map((a) => ({ id: a.id, name: a.name, hint: a.role })),
  ];
  return (
    <li>
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

function WorkspaceTeams(props: {
  workspaceId: number;
  focus: Focus | null;
  onFocus: (f: Focus) => void;
}) {
  const { t } = useTranslation();
  const load = useCallback(() => listTeams(props.workspaceId), [props.workspaceId]);
  // No single team to filter on (-1): loads now, on the fallback interval and when teams change.
  const { data: teams, run } = usePolled(load, NO_TEAMS, -1);
  useEffect(() => {
    const reload = () => void run();
    window.addEventListener(TEAMS_CHANGED, reload);
    return () => window.removeEventListener(TEAMS_CHANGED, reload);
  }, [run]);
  if (teams.length === 0) {
    return <p className="py-1 pr-2 pl-9 text-[11.5px] text-muted">{t("workspaces.noTeams")}</p>;
  }
  return (
    <ul className="mt-0.5 ml-3 border-l border-line pl-1.5">
      {teams.map((team, i) => (
        <TeamMembers key={team.id} team={team} defaultOpen={i === 0} {...props} />
      ))}
    </ul>
  );
}

interface Props {
  workspaces: Workspace[];
  selectedId: number | null;
  error: string | null;
  onSelect: (id: number) => void;
  onHome: () => void;
  focus: Focus | null;
  onFocus: (focus: Focus) => void;
  onAdd: (path: string) => void;
  onRemove: (id: number) => void;
}

export function Sidebar({
  workspaces,
  selectedId,
  error,
  onSelect,
  onHome,
  focus,
  onFocus,
  onAdd,
  onRemove,
}: Props) {
  const { t } = useTranslation();
  const [path, setPath] = useState("");
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(() => new Set());
  const toggle = (id: number) =>
    setExpanded((cur) => {
      const next = new Set(cur);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (path.trim()) {
      onAdd(path.trim());
      setPath("");
    }
  };

  return (
    <aside className="flex w-64 shrink-0 flex-col aero-glass m-2 mr-0 rounded-2xl bg-sidebar text-[13px]">
      <div className="px-2.5 pt-2.5">
        <button
          className={`flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left transition ${
            selectedId === null ? "bg-accent font-medium text-white" : "text-fg hover:bg-hover"
          }`}
          aria-current={selectedId === null}
          onClick={onHome}
        >
          <svg aria-hidden viewBox="0 0 16 16" className="size-3.5 shrink-0 opacity-90">
            <path
              fill="currentColor"
              d="M8 1.8 1.5 7.2V14a.8.8 0 0 0 .8.8h3.9V10h3.6v4.8h3.9a.8.8 0 0 0 .8-.8V7.2z"
            />
          </svg>
          {t("home.nav")}
        </button>
      </div>
      <h2 className="px-5 pt-3 pb-1.5 text-[11px] font-semibold text-muted">
        {t("workspaces.title")}
      </h2>
      <ul className="min-h-0 flex-1 overflow-y-auto px-2.5" aria-label={t("workspaces.title")}>
        {workspaces.length === 0 && (
          <li className="px-2.5 py-2 text-muted">{t("workspaces.empty")}</li>
        )}
        {workspaces.map((w) => {
          const open = expanded.has(w.id);
          const current = w.id === selectedId && !focus;
          return (
            <li key={w.id} className="mb-0.5">
              <div className="group flex items-center rounded-md">
                <button
                  className="flex size-6 shrink-0 items-center justify-center rounded text-muted hover:text-fg"
                  aria-expanded={open}
                  aria-label={t(open ? "workspaces.collapse" : "workspaces.expand", {
                    name: w.name,
                  })}
                  onClick={() => toggle(w.id)}
                >
                  <Chevron open={open} />
                </button>
                <button
                  className={`flex min-w-0 flex-1 items-center gap-2 truncate rounded-md px-2 py-1.5 text-left transition ${
                    current ? "bg-accent font-medium text-white" : "text-fg hover:bg-hover"
                  }`}
                  title={w.path}
                  aria-current={current}
                  onClick={() => {
                    onSelect(w.id);
                    if (!open) toggle(w.id);
                  }}
                >
                  <svg aria-hidden viewBox="0 0 16 16" className="size-3.5 shrink-0 opacity-80">
                    <path
                      fill="currentColor"
                      d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.1l1.5 1.5H13A1.5 1.5 0 0 1 14.5 5.5v6.5A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12z"
                    />
                  </svg>
                  <span className="truncate">{w.name}</span>
                </button>
                <button
                  className="ml-0.5 rounded-full px-1.5 text-muted opacity-0 transition group-hover:opacity-100 hover:bg-hover hover:text-fg focus:opacity-100"
                  aria-label={t("workspaces.remove", { name: w.name })}
                  title={t("workspaces.remove", { name: w.name })}
                  onClick={() => onRemove(w.id)}
                >
                  ×
                </button>
              </div>
              {open && <WorkspaceTeams workspaceId={w.id} focus={focus} onFocus={onFocus} />}
            </li>
          );
        })}
      </ul>
      <form onSubmit={submit} className="border-t border-line p-3">
        <label className="mb-1.5 block text-[11px] font-medium text-muted" htmlFor="ws-path">
          {t("workspaces.addLabel")}
        </label>
        <div className="flex gap-1">
          <input
            id="ws-path"
            className={`min-w-0 flex-1 px-2 py-1 text-xs ${field}`}
            placeholder="C:\projetos\app"
            value={path}
            onChange={(e) => setPath(e.target.value)}
          />
          <button className={`text-xs ${primaryButton}`} type="submit">
            {t("workspaces.add")}
          </button>
        </div>
        {error && (
          <p className="mt-1.5 text-xs text-danger" role="alert">
            {error}
          </p>
        )}
      </form>
    </aside>
  );
}
