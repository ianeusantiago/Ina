import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { usePersistedState } from "../lib/usePersistedState";
import { ORCHESTRATOR_ID, teamResumable, type PlanDraft } from "../lib/team";
import { IntegrationBar } from "./IntegrationBar";
import { InterfaceView, type Member } from "./InterfaceView";
import { ReadyBanner } from "./ReadyBanner";
import { TeamProgress } from "./TeamProgress";
import { PreviewView } from "./PreviewView";
import { notify } from "../lib/notify";
import { MessagePanel } from "./MessagePanel";
import { TaskPanel, useTeamTasks } from "./TaskPanel";
import { type PendingPermission } from "./TerminalPane";
import { AgentTerminal } from "./AgentTerminal";
import { parseSplit, Splitter } from "./Splitter";
import { Segmented } from "./Segmented";

const AUTO_START_KEY = "ina.autoStart";

/** Missing or anything but "off" means on. */
const parseAutoStart = (v: string | null) => v !== "off";

const SPLIT_KEY = "ina.teamSplit";
const VIEW_KEY = "ina.view";
const VIEWS = ["interface", "terminals", "preview"] as const;
const parseView = (v: string | null): (typeof VIEWS)[number] =>
  v === "terminals" ? "terminals" : v === "preview" ? "preview" : "interface";

/** An approved team: integration, task registry and one terminal per session. */
export function TeamRun({
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
  const [resumed_, setResumed] = useState<ReadonlySet<string>>(() => new Set());
  const [resumable, setResumable] = useState<ReadonlySet<string>>(() => new Set());
  useEffect(() => {
    teamResumable(id)
      .then((ids) => setResumable(new Set(ids)))
      .catch((e: unknown) => console.error("[resume]", e));
  }, [id]);
  const openSession = useCallback((agentId: string, resume = false) => {
    if (resume) setResumed((cur) => new Set([...cur, agentId]));
    setOpened((cur) => (cur.has(agentId) ? cur : new Set([...cur, agentId])));
  }, []);
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
    if (autoOpen) autoOpen.split(",").forEach((a) => openSession(a));
  }, [autoOpen, openSession]);
  const members: Member[] = [
    {
      id: ORCHESTRATOR_ID,
      name: t("team.orchestrator"),
      subtitle: p.summary,
      task: p.summary,
      cli: "claude",
      open: opened.has(ORCHESTRATOR_ID),
      canResume: resumable.has(ORCHESTRATOR_ID),
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
      canResume: resumable.has(a.id),
      permission: permissions[a.id],
      status: status.get(a.id),
      waitingFor: waiting(a.id),
    })),
  ];
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <TeamProgress planId={id} tasks={tasks} agents={p.agents.length} merged={merged} />
      <ReadyBanner planId={id} tasks={tasks} onMerged={() => setMerged(true)} />
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <IntegrationBar planId={id} />
        </div>
        <Segmented
          variant="pill"
          label={t("activity.view")}
          options={VIEWS}
          value={mode}
          onChange={changeMode}
          optionLabel={(v) => t(`activity.views.${v}`)}
        />
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
              onOpen={(r) => openSession(ORCHESTRATOR_ID, r)}
              resume={resumed_.has(ORCHESTRATOR_ID)}
              canResume={resumable.has(ORCHESTRATOR_ID)}
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
                  onOpen={(r) => openSession(a.id, r)}
                  resume={resumed_.has(a.id)}
                  canResume={resumable.has(a.id)}
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
