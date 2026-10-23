import { useState } from "react";
import { Dialog } from "./Dialog";
import styles from "./Builder.module.css";

/** A removal that touches a bundle and so needs an answer. */
export type RemoveAsk =
  | { kind: "member"; id: string; parents: string[] }
  | { kind: "lead"; id: string; members: string[] };

/** The prototype's "X is part of a bundle" confirm, and its missing twin for a picked lead. */
export function RemoveDialog({
  ask,
  nameOf,
  onConfirm,
  onClose,
}: {
  ask: RemoveAsk;
  nameOf: (id: string) => string;
  onConfirm: (ids: string[]) => void;
  onClose: () => void;
}) {
  const [alsoMembers, setAlsoMembers] = useState(true);
  const name = nameOf(ask.id);
  if (ask.kind === "member") {
    const parents = ask.parents.map(nameOf);
    return (
      <Dialog
        title={`${name} is part of a bundle`}
        submit={`Remove bundle (${parents.length + 1})`}
        danger
        busy={false}
        onSubmit={() => onConfirm([ask.id, ...ask.parents])}
        onClose={onClose}
      >
        <p className={styles.dialogText}>
          <b>{parents.join(", ")}</b> {parents.length === 1 ? "needs" : "need"}{" "}
          it. Removing only {name} would ship a half-wired profile, so the whole
          bundle goes: {name}, {parents.join(", ")}.
        </p>
      </Dialog>
    );
  }
  const members = ask.members.map(nameOf);
  return (
    <Dialog
      title={`Remove ${name}`}
      submit={
        alsoMembers ? `Remove bundle (${members.length + 1})` : `Remove ${name}`
      }
      danger
      busy={false}
      onSubmit={() =>
        onConfirm(alsoMembers ? [ask.id, ...ask.members] : [ask.id])
      }
      onClose={onClose}
    >
      <p className={styles.dialogText}>
        You also picked {members.join(", ")}, which only {name} needs.
      </p>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={alsoMembers}
          onChange={(e) => setAlsoMembers(e.target.checked)}
        />
        Also remove {members.length === 1 ? "that member" : "those members"}
      </label>
    </Dialog>
  );
}
