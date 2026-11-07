import { CircleAlert, Info, TriangleAlert } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Link } from "react-router";
import type { CategoryChange } from "../../bindings/CategoryChange";
import type { DrawFailure } from "../../bindings/DrawFailure";
import type { Staleness } from "../../bindings/Staleness";
import { Button, Callout } from "../../ui";
import { modelLabel, took, type DrawModel } from "./draw";
import styles from "./Harnmap.module.css";

function Card({
  title,
  label,
  children,
}: {
  title: ReactNode;
  label: string;
  children: ReactNode;
}) {
  return (
    <section className={styles.card} aria-label={label}>
      <h2 className={styles.cardTitle}>{title}</h2>
      {children}
    </section>
  );
}

export interface FailureActions {
  /** Draw again with the same description and model. */
  onRetry: () => void;
  /** Store the other model, then draw again. */
  onSwitchModel: (model: DrawModel) => void;
  /** Ask the backend whether the `claude` CLI is there now. */
  onCheck: () => void;
  /** Open the official setup guide. */
  onGuide: () => void;
  onDismiss: () => void;
}

/** One state card per way a draw can fail, each with its actions. Nothing is saved on failure. */
export function FailureCard({
  failure,
  model,
  drawn,
  elapsedMs,
  actions,
  addLink,
}: {
  failure: Exclude<DrawFailure, { type: "cancelled" }>;
  model: DrawModel;
  drawn: boolean;
  elapsedMs: number;
  actions: FailureActions;
  addLink: string;
}) {
  const [showAnswer, setShowAnswer] = useState(false);
  const other: DrawModel = model === "opus" ? "sonnet" : "opus";
  const unchanged = drawn
    ? "Your map is unchanged."
    : "Nothing was saved. Your description is kept.";
  switch (failure.type) {
    case "claudeMissing":
      return (
        <Card
          label="claude CLI not found"
          title={
            <>
              <code>claude</code> CLI not found
            </>
          }
        >
          <Callout tone="warn" icon={TriangleAlert}>
            Harneo draws maps through your own Claude Code CLI, and it isn’t on
            your PATH. The map and its template still work by hand.
          </Callout>
          <p className={styles.p}>
            Install Claude Code with the official setup guide, sign in once in a
            terminal, then check again. Harneo never asks for a key.
          </p>
          <div className={styles.row}>
            <Button onClick={actions.onCheck}>Check again</Button>
            <Button variant="ghost" onClick={actions.onGuide}>
              Claude Code setup guide
            </Button>
          </div>
        </Card>
      );
    case "claudeFailed":
      return claudeFailed(failure.kind, failure.message);
    case "badAnswer":
      return (
        <Card label="Answer did not check out" title="Answer didn’t check out">
          <Callout tone="bad" icon={CircleAlert}>
            <b>Claude’s answer failed Harneo’s checks twice.</b>
          </Callout>
          <p className={styles.p}>
            Harneo retried once on its own. {unchanged} Nothing half-drawn was
            saved.
          </p>
          {showAnswer && (
            <ul className={styles.reasons} aria-label="Why the answer failed">
              {failure.reasons.map((reason) => (
                <li key={reason}>{reason}</li>
              ))}
            </ul>
          )}
          <div className={styles.row}>
            <Button onClick={actions.onRetry}>Try again</Button>
            <Button
              variant="ghost"
              aria-expanded={showAnswer}
              onClick={() => setShowAnswer(!showAnswer)}
            >
              {showAnswer ? "Hide the answer" : "Show the answer"}
            </Button>
          </div>
        </Card>
      );
    case "emptyProfile":
      return (
        <Card label="Empty profile" title="Nothing to place yet">
          <p className={styles.p}>
            This profile has no artifacts, so there is nothing to draw. Add
            artifacts from the library and Claude can draw its factory.
          </p>
          <div className={styles.row}>
            <Link to={addLink} className={styles.linkBtn}>
              Add from library
            </Link>
            <Button variant="ghost" onClick={actions.onDismiss}>
              Dismiss
            </Button>
          </div>
        </Card>
      );
    case "other":
      return (
        <Card label="Drawing failed" title="Drawing failed">
          <Callout tone="bad" icon={CircleAlert}>
            <b>Claude couldn’t draw the map.</b> {failure.message}
          </Callout>
          <p className={styles.p}>{unchanged}</p>
          <div className={styles.row}>
            <Button onClick={actions.onRetry}>Try again</Button>
          </div>
        </Card>
      );
  }

  function claudeFailed(kind: string, message: string) {
    switch (kind) {
      case "not_logged_in":
        return (
          <Card
            label="claude not signed in"
            title={
              <>
                <code>claude</code> is not signed in
              </>
            }
          >
            <Callout tone="warn" icon={TriangleAlert}>
              The CLI is installed but not signed in, so Claude can’t draw. Your
              map and templates still work by hand.
            </Callout>
            <p className={styles.p}>
              Run <code>claude</code> once in a terminal and sign in with your
              plan, then check again.
            </p>
            <div className={styles.row}>
              <Button onClick={actions.onCheck}>Check again</Button>
            </div>
          </Card>
        );
      case "limit":
        return (
          <Card label="Usage limit" title="Usage limit reached">
            <Callout tone="bad" icon={CircleAlert}>
              <b>
                Claude stopped after {took(elapsedMs)}: usage limit reached.
              </b>{" "}
              {unchanged}
            </Callout>
            <p className={styles.p}>
              Your description is kept, so retrying costs one click.
            </p>
            <div className={styles.row}>
              <Button onClick={actions.onRetry}>Try again</Button>
            </div>
          </Card>
        );
      case "model_unavailable":
        return (
          <Card label="Model not available" title="Model not available">
            <Callout tone="bad" icon={CircleAlert}>
              <b>{modelLabel(model)} isn’t available on your plan right now.</b>{" "}
              Nothing was sent.
            </Callout>
            <p className={styles.p}>
              {modelLabel(other)} can draw the map too. The history entry says
              which model drew it.
            </p>
            <div className={styles.row}>
              <Button onClick={() => actions.onSwitchModel(other)}>
                Draw with {modelLabel(other)}
              </Button>
              <Button variant="ghost" onClick={actions.onDismiss}>
                Cancel
              </Button>
            </div>
          </Card>
        );
      case "timeout":
        return (
          <Card label="Timed out" title="Timed out">
            <Callout tone="bad" icon={CircleAlert}>
              <b>No answer after 5 min.</b> Claude was stopped. {unchanged}
            </Callout>
            <p className={styles.p}>
              Big profiles take longer. Try again
              {model === "opus"
                ? ", or draw with Sonnet, which is quicker"
                : ""}
              .
            </p>
            <div className={styles.row}>
              <Button onClick={actions.onRetry}>Try again</Button>
              {model === "opus" && (
                <Button
                  variant="ghost"
                  onClick={() => actions.onSwitchModel("sonnet")}
                >
                  Use Sonnet
                </Button>
              )}
            </div>
          </Card>
        );
      default:
        return (
          <Card label="Drawing failed" title="Drawing failed">
            <Callout tone="bad" icon={CircleAlert}>
              <b>Claude couldn’t draw the map.</b> {message}
            </Callout>
            <p className={styles.p}>{unchanged}</p>
            <div className={styles.row}>
              <Button onClick={actions.onRetry}>Try again</Button>
            </div>
          </Card>
        );
    }
  }
}

