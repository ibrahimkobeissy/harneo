import {
  ArrowDown,
  ArrowUp,
  Archive,
  CircleCheck,
  TriangleAlert,
} from "lucide-react";
import { useState } from "react";
import { Link } from "react-router";
import type { Comparison } from "../../bindings/Comparison";
import type { RefitAction } from "../../bindings/RefitAction";
import type { Row } from "../../bindings/Row";
import { Badge, Button, Callout, EmptyState } from "../../ui";
import styles from "./Refit.module.css";
import {
  actionOf,
  baseName,
  counts,
  criticalLosses,
  isCritical,
  lossOf,
  lossText,
} from "./summary";

type SortBy = "item" | "inProject" | "inProfile" | "action";
const heads: { id: SortBy; label: string }[] = [
  { id: "item", label: "File" },
  { id: "inProject", label: "In project" },
  { id: "inProfile", label: "In profile" },
  { id: "action", label: "Action" },
];

const actionLabel: Record<RefitAction, string> = {
  keep: "Keep",
  replace: "Replace",
  delete: "Delete",
  add: "Add",
  skip: "Skip",
};

function ProjectBadge({ value }: { value: Row["inProject"] }) {
  if (value === "absent") return <span className={styles.muted}>—</span>;
  if (value === "same") return <Badge tone="ok">identical</Badge>;
  return <Badge>{value === "differs" ? "modified" : "exists"}</Badge>;
}

function ProfileBadge({ value }: { value: Row["inProfile"] }) {
  if (value === "absent") return <span className={styles.muted}>—</span>;
  if (value === "same") return <Badge tone="ok">identical</Badge>;
  if (value === "new") return <Badge tone="info">new</Badge>;
  return <Badge tone="warn">different</Badge>;
}

