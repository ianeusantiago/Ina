import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { errorText, field, ghostButton, primaryButton } from "./ui";

/** Only local dev servers: the preview never loads arbitrary sites. */
export function localUrl(input: string): string | null {
  try {
    const u = new URL(input.trim());
    const local = ["localhost", "127.0.0.1"].includes(u.hostname);
    return local && (u.protocol === "http:" || u.protocol === "https:") ? u.href : null;
  } catch {
    return null;
  }
}

/**
 * The project's site inside INA: its `index.html` (via the asset protocol) or a local dev
 * server. Sandboxed without same-origin, so the page cannot reach INA's backend.
 */
export function PreviewView({ workspaceId }: { workspaceId: number }) {
  const { t } = useTranslation();
  const [src, setSrc] = useState<string | null>(null);
  const [found, setFound] = useState<boolean | null>(null);
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [nonce, setNonce] = useState(0);

  const loadPage = useCallback(() => {
    invoke<string | null>("preview_page", { workspaceId })
      .then((page) => {
        setFound(page !== null);
        if (page) setSrc(convertFileSrc(page));
      })
      .catch((e: unknown) => setError(String(e)));
  }, [workspaceId]);
  useEffect(loadPage, [loadPage]);

  const openUrl = (e: FormEvent) => {
    e.preventDefault();
    const ok = localUrl(url);
    setError(ok ? null : t("preview.url"));
    if (ok) setSrc(ok);
  };

  return (
    <section className="aero-glass flex min-h-0 flex-1 flex-col gap-2 rounded-2xl p-3">
      <div className="flex flex-wrap items-center gap-2 text-xs">
        <span className="font-semibold text-fg">{t("preview.title")}</span>
        <span className="text-muted">{t("preview.hint")}</span>
        <span className="flex-1" />
        <form onSubmit={openUrl} className="flex items-center gap-1.5">
          <input
            aria-label={t("preview.url")}
            placeholder={t("preview.url")}
            className={`w-64 px-2 py-1 text-xs ${field}`}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
          <button type="submit" className={`text-xs ${primaryButton}`}>
            {t("preview.open")}
          </button>
        </form>
        {found && (
          <button className={`text-xs ${ghostButton}`} onClick={loadPage}>
            {t("preview.file")}
          </button>
        )}
        {src && (
          <button className={`text-xs ${ghostButton}`} onClick={() => setNonce((n) => n + 1)}>
            {t("preview.reload")}
          </button>
        )}
      </div>
      {error && <p className={`text-xs ${errorText}`}>{error}</p>}
      {src ? (
        <iframe
          key={`${src}#${nonce}`}
          title={t("preview.title")}
          src={src}
          sandbox="allow-scripts allow-forms allow-modals"
          className="min-h-0 flex-1 rounded-xl border border-line bg-white"
        />
      ) : (
        found === false && (
          <p className="m-auto max-w-md text-center text-[13px] text-muted">{t("preview.none")}</p>
        )
      )}
    </section>
  );
}
