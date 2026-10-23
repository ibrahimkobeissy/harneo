import { Info, Save, X } from "lucide-react";
import { useId } from "react";
import type { Layout } from "../../bindings/Layout";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import type { Resolved } from "../../bindings/Resolved";
import { Button, kinds, Panel } from "../../ui";
import styles from "./Builder.module.css";
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

export function Cart({
  name,
  onName,
  saved,
  isNew,
  dirty,
  specChanged,
  busy,
  projectCount,
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
  isNew: boolean;
  dirty: boolean;
  specChanged: boolean;
  busy: boolean;
  projectCount: number;
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
  } else if (dirty) version = `v${saved.version} · rename on save`;
  else version = `v${saved.version} · saved`;

  const counts = countedKinds
    .map((k) => ({
      kind: k,
      n: items?.filter((a) => a.kind === k).length ?? 0,
    }))
    .filter((c) => c.n > 0);

  return (
    <aside className={styles.cart} aria-label="Profile">
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

      <Panel title="In this profile" titleId="cart-items">
        {items && items.length > 0 ? (
          <ul className={styles.cartList} aria-label="Artifacts">
            {items.map((a) => {
              const Icon = kinds[a.kind].icon;
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
      <ProjectTree
        layout={layout}
        resolved={resolved}
        onManualOnly={onManualOnly}
      />

      <Panel title="Totals" titleId="cart-totals">
        <dl className={styles.totals}>
          {counts.map((c) => (
            <div key={c.kind}>
              <dt>{kinds[c.kind].plural}</dt>
              <dd>{c.n}</dd>
            </div>
          ))}
          <div>
            <dt>Settings template</dt>
            <dd>{spec.settings ? 1 : 0}</dd>
          </div>
          <div>
            <dt>MCP servers</dt>
            <dd>{spec.mcp.length}</dd>
          </div>
          <div className={styles.sum}>
            <dt>Files written</dt>
            <dd>{layout?.paths.length ?? 0}</dd>
          </div>
        </dl>
      </Panel>

      {!isNew && projectCount > 0 && (
        <div className={styles.callout}>
          <Info size={14} aria-hidden="true" />
          <p>
            Used by {projectCount} {projectCount === 1 ? "project" : "projects"}
            . Saving creates v{saved.version + 1}; projects keep v
            {saved.version} until you switch them from their page.
          </p>
        </div>
      )}
    </aside>
  );
}
