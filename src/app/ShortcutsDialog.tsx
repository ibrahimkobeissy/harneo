import { Modal } from "./Modal";

const shortcuts: readonly { keys: string; does: string }[] = [
  { keys: "Ctrl K", does: "Search and commands" },
  { keys: "?", does: "This list" },
  { keys: "Esc", does: "Close dialog or palette" },
];

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Modal label="Keyboard shortcuts">
      <h2>Keyboard shortcuts</h2>
      <dl>
        {shortcuts.map((s) => (
          <div key={s.keys}>
            <dt>
              <kbd>{s.keys}</kbd>
            </dt>
            <dd>{s.does}</dd>
          </div>
        ))}
      </dl>
      <button type="button" onClick={onClose}>
        Close
      </button>
    </Modal>
  );
}
