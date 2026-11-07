import { Check, FileQuestion, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router";
import type { DrawFailure } from "../../bindings/DrawFailure";
import type { DrawOutcome } from "../../bindings/DrawOutcome";
import type { MapView } from "../../bindings/MapView";
import type { Template } from "../../bindings/Template";
import {
  claudeCliStatus,
  harnmapCancel,
  harnmapDraw,
  harnmapGet,
  openExternal,
  profileGet,
  settingsGet,
  settingsSet,
  templatesList,
} from "../../lib/ipc";
import { announce, Badge, Button, EmptyState } from "../../ui";
import {
  asFailure,
  changeLines,
  MODEL_KEY,
  modelLabel,
  replayGap,
  REPLAY_HOLD_MS,
  toMapData,
  toModel,
  took,
  type DrawModel,
} from "./draw";
import styles from "./Harnmap.module.css";
import { MapCanvas } from "./MapCanvas";
import { RedrawDialog } from "./RedrawDialog";
import {
  FirstDrawPanel,
  ResultPanel,
  Changes,
  WaitingPanel,
} from "./SidePanel";
import {
  CategoryChangedCard,
  FailureCard,
  NothingOnLineCard,
  StaleCard,
} from "./states";
import { DEFAULT_VIEW, type MapViewState } from "./types";

type Load =
  | { status: "loading" }
  | { status: "error"; message: string }
  | {
      status: "ready";
      name: string;
      view: MapView;
      templates: Template[];
      model: DrawModel;
    };

interface Replay {
  view: MapView;
  outcome: DrawOutcome;
  shown: number;
}

/** True when the user asked the system for no animation. */
function reducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

function when(createdAt: number): string {
  return new Date(createdAt * 1000).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
  });
}

/**
 * The Harnmap screen: the map canvas with the draw flow around it. Before the first draw the
 * template skeleton and a description box; while Claude works an honest wait; then a replay of
 * the placements and the computed changes. A failure leaves the current map as it was.
 */
