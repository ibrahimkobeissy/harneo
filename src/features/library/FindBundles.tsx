import { Boxes, Ellipsis } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Detection } from "../../bindings/Detection";
import { Modal } from "../../app/Modal";
import {
  bundlesDetectLibrary,
  claudeCliStatus,
  linkAdd,
  linkReject,
  onBundlesDetectProgress,
} from "../../lib/ipc";
import { resolveNames } from "../../lib/names";
import { announce, Button, Proposals, useProposals } from "../../ui";
import { DialogFrame } from "./NewCollectionDialog";
import styles from "./FindBundles.module.css";
import { useEscape } from "./useEscape";

type Phase =
  | { kind: "running"; done: number; total: number }
  | { kind: "missing" }
  | { kind: "error"; message: string }
  | { kind: "done"; detection: Detection };

const noLinks: Detection["links"] = [];

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * "Find bundles with Claude": asks Claude which of `ids` (the whole library when null) need each
 * other and shows its proposals as in the import Review step. Accepting stores them as Claude
 * links; a group switched off is remembered so a re-run does not offer it again.
 */
export function FindBundlesDialog({
  ids,
  onClose,
  onAccepted,
}: {
  ids: string[] | null;
  onClose: () => void;
  onAccepted: () => void;
}) {
  const [phase, setPhase] = useState<Phase>({
    kind: "running",
    done: 0,
    total: 0,
  });
  const [attempt, setAttempt] = useState(0);
  const [names, setNames] = useState<ReadonlyMap<string, string>>(new Map());
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const links = phase.kind === "done" ? phase.detection.links : noLinks;
  const proposals = useProposals(links);
  const scope = useRef(ids);
  useEscape(onClose);

  useEffect(() => {
    let live = true;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const cli = await claudeCliStatus();
        if (!live) return;
        if (!cli?.found) {
          setPhase({ kind: "missing" });
          return;
        }
        stop = await onBundlesDetectProgress((p) => {
          if (live) setPhase({ kind: "running", done: p.done, total: p.total });
        }).catch(() => undefined);
        const detection = await bundlesDetectLibrary(scope.current);
        if (!live) return;
        const found = await resolveNames(
          detection.links.flatMap((l) => [l.from, l.to]),
        );
        if (!live) return;
        setNames(found);
        setPhase({ kind: "done", detection });
      } catch (error) {
        if (live) setPhase({ kind: "error", message: messageOf(error) });
      } finally {
        stop?.();
      }
    })();
    return () => {
      live = false;
    };
  }, [attempt]);

  async function accept() {
    setSaving(true);
    setSaveError(null);
    try {
      for (const l of proposals.accepted) {
        await linkAdd(l.from, l.to, "ai", l.reason);
      }
      for (const l of proposals.rejected) {
        await linkReject(l.from, l.to).catch(() => undefined);
      }
      const n = proposals.accepted.length;
      announce(`Added ${n} link${n === 1 ? "" : "s"} found by Claude`);
      onAccepted();
      onClose();
    } catch (error) {
      setSaveError(messageOf(error));
      setSaving(false);
    }
  }

  const count = proposals.accepted.length;
  const subject =
    ids === null
      ? "the whole library"
      : ids.length === 1
        ? "this artifact"
        : `${ids.length} artifacts`;
  return (
    <Modal label="Find bundles with Claude" onClose={onClose}>
      <DialogFrame
        title="Find bundles with Claude"
        sub={`Claude reads ${subject} through your claude CLI and proposes which artifacts only work together. Nothing is saved until you accept.`}
        onSubmit={(e) => {
          e.preventDefault();
          if (phase.kind === "done" && count > 0) void accept();
        }}
        footer={
          <>
            <Button onClick={onClose}>
              {phase.kind === "done" && count > 0 ? "Cancel" : "Close"}
            </Button>
            {phase.kind === "done" && (
              <Button
                variant="primary"
                type="submit"
                disabled={count === 0 || saving}
              >
                {count === 0
                  ? "Nothing to add"
                  : `Add ${count} link${count === 1 ? "" : "s"}`}
              </Button>
            )}
          </>
        }
      >
        {phase.kind === "running" && (
          <div className={styles.busy} role="status">
            <span className={styles.spinner} aria-hidden="true" />
            <div className={styles.busyText}>
              <b>Claude is reading…</b>
              {phase.total > 0 && (
                <span className={styles.muted}>
                  Batch {Math.min(phase.done + 1, phase.total)} of {phase.total}
                </span>
              )}
              <progress
                className={styles.bar}
                aria-label="Detection progress"
                max={Math.max(phase.total, 1)}
                value={phase.done}
              />
            </div>
          </div>
        )}
        {phase.kind === "missing" && (
          <p role="alert" className={styles.warn}>
            <b>Claude CLI not found.</b> Install it, then try again. You can
            still link artifacts by hand from their detail panel.
          </p>
        )}
        {phase.kind === "error" && (
          <div role="alert" className={styles.fail}>
            <p>Claude couldn&rsquo;t finish: {phase.message}.</p>
            <Button
              small
              onClick={() => {
                setPhase({ kind: "running", done: 0, total: 0 });
                setAttempt((n) => n + 1);
              }}
            >
              Retry
            </Button>
          </div>
        )}
        {phase.kind === "done" && (
          <Proposals
            state={proposals}
            names={names}
            empty="No new bundles found: these artifacts don't depend on each other."
          />
        )}
        {saveError && (
          <p role="alert" className={styles.fail}>
            {saveError}
          </p>
        )}
      </DialogFrame>
    </Modal>
  );
}

/** The toolbar's overflow menu, with Find bundles for the whole library. */
export function LibraryMenu({ onFind }: { onFind: () => void }) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    function onDown(event: MouseEvent) {
      if (wrap.current && !wrap.current.contains(event.target as Node))
        setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      wrap.current?.querySelector<HTMLElement>("button")?.focus();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKeyDown, true);
    };
  }, [open]);
  return (
    <div className={styles.anchor} ref={wrap}>
      <Button
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label="More library actions"
        onClick={() => setOpen(!open)}
      >
        <Ellipsis size={14} aria-hidden="true" />
      </Button>
      {open && (
        <div className={styles.menu} role="menu" aria-label="Library actions">
          <button
            type="button"
            role="menuitem"
            className={styles.menuItem}
            onClick={() => {
              setOpen(false);
              onFind();
            }}
          >
            <Boxes size={14} aria-hidden="true" />
            Find bundles with Claude…
          </button>
        </div>
      )}
    </div>
  );
}
