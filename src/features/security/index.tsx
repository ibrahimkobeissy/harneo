import {
  Globe,
  KeyRound,
  Files,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Terminal,
  X,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ConfirmProgress } from "../../bindings/ConfirmProgress";
import type { ArtifactAudit } from "../../bindings/ArtifactAudit";
import { useArtifactPanelChanged } from "../../lib/artifactPanel";
import {
  auditConfirmAll,
  auditConfirmSettingGet,
  claudeCliStatus,
  libraryAudits,
  libraryReaudit,
  onAuditConfirmProgress,
} from "../../lib/ipc";
import {
  Button,
  Phase1Warning,
  EmptyState,
  Panel,
  Screen,
  severityLabels,
  severityOrder,
  tierLabels,
  tiers,
  TrustMeter,
} from "../../ui";
import {
  barShare,
  groupFindings,
  histogram,
  isAudited,
  kindLabel,
  reachDetail,
  reaches,
  reachesAnything,
  reachTiles,
  scoreChanges,
  tierOfBucket,
  type Audited,
  type ReachKey,
  type ScoreChange,
} from "./model";
import { ArtifactLink, FindingsTable, ReachTable } from "./Tables";
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

/** What a re-audit did: the counts from the core and the artifacts whose score moved. */
interface Rerun {
  audited: number;
  changed: number;
  moved: ScoreChange[];
}

function kindLabelOf(a: Audited): string {
  return kindLabel[a.kind];
}

const shownMoves = 8;
const shownAttention = 8;

function Moves({ rerun, onClose }: { rerun: Rerun; onClose: () => void }) {
  const { moved } = rerun;
  const hidden = Math.max(0, moved.length - shownMoves);
  const silent = rerun.changed - moved.length;
  return (
    <div className={styles.rerun} role="status">
      <div className={styles.rerunHead}>
        <b>
          Re-audited {plural(rerun.audited, "artifact")}: {rerun.changed}{" "}
          changed
        </b>
        <Button variant="ghost" small onClick={onClose}>
          <X size={13} aria-hidden="true" />
          Dismiss
        </Button>
      </div>
      {moved.length > 0 && (
        <ul className={styles.moves}>
          {moved.slice(0, shownMoves).map((m) => (
            <li key={m.id} data-tier={m.tier}>
              <span className={styles.moveName}>{m.name}:</span>{" "}
              <span className={styles.mono}>
                {m.before === null ? "not audited" : m.before} → {m.after}
              </span>{" "}
              <span className={styles.moveTier}>({tierLabels[m.tier]})</span>
            </li>
          ))}
          {hidden > 0 && <li className={styles.muted}>and {hidden} more</li>}
        </ul>
      )}
      {silent > 0 && (
        <p className={styles.muted}>
          {silent === 1
            ? "1 more changed its findings but kept its score."
            : `${silent} more changed their findings but kept their scores.`}
        </p>
      )}
    </div>
  );
}

/** Dangerous and Risky artifacts, worst first: what to look at before anything else. */
function Attention({ list }: { list: readonly Audited[] }) {
  const rows = list
    .filter((a) => a.audit.tier === "dangerous" || a.audit.tier === "risky")
    .sort((a, b) => a.audit.score - b.audit.score);
  if (rows.length === 0) return null;
  const hidden = rows.length - shownAttention;
  return (
    <section className={styles.attention} aria-labelledby="sec-attention">
      <h2 className={styles.h} id="sec-attention">
        Needs attention
        <span className={styles.hint}>
          {plural(rows.length, "artifact")} scored Risky or Dangerous
        </span>
      </h2>
      <ul className={styles.attList}>
        {rows.slice(0, shownAttention).map((a) => {
          const top = groupFindings([a])[0];
          return (
            <li key={a.id} data-tier={a.audit.tier}>
              <ArtifactLink a={a} />
              <span className={styles.kind}>{kindLabelOf(a)}</span>
              <span className={styles.attNote}>
                {top
                  ? `${plural(top.findings.length, "finding")}, worst ${severityLabels[top.worst].toLowerCase()}`
                  : "no findings"}
              </span>
              <TrustMeter score={a.audit.score} tier={a.audit.tier} size="md" />
            </li>
          );
        })}
      </ul>
      {hidden > 0 && (
        <p className={styles.muted}>
          and {hidden} more in the tables below, lowest trust first
        </p>
      )}
    </section>
  );
}

