import { Check, FileQuestion, History, ListTree, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useParams } from "react-router";
import type { DrawFailure } from "../../bindings/DrawFailure";
import type { DrawOutcome } from "../../bindings/DrawOutcome";
import type { DrawSummary } from "../../bindings/DrawSummary";
import type { MapDoc } from "../../bindings/MapDoc";
import type { MapEdit } from "../../bindings/MapEdit";
import type { MapView } from "../../bindings/MapView";
import type { Template } from "../../bindings/Template";
import type { Zone } from "../../bindings/Zone";
import {
  claudeCliStatus,
  harnmapCancel,
  harnmapDraw,
  harnmapDrawAt,
  harnmapEdit,
  harnmapGet,
  harnmapHistory,
  harnmapRestore,
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
import { HistoryPanel } from "./HistoryPanel";
import { ListView } from "./ListView";
import { MapCanvas } from "./MapCanvas";
import { NodeDetail } from "./NodeDetail";
import { nextOrder, sameZone, zoneName, zonePhrase } from "./place";
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
import { StagesPopover } from "./StagesPopover";
import { DEFAULT_VIEW, type Dnd, type MapViewState } from "./types";
import { OnTheMap, UnplacedPanel } from "./UnplacedPanel";
import { WhyHere } from "./WhyHere";

/** How long a toast stays. */
const TOAST_MS = 5000;

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
  const [list, setList] = useState(false);
  const [stagesOpen, setStagesOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [history, setHistory] = useState<DrawSummary[] | null>(null);
  const [preview, setPreview] = useState<{ no: number; doc: MapDoc } | null>(
    null,
  );
  const [dragging, setDragging] = useState<string | null>(null);
  const [placing, setPlacing] = useState<{
    artifact: string;
    zone: Zone;
  } | null>(null);
  // Harnfit's Add lands here with its toast in the route state.
  const arrived = (useLocation().state as { toast?: string } | null)?.toast;
  const [toast, setToast] = useState<string | null>(arrived ?? null);
  const [edited, setEdited] = useState(false);
  const [focusFilter, setFocusFilter] = useState(0);
  const filter = useRef<HTMLInputElement>(null);
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

  // A toast leaves by itself.
  useEffect(() => {
    if (toast === null) return;
    const timer = setTimeout(() => setToast(null), TOAST_MS);
    return () => clearTimeout(timer);
  }, [toast]);

  // The history follows the draws while its panel is open.
  const drawCount = view.drawCount;
  useEffect(() => {
    if (!historyOpen) return;
    let live = true;
    harnmapHistory(profile).then(
      (draws) => live && setHistory(draws),
      () => live && setHistory([]),
    );
    return () => {
      live = false;
    };
  }, [historyOpen, profile, drawCount]);

  // "Place by hand" puts the cursor in the Unplaced filter once the panel is there.
  useEffect(() => {
    if (focusFilter > 0) filter.current?.focus();
  }, [focusFilter]);

  const say = useCallback((text: string) => {
    setToast(text);
    announce(text);
  }, []);

  /** Sends one edit; the map is saved at once and the view follows what came back. */
  async function edit(change: MapEdit, message?: string): Promise<boolean> {
    try {
      setView(await harnmapEdit(profile, change));
      setEdited(true);
      if (message) say(message);
      return true;
    } catch (err) {
      say(`Could not save that change: ${String(err)}`);
      return false;
    }
  }

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
  const shownDoc = preview?.doc ?? doc;
  const data = { ...toMapData(current), doc: shownDoc };
  // Hidden artifacts and types are part of the saved map once it is drawn.
  const viewState: MapViewState = {
    ...mapView,
    hidden: shownDoc.hidden,
    hiddenKinds: view.drawn ? shownDoc.hiddenTypes : mapView.hiddenKinds,
  };

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

  const editable = view.drawn && !busy && preview === null;
  const placement = view.doc.placements.find((p) => p.artifact === selected);
  const detail = selected !== null && editable && placement !== undefined;

  function changeView(next: MapViewState) {
    setMapView(next);
    if (!editable) return;
    const was = view.doc.hiddenTypes;
    void (async () => {
      for (const kind of next.hiddenKinds.filter((k) => !was.includes(k)))
        await edit({ type: "hideType", kind });
      for (const kind of was.filter((k) => !next.hiddenKinds.includes(k)))
        await edit({ type: "showType", kind });
    })();
  }

  function drop(zone: Zone) {
    const artifact = dragging;
    setDragging(null);
    if (!artifact) return;
    const there = view.doc.placements.find((p) => p.artifact === artifact);
    if (!there) {
      setPlacing({ artifact, zone });
      return;
    }
    if (sameZone(there.zone, zone)) return;
    void move(artifact, zone);
  }

  function move(artifact: string, zone: Zone) {
    const parent = view.doc.placements.find(
      (p) => p.artifact === artifact,
    )?.parent;
    return edit(
      { type: "move", artifact, zone, order: nextOrder(view.doc, zone) },
      `${nameOf(artifact)}${parent ? ` detached from ${nameOf(parent)},` : ""} moved to ${zoneName(view.doc, zone)} and pinned. It stays on re-draw.`,
    );
  }

  async function place(zone: Zone, note: string) {
    if (!placing) return;
    const artifact = placing.artifact;
    const ok = await edit(
      { type: "place", artifact, zone, note },
      `${nameOf(artifact)} placed ${zonePhrase(view.doc, zone)} and pinned. It stays on re-draw.`,
    );
    if (ok) setPlacing(null);
  }

  const dnd: Dnd | undefined = editable
    ? {
        active: dragging !== null,
        onStart: setDragging,
        onEnd: () => setDragging(null),
        onDrop: drop,
      }
    : undefined;

  function openHistory() {
    setHistoryOpen(true);
    setSelected(null);
    setStagesOpen(false);
  }

  function closeHistory() {
    setHistoryOpen(false);
    setPreview(null);
  }

  async function previewDraw(no: number) {
    try {
      setPreview({ no, doc: await harnmapDrawAt(profile, no) });
      setSelected(null);
    } catch (err) {
      say(`Could not open draw ${no}: ${String(err)}`);
    }
  }

  async function restore(no: number) {
    try {
      const created = await harnmapRestore(profile, no);
      setView(await harnmapGet(profile));
      setPreview(null);
      setEdited(true);
      say(`Draw ${no} restored as draw ${created}. Nothing was overwritten.`);
    } catch (err) {
      say(`Could not restore draw ${no}: ${String(err)}`);
    }
  }

  function placeByHand() {
    setHistoryOpen(false);
    setPreview(null);
    setSelected(null);
    setList(false);
    setFocusFilter((n) => n + 1);
  }

  function pick(artifact: string) {
    setList(false);
    setSelected(artifact);
  }

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
          <span className={styles.saved} role="status">
            <Check size={12} aria-hidden="true" />{" "}
            {edited ? "Saved just now" : "Saved"}
          </span>
        )}
        <span className={styles.grow} />
        <div className={styles.seg} role="group" aria-label="View">
          <button
            type="button"
            aria-pressed={!list}
            onClick={() => setList(false)}
          >
            Map
          </button>
          <button
            type="button"
            aria-pressed={list}
            onClick={() => setList(true)}
          >
            List
          </button>
        </div>
        {view.drawn && (
          <>
            <Button
              small
              aria-expanded={stagesOpen}
              disabled={busy || preview !== null}
              onClick={() => setStagesOpen(!stagesOpen)}
            >
              <ListTree size={14} aria-hidden="true" /> Stages
            </Button>
            <Button
              small
              aria-expanded={historyOpen}
              disabled={busy}
              onClick={() => (historyOpen ? closeHistory() : openHistory())}
            >
              <History size={14} aria-hidden="true" /> Map history
            </Button>
            <Button small disabled={busy} onClick={() => setRedraw(true)}>
              Re-draw
            </Button>
          </>
        )}
        <Link to={base} className={styles.close} aria-label="Close">
          <X size={16} aria-hidden="true" />
        </Link>
      </header>
      <div className={styles.body}>
        <div className={styles.map}>
          {preview && (
            <div className={styles.banner} role="status">
              <b>Previewing draw {preview.no}</b>
              <Button small onClick={() => void restore(preview.no)}>
                Restore
              </Button>
              <Button small variant="ghost" onClick={() => setPreview(null)}>
                Back to current
              </Button>
            </div>
          )}
          <div className={styles.mapBody}>
            {list ? (
              <ListView view={current} profile={name} onPick={pick} />
            ) : (
              <MapCanvas
                data={data}
                selected={selected}
                view={viewState}
                onSelect={setSelected}
                onView={changeView}
                dnd={dnd}
              />
            )}
          </div>
          {stagesOpen && view.drawn && (
            <StagesPopover
              stages={view.doc.stages}
              onRename={(id, next) =>
                void edit({ type: "renameStage", id, name: next })
              }
              onRemove={(id) =>
                void edit(
                  { type: "removeStage", id },
                  `${stageName(id)} removed. Its stations went back to Unplaced.`,
                )
              }
              onMove={(id, to) => void edit({ type: "moveStage", id, to })}
              onAdd={() =>
                void edit({
                  type: "addStage",
                  after: view.doc.stages.at(-1)?.id ?? null,
                  name: "New stage",
                })
              }
              onClose={() => setStagesOpen(false)}
            />
          )}
          {placing && (
            <WhyHere
              key={placing.artifact}
              name={nameOf(placing.artifact)}
              doc={view.doc}
              zone={placing.zone}
              onPlace={(zone, note) => void place(zone, note)}
              onCancel={() => setPlacing(null)}
            />
          )}
          {toast && (
            <div className={styles.toast} role="status">
              <Check size={12} aria-hidden="true" /> {toast}
            </div>
          )}
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
          {!busy && !done && view.drawn && historyOpen && (
            <HistoryPanel
              draws={history}
              previewing={preview?.no ?? null}
              busy={busy}
              onPreview={(no) => void previewDraw(no)}
              onRestore={(no) => void restore(no)}
              onClose={closeHistory}
            />
          )}
          {!busy && !done && view.drawn && !historyOpen && detail && (
            <NodeDetail
              key={selected}
              id={selected}
              info={
                current.artifacts[selected] ?? { name: selected, kind: "skill" }
              }
              placement={placement}
              facts={current.facts[selected] ?? []}
              doc={view.doc}
              parentName={
                placement.parent ? nameOf(placement.parent) : undefined
              }
              alsoRunBy={(placement.alsoRunBy ?? []).map(nameOf)}
              onDetach={() =>
                void edit(
                  { type: "detach", artifact: selected },
                  `${nameOf(selected)} detached from ${placement.parent ? nameOf(placement.parent) : "its parent"} and pinned. It stays on re-draw.`,
                )
              }
              onClose={() => setSelected(null)}
              onNote={(text) =>
                void edit({ type: "note", artifact: selected, text })
              }
              onMove={(zone) => void move(selected, zone)}
              onRemove={() => {
                setSelected(null);
                void edit(
                  { type: "remove", artifact: selected },
                  `${nameOf(selected)} went back to Unplaced`,
                );
              }}
              onHide={() => {
                setSelected(null);
                void edit(
                  { type: "hide", artifact: selected },
                  `${nameOf(selected)} hidden. It is still in the profile.`,
                );
              }}
              onUnpin={() =>
                void edit(
                  { type: "unpin", artifact: selected },
                  `${nameOf(selected)} unpinned. Claude places it on the next re-draw.`,
                )
              }
            />
          )}
          {!busy && !done && view.drawn && !historyOpen && !detail && (
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
                  onPlaceByHand={placeByHand}
                />
              )}
              {!onLine && <NothingOnLineCard addLink={base} />}
              <UnplacedPanel
                ref={filter}
                unplaced={view.unplaced}
                artifacts={view.artifacts}
                hidden={view.doc.hidden}
                disabled={preview !== null}
                onPick={(artifact) =>
                  setPlacing({
                    artifact,
                    zone: view.doc.stages[0]
                      ? { type: "stage", id: view.doc.stages[0].id }
                      : { type: "toolbelt" },
                  })
                }
                onDragStart={setDragging}
                onDragEnd={() => setDragging(null)}
                onShow={(artifact) => void edit({ type: "show", artifact })}
                onShowAll={() =>
                  void (async () => {
                    for (const artifact of view.doc.hidden)
                      await edit({ type: "show", artifact });
                  })()
                }
              />
              <OnTheMap
                doc={view.doc}
                bundles={view.bundles.length}
                nameOf={nameOf}
              />
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
