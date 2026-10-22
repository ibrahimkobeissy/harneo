import {
  ArrowDown,
  ArrowUp,
  Bookmark,
  LayoutGrid,
  List,
  Download,
  ListFilter,
  SearchX,
  SlidersHorizontal,
  TriangleAlert,
} from "lucide-react";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router";
import type { Collection } from "../../bindings/Collection";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { Page } from "../../bindings/Page";
import type { Query } from "../../bindings/Query";
import type { SortKey } from "../../bindings/SortKey";
import { useCollections } from "../../app/collections";
import { collectionPage, libraryList } from "../../lib/ipc";
import {
  Badge,
  Button,
  decodeBool,
  EmptyState,
  Screen,
  usePersisted,
} from "../../ui";
import { CollectionBar } from "./CollectionBar";
import { DetailPanel } from "./DetailPanel";
import { FilterColumn, type FilterGroup } from "./FilterColumn";
import { clampWidth, FILTERS_DEFAULT } from "./FilterResizer";
import { FilterSheet } from "./FilterSheet";
import { onGridKeyDown } from "./gridNav";
import { ItemCard } from "./ItemCard";
import { ItemTable } from "./ItemTable";
import cstyles from "./Collections.module.css";
import styles from "./Library.module.css";
import {
  type DefaultSort,
  defaultDescending,
  hasFilters,
  type LibraryParams,
  PER_PAGE,
  parseParams,
  sortKeys,
  sortLabels,
  toFilter,
  toQuery,
  toSearchParams,
  type View,
} from "./params";
import { Pager } from "./Pager";
import { ResultsToolbar } from "./ResultsToolbar";
import { SearchBox } from "./SearchBox";
import { useNarrow } from "./useNarrow";
import { Welcome } from "./Welcome";

type Load =
  | { status: "loading"; page: Page | null }
  | { status: "error"; message: string }
  | { status: "ready"; page: Page };

/** What the last settled request produced, tagged with the request it answered. */
interface Settled {
  id: string;
  page: Page | null;
  error: string | null;
}

/**
 * Loads the page for `key` (a JSON `{ collection, query }`: the library when `collection` is
 * null). While a newer request is in flight the previous page stays on screen; `reload`
 * re-runs the request after an error or a change made elsewhere.
 */
function useLibraryPage(key: string): [Load, () => void] {
  const [settled, setSettled] = useState<Settled>({
    id: "",
    page: null,
    error: null,
  });
  const [attempt, setAttempt] = useState(0);
  const id = `${attempt}:${key}`;
  useEffect(() => {
    let current = true;
    const { collection, query } = JSON.parse(key) as {
      collection: number | null;
      query: Query;
    };
    (collection === null
      ? libraryList(query)
      : collectionPage(BigInt(collection), query)
    ).then(
      (page) => {
        if (current) setSettled({ id, page, error: null });
      },
      (err: unknown) => {
        if (current) setSettled({ id, page: null, error: String(err) });
      },
    );
    return () => {
      current = false;
    };
  }, [key, id]);
  const reload = () => setAttempt((n) => n + 1);
  if (settled.id !== id)
    return [{ status: "loading", page: settled.page }, reload];
  if (settled.error !== null) {
    return [{ status: "error", message: settled.error }, reload];
  }
  return [{ status: "ready", page: settled.page as Page }, reload];
}

/**
 * The library: filter column, search, cards or list, pager. Everything the user chooses lives
 * in the URL query so Back restores it, including the artifact open in the detail panel
 * (`?open=<id>`). `onOpen` is also told when a card is opened.
 */
