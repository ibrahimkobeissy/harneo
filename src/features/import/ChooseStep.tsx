import {
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Info,
  ShieldAlert,
  TriangleAlert,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { AuditSeverity } from "../../bindings/AuditSeverity";
import type { Candidate } from "../../bindings/Candidate";
import type { Preview } from "../../bindings/Preview";
import {
  Badge,
  Button,
  FindingTitle,
  Phase1Warning,
  SeverityBadge,
  severityOrder,
  severityTone,
} from "../../ui";
import type { ConfirmProgress } from "../../bindings/ConfirmProgress";
import { ConfirmFailureCallout, ConfirmingNote, DismissedNote } from "./Phase2";
import type { PreviewConfirmation } from "./usePreviewConfirmation";
import { kindPlurals } from "./kindLabels";
import { claudeProvidedIds, managedNote, originLabel } from "./origin";
import { criticalRule, TierLabel } from "./trust";
import { LayoutNotes, PlacementLabel } from "./Layout";
import type { LayoutRun } from "./useLayoutRun";
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
  const audit = candidate.audit.findings;
  const worst = severityOrder.find((s) => audit.some((f) => f.severity === s));
  if (errors + warnings + audit.length === 0) return null;
  const what =
    audit.length > 0 && errors + warnings > 0
      ? "findings and diagnostics"
      : audit.length > 0
        ? "findings"
        : "diagnostics";
  return (
    <button
      type="button"
      className={styles.findings}
      aria-expanded={open}
      aria-controls={`details-${candidate.id}`}
      aria-label={`${open ? "Hide" : "Show"} ${what} of ${candidate.artifact.name}`}
      onClick={onToggle}
    >
      <span className={styles.badges}>
        {errors > 0 && <Badge tone="bad">{plural(errors, "error")}</Badge>}
        {warnings > 0 && (
          <Badge tone="warn">{plural(warnings, "warning")}</Badge>
        )}
        {worst && (
          <Badge tone={severityTone[worst]}>
            {plural(audit.length, "audit finding")}
          </Badge>
        )}
      </span>
      <ChevronDown size={14} aria-hidden="true" data-open={open} />
    </button>
  );
}

