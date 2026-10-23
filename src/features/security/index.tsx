import {
  Globe,
  KeyRound,
  Files,
  RefreshCw,
  ShieldCheck,
  Terminal,
  X,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { ArtifactAudit } from "../../bindings/ArtifactAudit";
import { libraryAudits, libraryReaudit } from "../../lib/ipc";
import {
  Button,
  EmptyState,
  Panel,
  Screen,
  severityLabels,
  severityOrder,
  tiers,
} from "../../ui";
import {
  histogram,
  isAudited,
  reaches,
  reachesAnything,
  reachTiles,
  tierOfBucket,
  type ReachKey,
} from "./model";
import { FindingsTable, ReachTable } from "./Tables";
import styles from "./Security.module.css";

const tileIcons: Record<ReachKey, LucideIcon> = {
  network: Globe,
  shell: Terminal,
  env: KeyRound,
  files: Files,
};

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}

export function SecurityScreen() {
  const [all, setAll] = useState<ArtifactAudit[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [filter, setFilter] = useState<ReachKey | null>(null);

  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    libraryAudits().then(
      (list) => {
        if (!active) return;
        setAll(list);
        setError(null);
      },
      (e: unknown) => {
        if (active) setError(String(e));
      },
    );
    return () => {
      active = false;
    };
  }, [attempt]);

  async function rerun() {
    setRunning(true);
    setResult(null);
    try {
      const report = await libraryReaudit();
      setResult(
        `Re-audited ${plural(report.audited, "artifact")}: ${report.changed} changed`,
      );
      setAttempt((n) => n + 1);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  const rerunButton = (
    <Button onClick={() => void rerun()} disabled={running}>
      <RefreshCw size={14} aria-hidden="true" />
      {running ? "Re-running…" : "Re-run audit"}
    </Button>
  );
  const header = (
    <>
      <span className={styles.lede}>
        Every artifact is audited when it is imported or updated: trust score,
        findings, and exactly what it can reach.
      </span>
      <span className={styles.grow} />
      {result && (
        <span className={styles.result} role="status">
          {result}
        </span>
      )}
      {rerunButton}
    </>
  );

  if (error !== null) {
    return (
      <Screen title="Security" actions={header}>
        <p className={styles.error} role="alert">
          Could not read the audits: {error}
        </p>
      </Screen>
    );
  }
  if (all === null) {
    return (
      <Screen title="Security" actions={header}>
        <p className={styles.muted}>Loading…</p>
      </Screen>
    );
  }
  if (all.length === 0) {
    return (
      <Screen title="Security" actions={header}>
        <EmptyState icon={ShieldCheck} title="Nothing to audit yet">
          The library is empty. Import artifacts and each one is audited on the
          way in.
        </EmptyState>
      </Screen>
    );
  }
  const audited = all.filter(isAudited);
  if (audited.length === 0) {
    return (
      <Screen title="Security" actions={header}>
        <EmptyState icon={ShieldCheck} title="Nothing audited yet">
          {plural(all.length, "artifact")} in the library predate the audit.
          Choose Re-run audit to score them.
        </EmptyState>
      </Screen>
    );
  }

  const hist = histogram(audited);
  const hmax = Math.max(1, ...hist);
  const tierCount = tiers.map(
    (t) => audited.filter((a) => a.audit.tier === t.tier).length,
  );
  const sevCount = severityOrder.map(
    (s) =>
      audited.flatMap((a) => a.audit.findings).filter((f) => f.severity === s)
        .length,
  );
  const findingTotal = sevCount.reduce((x, y) => x + y, 0);
  const withFindings = audited.filter((a) => a.audit.findings.length > 0);
  const reachList = audited.filter((a) =>
    filter === null
      ? reachesAnything(a.audit.reach)
      : reaches(a.audit.reach, filter),
  );
  const tile = reachTiles.find((t) => t.key === filter);
  const unaudited = all.length - audited.length;

  return (
    <Screen title="Security" actions={header}>
      <div className={styles.panels}>
        <Panel title="Trust scores" titleId="sec-trust">
          <p className={styles.sub}>
            {plural(audited.length, "artifact")}
            {unaudited > 0 && `, ${unaudited} not audited yet`}
          </p>
          <div
            className={styles.bars}
            role="img"
            aria-label={`Trust score distribution: ${tiers
              .map((t, i) => `${tierCount[i]} ${t.label}`)
              .join(", ")}`}
          >
            {hist.map((n, i) => (
              <i
                key={i}
                data-tier={tierOfBucket(i)}
                data-empty={n === 0 ? "" : undefined}
                style={{
                  height: `${Math.max(4, Math.round((n / hmax) * 100))}%`,
                }}
                title={`${i * 10}–${i * 10 + 9}: ${n}`}
              />
            ))}
          </div>
          <div className={styles.axis} aria-hidden="true">
            <span>0</span>
            <span>50</span>
            <span>100</span>
          </div>
          <ul className={styles.legend}>
            {tiers.map((t, i) => (
              <li key={t.tier} data-tier={t.tier}>
                <i className={styles.dot} aria-hidden="true" />
                <b>{tierCount[i]}</b> {t.label}{" "}
                <span className={styles.mono}>
                  {t.lo}–{t.hi}
                </span>
              </li>
            ))}
          </ul>
        </Panel>

        <Panel title="Findings" titleId="sec-findings">
          <p className={styles.sub}>
            {findingTotal} in {plural(withFindings.length, "artifact")}
          </p>
          <div className={styles.sevRow}>
            {severityOrder.map((s, i) => (
              <div key={s} data-sev={s}>
                <b>{sevCount[i]}</b>
                <span>{severityLabels[s]}</span>
              </div>
            ))}
          </div>
          <p className={styles.note}>
            A critical finding drops the trust score by 25 points, a high one by
            15, so risky artifacts stand out in the library.
          </p>
        </Panel>

        <Panel title="What artifacts can reach" titleId="sec-reach">
          <p className={styles.sub}>Select one to filter the table below.</p>
          <div className={styles.tiles}>
            {reachTiles.map((t) => {
              const Icon = tileIcons[t.key];
              const n = audited.filter((a) => reaches(a.audit.reach, t.key));
              return (
                <button
                  key={t.key}
                  type="button"
                  className={styles.tile}
                  aria-pressed={filter === t.key}
                  onClick={() => setFilter(filter === t.key ? null : t.key)}
                >
                  <Icon size={16} aria-hidden="true" />
                  <span>
                    <b>{n.length}</b> {t.label}
                  </span>
                </button>
              );
            })}
          </div>
        </Panel>
      </div>

      <h2 className={styles.h}>Findings</h2>
      {findingTotal > 0 ? (
        <FindingsTable list={audited} />
      ) : (
        <EmptyState icon={ShieldCheck} title="No findings">
          Nothing in the library triggered an audit rule.
        </EmptyState>
      )}

      <h2 className={styles.h}>
        Reach
        {filter !== null && (
          <Button variant="ghost" small onClick={() => setFilter(null)}>
            <X size={13} aria-hidden="true" />
            Show all
          </Button>
        )}
        <span className={styles.hint}>
          {tile
            ? `filtered: ${tile.label}`
            : "artifacts that reach beyond their own files"}
        </span>
      </h2>
      <ReachTable list={reachList} />
    </Screen>
  );
}
