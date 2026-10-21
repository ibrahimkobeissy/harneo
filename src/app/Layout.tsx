import { useEffect, useState } from "react";
import { Outlet } from "react-router";
import styles from "./Layout.module.css";
import { Palette } from "./Palette";
import { ShortcutsDialog } from "./ShortcutsDialog";
import { Sidebar } from "./Sidebar";

type Overlay = "none" | "shortcuts" | "palette";

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
  );
}

export function Layout() {
  const [overlay, setOverlay] = useState<Overlay>("none");

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
      } else if (ev.key === "?" && !ev.ctrlKey && !ev.metaKey && !ev.altKey) {
        ev.preventDefault();
        setOverlay((o) => (o === "shortcuts" ? "none" : "shortcuts"));
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const close = () => setOverlay("none");

  return (
    <div className={styles.frame}>
      <a href="#main" className={styles.skip}>
        Skip to content
      </a>
      <Sidebar
        onShortcuts={() => setOverlay("shortcuts")}
        onSearch={() => setOverlay("palette")}
      />
      <main id="main" tabIndex={-1} className={styles.main}>
        <Outlet />
      </main>
      {overlay === "shortcuts" && <ShortcutsDialog onClose={close} />}
      {overlay === "palette" && (
        <Palette onClose={close} onShortcuts={() => setOverlay("shortcuts")} />
      )}
    </div>
  );
}
