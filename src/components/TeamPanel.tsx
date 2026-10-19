import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ghostButton, field, primaryButton } from "./ui";
import { usePersistedState } from "../lib/usePersistedState";
import { TEAMS_CHANGED } from "../lib/usePolled";
import { spawnAgent, spawnShell, type Spawner } from "../lib/pty";
import {
  type TaskStatus,
  AGENT_CLIS,
  type AgentCli,
  approvePlan,
  checkText,
  generatePlan,
  listTeams,
  planDraft,
  ORCHESTRATOR_ID,
  workspaceTrusted,
  type PlanDraft,
} from "../lib/team";
import { IntegrationBar } from "./IntegrationBar";
import { InterfaceView, type Member } from "./InterfaceView";
import { ReadyBanner } from "./ReadyBanner";
import { TeamProgress } from "./TeamProgress";
import { PreviewView } from "./PreviewView";
import { TemplateChips } from "./Templates";
import { notify } from "../lib/notify";
import { MessagePanel } from "./MessagePanel";
import { TaskPanel, useTeamTasks } from "./TaskPanel";
import { TerminalPane, type PendingPermission } from "./TerminalPane";

type Stage =
  | { kind: "request" }
  | { kind: "planning" }
  | { kind: "review"; draft: PlanDraft }
  | { kind: "running"; draft: PlanDraft; resumed: boolean };

const AUTO_START_KEY = "ina.autoStart";

/** Missing or anything but "off" means on. */
const parseAutoStart = (v: string | null) => v !== "off";

const SPLIT_KEY = "ina.teamSplit";
const VIEW_KEY = "ina.view";
const VIEWS = ["interface", "terminals", "preview"] as const;
const parseView = (v: string | null): (typeof VIEWS)[number] =>
  v === "terminals" ? "terminals" : v === "preview" ? "preview" : "interface";
const SPLIT_MIN = 10;
const SPLIT_MAX = 70;

/** Height of the task/message area, in % of the team view. */
function parseSplit(v: string | null): number {
  const n = Number(v);
  return v !== null && n >= SPLIT_MIN && n <= SPLIT_MAX ? n : 40;
}

