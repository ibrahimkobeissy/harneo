import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button, announce } from "../../ui";
import styles from "./Settings.module.css";

type State = "idle" | "copied" | "failed";

/** A small button that copies `path` to the clipboard and says "Copied" beside it. */
export function CopyPath({ path, label }: { path: string; label: string }) {
  const [state, setState] = useState<State>("idle");
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  async function copy() {
    let next: State = "copied";
    try {
      await navigator.clipboard.writeText(path);
    } catch {
      next = "failed";
    }
    setState(next);
    announce(next === "copied" ? "Copied" : "Could not copy");
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setState("idle"), 2000);
  }

  return (
    <span className={styles.copy}>
      <Button
        small
        variant="ghost"
        aria-label={label}
        title={label}
        onClick={() => void copy()}
      >
        {state === "copied" ? (
          <Check size={14} aria-hidden="true" />
        ) : (
          <Copy size={14} aria-hidden="true" />
        )}
      </Button>
      <span
        className={state === "failed" ? styles.copyFail : styles.copyNote}
        aria-hidden="true"
      >
        {state === "copied" ? "Copied" : state === "failed" ? "Not copied" : ""}
      </span>
    </span>
  );
}
