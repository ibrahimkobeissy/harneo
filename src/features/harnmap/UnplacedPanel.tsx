import { ChevronDown, GripVertical, Search } from "lucide-react";
import { forwardRef, useMemo, useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { MapDoc } from "../../bindings/MapDoc";
import { Button, kinds } from "../../ui";
import styles from "./Harnmap.module.css";
import type { ArtifactInfo } from "./types";

/** "On the map": how many stations sit where, and what stays put. */
export function OnTheMap({
  doc,
  bundles,
  nameOf,
}: {
  doc: MapDoc;
  bundles: number;
  nameOf: (id: string) => string;
}) {
  const on = (type: "stage" | "toolbelt" | "floor") =>
    doc.placements.filter((p) => p.zone.type === type);
  const line = on("stage");
  const belt = on("toolbelt");
  const floor = on("floor");
  const pinned = doc.placements.filter((p) => p.pinned).length;
  return (
    <div className={styles.sect}>
      <h2 className={styles.h4}>On the map</h2>
      <dl className={styles.kv} aria-label="On the map">
        <dt>Line</dt>
        <dd>
          {line.length} {line.length === 1 ? "station" : "stations"} in{" "}
          {doc.stages.length} {doc.stages.length === 1 ? "stage" : "stages"},{" "}
          {bundles} {bundles === 1 ? "bundle" : "bundles"}
        </dd>
        <dt>Toolbelt</dt>
        <dd>
          {belt.length}
          {belt.length > 0 &&
            ` · ${belt.map((p) => nameOf(p.artifact)).join(", ")}`}
        </dd>
        <dt>Floor</dt>
        <dd>{floor.length} · always on</dd>
        <dt>Pinned</dt>
        <dd>{pinned} · stay put on re-draw</dd>
      </dl>
      <p className={styles.muted}>
        Click any node to see why Claude put it there. Focus a node and press{" "}
        <code>i</code> for the quick summary. The map saves as you edit and
        doesn’t change the profile’s version.
      </p>
    </div>
  );
}

/**
 * The artifacts with no place on the map, by type with a filter. Drag one onto a stage, the
 * toolbelt or the floor, or click it to choose where. Below, what is hidden and a summary.
 */
export const UnplacedPanel = forwardRef<
  HTMLInputElement,
  {
    unplaced: string[];
    artifacts: Record<string, ArtifactInfo>;
    hidden: string[];
    onPick: (id: string) => void;
    onDragStart: (id: string) => void;
    onDragEnd: () => void;
    onShow: (id: string) => void;
    onShowAll: () => void;
    disabled: boolean;
  }
>(function UnplacedPanel(
  {
    unplaced,
    artifacts,
    hidden,
    onPick,
    onDragStart,
    onDragEnd,
    onShow,
    onShowAll,
    disabled,
  },
  filterRef,
) {
  const [query, setQuery] = useState("");
  const [closed, setClosed] = useState<ArtifactKind[]>([]);
  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const by = new Map<ArtifactKind, string[]>();
    for (const id of unplaced) {
      const info = artifacts[id];
      if (!info || (q && !info.name.toLowerCase().includes(q))) continue;
      by.set(info.kind, [...(by.get(info.kind) ?? []), id]);
    }
    return [...by.entries()];
  }, [unplaced, artifacts, query]);
  const nameOf = (id: string) => artifacts[id]?.name ?? id;

  return (
    <>
      <div className={styles.sect}>
        <div className={styles.panelHead}>
          <h2 className={styles.h4}>Unplaced</h2>
          <span className={styles.count}>{unplaced.length}</span>
        </div>
        <p className={styles.p}>
          Not everything is a step. These are available at any time. Drag one
          onto a stage, the toolbelt or the floor to place it.
        </p>
        <div>
          <label className={styles.label} htmlFor="unplaced-filter">
            <Search size={12} aria-hidden="true" /> Filter unplaced artifacts
          </label>
          <input
            id="unplaced-filter"
            ref={filterRef}
            className={styles.search}
            style={{ width: "100%" }}
            placeholder="Filter"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>
      <div className={styles.sect}>
        <div
          className={styles.tree}
          role="tree"
          aria-label="Unplaced artifacts"
        >
          {groups.map(([kind, ids]) => {
            const open = !closed.includes(kind);
            return (
              <div key={kind} role="group">
                <button
                  type="button"
                  className={styles.group}
                  aria-expanded={open}
                  onClick={() =>
                    setClosed(
                      open
                        ? [...closed, kind]
                        : closed.filter((k) => k !== kind),
                    )
                  }
                >
                  <ChevronDown size={12} aria-hidden="true" />
                  {kinds[kind].plural}
                  <span className={styles.count}>{ids.length}</span>
                </button>
                {open &&
                  ids.map((id) => {
                    const Icon = kinds[kind].icon;
                    return (
                      <button
                        key={id}
                        type="button"
                        role="treeitem"
                        className={styles.item}
                        draggable={!disabled}
                        disabled={disabled}
                        aria-label={`Place ${nameOf(id)}`}
                        onClick={() => onPick(id)}
                        onDragStart={(ev) => {
                          ev.dataTransfer?.setData("text/plain", id);
                          onDragStart(id);
                        }}
                        onDragEnd={onDragEnd}
                      >
                        <GripVertical size={12} aria-hidden="true" />
                        <Icon size={13} aria-hidden="true" />
                        <span className={styles.name}>{nameOf(id)}</span>
                      </button>
                    );
                  })}
              </div>
            );
          })}
          {unplaced.length === 0 && (
            <p className={styles.muted}>Everything is on the map.</p>
          )}
          {unplaced.length > 0 && groups.length === 0 && (
            <p className={styles.muted}>Nothing matches “{query}”.</p>
          )}
        </div>
      </div>
      {hidden.length > 0 && (
        <div className={styles.sect}>
          <div className={styles.panelHead}>
            <h2 className={styles.h4}>Hidden on the map</h2>
            <Button small variant="ghost" onClick={onShowAll}>
              Show all
            </Button>
          </div>
          {hidden.map((id) => (
            <div key={id} className={styles.listRow}>
              <span className={styles.mono}>{nameOf(id)}</span>
              <Button
                small
                variant="ghost"
                aria-label={`Show ${nameOf(id)}`}
                onClick={() => onShow(id)}
              >
                Show
              </Button>
            </div>
          ))}
        </div>
      )}
    </>
  );
});
