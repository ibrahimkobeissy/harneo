import {
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Info,
  TriangleAlert,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Candidate } from "../../bindings/Candidate";
import type { Preview } from "../../bindings/Preview";
import { Badge, Button } from "../../ui";
import { kindPlurals } from "./kindLabels";
import { claudeProvidedIds, managedNote, originLabel } from "./origin";
import styles from "./Import.module.css";

/** Candidates grouped by kind, in the order kinds first appear. */
export function groupByKind(
  candidates: Candidate[],
): [ArtifactKind, Candidate[]][] {
  const groups = new Map<ArtifactKind, Candidate[]>();
  for (const c of candidates) {
    const list = groups.get(c.artifact.kind) ?? [];
    list.push(c);
    groups.set(c.artifact.kind, list);
  }
  return [...groups];
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** Whether the candidate has at least one error-level diagnostic. */
export function hasErrors(candidate: Candidate): boolean {
  return candidate.artifact.diagnostics.some((d) => d.severity === "error");
}

/** The error and warning counts as a button that opens the diagnostics under the row. */
function Findings({
  candidate,
  open,
  onToggle,
}: {
  candidate: Candidate;
  open: boolean;
  onToggle: () => void;
}) {
  const all = candidate.artifact.diagnostics;
  const errors = all.filter((d) => d.severity === "error").length;
  const warnings = all.filter((d) => d.severity === "warning").length;
  if (errors + warnings === 0) return null;
  return (
    <button
      type="button"
      className={styles.findings}
      aria-expanded={open}
      aria-controls={`diag-${candidate.id}`}
      aria-label={`${open ? "Hide" : "Show"} diagnostics of ${candidate.artifact.name}`}
      onClick={onToggle}
    >
      <span className={styles.badges}>
        {errors > 0 && <Badge tone="bad">{plural(errors, "error")}</Badge>}
        {warnings > 0 && (
          <Badge tone="warn">{plural(warnings, "warning")}</Badge>
        )}
      </span>
      <ChevronDown size={14} aria-hidden="true" data-open={open} />
    </button>
  );
}

/** The scan's findings for one artifact: code, message and file, errors first. */
function DiagnosticList({ candidate }: { candidate: Candidate }) {
  const sorted = [...candidate.artifact.diagnostics].sort(
    (a, b) => Number(b.severity === "error") - Number(a.severity === "error"),
  );
  return (
    <ul className={styles.diags} id={`diag-${candidate.id}`}>
      {sorted.map((d, i) => {
        const Icon = d.severity === "error" ? CircleAlert : TriangleAlert;
        return (
          <li
            key={`${d.code}:${i}`}
            className={styles.diag}
            data-severity={d.severity}
          >
            <Icon size={14} aria-hidden="true" />
            <span>
              <span className={styles.mono}>{d.code}</span>
              <span className={styles.msg}>{d.message}</span>
              <span className={`${styles.mono} ${styles.file}`}>{d.file}</span>
            </span>
          </li>
        );
      })}
    </ul>
  );
}

/** Step 2 while the source is being scanned. */
export function Scanning({ source }: { source: string }) {
  return (
    <div className={styles.panel} role="status">
      <span className={styles.spinner} aria-hidden="true" />
      <div>
        <b>Scanning {source}…</b>
        <div className={styles.sec}>
          Looking for the 10 artifact types: skills, commands, subagents,
          settings, MCP servers, rules, output styles, workflows, plugins,
          CLAUDE.md.
        </div>
      </div>
    </div>
  );
}

/** Groups with more candidates than this start collapsed. */
const LARGE_GROUP = 20;
const EXCLUDED = "excluded";

/** The "N copies" toggle; the paths it opens are listed by [`CopyList`]. */
function Copies({
  candidate,
  open,
  onToggle,
}: {
  candidate: Candidate;
  open: boolean;
  onToggle: () => void;
}) {
  const n = candidate.copies.length;
  if (n === 0) return null;
  return (
    <button
      type="button"
      className={styles.copies}
      aria-expanded={open}
      aria-controls={`copies-${candidate.id}`}
      onClick={onToggle}
    >
      {n} {n === 1 ? "copy" : "copies"}
      <ChevronDown size={12} aria-hidden="true" data-open={open} />
    </button>
  );
}

function CopyList({ candidate }: { candidate: Candidate }) {
  return (
    <ul className={styles.copyList} id={`copies-${candidate.id}`}>
      {candidate.copies.map((path) => (
        <li key={path} className={styles.mono}>
          {path}
        </li>
      ))}
    </ul>
  );
}

/** Step 2: every candidate starts included; the user excludes what they do not want. */
export function ChooseStep({
  preview,
  sourceName,
  excluded,
  onExclude,
  onRestore,
  onIncludeManaged,
  refreshing,
}: {
  preview: Preview;
  sourceName: string;
  excluded: ReadonlySet<string>;
  onExclude: (ids: string[]) => void;
  onRestore: (ids: string[]) => void;
  /** Re-previews with the Claude-managed folders included. */
  onIncludeManaged: () => void;
  refreshing: boolean;
}) {
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());
  const [copiesOpen, setCopiesOpen] = useState<ReadonlySet<string>>(new Set());
  // Per group: what the user chose. Without a choice, only large groups start collapsed.
  const [overrides, setOverrides] = useState<ReadonlyMap<string, boolean>>(
    new Map(),
  );
  const focusAfter = useRef<string | null>(null);

  useEffect(() => {
    const id = focusAfter.current;
    if (!id) return;
    focusAfter.current = null;
    document.querySelector<HTMLElement>(`[data-xid="${id}"]`)?.focus();
  }, [excluded]);

  const all = preview.candidates;
  const groups = groupByKind(all);
  const free = all.filter((c) => !c.alreadyInLibrary);
  const freeIds = free.map((c) => c.id);
  const withErrors = free.filter(hasErrors).map((c) => c.id);
  const errorsOut =
    withErrors.length > 0 && withErrors.every((id) => excluded.has(id));
  const claude = claudeProvidedIds(all);
  const toImport = free.filter((c) => !excluded.has(c.id)).length;
  const gone = all.filter((c) => excluded.has(c.id));
  const note = managedNote(preview.diagnostics);

  const isCollapsed = (key: string, size: number) =>
    overrides.get(key) ?? size > LARGE_GROUP;
  function setCollapsed(key: string, on: boolean) {
    setOverrides((prev) => new Map(prev).set(key, on));
  }
  function setEvery(on: boolean) {
    setOverrides(
      new Map(
        [...groups.map(([k]) => k as string), EXCLUDED].map((k) => [k, on]),
      ),
    );
  }
  const flipIn =
    (set: typeof setOpened) =>
    (id: string): void =>
      set((prev) => {
        const next = new Set(prev);
        if (!next.delete(id)) next.add(id);
        return next;
      });
  const flip = flipIn(setOpened);
  const flipCopies = flipIn(setCopiesOpen);
  /** Excludes one row and queues focus for the row that takes its place. */
  function excludeOne(id: string) {
    const visible = free.filter((c) => !excluded.has(c.id)).map((c) => c.id);
    const at = visible.indexOf(id);
    focusAfter.current = visible[at + 1] ?? visible[at - 1] ?? null;
    onExclude([id]);
  }

  return (
    <>
      <p className={styles.lead}>
        Found {plural(all.length, "artifact")} in{" "}
        <span className={styles.mono}>{sourceName}</span>. Everything is
        included; take out what you do not want.
      </p>
      {note && (
        <div className={styles.callout} data-tone="neutral">
          <Info size={16} aria-hidden="true" />
          <p>{note}</p>
          <Button small disabled={refreshing} onClick={onIncludeManaged}>
            Include them
          </Button>
        </div>
      )}
      <div className={styles.bulk}>
        <Button
          small
          disabled={excluded.size === 0}
          onClick={() => onRestore([...excluded])}
        >
          Restore all
        </Button>
        <Button
          small
          disabled={toImport === 0}
          onClick={() => onExclude(freeIds)}
        >
          Exclude all
        </Button>
        {claude.length > 0 && (
          <Button small onClick={() => onExclude(claude)}>
            Exclude Claude-provided
          </Button>
        )}
        {withErrors.length > 0 && (
          <Button
            small
            aria-pressed={errorsOut}
            onClick={() =>
              errorsOut ? onRestore(withErrors) : onExclude(withErrors)
            }
          >
            Exclude rows with errors
          </Button>
        )}
        <span className={styles.grow} />
        <Button small variant="ghost" onClick={() => setEvery(false)}>
          Expand all
        </Button>
        <Button small variant="ghost" onClick={() => setEvery(true)}>
          Collapse all
        </Button>
      </div>
      <p className={`${styles.muted} ${styles.counter}`} aria-live="polite">
        {toImport} to import, {excluded.size} excluded
      </p>
      {groups.map(([kind, everyone]) => {
        const list = everyone.filter((c) => !excluded.has(c.id));
        if (list.length === 0) return null;
        const ids = list.filter((c) => !c.alreadyInLibrary).map((c) => c.id);
        const back = everyone
          .filter((c) => excluded.has(c.id))
          .map((c) => c.id);
        const label = kindPlurals[kind];
        const shut = isCollapsed(kind, everyone.length);
        return (
          <section
            key={kind}
            className={styles.group}
            aria-label={`${label}, ${list.length}`}
          >
            <header className={styles.ghead}>
              <button
                type="button"
                className={styles.toggle}
                aria-expanded={!shut}
                aria-label={`${shut ? "Expand" : "Collapse"} ${label}`}
                onClick={() => setCollapsed(kind, !shut)}
              >
                <ChevronRight size={14} aria-hidden="true" data-open={!shut} />
                <span>{label}</span>
                <span className={styles.count}>{list.length}</span>
              </button>
              <span className={styles.grow} />
              {back.length > 0 && (
                <Button
                  variant="ghost"
                  small
                  onClick={() => onRestore(back)}
                  aria-label={`Restore all ${label}`}
                >
                  Restore all
                </Button>
              )}
              <Button
                variant="ghost"
                small
                disabled={ids.length === 0}
                onClick={() => onExclude(ids)}
                aria-label={`Exclude all ${label}`}
              >
                Exclude all
              </Button>
            </header>
            {!shut &&
              list.map((c) => {
                const open = opened.has(c.id);
                return (
                  <div key={c.id} className={styles.item}>
                    <div className={styles.line1}>
                      <div className={styles.pick}>
                        <span className={styles.grow}>
                          <span className={styles.title}>
                            <b>{c.artifact.name}</b>
                            <span className={styles.origin}>
                              {originLabel(c.artifact.origin)}
                            </span>
                            {c.alreadyInLibrary && (
                              <Badge>Already in library</Badge>
                            )}
                            <Copies
                              candidate={c}
                              open={copiesOpen.has(c.id)}
                              onToggle={() => flipCopies(c.id)}
                            />
                          </span>
                          <span className={`${styles.line} ${styles.block}`}>
                            <span className={styles.mono}>
                              {c.artifact.path}
                            </span>
                          </span>
                          {c.artifact.description && (
                            <span className={`${styles.desc} ${styles.block}`}>
                              {c.artifact.description}
                            </span>
                          )}
                          {copiesOpen.has(c.id) && <CopyList candidate={c} />}
                        </span>
                      </div>
                      <Findings
                        candidate={c}
                        open={open}
                        onToggle={() => flip(c.id)}
                      />
                      {c.alreadyInLibrary ? (
                        <span className={styles.xslot} />
                      ) : (
                        <button
                          type="button"
                          className={styles.x}
                          data-xid={c.id}
                          aria-label={`Exclude ${c.artifact.name}`}
                          onClick={() => excludeOne(c.id)}
                        >
                          <X size={14} aria-hidden="true" />
                        </button>
                      )}
                    </div>
                    {open && <DiagnosticList candidate={c} />}
                  </div>
                );
              })}
          </section>
        );
      })}
      {gone.length > 0 && (
        <section
          className={styles.group}
          aria-label={`Excluded, ${gone.length}`}
        >
          <header className={styles.ghead}>
            <button
              type="button"
              className={styles.toggle}
              aria-expanded={!isCollapsed(EXCLUDED, gone.length)}
              aria-label={`${isCollapsed(EXCLUDED, gone.length) ? "Expand" : "Collapse"} Excluded`}
              onClick={() =>
                setCollapsed(EXCLUDED, !isCollapsed(EXCLUDED, gone.length))
              }
            >
              <ChevronRight
                size={14}
                aria-hidden="true"
                data-open={!isCollapsed(EXCLUDED, gone.length)}
              />
              <span>Excluded</span>
              <span className={styles.count}>({gone.length})</span>
            </button>
          </header>
          {!isCollapsed(EXCLUDED, gone.length) &&
            gone.map((c) => (
              <div key={c.id} className={styles.item}>
                <div className={styles.line1}>
                  <div className={styles.pick} data-excluded="true">
                    <span className={styles.grow}>
                      <span className={styles.title}>
                        <b>{c.artifact.name}</b>
                        <span className={styles.origin}>
                          {originLabel(c.artifact.origin)}
                        </span>
                        <span className={styles.origin}>
                          {kindPlurals[c.artifact.kind]}
                        </span>
                      </span>
                      <span className={`${styles.line} ${styles.block}`}>
                        <span className={styles.mono}>{c.artifact.path}</span>
                      </span>
                    </span>
                  </div>
                  <Button
                    small
                    variant="ghost"
                    aria-label={`Restore ${c.artifact.name}`}
                    onClick={() => onRestore([c.id])}
                  >
                    Restore
                  </Button>
                </div>
              </div>
            ))}
        </section>
      )}
    </>
  );
}
