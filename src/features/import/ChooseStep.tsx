import { Check } from "lucide-react";
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

function Findings({ candidate }: { candidate: Candidate }) {
  const all = candidate.artifact.diagnostics;
  const errors = all.filter((d) => d.severity === "error").length;
  const warnings = all.length - errors;
  if (all.length === 0) return null;
  return (
    <span className={styles.badges}>
      {errors > 0 && <Badge tone="bad">{plural(errors, "error")}</Badge>}
      {warnings > 0 && <Badge tone="warn">{plural(warnings, "warning")}</Badge>}
    </span>
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
}: {
  preview: Preview;
  sourceName: string;
  selected: ReadonlySet<string>;
  onToggle: (id: string) => void;
  onToggleGroup: (ids: string[], on: boolean) => void;
}) {
  const groups = groupByKind(preview.candidates);
  return (
    <>
      <p className={styles.lead}>
        Found {plural(preview.candidates.length, "artifact")} in{" "}
        <span className={styles.mono}>{sourceName}</span>. Tick what to import.
      </p>
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
              return (
                <button
                  key={c.id}
                  type="button"
                  className={styles.row}
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
                  <Findings candidate={c} />
                  {c.alreadyInLibrary && <Badge>Already in library</Badge>}
                </button>
              );
            })}
          </section>
        );
      })}
      <p className={styles.muted} aria-live="polite">
        {plural(selected.size, "artifact")} selected.
      </p>
    </>
  );
}
