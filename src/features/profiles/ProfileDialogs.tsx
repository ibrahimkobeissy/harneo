import { useState } from "react";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { profileDelete, profileRename } from "../../lib/ipc";
import { Dialog, NameField } from "./Dialog";
import styles from "./Profiles.module.css";

/** Asks for a new name; the core refuses a name that is taken. */
export function RenameDialog({
  profile,
  onDone,
  onClose,
}: {
  profile: ProfileSummary;
  onDone: (name: string) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState(profile.name);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function rename() {
    setBusy(true);
    setError(null);
    try {
      await profileRename(profile.id, name.trim());
      onDone(name.trim());
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Dialog
      title="Rename profile"
      sub="A new name is not a new version. Projects keep using the profile."
      submit="Rename"
      busy={busy}
      canSubmit={name.trim() !== "" && name.trim() !== profile.name}
      onSubmit={() => void rename()}
      onClose={onClose}
    >
      <NameField value={name} onChange={setName} />
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </Dialog>
  );
}

/** Confirms a delete; when projects use the profile the core refuses and says which. */
export function DeleteDialog({
  profile,
  onDone,
  onClose,
}: {
  profile: ProfileSummary;
  onDone: () => void;
  onClose: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function remove() {
    setBusy(true);
    setError(null);
    try {
      await profileDelete(profile.id);
      onDone();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Dialog
      title={`Delete ${profile.name}?`}
      sub="The profile and its versions are removed. Library artifacts stay."
      submit="Delete profile"
      danger
      busy={busy}
      onSubmit={() => void remove()}
      onClose={onClose}
    >
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </Dialog>
  );
}
