import { Link } from "react-router";
import type { Template } from "../../bindings/Template";
import { Button, Kbd } from "../../ui";
import { clock, modelLabel, type ChangeLine } from "./draw";
import styles from "./Harnmap.module.css";
import { SentLine } from "./SentLine";

/** The side panel before the first draw: description, chips, facts, what is sent, Draw. */
export function FirstDrawPanel({
  description,
  onDescription,
  template,
  count,
  model,
  canDraw,
  onDraw,
  addLink,
}: {
  description: string;
  onDescription: (text: string) => void;
  template: Template | undefined;
  count: number;
  model: string;
  canDraw: boolean;
  onDraw: () => void;
  addLink: string;
}) {
  return (
    <>
      <div className={styles.sect}>
        <div className={styles.eyebrow}>HARNMAP · NOT DRAWN YET</div>
        <h2 className={styles.panelTitle}>Draw the factory</h2>
        <p className={styles.p}>
          Claude sends this profile’s {count} artifacts down the line and puts
          each one where it works: on a stage, on the toolbelt, or on the floor.
        </p>
      </div>
      <div className={styles.sect}>
        <div className={styles.labelRow}>
          <label className={styles.label} htmlFor="harnmap-description">
            How do you see this factory?
          </label>
          <span className={styles.muted}>Optional</span>
        </div>
        <textarea
          id="harnmap-description"
          className={styles.textarea}
          placeholder="Leave it empty and Claude works it out from your artifacts."
          value={description}
          onChange={(e) => onDescription(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && canDraw)
              onDraw();
          }}
        />
        {template && (
          <div className={styles.chips} role="group" aria-label="Suggestions">
            {template.suggestions.map((chip) => (
              <button
                key={chip}
                type="button"
                className={styles.chip}
                onClick={() =>
                  onDescription(description ? `${description} ${chip}` : chip)
                }
              >
                + {chip}
              </button>
            ))}
          </div>
        )}
      </div>
      <div className={styles.sect}>
        <dl className={styles.facts} aria-label="What a draw involves">
          <div>
            <dt>Sends</dt>
            <dd>
              {count} artifact{count === 1 ? "" : "s"}
            </dd>
          </div>
          <div>
            <dt>Through</dt>
            <dd>your claude CLI</dd>
          </div>
          <div>
            <dt>Uses</dt>
            <dd>your plan · {modelLabel(model)}</dd>
          </div>
          <div>
            <dt>Takes</dt>
            <dd>usually 1–3 min</dd>
          </div>
        </dl>
        <Button
          variant="primary"
          className={styles.go}
          disabled={!canDraw}
          onClick={onDraw}
        >
          Draw <Kbd>Ctrl ↵</Kbd>
        </Button>
        {!canDraw && (
          <p className={styles.muted}>
            Nothing to draw yet.{" "}
            <Link to={addLink}>Add artifacts from the library</Link> first.
          </p>
        )}
        <SentLine count={count} />
        <p className={styles.center}>
          Saved with the profile as draw 1. Nothing to save by hand.
        </p>
      </div>
    </>
  );
}

/** The honest wait: the clock, the model, Cancel. */
export function WaitingPanel({
  elapsedMs,
  model,
  onCancel,
}: {
  elapsedMs: number;
  model: string;
  onCancel: () => void;
}) {
  return (
    <div className={styles.sect}>
      <div className={styles.waitRow}>
        <div>
          <div className={styles.eyebrow}>
            CLAUDE IS WORKING · {modelLabel(model).toUpperCase()}
          </div>
          <div className={styles.clock} role="timer" aria-label="Time elapsed">
            {clock(elapsedMs)}
          </div>
        </div>
        <Button small variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </div>
      <p className={styles.p}>
        Claude is reading the artifacts and choosing where each one goes. This
        usually takes 1–3 minutes, and up to 5. Nothing is saved until the
        answer passes Harneo’s checks.
      </p>
    </div>
  );
}

/** What the changes list renders: mark, words, artifact names. */
export function Changes({ lines }: { lines: ChangeLine[] }) {
  return (
    <ul className={styles.changes} aria-label="Changes from the template">
      {lines.map((line) => (
        <li key={`${line.mark}${line.text}`}>
          <span className={styles.mark} aria-hidden="true">
            {line.mark}
          </span>
          <span>
            {line.text}{" "}
            {line.names?.map((name, i) => (
              <span key={name}>
                {i > 0 && ", "}
                <code>{name}</code>
              </span>
            ))}
          </span>
        </li>
      ))}
    </ul>
  );
}

/** The result of a draw: saved, its changes from the template, Open the map. */
export function ResultPanel({
  no,
  model,
  tookText,
  placed,
  total,
  lines,
  onOpen,
}: {
  no: number;
  model: string;
  tookText: string;
  placed: number;
  total: number;
  lines: ChangeLine[];
  onOpen: () => void;
}) {
  return (
    <>
      <div className={styles.sect}>
        <div className={styles.eyebrow}>DRAW {no} · SAVED</div>
        <p className={styles.p}>
          {modelLabel(model)} · {tookText} · {placed} of {total} placed
        </p>
      </div>
      <div className={styles.sect}>
        <div className={styles.eyebrow}>
          CHANGES FROM THE TEMPLATE · COMPUTED BY HARNEO
        </div>
        <Changes lines={lines} />
      </div>
      <div className={styles.sect}>
        <Button variant="primary" className={styles.go} onClick={onOpen}>
          Open the map
        </Button>
      </div>
    </>
  );
}