/** Marks for the +/−/~ lines. */
function Row({
  mark,
  name,
  note,
}: {
  mark: string;
  name: string;
  note: string;
}) {
  return (
    <li className={styles.diffRow}>
      <span className={styles.mark} aria-hidden="true">
        {mark}
      </span>
      <code>{name}</code>
      <span className={styles.muted}>{note}</span>
    </li>
  );
}

/** The map is out of date: artifacts added, removed or changed since the draw. */
export function StaleCard({
  stale,
  profile,
  no,
  nameOf,
  onRedraw,
}: {
  stale: Staleness;
  profile: string;
  no: number;
  nameOf: (id: string) => string;
  onRedraw: () => void;
}) {
  return (
    <Card label="Map out of date" title="Out of date">
      <Callout tone="warn" icon={TriangleAlert}>
        {profile} changed since draw {no}.
      </Callout>
      <ul className={styles.diff} aria-label="What changed">
        {stale.added.map((id) => (
          <Row key={`+${id}`} mark="+" name={nameOf(id)} note="in Unplaced" />
        ))}
        {stale.removed.map((id) => (
          <Row
            key={`-${id}`}
            mark="−"
            name={nameOf(id)}
            note="removed from the map"
          />
        ))}
        {stale.changed.map((id) => (
          <Row
            key={`~${id}`}
            mark="~"
            name={nameOf(id)}
            note="differs from the drawn copy"
          />
        ))}
      </ul>
      <p className={styles.muted}>
        The map still works. New artifacts wait in Unplaced; re-draw to place
        them.
      </p>
      <div className={styles.row}>
        <Button onClick={onRedraw}>Re-draw</Button>
      </div>
    </Card>
  );
}

/** The category changed since the draw: offer a re-draw on the new template. */
export function CategoryChangedCard({
  change,
  label,
  no,
  onRedraw,
  onKeep,
}: {
  change: CategoryChange;
  label: (id: string) => string;
  no: number;
  onRedraw: () => void;
  onKeep: () => void;
}) {
  return (
    <Card label="Category changed" title="Category changed">
      <p className={styles.p}>
        This profile is now <b>{label(change.to)}</b>. Its map was drawn on{" "}
        {label(change.from)}.
      </p>
      <p className={styles.muted}>
        Re-draw on the {label(change.to)} template? Draw {no} stays in Map
        history and comes back if you switch back.
      </p>
      <div className={styles.row}>
        <Button variant="primary" onClick={onRedraw}>
          Re-draw on {label(change.to)}
        </Button>
        <Button variant="ghost" onClick={onKeep}>
          Keep the old map for now
        </Button>
      </div>
    </Card>
  );
}

/** A drawn map with nothing on the line: valid, and says so. */
export function NothingOnLineCard({ addLink }: { addLink: string }) {
  return (
    <Card label="Nothing on the line" title="Nothing on the line">
      <p className={styles.p}>
        Claude put everything on the floor and the toolbelt. That’s a valid map:
        the stages stay empty and say so.
      </p>
      <p className={styles.muted}>
        <Info size={12} aria-hidden="true" /> Add skills or commands to give the
        line work to do.
      </p>
      <div className={styles.row}>
        <Link to={addLink} className={styles.linkBtn}>
          Add from library
        </Link>
      </div>
    </Card>
  );
}
