import { useEffect, useRef } from "react";
import type { Fact } from "../../bindings/Fact";
import type { Placement } from "../../bindings/Placement";
import styles from "./MapCanvas.module.css";
import { sourceOf, type Source } from "./tooltip";

function Tag({ source, label }: { source: Source; label?: string }) {
  return (
    <span className={styles.src} data-source={source}>
      {label ?? (source === "files" ? "files" : "Claude")}
    </span>
  );
}

/** The heartbeat tooltip: Role, When and Hands off, each tagged files or Claude. */
export function Tooltip({
  name,
  placement,
  facts,
  parentName,
  alsoRunBy = [],
  x,
  y,
  focus,
}: {
  name: string;
  placement: Placement;
  facts: Fact[];
  /** Display name of the artifact this one runs only through, when it has a parent. */
  parentName?: string;
  /** Display names of the other artifacts that also run it. */
  alsoRunBy?: string[];
  x: number;
  y: number;
  /** Move focus into the tooltip (it was opened from the keyboard). */
  focus: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focus) ref.current?.focus();
  }, [focus]);
  const rows: Array<[string, "role" | "when" | "handsOff", string]> = [
    ["Role", "role", placement.role],
    ["When", "when", placement.when],
    ["Hands off", "handsOff", placement.handsOff],
  ];
  const modelOnly = facts.find((f) => f.kind.type === "modelOnly");
  return (
    <div
      ref={ref}
      role="dialog"
      aria-label={`${name}: how it takes part in the flow`}
      tabIndex={-1}
      className={styles.tip}
      style={{ left: x, top: y }}
      onClick={(ev) => ev.stopPropagation()}
      onPointerDown={(ev) => ev.stopPropagation()}
    >
      <div className={styles.tn}>
        <i />
        {name}
      </div>
      <dl>
        {rows.map(([label, field, text]) => (
          <div key={field} className={styles.tr}>
            <dt>{label}</dt>
            <dd>
              {text || "Not stated"} <Tag source={sourceOf(field, facts)} />
            </dd>
          </div>
        ))}
        {parentName !== undefined && (
          <div className={styles.tr}>
            <dt>Parent</dt>
            <dd>
              Runs only through {parentName}{" "}
              <Tag source="claude" label="Claude’s reading" />
            </dd>
          </div>
        )}
        {parentName !== undefined && alsoRunBy.length > 0 && (
          <div className={styles.tr}>
            <dt>Also run by</dt>
            <dd>
              {alsoRunBy.join(", ")}{" "}
              <Tag source="claude" label="Claude’s reading" />
            </dd>
          </div>
        )}
        {modelOnly && (
          <div className={styles.tr}>
            <dt>Model only</dt>
            <dd>
              {modelOnly.text} <Tag source="files" />
            </dd>
          </div>
        )}
      </dl>
    </div>
  );
}
