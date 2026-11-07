import { useState } from "react";
import { useNavigate } from "react-router";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { profileClone } from "../../lib/ipc";
import { Dialog, NameField } from "./Dialog";
import styles from "./Profiles.module.css";

/** Asks for the copy's name, clones, then opens the copy. */
export function CloneDialog({
  profile,
  onClose,
}: {
  profile: ProfileSummary;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const [name, setName] = useState(`${profile.name} copy`);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function clone() {
    setBusy(true);
    setError(null);
    try {
      const id = await profileClone(profile.id, name.trim());
      onClose();
      void navigate(`/profiles/${id}`);
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Dialog
      title="Clone profile"
      sub={`A copy of ${profile.name} v${profile.version}, saved as its own profile at v1.`}
      submit="Clone profile"
      busy={busy}
      canSubmit={name.trim() !== ""}
      onSubmit={() => void clone()}
      onClose={onClose}
    >
      <NameField value={name} onChange={setName} />
      <p className={styles.cloneNote}>
        The copy gets {profile.name}’s map, its history and its saved fits.
        Edits to one never touch the other.
      </p>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </Dialog>
  );
}
