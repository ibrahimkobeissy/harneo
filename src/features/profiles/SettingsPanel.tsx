import { Eye } from "lucide-react";
import { useEffect, useState } from "react";
import { Link } from "react-router";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import { Modal } from "../../app/Modal";
import { libraryDetail, libraryList } from "../../lib/ipc";
import { Button, Panel } from "../../ui";
import styles from "./Builder.module.css";
import { settingsJson, settingsSummary } from "./spec";

interface Template {
  id: string;
  name: string;
  description: string | null;
  detail: ArtifactDetail | null;
}

/** Every Settings template in the library, with its stored contents. */
async function loadTemplates(): Promise<Template[]> {
  const page = await libraryList({
    filter: {
      types: ["settings"],
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
    limit: 100,
  });
  return Promise.all(
    page.items.map(async (it) => ({
      id: it.id,
      name: it.name,
      description: it.description,
      detail: await libraryDetail(it.id).catch(() => null),
    })),
  );
}

/** Exactly one Settings template, or none: a radio list with each template's summary. */
export function SettingsPanel({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (id: string | null) => void;
}) {
  const [templates, setTemplates] = useState<Template[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [viewing, setViewing] = useState<Template | null>(null);

  useEffect(() => {
    let stale = false;
    loadTemplates().then(
      (t) => {
        if (!stale) setTemplates(t);
      },
      (err: unknown) => {
        if (!stale) setFailed(String(err));
      },
    );
    return () => {
      stale = true;
    };
  }, []);

  const manage = (
    <span className={styles.panelNote}>
      exactly one ·{" "}
      <Link to="/library?type=settings" className={styles.link}>
        Manage
      </Link>
    </span>
  );
  return (
    <Panel title="Settings template" titleId="cart-settings" aside={manage}>
      <p className={styles.panelHelp}>
        The project&apos;s whole{" "}
        <span className={styles.mono}>.claude/settings.json</span>: permissions,
        hooks and their scripts, status line, style, plugins.
      </p>
      {failed && (
        <p role="alert" className={styles.errorText}>
          Templates did not load: {failed}
        </p>
      )}
      <div
        role="radiogroup"
        aria-label="Settings template"
        className={styles.optlist}
      >
        <Option
          selected={value === null}
          name="None"
          description="The project keeps whatever settings it has."
          onPick={() => onChange(null)}
        />
        {templates?.map((t) => (
          <Option
            key={t.id}
            selected={value === t.id}
            name={t.name}
            description={t.description}
            summary={t.detail ? settingsSummary(t.detail) : undefined}
            onPick={() => onChange(t.id)}
            onView={t.detail ? () => setViewing(t) : undefined}
          />
        ))}
      </div>
      {templates && templates.length === 0 && (
        <p className={styles.quiet}>
          No Settings templates in the library yet.
        </p>
      )}
      {viewing?.detail && (
        <SettingsView template={viewing} onClose={() => setViewing(null)} />
      )}
    </Panel>
  );
}

function Option({
  selected,
  name,
  description,
  summary,
  onPick,
  onView,
}: {
  selected: boolean;
  name: string;
  description: string | null;
  summary?: string | undefined;
  onPick: () => void;
  onView?: (() => void) | undefined;
}) {
  return (
    <div className={`${styles.opt} ${selected ? styles.optOn : ""}`}>
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        className={styles.optPick}
        onClick={onPick}
      >
        <i className={styles.radio} aria-hidden="true" />
        <span className={styles.optText}>
          <b>{name}</b>
          {description && <span className={styles.optDesc}>{description}</span>}
          {summary && <span className={styles.optSummary}>{summary}</span>}
        </span>
      </button>
      {onView && (
        <Button
          variant="ghost"
          small
          aria-label={`View ${name} settings.json`}
          title="View settings.json"
          onClick={onView}
        >
          <Eye size={14} aria-hidden="true" />
        </Button>
      )}
    </div>
  );
}

function SettingsView({
  template,
  onClose,
}: {
  template: Template;
  onClose: () => void;
}) {
  // The Modal leaves Esc to its owner.
  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);
  const detail = template.detail;
  if (!detail) return null;
  const scripts = detail.files.filter((f) => !f.endsWith("settings.json"));
  return (
    <Modal label={`${template.name}: settings.json`} wide onClose={onClose}>
      <div className={styles.viewHead}>
        <h2>{template.name}: settings.json</h2>
        <p>
          {settingsSummary(detail)}.
          {scripts.length > 0 &&
            ` Hook scripts are copied with it: ${scripts.join(", ")}.`}
        </p>
      </div>
      <pre className={styles.code} aria-label="settings.json">
        {settingsJson(detail)}
      </pre>
      <div className={styles.viewFoot}>
        <Button onClick={onClose}>Close</Button>
      </div>
    </Modal>
  );
}
