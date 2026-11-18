import { FileQuestion, Sparkles } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type { Candidate } from "../../bindings/Candidate";
import type { LayoutStage } from "../../bindings/LayoutStage";
import type { Preview } from "../../bindings/Preview";
import { Modal } from "../../app/Modal";
import { Button, Callout } from "../../ui";
import { placementOf } from "./placement";
import type { SplitBlock } from "./splitBlock";
import type { LayoutRun } from "./useLayoutRun";
import styles from "./Import.module.css";

/** How a row was placed. Claude's reason shows on hover and on keyboard focus. */
export function PlacementLabel({ candidate }: { candidate: Candidate }) {
  const [shown, setShown] = useState(false);
  const tipId = useId();
  const placement = placementOf(candidate);
  if (!placement) return null;
  if (!placement.reason) {
    return <span className={styles.origin}>{placement.label}</span>;
  }
  return (
    <span className={styles.placed}>
      <span
        className={`${styles.origin} ${styles.reasoned}`}
        tabIndex={0}
        aria-describedby={shown ? tipId : undefined}
        onMouseEnter={() => setShown(true)}
        onMouseLeave={() => setShown(false)}
        onFocus={() => setShown(true)}
        onBlur={() => setShown(false)}
        onKeyDown={(event) => {
          if (event.key === "Escape" && shown) {
            event.stopPropagation();
            setShown(false);
          }
        }}
      >
        {placement.label}
      </span>
      {shown && (
        <span role="tooltip" id={tipId} className={styles.tip}>
          {placement.reason}
        </span>
      )}
    </span>
  );
}

const stages: Record<LayoutStage, string> = {
  reading: "Reading the repo…",
  asking: "Asking Claude what's in this repo…",
  checking: "Checking Claude's answer…",
  done: "Done",
};

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** What blocks a plugin's parts, in plain words (ADR-008, amended 2026-11-18). */
function blockText({ name, why }: SplitBlock): string {
  switch (why.kind) {
    case "notAsked":
      return `Claude was not asked what the parts of ${name} need, so they cannot be imported yet. Ask Claude, or take the whole plugin.`;
    case "failed":
      return `Claude's call failed, so what the parts of ${name} need is unknown: ${why.message}. Ask Claude again, or take the whole plugin.`;
    case "incomplete":
      return `Claude's answer doesn't cover ${why.missing.map((m) => `\`${m}\``).join(", ")}. Ask Claude again, or take the whole plugin.`;
  }
}

/**
 * The layout step's notes: progress while Claude works, the failure pause with its three
 * choices, and the files Harneo could not place with the button that asks Claude about them.
 */
export function LayoutNotes({
  preview,
  run,
  onCancel,
  blocks = [],
  onWhole,
}: {
  preview: Preview;
  run: LayoutRun;
  onCancel: () => void;
  /** The plugins set to Pick parts that Claude's answer does not allow importing. */
  blocks?: SplitBlock[];
  /** Switches a blocked plugin (by folder) to Whole plugin. */
  onWhole?: (folder: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const listId = useId();
  const working = run.stage !== null;
  const waitRef = useRef<HTMLDivElement>(null);
  const waiting = run.stage !== null && run.failure === null;
  // Focus rests on the dialog itself, not on Cancel: the Modal focuses its first button, then
  // this effect (a parent's, so it runs after) moves focus to the dialog.
  useEffect(() => {
    if (waiting) waitRef.current?.focus();
  }, [waiting]);
  const count = preview.unplaced.length + preview.unplacedOverflow;
  return (
    <>
      {blocks.map((block) => (
        <Callout key={block.folder} tone="bad" icon={Sparkles}>
          <p className={styles.noMargin}>{blockText(block)}</p>
          <div className={styles.failureActions}>
            <Button small disabled={working} onClick={run.ask}>
              {working ? "Asking Claude…" : "Ask Claude again"}
            </Button>
            <Button
              small
              disabled={working}
              onClick={() => onWhole?.(block.folder)}
            >
              Take the whole plugin instead
            </Button>
          </div>
        </Callout>
      ))}
      {blocks.length === 0 && run.failure !== null && !run.skipped && (
        <Callout tone="bad" icon={Sparkles}>
          <p className={styles.noMargin}>
            {`Claude could not read the repo's layout: ${run.failure}. Nothing is imported until you choose.`}
          </p>
          <div className={styles.failureActions}>
            <Button small disabled={working} onClick={run.ask}>
              {working ? "Retrying…" : "Retry"}
            </Button>
            <Button small onClick={run.skip}>
              Continue with what was found
            </Button>
            <Button small variant="ghost" onClick={onCancel}>
              Cancel import
            </Button>
          </div>
        </Callout>
      )}
      {blocks.length === 0 && run.failure !== null && run.skipped && (
        <Callout tone="warn" icon={Sparkles}>
          <p className={styles.noMargin}>
            {`Claude could not read the repo's layout: ${run.failure}. Only what the manifests and known folders show is listed.`}
          </p>
        </Callout>
      )}
      {run.stage !== null && run.failure === null && (
        <Modal label="Claude is reading the repo">
          {/* Keeps the app's own shortcuts (Ctrl+K, ?) away while Claude works; Tab stays trapped. */}
          <div
            className={styles.wait}
            ref={waitRef}
            tabIndex={-1}
            onKeyDown={(e) => {
              if (e.key !== "Tab") e.nativeEvent.stopPropagation();
            }}
          >
            <div className={styles.waitHead} role="status">
              <span className={styles.spinner} aria-hidden="true" />
              <h2>{stages[run.stage]}</h2>
            </div>
            <p className={styles.waitBody}>
              Claude is working out which parts and folders this repo holds.
              This can take a minute; the import goes on once it answers.
            </p>
            <div className={styles.waitFoot}>
              <Button small variant="ghost" onClick={onCancel}>
                Cancel import
              </Button>
            </div>
          </div>
        </Modal>
      )}
      {count > 0 && (
        <div className={styles.callout} data-tone="neutral">
          <FileQuestion size={16} aria-hidden="true" />
          <div>
            <p>{`${plural(count, "file")} Harneo couldn't place`}</p>
            <button
              type="button"
              className={styles.linkButton}
              aria-expanded={open}
              aria-controls={listId}
              onClick={() => setOpen(!open)}
            >
              {open ? "Hide the paths" : "Show the paths"}
            </button>
            {open && (
              <ul className={styles.diags} id={listId}>
                {preview.unplaced.map((path) => (
                  <li key={path} className={`${styles.diag} ${styles.mono}`}>
                    {path}
                  </li>
                ))}
                {preview.unplacedOverflow > 0 && (
                  <li className={styles.diag}>
                    {`and ${preview.unplacedOverflow} more`}
                  </li>
                )}
              </ul>
            )}
          </div>
          <Button small disabled={working} onClick={run.ask}>
            Ask Claude what&apos;s in this repo
          </Button>
        </div>
      )}
    </>
  );
}
