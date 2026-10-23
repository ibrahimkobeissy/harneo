import {
  CircleArrowUp,
  CircleCheck,
  Copy,
  Info,
  Moon,
  ShieldAlert,
  type LucideIcon,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Link } from "react-router";
import type { Cleanup } from "../../bindings/Cleanup";
import type { CleanupItem } from "../../bindings/CleanupItem";
import type { Tier } from "../../bindings/Tier";
import {
  cleanupList,
  settingsGet,
  updateAll,
  updateApply,
} from "../../lib/ipc";
import { useChanged } from "../../lib/useChanged";
import { Badge, Button, EmptyState, Screen, TrustMeter, kinds } from "../../ui";
import styles from "./Cleanup.module.css";
import {
  plural,
  PruneDialog,
  RemoveFromLibrary,
  UpdateAllDialog,
} from "./Dialogs";

const UNUSED_SHOWN = 12;
const UNUSED_KEY = "usage.unusedWeeks";
const DEFAULT_WEEKS = 8;
const NOTICE_MS = 6000;

interface Notice {
  text: string;
  tone: "ok" | "bad";
}

type Dialog =
  | { kind: "remove"; item: CleanupItem }
  | { kind: "prune"; item: CleanupItem }
  | { kind: "updateAll" };

/** The trust tier of a score, as the audit bands it. */
function tierOf(score: number | null): Tier | null {
  if (score === null) return null;
  if (score >= 80) return "trusted";
  if (score >= 60) return "review";
  if (score >= 40) return "risky";
  return "dangerous";
}

function Group({
  id,
  title,
  icon: Icon,
  count,
  actions,
  children,
}: {
  id: string;
  title: string;
  icon: LucideIcon;
  count: number;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className={styles.group} aria-labelledby={id}>
      <header className={styles.ghead}>
        <Icon size={15} aria-hidden="true" />
        <h2 id={id}>{title}</h2>
        <span className={styles.count}>{count}</span>
        <span className={styles.grow} />
        {actions}
      </header>
      {children}
    </section>
  );
}

function Who({ item }: { item: CleanupItem }) {
  return (
    <div className={styles.grow}>
      <Link
        className={styles.name}
        to={`/library?open=${encodeURIComponent(item.id)}`}
      >
        {item.name}
      </Link>
      <div className={styles.kind}>{kinds[item.kind].label}</div>
    </div>
  );
}

function inProfiles(item: CleanupItem): string {
  return item.profiles.length > 0
    ? `in ${item.profiles.map((p) => p.name).join(", ")}`
    : "in no profile";
}

function lastUsedText(item: CleanupItem): string {
  return item.lastUsed ? `last used ${item.lastUsed}` : "never used";
}

