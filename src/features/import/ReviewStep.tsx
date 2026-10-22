import { TriangleAlert, X } from "lucide-react";
import type { Candidate } from "../../bindings/Candidate";
import { Tag } from "../../ui";
import styles from "./Import.module.css";

/** Step 3: tags to accept or remove, and the secrets the library copy will not hold. */
export function ReviewStep({
  chosen,
  tags,
  onRemoveTag,
}: {
  chosen: Candidate[];
  tags: Readonly<Record<string, string[]>>;
  onRemoveTag: (id: string, tag: string) => void;
}) {
  const withSecrets = chosen.filter((c) => c.replacedSecretKeys.length > 0);
  return (
    <>
      <h2 className={styles.h}>
        Suggested tags{" "}
        <span>from each artifact's keywords: remove any you don't want</span>
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
                  <span className={styles.muted}>no suggestion</span>
                )}
                {own.map((t) => (
                  <Tag key={t} dot>
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
