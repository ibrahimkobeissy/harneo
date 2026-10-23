import type { Plan } from "../../bindings/Plan";
import type { Write } from "../../bindings/Write";
import { Badge, type BadgeTone } from "../../ui";
import { plural } from "./format";
import styles from "./Projects.module.css";

export type Outcome = "add" | "update" | "deleted" | "restored";
export type GroupKey = Outcome | "edited";

const outcomes: Record<Outcome, { label: string; tone: BadgeTone }> = {
  add: { label: "Add", tone: "ok" },
  update: { label: "Update", tone: "warn" },
  deleted: { label: "Delete", tone: "bad" },
  restored: { label: "Restore", tone: "ok" },
};

const titles: Record<GroupKey, string> = {
  add: "Add",
  update: "Update",
  deleted: "Deleted: Harneo created these",
  restored: "Restored: your original comes back",
  edited: "Edited in project, backed up first",
};

/**
 * What happens to a path: a path the plan lists as restored is only ever "restored", whatever
 * write puts the original back; otherwise the write's action decides.
 */
function outcomeOf(w: Write, restored: ReadonlySet<string>): Outcome | null {
  if (restored.has(w.path)) return "restored";
  switch (w.action) {
    case "new":
      return "add";
    case "replace":
      return "update";
    case "delete":
      return "deleted";
    default:
      return null;
  }
}

/** The plan's changing paths in groups; a path edited in the project goes to "edited" only. */
export function groupPlan(plan: Plan): Record<GroupKey, Write[]> {
  const restored = new Set(plan.restored);
  const edited = new Set(plan.editedInProject);
  const out: Record<GroupKey, Write[]> = {
    add: [],
    update: [],
    deleted: [],
    restored: [],
    edited: [],
  };
  for (const w of plan.writes) {
    const outcome = outcomeOf(w, restored);
    if (outcome === null) continue;
    out[edited.has(w.path) ? "edited" : outcome].push(w);
  }
  return out;
}

/** The files of a plan under the headings the user reads, then where the backup goes. */
export function PlanFiles({
  plan,
  show,
  backupDir,
}: {
  plan: Plan;
  show: GroupKey[];
  /** The folder backups are written to, when known. */
  backupDir: string | null;
}) {
  const grouped = groupPlan(plan);
  const restored = new Set(plan.restored);
  return (
    <section className={styles.diff} aria-label="Files">
      {show.map((key) => {
        const writes = grouped[key];
        if (writes.length === 0) return null;
        return (
          <div key={key} className={styles.fileGroup}>
            <h3 className={styles.groupHead}>
              {titles[key]}{" "}
              <span className={styles.muted}>{writes.length}</span>
            </h3>
            <ul className={styles.diffGroup} aria-label={titles[key]}>
              {writes.map((w) => {
                const outcome = outcomeOf(w, restored);
                const look = outcomes[outcome ?? "update"];
                return (
                  <li key={w.path} className={styles.diffRow}>
                    <span className={styles.diffTag}>
                      <Badge tone={look.tone}>{look.label}</Badge>
                    </span>
                    <span className={styles.diffPath}>{w.path}</span>
                  </li>
                );
              })}
            </ul>
          </div>
        );
      })}
      {plan.backupCount > 0 && (
        <p className={styles.backupLine}>
          {plural(plan.backupCount, "file")} backed up before anything changes
          {backupDir ? (
            <>
              , to <span className={styles.mono}>{backupDir}</span>
            </>
          ) : null}
          .
        </p>
      )}
    </section>
  );
}
