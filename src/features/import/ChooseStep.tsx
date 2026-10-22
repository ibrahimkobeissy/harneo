import { Check, ChevronDown, CircleAlert, TriangleAlert } from "lucide-react";
import { useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Candidate } from "../../bindings/Candidate";
import type { Preview } from "../../bindings/Preview";
import { Badge, Button } from "../../ui";
import { kindPlurals } from "./kindLabels";
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
  const warnings = all.length - errors;
  if (all.length === 0) return null;
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

/** Step 2: the candidates, grouped by kind, with tick boxes. */
export function ChooseStep({
  preview,
  sourceName,
  selected,
  onToggle,
  onToggleGroup,
  onSelect,
}: {
  preview: Preview;
  sourceName: string;
  selected: ReadonlySet<string>;
  onToggle: (id: string) => void;
  onToggleGroup: (ids: string[], on: boolean) => void;
  /** Replaces the whole selection. */
  onSelect: (ids: string[]) => void;
}) {
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());
  const groups = groupByKind(preview.candidates);
  const free = preview.candidates.filter((c) => !c.alreadyInLibrary);
  const clean = free.filter((c) => !hasErrors(c)).map((c) => c.id);
  const everyFree = free.map((c) => c.id);
  const hasBroken = clean.length < free.length;
  const onlyClean =
    hasBroken &&
    selected.size === clean.length &&
    clean.every((id) => selected.has(id));
  const allSelected =
    everyFree.length > 0 && everyFree.every((id) => selected.has(id));
  function flip(id: string) {
    setOpened((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }
  return (
    <>
      <p className={styles.lead}>
        Found {plural(preview.candidates.length, "artifact")} in{" "}
        <span className={styles.mono}>{sourceName}</span>. Tick what to import.
      </p>
      <div className={styles.bulk}>
        <Button
          small
          disabled={everyFree.length === 0}
          onClick={() => onSelect(allSelected ? [] : everyFree)}
        >
          {allSelected ? "Select none" : "Select all"}
        </Button>
        {hasBroken && (
          <Button
            small
            aria-pressed={onlyClean}
            onClick={() => onSelect(onlyClean ? everyFree : clean)}
          >
            Only without errors
          </Button>
        )}
        <span className={styles.muted} aria-live="polite">
          {selected.size} of {plural(free.length, "artifact")} selected
        </span>
      </div>
      {groups.map(([kind, list]) => {
        const free = list.filter((c) => !c.alreadyInLibrary).map((c) => c.id);
        const allOn = free.length > 0 && free.every((id) => selected.has(id));
        const label = kindPlurals[kind];
        return (
          <section
            key={kind}
            className={styles.group}
            aria-label={`${label}, ${list.length}`}
          >
            <header className={styles.ghead}>
              <span>{label}</span>
              <span className={styles.count}>{list.length}</span>
              <span className={styles.grow} />
              <Button
                variant="ghost"
                small
                disabled={free.length === 0}
                onClick={() => onToggleGroup(free, !allOn)}
                aria-label={`${allOn ? "Select none" : "Select all"} ${label}`}
              >
                {allOn ? "Select none" : "Select all"}
              </Button>
            </header>
            {list.map((c) => {
              const on = selected.has(c.id);
              const open = opened.has(c.id);
              return (
                <div key={c.id} className={styles.item}>
                  <div className={styles.line1}>
                    <button
                      type="button"
                      className={styles.pick}
                      role="checkbox"
                      aria-checked={on}
                      disabled={c.alreadyInLibrary}
                      onClick={() => onToggle(c.id)}
                    >
                      <i
                        className={styles.cb}
                        data-on={on}
                        data-disabled={c.alreadyInLibrary}
                        aria-hidden="true"
                      >
                        {on && <Check size={11} strokeWidth={3} />}
                      </i>
                      <span className={styles.grow}>
                        <b>{c.artifact.name}</b>
                        <span className={`${styles.line} ${styles.block}`}>
                          <span className={styles.mono}>{c.artifact.path}</span>
                        </span>
                        {c.artifact.description && (
                          <span className={`${styles.desc} ${styles.block}`}>
                            {c.artifact.description}
                          </span>
                        )}
                      </span>
                      {c.alreadyInLibrary && <Badge>Already in library</Badge>}
                    </button>
                    <Findings
                      candidate={c}
                      open={open}
                      onToggle={() => flip(c.id)}
                    />
                  </div>
                  {open && <DiagnosticList candidate={c} />}
                </div>
              );
            })}
          </section>
        );
      })}
    </>
  );
}
