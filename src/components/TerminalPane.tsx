import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ghostButton } from "./ui";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { killPty, resizePty, writePty, type Spawner } from "../lib/pty";

type Status =
  | { kind: "starting" }
  | { kind: "running" }
  | { kind: "exited"; code: number | null }
  | { kind: "error"; message: string };

const report = (e: unknown) => console.error("[pty]", e);

export function TerminalPane({
  title,
  subtitle,
  spawn,
  actions,
}: {
  title: string;
  subtitle?: string;
  /** Extra header buttons. */
  actions?: ReactNode;
  /** Must be stable (memoized); a new function restarts the session. */
  spawn: Spawner;
}) {
  const { t } = useTranslation();
  const host = useRef<HTMLDivElement>(null);
  const sessionId = useRef<string | null>(null);
  const [generation, setGeneration] = useState(0);
  const [status, setStatus] = useState<Status>({ kind: "starting" });

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const id = `s-${crypto.randomUUID()}`;
    sessionId.current = id;
    let alive = true;

    const term = new Terminal({
      fontFamily: "Cascadia Mono, Consolas, monospace",
      fontSize: 13,
      cursorBlink: true,
      theme: { background: "#0b0b0c", foreground: "#e4e4e7", cursor: "#9ca3af" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    fit.fit();

    const input = term.onData((data) => writePty(id, data).catch(report));
    const resize = term.onResize(({ cols, rows }) => resizePty(id, cols, rows).catch(report));
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(el);

    setStatus({ kind: "starting" });
    spawn(id, term.cols, term.rows, (msg) => {
      if (!alive) return;
      if (msg.type === "output") term.write(new Uint8Array(msg.data));
      else setStatus({ kind: "exited", code: msg.code });
    })
      .then(() => alive && setStatus({ kind: "running" }))
      .catch((e: unknown) => alive && setStatus({ kind: "error", message: String(e) }));

    return () => {
      alive = false;
      observer.disconnect();
      input.dispose();
      resize.dispose();
      term.dispose();
      // Already exited is fine; a kill that beats the spawn makes the backend refuse it.
      killPty(id).catch(() => undefined);
    };
  }, [generation, spawn]);

  const label =
    status.kind === "running"
      ? t("terminal.running")
      : status.kind === "exited"
        ? status.code === null
          ? t("terminal.exitedUnknown")
          : t("terminal.exited", { code: status.code })
        : status.kind === "error"
          ? t("terminal.spawnError", { error: status.message })
          : "…";

  return (
    <section className="flex h-full min-h-0 flex-col rounded border border-line bg-panel">
      <header className="flex items-center gap-3 border-b border-line px-3 py-1.5 text-xs">
        <span className="font-medium">{title}</span>
        {subtitle && <span className="truncate text-muted">{subtitle}</span>}
        <span className="text-muted" role="status">
          {label}
        </span>
        <span className="flex-1" />
        {actions}
        <button className={ghostButton} onClick={() => setGeneration((g) => g + 1)}>
          {t("terminal.restart")}
        </button>
        <button
          className={ghostButton}
          disabled={status.kind !== "running"}
          onClick={() => sessionId.current && killPty(sessionId.current).catch(report)}
        >
          {t("terminal.kill")}
        </button>
      </header>
      <div ref={host} className="min-h-0 flex-1 p-1" />
    </section>
  );
}
