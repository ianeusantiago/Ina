import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

/** Fallback interval; the backend tick pushes `TEAM_CHANGED` as soon as something changes. */
export const POLL_MS = 15000;
/** Backend event (payload: plan id) emitted when a team's tasks or messages changed. */
export const TEAM_CHANGED = "team-changed";

/**
 * Load now, on every `TEAM_CHANGED` for `planId`, and every `POLL_MS`.
 * `load` must be stable (memoized).
 */
export function usePolled<T>(load: () => Promise<T>, initial: T, planId: number) {
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
    listen<number>(TEAM_CHANGED, (e) => {
      if (e.payload === planId) void run();
    })
      .then((unlisten) => (gone ? unlisten() : (stop = unlisten)))
      .catch((e: unknown) => setError(String(e)));
    return () => {
      gone = true;
      clearTimeout(first);
      clearInterval(timer);
      stop?.();
    };
  }, [run, planId]);

  return { data, error, setError, run };
}
