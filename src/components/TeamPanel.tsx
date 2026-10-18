import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ghostButton } from "./ui";
import { spawnAgent, spawnShell, type Spawner } from "../lib/pty";
import {
  type TaskStatus,
  approvePlan,
  checkText,
  generatePlan,
  listTeams,
  ORCHESTRATOR_ID,
  prerequisites,
  workspaceTrusted,
  type PlanDraft,
} from "../lib/team";
import { IntegrationBar } from "./IntegrationBar";
import { MessagePanel } from "./MessagePanel";
import { TaskPanel, useTeamTasks } from "./TaskPanel";
import { TerminalPane } from "./TerminalPane";

type Stage =
  | { kind: "request" }
  | { kind: "planning" }
  | { kind: "review"; draft: PlanDraft }
  | { kind: "running"; draft: PlanDraft; resumed: boolean };

const AUTO_START_KEY = "ina.autoStart";

function savedAutoStart(): boolean {
  try {
    return localStorage.getItem(AUTO_START_KEY) !== "off";
  } catch {
    return true;
  }
}

const button =
  "rounded border border-line px-3 py-1 text-xs hover:bg-line disabled:opacity-40 disabled:hover:bg-transparent";

function AgentTerminal({
  planId,
  agentId,
  title,
  subtitle,
  lazy,
  waitingFor,
  autoOpen,
  status,
  maximized,
  onToggleMaximize,
}: {
  planId: number;
  agentId: string;
  title: string;
  subtitle: string;
  maximized: boolean;
  onToggleMaximize: () => void;
  /** Resumed teams open sessions on demand, so the task is not re-sent unasked. */
  lazy: boolean;
  /** Prerequisites not done yet; the backend refuses to start until they are. */
  waitingFor: string[];
  /** Open now (auto-start once prerequisites are done). Latched: never closes a session. */
  autoOpen: boolean;
  status?: TaskStatus;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(!lazy);
  if (autoOpen && !open) setOpen(true);
  const spawn = useMemo<Spawner>(
    () => (id, cols, rows, onEvent) => spawnAgent(planId, agentId, id, cols, rows, onEvent),
    [planId, agentId],
  );
  if (!open) {
    return (
      <section className="flex h-full min-h-0 flex-col items-start gap-2 rounded border border-line bg-panel p-3 text-xs">
        <span className="font-medium">{title}</span>
        <span className="text-muted">{subtitle}</span>
        {status === "blocked" ? (
          <span className="text-amber-300">{t("team.blocked")}</span>
        ) : (
          waitingFor.length > 0 && (
            <span className="text-amber-300">
              {t("team.waitingFor", { agents: waitingFor.join(", ") })}
            </span>
          )
        )}
        <button className={button} onClick={() => setOpen(true)}>
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
      actions={
        <button className={ghostButton} onClick={onToggleMaximize}>
          {t(maximized ? "terminal.restore" : "terminal.maximize")}
        </button>
      }
    />
  );
}

/** An approved team: integration, task registry and one terminal per session. */
function TeamRun({ draft, resumed }: { draft: PlanDraft; resumed: boolean }) {
  const { t } = useTranslation();
  const { id, plan: p } = draft;
  const { tasks, error, refresh } = useTeamTasks(id);
  const [maxId, setMaxId] = useState<string | null>(null);
  const [autoStart, setAutoStart] = useState(savedAutoStart);
  const changeAutoStart = (on: boolean) => {
    setAutoStart(on);
    try {
      localStorage.setItem(AUTO_START_KEY, on ? "on" : "off");
    } catch {
      // Not persisted; applies to this session.
    }
  };
  const status = new Map(tasks.map((x) => [x.agentId, x.status]));
  const count = p.agents.length + 1;
  const cols = maxId !== null ? 1 : count <= 2 ? count : count <= 4 ? 2 : 3;
  // Hidden, not unmounted: a maximized pane must not kill the other sessions.
  const cell = (agentId: string) => (maxId !== null && maxId !== agentId ? "hidden" : "min-h-0");
  const toggle = (agentId: string) => () => setMaxId((cur) => (cur === agentId ? null : agentId));
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <IntegrationBar planId={id} />
      <div className="grid max-h-[40%] min-h-0 grid-cols-2 gap-2 overflow-hidden">
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
            lazy={resumed}
            waitingFor={[]}
            autoOpen={false}
            maximized={maxId === ORCHESTRATOR_ID}
            onToggleMaximize={toggle(ORCHESTRATOR_ID)}
          />
        </div>
        {p.agents.map((a) => {
          const prereqs = prerequisites(p, a.id);
          const waitingFor = prereqs.filter((d) => status.get(d) !== "done");
          const autoOpen =
            autoStart &&
            prereqs.length > 0 &&
            waitingFor.length === 0 &&
            status.get(a.id) === "pending";
          return (
            <div key={a.id} className={cell(a.id)}>
              <AgentTerminal
                planId={id}
                agentId={a.id}
                title={a.name}
                subtitle={a.role}
                lazy={resumed || prereqs.length > 0}
                waitingFor={waitingFor}
                autoOpen={autoOpen}
                status={status.get(a.id)}
                maximized={maxId === a.id}
                onToggleMaximize={toggle(a.id)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}

export function TeamPanel({
  workspaceId,
  workspaceName,
  workspacePath,
}: {
  workspaceId: number;
  workspaceName: string;
  workspacePath: string;
}) {
  const { t } = useTranslation();
  const [request, setRequest] = useState("");
  const [stage, setStage] = useState<Stage>({ kind: "request" });
  const [error, setError] = useState<string | null>(null);
  const [untrusted, setUntrusted] = useState(false);
  const [openTeams, setOpenTeams] = useState<PlanDraft[]>([]);

  useEffect(() => {
    listTeams(workspaceId)
      .then(setOpenTeams)
      .catch((e: unknown) => setError(String(e)));
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
      await approvePlan(draft.id);
      setStage({ kind: "running", draft, resumed: false });
    } catch (e) {
      setError(String(e));
    }
  };

  if (stage.kind === "running") {
    return <TeamRun draft={stage.draft} resumed={stage.resumed} />;
  }

  return (
    <div className="flex h-full min-h-0 gap-2">
      <section className="flex w-[28rem] shrink-0 flex-col gap-3 overflow-y-auto rounded border border-line bg-panel p-3 text-sm">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-muted">
          {t("team.title")}
        </h2>
        {openTeams.length > 0 && (
          <div className="flex flex-col gap-1 text-xs">
            <span className="text-muted">{t("team.openTeams")}</span>
            {openTeams.map((d) => (
              <div key={d.id} className="flex items-center gap-2 rounded border border-line p-2">
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
        <label className="flex flex-col gap-1 text-xs text-muted">
          {t("team.requestLabel")}
          <textarea
            className="min-h-28 resize-y rounded border border-line bg-transparent p-2 text-sm text-zinc-100 outline-none focus:border-zinc-500"
            value={request}
            placeholder={t("team.requestPlaceholder")}
            disabled={stage.kind === "planning"}
            onChange={(e) => setRequest(e.target.value)}
          />
        </label>
        <div className="flex gap-2">
          <button
            className={button}
            disabled={!request.trim() || stage.kind === "planning"}
            onClick={() => void plan()}
          >
            {stage.kind === "planning" ? t("team.planning") : t("team.generate")}
          </button>
          {stage.kind === "review" && (
            <button className={button} onClick={() => void approve(stage.draft)}>
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
          <div role="alert" className="flex flex-col gap-1 rounded border border-line p-2 text-xs">
            <p className="text-zinc-100">{t("team.untrusted")}</p>
            <code className="text-muted">cd &quot;{workspacePath}&quot; ; claude</code>
            <p className="text-muted">{t("team.untrustedHint")}</p>
          </div>
        )}
        {error && (
          <p role="alert" className="break-words text-xs text-red-400">
            {error}
          </p>
        )}
        {stage.kind === "review" && (
          <div className="flex flex-col gap-2">
            <p>{stage.draft.plan.summary}</p>
            <p className="text-xs text-muted">{t(`team.mode.${stage.draft.plan.mode}`)}</p>
            <ol className="flex flex-col gap-2">
              {stage.draft.plan.agents.map((a) => (
                <li key={a.id} className="rounded border border-line p-2 text-xs">
                  <div className="font-medium text-zinc-100">
                    {a.name} <span className="text-muted">· {a.role}</span>
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
                      <span className="text-amber-300">{t("team.noVerify")}</span>
                    ) : (
                      a.verify.map((c) => (
                        <code
                          key={checkText(c)}
                          className="mr-2 rounded bg-line px-1 text-zinc-100"
                        >
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
