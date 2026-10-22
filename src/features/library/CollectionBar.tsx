import { Pencil, SlidersHorizontal, Trash2 } from "lucide-react";
import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router";
import type { Collection } from "../../bindings/Collection";
import type { Filter } from "../../bindings/Filter";
import { Modal } from "../../app/Modal";
import { useCollections } from "../../app/collections";
import {
  collectionDelete,
  collectionRename,
  collectionUpdateRule,
} from "../../lib/ipc";
import { Button, Tag } from "../../ui";
import styles from "./Collections.module.css";
import { kinds } from "./kinds";
import { DialogFrame, NameField } from "./NewCollectionDialog";
import {
  cleanRule,
  RuleBuilder,
  ruleIsEmpty,
  useLibraryFacets,
} from "./RuleBuilder";
import { useEscape } from "./useEscape";

/** The rule as readable chips: types, #tags, source names and the text. */
function RuleChips({ rule }: { rule: Filter }) {
  const facets = useLibraryFacets(rule.sources.length > 0);
  const sourceLabel = (id: string) => {
    const found = facets?.sources.find((s) => s.source.id === id)?.source;
    if (!found) return "a source";
    return found.branch ? `${found.label} @ ${found.branch}` : found.label;
  };
  const text = (rule.text ?? "").trim();
  return (
    <ul className={styles.chips} aria-label="Rule">
      {rule.types.map((t) => (
        <li key={`type-${t}`}>
          <Tag tint="plain">{kinds[t].plural}</Tag>
        </li>
      ))}
      {rule.tags.map((t) => (
        <li key={`tag-${t}`}>
          <Tag tint="plain">#{t}</Tag>
        </li>
      ))}
      {rule.sources.map((s) => (
        <li key={`source-${s}`}>
          <Tag tint="plain">{sourceLabel(s)}</Tag>
        </li>
      ))}
      {text !== "" && (
        <li>
          <Tag tint="plain">“{text}”</Tag>
        </li>
      )}
    </ul>
  );
}

function RenameDialog({
  collection,
  onClose,
}: {
  collection: Collection;
  onClose: () => void;
}) {
  const { refresh } = useCollections();
  const [name, setName] = useState(collection.name);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEscape(onClose);

  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await collectionRename(collection.id, name);
      refresh();
      onClose();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Modal label="Rename collection" onClose={onClose}>
      <DialogFrame
        title="Rename collection"
        onSubmit={(e) => void save(e)}
        footer={
          <>
            <Button onClick={onClose}>Cancel</Button>
            <Button variant="primary" type="submit" disabled={busy}>
              Rename
            </Button>
          </>
        }
      >
        <NameField value={name} onChange={setName} />
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
      </DialogFrame>
    </Modal>
  );
}

function EditRuleDialog({
  collection,
  rule,
  onSaved,
  onClose,
}: {
  collection: Collection;
  rule: Filter;
  onSaved: () => void;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<Filter>(rule);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEscape(onClose);

  async function save(event: FormEvent) {
    event.preventDefault();
    if (ruleIsEmpty(draft)) {
      setError("Add at least one condition to the rule.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await collectionUpdateRule(collection.id, cleanRule(draft));
      onSaved();
      onClose();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Modal label="Edit rule" onClose={onClose}>
      <DialogFrame
        title="Edit rule"
        sub={`Which artifacts “${collection.name}” holds.`}
        onSubmit={(e) => void save(e)}
        footer={
          <>
            <Button onClick={onClose}>Cancel</Button>
            <Button variant="primary" type="submit" disabled={busy}>
              Save rule
            </Button>
          </>
        }
      >
        <RuleBuilder value={draft} onChange={setDraft} />
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
      </DialogFrame>
    </Modal>
  );
}

function DeleteDialog({
  collection,
  onClose,
}: {
  collection: Collection;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const { refresh } = useCollections();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEscape(onClose);

  async function remove() {
    setBusy(true);
    setError(null);
    try {
      await collectionDelete(collection.id);
      refresh();
      onClose();
      void navigate("/library");
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Modal label={`Delete ${collection.name}`} onClose={onClose}>
      <div className={styles.confirm}>
        <h2>Delete “{collection.name}”?</h2>
        <p>
          The artifacts stay in your library; only this grouping is removed.
        </p>
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
        <div className={styles.dfoot}>
          <Button onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button
            variant="danger-solid"
            onClick={() => void remove()}
            disabled={busy}
          >
            Delete collection
          </Button>
        </div>
      </div>
    </Modal>
  );
}

type Dialog = "rename" | "rule" | "delete" | null;

/**
 * Under the collection's title: what it holds (the rule as chips, or "hand-picked") and the
 * rename, edit-rule and delete actions. `onRuleSaved` tells the screen to reload its page.
 */
export function CollectionBar({
  collection,
  onRuleSaved,
}: {
  collection: Collection;
  onRuleSaved: () => void;
}) {
  const [dialog, setDialog] = useState<Dialog>(null);
  const { refresh } = useCollections();
  const close = () => setDialog(null);
  return (
    <div className={styles.bar}>
      <div className={styles.barText}>
        {collection.rule ? (
          <>
            <span className={styles.barLabel}>Saved rule</span>
            <RuleChips rule={collection.rule} />
          </>
        ) : (
          <span className={styles.barLabel}>
            Hand-picked. Add artifacts from their detail panel.
          </span>
        )}
        <p className={styles.note}>
          Collections only organize the library; nothing is installed until you
          use a profile.
        </p>
      </div>
      <div className={styles.barActions}>
        {collection.rule && (
          <Button small onClick={() => setDialog("rule")}>
            <SlidersHorizontal size={14} aria-hidden="true" />
            Edit rule
          </Button>
        )}
        <Button small onClick={() => setDialog("rename")}>
          <Pencil size={14} aria-hidden="true" />
          Rename
        </Button>
        <Button small variant="danger" onClick={() => setDialog("delete")}>
          <Trash2 size={14} aria-hidden="true" />
          Delete
        </Button>
      </div>
      {dialog === "rename" && (
        <RenameDialog collection={collection} onClose={close} />
      )}
      {dialog === "rule" && collection.rule && (
        <EditRuleDialog
          collection={collection}
          rule={collection.rule}
          onSaved={() => {
            refresh();
            onRuleSaved();
          }}
          onClose={close}
        />
      )}
      {dialog === "delete" && (
        <DeleteDialog collection={collection} onClose={close} />
      )}
    </div>
  );
}
