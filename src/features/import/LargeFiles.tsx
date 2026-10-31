import { FileWarning } from "lucide-react";
import type { Candidate } from "../../bindings/Candidate";
import styles from "./Import.module.css";

/**
 * "Copy large files too": the import's one switch for files over 1 MiB, which are left out
 * unless it is on. Shown on Choose and again on Review, next to Import, so it is not missed.
 */
export function LargeFilesChoice({
  candidates,
  on,
  onChange,
}: {
  /** The artifacts being imported. */
  candidates: readonly Candidate[];
  on: boolean;
  onChange: (on: boolean) => void;
}) {
  const owners = candidates.filter((c) => c.audit.unaudited.length > 0);
  if (owners.length === 0) return null;
  const files = owners.reduce((n, c) => n + c.audit.unaudited.length, 0);
  const names = owners.map((c) => c.artifact.name).join(", ");
  return (
    <div className={styles.callout} data-tone="warn">
      <FileWarning size={16} aria-hidden="true" />
      <div>
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={on}
            onChange={(e) => onChange(e.target.checked)}
          />
          <span>
            <b>Copy large files too</b>{" "}
            {`${files} ${files === 1 ? "file" : "files"} over 1 MiB in ${names}`}
          </span>
        </label>
        <p className={styles.sec}>
          {on
            ? "They will be copied (up to 100 MiB each). They are not security audited and are listed as such on the artifact."
            : "Off: they will not be copied. Tick to copy them (up to 100 MiB each); they are not security audited."}
        </p>
      </div>
    </div>
  );
}
