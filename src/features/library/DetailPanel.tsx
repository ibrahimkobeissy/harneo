import {
  ArrowLeft,
  Boxes,
  CircleAlert,
  CircleArrowUp,
  ExternalLink,
  FileText,
  FolderMinus,
  Trash2,
  TriangleAlert,
  X,
  ShieldAlert,
} from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import type { Collection } from "../../bindings/Collection";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { Audit } from "../../bindings/Audit";
import type { Diagnostic } from "../../bindings/Diagnostic";
import type { Reference } from "../../bindings/Reference";
import type { Links } from "../../bindings/Links";
import type { UpdateStatus } from "../../bindings/UpdateStatus";
import { Modal } from "../../app/Modal";
import {
  collectionRemove,
  libraryDetail,
  libraryFind,
  libraryOpenFile,
  libraryRemove,
  linksOf,
  updateStatus,
} from "../../lib/ipc";
import { useChanged } from "../../lib/useChanged";
import {
  AnyNote,
  Badge,
  Button,
  FindingTitle,
  announce,
  hasAny,
  severityLabels,
  severityOrder,
  TrustMeter,
} from "../../ui";
import { AddToMenu } from "./AddToMenu";
import { LocalEditDialog, UpdateDialog, versionStep } from "./UpdateDialogs";
import updates from "./Updates.module.css";
import { BundleSection } from "./BundleSection";
import { FindBundlesDialog } from "./FindBundles";
import styles from "./Detail.module.css";
import { FileDialog } from "./FileDialog";
import { formatDate } from "./ItemMeta";
import { KindChip } from "./KindChip";
import { kinds } from "./kinds";
import { ManualOnlySection, UsedInSection } from "./ProfileSections";
import { TagsSection } from "./TagsSection";
import { UsageSection } from "./UsageSection";

type Load =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; detail: ArtifactDetail };

/** Loads one artifact; the previous one is dropped as soon as `id` changes. */
function useDetail(id: string, tick: number): Load {
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
  }, [id, tick]);
  return state.id === id ? state.load : { status: "loading" };
}

/** The update state of one artifact, re-read when `tick` changes; null while unknown or current. */
function useUpdateStatus(id: string, tick: number): UpdateStatus | null {
  const [state, setState] = useState<{
    id: string;
    status: UpdateStatus | null;
  }>({ id: "", status: null });
  useEffect(() => {
    let current = true;
    updateStatus(id).then(
      (status) => {
        if (current) setState({ id, status: status ?? null });
      },
      () => {
        if (current) setState({ id, status: null });
      },
    );
    return () => {
      current = false;
    };
  }, [id, tick]);
  return state.id === id ? state.status : null;
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

/** The drawer's notice that the artifact changed at its source, with the Update button. */
function UpdateCallout({
  status,
  onUpdate,
}: {
  status: UpdateStatus | null;
  onUpdate: (status: UpdateStatus) => void;
}) {
  if (status === null || status.state !== "outdated") return null;
  const step = versionStep(status.fromVersion, status.toVersion);
  return (
    <section className={styles.sec} aria-label="Update available">
      <div className={updates.callout} data-tone="warn">
        <CircleArrowUp size={16} aria-hidden="true" />
        <div className={updates.calloutBody}>
          <b>{step ? `Update available: ${step}` : "Update available"}</b>
          <p>
            {status.localEdits
              ? "You edited this artifact locally. You'll choose between your version and upstream."
              : "Upstream changed since you imported it."}
          </p>
        </div>
        <Button small onClick={() => onUpdate(status)}>
          Update
        </Button>
      </div>
    </section>
  );
}

function SourceSection({
  detail,
  update,
}: {
  detail: ArtifactDetail;
  update: UpdateStatus | null;
}) {
  const { source, sourcePath, importedAt } = detail.provenance;
  const imported = formatDate(importedAt);
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
        {update !== null && update.state === "outdated" && (
          <Row label="Version">
            <span className={styles.mono}>
              {update.fromVersion ?? "imported"}
            </span>{" "}
            <Badge tone="warn">
              {update.toVersion ? `${update.toVersion} available` : "changed"}
            </Badge>
          </Row>
        )}
        {update !== null && update.state === "removed" && (
          <Row label="Upstream">
            <Badge tone="bad">Removed at source</Badge>
          </Row>
        )}
        <Row label="Path">
          <span className={styles.mono}>{sourcePath}</span>
        </Row>
        <Row label="Imported">
          <span className={styles.mono}>{imported}</span>
        </Row>
      </dl>
    </section>
  );
}

const reachRows = [
  { key: "files", label: "Files" },
  { key: "network", label: "Network" },
  { key: "shell", label: "Shell" },
  { key: "env", label: "Env vars" },
] as const;

