import { Search } from "lucide-react";
import { useEffect, useState } from "react";
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
