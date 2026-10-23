import {
  Copy,
  FileDown,
  FolderGit2,
  Layers,
  MoreHorizontal,
  Pencil,
  Trash2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { Button, kinds } from "../../ui";
import styles from "./Profiles.module.css";

/** The card's "…" menu: Rename and Delete. Esc and a click outside close it. */
function CardMenu({
  profile,
  onRename,
  onDelete,
}: {
  profile: ProfileSummary;
  onRename: (profile: ProfileSummary) => void;
  onDelete: (profile: ProfileSummary) => void;
}) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    function onDown(ev: MouseEvent) {
      if (wrap.current && !wrap.current.contains(ev.target as Node))
        setOpen(false);
    }
    function onKeyDown(ev: KeyboardEvent) {
      if (ev.key !== "Escape") return;
      // Captured so a dialog or screen behind does not also close on this Esc.
      ev.preventDefault();
      ev.stopPropagation();
      setOpen(false);
      wrap.current?.querySelector<HTMLElement>("button")?.focus();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKeyDown, true);
    };
  }, [open]);
  function pick(action: (profile: ProfileSummary) => void) {
    setOpen(false);
    action(profile);
  }
  return (
    <div className={styles.menuAnchor} ref={wrap}>
      <Button
        variant="ghost"
        small
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`More actions for ${profile.name}`}
        title="More"
        onClick={() => setOpen(!open)}
      >
        <MoreHorizontal size={14} aria-hidden="true" />
      </Button>
      {open && (
        <div className={styles.menu} role="menu" aria-label={profile.name}>
          <button
            type="button"
            role="menuitem"
            className={styles.menuItem}
            onClick={() => pick(onRename)}
          >
            <Pencil size={14} aria-hidden="true" />
            Rename
          </button>
          <button
            type="button"
            role="menuitem"
            className={`${styles.menuItem} ${styles.menuDanger}`}
            onClick={() => pick(onDelete)}
          >
            <Trash2 size={14} aria-hidden="true" />
            Delete
          </button>
        </div>
      )}
    </div>
  );
}

export function ProfileCard({
  profile,
  onClone,
  onExport,
  onRename,
  onDelete,
}: {
  profile: ProfileSummary;
  onClone: (profile: ProfileSummary) => void;
  onExport: (profile: ProfileSummary) => void;
  onRename: (profile: ProfileSummary) => void;
  onDelete: (profile: ProfileSummary) => void;
}) {
  const used = profile.projectCount;
  return (
    <li className={styles.card} data-profile-id={String(profile.id)}>
      <Link to={`/profiles/${profile.id}`} className={styles.top}>
        <span className={styles.ico}>
          <Layers size={16} aria-hidden="true" />
        </span>
        <span className={styles.topText}>
          <h2 className={styles.title}>
            <span className={styles.name}>{profile.name}</span>
            <span className={styles.ver}>v{profile.version}</span>
          </h2>
          {profile.description && (
            <p className={styles.desc}>{profile.description}</p>
          )}
        </span>
      </Link>
      <ul className={styles.counts} aria-label="Artifacts in this profile">
        {profile.counts.map(({ kind, count }) => {
          const meta = kinds[kind];
          const Icon = meta.icon;
          return (
            <li key={kind}>
              <Icon size={13} aria-hidden="true" />
              {count} {(count === 1 ? meta.label : meta.plural).toLowerCase()}
            </li>
          );
        })}
        {profile.counts.length === 0 && <li>No artifacts</li>}
      </ul>
      <div className={styles.foot}>
        <FolderGit2 size={14} aria-hidden="true" />
        {used > 0 ? (
          <Link to="/projects" className={styles.used}>
            Used in {used} {used === 1 ? "project" : "projects"}
          </Link>
        ) : (
          <span>Not used yet</span>
        )}
        <Button
          variant="ghost"
          small
          title="Clone"
          aria-label={`Clone ${profile.name}`}
          onClick={() => onClone(profile)}
        >
          <Copy size={13} aria-hidden="true" />
          Clone
        </Button>
        <Button
          variant="ghost"
          small
          title="Export"
          aria-label={`Export ${profile.name}`}
          onClick={() => onExport(profile)}
        >
          <FileDown size={13} aria-hidden="true" />
          Export
        </Button>
        <CardMenu profile={profile} onRename={onRename} onDelete={onDelete} />
      </div>
    </li>
  );
}