/** The audit's findings for one artifact: severity, rule, message, location and evidence. */
function AuditList({
  candidate,
  only,
  idPrefix = "audit",
}: {
  candidate: Candidate;
  /** Lists just this severity (the restore confirm shows the critical findings). */
  only?: AuditSeverity;
  idPrefix?: string;
}) {
  const sorted = candidate.audit.findings
    .filter((f) => only === undefined || f.severity === only)
    .sort(
      (a, b) =>
        severityOrder.indexOf(a.severity) - severityOrder.indexOf(b.severity),
    );
  return (
    <ul
      className={styles.diags}
      id={`${idPrefix}-${candidate.id}`}
      aria-label={`${only ? "Critical findings" : "Audit findings"} of ${candidate.artifact.name}`}
    >
      {sorted.map((f, i) => (
        <li
          key={`${f.rule}:${f.file}:${f.line ?? 0}:${i}`}
          className={styles.diag}
          data-sev={f.severity}
        >
          <ShieldAlert size={14} aria-hidden="true" />
          <span>
            <span>
              <SeverityBadge severity={f.severity} />
              <FindingTitle rule={f.rule} />
              <span className={styles.msg}>{f.message}</span>
            </span>
            <span className={`${styles.mono} ${styles.file}`}>
              {f.file}
              {f.line !== null ? `:${f.line}` : ""}
            </span>
            {f.evidence && (
              <code className={styles.evidence}>{f.evidence}</code>
            )}
          </span>
        </li>
      ))}
    </ul>
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

/** What a row opens to: the scan's diagnostics, then the audit's findings. */
function Details({ candidate }: { candidate: Candidate }) {
  return (
    <div id={`details-${candidate.id}`}>
      {candidate.artifact.diagnostics.length > 0 && (
        <>
          <h4 className={styles.subhead}>Scan diagnostics</h4>
          <DiagnosticList candidate={candidate} />
        </>
      )}
      {candidate.audit.findings.length > 0 && (
        <>
          <h4 className={styles.subhead}>Security audit</h4>
          <AuditList candidate={candidate} />
        </>
      )}
    </div>
  );
}

/** The inline step before a Dangerous artifact comes back: its critical findings, restated. */
function RestoreConfirm({
  candidate,
  onRestore,
  onKeep,
}: {
  candidate: Candidate;
  onRestore: () => void;
  onKeep: () => void;
}) {
  return (
    <div
      className={styles.confirm}
      role="group"
      aria-label={`Restore ${candidate.artifact.name}?`}
      onKeyDown={(ev) => {
        if (ev.key === "Escape") {
          ev.stopPropagation();
          onKeep();
        }
      }}
    >
      <p>
        <b>{candidate.artifact.name}</b> has a critical finding. Restoring it
        puts it back in the import.
      </p>
      <AuditList candidate={candidate} only="critical" idPrefix="critical" />
      <div className={styles.confirmActions}>
        <Button small autoFocus onClick={onKeep}>
          Keep excluded
        </Button>
        <Button small onClick={onRestore}>
          Restore anyway
        </Button>
      </div>
    </div>
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
  progress,
  phase1Only,
  phase2,
  layout,
  onCancelImport,
}: {
  preview: Preview;
  sourceName: string;
  excluded: ReadonlySet<string>;
  onExclude: (ids: string[]) => void;
  onRestore: (ids: string[]) => void;
  /** Re-previews with the Claude-managed folders included. */
  onIncludeManaged: () => void;
  refreshing: boolean;
  /** Phase 2 progress while Claude confirms the flagged candidates in the background. */
  progress: ConfirmProgress | null;
  /** Claude is not confirming findings; null when it is or it is not known yet. */
  phase1Only: { cliMissing: boolean } | null;
  /** Phase 2's state, for the failure callout with its choices. */
  phase2: PreviewConfirmation;
  /** The layout resolver's tier 3 run: progress, failure pause and the unplaced notice. */
  layout: LayoutRun;
  /** Leaves the import. */
  onCancelImport: () => void;
}) {
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());
  const [copiesOpen, setCopiesOpen] = useState<ReadonlySet<string>>(new Set());
  // Per group: what the user chose. Without a choice, only large groups start collapsed.
  const [overrides, setOverrides] = useState<ReadonlyMap<string, boolean>>(
    new Map(),
  );
  const focusAfter = useRef<string | null>(null);
  // The excluded row whose restore is waiting on the user to confirm.
  const [confirming, setConfirming] = useState<string | null>(null);
  const refocusRestore = useRef<string | null>(null);
  // Set by a bulk restore that left critical rows out.
  const [skipNote, setSkipNote] = useState<string | null>(null);

  useEffect(() => {
    const id = refocusRestore.current;
    if (!id || confirming !== null) return;
    refocusRestore.current = null;
    document.querySelector<HTMLElement>(`[data-rid="${id}"]`)?.focus();
  }, [confirming]);

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
  const riskyIds = free.filter(
    (c) => c.audit.tier === "risky" || c.audit.tier === "dangerous",
  );
  const riskyOut =
    riskyIds.length > 0 && riskyIds.every((c) => excluded.has(c.id));
  // Rows with a critical finding come back one by one, through their confirm.
  const riskyBack = riskyIds.filter((c) => criticalRule(c) === null);
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
  /** Bulk restore: rows with a critical finding stay out and come back one by one. */
  function restoreMany(list: Candidate[]) {
    const stay = list.filter((c) => criticalRule(c) !== null);
    onRestore(list.filter((c) => criticalRule(c) === null).map((c) => c.id));
    setSkipNote(
      stay.length === 0
        ? null
        : `${plural(stay.length, "artifact")} with critical findings stayed excluded. Restore ${stay.length === 1 ? "it" : "them"} one by one.`,
    );
  }
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
      {phase1Only && <Phase1Warning cliMissing={phase1Only.cliMissing} />}
      <ConfirmFailureCallout phase2={phase2} onCancel={onCancelImport} />
      <LayoutNotes preview={preview} run={layout} onCancel={onCancelImport} />
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
          onClick={() => restoreMany(all.filter((c) => excluded.has(c.id)))}
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
        {riskyIds.length > 0 && (
          <Button
            small
            aria-pressed={riskyOut}
            disabled={riskyOut && riskyBack.length === 0}
            onClick={() =>
              riskyOut
                ? onRestore(riskyBack.map((c) => c.id))
                : onExclude(riskyIds.map((c) => c.id))
            }
          >
            Exclude Risky and Dangerous
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
      <ConfirmingNote progress={progress} />
      {skipNote && (
        <p className={styles.reason} role="status">
          {skipNote}
        </p>
      )}
      {groups.map(([kind, everyone]) => {
        const list = everyone.filter((c) => !excluded.has(c.id));
        if (list.length === 0) return null;
        const ids = list.filter((c) => !c.alreadyInLibrary).map((c) => c.id);
        const back = everyone.filter((c) => excluded.has(c.id));
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
                  onClick={() => restoreMany(back)}
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
                            <PlacementLabel candidate={c} />
                            <TierLabel candidate={c} />
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
                          <DismissedNote candidate={c} />
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
                    {open && <Details candidate={c} />}
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
            gone.map((c) => {
              const open = opened.has(c.id);
              return (
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
                          <PlacementLabel candidate={c} />
                          <TierLabel candidate={c} />
                        </span>
                        {criticalRule(c) && (
                          <span className={`${styles.reason} ${styles.block}`}>
                            Critical finding:{" "}
                            <FindingTitle rule={criticalRule(c) ?? ""} />
                          </span>
                        )}
                        <span className={`${styles.line} ${styles.block}`}>
                          <span className={styles.mono}>{c.artifact.path}</span>
                        </span>
                        <DismissedNote candidate={c} />
                      </span>
                    </div>
                    <Findings
                      candidate={c}
                      open={open}
                      onToggle={() => flip(c.id)}
                    />
                    <Button
                      small
                      variant="ghost"
                      data-rid={c.id}
                      aria-label={`Restore ${c.artifact.name}`}
                      onClick={() => {
                        if (criticalRule(c) === null) {
                          onRestore([c.id]);
                        } else {
                          refocusRestore.current = c.id;
                          setConfirming(c.id);
                        }
                      }}
                    >
                      Restore
                    </Button>
                  </div>
                  {confirming === c.id && (
                    <RestoreConfirm
                      candidate={c}
                      onKeep={() => setConfirming(null)}
                      onRestore={() => {
                        refocusRestore.current = null;
                        setConfirming(null);
                        onRestore([c.id]);
                      }}
                    />
                  )}
                  {open && <Details candidate={c} />}
                </div>
              );
            })}
        </section>
      )}
    </>
  );
}
