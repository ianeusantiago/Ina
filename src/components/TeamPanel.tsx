import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { field, pillButton, primaryButton } from "./ui";
import { TEAMS_CHANGED } from "../lib/usePolled";
import { spawnShell, type Spawner } from "../lib/pty";
import {
  AGENT_CLIS,
  type AgentCli,
  approvePlan,
  checkText,
  generatePlan,
  listTeams,
  planDraft,
  workspaceTrusted,
  type PlanDraft,
} from "../lib/team";
import { TemplateChips } from "./Templates";
import { TerminalPane } from "./TerminalPane";
import { TeamRun } from "./TeamRun";

type Stage =
  | { kind: "request" }
  | { kind: "planning" }
  | { kind: "review"; draft: PlanDraft }
  | { kind: "running"; draft: PlanDraft; resumed: boolean };

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
                  className={pillButton}
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
