import { useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ghostButton, pillButton } from "./ui";
import { spawnAgent, type Spawner } from "../lib/pty";
import { type TaskStatus } from "../lib/team";
import { TerminalPane, type PendingPermission } from "./TerminalPane";

export function AgentTerminal({
  planId,
  agentId,
  title,
  subtitle,
  open,
  onOpen,
  resume,
  canResume,
  waitingFor,
  status,
  maximized,
  onToggleMaximize,
  onPermission,
}: {
  planId: number;
  agentId: string;
  onPermission: (agentId: string, pending: PendingPermission | null) => void;
  title: string;
  subtitle: string;
  maximized: boolean;
  onToggleMaximize: () => void;
  /** Session started; owned by TeamRun so the interface view can start it too. */
  open: boolean;
  onOpen: (resume: boolean) => void;
  /** Opened to continue the last conversation. */
  resume: boolean;
  canResume: boolean;
  /** Prerequisites not done yet; the backend refuses to start until they are. */
  waitingFor: string[];
  status?: TaskStatus;
}) {
  const { t } = useTranslation();
  const spawn = useMemo<Spawner>(
    () => (id, cols, rows, onEvent) => spawnAgent(planId, agentId, id, cols, rows, onEvent, resume),
    [planId, agentId, resume],
  );
  const permission = useCallback(
    (pending: PendingPermission | null) => onPermission(agentId, pending),
    [onPermission, agentId],
  );
  if (!open) {
    return (
      <section className="flex h-full min-h-0 flex-col items-start gap-2 aero-glass rounded-2xl p-4  text-xs">
        <span className="font-medium">{title}</span>
        <span className="text-muted">{subtitle}</span>
        {status === "blocked" ? (
          <span className="text-warning">{t("team.blocked")}</span>
        ) : (
          waitingFor.length > 0 && (
            <span className="text-warning">
              {t("team.waitingFor", { agents: waitingFor.join(", ") })}
            </span>
          )
        )}
        <div className="flex gap-2">
          {canResume && (
            <button className={pillButton} onClick={() => onOpen(true)}>
              {t("resume.continue")}
            </button>
          )}
          <button className={pillButton} onClick={() => onOpen(false)}>
            {canResume ? t("resume.fresh") : t("team.openTerminal")}
          </button>
        </div>
      </section>
    );
  }
  return (
    <TerminalPane
      title={title}
      subtitle={subtitle}
      spawn={spawn}
      onPermission={permission}
      actions={
        <button className={ghostButton} onClick={onToggleMaximize}>
          {t(maximized ? "terminal.restore" : "terminal.maximize")}
        </button>
      }
    />
  );
}
