import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  teamCleanup,
  teamPreview,
  type IntegrationPreview,
  teamIntegrate,
  teamStatus,
  type AgentBranch,
  type CleanupResult,
  type IntegrateStep,
} from "../lib/team";

type Result =
  | { kind: "status"; rows: AgentBranch[] }
  | { kind: "integrate"; steps: IntegrateStep[] }
  | { kind: "cleanup"; rows: CleanupResult[] };

type Action = "integrate" | "cleanup" | "forceCleanup";

const button =
  "rounded border border-line px-2 py-0.5 text-xs hover:bg-line disabled:opacity-40 disabled:hover:bg-transparent";

/** Branch status, controlled integration and cleanup for an approved team. */
export function IntegrationBar({ planId }: { planId: number }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<Action | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<IntegrationPreview | null>(null);

  const run = async (action: () => Promise<Result>) => {
    setBusy(true);
    setError(null);
    setConfirm(null);
    try {
      setResult(await action());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const status = () => run(async () => ({ kind: "status", rows: await teamStatus(planId) }));
  const integrate = () =>
    run(async () => ({ kind: "integrate", steps: await teamIntegrate(planId) }));
  // Integration is confirmed against a dry run, so the user sees conflicts before merging.
  const askIntegrate = async () => {
    setBusy(true);
    setError(null);
    try {
      setPreview(await teamPreview(planId));
      setConfirm("integrate");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const cleanup = (discardUntracked: boolean) =>
    run(async () => ({ kind: "cleanup", rows: await teamCleanup(planId, discardUntracked) }));
  const confirmed = (action: Action) =>
    action === "integrate" ? integrate() : cleanup(action === "forceCleanup");
  const cleanupFailed = result?.kind === "cleanup" && result.rows.some((r) => r.error);

  return (
    <section className="flex flex-col gap-1 rounded border border-line bg-panel px-3 py-2 text-xs">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-semibold uppercase tracking-wide text-muted">
          {t("integration.title")}
        </span>
        <button className={button} disabled={busy} onClick={() => void status()}>
          {t("integration.status")}
        </button>
        {confirm === null ? (
          <>
            <button className={button} disabled={busy} onClick={() => void askIntegrate()}>
              {t("integration.integrate")}
            </button>
            <button className={button} disabled={busy} onClick={() => setConfirm("cleanup")}>
              {t("integration.cleanup")}
            </button>
            {cleanupFailed && (
              <button
                className={`${button} border-red-900 text-red-300`}
                disabled={busy}
                onClick={() => setConfirm("forceCleanup")}
              >
                {t("integration.forceCleanup")}
              </button>
            )}
          </>
        ) : (
          <>
            <span className="text-zinc-100">{t(`integration.confirm.${confirm}`)}</span>
            <button className={button} disabled={busy} onClick={() => void confirmed(confirm)}>
              {t("integration.yes")}
            </button>
            <button className={button} disabled={busy} onClick={() => setConfirm(null)}>
              {t("integration.cancel")}
            </button>
          </>
        )}
        {busy && <span className="text-muted">…</span>}
      </div>
      {confirm === "integrate" && preview && (
        <div className="flex flex-col gap-0.5">
          <span className="text-muted">{t("integration.preview.title")}</span>
          <ul>
            {preview.steps.map((s) => (
              <li key={s.agentId} className={s.kind === "conflict" ? "text-red-400" : undefined}>
                <span className="text-zinc-100">{s.agentId}</span>{" "}
                {s.kind === "conflict"
                  ? t("integration.preview.conflict", { files: s.files.join(", ") })
                  : t(`integration.preview.${s.kind}`)}
              </li>
            ))}
          </ul>
          {preview.overlaps.map((o) => (
            <p key={o.file} className="text-amber-300">
              {t("integration.preview.overlap", { file: o.file, agents: o.agents.join(", ") })}
            </p>
          ))}
        </div>
      )}
      {error && (
        <p role="alert" className="break-words text-red-400">
          {error}
        </p>
      )}
      {result?.kind === "status" && (
        <ul>
          {result.rows.map((r) => (
            <li key={r.agentId}>
              <span className="text-zinc-100">{r.agentId}</span>{" "}
              <span className="text-muted">
                {r.status.branch} · {t("integration.ahead", { count: r.status.ahead })}
                {r.status.dirty && ` · ${t("integration.dirty")}`}
                {r.status.untracked.length > 0 &&
                  ` · ${t("integration.untracked", { files: r.status.untracked.join(", ") })}`}
                {r.status.files.length > 0 && ` · ${r.status.files.join(", ")}`}
              </span>
            </li>
          ))}
        </ul>
      )}
      {result?.kind === "integrate" && (
        <ul>
          {result.steps.map((s) => (
            <li
              key={s.agentId}
              className={s.kind === "conflict" || s.kind === "failed" ? "text-red-400" : undefined}
            >
              <span className="text-zinc-100">{s.agentId}</span>{" "}
              {s.kind === "conflict"
                ? t("integration.conflict", { files: s.files.join(", ") })
                : s.kind === "failed"
                  ? t("integration.failed", { error: s.error })
                  : t(`integration.${s.kind}`)}
            </li>
          ))}
        </ul>
      )}
      {result?.kind === "cleanup" && (
        <ul>
          {result.rows.map((r) => (
            <li key={r.agentId} className={r.error ? "text-red-400" : undefined}>
              <span className="text-zinc-100">{r.agentId}</span>{" "}
              {r.error ?? t("integration.removed")}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
