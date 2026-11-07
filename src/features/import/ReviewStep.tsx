import { TriangleAlert, X } from "lucide-react";
import { useEffect, useState } from "react";
import type { Candidate } from "../../bindings/Candidate";
import type { TagColor } from "../../bindings/TagColor";
import type { TagWithCount } from "../../bindings/TagWithCount";
import { tagsList } from "../../lib/ipc";
import type { ConfirmProgress } from "../../bindings/ConfirmProgress";
import type { PreviewConfirmation } from "./usePreviewConfirmation";
import { Button, Tag, TagPicker } from "../../ui";
import styles from "./Import.module.css";
import { AuditSummary } from "./AuditSummary";
import { DetectionSection } from "./DetectionSection";
import { HarnfitSection } from "./HarnfitSection";
import { LargeFilesChoice } from "./LargeFiles";
import type { DetectionControl } from "./useDetection";
import type { ProposalState } from "../../ui";
import { defaultTagColor } from "./tagColor";

/** Core's limit on a tag name (`MAX_NAME_CHARS`). */
const MAX_TAG_CHARS = 40;

/** Step 3: tags to accept or remove, and the secrets the library copy will not hold. */
export function ReviewStep({
  chosen,
  tags,
  colors,
  onRemoveTag,
  onAddTag,
  onAddAll,
  onRemoveAll,
  onClearTags,
  largeFiles,
  onLargeFiles,
  heldBack,
  phase1Only,
  confirming,
  phase2,
  onCancelImport,
  onReviewHeldBack,
  detection,
  proposals,
  names,
  token,
}: {
  chosen: Candidate[];
  detection: DetectionControl;
  proposals: ProposalState;
  names: ReadonlyMap<string, string>;
  /** The preview's cache token, for Harnfit on a candidate that is not imported yet. */
  token: string | null;
  heldBack: number;
  phase1Only: { cliMissing: boolean } | null;
  confirming: ConfirmProgress | null;
  phase2: PreviewConfirmation;
  onCancelImport: () => void;
  onReviewHeldBack: () => void;
  tags: Readonly<Record<string, string[]>>;
  /** Colors picked in Tag all for names that are new to the library (lowercased name). */
  colors: Readonly<Record<string, TagColor>>;
  onAddAll: (name: string, color: TagColor | null) => void;
  onRemoveAll: (name: string) => void;
  /** Takes every tag off every artifact being imported. */
  onClearTags: () => void;
  /** The import's "Copy large files too" switch, also on the Choose step. */
  largeFiles: boolean;
  onLargeFiles: (on: boolean) => void;
  onRemoveTag: (id: string, tag: string) => void;
  onAddTag: (id: string, tag: string, color: TagColor | null) => void;
}) {
  // Existing library tags keep their color; a new name gets the one core will give it.
  const [list, setList] = useState<TagWithCount[]>([]);
  const known = new Map(list.map((t) => [t.name.toLowerCase(), t.color]));
  useEffect(() => {
    let live = true;
    tagsList()
      .then((list) => {
        if (live && Array.isArray(list)) {
          setList(list);
        }
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);
  const tint = (name: string) =>
    known.get(name.toLowerCase()) ??
    colors[name.toLowerCase()] ??
    defaultTagColor(name);
  // Tags that every row carries, in the first row's order and spelling.
  const rows = chosen.map((c) => tags[c.id] ?? []);
  const common = (rows[0] ?? []).filter((t) =>
    rows.every((own) => own.some((o) => o.toLowerCase() === t.toLowerCase())),
  );
  const withSecrets = chosen.filter((c) => c.replacedSecretKeys.length > 0);
  return (
    <>
      <LargeFilesChoice
        candidates={chosen}
        on={largeFiles}
        onChange={onLargeFiles}
      />
      <AuditSummary
        chosen={chosen}
        heldBack={heldBack}
        phase1Only={phase1Only}
        confirming={confirming}
        phase2={phase2}
        onCancelImport={onCancelImport}
        onReviewHeldBack={onReviewHeldBack}
      />
      <DetectionSection
        control={detection}
        proposals={proposals}
        names={names}
        count={chosen.length}
      />
      <h2 className={styles.h}>
        Suggested tags{" "}
        <span>
          from each artifact's keywords
          {detection.phase.kind === "done" && " and Claude"}: remove any you
          don't want, or add your own
        </span>
      </h2>
      <div className={styles.bulkTags}>
        <TagPicker
          existing={list}
          skip={new Set(common.map((t) => t.toLowerCase()))}
          maxChars={MAX_TAG_CHARS}
          onPick={onAddAll}
        />
        <Button
          small
          variant="ghost"
          disabled={chosen.every((c) => (tags[c.id] ?? []).length === 0)}
          onClick={onClearTags}
        >
          <X size={14} aria-hidden="true" />
          Remove all tags
        </Button>
        {common.length > 0 && (
          <span className={styles.bulkLabel}>On every artifact:</span>
        )}
        {common.map((t) => (
          <Tag key={t} tint={tint(t)} dot>
            {t}
            <button
              type="button"
              className={styles.chipX}
              aria-label={`Remove ${t} from all`}
              onClick={() => onRemoveAll(t)}
            >
              <X size={12} aria-hidden="true" />
            </button>
          </Tag>
        ))}
      </div>
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
                <TagPicker
                  existing={list}
                  skip={new Set(own.map((t) => t.toLowerCase()))}
                  maxChars={MAX_TAG_CHARS}
                  label="Add tag"
                  triggerLabel={`Add a tag to ${c.artifact.name}`}
                  onPick={(name, color) => onAddTag(c.id, name, color)}
                />
              </div>
            </div>
          );
        })}
      </div>
      <HarnfitSection chosen={chosen} token={token} />
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
