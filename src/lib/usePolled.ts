import { useCallback, useEffect, useState } from "react";

export const POLL_MS = 5000;

/** Poll `load` now and every `POLL_MS`; `load` must be stable (memoized). */
export function usePolled<T>(load: () => Promise<T>, initial: T) {
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
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [run]);

  return { data, error, setError, run };
}
