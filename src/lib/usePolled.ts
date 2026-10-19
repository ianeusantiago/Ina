import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import type { TaskView } from "./team";

/** Fallback interval; the backend tick pushes `TEAM_CHANGED` as soon as something changes. */
export const POLL_MS = 15000;
/** Backend event emitted when a team's tasks or messages changed. */
export const TEAM_CHANGED = "team-changed";
/** Window event the UI dispatches after approving a team (the list of teams changed). */
export const TEAMS_CHANGED = "ina-teams-changed";

export interface TeamChanged {
  planId: number;
  /** The task registry as the backend tick just synced it. */
  tasks: TaskView[];
}

/**
 * Load now, on every `TEAM_CHANGED` for `planId`, and every `POLL_MS`.
 * `fromEvent`, if given, takes the data from the event instead of loading again.
 * `load` and `fromEvent` must be stable (memoized or module-level).
 */
export function usePolled<T>(
  load: () => Promise<T>,
  initial: T,
  planId: number,
  fromEvent?: (e: TeamChanged) => T,
) {
  const [data, setData] = useState<T>(initial);
  const [error, setError] = useState<string | null>(null);

  /** Run `action` (default: `load`) and keep its result as the current data. */
  const run = useCallback(
    async (action: () => Promise<T> = load) => {
      try {
        setData(await action());
        setError(null);
      } catch (e) {
        setError(String(e));
      }
    },
    [load],
  );

  useEffect(() => {
    const first = setTimeout(() => void run(), 0);
    const timer = setInterval(() => void run(), POLL_MS);
    let stop: (() => void) | null = null;
    let gone = false;
    listen<TeamChanged>(TEAM_CHANGED, (e) => {
      if (e.payload.planId !== planId) return;
      if (fromEvent) {
        setData(fromEvent(e.payload));
        setError(null);
      } else void run();
    })
      .then((unlisten) => (gone ? unlisten() : (stop = unlisten)))
      .catch((e: unknown) => setError(String(e)));
    return () => {
      gone = true;
      clearTimeout(first);
      clearInterval(timer);
      stop?.();
    };
  }, [run, planId, fromEvent]);

  return { data, error, setError, run };
}
