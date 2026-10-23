import { ShieldAlert, X } from "lucide-react";
import { useEffect, useId, useState } from "react";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import { libraryDetail, libraryList } from "../../lib/ipc";
import { Button, kinds, Panel } from "../../ui";
import styles from "./Builder.module.css";
import {
  addMcp,
  mcpEnvKeys,
  placeholderPattern,
  removeMcp,
  setEnv,
} from "./spec";

interface Server {
  id: string;
  name: string;
}

/** MCP servers: add from the library, remove, and edit env placeholders. */
export function McpPanel({
  spec,
  onSpec,
}: {
  spec: ProfileSpec;
  onSpec: (update: (s: ProfileSpec) => ProfileSpec) => void;
}) {
  const selectId = useId();
  const [servers, setServers] = useState<Server[]>([]);
  const [failed, setFailed] = useState<string | null>(null);

  useEffect(() => {
    let stale = false;
    libraryList({
      filter: {
        types: ["mcp"],
        tags: [],
        sources: [],
        text: null,
        trustMin: null,
        tiers: [],
        hasFindings: false,
        outdated: false,
        statuses: [],
      },
      sort: "name",
      descending: false,
      offset: 0,
      limit: 200,
    }).then(
      (page) => {
        if (!stale) {
          setServers(page.items.map((i) => ({ id: i.id, name: i.name })));
        }
      },
      (err: unknown) => {
        if (!stale) setFailed(String(err));
      },
    );
    return () => {
      stale = true;
    };
  }, []);

  const nameOf = (id: string) => servers.find((s) => s.id === id)?.name ?? id;
  const available = servers.filter(
    (s) => !spec.mcp.some((m) => m.artifact === s.id),
  );
  const Icon = kinds.mcp.icon;

  async function add(id: string) {
    if (id === "") return;
    let keys: string[];
    try {
      keys = mcpEnvKeys(await libraryDetail(id));
    } catch (err) {
      setFailed(String(err));
      return;
    }
    onSpec((s) => addMcp(s, id, keys));
  }

  return (
    <Panel
      title="MCP servers"
      titleId="cart-mcp"
      aside={
        <span className={styles.panelNote}>secrets as {"${VAR}"} only</span>
      }
    >
      {failed && (
        <p role="alert" className={styles.errorText}>
          {failed}
        </p>
      )}
      {spec.mcp.length === 0 && <p className={styles.quiet}>No servers.</p>}
      {spec.mcp.map((m) => (
        <div key={m.artifact} className={styles.server}>
          <div className={styles.serverHead}>
            <Icon size={14} aria-hidden="true" />
            <b>{nameOf(m.artifact)}</b>
            <span className={styles.grow} />
            <Button
              variant="ghost"
              small
              aria-label={`Remove ${nameOf(m.artifact)} server`}
              onClick={() => onSpec((s) => removeMcp(s, m.artifact))}
            >
              <X size={13} aria-hidden="true" />
            </Button>
          </div>
          {Object.entries(m.env).map(([key, value]) => (
            <EnvRow
              key={`${key}=${value}`}
              server={m.artifact}
              name={key}
              value={value}
              onCommit={(next) =>
                onSpec((s) => setEnv(s, m.artifact, key, next))
              }
            />
          ))}
        </div>
      ))}
      {available.length > 0 && (
        <>
          <label className={styles.sr} htmlFor={selectId}>
            Add MCP server
          </label>
          <select
            id={selectId}
            className={styles.select}
            value=""
            onChange={(e) => void add(e.target.value)}
          >
            <option value="">Add a server…</option>
            {available.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </>
      )}
    </Panel>
  );
}

/**
 * One env variable. The text is checked when the field loses focus or Enter is pressed: a
 * `${NAME}` placeholder is kept, anything else is dropped (never stored, never echoed) and the
 * stored placeholder comes back. The parent re-keys the row when the stored value changes.
 */
function EnvRow({
  server,
  name,
  value,
  onCommit,
}: {
  server: string;
  name: string;
  value: string;
  onCommit: (value: string) => void;
}) {
  const [text, setText] = useState(value);
  const [refused, setRefused] = useState(false);

  function commit() {
    const next = text.trim();
    if (placeholderPattern.test(next)) {
      setRefused(false);
      if (next !== value) onCommit(next);
      setText(next);
    } else {
      setRefused(true);
      setText(value);
    }
  }
  const inputId = `env-${server}-${name}`;
  return (
    <div>
      <div className={styles.envrow}>
        <label htmlFor={inputId} className={styles.mono}>
          {name}
        </label>
        <input
          id={inputId}
          className={`${styles.input} ${styles.mono} ${styles.envInput}`}
          value={text}
          aria-invalid={refused}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => {
            setText(e.target.value);
            setRefused(false);
          }}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
          }}
        />
      </div>
      {refused && (
        <div className={`${styles.callout} ${styles.warnBox}`} role="alert">
          <ShieldAlert size={14} aria-hidden="true" />
          <p>
            That looked like a real secret, so it wasn&apos;t saved. Kept{" "}
            <span className={styles.mono}>{value}</span>; Harneo stores
            placeholders only.
          </p>
        </div>
      )}
    </div>
  );
}
