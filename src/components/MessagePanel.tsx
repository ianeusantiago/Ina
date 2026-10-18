import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  decideMessage,
  sendMessage,
  teamMessages,
  type AgentSpec,
  type MessageView,
} from "../lib/team";

const POLL_MS = 5000;
const SHOWN = 30;
const button =
  "rounded border border-line px-2 py-0.5 hover:bg-line disabled:opacity-40 disabled:hover:bg-transparent";

/** Messages between agents, orchestrator and user; orchestrator instructions need approval. */
export function MessagePanel({ planId, agents }: { planId: number; agents: AgentSpec[] }) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<MessageView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [to, setTo] = useState(agents[0]?.id ?? "");
  const [text, setText] = useState("");
  const [replyTo, setReplyTo] = useState<MessageView | null>(null);

  const run = useCallback(async (action: () => Promise<MessageView[]>) => {
    try {
      setMessages(await action());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    const poll = () => void run(() => teamMessages(planId));
    const first = setTimeout(poll, 0);
    const timer = setInterval(poll, POLL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [planId, run]);

  const act = async (action: () => Promise<MessageView[]>) => {
    setBusy(true);
    await run(action);
    setBusy(false);
  };
  const send = () =>
    act(async () => {
      const list = await sendMessage(planId, to, text, replyTo?.id ?? null);
      setText("");
      setReplyTo(null);
      return list;
    });

  const name = (id: string) => agents.find((a) => a.id === id)?.name ?? t(`messages.who.${id}`, id);

  return (
    <section className="flex min-h-0 flex-col gap-1 rounded border border-line bg-panel px-3 py-2 text-xs">
      <span className="font-semibold uppercase tracking-wide text-muted">
        {t("messages.title")}
      </span>
      {error && (
        <p role="alert" className="break-words text-red-400">
          {error}
        </p>
      )}
      {messages.length === 0 && !error && <p className="text-muted">{t("messages.none")}</p>}
      <ul className="flex max-h-40 flex-col gap-1 overflow-y-auto">
        {messages.slice(-SHOWN).map((m) => (
          <li key={m.id} className={m.status === "rejected" ? "text-muted" : undefined}>
            <span className="text-zinc-100">{name(m.from)}</span>
            {m.to && <span className="text-muted"> → {name(m.to)}</span>}{" "}
            <span className="text-sky-300">{t(`messages.kind.${m.kind}`, m.kind)}</span>{" "}
            <span className="text-muted">({t(`messages.status.${m.status}`, m.status)})</span>{" "}
            <span className="whitespace-pre-wrap break-words text-zinc-300">{m.text}</span>
            {m.status === "proposed" && (
              <span className="ml-2 inline-flex gap-1">
                <button
                  className={button}
                  disabled={busy}
                  onClick={() => void act(() => decideMessage(planId, m.id, true))}
                >
                  {t("messages.approve")}
                </button>
                <button
                  className={button}
                  disabled={busy}
                  onClick={() => void act(() => decideMessage(planId, m.id, false))}
                >
                  {t("messages.reject")}
                </button>
              </span>
            )}
            {m.kind === "question" && agents.some((a) => a.id === m.from) && (
              <button
                className={`${button} ml-2`}
                onClick={() => {
                  setTo(m.from);
                  setReplyTo(m);
                }}
              >
                {t("messages.reply")}
              </button>
            )}
          </li>
        ))}
      </ul>
      <form
        className="flex items-center gap-1"
        onSubmit={(e) => {
          e.preventDefault();
          void send();
        }}
      >
        <select
          aria-label={t("messages.to")}
          className="rounded border border-line bg-panel px-1 py-0.5 text-zinc-100"
          value={to}
          onChange={(e) => setTo(e.target.value)}
        >
          {agents.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
        <input
          className="min-w-0 flex-1 rounded border border-line bg-transparent px-2 py-0.5 text-zinc-100 outline-none focus:border-zinc-500"
          placeholder={
            replyTo
              ? t("messages.replyingTo", { text: replyTo.text.slice(0, 40) })
              : t("messages.placeholder")
          }
          maxLength={4000}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <button className={button} type="submit" disabled={busy || !text.trim() || !to}>
          {t("messages.send")}
        </button>
      </form>
    </section>
  );
}
