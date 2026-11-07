import { CircleAlert, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import type { DrawFailure } from "../../bindings/DrawFailure";
import type { FitChanges } from "../../bindings/FitChanges";
import { Button, Callout } from "../../ui";
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

/** The profile has no drawn map: Harnfit judges a candidate against it. */
export function NeedsMapCard({
  candidate,
  profile,
  onDraw,
}: {
  candidate: string;
  profile: string;
  onDraw: () => void;
}) {
  return (
    <Card label="Harnfit needs a map" title="Harnfit needs a map">
      <p className={styles.p}>
        To fit {candidate} into {profile}, {profile} needs a map first. Harnfit
        compares the candidate with what is already placed, and nothing is
        placed yet.
      </p>
      <div className={styles.row}>
        <Button variant="primary" onClick={onDraw}>
          Draw {profile}’s map first
        </Button>
      </div>
    </Card>
  );
}

/** Harnfit does not take a plugin whole yet. */
export function PluginCard({ onBack }: { onBack: () => void }) {
  return (
    <Card label="Plugin not supported yet" title="Plugins come later">
      <p className={styles.p}>
        Harnfit for plugins comes later. A plugin installs as one unit, so for
        now fit its skills, commands or agents one at a time.
      </p>
      <div className={styles.row}>
        <Button onClick={onBack}>Back</Button>
      </div>
    </Card>
  );
}

/** The candidate is already in the profile: nothing to fit. */
export function AlreadyInCard({
  message,
  onBack,
}: {
  message: string;
  onBack: () => void;
}) {
  return (
    <Card label="Already in the profile" title="Already in this profile">
      <p className={styles.p}>{message}</p>
      <div className={styles.row}>
        <Button onClick={onBack}>Back</Button>
      </div>
    </Card>
  );
}

/** No saved fit for this candidate, and it can't be run from here. */
export function NoFitCard({ onBack }: { onBack: () => void }) {
  return (
    <Card label="No saved fit" title="No saved fit here">
      <p className={styles.p}>
        This candidate has no saved fit, and it can’t be fitted from this link.
        Start Harnfit again from the library or the import review.
      </p>
      <div className={styles.row}>
        <Button onClick={onBack}>Back</Button>
      </div>
    </Card>
  );
}

/** What changed since the fit, in words. */
export function changeSummary(
  changes: FitChanges,
  nameOf: (id: string) => string,
): string {
  const list = (ids: string[]) => ids.map(nameOf).join(", ");
  const parts: string[] = [];
  if (changes.added.length > 0) parts.push(`added ${list(changes.added)}`);
  if (changes.removed.length > 0)
    parts.push(`removed ${list(changes.removed)}`);
  if (changes.changed.length > 0)
    parts.push(`changed ${list(changes.changed)}`);
  if (changes.candidateChanged) parts.push("the candidate’s files changed");
  return parts.length > 0 ? parts.join("; ") : "something changed";
}

/** The fit is out of date: Run again, or keep reading the old verdict. */
export function StaleBanner({
  summary,
  canRun,
  onRun,
  onKeep,
}: {
  summary: string;
  canRun: boolean;
  onRun: () => void;
  onKeep: () => void;
}) {
  return (
    <div className={styles.banner} role="status">
      <b>This fit is out of date.</b>
      <span>Since it was made: {summary}.</span>
      <Button small disabled={!canRun} onClick={onRun}>
        Run again
      </Button>
      <Button small variant="ghost" onClick={onKeep}>
        Keep the old verdict
      </Button>
    </div>
  );
}

/** A fit that failed: nothing was saved and the verdict on screen, if any, stands. */
export function FitFailureCard({
  failure,
  canRun,
  onRetry,
  onGuide,
}: {
  failure: Exclude<DrawFailure, { type: "cancelled" | "emptyProfile" }>;
  canRun: boolean;
  onRetry: () => void;
  onGuide: () => void;
}) {
  const retry = (
    <div className={styles.row}>
      <Button variant="primary" disabled={!canRun} onClick={onRetry}>
        Try again
      </Button>
      {failure.type === "claudeMissing" && (
        <Button variant="ghost" onClick={onGuide}>
          Claude Code setup guide
        </Button>
      )}
    </div>
  );
  switch (failure.type) {
    case "claudeMissing":
      return (
        <Card label="Harnfit failed" title="claude CLI not found">
          <Callout tone="warn" icon={TriangleAlert}>
            Harnfit asks Claude through your own Claude Code CLI, and it isn’t
            on your PATH.
          </Callout>
          {retry}
        </Card>
      );
    case "badAnswer":
      return (
        <Card label="Harnfit failed" title="Answer didn’t check out">
          <Callout tone="bad" icon={CircleAlert}>
            <b>Claude’s answer failed Harneo’s checks.</b> Nothing was saved.
          </Callout>
          <ul className={styles.reasons}>
            {failure.reasons.map((r) => (
              <li key={r}>{r}</li>
            ))}
          </ul>
          {retry}
        </Card>
      );
    case "claudeFailed":
    case "other":
      return (
        <Card label="Harnfit failed" title="Harnfit failed">
          <Callout tone="bad" icon={CircleAlert}>
            <b>Claude couldn’t give a verdict.</b> {failure.message} Nothing was
            saved.
          </Callout>
          {retry}
        </Card>
      );
  }
}
