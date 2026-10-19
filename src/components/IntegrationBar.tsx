import { useState } from "react";
import { useTranslation } from "react-i18next";
import { smallButton, panel, panelTitle, errorText } from "./ui";
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

const button = smallButton;

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
    } catch (e) {
      // Without a preview (e.g. git older than 2.38) integration still works; say so.
      setPreview(null);
      setError(t("integration.preview.unavailable", { error: String(e) }));
    } finally {
      setConfirm("integrate");
      setBusy(false);
    }
  };
  const cleanup = (discardUntracked: boolean) =>
    run(async () => ({ kind: "cleanup", rows: await teamCleanup(planId, discardUntracked) }));
  const confirmed = (action: Action) =>
    action === "integrate" ? integrate() : cleanup(action === "forceCleanup");
  const cleanupFailed = result?.kind === "cleanup" && result.rows.some((r) => r.error);

  return (
    <section className={panel}>
      <div className="flex flex-wrap items-center gap-2">
        <span className={panelTitle}>{t("integration.title")}</span>
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
                className={`${button} text-danger`}
                disabled={busy}
                onClick={() => setConfirm("forceCleanup")}
              >
                {t("integration.forceCleanup")}
              </button>
            )}
          </>
        ) : (
          <>
            <span className="text-fg">{t(`integration.confirm.${confirm}`)}</span>
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
              <li key={s.agentId} className={s.kind === "conflict" ? "text-danger" : undefined}>
                <span className="text-fg">{s.agentId}</span>{" "}
                {s.kind === "conflict"
                  ? t("integration.preview.conflict", { files: s.files.join(", ") })
                  : t(`integration.preview.${s.kind}`)}
              </li>
            ))}
          </ul>
          {preview.overlaps.map((o) => (
            <p key={o.file} className="text-warning">
              {t("integration.preview.overlap", { file: o.file, agents: o.agents.join(", ") })}
            </p>
          ))}
        </div>
      )}
      {error && (
        <p role="alert" className={errorText}>
          {error}
        </p>
      )}
      {result?.kind === "status" && (
        <ul>
          {result.rows.map((r) => (
            <li key={r.agentId}>
              <span className="text-fg">{r.agentId}</span>{" "}
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
              className={s.kind === "conflict" || s.kind === "failed" ? "text-danger" : undefined}
            >
              <span className="text-fg">{s.agentId}</span>{" "}
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
            <li key={r.agentId} className={r.error ? "text-danger" : undefined}>
              <span className="text-fg">{r.agentId}</span> {r.error ?? t("integration.removed")}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
