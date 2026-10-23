import type { Hunk } from "../bindings/Hunk";
import styles from "./CodeView.module.css";

/** UTF-8 text of base64 `bytes`, or `null` when they are not valid UTF-8 (or not base64). */
export function decodeText(bytes: string): string | null {
  try {
    const raw = atob(bytes);
    const buf = Uint8Array.from(raw, (c) => c.charCodeAt(0));
    return new TextDecoder("utf-8", { fatal: true }).decode(buf);
  } catch {
    return null;
  }
}

function linesOf(text: string): string[] {
  const lines = text.split(/\r?\n/);
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
  return lines;
}

const marker = { context: " ", added: "+", removed: "−" } as const;

/**
 * One file's content: UTF-8 text with line numbers, or a +/− diff when `hunks` is given. Bytes that
 * are not valid UTF-8 show a notice instead of garbage.
 */
export function CodeView({
  path,
  bytes,
  hunks,
  hideHead = false,
}: {
  path: string;
  /** Base64 content, as a plan carries it. */
  bytes: string | null;
  hunks?: readonly Hunk[] | null;
  /** Leave out the path bar when the caller already shows the path. */
  hideHead?: boolean;
}) {
  const diff = hunks && hunks.length > 0 ? hunks : null;
  const text = !diff && bytes !== null ? decodeText(bytes) : null;
  const lines = text === null ? [] : linesOf(text);

  let body;
  let meta: string | null = null;
  if (diff) {
    const added = diff
      .flatMap((h) => h.lines)
      .filter((l) => l.kind === "added");
    const removed = diff
      .flatMap((h) => h.lines)
      .filter((l) => l.kind === "removed");
    meta = `+${added.length} −${removed.length}`;
    body = (
      <pre className={styles.pre}>
        {diff.map((hunk, h) => {
          let oldNo = hunk.oldStart;
          let newNo = hunk.newStart;
          return (
            <div key={h}>
              {h > 0 && <div className={styles.sep}>⋯</div>}
              {hunk.lines.map((line, i) => {
                const no = line.kind === "removed" ? oldNo : newNo;
                if (line.kind !== "added") oldNo += 1;
                if (line.kind !== "removed") newNo += 1;
                return (
                  <div key={i} className={styles.ln} data-kind={line.kind}>
                    <span className={styles.no}>{no}</span>
                    <span className={styles.mk} aria-hidden="true">
                      {marker[line.kind]}
                    </span>
                    <span>{line.text}</span>
                  </div>
                );
              })}
            </div>
          );
        })}
      </pre>
    );
  } else if (bytes === null) {
    body = <p className={styles.notice}>This file has no content to show.</p>;
  } else if (text === null) {
    body = (
      <p className={styles.notice} role="note">
        Binary file, not shown. It is not valid UTF-8 text.
      </p>
    );
  } else {
    meta = `${lines.length} ${lines.length === 1 ? "line" : "lines"}`;
    body = (
      <pre className={styles.pre}>
        {lines.map((line, i) => (
          <div key={i} className={styles.ln}>
            <span className={styles.no}>{i + 1}</span>
            <span>{line}</span>
          </div>
        ))}
      </pre>
    );
  }

  return (
    <div
      className={styles.code}
      role="region"
      aria-label={`Content of ${path}`}
    >
      {!hideHead && (
        <div className={styles.head}>
          <span>{path}</span>
          {meta && <span className={styles.meta}>{meta}</span>}
        </div>
      )}
      {body}
    </div>
  );
}
