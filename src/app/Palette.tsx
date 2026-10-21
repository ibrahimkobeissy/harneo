import { useState, type KeyboardEvent } from "react";
import { useNavigate } from "react-router";
import { Modal } from "./Modal";
import { actions, places, type Destination } from "./routes";

const entries: readonly (Destination & { group: string })[] = [
  ...places.map((d) => ({ ...d, group: "Places" })),
  ...actions.map((d) => ({ ...d, group: "Actions" })),
];

export function Palette({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);

  const q = query.trim().toLowerCase();
  const matches = entries.filter((entry) =>
    entry.label.toLowerCase().includes(q),
  );
  const current = Math.min(active, Math.max(matches.length - 1, 0));

  function go(entry: Destination | undefined) {
    if (!entry) return;
    onClose();
    void navigate(entry.path);
  }

  function onKeyDown(ev: KeyboardEvent<HTMLInputElement>) {
    if (ev.key === "ArrowDown") {
      ev.preventDefault();
      setActive(Math.min(current + 1, matches.length - 1));
    } else if (ev.key === "ArrowUp") {
      ev.preventDefault();
      setActive(Math.max(current - 1, 0));
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      go(matches[current]);
    }
  }

  return (
    <Modal label="Search and commands">
      <input
        type="text"
        role="combobox"
        aria-label="Search places and actions"
        aria-expanded="true"
        aria-controls="palette-list"
        aria-activedescendant={
          matches[current] ? `palette-${matches[current].path}` : undefined
        }
        placeholder="Search places and actions"
        autoComplete="off"
        autoFocus
        value={query}
        onChange={(ev) => {
          setQuery(ev.target.value);
          setActive(0);
        }}
        onKeyDown={onKeyDown}
      />
      <ul id="palette-list" role="listbox" aria-label="Results">
        {matches.map((m, i) => (
          <li
            key={m.path}
            id={`palette-${m.path}`}
            role="option"
            aria-selected={i === current}
            onClick={() => go(m)}
          >
            {m.label} <small>{m.group}</small>
          </li>
        ))}
      </ul>
      {matches.length === 0 && <p>No matches.</p>}
    </Modal>
  );
}