/** Step 3: every item side by side, one action each. Lost content shows in red before anything is written. */
export function CompareStep({
  comparison,
  choices,
  onChoose,
}: {
  comparison: Comparison;
  choices: Record<string, RefitAction>;
  onChoose: (item: string, action: RefitAction) => void;
}) {
  // The default order pins rows that would lose the user's own setup to the top.
  const [sort, setSort] = useState<{
    by: SortBy;
    desc: boolean;
    pinned: boolean;
  }>({ by: "item", desc: false, pinned: true });
  const [open, setOpen] = useState<Set<string>>(new Set());

  if (comparison.nothingToChange) {
    return (
      <div className={styles.result}>
        <EmptyState icon={CircleCheck} title="Nothing to change">
          <span className={styles.mono}>{comparison.target}</span> already
          matches {comparison.profileName} v{comparison.profileVersion}, file
          for file.
        </EmptyState>
        <Link className={styles.linkBtn} to="/projects">
          Back to projects
        </Link>
      </div>
    );
  }

  const value = (row: Row, by: SortBy): string =>
    by === "item"
      ? row.item
      : by === "inProject"
        ? row.inProject
        : by === "inProfile"
          ? row.inProfile
          : actionOf(row, choices);
  const pin = (r: Row) =>
    isCritical(r.item) && r.loss ? (r.item === "CLAUDE.md" ? 0 : 1) : 2;
  const rows = [...comparison.rows].sort((a, b) => {
    if (sort.pinned && pin(a) !== pin(b)) return pin(a) - pin(b);
    const order = value(a, sort.by).localeCompare(value(b, sort.by));
    return (sort.desc ? -order : order) || a.item.localeCompare(b.item);
  });
  const flagged = criticalLosses(comparison.rows, choices);
  const total = counts(comparison.rows, choices);
  const backed = total.replace + total.delete;

  return (
    <>
      {flagged.length > 0 && (
        <Callout tone="bad" icon={TriangleAlert}>
          <p>
            <b>
              {flagged
                .map((f) => `${baseName(f.row.item)}: ${lossText(f.loss)}`)
                .join(" · ")}
            </b>
          </p>
          <p>
            The project&apos;s version has content the profile doesn&apos;t.
            Harneo replaces, never merges. A backup is kept, but choose{" "}
            <b>Keep</b> to leave a file untouched.
          </p>
          <div className={styles.keepRow}>
            {flagged.map((f) => (
              <Button
                key={f.row.item}
                small
                onClick={() => onChoose(f.row.item, "keep")}
              >
                Keep my {baseName(f.row.item)}
              </Button>
            ))}
          </div>
        </Callout>
      )}
      <div className={styles.tableWrap}>
        <table className={styles.table}>
          <thead>
            <tr>
              {heads.map(({ id, label }) => {
                const on = sort.by === id;
                return (
                  <th
                    key={id}
                    scope="col"
                    aria-sort={
                      on ? (sort.desc ? "descending" : "ascending") : "none"
                    }
                  >
                    <button
                      type="button"
                      className={styles.sortBtn}
                      onClick={() =>
                        setSort({
                          by: id,
                          desc: on ? !sort.desc : false,
                          pinned: false,
                        })
                      }
                    >
                      {id === "inProfile"
                        ? `In ${comparison.profileName}`
                        : label}
                      {on &&
                        (sort.desc ? (
                          <ArrowDown size={12} aria-hidden="true" />
                        ) : (
                          <ArrowUp size={12} aria-hidden="true" />
                        ))}
                    </button>
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const loss = lossOf(row, choices);
              const red = loss !== null && isCritical(row.item);
              const shown = open.has(row.item);
              const action = actionOf(row, choices);
              return (
                <tr key={row.item} className={red ? styles.flag : undefined}>
                  <td>
                    <span className={styles.mono}>{row.item}</span>
                    {loss && (
                      <div className={red ? styles.lossRed : styles.lossNote}>
                        {red && <TriangleAlert size={13} aria-hidden="true" />}
                        <b>{lossText(loss)}</b>
                        <Button
                          small
                          variant="ghost"
                          aria-expanded={shown}
                          aria-label={`${shown ? "Hide" : "Show"} lost lines of ${row.item}`}
                          onClick={() =>
                            setOpen((prev) => {
                              const next = new Set(prev);
                              if (!next.delete(row.item)) next.add(row.item);
                              return next;
                            })
                          }
                        >
                          {shown ? "Hide" : "Show"}
                        </Button>
                      </div>
                    )}
                    {loss && shown && (
                      <div className={styles.lost}>
                        {loss.lines.map((line, i) => (
                          <div key={i}>− {line}</div>
                        ))}
                        {loss.count > loss.lines.length && (
                          <div className={styles.muted}>
                            … {loss.count - loss.lines.length} more
                          </div>
                        )}
                      </div>
                    )}
                  </td>
                  <td>
                    <ProjectBadge value={row.inProject} />
                  </td>
                  <td>
                    <ProfileBadge value={row.inProfile} />
                  </td>
                  <td>
                    <div
                      className={styles.seg}
                      role="group"
                      aria-label={`Action for ${row.item}`}
                    >
                      {row.options.map((option) => (
                        <button
                          key={option}
                          type="button"
                          aria-pressed={action === option}
                          data-danger={
                            action === option &&
                            (option === "delete" ||
                              (option === "replace" && loss !== null))
                              ? "true"
                              : undefined
                          }
                          onClick={() => onChoose(row.item, option)}
                        >
                          {actionLabel[option]}
                        </button>
                      ))}
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <Callout icon={Archive}>
        <p>
          Replace {total.replace} · add {total.add} · delete {total.delete} ·
          keep {total.keep} · skip {total.skip}.{" "}
          <b>
            {backed} {backed === 1 ? "file" : "files"}
          </b>{" "}
          backed up first. <span className={styles.mono}>CLAUDE.local.md</span>{" "}
          and <span className={styles.mono}>settings.local.json</span> are never
          touched.
        </p>
      </Callout>
    </>
  );
}
