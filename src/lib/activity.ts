import { invoke } from "@tauri-apps/api/core";

export type Tool =
  "shell" | "edit" | "write" | "read" | "search" | "web" | "agent" | "todo" | "other";

/** One thing a team member did, read from its CLI's session log. */
export type ActivityEvent =
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string }
  | { kind: "tool"; tool: Tool; detail: string; added: number; removed: number };

export interface ChatMessage {
  role: "user" | "agent";
  text: string;
}

export interface Conversation {
  /** Messages after the task INA sent first (shown as the task card instead). */
  messages: ChatMessage[];
  /** The member is mid-turn: the log ends with a message to it or a tool use. */
  working: boolean;
  /** What it is doing right now (the trailing tool use), if any. */
  current: { tool: Tool; detail: string } | null;
  /** Times each user text appears, to retire optimistic messages once logged. */
  sent: Map<string, number>;
}

export function conversation(events: ActivityEvent[]): Conversation {
  const messages: ChatMessage[] = [];
  const sent = new Map<string, number>();
  let first = true;
  for (const e of events) {
    if (e.kind === "tool") continue;
    if (e.kind === "user") {
      sent.set(e.text, (sent.get(e.text) ?? 0) + 1);
      // The first message of every INA session is the task prompt.
      if (first) {
        first = false;
        continue;
      }
    }
    first = false;
    messages.push({ role: e.kind === "user" ? "user" : "agent", text: e.text });
  }
  const last = events[events.length - 1];
  return {
    messages,
    working: last !== undefined && last.kind !== "assistant",
    current: last?.kind === "tool" ? { tool: last.tool, detail: last.detail } : null,
    sent,
  };
}

/** Last path segment, for "Editing App.tsx…". */
export const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export const agentActivity = (planId: number, agentId: string) =>
  invoke<ActivityEvent[]>("agent_activity", { planId, agentId });
export const agentSend = (planId: number, agentId: string, text: string) =>
  invoke<null>("agent_send", { planId, agentId, text });
