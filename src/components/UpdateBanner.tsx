import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { errorText, ghostButton, primaryButton } from "./ui";

/**
 * Checks GitHub Releases once at startup (signed updates only, see tauri.conf `plugins.updater`)
 * and offers to install and restart. A failed check is logged, never shown: offline is normal.
 */
export function UpdateBanner() {
  const { t } = useTranslation();
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    // The dev build has no installed version to replace.
    if (import.meta.env.DEV) return;
    check()
      .then(setUpdate)
      .catch((e: unknown) => console.error("[update]", e));
  }, []);

  if (!update || dismissed) return null;

  const install = async () => {
    setError(null);
    let total = 0;
    let got = 0;
    try {
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        if (ev.event === "Progress") {
          got += ev.data.chunkLength;
          setProgress(total ? Math.round((got / total) * 100) : null);
        }
      });
      await relaunch();
    } catch (e) {
      setError(String(e));
      setProgress(null);
    }
  };

  return (
    <div
      role="status"
      className="aero-card mx-3 mt-2 flex flex-wrap items-center gap-3 px-4 py-2 text-[12.5px] text-fg"
    >
      <span className="font-semibold">{t("update.available", { version: update.version })}</span>
      <span className="text-fg-2">{t("update.hint")}</span>
      <span className="flex-1" />
      {error && <span className={`text-xs ${errorText}`}>{error}</span>}
      {progress !== null ? (
        <span className="text-xs text-muted">{t("update.downloading", { pct: progress })}</span>
      ) : (
        <>
          <button className={`text-xs ${ghostButton}`} onClick={() => setDismissed(true)}>
            {t("update.later")}
          </button>
          <button className={`text-xs ${primaryButton}`} onClick={() => void install()}>
            {t("update.install")}
          </button>
        </>
      )}
    </div>
  );
}
