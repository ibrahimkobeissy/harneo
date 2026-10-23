import { useEffect, useState } from "react";
import type { DiffStatus } from "../../bindings/DiffStatus";
import type { FileDiff } from "../../bindings/FileDiff";
import type { Hunk } from "../../bindings/Hunk";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { UpdateChoice } from "../../bindings/UpdateChoice";
import type { UpdateReport } from "../../bindings/UpdateReport";
import { Modal } from "../../app/Modal";
import { libraryList, updateAll, updateApply, updateDiff } from "../../lib/ipc";
import { Button } from "../../ui";
import styles from "./Updates.module.css";
import { useEscape } from "./useEscape";

type Diffs =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; files: FileDiff[] };

function useDiff(id: string): Diffs {
  const [state, setState] = useState<{ id: string; diffs: Diffs }>({
    id: "",
    diffs: { status: "loading" },
  });
  useEffect(() => {
    let current = true;
    updateDiff(id).then(
      (files) => {
        if (current)
          setState({ id, diffs: { status: "ready", files: files ?? [] } });
      },
      (err: unknown) => {
        if (current)
          setState({
            id,
            diffs: { status: "error", message: String(err) },
          });
      },
    );
    return () => {
      current = false;
    };
  }, [id]);
  return state.id === id ? state.diffs : { status: "loading" };
}

const marks: Record<DiffStatus, { sign: string; word: string }> = {
  added: { sign: "+", word: "added" },
  changed: { sign: "~", word: "changed" },
  removed: { sign: "−", word: "removed" },
};

export function versionStep(
  from: string | null,
  to: string | null,
): string | null {
  if (from && to) return `${from} → ${to}`;
  if (to) return `→ ${to}`;
  return null;
}

/** One file of the changes list: `+`, `~` or `−`, with the hunks behind a disclosure. */
function ChangeRow({ file }: { file: FileDiff }) {
  const [open, setOpen] = useState(false);
  const { sign, word } = marks[file.status];
  const expandable = file.hunks.length > 0;
  return (
    <li className={styles.change} data-status={file.status}>
      <div className={styles.changeHead}>
        <span className={styles.sign} aria-hidden="true">
          {sign}
        </span>
        <span className={styles.path}>{file.path}</span>
        <span className={styles.word}>{word}</span>
        {expandable ? (
          <button
            type="button"
            className={styles.toggle}
            aria-expanded={open}
            aria-label={`${open ? "Hide" : "Show"} changes in ${file.path}`}
            onClick={() => setOpen(!open)}
          >
            {open ? "Hide changes" : "Show changes"}
          </button>
        ) : (
          <span className={styles.word}>no text preview</span>
        )}
      </div>
      {open && (
        <div className={styles.hunks}>
          {file.hunks.map((h) => (
            <pre key={`${h.oldStart}:${h.newStart}`} className={styles.code}>
              {h.lines.map((l, i) => (
                <span key={i} className={styles.ln} data-kind={l.kind}>
                  {l.kind === "added"
                    ? "+ "
                    : l.kind === "removed"
                      ? "- "
                      : "  "}
                  {l.text}
                  {"\n"}
                </span>
              ))}
            </pre>
          ))}
        </div>
      )}
    </li>
  );
}

function Loading({ diffs }: { diffs: Diffs }) {
  if (diffs.status === "loading")
    return (
      <p role="status" className={styles.muted}>
        Comparing with the source…
      </p>
    );
  if (diffs.status === "error")
    return (
      <p role="alert" className={styles.error}>
        {diffs.message}
      </p>
    );
  return null;
}

/** Everything a dialog says after the call: the outcome, the backup and what to do next. */
function Outcome({ report }: { report: UpdateReport }) {
  if (report.outcome === "failed")
    return (
      <p role="alert" className={styles.error}>
        {report.message ?? "The update failed."}
      </p>
    );
  return (
    <div role="status" className={styles.done}>
      <p>
        {report.outcome === "kept"
          ? "Kept your version. This upstream change is skipped until the source changes again."
          : `Updated the library copy${report.toVersion ? ` to ${report.toVersion}` : ""}. Projects get it when you re-apply or switch their profile.`}
      </p>
      {report.backup && (
        <p>
          Your edited copy was saved to{" "}
          <span className={styles.mono}>{report.backup}</span>.
        </p>
      )}
    </div>
  );
}

