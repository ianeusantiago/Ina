import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { claudeSetup, type ClaudeSetup } from "../lib/team";

/** First-run check: shown only while Claude Code is missing, not set up or not logged in. */
export function SetupBanner() {
  const { t } = useTranslation();
  const [setup, setSetup] = useState<ClaudeSetup | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(true);

  const check = useCallback(async () => {
    setBusy(true);
    try {
      setSetup(await claudeSetup());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    const first = setTimeout(() => void check(), 0);
    return () => clearTimeout(first);
  }, [check]);

  if (busy && !setup) {
    return <p className="px-4 py-1.5 text-xs text-muted">{t("setup.checking")}</p>;
  }
  const problems = setup
    ? [
        !setup.installed && t("setup.notInstalled"),
        setup.installed && !setup.onboarded && t("setup.notOnboarded"),
        setup.installed && !setup.loggedIn && t("setup.notLoggedIn"),
        setup.error && t("setup.error", { error: setup.error }),
      ].filter((p): p is string => Boolean(p))
    : [];
  if (error) problems.push(t("setup.error", { error }));
  if (problems.length === 0) return null;

  return (
    <section
      role="alert"
      className="flex flex-col gap-1 mx-3 mt-2 rounded-mac bg-warning/10 px-4 py-2.5 text-xs shadow-card"
    >
      <span className="font-semibold text-warning">{t("setup.title")}</span>
      <ul className="ml-4 list-disc text-fg-2">
        {problems.map((p) => (
          <li key={p} className="select-text">
            {p}
          </li>
        ))}
      </ul>
      <button
        className="self-start rounded-md bg-panel px-2 py-1 font-medium shadow-[0_0_0_0.5px_var(--ina-separator),0_1px_1px_rgb(0_0_0/0.05)] transition hover:bg-hover active:scale-[0.98] disabled:opacity-40"
        disabled={busy}
        onClick={() => void check()}
      >
        {t("setup.recheck")}
      </button>
    </section>
  );
}
