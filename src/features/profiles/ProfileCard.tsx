import { Copy, FileDown, FolderGit2, Layers } from "lucide-react";
import { Link } from "react-router";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { Button, kinds } from "../../ui";
import styles from "./Profiles.module.css";

export function ProfileCard({
  profile,
  onClone,
  onExport,
}: {
  profile: ProfileSummary;
  onClone: (profile: ProfileSummary) => void;
  onExport: (profile: ProfileSummary) => void;
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
        <span>
          {used > 0
            ? `Used in ${used} ${used === 1 ? "project" : "projects"}`
            : "Not used yet"}
        </span>
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
      </div>
    </li>
  );
}
