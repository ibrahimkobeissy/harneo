import { ArrowLeft, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import type { Layout } from "../../bindings/Layout";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import type { Resolved } from "../../bindings/Resolved";
import {
  bundleOf,
  profileCreate,
  profileGet,
  profileLayout,
  profileRename,
  profileResolve,
  profileSave,
  profilesList,
} from "../../lib/ipc";
import { Button, EmptyState, Screen } from "../../ui";
import styles from "./Builder.module.css";
import { Cart } from "./Cart";
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
}

type Load =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; saved: Saved; projectCount: number };

export interface Notice {
  text: string;
  tone: "ok" | "warn" | "bad";
}

const LIVE_DELAY_MS = 150;
const NOTICE_MS = 6000;
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
          saved: { id: null, name: "", spec: emptySpec, version: 0 },
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
        },
        projectCount,
      } as const;
    })().then(
      (next) => {
        if (!stale) setLoad(next);
      },
      (err: unknown) => {
        if (!stale) setLoad({ status: "error", message: String(err) });
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
  const [liveResolved, setResolved] = useState<Resolved | null>(null);
  const [liveLayout, setLayout] = useState<Layout | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
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
    if (dirty) keepDraft(routeId, { name, spec });
    else dropDraft(routeId);
  }, [routeId, dirty, name, spec]);

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

  // A success or warning toast leaves by itself; an error stays until dismissed.
  useEffect(() => {
    if (!notice || notice.tone === "bad") return;
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
      item.trustScore < 60 &&
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
        const created = await profileCreate(finalName, "", spec);
        bypass.current = true;
        dropDraft(routeId);
        void navigate(`/profiles/${created}`, { replace: true });
        return;
      }
      if (finalName !== saved.name) await profileRename(saved.id, finalName);
      const version = specChanged
        ? await profileSave(saved.id, spec)
        : saved.version;
      setSaved({ id: saved.id, name: finalName, spec, version });
      setName(finalName);
      setNotice({ text: `Saved ${finalName} v${version}`, tone: "ok" });
    } catch (err) {
      setNotice({ text: `Could not save: ${String(err)}`, tone: "bad" });
    } finally {
      setBusy(false);
    }
  }

  function discard() {
    setName(saved.name);
    setSpec(saved.spec);
    setNotice(null);
  }

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
      <div className={styles.builder}>
        <Picker picked={picked} onAdd={(i) => void add(i)} onRemove={remove} />
        <Cart
          name={name}
          onName={setName}
          saved={saved}
          isNew={isNew}
          dirty={dirty}
          specChanged={specChanged}
          busy={busy}
          projectCount={projectCount}
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
      <LeaveGuard when={dirty} bypass={bypass} />
    </Screen>
  );
}
