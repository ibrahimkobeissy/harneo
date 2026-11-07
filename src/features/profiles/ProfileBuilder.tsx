import { ArrowLeft, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";
import type { CategoryId } from "../../bindings/CategoryId";
import type { Layout } from "../../bindings/Layout";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import type { Resolved } from "../../bindings/Resolved";
import {
  bundleOf,
  libraryDetail,
  profileCreate,
  profileGet,
  profileLayout,
  profileRename,
  profileResolve,
  profileSetCategory,
  profileSave,
  profilesList,
} from "../../lib/ipc";
import { Button, EmptyState, Screen } from "../../ui";
import styles from "./Builder.module.css";
import { Cart } from "./Cart";
import { CategoryConfirm, CategoryMenu, NewProfileDialog } from "./Category";
import { labelOf, useTemplates } from "./category";
import { RemoveDialog, type RemoveAsk } from "./RemoveDialog";
import { dropDraft, draftFor, keepDraft } from "./drafts";
import { LeaveGuard } from "./LeaveGuard";
import { Picker } from "./Picker";
import {
  addItem,
  defaultName,
  dropItems,
  emptySpec,
  isEmpty,
  setManualOnly,
  specKey,
} from "./spec";

interface Saved {
  /** `null` until a new profile's first save. */
  id: bigint | null;
  name: string;
  spec: ProfileSpec;
  version: number;
  category: CategoryId;
  categoryNote: string | null;
}

type Load =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; saved: Saved; projectCount: number };

export interface Notice {
  text: string;
  tone: "ok" | "warn" | "bad";
  /** An ok notice that stays until dismissed (warnings and errors always do). */
  sticky?: boolean;
}

/** Router state a screen can hand to the builder it navigates to. */
export interface BuilderState {
  notice?: Notice;
  /** A new profile's name and category are already chosen: skip the New profile dialog. */
  chosen?: boolean;
}

/** Trust scores under this line get a warning in the cart. */
const LOW_TRUST = 60;

const LIVE_DELAY_MS = 150;
const NOTICE_MS = 6000;
const notFound =
  "This profile is no longer in your library. It may have been removed; go back to Profiles to pick another.";
const noneResolved: Resolved = { artifacts: [], missing: [], worstTrust: null };
const noneLayout: Layout = { paths: [], conflicts: [] };

const back = (
  <Link to="/profiles" className={styles.back}>
    <ArrowLeft size={14} aria-hidden="true" />
    Profiles
  </Link>
);

/** `/profiles/:id` and `/profiles/new`: the picker, the cart and the project it produces. */
export function ProfileBuilder() {
  const { id = "new" } = useParams();
  return <Loader key={id} id={id} />;
}

function Loader({ id }: { id: string }) {
  const [load, setLoad] = useState<Load>({ status: "loading" });

  useEffect(() => {
    let stale = false;
    (async () => {
      if (id === "new") {
        return {
          status: "ready",
          saved: {
            id: null,
            name: "",
            spec: emptySpec,
            version: 0,
            category: "software",
            categoryNote: null,
          },
          projectCount: 0,
        } as const;
      }
      const pid = BigInt(id);
      const view = await profileGet(pid);
      const projectCount = await profilesList()
        .then(
          (all) =>
            all.find((p) => Number(p.id) === Number(pid))?.projectCount ?? 0,
        )
        .catch(() => 0);
      return {
        status: "ready",
        saved: {
          id: pid,
          name: view.name,
          spec: view.spec,
          version: view.currentVersion,
          category: view.category,
          categoryNote: view.categoryNote,
        },
        projectCount,
      } as const;
    })().then(
      (next) => {
        if (!stale) setLoad(next);
      },
      () => {
        if (!stale) setLoad({ status: "error", message: notFound });
      },
    );
    return () => {
      stale = true;
    };
  }, [id]);

  if (load.status !== "ready") {
    return (
      <Screen title="Profile" crumb={back}>
        {load.status === "error" ? (
          <EmptyState icon={TriangleAlert} title="Profile not found">
            {load.message}
          </EmptyState>
        ) : (
          <p className={styles.quiet}>Loading…</p>
        )}
      </Screen>
    );
  }
  return (
    <Builder
      routeId={id}
      initial={load.saved}
      projectCount={load.projectCount}
    />
  );
}

