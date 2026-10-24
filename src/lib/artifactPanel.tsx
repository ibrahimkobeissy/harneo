import {
  type ButtonHTMLAttributes,
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

/** What the side panel was asked to show. */
export interface OpenRequest {
  id: string;
  /** Scroll to the trust section and focus its heading. */
  focus: "trust" | null;
  /** The element to give focus back to when the panel closes. */
  returnFocus: HTMLElement | null;
}

export interface OpenOptions {
  focus?: "trust";
  returnFocus?: HTMLElement | null;
}

interface ArtifactPanelValue {
  request: OpenRequest | null;
  /** Shows the artifact's details beside the current screen. */
  openArtifact: (id: string, options?: OpenOptions) => void;
  /** Swaps the shown artifact (a reference followed inside the panel); focus target is kept. */
  swapArtifact: (id: string) => void;
  closeArtifact: () => void;
  /** Counts changes made from the panel (delete, tag, add to a profile): hosts reload on it. */
  changes: number;
  notifyChanged: () => void;
}

const idle: ArtifactPanelValue = {
  request: null,
  openArtifact: () => {},
  swapArtifact: () => {},
  closeArtifact: () => {},
  changes: 0,
  notifyChanged: () => {},
};

const Context = createContext<ArtifactPanelValue>(idle);

/**
 * The artifact details panel any screen can open in place. Outside a provider the hook ignores
 * requests, so screens render alone.
 */
export function ArtifactPanelProvider({ children }: { children: ReactNode }) {
  const [request, setRequest] = useState<OpenRequest | null>(null);
  const [changes, setChanges] = useState(0);

  const openArtifact = useCallback((id: string, options: OpenOptions = {}) => {
    setRequest({
      id,
      focus: options.focus ?? null,
      returnFocus: options.returnFocus ?? null,
    });
  }, []);
  const swapArtifact = useCallback((id: string) => {
    setRequest((r) => (r ? { ...r, id, focus: null } : r));
  }, []);
  const closeArtifact = useCallback(() => setRequest(null), []);
  const notifyChanged = useCallback(() => setChanges((n) => n + 1), []);

  const value = useMemo(
    () => ({
      request,
      openArtifact,
      swapArtifact,
      closeArtifact,
      changes,
      notifyChanged,
    }),
    [
      request,
      openArtifact,
      swapArtifact,
      closeArtifact,
      changes,
      notifyChanged,
    ],
  );
  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useArtifactPanel(): ArtifactPanelValue {
  return useContext(Context);
}

/** Calls `callback` each time the panel reports a change (not on mount). */
export function useArtifactPanelChanged(callback: () => void): void {
  const { changes } = useContext(Context);
  const latest = useRef(callback);
  const seen = useRef(changes);
  useEffect(() => {
    latest.current = callback;
  });
  useEffect(() => {
    if (seen.current === changes) return;
    seen.current = changes;
    latest.current();
  }, [changes]);
}

/** A button that opens an artifact in the side panel; it gets focus back when the panel closes. */
export function OpenArtifactButton({
  id,
  focus,
  onOpened,
  children,
  ...rest
}: {
  id: string;
  focus?: "trust";
  /** Called after the panel was asked to open (a dialog closes itself here). */
  onOpened?: () => void;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onClick" | "type">) {
  const { openArtifact } = useContext(Context);
  return (
    <button
      {...rest}
      type="button"
      data-artifact-id={id}
      onClick={(ev) => {
        openArtifact(id, { focus, returnFocus: ev.currentTarget });
        onOpened?.();
      }}
    >
      {children}
    </button>
  );
}
