import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import type { Collection } from "../bindings/Collection";
import type { Filter } from "../bindings/Filter";
import { collectionsList } from "../lib/ipc";

/** What the New collection dialog opens with. */
export interface NewCollectionOptions {
  /** Start on the Smart choice with this rule (the Library's current filters). */
  rule?: Filter;
}

interface CollectionsValue {
  collections: Collection[];
  /** False until the first load has answered. */
  loaded: boolean;
  /** Reloads the list (names, counts); call after anything that changes a collection. */
  refresh: () => void;
  /** The New collection request waiting to be shown, if any. */
  request: NewCollectionOptions | null;
  openNew: (options?: NewCollectionOptions) => void;
  closeNew: () => void;
}

const idle: CollectionsValue = {
  collections: [],
  loaded: true,
  refresh: () => {},
  request: null,
  openNew: () => {},
  closeNew: () => {},
};

const Context = createContext<CollectionsValue>(idle);

/**
 * The collections the sidebar lists, shared with every screen that changes them. Outside a
 * provider the hook reports no collections and ignores requests, so screens render alone.
 */
export function CollectionsProvider({ children }: { children: ReactNode }) {
  const [collections, setCollections] = useState<Collection[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [request, setRequest] = useState<NewCollectionOptions | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let current = true;
    collectionsList().then(
      (list) => {
        if (!current) return;
        setCollections(Array.isArray(list) ? list : []);
        setLoaded(true);
      },
      () => {
        if (!current) return;
        setCollections([]);
        setLoaded(true);
      },
    );
    return () => {
      current = false;
    };
  }, [tick]);

  const refresh = useCallback(() => setTick((n) => n + 1), []);
  const openNew = useCallback(
    (options: NewCollectionOptions = {}) => setRequest(options),
    [],
  );
  const closeNew = useCallback(() => setRequest(null), []);
  const value = useMemo(
    () => ({ collections, loaded, refresh, request, openNew, closeNew }),
    [collections, loaded, refresh, request, openNew, closeNew],
  );
  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useCollections(): CollectionsValue {
  return useContext(Context);
}