export function LibraryScreen({
  onOpen = noop,
  collection = null,
}: {
  onOpen?: (item: LibraryItem) => void;
  /** Show this collection's members instead of the whole library. */
  collection?: Collection | null;
}) {
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const { refresh: refreshCollections, openNew } = useCollections();
  const manual = collection?.kind === "manual";
  const fallback: DefaultSort = manual ? "position" : "name";
  const params = parseParams(searchParams, fallback);
  const query = toQuery(params);
  const [load, reload] = useLibraryPage(
    JSON.stringify({
      collection: collection ? Number(collection.id) : null,
      query,
    }),
  );
  const page = load.status === "error" ? null : load.page;
  const perPage = PER_PAGE[params.view];
  // Artifacts left behind by following references, newest last; Back pops one.
  const narrow = useNarrow();
  const [sheetOpen, setSheetOpen] = useState(false);
  const [collapsed, setCollapsed] = usePersisted(
    "harneo.filters.collapsed",
    false,
    decodeBool,
  );
  const [width, setWidth] = usePersisted(
    "harneo.filters.width",
    FILTERS_DEFAULT,
    (raw) => {
      const n = Number(raw);
      return Number.isFinite(n) ? clampWidth(n) : null;
    },
  );
  const [dragging, setDragging] = useState(false);
  const filtersButton = useRef<HTMLButtonElement>(null);
  const [trail, setTrail] = useState<{ id: string; name: string }[]>([]);
  // The card to give focus back to once the panel has closed.
  const returnTo = useRef<string | null>(null);

  useEffect(() => {
    if (params.open !== null || returnTo.current === null) return;
    const id = returnTo.current;
    returnTo.current = null;
    document
      .querySelector<HTMLElement>(`[data-item-id="${CSS.escape(id)}"]`)
      ?.focus();
  });

  function openPanel(id: string | null) {
    update({ open: id, page: params.page });
  }

  function openItem(item: LibraryItem) {
    setTrail([]);
    openPanel(item.id);
    onOpen(item);
  }

  function closePanel() {
    returnTo.current = trail[0]?.id ?? params.open;
    setTrail([]);
    openPanel(null);
  }

  function update(
    patch: Partial<LibraryParams>,
    options: { replace?: boolean } = {},
  ) {
    const resetsPage = !("page" in patch);
    setSearchParams(
      toSearchParams(
        {
          ...params,
          ...patch,
          ...(resetsPage ? { page: 1 } : {}),
        },
        fallback,
      ),
      options,
    );
  }

  // A page past the end (after a filter narrowed the list elsewhere) steps back to the last.
  useEffect(() => {
    if (
      load.status === "ready" &&
      load.page.items.length === 0 &&
      load.page.total > 0 &&
      params.page > 1
    ) {
      setSearchParams(
        toSearchParams(
          {
            ...params,
            page: Math.ceil(load.page.total / perPage),
          },
          fallback,
        ),
        { replace: true },
      );
    }
  });

  const filtered = hasFilters(params);

  if (
    load.status === "ready" &&
    load.page.total === 0 &&
    !filtered &&
    !collection
  ) {
    return <Welcome />;
  }

  function toggle(group: FilterGroup, value: string) {
    const current = params[group] as string[];
    const next = current.includes(value)
      ? current.filter((v) => v !== value)
      : [...current, value];
    update({ [group]: next } as Partial<LibraryParams>);
  }

  function clearAll() {
    update({ types: [], tags: [], sources: [], text: "" });
  }

  function sortBy(key: SortKey) {
    if (key === params.sort) update({ descending: !params.descending });
    else update({ sort: key, descending: defaultDescending(key) });
  }
  const sorts: readonly SortKey[] = manual
    ? ["position", ...sortKeys]
    : sortKeys;

  function setView(view: View) {
    update({ view });
  }

  const filterColumn = (docked: boolean) =>
    page && (
      <FilterColumn
        facets={page.facets}
        params={params}
        onToggle={toggle}
        onClearGroup={(group) =>
          update({ [group]: [] } as Partial<LibraryParams>)
        }
        dock={
          docked
            ? {
                collapsed,
                width,
                onCollapse: () => setCollapsed(true),
                onWidth: setWidth,
                onDragging: setDragging,
              }
            : undefined
        }
      />
    );
  const activeFilters =
    params.types.length + params.tags.length + params.sources.length;

  const toolbar = (
    <>
      {collection && (
        <CollectionBar
          collection={collection}
          onRuleSaved={() => {
            reload();
            refreshCollections();
          }}
        />
      )}
      {page && (
        <div className={styles.toolbar}>
          {(narrow || collapsed) && (
            <button
              ref={filtersButton}
              type="button"
              className={styles.filtersBtn}
              aria-haspopup={narrow ? "dialog" : undefined}
              aria-expanded={narrow ? sheetOpen : false}
              onClick={() =>
                narrow ? setSheetOpen(true) : setCollapsed(false)
              }
            >
              <SlidersHorizontal size={14} aria-hidden="true" />
              Filters
              {activeFilters > 0 && (
                <span className={styles.filtersCount}>{activeFilters}</span>
              )}
            </button>
          )}
          <ResultsToolbar
            params={params}
            facets={page.facets}
            total={page.total}
            unfilteredTotal={page.unfilteredTotal}
            filtered={filtered}
            onRemove={toggle}
            onClearText={() => update({ text: "" })}
            onClearAll={clearAll}
            onSaveCollection={
              collection ? undefined : () => openNew({ rule: toFilter(params) })
            }
          />
          <span className={styles.grow} />
          {params.view === "cards" && (
            <div className={styles.sorts} role="group" aria-label="Sort by">
              <span>Sort</span>
              {sorts.map((key) => {
                const active = key === params.sort;
                const Arrow = params.descending ? ArrowDown : ArrowUp;
                return (
                  <button
                    key={key}
                    type="button"
                    className={
                      active
                        ? `${styles.sortChip} ${styles.sortChipOn}`
                        : styles.sortChip
                    }
                    aria-pressed={active}
                    onClick={() => sortBy(key)}
                  >
                    {sortLabels[key]}
                    {active && (
                      <>
                        <Arrow size={12} aria-hidden="true" />
                        <span className={styles.sr}>
                          {params.descending ? ", descending" : ", ascending"}
                        </span>
                      </>
                    )}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}
    </>
  );

  const actions = (
    <>
      <SearchBox
        value={params.text}
        onCommit={(text) => update({ text }, { replace: true })}
      />
      <div className={styles.seg} role="group" aria-label="View">
        <button
          type="button"
          className={params.view === "cards" ? styles.segOn : undefined}
          aria-pressed={params.view === "cards"}
          onClick={() => setView("cards")}
        >
          <LayoutGrid size={14} aria-hidden="true" />
          Cards
        </button>
        <button
          type="button"
          className={params.view === "list" ? styles.segOn : undefined}
          aria-pressed={params.view === "list"}
          onClick={() => setView("list")}
        >
          <List size={14} aria-hidden="true" />
          List
        </button>
      </div>
      {!collection && (
        <Button variant="primary" onClick={() => void navigate("/import")}>
          <Download size={14} aria-hidden="true" />
          Import
        </Button>
      )}
    </>
  );

  return (
    <div
      className={styles.lib}
      data-collapsed={(!narrow && collapsed) || undefined}
      data-dragging={dragging || undefined}
      style={{ "--filters-w": `${width}px` } as CSSProperties}
    >
      {!narrow && filterColumn(true)}
      {narrow && sheetOpen && page && (
        <FilterSheet
          onClose={() => setSheetOpen(false)}
          returnFocus={filtersButton}
        >
          {filterColumn(false)}
        </FilterSheet>
      )}
      <Screen
        title={collection ? collection.name : "Library"}
        icon={
          collection && (
            <span className={cstyles.titleIcon}>
              {collection.kind === "smart" ? (
                <ListFilter size={20} aria-hidden="true" />
              ) : (
                <Bookmark size={20} aria-hidden="true" />
              )}
            </span>
          )
        }
        badge={
          collection && (
            <Badge>{collection.kind === "smart" ? "Smart" : "Manual"}</Badge>
          )
        }
        actions={actions}
        toolbar={toolbar}
      >
        <div aria-busy={load.status === "loading"}>
          {load.status === "error" && (
            <div role="alert">
              <EmptyState
                icon={TriangleAlert}
                title="The library could not load"
              >
                {load.message}
              </EmptyState>
              <div className={styles.retry}>
                <Button onClick={reload}>Retry</Button>
              </div>
            </div>
          )}
          {load.status === "loading" && !page && (
            <p role="status" className={styles.muted}>
              Loading the library…
            </p>
          )}
          {page && page.items.length === 0 && page.total === 0 && (
            <div>
              {collection && !filtered ? (
                <EmptyState
                  icon={SearchX}
                  title="Nothing in this collection yet"
                >
                  {manual
                    ? "Open an artifact and use Add to… to put it here."
                    : "No artifact matches this rule yet. It will fill as you import."}{" "}
                  Collections only organize the library; nothing is installed
                  until you use a profile.
                </EmptyState>
              ) : (
                <>
                  <EmptyState icon={SearchX} title="No artifacts match">
                    Nothing in {collection ? "this collection" : "the library"}{" "}
                    matches these filters
                    {params.text ? ` and “${params.text}”` : ""}.
                  </EmptyState>
                  <div className={styles.retry}>
                    <Button onClick={clearAll}>Clear filters</Button>
                  </div>
                </>
              )}
            </div>
          )}
          {page && page.items.length > 0 && params.view === "cards" && (
            <div className={styles.grid} onKeyDown={onGridKeyDown}>
              {page.items.map((item) => (
                <ItemCard key={item.id} item={item} onOpen={openItem} />
              ))}
            </div>
          )}
          {page && page.items.length > 0 && params.view === "list" && (
            <ItemTable
              items={page.items}
              sort={params.sort}
              descending={params.descending}
              onSort={sortBy}
              onOpen={openItem}
            />
          )}
          {page && (
            <Pager
              page={params.page}
              perPage={perPage}
              total={page.total}
              onPage={(p) => update({ page: p })}
            />
          )}
        </div>
      </Screen>
      {params.open && (
        <DetailPanel
          id={params.open}
          previousName={trail.at(-1)?.name ?? null}
          onOpen={(id, fromName) => {
            setTrail((t) => [...t, { id: params.open ?? "", name: fromName }]);
            openPanel(id);
          }}
          onBack={() => {
            const previous = trail.at(-1);
            if (!previous) return;
            setTrail(trail.slice(0, -1));
            openPanel(previous.id);
          }}
          onClose={closePanel}
          onTagsChanged={reload}
          collection={collection}
          onMembershipChanged={() => {
            reload();
            refreshCollections();
          }}
          onLeftCollection={() => {
            setTrail([]);
            openPanel(null);
            reload();
            refreshCollections();
          }}
          onRemoved={() => {
            setTrail([]);
            openPanel(null);
            reload();
          }}
        />
      )}
    </div>
  );
}

function noop() {}

export { NewCollectionHost } from "./NewCollectionDialog";

/** `#/collections/:id`: the library's screen over one collection's members. */
export function CollectionScreen() {
  const { id } = useParams();
  const { collections, loaded } = useCollections();
  const found = collections.find((c) => String(c.id) === id);
  if (found) return <LibraryScreen key={id} collection={found} />;
  return (
    <Screen title="Collection">
      {loaded ? (
        <EmptyState icon={SearchX} title="This collection is gone">
          It was deleted or never existed.{" "}
          <Link to="/library">Back to the library</Link>
        </EmptyState>
      ) : (
        <p role="status" className={styles.muted}>
          Loading the collection…
        </p>
      )}
    </Screen>
  );
}
