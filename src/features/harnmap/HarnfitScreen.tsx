import { FileQuestion, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";
import type { DrawFailure } from "../../bindings/DrawFailure";
import type { FitCandidate } from "../../bindings/FitCandidate";
import type { FitSummary } from "../../bindings/FitSummary";
import type { FitView } from "../../bindings/FitView";
import type { MapView } from "../../bindings/MapView";
import type { Zone } from "../../bindings/Zone";
import {
  harnfitAdd,
  harnfitCancel,
  harnfitGet,
  harnfitList,
  harnfitRun,
  harnmapEdit,
  harnmapGet,
  openExternal,
  profileGet,
  settingsGet,
} from "../../lib/ipc";
import { announce, Badge, Button, EmptyState } from "../../ui";
import {
  asFailure,
  clock,
  MODEL_KEY,
  modelLabel,
  toMapData,
  toModel,
} from "./draw";
import {
  AlreadyInCard,
  changeSummary,
  FitFailureCard,
  NeedsMapCard,
  NoFitCard,
  PluginCard,
  StaleBanner,
} from "./FitStates";
import {
  candidateFor,
  fitDate,
  fitProblem,
  harnfitPath,
  PREVIEW_PREFIX,
  verdictLabel,
  verdictTone,
  type FitProblem,
  type FitRouteState,
} from "./harnfit";
import fitStyles from "./Harnfit.module.css";
import styles from "./Harnmap.module.css";
import { MapCanvas } from "./MapCanvas";
import { sameZone, zoneName } from "./place";
import { DEFAULT_VIEW, type Dnd, type MapViewState } from "./types";
import { VerdictCard } from "./VerdictCard";

/** How long a toast stays. */
const TOAST_MS = 5000;

interface Base {
  name: string;
  view: MapView;
  fits: FitSummary[];
  model: string;
}

type Load =
  { status: "loading" } | { status: "error" } | ({ status: "ready" } & Base);

interface Problem {
  kind: FitProblem;
  message: string;
}

function parse(id: string | undefined): bigint | null {
  try {
    return id ? BigInt(id) : null;
  } catch {
    return null;
  }
}

/**
 * The Harnfit screen: the profile's map faded, the candidate as a dashed NEW node at its
 * proposed zone, and the verdict beside it. A saved fit opens at once with no Claude call; a
 * candidate with none runs Claude (the honest wait) and saves the verdict.
 */
export function HarnfitScreen() {
  const { id, candidate = "" } = useParams();
  const profile = parse(id);
  const [load, setLoad] = useState<Load>({ status: "loading" });

  useEffect(() => {
    if (profile === null) return;
    let live = true;
    Promise.all([
      profileGet(profile),
      harnmapGet(profile),
      harnfitList(profile),
      settingsGet(MODEL_KEY),
    ]).then(
      ([p, view, fits, model]) => {
        if (live)
          setLoad({
            status: "ready",
            name: p.name,
            view,
            fits: Array.isArray(fits) ? fits : [],
            model: toModel(model),
          });
      },
      () => live && setLoad({ status: "error" }),
    );
    return () => {
      live = false;
    };
  }, [profile]);

  if (profile === null || load.status === "error")
    return (
      <div className={styles.screen}>
        <EmptyState icon={FileQuestion} title="This profile is no longer here">
          It may have been removed; go back to Profiles to pick another.
        </EmptyState>
      </div>
    );
  if (load.status === "loading")
    return (
      <div className={styles.screen}>
        <p className={styles.loading} role="status">
          Loading Harnfit…
        </p>
      </div>
    );
  return (
    <Loaded
      key={candidate}
      profile={profile}
      candidate={candidate}
      base={load}
    />
  );
}

function Loaded({
  profile,
  candidate,
  base,
}: {
  profile: bigint;
  candidate: string;
  base: Base;
}) {
  const navigate = useNavigate();
  const state = (useLocation().state as FitRouteState | null) ?? null;
  const run: FitCandidate | null = candidateFor(candidate, state);
  const mapPath = `/profiles/${profile}/harnmap`;
  const { view, name } = base;
  const [current, setCurrent] = useState<FitView | null>(null);
  const [fits, setFits] = useState(base.fits);
  const [since, setSince] = useState<number | null>(null);
  const [now, setNow] = useState(0);
  const [failure, setFailure] = useState<Exclude<
    DrawFailure,
    { type: "cancelled" | "emptyProfile" }
  > | null>(null);
  const [problem, setProblem] = useState<Problem | null>(null);
  const [kept, setKept] = useState(false);
  const [zone, setZone] = useState<Zone | null>(null);
  const [dragging, setDragging] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [mapView, setMapView] = useState<MapViewState>(DEFAULT_VIEW);
  const [busy, setBusy] = useState(false);
  const started = useRef(false);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  // The clock of the honest wait.
  useEffect(() => {
    if (since === null) return;
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(tick);
  }, [since]);

  // A toast leaves by itself.
  useEffect(() => {
    if (toast === null) return;
    const timer = setTimeout(() => setToast(null), TOAST_MS);
    return () => clearTimeout(timer);
  }, [toast]);

  const nameOf = (artifact: string) =>
    view.artifacts[artifact]?.name ?? artifact;

  function say(text: string) {
    setToast(text);
    announce(text);
  }

  async function ask() {
    if (!run) return;
    setFailure(null);
    setProblem(null);
    setKept(false);
    setSince(Date.now());
    setNow(Date.now());
    try {
      const done = await harnfitRun(profile, run);
      if (!alive.current) return;
      setSince(null);
      setCurrent(done);
      setZone(null);
      harnfitList(profile).then(
        (list) => alive.current && Array.isArray(list) && setFits(list),
        () => undefined,
      );
      // A preview candidate had no key until now: show the saved fit under its own address.
      if (candidate.startsWith(PREVIEW_PREFIX))
        navigate(harnfitPath(profile, done.verdict.candidate.key), {
          replace: true,
          state,
        });
    } catch (err) {
      if (!alive.current) return;
      setSince(null);
      const failed = asFailure(err);
      if (failed.type === "cancelled") {
        announce("Harnfit cancelled. Nothing was saved.");
        return;
      }
      if (failed.type === "emptyProfile") {
        setProblem({ kind: "noMap", message: "" });
        return;
      }
      const message = failed.type === "other" ? failed.message : "";
      const kind = fitProblem(message);
      if (kind) setProblem({ kind, message });
      else setFailure(failed);
    }
  }

  // The latest `ask` for the opening effect, which runs once.
  const latest = useRef({ ask, canRun: run !== null });
  useEffect(() => {
    latest.current = { ask, canRun: run !== null };
  });

  // Open: a saved fit shows at once; otherwise ask Claude, which the click that got us here allows.
  const drawn = view.drawn;
  useEffect(() => {
    if (started.current || !drawn) return;
    started.current = true;
    if (candidate.startsWith(PREVIEW_PREFIX)) {
      void latest.current.ask();
      return;
    }
    harnfitGet(profile, candidate).then(
      (saved) => {
        if (!alive.current) return;
        if (saved) setCurrent(saved);
        else if (latest.current.canRun) void latest.current.ask();
        else setProblem({ kind: "notFound", message: "" });
      },
      (err: unknown) => {
        if (alive.current) setFailure({ type: "other", message: String(err) });
      },
    );
  }, [drawn, profile, candidate]);

  const proposed = current?.verdict.zone ?? null;
  const placed = zone ?? proposed;

  async function add() {
    if (!current || !placed) return;
    const v = current.verdict;
    setBusy(true);
    try {
      await harnfitAdd(profile, v.candidate.key);
      // The fit saves only the proposed zone (v1): a zone the user dragged it to is applied as
      // a pinned placement right after the add.
      if (zone && proposed && !sameZone(zone, proposed))
        await harnmapEdit(profile, {
          type: "place",
          artifact: v.candidate.key.replace(/^lib:/, ""),
          zone,
          note: v.why,
        });
      const text = `${v.candidate.label} added to ${name} and placed on the map, pinned.`;
      announce(text);
      navigate(mapPath, { state: { toast: text } });
    } catch (err) {
      if (!alive.current) return;
      setBusy(false);
      const message = String(err);
      const kind = fitProblem(message);
      if (kind === "importFirst")
        say("Import it first: it isn’t in your library yet.");
      else if (kind) setProblem({ kind, message });
      else say(`Could not add it: ${message}`);
    }
  }

  function discard() {
    const text = "Discarded. The fit stays in Saved fits.";
    announce(text);
    navigate(state?.from ?? mapPath, { state: { toast: text } });
  }

  const dnd: Dnd | undefined = current
    ? {
        active: dragging,
        onStart: () => setDragging(true),
        onEnd: () => setDragging(false),
        onDrop: (z) => {
          setDragging(false);
          setZone(z);
          announce(`Moved to ${zoneName(view.doc, z)}.`);
        },
      }
    : undefined;

  const data = useMemo(() => toMapData(view), [view]);
  const viewState: MapViewState = {
    ...mapView,
    hidden: view.doc.hidden,
    hiddenKinds: view.drawn ? view.doc.hiddenTypes : mapView.hiddenKinds,
  };
  const label =
    current?.verdict.candidate.label ?? state?.name ?? "this candidate";
  const waiting = since !== null;
  const stale = current?.stale === true && !kept && !waiting;
  const back = () => navigate(state?.from ?? mapPath);

  return (
    <div className={styles.screen}>
      <header className={styles.head}>
        <nav aria-label="Breadcrumb" className={styles.crumb}>
          <Link to={`/profiles/${profile}`}>{name}</Link>
          <span aria-hidden="true">→</span>
          <Link to={mapPath}>Harnmap</Link>
          <span aria-hidden="true">→</span>
          <b>Harnfit</b>
        </nav>
        {current && (
          <>
            <b>{label}</b>
            <Badge tone={verdictTone(current.verdict.verdict)}>
              {verdictLabel(current.verdict.verdict)}
            </Badge>
            <span className={styles.note}>
              Fit saved {fitDate(current.verdict.createdAt)} ·{" "}
              {modelLabel(current.verdict.model)} ·{" "}
              {Math.round(current.verdict.durationMs / 1000)} s
            </span>
          </>
        )}
        <span className={styles.grow} />
        {current && (
          <Button small disabled={waiting || !run} onClick={() => void ask()}>
            Run again
          </Button>
        )}
        {current && !run && (
          <span className={styles.note}>
            Run again needs the import preview, which is gone.
          </span>
        )}
        <Link to={mapPath} className={styles.close} aria-label="Close Harnfit">
          <X size={16} aria-hidden="true" />
        </Link>
      </header>
      {stale && current && (
        <StaleBanner
          summary={changeSummary(current.changes, nameOf)}
          canRun={run !== null}
          onRun={() => void ask()}
          onKeep={() => setKept(true)}
        />
      )}
      <div className={styles.body}>
        <div className={styles.map}>
          <div className={styles.mapBody}>
            {view.drawn && (
              <MapCanvas
                data={data}
                selected={null}
                view={viewState}
                onSelect={() => undefined}
                onView={setMapView}
                dnd={dnd}
                faded
                ghost={
                  current && placed
                    ? {
                        name: label,
                        kind: current.verdict.candidate.kind,
                        zone: placed,
                        conflicts: current.verdict.conflictsWith,
                      }
                    : undefined
                }
              />
            )}
          </div>
          {toast && (
            <div className={styles.toast} role="status">
              {toast}
            </div>
          )}
        </div>
        <aside className={styles.side} aria-label="Harnfit panel">
          {!view.drawn && (
            <NeedsMapCard
              candidate={label}
              profile={name}
              onDraw={() => navigate(mapPath)}
            />
          )}
          {waiting && (
            <div className={styles.sect}>
              <div className={styles.waitRow}>
                <div>
                  <div className={styles.eyebrow}>
                    CLAUDE IS WORKING · {modelLabel(base.model).toUpperCase()}
                  </div>
                  <div
                    className={styles.clock}
                    role="timer"
                    aria-label="Time elapsed"
                  >
                    {clock(now - (since ?? now))}
                  </div>
                </div>
                <Button
                  small
                  variant="ghost"
                  onClick={() => void harnfitCancel(profile)}
                >
                  Cancel
                </Button>
              </div>
              <p className={styles.p}>
                Claude is reading {label} and {name}’s map. This usually takes
                30–90 s.{" "}
                {current
                  ? "The saved verdict stays until the new one lands."
                  : "Nothing is saved until the answer passes Harneo’s checks."}
              </p>
            </div>
          )}
          {failure && (
            <FitFailureCard
              failure={failure}
              canRun={run !== null}
              onRetry={() => void ask()}
              onGuide={() =>
                void openExternal("claudeInstall").catch(() => undefined)
              }
            />
          )}
          {problem?.kind === "noMap" && (
            <NeedsMapCard
              candidate={label}
              profile={name}
              onDraw={() => navigate(mapPath)}
            />
          )}
          {problem?.kind === "plugin" && <PluginCard onBack={back} />}
          {problem?.kind === "alreadyIn" && (
            <AlreadyInCard message={problem.message} onBack={back} />
          )}
          {problem?.kind === "notFound" && <NoFitCard onBack={back} />}
          {current && (
            <VerdictCard
              fit={current}
              zone={placed ? zoneName(view.doc, placed) : ""}
              profile={name}
              nameOf={nameOf}
              busy={busy || waiting}
              onAdd={() => void add()}
              onElsewhere={() =>
                say(
                  "Drag the dashed node to another stage, the toolbelt or the floor.",
                )
              }
              onDiscard={discard}
              onImportServer={() => navigate("/import")}
            />
          )}
          <div className={styles.sect}>
            <div className={styles.labelRow}>
              <h2 className={styles.cardTitle}>Saved fits for {name}</h2>
              <span className={styles.note}>{fits.length}</span>
            </div>
            {fits.length === 0 ? (
              <p className={styles.muted}>No saved fits yet.</p>
            ) : (
              <ul className={fitStyles.fits} aria-label="Saved fits">
                {fits.map((f) => (
                  <li key={f.candidate.key}>
                    <button
                      type="button"
                      className={fitStyles.fitRow}
                      aria-current={f.candidate.key === candidate}
                      onClick={() =>
                        navigate(harnfitPath(profile, f.candidate.key), {
                          state: { from: state?.from },
                        })
                      }
                    >
                      <span
                        className={fitStyles.fitName}
                        title={f.candidate.label}
                      >
                        {f.candidate.label}
                      </span>
                      <Badge tone={verdictTone(f.verdict)}>
                        {verdictLabel(f.verdict)}
                      </Badge>
                      <span className={fitStyles.fitMeta}>
                        {fitDate(f.createdAt)}
                        {f.stale && " · out of date"}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <p className={styles.muted}>
              Every fit is saved with the profile. Reopening one costs no
              tokens; it shows out of date when the candidate or {name} changes.
            </p>
          </div>
          <div className={styles.sect}>
            <h2 className={styles.eyebrow}>THIS IS CLAUDE’S OPINION</h2>
            <p className={styles.p}>
              Harneo never blocks an add. A conflict is a warning with the lines
              that clash, so you can judge for yourself.
            </p>
          </div>
        </aside>
      </div>
    </div>
  );
}