/** Cleanup: what is risky, stale, never used or repeated, with the one-click ways out. */
export function CleanupScreen() {
  const [data, setData] = useState<Cleanup | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [weeks, setWeeks] = useState(DEFAULT_WEEKS);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [busy, setBusy] = useState(false);
  const alive = useRef(true);

  const load = useCallback(() => {
    cleanupList().then(
      (next) => {
        if (!alive.current) return;
        setData(next);
        setLoadError(null);
      },
      (err: unknown) => {
        if (alive.current) setLoadError(String(err));
      },
    );
  }, []);

  useEffect(() => {
    alive.current = true;
    load();
    settingsGet(UNUSED_KEY).then(
      (v) => {
        if (alive.current && typeof v === "number" && v >= 1) setWeeks(v);
      },
      () => undefined,
    );
    return () => {
      alive.current = false;
    };
  }, [load]);
  useChanged(() => load());

  useEffect(() => {
    if (!notice || notice.tone === "bad") return;
    const timer = setTimeout(() => setNotice(null), NOTICE_MS);
    return () => clearTimeout(timer);
  }, [notice]);

  function finish(text: string, tone: Notice["tone"] = "ok") {
    setDialog(null);
    setNotice({ text, tone });
    load();
  }

  async function updateOne(item: CleanupItem) {
    try {
      const report = await updateApply(item.id, "takeUpstream");
      if (report.outcome === "failed")
        setNotice({
          text: `Could not update ${item.name}. ${report.message ?? ""}`.trim(),
          tone: "bad",
        });
      else setNotice({ text: `Updated ${item.name}`, tone: "ok" });
    } catch (err) {
      setNotice({
        text: `Could not update ${item.name}. ${String(err)}`,
        tone: "bad",
      });
    }
    load();
  }

  async function runUpdateAll() {
    setBusy(true);
    try {
      const reports = await updateAll();
      const done = reports.filter((r) => r.outcome === "updated").length;
      const failed = reports.filter((r) => r.outcome === "failed").length;
      finish(
        failed > 0
          ? `Updated ${plural(done, "artifact")}; ${failed} failed`
          : `Updated ${plural(done, "artifact")}`,
        failed > 0 ? "bad" : "ok",
      );
    } catch (err) {
      finish(`Could not update. ${String(err)}`, "bad");
    } finally {
      setBusy(false);
    }
  }

  const empty =
    data !== null &&
    data.findings.length +
      data.outdated.length +
      data.unused.length +
      data.repeated.length +
      data.duplicates.length ===
      0;

  return (
    <Screen title="Cleanup" narrow>
      <p className={styles.sub}>
        Keep the harness lean: what&apos;s risky, stale or never used.
      </p>
      {loadError && (
        <p className={styles.err} role="alert">
          Could not load the suggestions. {loadError}
        </p>
      )}
      {!data && !loadError && (
        <p className={styles.muted} role="status">
          Looking through the library…
        </p>
      )}
      {empty && (
        <EmptyState icon={CircleCheck} title="Nothing to clean up">
          No findings, no outdated or unused artifacts.
        </EmptyState>
      )}
      {data && data.findings.length > 0 && (
        <Group
          id="cleanup-findings"
          title="Security findings"
          icon={ShieldAlert}
          count={data.findings.length}
        >
          {data.findings.map((a) => (
            <div className={styles.row} key={a.id}>
              <Who item={a} />
              <TrustMeter score={a.trustScore} tier={tierOf(a.trustScore)} />
              <Badge tone="bad">{plural(a.findingCount, "finding")}</Badge>
              <Button
                small
                variant="danger"
                onClick={() => setDialog({ kind: "remove", item: a })}
              >
                Remove from library
              </Button>
            </div>
          ))}
        </Group>
      )}
      {data && data.outdated.length > 0 && (
        <Group
          id="cleanup-outdated"
          title="Outdated"
          icon={CircleArrowUp}
          count={data.outdated.length}
          actions={
            <Button small onClick={() => setDialog({ kind: "updateAll" })}>
              Update all
            </Button>
          }
        >
          {data.outdated.map((a) => (
            <div className={styles.row} key={a.id}>
              <Who item={a} />
              {a.localEdits && <Badge tone="warn">local edits</Badge>}
              <span className={styles.mono}>
                {a.fromVersion ?? "?"} → {a.toVersion ?? "?"}
              </span>
              {a.localEdits ? (
                <Link
                  className={styles.link}
                  to={`/library?open=${encodeURIComponent(a.id)}`}
                >
                  Review
                </Link>
              ) : (
                <Button small onClick={() => void updateOne(a)}>
                  Update
                </Button>
              )}
            </div>
          ))}
        </Group>
      )}
      {data && data.unused.length > 0 && (
        <Group
          id="cleanup-unused"
          title={`Unused for ${weeks}+ weeks, in every project`}
          icon={Moon}
          count={data.unused.length}
        >
          {data.unused.slice(0, UNUSED_SHOWN).map((a) => (
            <div className={styles.row} key={a.id}>
              <Who item={a} />
              <span className={styles.muted}>{lastUsedText(a)}</span>
              <span className={styles.muted}>{inProfiles(a)}</span>
              {a.profiles.length > 0 ? (
                <Button
                  small
                  onClick={() => setDialog({ kind: "prune", item: a })}
                >
                  Remove from profiles
                </Button>
              ) : (
                <Button
                  small
                  variant="danger"
                  onClick={() => setDialog({ kind: "remove", item: a })}
                >
                  Remove from library
                </Button>
              )}
            </div>
          ))}
          {data.unused.length > UNUSED_SHOWN && (
            <div className={styles.row}>
              <Link className={styles.link} to="/library?status=unused">
                See all {data.unused.length} in the library
              </Link>
            </div>
          )}
        </Group>
      )}
      {data && data.repeated.length > 0 && (
        <Group
          id="cleanup-repeated"
          title="Repeated across profiles"
          icon={Copy}
          count={data.repeated.length}
        >
          {data.repeated.map((a) => (
            <div className={styles.row} key={a.id}>
              <Who item={a} />
              <span className={styles.muted}>{inProfiles(a)}</span>
            </div>
          ))}
          <div className={`${styles.row} ${styles.note}`}>
            <Info size={15} aria-hidden="true" />
            Not a problem in itself; a candidate for one base profile you clone
            from.
          </div>
        </Group>
      )}
      {data && data.duplicates.length > 0 && (
        <Group
          id="cleanup-duplicates"
          title="Duplicate names"
          icon={Copy}
          count={data.duplicates.length}
        >
          {data.duplicates.map((d) => (
            <div className={styles.row} key={`${d.kind}:${d.name}`}>
              <div className={styles.grow}>
                <div className={styles.name}>{d.name}</div>
                <div className={styles.kind}>{kinds[d.kind].label}</div>
              </div>
              <span className={styles.muted}>
                {plural(d.ids.length, "copy", "copies")}
              </span>
              <Link
                className={styles.link}
                to={`/library?q=${encodeURIComponent(d.name)}`}
              >
                Show in library
              </Link>
            </div>
          ))}
        </Group>
      )}
      {dialog?.kind === "remove" && (
        <RemoveFromLibrary
          item={dialog.item}
          onClose={() => setDialog(null)}
          onDone={(text) => finish(text)}
        />
      )}
      {dialog?.kind === "prune" && (
        <PruneDialog
          item={dialog.item}
          onClose={() => setDialog(null)}
          onDone={(report) => {
            const name = dialog.item.name;
            if (report.failed)
              finish(
                `Stopped at ${report.failed.subject}: ${report.failed.message}. ${plural(report.profiles.length, "profile")} saved, ${plural(report.switched.length, "project")} switched before it.`,
                "bad",
              );
            else
              finish(
                `Removed ${name} from ${plural(report.profiles.length, "profile")}` +
                  (report.switched.length > 0
                    ? ` and switched ${plural(report.switched.length, "project")}`
                    : ""),
              );
          }}
        />
      )}
      {dialog?.kind === "updateAll" && data && (
        <UpdateAllDialog
          total={data.outdated.length}
          edited={data.outdated.filter((a) => a.localEdits)}
          busy={busy}
          onClose={() => setDialog(null)}
          onConfirm={() => void runUpdateAll()}
        />
      )}
      {notice && (
        <div
          className={styles.toast}
          data-tone={notice.tone}
          role={notice.tone === "bad" ? "alert" : "status"}
        >
          <span className={styles.toastText}>{notice.text}</span>
          <button
            type="button"
            className={styles.toastClose}
            onClick={() => setNotice(null)}
          >
            Dismiss
          </button>
        </div>
      )}
    </Screen>
  );
}
