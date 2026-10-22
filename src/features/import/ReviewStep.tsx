import { Plus, TriangleAlert, X } from "lucide-react";
import { useEffect, useState } from "react";
import type { Candidate } from "../../bindings/Candidate";
import type { TagColor } from "../../bindings/TagColor";
import { tagsList } from "../../lib/ipc";
import { Tag } from "../../ui";
import styles from "./Import.module.css";
import { defaultTagColor } from "./tagColor";

/** Core's limit on a tag name (`MAX_NAME_CHARS`). */
const MAX_TAG_CHARS = 40;

/** A button that turns into a one-field form: Enter adds the tag, Escape cancels. */
function AddTag({
  artifact,
  onAdd,
}: {
  artifact: string;
  onAdd: (tag: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  function commit() {
    const name = text.trim();
    if (name) onAdd(name);
    setText("");
    setOpen(false);
  }
  if (!open) {
    return (
      <button
        type="button"
        className={styles.addTag}
        aria-label={`Add a tag to ${artifact}`}
        onClick={() => setOpen(true)}
      >
        <Plus size={12} aria-hidden="true" />
        Add tag
      </button>
    );
  }
  return (
    <input
      className={styles.tagInput}
      aria-label={`New tag for ${artifact}`}
      placeholder="tag name"
      maxLength={MAX_TAG_CHARS}
      value={text}
      autoFocus
      autoComplete="off"
      spellCheck={false}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        const { key } = e;
        if (key === "Enter") {
          e.preventDefault();
          commit();
        } else if (key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          setText("");
          setOpen(false);
        }
      }}
    />
  );
}

/** Step 3: tags to accept or remove, and the secrets the library copy will not hold. */
export function ReviewStep({
  chosen,
  tags,
  onRemoveTag,
  onAddTag,
}: {
  chosen: Candidate[];
  tags: Readonly<Record<string, string[]>>;
  onRemoveTag: (id: string, tag: string) => void;
  onAddTag: (id: string, tag: string) => void;
}) {
  // Existing library tags keep their color; a new name gets the one core will give it.
  const [known, setKnown] = useState<ReadonlyMap<string, TagColor>>(new Map());
  useEffect(() => {
    let live = true;
    tagsList()
      .then((list) => {
        if (live && Array.isArray(list)) {
          setKnown(new Map(list.map((t) => [t.name.toLowerCase(), t.color])));
        }
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);
  const tint = (name: string) =>
    known.get(name.toLowerCase()) ?? defaultTagColor(name);
  const withSecrets = chosen.filter((c) => c.replacedSecretKeys.length > 0);
  return (
    <>
      <h2 className={styles.h}>
        Suggested tags{" "}
        <span>
          from each artifact's keywords: remove any you don't want, or add your
          own
        </span>
      </h2>
      <div className={styles.group}>
        {chosen.map((c) => {
          const own = tags[c.id] ?? [];
          return (
            <div key={c.id} className={styles.row} role="group">
              <span className={styles.artifactName} title={c.artifact.name}>
                {c.artifact.name}
              </span>
              <div className={styles.chips}>
                {own.length === 0 && (
                  <span className={styles.muted}>no tags</span>
                )}
                {own.map((t) => (
                  <Tag key={t} tint={tint(t)} dot>
                    {t}
                    <button
                      type="button"
                      className={styles.chipX}
                      aria-label={`Remove ${t} from ${c.artifact.name}`}
                      onClick={() => onRemoveTag(c.id, t)}
                    >
                      <X size={12} aria-hidden="true" />
                    </button>
                  </Tag>
                ))}
                <AddTag
                  artifact={c.artifact.name}
                  onAdd={(t) => onAddTag(c.id, t)}
                />
              </div>
            </div>
          );
        })}
      </div>
      {withSecrets.length > 0 && (
        <>
          <h2 className={styles.h}>Secrets that will be replaced</h2>
          <div className={styles.callout} data-tone="warn">
            <TriangleAlert size={16} aria-hidden="true" />
            <p>
              These settings hold literal secrets. The library copy keeps a{" "}
              <span className={styles.mono}>{"${KEY}"}</span> placeholder
              instead, so the value is never stored. Values are not shown here.
              Your original files stay as they are.
            </p>
          </div>
          <div className={styles.group}>
            {withSecrets.map((c) => (
              <div key={c.id} className={styles.row} role="group">
                <span className={styles.artifactName} title={c.artifact.name}>
                  {c.artifact.name}
                </span>
                <ul
                  className={styles.secrets}
                  aria-label={`Secrets in ${c.artifact.name}`}
                >
                  {c.replacedSecretKeys.map((k) => (
                    <li key={k} className={styles.secretName}>
                      {k}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </>
      )}
    </>
  );
}
