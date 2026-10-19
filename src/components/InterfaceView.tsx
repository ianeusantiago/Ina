import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  agentActivity,
  agentSend,
  baseName,
  conversation,
  plainText,
  type Conversation,
} from "../lib/activity";
import type { AgentCli, TaskStatus } from "../lib/team";
import { errorText, field, ghostButton, primaryButton } from "./ui";

const POLL_MS = 2000;
/** No log change for this long while working: the CLI may be asking something in its terminal. */
const STALL_MS = 45000;

export interface Member {
  id: string;
  name: string;
  subtitle: string;
  /** Plain task text from the plan (shown instead of the prompt INA sent). */
  task: string;
  cli: AgentCli;
  /** Terminal session started (the interface reads it; terminals keep running behind). */
  open: boolean;
  status?: TaskStatus;
  waitingFor: string[];
}

/** A message typed here and not yet in the CLI's log. */
interface Pending {
  text: string;
  /** Times this text was already in the log when it was sent. */
  seen: number;
}

const NONE: Pending[] = [];

const STATUS_DOT: Partial<Record<TaskStatus, string>> = {
  running: "bg-[#34c759] shadow-[0_0_6px_#34c759]",
  verifying: "bg-[#0a8fe0] shadow-[0_0_6px_#0a8fe0]",
  done: "bg-[#2a9d3a]",
  failed: "bg-danger",
  blocked: "bg-warning",
};

/**
 * Chat-like mask over the team: each member's conversation, read from its CLI's session log,
 * and a message box that types into its terminal. Stays mounted while the terminals view is
 * shown, so the selected member and unsent drafts survive switching views.
 */
export function InterfaceView({
  planId,
  members,
  visible,
  onOpen,
  onShowTerminals,
}: {
  planId: number;
  members: Member[];
  visible: boolean;
  onOpen: (id: string) => void;
  onShowTerminals: () => void;
}) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState(members[0]?.id ?? "");
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [pending, setPending] = useState<Record<string, Pending[]>>({});
  // Stable, and a no-op update keeps the same state (no re-render loop).
  const updatePending = useCallback(
    (id: string, update: (p: Pending[]) => Pending[]) =>
      setPending((all) => {
        const cur = all[id] ?? NONE;
        const next = update(cur);
        return next === cur ? all : { ...all, [id]: next };
      }),
    [],
  );
  const member = members.find((m) => m.id === selected) ?? members[0];
  return (
    <div className="flex h-full min-h-0 gap-2">
      <nav
        aria-label={t("activity.members")}
        className="aero-glass flex w-56 shrink-0 flex-col gap-1 overflow-y-auto rounded-2xl p-2"
      >
        {members.map((m) => {
          const active = m.id === member?.id;
          return (
            <button
              key={m.id}
              onClick={() => setSelected(m.id)}
              aria-current={active}
              className={`flex flex-col items-start gap-0.5 px-3 py-2 text-left transition ${
                active ? "aero-btn aero-square" : "rounded-[10px] hover:bg-hover"
              }`}
            >
              <span className="relative flex w-full items-center gap-2 text-[13px] font-semibold">
                <span
                  className={`size-2 shrink-0 rounded-full ${
                    (m.status && STATUS_DOT[m.status]) ??
                    (m.open ? STATUS_DOT.running : "bg-muted/50")
                  }`}
                />
                <span className="truncate">{m.name}</span>
              </span>
              <span
                className={`relative w-full truncate text-[11px] ${active ? "text-white/85" : "text-muted"}`}
              >
                {t(`team.clis.${m.cli}`)} · {m.status ? t(`tasks.status.${m.status}`) : m.subtitle}
              </span>
            </button>
          );
        })}
      </nav>
      {member && (
        <Chat
          key={member.id}
          planId={planId}
          member={member}
          visible={visible}
          draft={drafts[member.id] ?? ""}
          onDraft={(text) => setDrafts((d) => ({ ...d, [member.id]: text }))}
          pending={pending[member.id] ?? NONE}
          onPending={updatePending}
          onOpen={onOpen}
          onShowTerminals={onShowTerminals}
        />
      )}
    </div>
  );
}

