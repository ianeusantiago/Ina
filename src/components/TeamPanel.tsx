import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { spawnAgent, spawnShell, type Spawner } from "../lib/pty";
import {
  approvePlan,
  generatePlan,
  ORCHESTRATOR_ID,
  workspaceTrusted,
  type PlanDraft,
} from "../lib/team";
import { IntegrationBar } from "./IntegrationBar";
import { TerminalPane } from "./TerminalPane";

type Stage =
  | { kind: "request" }
  | { kind: "planning" }
  | { kind: "review"; draft: PlanDraft }
  | { kind: "running"; draft: PlanDraft };

const button =
  "rounded border border-line px-3 py-1 text-xs hover:bg-line disabled:opacity-40 disabled:hover:bg-transparent";

function AgentTerminal({
  planId,
  agentId,
  title,
  subtitle,
}: {
  planId: number;
  agentId: string;
  title: string;
  subtitle: string;
}) {
  const spawn = useMemo<Spawner>(
    () => (id, cols, rows, onEvent) => spawnAgent(planId, agentId, id, cols, rows, onEvent),
    [planId, agentId],
  );
  return <TerminalPane title={title} subtitle={subtitle} spawn={spawn} />;
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
      setStage({ kind: "running", draft });
    } catch (e) {
      setError(String(e));
    }
  };

  if (stage.kind === "running") {
    const { id, plan: p } = stage.draft;
    const count = p.agents.length + 1;
    const cols = count <= 2 ? count : count <= 4 ? 2 : 3;
    return (
      <div className="flex h-full min-h-0 flex-col gap-2">
        <IntegrationBar planId={id} />
        <div
          className="grid min-h-0 flex-1 gap-2"
          style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
        >
          <AgentTerminal
            planId={id}
            agentId={ORCHESTRATOR_ID}
            title={t("team.orchestrator")}
            subtitle={p.summary}
          />
          {p.agents.map((a) => (
            <AgentTerminal key={a.id} planId={id} agentId={a.id} title={a.name} subtitle={a.role} />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 gap-2">
      <section className="flex w-[28rem] shrink-0 flex-col gap-3 overflow-y-auto rounded border border-line bg-panel p-3 text-sm">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-muted">
          {t("team.title")}
        </h2>
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
