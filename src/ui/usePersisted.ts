import { useCallback, useState } from "react";

function read<T>(key: string, fallback: T, decode: (raw: string) => T | null) {
  try {
    const raw = window.localStorage.getItem(key);
    if (raw === null) return fallback;
    return decode(raw) ?? fallback;
  } catch {
    return fallback;
  }
}

/**
 * State remembered per viewer in localStorage. Storage can be missing or throw (private
 * windows, blocked site data): reads then give `fallback` and writes are dropped, and the
 * state still works for the session. `decode` returns null for a stored value it rejects.
 */
export function usePersisted<T>(
  key: string,
  fallback: T,
  decode: (raw: string) => T | null,
): [T, (next: T) => void] {
  const [value, setValue] = useState(() => read(key, fallback, decode));
  const set = useCallback(
    (next: T) => {
      setValue(next);
      try {
        window.localStorage.setItem(key, String(next));
      } catch {
        // Not remembered; the session still has it.
      }
    },
    [key],
  );
  return [value, set];
}

export const decodeBool = (raw: string): boolean | null =>
  raw === "true" ? true : raw === "false" ? false : null;
