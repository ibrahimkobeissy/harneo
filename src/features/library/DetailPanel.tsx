import {
  ArrowLeft,
  CircleAlert,
  FileText,
  Trash2,
  TriangleAlert,
  X,
} from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { Diagnostic } from "../../bindings/Diagnostic";
import type { Reference } from "../../bindings/Reference";
import { Modal } from "../../app/Modal";
import { libraryDetail, libraryFind, libraryRemove } from "../../lib/ipc";
import { Button } from "../../ui";
import styles from "./Detail.module.css";
import { KindChip } from "./KindChip";
import { kinds } from "./kinds";
import { TagsSection } from "./TagsSection";

type Load =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; detail: ArtifactDetail };

/** Loads one artifact; the previous one is dropped as soon as `id` changes. */
function useDetail(id: string): Load {
  const [state, setState] = useState<{ id: string; load: Load }>({
    id: "",
    load: { status: "loading" },
  });
  useEffect(() => {
    let current = true;
    libraryDetail(id).then(
      (detail) => {
        if (current) setState({ id, load: { status: "ready", detail } });
      },
      (err: unknown) => {
        if (current)
          setState({ id, load: { status: "error", message: String(err) } });
      },
    );
    return () => {
      current = false;
    };
  }, [id]);
  return state.id === id ? state.load : { status: "loading" };
}

const refKey = (r: Reference) => `${r.kind}:${r.target}`;

/** For each reference the scan resolved, the library id it opens (null: not imported). */
function useReferenceTargets(
  references: Reference[] | null,
): Record<string, string | null> {
  const [found, setFound] = useState<Record<string, string | null>>({});
  useEffect(() => {
    if (!references) return;
    let current = true;
    for (const r of references) {
      if (!r.resolved) continue;
      libraryFind(r.kind, r.target).then(
        (id) => {
          if (current) setFound((f) => ({ ...f, [refKey(r)]: id }));
        },
        () => {
          if (current) setFound((f) => ({ ...f, [refKey(r)]: null }));
        },
      );
    }
    return () => {
      current = false;
    };
  }, [references]);
  return found;
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </>
  );
}

function SourceSection({ detail }: { detail: ArtifactDetail }) {
  const { source, sourcePath, importedAt } = detail.provenance;
  const imported = new Date(Number(importedAt) * 1000).toLocaleDateString();
  return (
    <section className={styles.sec} aria-labelledby="d-source">
      <h3 id="d-source">Source</h3>
      <dl className={styles.kv}>
        {source.kind === "git" ? (
          <>
            <Row label="URL">
              <span className={styles.mono}>{source.location}</span>
            </Row>
            {source.branch && (
              <Row label="Branch">
                <span className={styles.mono}>{source.branch}</span>
              </Row>
            )}
            {source.subfolder && (
              <Row label="Subfolder">
                <span className={styles.mono}>{source.subfolder}</span>
              </Row>
            )}
            {source.commitSha && (
              <Row label="Commit">
                <span className={styles.mono} title={source.commitSha}>
                  {source.commitSha.slice(0, 7)}
                </span>
              </Row>
            )}
          </>
        ) : (
          <Row label="Folder">
            <span className={styles.mono}>{source.location}</span>
          </Row>
        )}
        <Row label="Path">
          <span className={styles.mono}>{sourcePath}</span>
        </Row>
        <Row label="Imported">{imported}</Row>
      </dl>
    </section>
  );
}

function Diagnostics({ diagnostics }: { diagnostics: Diagnostic[] }) {
  if (diagnostics.length === 0) return null;
  const groups = [
    { severity: "error", title: "Errors", Icon: CircleAlert },
    { severity: "warning", title: "Warnings", Icon: TriangleAlert },
  ] as const;
  return (
    <section className={styles.sec} aria-labelledby="d-diag">
      <h3 id="d-diag">Scan diagnostics</h3>
      {groups.map(({ severity, title, Icon }) => {
        const items = diagnostics.filter((d) => d.severity === severity);
        if (items.length === 0) return null;
        return (
          <div key={severity}>
            <h4 className={styles.sub}>
              {title} ({items.length})
            </h4>
            <ul className={styles.diags}>
              {items.map((d, i) => (
                <li
                  key={`${d.code}:${i}`}
                  className={styles.diag}
                  data-severity={severity}
                >
                  <Icon size={14} aria-hidden="true" />
                  <span>
                    <span className={styles.mono}>{d.code}</span>
                    <span className={styles.msg}>{d.message}</span>
                    <span className={`${styles.mono} ${styles.file}`}>
                      {d.file}
                    </span>
                  </span>
                </li>
              ))}
            </ul>
          </div>
        );
      })}
    </section>
  );
}

