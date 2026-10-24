import { PanelLeftOpen } from "lucide-react";
import { useEffect, useState } from "react";
import { Outlet } from "react-router";
import { NewCollectionHost } from "../features/library";
import { ArtifactPanelHost } from "./ArtifactPanelHost";
import { ArtifactPanelProvider } from "../lib/artifactPanel";
import { decodeBool, usePersisted } from "../ui";
import { CollectionsProvider } from "./collections";
import styles from "./Layout.module.css";
import { Palette } from "./Palette";
import { sidebarShortcut } from "./platform";
import { UsageIndexProvider } from "./usageIndex";
import { ShortcutsDialog } from "./ShortcutsDialog";
import { Sidebar } from "./Sidebar";
import { useAutoUpdateCheck } from "./useAutoUpdateCheck";

type Overlay = "none" | "shortcuts" | "palette";

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
  );
}

export function Layout() {
  useAutoUpdateCheck();
  const [overlay, setOverlay] = useState<Overlay>("none");
  const [collapsed, setCollapsed] = usePersisted(
    "harneo.sidebar.collapsed",
    false,
    decodeBool,
  );

  useEffect(() => {
    function onKeyDown(ev: KeyboardEvent) {
      if (ev.key === "Escape") {
        setOverlay("none");
        return;
      }
      if (isTyping(ev.target)) return;
      if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "k") {
        ev.preventDefault();
        setOverlay((o) => (o === "palette" ? "none" : "palette"));
      } else if (
        (ev.ctrlKey || ev.metaKey) &&
        !ev.altKey &&
        !ev.shiftKey &&
        ev.key.toLowerCase() === "b"
      ) {
        ev.preventDefault();
        setCollapsed(!collapsed);
      } else if (ev.key === "?" && !ev.ctrlKey && !ev.metaKey && !ev.altKey) {
        ev.preventDefault();
        setOverlay((o) => (o === "shortcuts" ? "none" : "shortcuts"));
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [collapsed, setCollapsed]);

  const close = () => setOverlay("none");

  return (
    <CollectionsProvider>
      <UsageIndexProvider>
        <ArtifactPanelProvider>
          <div className={styles.frame} data-collapsed={collapsed || undefined}>
            {/* The router owns the hash, so move focus by hand instead of following #main. */}
            <a
              href="#main"
              className={styles.skip}
              onClick={(ev) => {
                ev.preventDefault();
                document.getElementById("main")?.focus();
              }}
            >
              Skip to content
            </a>
            <div className={styles.slot}>
              <div className={styles.slotIn} inert={collapsed}>
                <Sidebar
                  onShortcuts={() => setOverlay("shortcuts")}
                  onSearch={() => setOverlay("palette")}
                  onCollapse={() => setCollapsed(true)}
                />
              </div>
              {collapsed && (
                <button
                  type="button"
                  className={styles.expand}
                  aria-label="Show sidebar"
                  title={`Show sidebar  ${sidebarShortcut()}`}
                  onClick={() => setCollapsed(false)}
                >
                  <PanelLeftOpen size={16} aria-hidden="true" />
                </button>
              )}
            </div>
            <main id="main" tabIndex={-1} className={styles.main}>
              <Outlet />
            </main>
            {overlay === "shortcuts" && <ShortcutsDialog onClose={close} />}
            {overlay === "palette" && (
              <Palette
                onClose={close}
                onShortcuts={() => setOverlay("shortcuts")}
              />
            )}
            <NewCollectionHost />
            <ArtifactPanelHost />
          </div>
        </ArtifactPanelProvider>
      </UsageIndexProvider>
    </CollectionsProvider>
  );
}
