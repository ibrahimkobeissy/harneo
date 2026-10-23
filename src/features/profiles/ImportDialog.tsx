import { useState } from "react";
import type { ProfileImportPreview } from "../../bindings/ProfileImportPreview";
import type { ProfileImportReport } from "../../bindings/ProfileImportReport";
import { profileImportCommit } from "../../lib/ipc";
import { kinds } from "../../ui";
import { Dialog, NameField } from "./Dialog";
import styles from "./Profiles.module.css";

function ArtifactList({
  title,
  items,
}: {
  title: string;
  items: ProfileImportPreview["artifacts"];
}) {
  if (items.length === 0) return null;
  return (
    <div className={styles.group}>
      <h3>{title}</h3>
      <ul className={styles.files}>
        {items.map((a) => (
          <li key={a.id}>
            {a.name}
            <span className={styles.kind}>{kinds[a.kind].label}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** What a profile file would do: artifacts already here, artifacts it adds, a name clash. */
export function ImportDialog({
  path,
  preview,
  onDone,
  onClose,
}: {
  path: string;
  preview: ProfileImportPreview;
  onDone: (report: ProfileImportReport) => void;
  onClose: () => void;
}) {
  const { profile, artifacts, nameClash } = preview;
  const [name, setName] = useState(`${profile.name} (imported)`);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const matched = artifacts.filter((a) => a.inLibrary);
  const added = artifacts.filter((a) => !a.inLibrary);

  async function commit() {
    setBusy(true);
    setError(null);
    try {
      onDone(
        await profileImportCommit(path, nameClash ? name.trim() : undefined),
      );
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Dialog
      title="Import profile file"
      sub={`${profile.name} v${profile.version}: ${matched.length} of ${artifacts.length} artifacts are already in your library.`}
      submit="Import profile"
      busy={busy}
      canSubmit={!nameClash || name.trim() !== ""}
      onSubmit={() => void commit()}
      onClose={onClose}
    >
      {nameClash && (
        <>
          <p className={styles.warn} role="status">
            A profile named “{profile.name}” already exists. Pick another name
            for the imported one.
          </p>
          <NameField value={name} onChange={setName} />
        </>
      )}
      <ArtifactList title="Already in your library" items={matched} />
      <ArtifactList title="New: added to your library" items={added} />
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </Dialog>
  );
}
