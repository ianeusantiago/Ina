import { useEffect, useRef, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import inaIcon from "../assets/ina-icon.svg";
import { createWorkspace, homeInfo, homeIntent, type HomeBase } from "../lib/home";
import { approvePlan, generatePlan, workspaceTrusted, type PlanDraft } from "../lib/team";
import { TEAMS_CHANGED } from "../lib/usePolled";
import type { Workspace } from "../lib/workspaces";
import { addHomeItem, newHomeConversation, setHome, useHome } from "../lib/homeStore";
import { Bubble } from "./InterfaceView";
import { errorText, field, ghostButton, primaryButton } from "./ui";

/** Home: talk to Claude; it can create a workspace and set up a team for a request. */
export function Home({
  onWorkspacesChanged,
  onStarted,
}: {
  onWorkspacesChanged: () => void;
  /** The team was approved: open its workspace with the sessions running. */
  onStarted: (workspace: Workspace, draft: PlanDraft) => void;
}) {
  const { t } = useTranslation();
  const { items, draft, busy, error } = useHome();
  const setDraft = (v: string) => setHome({ draft: v });
  const setBusy = (v: string | null) => setHome({ busy: v });
  const setError = (v: string | null) => setHome({ error: v });
  const [base, setBase] = useState<HomeBase | null>(null);
  const scroller = useRef<HTMLDivElement>(null);

  useEffect(() => {
    homeInfo()
      .then(setBase)
      .catch((e: unknown) => setError(String(e)));
  }, []);
  useEffect(() => {
    const el = scroller.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [items.length, busy]);

  const add = addHomeItem;

  /** Plan the team in the new workspace (needs the base folder trusted in Claude Code). */
  const plan = async (workspace: Workspace, task: string) => {
    if (!(await workspaceTrusted(workspace.id))) {
      add({ kind: "trust", path: base?.path ?? workspace.path, workspace, task });
      return;
    }
    setBusy(t("home.planning"));
    add({ kind: "plan", workspace, draft: await generatePlan(workspace.id, task), started: false });
  };

  const step = async (action: () => Promise<void>) => {
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const send = (e: FormEvent) => {
    e.preventDefault();
    const text = draft.trim();
    if (!text || busy) return;
    setDraft("");
    add({ kind: "user", text });
    void step(async () => {
      setBusy(t("home.thinking"));
      const intent = await homeIntent(text);
      add({ kind: "agent", text: intent.reply });
      if (intent.action !== "create") return;
      setBusy(t("home.creating", { name: intent.workspaceName }));
      const workspace = await createWorkspace(intent.workspaceName);
      onWorkspacesChanged();
      await plan(workspace, intent.task);
    });
  };

  const start = (index: number, workspace: Workspace, d: PlanDraft) =>
    void step(async () => {
      setBusy(t("home.starting"));
      await approvePlan(d.id, Object.fromEntries(d.plan.agents.map((a) => [a.id, a.cli])));
      window.dispatchEvent(new Event(TEAMS_CHANGED));
      setHome((s) => ({
        items: s.items.map((it, i) =>
          i === index && it.kind === "plan" ? { ...it, started: true } : it,
        ),
      }));
      onStarted(workspace, d);
    });

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto px-6 py-6">
        <div className="mx-auto flex max-w-2xl flex-col gap-3">
          {items.length === 0 && (
            <div className="flex flex-col items-center gap-3 pt-[12vh] text-center">
              <img src={inaIcon} alt="" className="size-20 drop-shadow-lg" />
              <p className="text-[24px] font-semibold tracking-tight text-fg">{t("home.title")}</p>
              <p className="max-w-md text-[13px] text-muted">{t("home.hint")}</p>
              <div className="mt-2 flex flex-wrap justify-center gap-2">
                {[t("home.example1"), t("home.example2")].map((ex) => (
                  <button
                    key={ex}
                    className={`aero-card px-3 py-1.5 text-[12px] text-fg-2 ${ghostButton}`}
                    onClick={() => setDraft(ex)}
                  >
                    {ex}
                  </button>
                ))}
              </div>
            </div>
          )}
          {items.map((it, i) =>
            it.kind === "user" || it.kind === "agent" ? (
              <Bubble key={i} role={it.kind}>
                {it.text}
              </Bubble>
            ) : it.kind === "trust" ? (
              <div key={i} role="alert" className="aero-card flex flex-col gap-2 px-4 py-3 text-xs">
                <p className="text-fg">{t("home.trust")}</p>
                <code className="text-muted">cd &quot;{it.path}&quot; ; claude</code>
                <button
                  className={`self-start text-xs ${primaryButton}`}
                  disabled={busy !== null}
                  onClick={() => void step(() => plan(it.workspace, it.task))}
                >
                  {t("home.trustRetry")}
                </button>
              </div>
            ) : (
              <div key={i} className="aero-card flex flex-col gap-2 px-4 py-3 text-[13px]">
                <p className="text-[11px] font-semibold text-muted">
                  {t("home.planFor", { name: it.workspace.name })}
                </p>
                <p className="text-fg">{it.draft.plan.summary}</p>
                <ul className="flex flex-col gap-1 text-xs">
                  <li className="text-fg">
                    <span className="font-medium">{t("team.orchestrator")}</span>
                    <span className="text-muted"> · {t("home.orchestratorRole")}</span>
                  </li>
                  {it.draft.plan.agents.map((a) => (
                    <li key={a.id} className="text-fg">
                      <span className="font-medium">{a.name}</span>
                      <span className="text-muted"> · {a.role}</span>
                    </li>
                  ))}
                </ul>
                {it.started ? (
                  <p className="text-xs text-success">{t("home.started")}</p>
                ) : (
                  <button
                    className={`self-start text-xs ${primaryButton}`}
                    disabled={busy !== null}
                    onClick={() => start(i, it.workspace, it.draft)}
                  >
                    {t("home.start", { count: it.draft.plan.agents.length + 1 })}
                  </button>
                )}
              </div>
            ),
          )}
          {busy && (
            <p role="status" className="self-start text-xs text-muted">
              {busy}
            </p>
          )}
          {error && (
            <p role="alert" className={`text-xs ${errorText}`}>
              {error}
            </p>
          )}
        </div>
      </div>
      {items.length > 0 && (
        <div className="mx-auto flex w-full max-w-2xl justify-end px-6 pb-2">
          <button
            className={`text-xs ${ghostButton}`}
            disabled={busy !== null}
            onClick={newHomeConversation}
          >
            {t("home.newConversation")}
          </button>
        </div>
      )}
      <form onSubmit={send} className="px-6 pb-5">
        <div className={`aero-card mx-auto flex max-w-2xl items-end gap-2 p-2 ${field}`}>
          <textarea
            rows={2}
            aria-label={t("home.inputLabel")}
            className="max-h-40 min-h-10 flex-1 resize-none bg-transparent px-2 py-1.5 text-[13.5px] outline-none placeholder:text-muted"
            placeholder={t("home.placeholder")}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                e.currentTarget.form?.requestSubmit();
              }
            }}
          />
          <button
            type="submit"
            className={`text-xs ${primaryButton}`}
            disabled={!draft.trim() || busy !== null}
          >
            {t("home.send")}
          </button>
        </div>
      </form>
    </div>
  );
}