/** The audit score of each picked artifact that falls under the warning line. */
function useLowTrust(resolved: Resolved | null): Record<string, number> {
  const [scores, setScores] = useState<Record<string, number | null>>({});
  useEffect(() => {
    const ids = (resolved?.artifacts ?? [])
      .filter((a) => a.kind !== "settings" && a.kind !== "mcp")
      .map((a) => a.id)
      .filter((id) => !(id in scores));
    if (ids.length === 0) return;
    let stale = false;
    Promise.all(
      ids.map((id) =>
        libraryDetail(id)
          .then((d) => [id, d.audit?.score ?? null] as const)
          .catch(() => [id, null] as const),
      ),
    ).then((pairs) => {
      if (!stale)
        setScores((prev) => ({ ...prev, ...Object.fromEntries(pairs) }));
    });
    return () => {
      stale = true;
    };
  }, [resolved, scores]);
  const low: Record<string, number> = {};
  for (const [id, score] of Object.entries(scores)) {
    if (score !== null && score < LOW_TRUST) low[id] = score;
  }
  return low;
}

function Builder({
  routeId,
  initial,
  projectCount,
}: {
  routeId: string;
  initial: Saved;
  projectCount: number;
}) {
  const navigate = useNavigate();
  const [saved, setSaved] = useState(initial);
  const kept = draftFor(routeId);
  const [name, setName] = useState(kept?.name ?? initial.name);
  const [spec, setSpec] = useState(kept?.spec ?? initial.spec);
  const [category, setCategory] = useState(kept?.category ?? initial.category);
  const [categoryNote, setCategoryNote] = useState(
    kept?.categoryNote ?? initial.categoryNote,
  );
  const [pending, setPending] = useState<CategoryId | null>(null);
  const templates = useTemplates();
  const [liveResolved, setResolved] = useState<Resolved | null>(null);
  const [liveLayout, setLayout] = useState<Layout | null>(null);
  const location = useLocation();
  const [notice, setNotice] = useState<Notice | null>(
    (location.state as BuilderState | null)?.notice ?? null,
  );
  const [naming, setNaming] = useState(
    initial.id === null &&
      !kept &&
      !(location.state as BuilderState | null)?.chosen,
  );
  const [ask, setAsk] = useState<RemoveAsk | null>(null);
  const [busy, setBusy] = useState(false);
  const bypass = useRef(false);

  const isNew = saved.id === null;
  const specChanged = specKey(spec) !== specKey(saved.spec);
  const nameChanged = name.trim() !== saved.name;
  const dirty = isNew
    ? !isEmpty(spec) || name.trim() !== ""
    : specChanged || nameChanged;

  // Unsaved edits survive leaving the screen.
  useEffect(() => {
    if (dirty) keepDraft(routeId, { name, spec, category, categoryNote });
    else dropDraft(routeId);
  }, [routeId, dirty, name, spec, category, categoryNote]);

  useEffect(() => {
    if (!dirty) return;
    const onUnload = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener("beforeunload", onUnload);
    return () => window.removeEventListener("beforeunload", onUnload);
  }, [dirty]);

  // The cart and the tree follow every change.
  useEffect(() => {
    if (isEmpty(spec)) return;
    let stale = false;
    const timer = setTimeout(() => {
      Promise.all([profileResolve(spec), profileLayout(spec)]).then(
        ([r, l]) => {
          if (stale) return;
          setResolved(r);
          setLayout(l);
        },
        (err: unknown) => {
          if (!stale) setNotice({ text: String(err), tone: "bad" });
        },
      );
    }, LIVE_DELAY_MS);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [spec]);

  // Nothing picked: nothing to resolve.
  const resolved = isEmpty(spec) ? noneResolved : liveResolved;
  const layout = isEmpty(spec) ? noneLayout : liveLayout;

  // Only a plain success toast leaves by itself; warnings, errors and the first save stay until dismissed.
  useEffect(() => {
    if (!notice || notice.tone !== "ok" || notice.sticky) return;
    const timer = setTimeout(() => setNotice(null), NOTICE_MS);
    return () => clearTimeout(timer);
  }, [notice]);

  const change = useCallback(
    (update: (s: ProfileSpec) => ProfileSpec) => setSpec(update),
    [],
  );

  const nameOf = useCallback(
    (itemId: string) =>
      resolved?.artifacts.find((a) => a.id === itemId)?.name ?? itemId,
    [resolved],
  );

  async function add(item: LibraryItem) {
    const have = new Set(resolved?.artifacts.map((a) => a.id));
    let pulled = 0;
    try {
      const members = await bundleOf(item.id);
      pulled = members.filter((m) => !have.has(m)).length;
    } catch {
      // The bundle is pulled in by the core either way; only the toast loses its count.
    }
    change((s) => addItem(s, item.id));
    if (
      item.trustScore !== null &&
      item.trustScore < LOW_TRUST &&
      item.findingCount > 0
    ) {
      setNotice({
        text: `Added ${item.name}. Trust ${item.trustScore}: ${item.findingCount} security ${item.findingCount === 1 ? "finding" : "findings"}.`,
        tone: "warn",
      });
    } else {
      setNotice({
        text: `Added ${item.name}${pulled > 0 ? ` + ${pulled} bundled` : ""}`,
        tone: "ok",
      });
    }
  }

  /** Removes one artifact, asking first when a bundle is involved. */
  async function remove(itemId: string) {
    try {
      const bundles = new Map<string, string[]>();
      await Promise.all(
        spec.items.map(async (it) => bundles.set(it, await bundleOf(it))),
      );
      const parents = spec.items.filter(
        (it) => it !== itemId && bundles.get(it)?.includes(itemId),
      );
      if (parents.length > 0) {
        setAsk({ kind: "member", id: itemId, parents });
        return;
      }
      const members = (bundles.get(itemId) ?? []).filter(
        (m) =>
          spec.items.includes(m) &&
          !spec.items.some(
            (other) =>
              other !== itemId &&
              other !== m &&
              bundles.get(other)?.includes(m),
          ),
      );
      if (members.length > 0) {
        setAsk({ kind: "lead", id: itemId, members });
        return;
      }
      change((s) => dropItems(s, [itemId]));
    } catch (err) {
      setNotice({ text: String(err), tone: "bad" });
    }
  }

  async function save() {
    if (spec.items.length === 0) {
      setNotice({
        text: "Add at least one artifact before saving.",
        tone: "bad",
      });
      return;
    }
    const finalName = name.trim() || defaultName;
    setBusy(true);
    try {
      if (saved.id === null) {
        const created = await profileCreate(
          finalName,
          "",
          spec,
          category,
          categoryNote,
        );
        bypass.current = true;
        dropDraft(routeId);
        const state: BuilderState = {
          notice: {
            text: `Saved ${finalName} v1`,
            tone: "ok",
            sticky: true,
          },
        };
        void navigate(`/profiles/${created}`, { replace: true, state });
        return;
      }
      if (finalName !== saved.name) await profileRename(saved.id, finalName);
      const version = specChanged
        ? await profileSave(saved.id, spec)
        : saved.version;
      setSaved({ ...saved, name: finalName, spec, version });
      setName(finalName);
      setNotice({ text: `Saved ${finalName} v${version}`, tone: "ok" });
    } catch (err) {
      setNotice({ text: `Could not save: ${String(err)}`, tone: "bad" });
    } finally {
      setBusy(false);
    }
  }

  /** Stores a saved profile's new category. Not a version; nothing is sent. */
  async function switchCategory(target: CategoryId, note: string | null) {
    if (saved.id === null) return;
    setBusy(true);
    try {
      await profileSetCategory(saved.id, target, note);
      setSaved({ ...saved, category: target, categoryNote: note });
      setCategory(target);
      setCategoryNote(note);
      setPending(null);
      setNotice({
        text: `Category is now ${labelOf(templates, target)}. Re-draw the map when you're ready.`,
        tone: "ok",
      });
    } catch (err) {
      setNotice({
        text: `Could not change the category: ${String(err)}`,
        tone: "bad",
      });
    } finally {
      setBusy(false);
    }
  }

  function pickCategory(target: CategoryId) {
    if (isNew) {
      setCategory(target);
      setCategoryNote(null);
      return;
    }
    setPending(target === saved.category ? null : target);
  }

  function discard() {
    setName(saved.name);
    setSpec(saved.spec);
    setNotice(null);
  }

  const lowTrust = useLowTrust(resolved);

  const picked = useMemo(
    () => new Set(resolved?.artifacts.map((a) => a.id)),
    [resolved],
  );

  return (
    <Screen
      title={saved.name || "New profile"}
      crumb={back}
      badge={
        dirty ? <span className={styles.unsaved}>Unsaved</span> : undefined
      }
      actions={
        saved.id !== null ? (
          <Link to={`/profiles/${saved.id}/harnmap`} className={styles.harnmap}>
            Harnmap
          </Link>
        ) : undefined
      }
    >
      {notice && (
        <div
          role="status"
          className={`${styles.notice} ${styles[notice.tone]}`}
        >
          <span className={styles.noticeText}>{notice.text}</span>
          <Button variant="ghost" small onClick={() => setNotice(null)}>
            Dismiss
          </Button>
        </div>
      )}
      <CategoryMenu
        templates={templates}
        current={isNew ? category : saved.category}
        onPick={pickCategory}
      />
      {pending && (
        <CategoryConfirm
          key={pending}
          profile={saved.name}
          currentLabel={labelOf(templates, saved.category)}
          targetLabel={labelOf(templates, pending)}
          custom={pending === "custom"}
          busy={busy}
          onKeep={() => setPending(null)}
          onSwitch={(note) => void switchCategory(pending, note)}
        />
      )}
      <div className={styles.builder}>
        <Picker picked={picked} onAdd={(i) => void add(i)} onRemove={remove} />
        <Cart
          name={name}
          onName={setName}
          saved={saved}
          profileId={saved.id}
          isNew={isNew}
          dirty={dirty}
          specChanged={specChanged}
          busy={busy}
          projectCount={projectCount}
          lowTrust={lowTrust}
          spec={spec}
          resolved={resolved}
          layout={layout}
          onSave={() => void save()}
          onDiscard={discard}
          onRemove={remove}
          onSpec={change}
          onManualOnly={(itemId, value) =>
            change((s) => setManualOnly(s, itemId, value))
          }
        />
      </div>
      {ask && (
        <RemoveDialog
          ask={ask}
          nameOf={nameOf}
          onClose={() => setAsk(null)}
          onConfirm={(ids) => {
            change((s) => dropItems(s, ids));
            setAsk(null);
          }}
        />
      )}
      {naming && (
        <NewProfileDialog
          templates={templates}
          initialName={name}
          onCancel={() => void navigate("/profiles")}
          onCreate={(choice) => {
            setName(choice.name);
            setCategory(choice.category);
            setCategoryNote(choice.note);
            setNaming(false);
          }}
        />
      )}
      <LeaveGuard when={dirty} bypass={bypass} />
    </Screen>
  );
}
