import type { MapView } from "../../bindings/MapView";
import styles from "./Harnmap.module.css";
import { inMapOrder, zoneName } from "./place";

const LAYER = { stage: "Line", toolbelt: "Toolbelt", floor: "Floor" } as const;

/**
 * The map as a table: every placed artifact in map order, then the unplaced ones. Same data as
 * the map, readable by screen readers. A name selects that artifact on the map.
 */
export function ListView({
  view,
  profile,
  onPick,
}: {
  view: MapView;
  profile: string;
  onPick: (id: string) => void;
}) {
  const nameOf = (id: string) => view.artifacts[id]?.name ?? id;
  const placed = inMapOrder(view.doc).filter((p) => view.artifacts[p.artifact]);
  const parents = new Map(
    placed.flatMap((p) => (p.parent ? [[p.artifact, p.parent] as const] : [])),
  );
  /** 0 for an ordinary row, 1 for a child, 2 for a grandchild. */
  const depthOf = (id: string): number => {
    let depth = 0;
    for (let at = parents.get(id); at && depth < 2; at = parents.get(at)) {
      depth++;
    }
    return depth;
  };
  return (
    <div className={styles.listWrap}>
      <table className={styles.table}>
        <caption>
          {profile}’s map as a list: same data as the map, readable by screen
          readers.
        </caption>
        <thead>
          <tr>
            <th scope="col">Artifact</th>
            <th scope="col">Layer</th>
            <th scope="col">Stage</th>
            <th scope="col">Note</th>
            <th scope="col">Placed by</th>
          </tr>
        </thead>
        <tbody>
          {placed.map((p) => (
            <tr key={p.artifact}>
              <td
                data-depth={depthOf(p.artifact)}
                style={{ paddingLeft: `${depthOf(p.artifact) * 20 + 8}px` }}
              >
                <button
                  type="button"
                  className={styles.nameBtn}
                  onClick={() => onPick(p.artifact)}
                >
                  {nameOf(p.artifact)}
                </button>
                {p.parent && (
                  <span className={styles.muted}>
                    {" "}
                    · runs only through {nameOf(p.parent)}
                  </span>
                )}
              </td>
              <td>{LAYER[p.zone.type]}</td>
              <td>
                {p.zone.type === "stage" ? zoneName(view.doc, p.zone) : "—"}
              </td>
              <td>{p.note || "—"}</td>
              <td>
                {p.pinned ? "You (pinned)" : p.by === "user" ? "You" : "Claude"}
              </td>
            </tr>
          ))}
          {view.unplaced.map((id) => (
            <tr key={id}>
              <td>
                <button
                  type="button"
                  className={styles.nameBtn}
                  onClick={() => onPick(id)}
                >
                  {nameOf(id)}
                </button>
              </td>
              <td>Unplaced</td>
              <td>—</td>
              <td>—</td>
              <td>—</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
