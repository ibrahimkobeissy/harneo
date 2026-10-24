import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { OpenArtifactButton } from "../../lib/artifactPanel";
import type { CleanupItem } from "../../bindings/CleanupItem";
import type { PrunePlan } from "../../bindings/PrunePlan";
import type { PruneReport } from "../../bindings/PruneReport";
import { Modal } from "../../app/Modal";
import { libraryRemove, prunePlan, pruneApply } from "../../lib/ipc";
import { Button, type ButtonVariant } from "../../ui";
import styles from "./Cleanup.module.css";

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** A confirm: title, body, Cancel and one action. Esc and the scrim close it, except while it applies. */
export function Confirm({
  title,
  submit,
  variant = "primary",
  busy,
  canSubmit = true,
  onSubmit,
  onClose,
  children,
}: {
  title: string;
  submit: string;
  variant?: ButtonVariant;
  busy: boolean;
  canSubmit?: boolean;
  onSubmit: () => void;
  onClose: () => void;
  children: ReactNode;
}) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !event.defaultPrevented && !busy) onClose();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose, busy]);
  function submitForm(event: FormEvent) {
    event.preventDefault();
    onSubmit();
  }
  return (
    <Modal label={title} onClose={busy ? undefined : onClose}>
      <form className={styles.dlg} onSubmit={submitForm} aria-busy={busy}>
        <div className={styles.dhead}>
          <h2>{title}</h2>
        </div>
        <div className={styles.dbody}>{children}</div>
        <div className={styles.dfoot}>
          <Button onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant={variant} type="submit" disabled={busy || !canSubmit}>
            {submit}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

/** Remove from the library, for a risky artifact or one in no profile. */
export function RemoveFromLibrary({
  item,
  onClose,
  onDone,
}: {
  item: CleanupItem;
  onClose: () => void;
  onDone: (text: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function run() {
    setBusy(true);
    setError(null);
    try {
      await libraryRemove(item.id);
      onDone(`Removed ${item.name} from the library`);
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }
  return (
    <Confirm
      title={`Remove ${item.name} from the library?`}
      submit={busy ? "Removing…" : "Remove from library"}
      variant="danger-solid"
      busy={busy}
      onSubmit={() => void run()}
      onClose={onClose}
    >
      <p>
        Deletes the library copy.
        {item.profiles.length > 0
          ? " Profiles that install it are listed below."
          : " It is in no profile."}
      </p>
      {item.profiles.length > 0 && (
        <ul aria-label="Profiles it is in">
          {item.profiles.map((p) => (
            <li key={String(p.id)}>
              {p.name} <span className={styles.mono}>v{p.version}</span>
            </li>
          ))}
        </ul>
      )}
      {error && (
        <p className={styles.err} role="alert">
          Nothing was changed. {error}
        </p>
      )}
    </Confirm>
  );
}

/** The prune confirm: the profiles that save a new version, the projects that switch. */
export function PruneDialog({
  item,
  onClose,
  onDone,
}: {
  item: CleanupItem;
  onClose: () => void;
  onDone: (report: PruneReport) => void;
}) {
  const [plan, setPlan] = useState<PrunePlan | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    prunePlan(item.id).then(
      (next) => {
        if (live) setPlan(next);
      },
      (err: unknown) => {
        if (live) setPlanError(messageOf(err));
      },
    );
    return () => {
      live = false;
    };
  }, [item.id]);

  async function run() {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      onDone(await pruneApply(item.id));
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  const profiles = plan?.profiles ?? [];
  const projects = plan?.projects ?? [];
  const stay = plan?.notSwitched ?? [];
  const count = plan ? profiles.length : item.profiles.length;
  return (
    <Confirm
      title={`Remove ${item.name} from ${plural(count, "profile")}?`}
      submit={
        busy
          ? "Removing…"
          : projects.length > 0
            ? `Remove and push to ${plural(projects.length, "project")}`
            : "Remove from profiles"
      }
      busy={busy}
      canSubmit={plan !== null}
      onSubmit={() => void run()}
      onClose={onClose}
    >
      {plan && (
        <>
          <p>
            Takes it out of these profiles; each saves a new version. It stays
            in your library.
          </p>
          <ul aria-label="Profiles that change">
            {profiles.map((p) => (
              <li key={String(p.id)}>
                <b>{p.name}</b>{" "}
                <span className={styles.mono}>
                  v{p.fromVersion} → v{p.toVersion}
                </span>
              </li>
            ))}
          </ul>
          {projects.length > 0 && (
            <>
              <p>
                These projects switch to the new versions, with a backup of
                every changed file.
              </p>
              <ul aria-label="Projects that switch">
                {projects.map((p) => (
                  <li key={String(p.id)}>{p.name}</li>
                ))}
              </ul>
            </>
          )}
          {stay.length > 0 && (
            <>
              <p>These projects are pinned and stay as they are.</p>
              <ul aria-label="Projects that stay">
                {stay.map((p) => (
                  <li key={String(p.id)}>
                    {p.name} stays on{" "}
                    <span className={styles.mono}>v{p.profileVersion}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      )}
      {!plan && !planError && <p role="status">Working out what changes…</p>}
      {planError && (
        <p className={styles.err} role="alert">
          Could not prepare the removal. {planError}
        </p>
      )}
      {error && (
        <p className={styles.err} role="alert">
          {error}
        </p>
      )}
    </Confirm>
  );
}

/** Update all: what is updated and which ones are skipped for local edits. */
export function UpdateAllDialog({
  total,
  edited,
  onClose,
  onConfirm,
  busy,
}: {
  total: number;
  edited: CleanupItem[];
  onClose: () => void;
  onConfirm: () => void;
  busy: boolean;
}) {
  const n = total - edited.length;
  return (
    <Confirm
      title={`Update ${n} of ${plural(total, "artifact")}?`}
      submit={busy ? "Updating…" : `Update ${n}`}
      busy={busy}
      canSubmit={n > 0}
      onSubmit={onConfirm}
      onClose={onClose}
    >
      <p>
        Updates the library copies. Projects get them when they re-apply or
        switch.
      </p>
      {edited.length > 0 && (
        <p className={styles.warn}>
          {edited.map((a, i) => (
            <span key={a.id}>
              {i > 0 && ", "}
              <OpenArtifactButton
                className={styles.warnLink}
                id={a.id}
                onOpened={onClose}
              >
                {a.name}
              </OpenArtifactButton>
            </span>
          ))}{" "}
          {edited.length === 1 ? "has" : "have"} local edits and{" "}
          {edited.length === 1 ? "is" : "are"} skipped; review{" "}
          {edited.length === 1 ? "it" : "them"} one by one to choose.
        </p>
      )}
    </Confirm>
  );
}
