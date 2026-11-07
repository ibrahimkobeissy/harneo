import { Check, CircleAlert, TriangleAlert } from "lucide-react";
import type { FitView } from "../../bindings/FitView";
import { Badge, Button } from "../../ui";
import { modelLabel, took } from "./draw";
import { fitDate, verdictLabel, verdictTone } from "./harnfit";
import styles from "./Harnfit.module.css";

/** The verdict: colour, where it would go, why, the change, the checked quotes, the missing MCP. */
export function VerdictCard({
  fit,
  zone,
  profile,
  nameOf,
  busy,
  onAdd,
  onElsewhere,
  onDiscard,
  onImportServer,
}: {
  fit: FitView;
  /** Where Add will place it: the proposal, or what the user dragged it to. */
  zone: string;
  profile: string;
  /** The name of a profile artifact by id. */
  nameOf: (artifact: string) => string;
  busy: boolean;
  onAdd: () => void;
  onElsewhere: () => void;
  onDiscard: () => void;
  onImportServer: (server: string) => void;
}) {
  const v = fit.verdict;
  const name = v.candidate.label;
  const inLibrary = v.candidate.key.startsWith("lib:");
  const missing = v.missingMcp.filter((m) => !m.inLibrary);
  const Icon =
    v.verdict === "fits"
      ? Check
      : v.verdict === "fitsWithChanges"
        ? TriangleAlert
        : CircleAlert;
  return (
    <section
      className={styles.verdict}
      data-verdict={v.verdict}
      aria-label={`Verdict for ${name}`}
    >
      <div className={styles.verdictHead}>
        <Badge tone={verdictTone(v.verdict)}>
          <Icon size={12} aria-hidden="true" /> {verdictLabel(v.verdict)}
        </Badge>
        <span className={styles.note}>Would go: {zone}</span>
      </div>
      <p className={styles.why}>{v.why}</p>
      {v.conflictsWith.length > 0 && (
        <p className={styles.p}>
          Clashes with{" "}
          {v.conflictsWith.map((id, i) => (
            <span key={id}>
              {i > 0 && ", "}
              <code>{nameOf(id)}</code>
            </span>
          ))}
          . Red lines on the map run to them.
        </p>
      )}
      {v.change && (
        <div className={styles.quote} data-kind="change">
          <b>Proposed change</b>
          {"\n"}
          {v.change}
        </div>
      )}
      {v.quotes.map((q) => (
        <div
          key={`${q.artifact}:${q.file}:${q.line}`}
          className={styles.quote}
          data-kind={q.artifact === "candidate" ? "own" : "other"}
        >
          <b>
            {q.artifact === "candidate" ? name : nameOf(q.artifact)} · {q.file}:
            {q.line}
          </b>
          {"\n"}
          {q.text}
        </div>
      ))}
      {v.quotes.length > 0 && (
        <p className={styles.checked}>
          <Check size={12} aria-hidden="true" /> Both quotes checked word for
          word against the files. Line numbers found by Harneo, not Claude.
        </p>
      )}
      {v.droppedQuotes > 0 && (
        <p className={styles.note}>
          {v.droppedQuotes} {v.droppedQuotes === 1 ? "quote" : "quotes"}{" "}
          couldn’t be checked and {v.droppedQuotes === 1 ? "was" : "were"}{" "}
          removed.
        </p>
      )}
      {v.missingMcp.map((m) => (
        <div key={m.server} className={styles.mcp}>
          <span>
            <code>{m.server}</code>{" "}
            {m.inLibrary
              ? "is in your library. Add still puts only this artifact in the profile; add the server to it yourself."
              : "isn’t in your library."}
          </span>
          {!m.inLibrary && (
            <Button small onClick={() => onImportServer(m.server)}>
              Import {m.server} first
            </Button>
          )}
        </div>
      ))}
      <div className={styles.actions}>
        <Button variant="primary" disabled={busy || !inLibrary} onClick={onAdd}>
          {missing.length > 0 ? "Add without it" : "Add to profile"}
        </Button>
        <Button disabled={busy} onClick={onElsewhere}>
          Place elsewhere
        </Button>
        <Button variant="ghost" onClick={onDiscard}>
          Discard
        </Button>
      </div>
      {!inLibrary && (
        <p className={styles.note}>
          Import it first: {name} isn’t in your library yet, so it can’t be
          added to {profile}.
        </p>
      )}
      <p className={styles.note}>
        Fit saved {fitDate(v.createdAt)} · {modelLabel(v.model)} ·{" "}
        {took(v.durationMs)}. This is Claude’s opinion: Harneo never blocks an
        add.
      </p>
    </section>
  );
}