/** Drag handle that resizes the area above it as a % of `container`'s height. */
function Splitter({
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
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
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

const button =
  "aero-pill px-3 py-1 text-xs font-medium transition hover:bg-hover active:scale-[0.98] disabled:opacity-40 disabled:hover:bg-panel";

function AgentTerminal({
  planId,
  agentId,
  title,
  subtitle,
  open,
  onOpen,
  waitingFor,
  status,
  maximized,
  onToggleMaximize,
  onPermission,
}: {
  planId: number;
  agentId: string;
  onPermission: (agentId: string, pending: PendingPermission | null) => void;
  title: string;
  subtitle: string;
  maximized: boolean;
  onToggleMaximize: () => void;
  /** Session started; owned by TeamRun so the interface view can start it too. */
  open: boolean;
  onOpen: () => void;
  /** Prerequisites not done yet; the backend refuses to start until they are. */
  waitingFor: string[];
  status?: TaskStatus;
}) {
  const { t } = useTranslation();
  const spawn = useMemo<Spawner>(
    () => (id, cols, rows, onEvent) => spawnAgent(planId, agentId, id, cols, rows, onEvent),
    [planId, agentId],
  );
  const permission = useCallback(
    (pending: PendingPermission | null) => onPermission(agentId, pending),
    [onPermission, agentId],
  );
  if (!open) {
    return (
      <section className="flex h-full min-h-0 flex-col items-start gap-2 aero-glass rounded-2xl p-4  text-xs">
        <span className="font-medium">{title}</span>
        <span className="text-muted">{subtitle}</span>
        {status === "blocked" ? (
          <span className="text-warning">{t("team.blocked")}</span>
        ) : (
          waitingFor.length > 0 && (
            <span className="text-warning">
              {t("team.waitingFor", { agents: waitingFor.join(", ") })}
            </span>
          )
        )}
        <button className={button} onClick={onOpen}>
          {t("team.openTerminal")}
        </button>
      </section>
    );
  }
  return (
    <TerminalPane
      title={title}
      subtitle={subtitle}
      spawn={spawn}
      onPermission={permission}
      actions={
        <button className={ghostButton} onClick={onToggleMaximize}>
          {t(maximized ? "terminal.restore" : "terminal.maximize")}
        </button>
      }
    />
  );
}

/** An approved team: integration, task registry and one terminal per session. */
function TeamRun({
  draft,
  resumed,
  focusAgent,
  workspaceId,
}: {
  workspaceId: number;
  draft: PlanDraft;
  resumed: boolean;
  /** Member picked in the sidebar (the orchestrator if none). */
  focusAgent: string | null;
}) {
  const { t } = useTranslation();
  const { id, plan: p } = draft;
  const { tasks, error, refresh } = useTeamTasks(id);
  const [maxId, setMaxId] = useState<string | null>(null);
  const [autoStart, changeAutoStart] = usePersistedState(AUTO_START_KEY, parseAutoStart, (on) =>
    on ? "on" : "off",
  );
  const view = useRef<HTMLDivElement>(null);
  const [split, changeSplit] = usePersistedState(SPLIT_KEY, parseSplit);
  const status = new Map(tasks.map((x) => [x.agentId, x.status]));
  const prerequisites = new Map(tasks.map((x) => [x.agentId, x.prerequisites]));
  const count = p.agents.length + 1;
  const cols = maxId !== null ? 1 : count <= 2 ? count : count <= 4 ? 2 : 3;
  // Hidden, not unmounted: a maximized pane must not kill the other sessions.
  const cell = (agentId: string) => (maxId !== null && maxId !== agentId ? "hidden" : "min-h-0");
  const toggle = (agentId: string) => () => setMaxId((cur) => (cur === agentId ? null : agentId));
  const [mode, changeMode] = usePersistedState(VIEW_KEY, parseView);
  const [merged, setMerged] = usePersistedState(
    `ina.merged.${id}`,
    (v) => v === "1",
    (v) => (v ? "1" : ""),
  );
  const [permissions, setPermissions] = useState<Record<string, PendingPermission>>({});
  const onPermission = useCallback(
    (agentId: string, pending: PendingPermission | null) => {
      if (pending && !document.hasFocus()) {
        const name =
          agentId === ORCHESTRATOR_ID
            ? t("team.orchestrator")
            : (p.agents.find((a) => a.id === agentId)?.name ?? agentId);
        void notify(t("notify.permission.title", { name }), t("notify.permission.body"));
      }
      setPermissions((all) => {
        if (!pending) {
          if (!(agentId in all)) return all;
          return Object.fromEntries(Object.entries(all).filter(([id]) => id !== agentId));
        }
        return { ...all, [agentId]: pending };
      });
    },
    [t, p.agents],
  );
  // Picking a member in the sidebar means talking to it: show the Interface.
  const [seenFocus, setSeenFocus] = useState<string | null>(null);
  if (focusAgent !== seenFocus) {
    setSeenFocus(focusAgent);
    if (focusAgent !== null) changeMode("interface");
  }
  // Resumed teams open sessions on demand, so the task is not re-sent unasked.
  const [opened, setOpened] = useState<ReadonlySet<string>>(
    () =>
      new Set(
        resumed
          ? []
          : [
              ORCHESTRATOR_ID,
              ...p.agents
                .filter((a) => (prerequisites.get(a.id) ?? []).length === 0)
                .map((a) => a.id),
            ],
      ),
  );
  const openSession = useCallback(
    (agentId: string) => setOpened((cur) => (cur.has(agentId) ? cur : new Set([...cur, agentId]))),
    [],
  );
  const waiting = (agentId: string) =>
    (prerequisites.get(agentId) ?? []).filter((d) => status.get(d) !== "done");
  // Auto-start once prerequisites are done. Latched: never closes a session.
  const autoOpen = p.agents
    .filter((a) => {
      const prereqs = prerequisites.get(a.id) ?? [];
      return (
        autoStart &&
        prereqs.length > 0 &&
        waiting(a.id).length === 0 &&
        status.get(a.id) === "pending" &&
        !opened.has(a.id)
      );
    })
    .map((a) => a.id)
    .join(",");
  useEffect(() => {
    if (autoOpen) autoOpen.split(",").forEach(openSession);
  }, [autoOpen, openSession]);
  const members: Member[] = [
    {
      id: ORCHESTRATOR_ID,
      name: t("team.orchestrator"),
      subtitle: p.summary,
      task: p.summary,
      cli: "claude",
      open: opened.has(ORCHESTRATOR_ID),
      permission: permissions[ORCHESTRATOR_ID],
      waitingFor: [],
    },
    ...p.agents.map((a) => ({
      id: a.id,
      name: a.name,
      subtitle: a.role,
      task: a.task,
      cli: a.cli,
      open: opened.has(a.id),
      permission: permissions[a.id],
      status: status.get(a.id),
      waitingFor: waiting(a.id),
    })),
  ];
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <TeamProgress tasks={tasks} agents={p.agents.length} merged={merged} />
      <ReadyBanner planId={id} tasks={tasks} onMerged={() => setMerged(true)} />
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <IntegrationBar planId={id} />
        </div>
        <div
          role="radiogroup"
          aria-label={t("activity.view")}
          className="aero-pill flex shrink-0 p-0.5 text-[12px] font-medium"
        >
          {VIEWS.map((v) => (
            <button
              key={v}
              role="radio"
              aria-checked={mode === v}
              onClick={() => changeMode(v)}
              className={`rounded-full px-3 py-1 transition ${mode === v ? "aero-btn" : "text-fg-2 hover:text-fg"}`}
            >
              <span className="relative">{t(`activity.views.${v}`)}</span>
            </button>
          ))}
        </div>
      </div>
      {/* Both views stay mounted: switching keeps the selected member, drafts and sessions. */}
      {mode === "preview" && <PreviewView workspaceId={workspaceId} />}
      <div className={`min-h-0 flex-1 ${mode === "interface" ? "" : "hidden"}`}>
        <InterfaceView
          planId={id}
          members={members}
          selected={focusAgent ?? ORCHESTRATOR_ID}
          visible={mode === "interface"}
          onOpen={openSession}
          onShowTerminals={() => changeMode("terminals")}
        />
      </div>
      {/* Hidden, not unmounted: the interface reads sessions that keep running here. */}
      <div
        ref={view}
        className={`flex min-h-0 flex-1 flex-col gap-1 ${mode === "terminals" ? "" : "hidden"}`}
      >
        <div
          className="grid min-h-0 shrink-0 grid-cols-2 gap-2 overflow-hidden"
          style={{ height: `${split}%` }}
        >
          <div className="min-h-0 overflow-y-auto">
            <TaskPanel
              agents={p.agents}
              tasks={tasks}
              error={error}
              onRefresh={() => void refresh()}
              autoStart={autoStart}
              onAutoStart={changeAutoStart}
            />
          </div>
          <MessagePanel planId={id} agents={p.agents} />
        </div>
        <Splitter container={view} value={split} onChange={changeSplit} />
        <div
          className="grid min-h-0 flex-1 gap-2"
          style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
        >
          <div className={cell(ORCHESTRATOR_ID)}>
            <AgentTerminal
              planId={id}
              agentId={ORCHESTRATOR_ID}
              title={t("team.orchestrator")}
              subtitle={p.summary}
              open={opened.has(ORCHESTRATOR_ID)}
              onOpen={() => openSession(ORCHESTRATOR_ID)}
              waitingFor={[]}
              maximized={maxId === ORCHESTRATOR_ID}
              onToggleMaximize={toggle(ORCHESTRATOR_ID)}
              onPermission={onPermission}
            />
          </div>
          {p.agents.map((a) => {
            return (
              <div key={a.id} className={cell(a.id)}>
                <AgentTerminal
                  planId={id}
                  agentId={a.id}
                  title={a.name}
                  subtitle={a.role}
                  open={opened.has(a.id)}
                  onOpen={() => openSession(a.id)}
                  waitingFor={waiting(a.id)}
                  status={status.get(a.id)}
                  maximized={maxId === a.id}
                  onToggleMaximize={toggle(a.id)}
                  onPermission={onPermission}
                />
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

export function TeamPanel({
  workspaceId,
  workspaceName,
  workspacePath,
  started,
  focus,
}: {
  workspaceId: number;
  workspaceName: string;
  workspacePath: string;
  /** Team approved elsewhere (Home): open it with its sessions starting now. */
  started?: PlanDraft;
  /** Team member picked in the sidebar: opens its team. */
  focus: { planId: number; agentId: string } | null;
}) {
  const { t } = useTranslation();
  const [request, setRequest] = useState("");
  const [stage, setStage] = useState<Stage>(
    started ? { kind: "running", draft: started, resumed: false } : { kind: "request" },
  );
  const [error, setError] = useState<string | null>(null);
  const [untrusted, setUntrusted] = useState(false);
  const [openTeams, setOpenTeams] = useState<PlanDraft[]>([]);

  useEffect(() => {
    const loadTeams = () =>
      listTeams(workspaceId)
        .then(setOpenTeams)
        .catch((e: unknown) => setError(String(e)));
    void loadTeams();
    window.addEventListener(TEAMS_CHANGED, loadTeams);
    // A plan generated before the window reloaded is shown again for review.
    planDraft(workspaceId)
      .then((d) => d && setStage((s) => (s.kind === "request" ? { kind: "review", draft: d } : s)))
      .catch((e: unknown) => setError(String(e)));
    return () => window.removeEventListener(TEAMS_CHANGED, loadTeams);
  }, [workspaceId]);
  const shell = useMemo<Spawner>(
    () => (id, cols, rows, onEvent) => spawnShell(workspaceId, id, cols, rows, onEvent),
    [workspaceId],
  );

  const plan = async () => {
    setError(null);
    setStage({ kind: "planning" });
    try {
      setStage({ kind: "review", draft: await generatePlan(workspaceId, request) });
    } catch (e) {
      setError(String(e));
      setStage({ kind: "request" });
    }
  };

  const approve = async (draft: PlanDraft) => {
    setError(null);
    try {
      // Agents would hang on Claude Code's trust prompt; the user trusts the folder outside INA.
      const trusted = await workspaceTrusted(workspaceId);
      setUntrusted(!trusted);
      if (!trusted) return;
      await approvePlan(draft.id, Object.fromEntries(draft.plan.agents.map((a) => [a.id, a.cli])));
      window.dispatchEvent(new Event(TEAMS_CHANGED));
      setStage({ kind: "running", draft, resumed: false });
    } catch (e) {
      setError(String(e));
    }
  };

  const setCli = (draft: PlanDraft, id: string, cli: AgentCli) => {
    const agents = draft.plan.agents.map((a) => (a.id === id ? { ...a, cli } : a));
    setStage({ kind: "review", draft: { ...draft, plan: { ...draft.plan, agents } } });
  };

  // A member picked in the sidebar opens its team (sessions on demand).
  const focused = focus && openTeams.find((d) => d.id === focus.planId);
  if (focused && (stage.kind !== "running" || stage.draft.id !== focused.id)) {
    setStage({ kind: "running", draft: focused, resumed: true });
  }

  if (stage.kind === "running") {
    return (
      <TeamRun
        key={stage.draft.id}
        draft={stage.draft}
        resumed={stage.resumed}
        focusAgent={focus?.planId === stage.draft.id ? focus.agentId : null}
        workspaceId={workspaceId}
      />
    );
  }

  return (
    <div className="flex h-full min-h-0 gap-2">
      <section className="flex w-[28rem] shrink-0 flex-col gap-3 overflow-y-auto aero-glass rounded-2xl p-4  text-sm">
        <h2 className="text-[15px] font-semibold tracking-tight text-fg">{t("team.title")}</h2>
        {openTeams.length > 0 && (
          <div className="flex flex-col gap-1 text-xs">
            <span className="text-muted">{t("team.openTeams")}</span>
            {openTeams.map((d) => (
              <div
                key={d.id}
                className="flex items-center gap-2 rounded-lg bg-hover p-2.5 ring-1 ring-line"
              >
                <span className="min-w-0 flex-1 truncate" title={d.plan.summary}>
                  {d.plan.summary}
                </span>
                <span className="text-muted">
                  {t("team.agents", { count: d.plan.agents.length })}
                </span>
                <button
                  className={button}
                  onClick={() => setStage({ kind: "running", draft: d, resumed: true })}
                >
                  {t("team.resume")}
                </button>
              </div>
            ))}
          </div>
        )}
        <TemplateChips where="team" disabled={stage.kind === "planning"} onPick={setRequest} />
        <label className="flex flex-col gap-1 text-xs text-muted">
          {t("team.requestLabel")}
          <textarea
            className={`min-h-28 resize-y p-2 text-sm ${field}`}
            value={request}
            placeholder={t("team.requestPlaceholder")}
            disabled={stage.kind === "planning"}
            onChange={(e) => setRequest(e.target.value)}
          />
        </label>
        <div className="flex gap-2">
          <button
            className={`text-xs ${primaryButton}`}
            disabled={!request.trim() || stage.kind === "planning"}
            onClick={() => void plan()}
          >
            {stage.kind === "planning" ? t("team.planning") : t("team.generate")}
          </button>
          {stage.kind === "review" && (
            <button
              className={`text-xs ${primaryButton}`}
              onClick={() => void approve(stage.draft)}
            >
              {untrusted
                ? t("team.recheck")
                : t("team.approve", { count: stage.draft.plan.agents.length + 1 })}
            </button>
          )}
        </div>
        {stage.kind === "review" && stage.draft.plan.agents.some((a) => a.verify.length > 0) && (
          <p className="text-xs text-muted">{t("team.verifyNote")}</p>
        )}
        {untrusted && stage.kind === "review" && (
          <div
            role="alert"
            className="flex flex-col gap-1 rounded-lg bg-hover p-2.5 ring-1 ring-line text-xs"
          >
            <p className="text-fg">{t("team.untrusted")}</p>
            <code className="text-muted">cd &quot;{workspacePath}&quot; ; claude</code>
            <p className="text-muted">{t("team.untrustedHint")}</p>
          </div>
        )}
        {error && (
          <p role="alert" className="break-words text-xs text-danger">
            {error}
          </p>
        )}
        {stage.kind === "review" && (
          <div className="flex flex-col gap-2">
            <p>{stage.draft.plan.summary}</p>
            <p className="text-xs text-muted">{t(`team.mode.${stage.draft.plan.mode}`)}</p>
            <ol className="flex flex-col gap-2">
              {stage.draft.plan.agents.map((a) => (
                <li key={a.id} className="rounded-lg bg-hover p-2.5 ring-1 ring-line text-xs">
                  <div className="flex items-center gap-2 font-medium text-fg">
                    <span className="flex-1">
                      {a.name} <span className="text-muted">· {a.role}</span>
                    </span>
                    <label className="flex items-center gap-1 text-[11px] font-normal text-muted">
                      {t("team.cli")}
                      <select
                        className="aero-pill px-2 py-0.5 text-[11px] text-fg outline-none"
                        value={a.cli}
                        onChange={(e) => setCli(stage.draft, a.id, e.target.value as AgentCli)}
                      >
                        {AGENT_CLIS.map((c) => (
                          <option key={c} value={c}>
                            {t(`team.clis.${c}`)}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                  <div className="mt-1">{a.task}</div>
                  <div className="mt-1 text-muted">
                    {t("team.scope")}: {a.scope}
                  </div>
                  {a.dependsOn.length > 0 && (
                    <div className="text-muted">
                      {t("team.dependsOn")}: {a.dependsOn.join(", ")}
                    </div>
                  )}
                  <ul className="mt-1 list-disc pl-4 text-muted">
                    {a.acceptance.map((c) => (
                      <li key={c}>{c}</li>
                    ))}
                  </ul>
                  <div className="mt-1 text-xs">
                    <span className="text-muted">{t("team.verify")}: </span>
                    {a.verify.length === 0 ? (
                      <span className="text-warning">{t("team.noVerify")}</span>
                    ) : (
                      a.verify.map((c) => (
                        <code key={checkText(c)} className="mr-2 rounded bg-line px-1 text-fg">
                          {checkText(c)}
                        </code>
                      ))
                    )}
                  </div>
                </li>
              ))}
            </ol>
          </div>
        )}
      </section>
      <div className="min-h-0 min-w-0 flex-1">
        <TerminalPane title={workspaceName} spawn={shell} />
      </div>
    </div>
  );
}