export function HarnmapScreen() {
  const { id } = useParams();
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const profile = useMemo(() => {
    try {
      return id ? BigInt(id) : null;
    } catch {
      return null;
    }
  }, [id]);

  useEffect(() => {
    if (profile === null) return;
    let live = true;
    Promise.all([
      profileGet(profile),
      harnmapGet(profile),
      templatesList(),
      settingsGet(MODEL_KEY),
    ]).then(
      ([p, view, templates, model]) => {
        if (live)
          setLoad({
            status: "ready",
            name: p.name,
            view,
            templates,
            model: toModel(model),
          });
      },
      (err: unknown) =>
        live && setLoad({ status: "error", message: String(err) }),
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
          Loading the map…
        </p>
      </div>
    );
  return (
    <Loaded
      profile={profile}
      name={load.name}
      initial={load.view}
      templates={load.templates}
      initialModel={load.model}
    />
  );
}

function Loaded({
  profile,
  name,
  initial,
  templates,
  initialModel,
}: {
  profile: bigint;
  name: string;
  initial: MapView;
  templates: Template[];
  initialModel: DrawModel;
}) {
  const [view, setView] = useState(initial);
  const [model, setModel] = useState(initialModel);
  const [description, setDescription] = useState(initial.description);
  const [mapView, setMapView] = useState<MapViewState>(DEFAULT_VIEW);
  const [selected, setSelected] = useState<string | null>(null);
  const [since, setSince] = useState<number | null>(null);
  const [now, setNow] = useState(0);
  const [replay, setReplay] = useState<Replay | null>(null);
  const [done, setDone] = useState<DrawOutcome | null>(null);
  const [failure, setFailure] = useState<Exclude<
    DrawFailure,
    { type: "cancelled" }
  > | null>(null);
  const [failedAfter, setFailedAfter] = useState(0);
  const [redraw, setRedraw] = useState(false);
  const [keepOld, setKeepOld] = useState(false);
  const drawing = since !== null;
  const busy = drawing || replay !== null;
  const base = `/profiles/${profile}`;

  // The clock of the honest wait.
  useEffect(() => {
    if (since === null) return;
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(tick);
  }, [since]);

  // The replay: one more artifact every ~0.11 s, then a short hold, then the result.
  useEffect(() => {
    if (!replay) return;
    const total = replay.view.doc.placements.length;
    const finished = replay.shown >= total;
    const timer = setTimeout(
      () => {
        if (!finished) {
          setReplay({ ...replay, shown: replay.shown + 1 });
          return;
        }
        setView(replay.view);
        setDone(replay.outcome);
        setReplay(null);
        announce(`Draw ${replay.outcome.no} saved`);
      },
      finished ? (reducedMotion() ? 0 : REPLAY_HOLD_MS) : replayGap(total),
    );
    return () => clearTimeout(timer);
  }, [replay]);

  const run = useCallback(
    async (text: string) => {
      setFailure(null);
      setDone(null);
      setRedraw(false);
      setKeepOld(false);
      const started = Date.now();
      setSince(started);
      setNow(started);
      try {
        const outcome = await harnmapDraw(profile, text);
        const next = await harnmapGet(profile);
        setSince(null);
        setReplay({
          view: next,
          outcome,
          shown: reducedMotion() ? next.doc.placements.length : 0,
        });
      } catch (err) {
        setSince(null);
        const failed = asFailure(err);
        if (failed.type === "cancelled") {
          announce("Drawing cancelled. Nothing was saved.");
        } else {
          setFailedAfter(Date.now() - started);
          setFailure(failed);
        }
      }
    },
    [profile],
  );

  function submit(text: string) {
    setDescription(text);
    void run(text);
  }

  async function switchModel(next: DrawModel) {
    // Simplest honest retry: store the other model as the setting, then draw again.
    try {
      await settingsSet(MODEL_KEY, next);
    } catch (err) {
      setFailure({ type: "other", message: String(err) });
      return;
    }
    setModel(next);
    void run(description);
  }

  async function check() {
    try {
      const status = await claudeCliStatus();
      if (status?.found) {
        setFailure(null);
        announce("The claude CLI is there. You can draw now.");
      } else announce("The claude CLI is still not found.");
    } catch {
      announce("The claude CLI is still not found.");
    }
  }

  function guide() {
    openExternal("claudeInstall").catch((err: unknown) =>
      setFailure({
        type: "other",
        message: `Could not open the guide: ${String(err)}`,
      }),
    );
  }

  const shownView = replay ?? null;
  const current = shownView ? shownView.view : view;
  const doc = shownView
    ? {
        ...shownView.view.doc,
        placements: shownView.view.doc.placements.slice(0, shownView.shown),
      }
    : current.doc;
  const data = { ...toMapData(current), doc };

  const nameOf = (artifact: string) =>
    current.artifacts[artifact]?.name ?? artifact;
  const template = templates.find((t) => t.id === current.doc.category);
  const label = (cat: string) =>
    templates.find((t) => t.id === cat)?.label ?? cat;
  const count = Object.keys(current.artifacts).length;
  const stageName = (stage: string) =>
    current.doc.stages.find((s) => s.id === stage)?.name ?? stage;
  const pinned = current.doc.placements
    .filter((p) => p.pinned)
    .map((p) => nameOf(p.artifact));
  const onLine = current.doc.placements.some((p) => p.zone.type === "stage");
  const lines = (v: MapView) =>
    changeLines(
      v.doc.changes,
      stageName,
      v.unplaced.map((a) => v.artifacts[a]?.name ?? a),
    );

  const actions = {
    onRetry: () => void run(description),
    onSwitchModel: (next: DrawModel) => void switchModel(next),
    onCheck: () => void check(),
    onGuide: guide,
    onDismiss: () => setFailure(null),
  };

  const stale = view.stale;
  const categoryChanged = stale?.categoryChanged ?? null;

  return (
    <div className={styles.screen}>
      <header className={styles.head}>
        <nav aria-label="Breadcrumb" className={styles.crumb}>
          <Link to={base}>{name}</Link>
          <span aria-hidden="true">→</span>
          <b>Harnmap</b>
        </nav>
        <Badge>{template?.label ?? current.doc.category}</Badge>
        {view.drawn && (
          <span className={styles.note}>
            Draw {view.no} · {when(view.createdAt)} · {modelLabel(view.model)} ·{" "}
            {took(view.durationMs)}
          </span>
        )}
        {view.drawn && (
          <span className={styles.saved}>
            <Check size={12} aria-hidden="true" /> Saved
          </span>
        )}
        <span className={styles.grow} />
        {/* The list view comes later; Map is the only view for now. */}
        <div className={styles.seg} role="group" aria-label="View">
          <button type="button" aria-pressed="true">
            Map
          </button>
          <button type="button" aria-pressed="false" disabled>
            List
          </button>
        </div>
        {view.drawn && (
          <Button small disabled={busy} onClick={() => setRedraw(true)}>
            Re-draw
          </Button>
        )}
        <Link to={base} className={styles.close} aria-label="Close">
          <X size={16} aria-hidden="true" />
        </Link>
      </header>
      <div className={styles.body}>
        <div className={styles.map}>
          <MapCanvas
            data={data}
            selected={selected}
            view={mapView}
            onSelect={setSelected}
            onView={setMapView}
          />
        </div>
        <aside className={styles.side} aria-label="Draw">
          {failure && (
            <FailureCard
              failure={failure}
              model={model}
              drawn={view.drawn}
              elapsedMs={failedAfter}
              actions={actions}
              addLink={base}
            />
          )}
          {drawing && (
            <WaitingPanel
              elapsedMs={now - (since ?? now)}
              model={model}
              onCancel={() => void harnmapCancel(profile)}
            />
          )}
          {replay && (
            <div className={styles.sect}>
              <div className={styles.eyebrow}>PLACING</div>
              <p className={styles.p} role="status">
                {replay.shown} of {replay.view.doc.placements.length} placed
              </p>
            </div>
          )}
          {!busy && done && (
            <ResultPanel
              no={done.no}
              model={done.model}
              tookText={took(done.durationMs)}
              placed={view.doc.placements.length}
              total={Object.keys(view.artifacts).length}
              lines={lines(view)}
              onOpen={() => setDone(null)}
            />
          )}
          {!busy && !done && !view.drawn && (
            <FirstDrawPanel
              description={description}
              onDescription={setDescription}
              template={template}
              count={count}
              model={model}
              canDraw={count > 0}
              onDraw={() => void run(description)}
              addLink={base}
            />
          )}
          {!busy && !done && view.drawn && (
            <>
              {categoryChanged && !keepOld && (
                <CategoryChangedCard
                  change={categoryChanged}
                  label={label}
                  no={view.no}
                  onRedraw={() => setRedraw(true)}
                  onKeep={() => setKeepOld(true)}
                />
              )}
              {stale && !categoryChanged && (
                <StaleCard
                  stale={stale}
                  profile={name}
                  no={view.no}
                  nameOf={nameOf}
                  onRedraw={() => setRedraw(true)}
                />
              )}
              {!onLine && <NothingOnLineCard addLink={base} />}
              <div className={styles.sect}>
                <div className={styles.eyebrow}>
                  DRAW {view.no} · {view.drawCount} IN HISTORY
                </div>
                <p className={styles.p}>
                  {view.doc.placements.length} of {count} artifacts placed.
                </p>
                <Changes lines={lines(view)} />
              </div>
            </>
          )}
        </aside>
      </div>
      {redraw && (
        <RedrawDialog
          initial={description}
          pinned={pinned}
          count={count}
          onDraw={submit}
          onClose={() => setRedraw(false)}
        />
      )}
    </div>
  );
}