export function SecurityScreen() {
  const [all, setAll] = useState<ArtifactAudit[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [rerunResult, setRerunResult] = useState<Rerun | null>(null);
  const [filter, setFilter] = useState<ReachKey | null>(null);
  const [rev, setRev] = useState(0);
  const [confirmOn, setConfirmOn] = useState(true);
  const [cliFound, setCliFound] = useState<boolean | null>(null);
  const [progress, setProgress] = useState<ConfirmProgress | null>(null);
  const [confirming, setConfirming] = useState(false);
  useArtifactPanelChanged(() => setRev((n) => n + 1));
  const reachRef = useRef<HTMLHeadingElement>(null);

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
  }, [rev]);

  // Settings and the CLI decide whether the warning shows; read again when the screen is shown.
  useEffect(() => {
    let live = true;
    auditConfirmSettingGet().then(
      (on) => live && setConfirmOn(on !== false),
      () => undefined,
    );
    claudeCliStatus().then(
      (cli) => live && setCliFound(cli?.found === true),
      () => live && setCliFound(false),
    );
    return () => {
      live = false;
    };
  }, []);

  // Phase 2 runs in the background after an import, an update or a re-audit, too.
  useEffect(() => {
    let live = true;
    let stop: (() => void) | undefined;
    onAuditConfirmProgress((p) => {
      if (!live) return;
      if (p.done >= p.total) {
        setProgress(null);
        setRev((n) => n + 1);
      } else {
        setProgress(p);
      }
    }).then(
      (fn) => {
        if (live) stop = fn;
        else fn();
      },
      () => undefined,
    );
    return () => {
      live = false;
      stop?.();
    };
  }, []);

  async function confirmNow() {
    setConfirming(true);
    setProgress({ done: 0, total: 0 });
    try {
      await auditConfirmAll();
      setAll(await libraryAudits());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setConfirming(false);
      setProgress(null);
    }
  }

  async function rerun() {
    setRunning(true);
    setRerunResult(null);
    try {
      const before = all ?? [];
      const report = await libraryReaudit();
      const after = await libraryAudits();
      setAll(after);
      setRerunResult({
        audited: report.audited,
        changed: report.changed,
        moved: scoreChanges(before, after),
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  function pick(key: ReachKey) {
    const next = filter === key ? null : key;
    setFilter(next);
    const heading = reachRef.current;
    const body = heading?.closest("[data-screen-body]");
    if (next !== null && heading && body instanceof HTMLElement) {
      // Scroll the screen body itself: scrollIntoView would also move ancestors.
      const top =
        heading.getBoundingClientRect().top -
        body.getBoundingClientRect().top +
        body.scrollTop;
      body.scrollTo?.({ top: Math.max(0, top - 8), behavior: "smooth" });
    }
  }

  const rerunButton = (
    <Button onClick={() => void rerun()} disabled={running}>
      <RefreshCw size={14} aria-hidden="true" />
      {running ? "Re-running…" : "Re-run audit"}
    </Button>
  );
  const working = confirming || progress !== null;
  const warn = !confirmOn || cliFound === false;
  const confirmBar = (
    <>
      {warn && <Phase1Warning cliMissing={confirmOn && cliFound === false} />}
      {working && (
        <p className={styles.progress} role="status">
          {progress !== null && progress.total > 0
            ? `Confirming ${Math.min(progress.done + 1, progress.total)} of ${progress.total} with Claude…`
            : "Confirming with Claude…"}
        </p>
      )}
    </>
  );
  const header = (
    <>
      <span className={styles.lede}>
        Every artifact is audited when it is imported or updated: trust score,
        findings, and exactly what it can reach.
      </span>
      <span className={styles.grow} />
      <Button
        onClick={() => void confirmNow()}
        disabled={working || running || !confirmOn || cliFound !== true}
      >
        <Sparkles size={14} aria-hidden="true" />
        Confirm now
      </Button>
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
        {confirmBar}
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
        {confirmBar}
        {rerunResult && (
          <Moves rerun={rerunResult} onClose={() => setRerunResult(null)} />
        )}
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
      {confirmBar}
      {rerunResult && (
        <Moves rerun={rerunResult} onClose={() => setRerunResult(null)} />
      )}
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
              <div
                key={i}
                className={styles.bucket}
                data-tier={tierOfBucket(i)}
                data-empty={n === 0 ? "" : undefined}
                title={`${i * 10}–${i * 10 + 9}: ${n}`}
              >
                <span className={styles.count}>{n > 0 ? n : ""}</span>
                <i
                  style={{ height: `${Math.round(barShare(n, hmax) * 100)}%` }}
                />
              </div>
            ))}
          </div>
          <div className={styles.axis} aria-hidden="true">
            <span>0</span>
            <span>50</span>
            <span>100</span>
          </div>
          <p className={styles.scaleNote}>
            Bar heights are square-root scaled so small groups stay visible.
          </p>
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

        <Panel title="By severity" titleId="sec-findings">
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
          <p className={styles.sub}>
            {tile ? (
              <>
                Showing {plural(reachList.length, "artifact")} that{" "}
                {reachList.length === 1 ? tile.singular : tile.label}.{" "}
                <button
                  type="button"
                  className={styles.inline}
                  onClick={() => setFilter(null)}
                >
                  Show all
                </button>
              </>
            ) : (
              "Select one to filter the Reach table."
            )}
          </p>
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
                  onClick={() => pick(t.key)}
                >
                  <Icon size={16} aria-hidden="true" className={styles.icon} />
                  <span>
                    <b>{n.length}</b>{" "}
                    {n.length === 1
                      ? `artifact ${t.singular}`
                      : `artifacts ${t.label}`}
                    <span className={styles.detail}>{reachDetail(n, t)}</span>
                  </span>
                </button>
              );
            })}
          </div>
        </Panel>
      </div>

      <Attention list={audited} />

      <h2 className={styles.h} ref={reachRef}>
        Reach
        <span className={styles.hint}>
          {tile
            ? `filtered: ${tile.label}`
            : "artifacts that reach beyond their own files"}
        </span>
      </h2>
      <ReachTable list={reachList} />

      <h2 className={styles.h}>Findings by artifact</h2>
      {findingTotal > 0 ? (
        <FindingsTable list={audited} />
      ) : (
        <EmptyState icon={ShieldCheck} title="No findings">
          Nothing in the library triggered an audit rule.
        </EmptyState>
      )}
    </Screen>
  );
}