/** Score, tier, findings by severity and what the artifact can reach, from its stored audit. */
function TrustSection({ audit }: { audit: Audit | null }) {
  if (audit === null) {
    return (
      <section className={styles.sec} aria-labelledby="d-trust">
        <h3 id="d-trust" tabIndex={-1}>
          Trust
        </h3>
        <p className={styles.muted}>
          Not audited yet — <Link to="/security">open Security</Link> to run the
          audit.
        </p>
      </section>
    );
  }
  const none = reachRows.every(({ key }) => audit.reach[key].length === 0);
  return (
    <section className={styles.sec} aria-labelledby="d-trust">
      <h3 id="d-trust" tabIndex={-1} className={styles.trustHead}>
        Trust
        <TrustMeter score={audit.score} tier={audit.tier} size="md" />
      </h3>
      {audit.findings.length === 0 ? (
        <p className={styles.muted}>No findings.</p>
      ) : (
        severityOrder.map((severity) => {
          const items = audit.findings.filter((f) => f.severity === severity);
          if (items.length === 0) return null;
          return (
            <div key={severity}>
              <h4 className={styles.sub}>
                {severityLabels[severity]} ({items.length})
              </h4>
              <ul className={styles.diags}>
                {items.map((f, i) => (
                  <li
                    key={`${f.rule}:${f.file}:${f.line ?? 0}:${i}`}
                    className={styles.finding}
                    data-sev={severity}
                  >
                    <ShieldAlert size={14} aria-hidden="true" />
                    <span>
                      <FindingTitle rule={f.rule} />
                      <span className={styles.msg}>{f.message}</span>
                      <span className={`${styles.mono} ${styles.file}`}>
                        {f.line === null ? f.file : `${f.file}:${f.line}`}
                      </span>
                    </span>
                    {f.evidence && (
                      <code className={styles.evidence}>{f.evidence}</code>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          );
        })
      )}
      {audit.reach.protects > 0 && (
        <p className={styles.muted}>
          Protects {audit.reach.protects} credential{" "}
          {audit.reach.protects === 1 ? "path" : "paths"}
        </p>
      )}
      <h4 className={styles.sub}>Reach</h4>
      {none ? (
        <p className={styles.muted}>Nothing beyond its own files.</p>
      ) : (
        <dl className={styles.kv}>
          {reachRows.map(({ key, label }) => (
            <Row key={key} label={label}>
              {audit.reach[key].length === 0 ? (
                <span className={styles.muted}>None</span>
              ) : (
                <span className={styles.mono}>
                  {audit.reach[key].join(", ")}
                </span>
              )}
            </Row>
          ))}
        </dl>
      )}
      {reachRows.some(({ key }) => hasAny(audit.reach[key])) && (
        <AnyNote className={styles.muted} />
      )}
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
  focusTrust = false,
  previousName,
  onOpen,
  onBack,
  onClose,
  onRemoved,
  onTagsChanged,
  onLinksChanged,
  collection = null,
  onMembershipChanged = () => {},
  onLeftCollection = () => {},
  onUpdated = () => {},
}: {
  id: string;
  /** Scroll to the Trust section and move focus to its heading (`?focus=trust`). */
  focusTrust?: boolean;
  previousName: string | null;
  onOpen: (id: string, fromName: string) => void;
  onBack: () => void;
  onClose: () => void;
  onRemoved: () => void;
  onTagsChanged: () => void;
  /** A link was added or removed in the Bundle section (the badges follow). */
  onLinksChanged: () => void;
  /** The collection whose screen this is, when there is one. */
  collection?: Collection | null;
  /** The artifact was added to or taken out of a collection from the Add to… menu. */
  onMembershipChanged?: () => void;
  /** The artifact was taken out of `collection` with "Remove from collection". */
  onLeftCollection?: () => void;
  /** The library copy was updated (or the user kept theirs): the list's badges follow. */
  onUpdated?: () => void;
}) {
  const [tick, setTick] = useState(0);
  useChanged((change) => {
    if (change.scope === "library") setTick((n) => n + 1);
  });
  // The status the dialog opened with: it must outlive the refresh that follows an update.
  const [updating, setUpdating] = useState<UpdateStatus | null>(null);
  const load = useDetail(id, tick);
  const update = useUpdateStatus(id, tick);
  const detail = load.status === "ready" ? load.detail : null;
  const targets = useReferenceTargets(
    detail ? detail.artifact.references : null,
  );
  const [confirming, setConfirming] = useState(false);
  const [finding, setFinding] = useState(false);
  // Links added by Find bundles: the Bundle section remounts on the fresh list.
  const [fresh, setFresh] = useState<{ id: string; links: Links } | null>(null);
  const [linksRev, setLinksRev] = useState(0);
  // Bumped when the Add to… menu saves a profile: Used in follows.
  const [profilesRev, setProfilesRev] = useState(0);
  const [viewing, setViewing] = useState<string | null>(null);
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

  // `?focus=trust`: once the detail has loaded, bring the Trust section into view. Done once per
  // artifact so a reload after a tag change does not pull the panel back.
  const focusedFor = useRef<string | null>(null);
  useEffect(() => {
    if (!focusTrust || detail === null || focusedFor.current === id) return;
    const heading = ref.current?.querySelector<HTMLElement>("#d-trust");
    if (!heading) return;
    focusedFor.current = id;
    heading.scrollIntoView?.({ block: "start" });
    heading.focus({ preventScroll: true });
  }, [focusTrust, detail, id]);

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

  async function openFile(path: string) {
    try {
      await libraryOpenFile(id, path);
    } catch (err) {
      setRemoveError(String(err));
    }
  }

  async function leaveCollection() {
    if (!collection) return;
    setRemoveError(null);
    try {
      await collectionRemove(collection.id, id);
      announce(`Removed from ${collection.name}`);
      onLeftCollection();
    } catch (err) {
      setRemoveError(String(err));
    }
  }

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
              <AddToMenu
                artifactId={id}
                artifactName={detail.artifact.name}
                artifactKind={detail.artifact.kind}
                onChanged={onMembershipChanged}
                onProfileChanged={() => setProfilesRev((n) => n + 1)}
              />
              <Button small onClick={() => setFinding(true)}>
                <Boxes size={14} aria-hidden="true" />
                Find bundles
              </Button>
              {collection?.kind === "manual" && (
                <Button small onClick={() => void leaveCollection()}>
                  <FolderMinus size={14} aria-hidden="true" />
                  Remove from collection
                </Button>
              )}
              <Button
                small
                variant="danger"
                onClick={() => {
                  setRemoveError(null);
                  setConfirming(true);
                }}
              >
                <Trash2 size={14} aria-hidden="true" />
                Delete from library…
              </Button>
            </div>
          )}
        </div>
        <div className={styles.body} data-body>
          {!confirming && removeError && (
            <p role="alert" className={styles.error}>
              {removeError}
            </p>
          )}
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
              <UpdateCallout status={update} onUpdate={setUpdating} />
              <section className={styles.sec}>
                <p className={styles.desc}>
                  {detail.artifact.description ?? "No description."}
                </p>
              </section>
              <TagsSection key={id} artifactId={id} onChanged={onTagsChanged} />
              {detail.artifact.kind === "skill" && (
                <ManualOnlySection
                  key={id}
                  id={id}
                  name={detail.artifact.name}
                  manualOnly={detail.manualOnly}
                  onChanged={onUpdated}
                />
              )}
              <TrustSection audit={detail.audit} />
              <BundleSection
                key={`${id}:${linksRev}`}
                artifactId={id}
                initial={fresh?.id === id ? fresh.links : detail.links}
                onOpen={(next) => onOpen(next, detail.artifact.name)}
                onChanged={onLinksChanged}
              />
              <UsageSection
                kind={detail.artifact.kind}
                name={detail.artifact.name}
              />
              <UsedInSection id={id} rev={profilesRev} />
              <SourceSection detail={detail} update={update} />
              <section className={styles.sec} aria-labelledby="d-files">
                <h3 id="d-files">Files ({detail.files.length})</h3>
                <ul className={styles.files}>
                  {detail.files.map((f) => (
                    <li key={f} className={styles.mono}>
                      <button
                        type="button"
                        className={styles.fileLink}
                        onClick={() => setViewing(f)}
                      >
                        <FileText size={13} aria-hidden="true" />
                        {f}
                      </button>
                      <button
                        type="button"
                        className={styles.fileOpen}
                        aria-label={`Open ${f} in editor`}
                        title="Open in editor"
                        onClick={() => void openFile(f)}
                      >
                        <ExternalLink size={12} aria-hidden="true" />
                      </button>
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
      {updating !== null &&
        detail &&
        (updating.localEdits ? (
          <LocalEditDialog
            id={id}
            name={detail.artifact.name}
            toVersion={updating.toVersion}
            onClose={() => setUpdating(null)}
            onUpdated={() => {
              setTick((n) => n + 1);
              onUpdated();
            }}
          />
        ) : (
          <UpdateDialog
            id={id}
            name={detail.artifact.name}
            fromVersion={updating.fromVersion}
            toVersion={updating.toVersion}
            onClose={() => setUpdating(null)}
            onUpdated={() => {
              setTick((n) => n + 1);
              onUpdated();
            }}
          />
        ))}
      {viewing !== null && (
        <FileDialog id={id} path={viewing} onClose={() => setViewing(null)} />
      )}
      {confirming && detail && (
        <Modal label={`Delete ${detail.artifact.name}`}>
          <div className={styles.confirm}>
            <h2>Delete {detail.artifact.name} from your library?</h2>
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
                Delete
              </Button>
            </div>
          </div>
        </Modal>
      )}
      {finding && (
        <FindBundlesDialog
          ids={[id]}
          onClose={() => setFinding(false)}
          onAccepted={() => {
            void linksOf(id).then(
              (links) => {
                setFresh({ id, links });
                setLinksRev((n) => n + 1);
              },
              () => undefined,
            );
            onLinksChanged();
          }}
        />
      )}
    </>
  );
}
