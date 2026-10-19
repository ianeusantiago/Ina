import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ghostButton } from "./ui";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { terminalTheme } from "../lib/theme";
import { killPty, resizePty, writePty, type Spawner } from "../lib/pty";
import { parsePermission, samePrompt, type PermissionPrompt } from "../lib/permission";

type Status =
  | { kind: "starting" }
  | { kind: "running" }
  | { kind: "exited"; code: number | null }
  | { kind: "error"; message: string };

const report = (e: unknown) => console.error("[pty]", e);

/** A permission menu on screen and how to answer it (types the option's key). */
export interface PendingPermission {
  prompt: PermissionPrompt;
  answer: (key: string) => void;
}

export function TerminalPane({
  title,
  subtitle,
  spawn,
  actions,
  onPermission,
}: {
  title: string;
  subtitle?: string;
  /** Extra header buttons. */
  actions?: ReactNode;
  /** Called when a Claude Code permission menu appears on screen or goes away. */
  onPermission?: (pending: PendingPermission | null) => void;
  /** Must be stable (memoized); a new function restarts the session. */
  spawn: Spawner;
}) {
  const { t } = useTranslation();
  const host = useRef<HTMLDivElement>(null);
  const sessionId = useRef<string | null>(null);
  const [generation, setGeneration] = useState(0);
  const [status, setStatus] = useState<Status>({ kind: "starting" });
  // Latest callback without restarting the session when it changes.
  const permissionRef = useRef(onPermission);
  useEffect(() => {
    permissionRef.current = onPermission;
  }, [onPermission]);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const id = `s-${crypto.randomUUID()}`;
    sessionId.current = id;
    let alive = true;

    const term = new Terminal({
      fontFamily: '"SF Mono", Menlo, "Cascadia Mono", Consolas, monospace',
      fontSize: 12.5,
      fontWeight: "400",
      fontWeightBold: "600",
      lineHeight: 1.2,
      cursorBlink: true,
      cursorStyle: "bar",
      theme: terminalTheme(),
    });
    // Follow theme switches (App sets `data-theme` on <html>).
    const themeWatch = new MutationObserver(() => (term.options.theme = terminalTheme()));
    themeWatch.observe(document.documentElement, { attributeFilter: ["data-theme"] });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    fit.fit();

    const input = term.onData((data) => writePty(id, data).catch(report));
    const resize = term.onResize(({ cols, rows }) => resizePty(id, cols, rows).catch(report));
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(el);

    // Screen scan for permission menus (works while hidden: the buffer still updates).
    let shown: PermissionPrompt | null = null;
    let scanTimer: ReturnType<typeof setTimeout> | undefined;
    const scan = () => {
      const buf = term.buffer.active;
      const lines: string[] = [];
      for (let y = buf.viewportY; y < buf.viewportY + term.rows; y++) {
        lines.push(buf.getLine(y)?.translateToString(true) ?? "");
      }
      const found = parsePermission(lines);
      if (samePrompt(found, shown)) return;
      shown = found;
      permissionRef.current?.(
        found && { prompt: found, answer: (key) => void writePty(id, key).catch(report) },
      );
    };
    const scheduleScan = () => {
      clearTimeout(scanTimer);
      scanTimer = setTimeout(scan, 150);
    };

    setStatus({ kind: "starting" });
    spawn(id, term.cols, term.rows, (msg) => {
      if (!alive) return;
      if (msg.type === "output") term.write(new Uint8Array(msg.data), scheduleScan);
      else {
        setStatus({ kind: "exited", code: msg.code });
        if (shown) {
          shown = null;
          permissionRef.current?.(null);
        }
      }
    })
      .then(() => alive && setStatus({ kind: "running" }))
      .catch((e: unknown) => alive && setStatus({ kind: "error", message: String(e) }));

    return () => {
      alive = false;
      clearTimeout(scanTimer);
      if (shown) permissionRef.current?.(null);
      observer.disconnect();
      input.dispose();
      resize.dispose();
      themeWatch.disconnect();
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
    <section className="flex h-full min-h-0 flex-col aero-glass rounded-2xl ">
      <header className="flex items-center gap-3 border-b border-line px-4 py-2 text-xs">
        <span className="font-semibold text-fg">{title}</span>
        {subtitle && <span className="truncate text-muted">{subtitle}</span>}
        <span className="flex items-center gap-1.5 text-muted" role="status">
          <span
            aria-hidden
            className={`size-1.5 rounded-full ${
              status.kind === "running" ? "bg-success" : "bg-muted/60"
            }`}
          />
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
      <div ref={host} className="min-h-0 flex-1 rounded-b-mac px-3 py-2" />
    </section>
  );
}
