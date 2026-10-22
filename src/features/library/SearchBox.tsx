import { Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import styles from "./Library.module.css";

const DEBOUNCE_MS = 200;

/**
 * The search field. Typing is kept locally and committed to the URL after a short pause, so
 * each keystroke does not hit the backend. A change of `value` from outside (Back, clear
 * filters) replaces what is typed.
 */
export function SearchBox({
  value,
  onCommit,
}: {
  value: string;
  onCommit: (text: string) => void;
}) {
  const [text, setText] = useState(value);
  const [seen, setSeen] = useState(value);
  if (value !== seen) {
    setSeen(value);
    setText(value);
  }

  const input = useRef<HTMLInputElement>(null);
  // "/" jumps to the search, unless a field, a dialog or a modifier is in the way.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key !== "/" || event.ctrlKey || event.metaKey || event.altKey)
        return;
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
      )
        return;
      if (document.querySelector('[aria-modal="true"]')) return;
      event.preventDefault();
      input.current?.focus();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (text === value) return;
    const timer = setTimeout(() => onCommit(text), DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [text, value, onCommit]);

  return (
    <div className={styles.search}>
      <label className={styles.sr} htmlFor="library-search">
        Search artifacts
      </label>
      <Search size={14} aria-hidden="true" className={styles.searchIcon} />
      <input
        id="library-search"
        ref={input}
        className={styles.input}
        type="search"
        placeholder="Search artifacts, sources, tags"
        autoComplete="off"
        value={text}
        onChange={(event) => setText(event.target.value)}
      />
    </div>
  );
}
