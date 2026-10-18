import { useState } from "react";

/** State mirrored to `localStorage[key]`; storage failures fall back to session-only state. */
export function usePersistedState<T>(
  key: string,
  parse: (stored: string | null) => T,
  serialize: (value: T) => string = String,
) {
  const [value, setValue] = useState<T>(() => {
    try {
      return parse(localStorage.getItem(key));
    } catch {
      return parse(null);
    }
  });
  const change = (next: T) => {
    setValue(next);
    try {
      localStorage.setItem(key, serialize(next));
    } catch {
      // Not persisted; applies to this session.
    }
  };
  return [value, change] as const;
}
