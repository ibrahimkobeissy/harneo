import type { CategoryId } from "../../bindings/CategoryId";
import styles from "./Category.module.css";

/** A node: `r` is a rounded square (x, y, w, h), `c` a circle (cx, cy, r). */
type Node =
  | readonly ["r", number, number, number, number]
  | readonly ["c", number, number, number];

interface Art {
  /** Faint vertical rails (phase boundaries). */
  rails?: string;
  /** The flow, drawn as one thick rounded line. */
  line: string;
  dashed?: boolean;
  nodes: readonly Node[];
}

const sq = (x: number, y = 26, s = 12): Node => ["r", x, y, s, s];

/** The thumbnails of the board `NewProfile`, static. */
const art: Record<CategoryId, Art> = {
  software: {
    line: "M13 24H193M163 30V48H103V30",
    nodes: [6, 36, 66, 96, 126, 156, 186].map(
      (x) => ["r", x, 18, 14, 12] as Node,
    ),
  },
  research: {
    line: "M16 32H30M42 32H78M122 32H146M158 32H166M178 32H186M78 32A22 22 0 0 1 122 32A22 22 0 0 1 78 32",
    nodes: [
      sq(4),
      sq(30),
      ["c", 100, 10, 5],
      ["c", 119, 43, 5],
      ["c", 81, 43, 5],
      sq(146),
      sq(166),
      sq(186),
    ],
  },
  writing: {
    line: "M10 32H190M94 26C94 8 122 8 122 26M150 38C150 58 94 58 94 38",
    nodes: [4, 32, 60, 88, 116, 144, 184].map((x) => sq(x)),
  },
  video: {
    rails: "M70 4V60M120 4V60",
    line: "M18 32H26M38 32H46M58 32H86M98 32H110V16H134M110 32V48H134M146 16H160V32H176M146 48H160V32",
    nodes: [sq(6), sq(26), sq(46), sq(86), sq(134, 10), sq(134, 42), sq(176)],
  },
  social: {
    line: "M34 32A22 22 0 0 1 78 32A22 22 0 0 1 34 32M78 32H100M100 32L124 10M100 32H124M100 32L124 54M136 10L160 32M136 32H160M136 54L160 32M172 32H186V4H56V10",
    nodes: [
      ["c", 56, 10, 5],
      ["c", 34, 32, 5],
      ["c", 56, 54, 5],
      ["c", 78, 32, 5],
      sq(124, 4),
      sq(124, 26),
      sq(124, 48),
      sq(160),
    ],
  },
  data: {
    line: "M10 32H190M62 26C62 10 98 10 98 26M98 38C98 54 62 54 62 38",
    nodes: [4, 28, 56, 92, 128, 156, 184].map((x) => sq(x)),
  },
  design: {
    line: "M16 32L58 6L100 32L58 58ZM100 32L142 6L184 32L142 58Z",
    nodes: [
      sq(4),
      ["c", 58, 6, 4],
      ["c", 100, 32, 5],
      ["c", 142, 6, 4],
      ["c", 142, 58, 4],
      sq(184),
    ],
  },
  custom: {
    line: "M16 32H88M112 32H184M100 18l3 8.5 8.5 3-8.5 3-3 8.5-3-8.5-8.5-3 8.5-3z",
    dashed: true,
    nodes: [sq(4), sq(184)],
  },
};

/** The shape of a category's flow, drawn small. Decoration: the card's text says the same. */
export function ShapeThumb({ id }: { id: CategoryId }) {
  const { rails, line, dashed, nodes } = art[id];
  return (
    <svg
      className={styles.thumb}
      viewBox="0 0 200 64"
      width="100%"
      height="64"
      aria-hidden="true"
    >
      {rails && <path className={styles.rail} d={rails} />}
      <path
        className={`${styles.flow} ${dashed ? styles.dashed : ""}`}
        d={line}
      />
      {nodes.map((n, i) =>
        n[0] === "r" ? (
          <rect
            key={i}
            className={styles.node}
            x={n[1]}
            y={n[2]}
            width={n[3]}
            height={n[4]}
            rx="3"
          />
        ) : (
          <circle
            key={i}
            className={styles.node}
            cx={n[1]}
            cy={n[2]}
            r={n[3]}
          />
        ),
      )}
    </svg>
  );
}
