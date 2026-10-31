import { CircleCheck, LayoutGrid } from "lucide-react";
import { Link } from "react-router";
import type { ImportReport } from "../../bindings/ImportReport";
import type { SkipReason } from "../../bindings/SkipReason";
import styles from "./Import.module.css";

const reasons: Record<SkipReason, string> = {
  notRegularFile: "not a regular file (link or special file)",
  secretFile: "looks like a secret file",
  localFile: "a local-only file",
  tooLarge: "too large (over 1 MiB, or 100 MiB with large files on)",
};

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** The success summary after an import. `names` maps ids to artifact names for the skipped list. */
export function Done({
  report,
  names,
}: {
  report: ImportReport;
  names: ReadonlyMap<string, string>;
}) {
  const { imported, alreadyInLibrary, skippedFiles } = report;
  return (
    <div className={styles.result}>
      <div className={styles.okIcon}>
        <CircleCheck size={22} aria-hidden="true" />
      </div>
      <h2>Imported {plural(imported.length, "artifact")} into your library</h2>
      {alreadyInLibrary.length > 0 && (
        <p>
          {plural(alreadyInLibrary.length, "artifact")} already in the library,
          left as they were.
        </p>
      )}
      {skippedFiles.length > 0 && (
        <div className={styles.skipped}>
          <h3 className={styles.h}>
            {plural(skippedFiles.length, "file")} not copied
          </h3>
          <ul className={`${styles.group} ${styles.plain}`}>
            {skippedFiles.map((s) => (
              <li key={`${s.artifactId}:${s.file}`} className={styles.row}>
                <span className={styles.mono}>{s.file}</span>
                <span className={styles.muted}>
                  {names.get(s.artifactId) ?? s.artifactId}: {reasons[s.reason]}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
      <Link
        to={`/library?source=${encodeURIComponent(report.sourceId)}`}
        className={styles.link}
      >
        <LayoutGrid size={14} aria-hidden="true" />
        Open the Library
      </Link>
    </div>
  );
}