function References({
  references,
  targets,
  onOpen,
}: {
  references: Reference[];
  targets: Record<string, string | null>;
  onOpen: (id: string) => void;
}) {
  if (references.length === 0) return null;
  return (
    <section className={styles.sec} aria-labelledby="d-refs">
      <h3 id="d-refs">References</h3>
      <ul className={styles.refs}>
        {references.map((r) => {
          const id = r.resolved ? targets[refKey(r)] : null;
          return (
            <li key={refKey(r)}>
              {id ? (
                <button
                  type="button"
                  className={styles.ref}
                  onClick={() => onOpen(id)}
                >
                  <span className={styles.mono}>{r.target}</span>
                  <span className={styles.muted}>{kinds[r.kind].label}</span>
                </button>
              ) : (
                <span className={styles.refPlain}>
                  <span className={styles.mono}>{r.target}</span>
                  <span className={styles.muted}>
                    {kinds[r.kind].label}, not in library
                  </span>
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/**
 * The detail panel: a non-modal right-hand `dialog` (as in the prototype), so the list behind
 * it stays readable. Focus moves in on open; Esc closes (the caller returns focus to the card).
 * `previousName` is the artifact Back returns to, when this one was opened from a reference.
 */
export function DetailPanel({
  id,
  previousName,
  onOpen,
  onBack,
  onClose,
  onRemoved,
  onTagsChanged,
}: {
  id: string;
  previousName: string | null;
  onOpen: (id: string, fromName: string) => void;
  onBack: () => void;
  onClose: () => void;
  onRemoved: () => void;
  onTagsChanged: () => void;
}) {
  const load = useDetail(id);
  const detail = load.status === "ready" ? load.detail : null;
  const targets = useReferenceTargets(
    detail ? detail.artifact.references : null,
  );
  const [confirming, setConfirming] = useState(false);
  const [removeError, setRemoveError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const ref = useRef<HTMLElement>(null);
  const hasBack = previousName !== null;

  // Focus enters the panel on open and again when a reference swaps the artifact.
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const target =
      node.querySelector<HTMLElement>("[data-back]") ??
      node.querySelector<HTMLElement>("[data-close]");
    (target ?? node).focus();
    const body = node.querySelector("[data-body]");
    if (body) body.scrollTop = 0;
  }, [id, hasBack]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (confirming) {
        setConfirming(false);
        return;
      }
      // Another overlay (palette, shortcuts) owns this Esc.
      if (document.querySelector('[aria-modal="true"]')) return;
      onClose();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [confirming, onClose]);

  async function remove() {
    setBusy(true);
    setRemoveError(null);
    try {
      await libraryRemove(id);
      setConfirming(false);
      onRemoved();
    } catch (err) {
      setRemoveError(String(err));
      setBusy(false);
    }
  }

  const name = detail?.artifact.name ?? "Artifact";
  return (
    <>
      <aside
        ref={ref}
        role="dialog"
        aria-label={`${name} details`}
        tabIndex={-1}
        className={styles.panel}
      >
        <div className={styles.head}>
          {previousName !== null && (
            <button
              type="button"
              data-back
              className={styles.back}
              onClick={onBack}
            >
              <ArrowLeft size={14} aria-hidden="true" />
              Back to {previousName}
            </button>
          )}
          <div className={styles.title}>
            {detail && <KindChip kind={detail.artifact.kind} />}
            <div className={styles.titleText}>
              <h2>{detail ? detail.artifact.name : "Loading…"}</h2>
              {detail && (
                <span className={styles.muted}>
                  {kinds[detail.artifact.kind].label}
                </span>
              )}
            </div>
            <Button
              small
              variant="ghost"
              aria-label="Close details"
              data-close
              onClick={onClose}
            >
              <X size={14} aria-hidden="true" />
            </Button>
          </div>
          {detail && (
            <div className={styles.actions}>
              <Button
                small
                variant="danger"
                onClick={() => {
                  setRemoveError(null);
                  setConfirming(true);
                }}
              >
                <Trash2 size={14} aria-hidden="true" />
                Remove
              </Button>
            </div>
          )}
        </div>
        <div className={styles.body} data-body>
          {load.status === "loading" && (
            <p role="status" className={styles.muted}>
              Loading the artifact…
            </p>
          )}
          {load.status === "error" && (
            <p role="alert" className={styles.error}>
              {load.message}
            </p>
          )}
          {detail && (
            <>
              <section className={styles.sec}>
                <p className={styles.desc}>
                  {detail.artifact.description ?? "No description."}
                </p>
              </section>
              <TagsSection key={id} artifactId={id} onChanged={onTagsChanged} />
              <SourceSection detail={detail} />
              <section className={styles.sec} aria-labelledby="d-files">
                <h3 id="d-files">Files ({detail.files.length})</h3>
                <ul className={styles.files}>
                  {detail.files.map((f) => (
                    <li key={f} className={styles.mono}>
                      <FileText size={13} aria-hidden="true" />
                      {f}
                    </li>
                  ))}
                </ul>
              </section>
              <Diagnostics diagnostics={detail.artifact.diagnostics} />
              <References
                references={detail.artifact.references}
                targets={targets}
                onOpen={(next) => onOpen(next, detail.artifact.name)}
              />
            </>
          )}
        </div>
      </aside>
      {confirming && detail && (
        <Modal label={`Remove ${detail.artifact.name}`}>
          <div className={styles.confirm}>
            <h2>Remove {detail.artifact.name}?</h2>
            <p>
              This deletes {detail.artifact.name} from your library, with its
              tags and collection entries. The original at its source is not
              touched.
            </p>
            {removeError && (
              <p role="alert" className={styles.error}>
                {removeError}
              </p>
            )}
            <div className={styles.confirmActions}>
              <Button onClick={() => setConfirming(false)} disabled={busy}>
                Cancel
              </Button>
              <Button
                variant="danger-solid"
                onClick={() => void remove()}
                disabled={busy}
              >
                Remove
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </>
  );
}
