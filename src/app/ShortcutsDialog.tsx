import { Button, Kbd } from "../ui";
import { Modal } from "./Modal";
import styles from "./ShortcutsDialog.module.css";

const shortcuts: readonly { keys: string; does: string }[] = [
  { keys: "Ctrl K", does: "Search and commands" },
  { keys: "?", does: "This list" },
  { keys: "Esc", does: "Close dialog or palette" },
];

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Modal label="Keyboard shortcuts" onClose={onClose}>
      <h2 className={styles.title}>Keyboard shortcuts</h2>
      <dl className={styles.keys}>
        {shortcuts.map((s) => (
          <div key={s.keys} className={styles.row}>
            <dt>{s.does}</dt>
            <dd>
              <Kbd>{s.keys}</Kbd>
            </dd>
          </div>
        ))}
      </dl>
      <div className={styles.foot}>
        <Button onClick={onClose}>Close</Button>
      </div>
    </Modal>
  );
}