function Chat({
  planId,
  member,
  visible,
  draft,
  onDraft,
  pending,
  onPending,
  onOpen,
  onShowTerminals,
}: {
  planId: number;
  member: Member;
  visible: boolean;
  draft: string;
  onDraft: (text: string) => void;
  pending: Pending[];
  onPending: (id: string, update: (p: Pending[]) => Pending[]) => void;
  onOpen: (id: string) => void;
  onShowTerminals: () => void;
}) {
  const { t } = useTranslation();
  const [chat, setChat] = useState<Conversation | null>(null);
  const [changedAt, setChangedAt] = useState(() => Date.now());
  const [now, setNow] = useState(() => Date.now());
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);
  const size = useRef(-1);

  useEffect(() => {
    if (!visible) return;
    let alive = true;
    const load = () =>
      agentActivity(planId, member.id)
        .then((events) => {
          if (!alive) return;
          if (events.length !== size.current) {
            size.current = events.length;
            setChangedAt(Date.now());
          }
          setChat(conversation(events));
          setNow(Date.now());
          setError(null);
        })
        .catch((e: unknown) => alive && setError(String(e)));
    void load();
    const timer = window.setInterval(() => void load(), POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [planId, member.id, visible]);

  // Retire optimistic messages once the CLI logged them.
  const sentCounts = chat?.sent;
  useEffect(() => {
    if (!sentCounts) return;
    onPending(member.id, (p) => {
      const left = p.filter((m) => (sentCounts.get(m.text) ?? 0) <= m.seen);
      return left.length === p.length ? p : left;
    });
  }, [sentCounts, onPending, member.id]);

  const messages = chat?.messages ?? [];
  useEffect(() => {
    const el = scroller.current;
    if (el && pinned.current) el.scrollTop = el.scrollHeight;
  }, [messages.length, pending.length, chat?.working]);

  const send = async () => {
    const text = draft.trim();
    if (!text) return;
    setSending(true);
    try {
      await agentSend(planId, member.id, text);
      onPending(member.id, (p) => [...p, { text, seen: chat?.sent.get(text) ?? 0 }]);
      onDraft("");
      setError(null);
      pinned.current = true;
    } catch (e) {
      setError(String(e));
    } finally {
      setSending(false);
    }
  };

  const working = member.open && (pending.length > 0 || (chat?.working ?? false));
  const stalled = working && pending.length === 0 && now - changedAt > STALL_MS;
  const doing = chat?.current
    ? t(`activity.doing.${chat.current.tool}`, { file: baseName(chat.current.detail) })
    : pending.length > 0
      ? t("activity.doing.sending")
      : t("activity.doing.thinking");

  return (
    <section className="aero-glass flex min-w-0 flex-1 flex-col rounded-2xl">
      <header className="flex items-baseline gap-2 border-b border-line px-5 py-3">
        <h2 className="text-[15px] font-semibold text-fg">{member.name}</h2>
        <span className="truncate text-xs text-muted">{member.subtitle}</span>
      </header>
      <div
        ref={scroller}
        onScroll={(e) => {
          const el = e.currentTarget;
          pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
        }}
        className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-5 py-4"
      >
        <div className="aero-card self-stretch px-4 py-3 text-[13px]">
          <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-accent">
            {t("activity.task")}
          </div>
          <p className="whitespace-pre-wrap break-words text-fg">{member.task}</p>
        </div>
        {!member.open && (
          <p className="m-auto max-w-sm text-center text-[13px] text-muted">
            {member.waitingFor.length > 0
              ? t("team.waitingFor", { agents: member.waitingFor.join(", ") })
              : t("activity.notStarted")}
          </p>
        )}
        {messages.map((m, i) => (
          <Bubble key={i} role={m.role}>
            {m.role === "agent" ? plainText(m.text, t("activity.technical")) : m.text}
          </Bubble>
        ))}
        {pending.map((m, i) => (
          <Bubble key={`p${i}`} role="user" faded>
            {m.text}
          </Bubble>
        ))}
        {working && (
          <div role="status" className="flex items-center gap-2 self-start text-[13px] text-fg-2">
            <span className="flex gap-1" aria-hidden>
              {[0, 1, 2].map((d) => (
                <span
                  key={d}
                  className="size-1.5 animate-bounce rounded-full bg-accent"
                  style={{ animationDelay: `${d * 0.15}s` }}
                />
              ))}
            </span>
            <span className="italic">{doing}</span>
          </div>
        )}
        {stalled && (
          <div className="aero-card flex items-center gap-3 self-start px-3.5 py-2.5 text-[12px] text-fg-2">
            <span>{t("activity.stalled")}</span>
            <button className={ghostButton} onClick={onShowTerminals}>
              {t("activity.openTerminals")}
            </button>
          </div>
        )}
      </div>
      {error && <p className={`${errorText} px-5 pb-1 text-xs`}>{error}</p>}
      <footer className="flex items-center gap-2 border-t border-line px-4 py-3">
        {member.open ? (
          <form
            className="flex flex-1 items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void send();
            }}
          >
            <input
              className={`${field} flex-1 rounded-[10px] px-4 py-2 text-[13px]`}
              placeholder={t("activity.placeholder", { name: member.name })}
              aria-label={t("activity.placeholder", { name: member.name })}
              value={draft}
              maxLength={4000}
              onChange={(e) => onDraft(e.target.value)}
            />
            <button type="submit" className={primaryButton} disabled={sending || !draft.trim()}>
              {t("activity.send")}
            </button>
          </form>
        ) : (
          <button
            className={primaryButton}
            disabled={member.waitingFor.length > 0}
            onClick={() => onOpen(member.id)}
          >
            {t("activity.start")}
          </button>
        )}
      </footer>
    </section>
  );
}

function Bubble({
  role,
  faded,
  children,
}: {
  role: "user" | "agent";
  faded?: boolean;
  children: ReactNode;
}) {
  return role === "user" ? (
    <div
      data-role="user"
      className={`aero-btn aero-square max-w-[75%] self-end whitespace-pre-wrap break-words px-3.5 py-2 text-[13px] ${faded ? "opacity-70" : ""}`}
    >
      <span className="relative">{children}</span>
    </div>
  ) : (
    <div
      data-role="agent"
      className="aero-card max-w-[85%] self-start whitespace-pre-wrap break-words px-3.5 py-2.5 text-[13px] leading-relaxed text-fg"
    >
      {children}
    </div>
  );
}
