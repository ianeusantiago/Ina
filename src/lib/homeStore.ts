import { useSyncExternalStore } from "react";
import type { PlanDraft } from "./team";
import type { Workspace } from "./workspaces";

export type HomeItem =
  | { kind: "user"; text: string }
  | { kind: "agent"; text: string }
  | { kind: "plan"; workspace: Workspace; draft: PlanDraft; started: boolean }
  | { kind: "trust"; path: string; workspace: Workspace; task: string };

export interface HomeState {
  items: HomeItem[];
  draft: string;
  /** Step in progress (not persisted: it would never finish after a reload). */
  busy: string | null;
  error: string | null;
}

const KEY = "ina.home";

/**
 * Lives outside the component so the conversation survives leaving Home, and requests still
 * in flight land in it; mirrored to `localStorage` so it survives restarts too.
 */
function load(): HomeState {
  const empty: HomeState = { items: [], draft: "", busy: null, error: null };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return empty;
    const v = JSON.parse(raw) as Partial<HomeState>;
    return {
      ...empty,
      items: Array.isArray(v.items) ? v.items : [],
      draft: typeof v.draft === "string" ? v.draft : "",
    };
  } catch {
    return empty;
  }
}

let state = load();
const listeners = new Set<() => void>();

export function setHome(change: Partial<HomeState> | ((s: HomeState) => Partial<HomeState>)) {
  state = { ...state, ...(typeof change === "function" ? change(state) : change) };
  try {
    localStorage.setItem(KEY, JSON.stringify({ items: state.items, draft: state.draft }));
  } catch {
    // Not persisted; kept for this session.
  }
  listeners.forEach((l) => l());
}

export const addHomeItem = (item: HomeItem) => setHome((s) => ({ items: [...s.items, item] }));

export function newHomeConversation() {
  setHome({ items: [], draft: "", error: null });
}

export function useHome(): HomeState {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => state,
  );
}