/** Update of an artifact without local edits: the changes, then "Update library copy". */
export function UpdateDialog({
  id,
  name,
  fromVersion,
  toVersion,
  onClose,
  onUpdated,
}: {
  id: string;
  name: string;
  fromVersion: string | null;
  toVersion: string | null;
  onClose: () => void;
  onUpdated: (report: UpdateReport) => void;
}) {
  const diffs = useDiff(id);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<UpdateReport | null>(null);
  useEscape(() => {
    if (!busy) onClose();
  });
  async function apply() {
    setBusy(true);
    setError(null);
    try {
      const r = await updateApply(id, "takeUpstream");
      setReport(r);
      if (r.outcome !== "failed") onUpdated(r);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }
  const step = versionStep(fromVersion, toVersion);
  return (
    <Modal label={`Update ${name}`}>
      <div className={styles.dialog}>
        <h2>{step ? `Update ${name}: ${step}` : `Update ${name}`}</h2>
        <p className={styles.sub}>
          Updates the library copy from its source. Projects get it when you
          re-apply or switch their profile.
        </p>
        <div className={styles.scroll}>
          <Loading diffs={diffs} />
          {diffs.status === "ready" && (
            <>
              <h3 className={styles.h3}>Upstream changes</h3>
              {diffs.files.length === 0 ? (
                <p className={styles.muted}>No file differs.</p>
              ) : (
                <ul className={styles.changes}>
                  {diffs.files.map((f) => (
                    <ChangeRow key={f.path} file={f} />
                  ))}
                </ul>
              )}
            </>
          )}
        </div>
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
        {report && <Outcome report={report} />}
        <div className={styles.actions}>
          {report && report.outcome !== "failed" ? (
            <Button variant="primary" onClick={onClose}>
              Done
            </Button>
          ) : (
            <>
              <Button onClick={onClose} disabled={busy}>
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={() => void apply()}
                disabled={busy || diffs.status !== "ready"}
              >
                Update library copy
              </Button>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
}

/** The lines of one side of a hunk: context plus what only that side has. */
function Side({ hunks, side }: { hunks: Hunk[]; side: "mine" | "upstream" }) {
  const only = side === "mine" ? "removed" : "added";
  const start = (h: Hunk) => (side === "mine" ? h.oldStart : h.newStart);
  return (
    <>
      {hunks.map((h) => {
        let no = start(h);
        return (
          <pre key={`${h.oldStart}:${h.newStart}`} className={styles.code}>
            {h.lines
              .filter((l) => l.kind === "context" || l.kind === only)
              .map((l) => (
                <span
                  key={`${no}:${l.kind}`}
                  className={styles.ln}
                  data-kind={l.kind}
                >
                  <span className={styles.no}>{no++}</span>
                  {l.text}
                  {"\n"}
                </span>
              ))}
          </pre>
        );
      })}
    </>
  );
}

function SideBySide({ file }: { file: FileDiff }) {
  const { sign, word } = marks[file.status];
  return (
    <section className={styles.pair} aria-label={file.path}>
      <h3 className={styles.pairHead}>
        <span className={styles.sign} aria-hidden="true">
          {sign}
        </span>
        <span className={styles.path}>{file.path}</span>
        <span className={styles.word}>{word} upstream</span>
      </h3>
      {file.status === "removed" && (
        <p className={styles.muted}>Take upstream deletes this file.</p>
      )}
      {file.hunks.length === 0 ? (
        file.status !== "removed" && (
          <p className={styles.muted}>No text preview for this file.</p>
        )
      ) : (
        <div className={styles.cols}>
          <div className={styles.col}>
            <span className={styles.colHead}>Yours (edited locally)</span>
            <Side hunks={file.hunks} side="mine" />
          </div>
          <div className={styles.col}>
            <span className={styles.colHead}>Upstream</span>
            <Side hunks={file.hunks} side="upstream" />
          </div>
        </div>
      )}
    </section>
  );
}

/**
 * Update of an artifact with local edits: Yours against Upstream per file, then one choice.
 * Harneo never merges: the other version is kept as a backup (take upstream) or skipped.
 */
export function LocalEditDialog({
  id,
  name,
  toVersion,
  onClose,
  onUpdated,
}: {
  id: string;
  name: string;
  toVersion: string | null;
  onClose: () => void;
  onUpdated: (report: UpdateReport) => void;
}) {
  const diffs = useDiff(id);
  const [choice, setChoice] = useState<UpdateChoice>("keepMine");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<UpdateReport | null>(null);
  useEscape(() => {
    if (!busy) onClose();
  });
  async function confirm() {
    setBusy(true);
    setError(null);
    try {
      const r = await updateApply(id, choice);
      setReport(r);
      if (r.outcome !== "failed") onUpdated(r);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }
  const finished = report !== null && report.outcome !== "failed";
  return (
    <Modal label={`${name}: your edits and upstream`} wide>
      <div className={styles.dialog}>
        <h2>{name}: your edits vs upstream</h2>
        <p className={styles.sub}>
          Harneo never merges and never overwrites silently. Pick one version;
          the other is backed up.
        </p>
        <div className={styles.scroll}>
          <Loading diffs={diffs} />
          {diffs.status === "ready" &&
            (diffs.files.length === 0 ? (
              <p className={styles.muted}>No file differs.</p>
            ) : (
              diffs.files.map((f) => <SideBySide key={f.path} file={f} />)
            ))}
        </div>
        {!finished && (
          <fieldset className={styles.choices} disabled={busy}>
            <legend className={styles.sr}>Which version to keep</legend>
            <label className={styles.choice} data-on={choice === "keepMine"}>
              <input
                type="radio"
                name="keep"
                checked={choice === "keepMine"}
                onChange={() => setChoice("keepMine")}
              />
              <span>
                <b>Keep mine</b>
                <span className={styles.sub}>
                  Stay on your edited copy; mark{" "}
                  {toVersion ?? "this upstream change"} as skipped.
                </span>
              </span>
            </label>
            <label
              className={styles.choice}
              data-on={choice === "takeUpstream"}
            >
              <input
                type="radio"
                name="keep"
                checked={choice === "takeUpstream"}
                onChange={() => setChoice("takeUpstream")}
              />
              <span>
                <b>Take upstream{toVersion ? ` ${toVersion}` : ""}</b>
                <span className={styles.sub}>
                  Your edited copy is backed up first, to Harneo&apos;s library
                  backups folder (
                  <span className={styles.mono}>
                    library/.backups/{id}/&lt;date&gt;
                  </span>
                  ).
                </span>
              </span>
            </label>
          </fieldset>
        )}
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
        {report && <Outcome report={report} />}
        <div className={styles.actions}>
          {finished ? (
            <Button variant="primary" onClick={onClose}>
              Done
            </Button>
          ) : (
            <>
              <Button onClick={onClose} disabled={busy}>
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={() => void confirm()}
                disabled={busy || diffs.status !== "ready"}
              >
                Confirm
              </Button>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
}

type Outdated =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; items: LibraryItem[] };

const NAMES_SHOWN = 5;

/** The first few names, then "and N more". */
function editedNames(names: string[]): string {
  if (names.length <= NAMES_SHOWN) return names.join(", ");
  return `${names.slice(0, NAMES_SHOWN).join(", ")} and ${names.length - NAMES_SHOWN} more`;
}

/** Update all: the artifacts that will update, the edited ones that are skipped, the result. */
export function UpdateAllDialog({
  onClose,
  onUpdated,
}: {
  onClose: () => void;
  onUpdated: () => void;
}) {
  const [outdated, setOutdated] = useState<Outdated>({ status: "loading" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reports, setReports] = useState<UpdateReport[] | null>(null);
  useEscape(() => {
    if (!busy) onClose();
  });
  useEffect(() => {
    let current = true;
    libraryList({
      filter: {
        types: [],
        tags: [],
        sources: [],
        text: null,
        trustMin: null,
        tiers: [],
        hasFindings: false,
        outdated: true,
        statuses: [],
      },
      sort: "name",
      descending: false,
      offset: 0,
      limit: 500,
    }).then(
      (page) => {
        if (current) setOutdated({ status: "ready", items: page.items });
      },
      (err: unknown) => {
        if (current) setOutdated({ status: "error", message: String(err) });
      },
    );
    return () => {
      current = false;
    };
  }, []);
  const items = outdated.status === "ready" ? outdated.items : [];
  const edited = items.filter((i) => i.update?.localEdits);
  const clean = items.filter((i) => !i.update?.localEdits);
  const nameOf = (id: string) => items.find((i) => i.id === id)?.name ?? id;
  async function run() {
    setBusy(true);
    setError(null);
    try {
      const out = (await updateAll()) ?? [];
      setReports(out);
      onUpdated();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }
  const updated = reports?.filter((r) => r.outcome === "updated") ?? [];
  const skipped =
    reports?.filter((r) => r.outcome === "skippedLocalEdits") ?? [];
  const failed = reports?.filter((r) => r.outcome === "failed") ?? [];
  return (
    <Modal label="Update all">
      <div className={styles.dialog}>
        <h2>
          {reports
            ? "Update finished"
            : `Update ${items.length} ${items.length === 1 ? "artifact" : "artifacts"}?`}
        </h2>
        {!reports && (
          <>
            <p className={styles.sub}>
              Updates the library copies. Projects get them when they re-apply
              or switch their profile.
            </p>
            {outdated.status === "loading" && (
              <p role="status" className={styles.muted}>
                Looking for outdated artifacts…
              </p>
            )}
            {outdated.status === "error" && (
              <p role="alert" className={styles.error}>
                {outdated.message}
              </p>
            )}
            <div className={styles.scroll}>
              {clean.length > 0 && (
                <ul className={styles.names}>
                  {clean.map((i) => (
                    <li key={i.id}>
                      <span className={styles.path}>{i.name}</span>
                      <span className={styles.word}>
                        {i.update?.toVersion
                          ? `→ ${i.update.toVersion}`
                          : "changed"}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
              {edited.length > 0 && (
                <div className={styles.callout} data-tone="warn" role="note">
                  <p>
                    {editedNames(edited.map((i) => i.name))}{" "}
                    {edited.length === 1 ? "has" : "have"} local edits and{" "}
                    {edited.length === 1 ? "is" : "are"} skipped; update{" "}
                    {edited.length === 1 ? "it" : "them"} one by one to choose.
                  </p>
                </div>
              )}
            </div>
          </>
        )}
        {reports && (
          <div role="status" className={styles.done}>
            <p>
              Updated {updated.length}{" "}
              {updated.length === 1 ? "artifact" : "artifacts"}.
            </p>
            {skipped.length > 0 && (
              <p>
                Skipped for local edits:{" "}
                {skipped.map((r) => nameOf(r.id)).join(", ")}.
              </p>
            )}
            {failed.map((r) => (
              <p key={r.id} className={styles.error}>
                {nameOf(r.id)}: {r.message ?? "the update failed"}
              </p>
            ))}
          </div>
        )}
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
        <div className={styles.actions}>
          {reports ? (
            <Button variant="primary" onClick={onClose}>
              Done
            </Button>
          ) : (
            <>
              <Button onClick={onClose} disabled={busy}>
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={() => void run()}
                disabled={busy || clean.length === 0}
              >
                Update {clean.length}
              </Button>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
}
