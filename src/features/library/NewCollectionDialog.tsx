import {
  useEffect,
  useId,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
  type Ref,
} from "react";
import { useLocation, useNavigate } from "react-router";
import type { Filter } from "../../bindings/Filter";
import { Modal } from "../../app/Modal";
import { useCollections } from "../../app/collections";
import { collectionCreateManual, collectionCreateSmart } from "../../lib/ipc";
import { Button } from "../../ui";
import styles from "./Collections.module.css";
import { cleanRule, emptyRule, RuleBuilder, ruleIsEmpty } from "./RuleBuilder";
import { useEscape } from "./useEscape";

type Kind = "manual" | "smart";

const nameRequired = "Give the collection a name.";

/** A titled dialog with a scrolling body and a footer for its buttons. */
export function DialogFrame({
  title,
  sub,
  onSubmit,
  children,
  footer,
}: {
  title: string;
  sub?: string;
  onSubmit: (event: FormEvent) => void;
  children: ReactNode;
  footer: ReactNode;
}) {
  return (
    <form className={styles.dlg} onSubmit={onSubmit}>
      <div className={styles.dhead}>
        <h2>{title}</h2>
        {sub && <p>{sub}</p>}
      </div>
      <div className={styles.dbody}>{children}</div>
      <div className={styles.dfoot}>{footer}</div>
    </form>
  );
}

export function NameField({
  value,
  onChange,
  placeholder,
  invalid = false,
  inputRef,
}: {
  value: string;
  onChange: (name: string) => void;
  placeholder?: string;
  invalid?: boolean;
  inputRef?: Ref<HTMLInputElement>;
}) {
  const id = useId();
  return (
    <div className={styles.field}>
      <label htmlFor={id}>Name</label>
      <input
        id={id}
        ref={inputRef}
        aria-invalid={invalid || undefined}
        className={styles.input}
        value={value}
        placeholder={placeholder}
        autoComplete="off"
        maxLength={60}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

const kindOptions: {
  kind: Kind;
  title: string;
  text: string;
  example: string;
}[] = [
  {
    kind: "manual",
    title: "Manual: a list you pick",
    text: "Starts empty. You add artifacts one by one from their detail panel (Add to… → collection). It only changes when you change it.",
    example:
      "Example: “My review kit”: code-reviewer, /review, security-auditor.",
  },
  {
    kind: "smart",
    title: "Smart: a saved rule",
    text: "Holds every artifact that matches the conditions you set, now and later: when you import something that matches, it appears on its own.",
    example: "Example: “Rust tooling” = everything tagged #rust.",
  },
];

/**
 * The New collection dialog: a name, Manual or Smart, and for Smart the rule builder. With
 * `rule` it opens on Smart with that rule (the Library's current filters). Creating goes to
 * the new collection's screen.
 */
export function NewCollectionDialog({
  rule,
  onClose,
}: {
  rule?: Filter;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const { refresh } = useCollections();
  const [kind, setKind] = useState<Kind>(rule ? "smart" : "manual");
  const [name, setName] = useState("");
  const [draft, setDraft] = useState<Filter>(rule ?? emptyRule);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);
  useEscape(onClose);

  // Any navigation (sidebar, palette, Back) closes the dialog instead of leaving it over a new screen.
  const { key: routeKey } = useLocation();
  const openedAt = useRef(routeKey);
  useEffect(() => {
    if (routeKey !== openedAt.current) onClose();
  }, [routeKey, onClose]);

  async function create(event: FormEvent) {
    event.preventDefault();
    const finalName = name.trim();
    if (finalName === "") {
      setError(nameRequired);
      nameRef.current?.focus();
      return;
    }
    if (kind === "smart" && ruleIsEmpty(draft)) {
      setError("Add at least one condition to the rule.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const made =
        kind === "smart"
          ? await collectionCreateSmart(finalName, cleanRule(draft))
          : await collectionCreateManual(finalName);
      refresh();
      onClose();
      void navigate(`/collections/${made.id}`);
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Modal label="New collection" onClose={onClose}>
      <DialogFrame
        title="New collection"
        sub="Collections organize your library: they group artifacts so you find them faster. They never install anything."
        onSubmit={(e) => void create(e)}
        footer={
          <>
            <Button onClick={onClose}>Cancel</Button>
            <Button variant="primary" type="submit" disabled={busy}>
              Create collection
            </Button>
          </>
        }
      >
        <NameField
          value={name}
          onChange={setName}
          inputRef={nameRef}
          invalid={error === nameRequired}
          placeholder={
            kind === "smart" ? "e.g. Rust tooling" : "e.g. My review kit"
          }
        />
        <div className={styles.kinds} role="radiogroup" aria-label="Kind">
          {kindOptions.map((o) => (
            <button
              key={o.kind}
              type="button"
              role="radio"
              aria-checked={kind === o.kind}
              className={
                kind === o.kind ? `${styles.opt} ${styles.optOn}` : styles.opt
              }
              onClick={() => setKind(o.kind)}
            >
              <i className={styles.radio} aria-hidden="true" />
              <span>
                <b>{o.title}</b>
                <span className={styles.optText}>{o.text}</span>
                <span className={styles.muted}>{o.example}</span>
              </span>
            </button>
          ))}
        </div>
        {kind === "smart" && (
          <>
            <h3 className={styles.ruleHead}>
              Rule
              {rule && (
                <span className={styles.muted}>
                  pre-filled from your current filters
                </span>
              )}
            </h3>
            <RuleBuilder value={draft} onChange={setDraft} />
          </>
        )}
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
      </DialogFrame>
    </Modal>
  );
}

/** Shows the New collection dialog whenever something asks for it (see `useCollections`). */
export function NewCollectionHost() {
  const { request, closeNew } = useCollections();
  if (!request) return null;
  return <NewCollectionDialog rule={request.rule} onClose={closeNew} />;
}
