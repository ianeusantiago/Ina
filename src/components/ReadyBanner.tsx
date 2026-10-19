import { useEffect, useState } from "react";
import { teamReport, type Report } from "../lib/report";
import { invoke } from "@tauri-apps/api/core";

interface Review {
  verdict: "ok" | "issues";
  summary: string;
  findings: { agent: string; severity: "high" | "medium" | "low"; problem: string }[];
  truncated: boolean;
}
import { useTranslation } from "react-i18next";
import {
  teamCleanup,
  teamIntegrate,
  teamPreview,
  type IntegrationPreview,
  type TaskView,
} from "../lib/team";
import { TEAMS_CHANGED } from "../lib/usePolled";
import { usePersistedState } from "../lib/usePersistedState";
import { errorText, ghostButton, primaryButton } from "./ui";

type Step =
  | { kind: "ask" }
  | { kind: "preview"; preview: IntegrationPreview | null }
  | { kind: "merged"; problems: string[] }
  | { kind: "cleaned" };

/**
 * Guided finish: once every agent is done, offer to look at what will change and merge it all
 * into the project in one click, then tidy up. Each step still needs the user's click.
 */
export function ReadyBanner({
  planId,
  tasks,
  onMerged,
}: {
  planId: number;
  tasks: TaskView[];
  /** Everything merged without problems. */
  onMerged: () => void;
}) {
  const { t } = useTranslation();
  const [step, setStep] = useState<Step>({ kind: "ask" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [finished, setFinished] = usePersistedState(
    `ina.ready.${planId}`,
    (v) => v === "done",
    (v) => (v ? "done" : ""),
  );
  const allDone = tasks.length > 0 && tasks.every((x) => x.status === "done");
  const [report, setReport] = useState<Report | null>(null);
  const [reportError, setReportError] = useState<string | null>(null);
  const [review, setReview] = useState<Review | null>(null);
  const [reviewing, setReviewing] = useState(false);
  useEffect(() => {
    if (!allDone || finished) return;
    let alive = true;
    teamReport(planId)
      .then((r) => alive && setReport(r))
      .catch((e: unknown) => alive && setReportError(String(e)));
    return () => {
      alive = false;
    };
  }, [allDone, finished, planId]);
  if (!allDone || finished) return null;

  const act = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const conflicts =
    step.kind === "preview" ? (step.preview?.steps.filter((s) => s.kind === "conflict") ?? []) : [];

  return (
    <section
      role="status"
      className="aero-card flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 text-[13px] text-fg"
    >
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <p className="font-semibold">
          {step.kind === "merged"
            ? step.problems.length === 0
              ? t("ready.merged")
              : t("ready.mergedWithProblems")
            : step.kind === "cleaned"
              ? t("ready.cleaned")
              : t("ready.title")}
        </p>
        <p className="text-xs text-fg-2">
          {step.kind === "ask" && t("ready.hint")}
          {step.kind === "preview" &&
            (step.preview === null
              ? t("ready.noPreview")
              : conflicts.length === 0
                ? t("ready.clean")
                : t("ready.conflicts", { count: conflicts.length }))}
          {step.kind === "merged" &&
            (step.problems.length === 0 ? t("ready.cleanupHint") : step.problems.join(" · "))}
        </p>
        {step.kind === "ask" &&
          (report ? (
            <div className="mt-1.5 flex flex-col gap-1.5 text-[12.5px] text-fg">
              <p className="whitespace-pre-wrap">{report.done}</p>
              <p className="text-[11px] font-semibold text-muted">{t("report.howToTest")}</p>
              <p className="whitespace-pre-wrap">{report.howToTest}</p>
            </div>
          ) : reportError ? (
            <p className="text-[11px] text-muted">{t("report.unavailable")}</p>
          ) : (
            <p className="text-[11px] text-muted">{t("report.writing")}</p>
          ))}
        {reviewing && <p className="mt-1 text-[11px] text-muted">{t("review.working")}</p>}
        {review && (
          <div
            className={`mt-1.5 flex flex-col gap-1 rounded-lg px-3 py-2 text-[12.5px] ${
              review.verdict === "ok" ? "bg-success/10" : "bg-warning/10"
            }`}
          >
            <p className="font-semibold">
              {review.verdict === "ok" ? t("review.ok") : t("review.issues")}
            </p>
            <p className="whitespace-pre-wrap text-fg-2">{review.summary}</p>
            {review.findings.length > 0 && (
              <ul className="flex flex-col gap-0.5">
                {review.findings.map((f, i) => (
                  <li key={i} className="flex gap-1.5">
                    <span
                      className={
                        f.severity === "high"
                          ? "text-danger"
                          : f.severity === "medium"
                            ? "text-warning"
                            : "text-muted"
                      }
                    >
                      ●
                    </span>
                    <span>
                      <b>{f.agent}:</b> {f.problem}
                    </span>
                  </li>
                ))}
              </ul>
            )}
            {review.truncated && <p className="text-[11px] text-muted">{t("review.truncated")}</p>}
          </div>
        )}
        {error && <p className={`text-xs ${errorText}`}>{error}</p>}
      </div>
      <div className="flex gap-2">
        {(step.kind === "ask" || step.kind === "preview") && (
          <button
            className={`text-xs ${ghostButton}`}
            disabled={busy || reviewing}
            onClick={() => {
              setReviewing(true);
              setError(null);
              invoke<Review>("team_review", { planId })
                .then(setReview)
                .catch((e: unknown) => setError(String(e)))
                .finally(() => setReviewing(false));
            }}
          >
            {review ? t("review.again") : t("review.ask")}
          </button>
        )}
        {step.kind === "ask" && (
          <button
            className={`text-xs ${primaryButton}`}
            disabled={busy}
            onClick={() =>
              void act(async () => {
                let preview: IntegrationPreview | null = null;
                try {
                  preview = await teamPreview(planId);
                } catch {
                  // Older git without merge-tree: merging still works, just no dry run.
                }
                setStep({ kind: "preview", preview });
              })
            }
          >
            {t("ready.look")}
          </button>
        )}
        {step.kind === "preview" && (
          <>
            <button className={`text-xs ${ghostButton}`} onClick={() => setStep({ kind: "ask" })}>
              {t("ready.notNow")}
            </button>
            <button
              className={`text-xs ${primaryButton}`}
              disabled={busy}
              onClick={() =>
                void act(async () => {
                  const steps = await teamIntegrate(planId);
                  const problems = steps.flatMap((s) =>
                    s.kind === "conflict"
                      ? [t("ready.conflictIn", { agent: s.agentId })]
                      : s.kind === "failed"
                        ? [t("ready.failedIn", { agent: s.agentId })]
                        : [],
                  );
                  setStep({ kind: "merged", problems });
                  if (problems.length === 0) onMerged();
                })
              }
            >
              {t("ready.merge")}
            </button>
          </>
        )}
        {step.kind === "merged" && step.problems.length === 0 && (
          <button
            className={`text-xs ${primaryButton}`}
            disabled={busy}
            onClick={() =>
              void act(async () => {
                await teamCleanup(planId);
                window.dispatchEvent(new Event(TEAMS_CHANGED));
                setStep({ kind: "cleaned" });
              })
            }
          >
            {t("ready.cleanup")}
          </button>
        )}
        {(step.kind === "cleaned" || step.kind === "merged") && (
          <button className={`text-xs ${ghostButton}`} onClick={() => setFinished(true)}>
            {t("ready.close")}
          </button>
        )}
      </div>
    </section>
  );
}
