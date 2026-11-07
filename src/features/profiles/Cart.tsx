import { Info, Save, TriangleAlert, X } from "lucide-react";
import { useId, useState } from "react";
import type { Layout } from "../../bindings/Layout";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import type { Resolved } from "../../bindings/Resolved";
import { Button, kinds, Panel } from "../../ui";
import styles from "./Builder.module.css";
import { HarnmapCard } from "./HarnmapCard";
import { McpPanel } from "./McpPanel";
import { ProjectTree } from "./ProjectTree";
import { SettingsPanel } from "./SettingsPanel";
import { defaultName } from "./spec";

/** The kinds the Totals panel counts, in the prototype's order. */
const countedKinds = [
  "skill",
  "agent",
  "command",
  "rule",
  "plugin",
  "outputStyle",
  "claudeMd",
  "workflow",
] as const;

const COLLAPSED_ROWS = 5;

export function Cart({
  name,
  onName,
  saved,
  profileId,
  isNew,
  dirty,
  specChanged,
  busy,
  projectCount,
  lowTrust,
  spec,
  resolved,
  layout,
  onSave,
  onDiscard,
  onRemove,
  onSpec,
  onManualOnly,
}: {
  name: string;
  onName: (name: string) => void;
  saved: { version: number };
  /** The saved profile's id; `null` until its first save. */
  profileId: bigint | null;
  isNew: boolean;
  dirty: boolean;
  specChanged: boolean;
  busy: boolean;
  projectCount: number;
  /** Trust score of the picked artifacts that score under the warning line. */
  lowTrust: Record<string, number>;
  spec: ProfileSpec;
  resolved: Resolved | null;
  layout: Layout | null;
  onSave: () => void;
  onDiscard: () => void;
  onRemove: (id: string) => void;
  onSpec: (update: (s: ProfileSpec) => ProfileSpec) => void;
  onManualOnly: (id: string, value: boolean) => void;
}) {
  const nameId = useId();
  const items = resolved?.artifacts.filter(
    (a) => a.kind !== "settings" && a.kind !== "mcp",
  );
  const nameOf = (id: string) =>
    resolved?.artifacts.find((a) => a.id === id)?.name ?? id;

  let version: string;
  if (isNew) version = "new · v1";
  else if (specChanged) {
    version = `v${saved.version} → v${saved.version + 1} on save`;
  } else version = `v${saved.version}`;

  const counts = countedKinds
    .map((k) => ({
      kind: k,
      n: items?.filter((a) => a.kind === k).length ?? 0,
    }))
    .filter((c) => c.n > 0);
  const summary = [
    ...counts.map(
      (c) =>
        `${c.n} ${c.n === 1 ? kinds[c.kind].label.toLowerCase() : kinds[c.kind].plural.toLowerCase()}`,
    ),
    ...(spec.settings ? ["1 settings template"] : []),
    ...(spec.mcp.length > 0
      ? [
          `${spec.mcp.length} MCP ${spec.mcp.length === 1 ? "server" : "servers"}`,
        ]
      : []),
  ].join(" · ");
  const conflictCount = layout?.conflicts.length ?? 0;
  const [showAll, setShowAll] = useState(false);
  const shown = showAll ? items : items?.slice(0, COLLAPSED_ROWS);

  return (
    <aside className={styles.cart} aria-label="Profile">
      <div className={styles.cartHead}>
        <div className={styles.field}>
          <label htmlFor={nameId}>Profile name</label>
          <input
            id={nameId}
            className={styles.input}
            value={name}
            placeholder={defaultName}
            maxLength={60}
            autoComplete="off"
            onChange={(e) => onName(e.target.value)}
          />
        </div>
        <div className={styles.versionRow}>
          <span className={styles.mono}>{version}</span>
          <span className={styles.grow} />
          {dirty && !isNew && (
            <Button variant="ghost" small onClick={onDiscard}>
              Discard
            </Button>
          )}
          <Button variant="primary" disabled={busy || !dirty} onClick={onSave}>
            <Save size={14} aria-hidden="true" />
            Save
          </Button>
        </div>
        {conflictCount > 0 && (
          <p className={styles.headWarn}>
            <TriangleAlert size={13} aria-hidden="true" />
            {conflictCount === 1
              ? "1 path is claimed twice."
              : `${conflictCount} paths are claimed twice.`}{" "}
            Remove one claimer before installing.
          </p>
        )}
      </div>

      <div className={styles.cartBody}>
        {profileId !== null && (
          <HarnmapCard profile={profileId} version={saved.version} />
        )}
        <ProjectTree
          layout={layout}
          resolved={resolved}
          summary={summary}
          onRemove={onRemove}
          onManualOnly={onManualOnly}
        />

        <Panel
          title="In this profile"
          titleId="cart-items"
          aside={
            items && items.length > 0 ? (
              <span className={styles.panelNote}>
                {items.length} {items.length === 1 ? "artifact" : "artifacts"}
              </span>
            ) : undefined
          }
        >
          {shown && shown.length > 0 ? (
            <ul
              className={`${styles.cartList} ${showAll ? styles.cartListAll : ""}`}
              aria-label="Artifacts"
            >
              {shown.map((a) => {
                const Icon = kinds[a.kind].icon;
                const trust = lowTrust[a.id];
                return (
                  <li key={a.id}>
                    <Icon size={14} aria-hidden="true" />
                    <span className={styles.cartName}>
                      {a.name}
                      {a.why.type === "pulledBy" && (
                        <span className={styles.why}>
                          pulled in by {nameOf(a.why.artifact)}
                        </span>
                      )}
                    </span>
                    {trust !== undefined && (
                      <span
                        className={styles.trustChip}
                        title="Low trust score: review its findings in the library"
                      >
                        trust {trust}
                      </span>
                    )}
                    {a.manualOnly && (
                      <span className={styles.manual}>manual only</span>
                    )}
                    <Button
                      variant="ghost"
                      small
                      aria-label={`Remove ${a.name} from the profile`}
                      onClick={() => onRemove(a.id)}
                    >
                      <X size={13} aria-hidden="true" />
                    </Button>
                  </li>
                );
              })}
            </ul>
          ) : (
            <p className={styles.quiet}>
              Nothing yet. Add artifacts from the library.
            </p>
          )}
          {items && items.length > COLLAPSED_ROWS && (
            <Button
              variant="ghost"
              small
              aria-expanded={showAll}
              onClick={() => setShowAll(!showAll)}
            >
              {showAll ? "Show fewer" : `Show all ${items.length}`}
            </Button>
          )}
          {resolved && resolved.missing.length > 0 && (
            <p role="alert" className={styles.errorText}>
              No longer in the library: {resolved.missing.join(", ")}.
            </p>
          )}
        </Panel>

        <SettingsPanel
          value={spec.settings}
          onChange={(id) => onSpec((s) => ({ ...s, settings: id }))}
        />
        <McpPanel spec={spec} onSpec={onSpec} />

        {!isNew && projectCount > 0 && (
          <div className={styles.callout}>
            <Info size={14} aria-hidden="true" />
            <p>
              Used by {projectCount}{" "}
              {projectCount === 1 ? "project" : "projects"}. Saving creates v
              {saved.version + 1}; projects keep v{saved.version} until you
              switch them from their page.
            </p>
          </div>
        )}
      </div>
    </aside>
  );
}
