import {
  Bookmark,
  Keyboard,
  ListFilter,
  PanelLeftClose,
  Plus,
  Search,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useLocation } from "react-router";
import { projectsList } from "../lib/ipc";
import { Kbd, Logo, NavItem } from "../ui";
import { initialsOf } from "../ui/initials";
import { useCollections } from "./collections";
import { searchShortcut, sidebarShortcut } from "./platform";
import { actions, places, settingsDestination } from "./routes";
import styles from "./Sidebar.module.css";

interface SidebarProps {
  onShortcuts: () => void;
  onSearch: () => void;
  onCollapse: () => void;
}

/** How many projects Harneo manages; refreshed on every navigation, quiet when it cannot load. */
function useProjectCount(): number | undefined {
  const { pathname } = useLocation();
  const [count, setCount] = useState<number | undefined>(undefined);
  useEffect(() => {
    let live = true;
    projectsList().then(
      (list) => {
        if (live && Array.isArray(list)) setCount(list.length);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [pathname]);
  return count;
}

export function Sidebar({ onShortcuts, onSearch, onCollapse }: SidebarProps) {
  const search = searchShortcut();
  const { collections, openNew } = useCollections();
  const projectCount = useProjectCount();
  return (
    <nav className={styles.side} aria-label="Main">
      <div className={styles.top}>
        {/* The brand goes home but is not a nav item, so only Library carries aria-current. */}
        <Link
          to="/library"
          aria-label="Harneo, go to Library"
          className={styles.brand}
        >
          <Logo />
          <span className={styles.brandName}>Harneo</span>
        </Link>
        <button
          type="button"
          className={styles.collapse}
          aria-label="Hide sidebar"
          data-tip={`Hide sidebar  ${sidebarShortcut()}`}
          onClick={onCollapse}
        >
          <PanelLeftClose size={16} aria-hidden="true" />
        </button>
      </div>
      {places.map((d) => (
        <NavItem
          key={d.path}
          to={d.path}
          label={d.label}
          icon={d.icon}
          count={d.path === "/projects" ? projectCount : undefined}
        />
      ))}
      <div className={styles.headingRow}>
        <h2 className={styles.heading}>Collections</h2>
        <button
          type="button"
          className={styles.plus}
          aria-label="New collection"
          data-tip="New collection"
          onClick={() => openNew()}
        >
          <Plus size={14} aria-hidden="true" />
        </button>
      </div>
      {collections.length === 0 && (
        <p className={styles.none}>
          None yet. Saved groupings of library artifacts appear here.
        </p>
      )}
      {collections.map((c) => (
        <NavItem
          key={String(c.id)}
          to={`/collections/${c.id}`}
          label={c.name}
          icon={c.kind === "smart" ? ListFilter : Bookmark}
          count={c.count}
          initials={initialsOf(c.name)}
        />
      ))}
      <h2 className={styles.heading}>Actions</h2>
      {actions.map((d) => (
        <NavItem key={d.path} to={d.path} label={d.label} icon={d.icon} />
      ))}
      <div className={styles.foot}>
        <button
          type="button"
          className={styles.kbdbar}
          aria-label="Keyboard shortcuts"
          data-tip="Keyboard shortcuts  ?"
          onClick={onShortcuts}
        >
          <Keyboard size={16} aria-hidden="true" />
          <span className={styles.lbl}>Keyboard shortcuts</span>
          <span className={styles.hint}>
            <Kbd>?</Kbd>
          </span>
        </button>
        <button
          type="button"
          className={styles.kbdbar}
          aria-label="Search and commands"
          data-tip={`Search and commands  ${search}`}
          onClick={onSearch}
        >
          <Search size={16} aria-hidden="true" />
          <span className={styles.lbl}>Search</span>
          <span className={styles.hint}>
            <Kbd>{search}</Kbd>
          </span>
        </button>
        <NavItem
          to={settingsDestination.path}
          label={settingsDestination.label}
          icon={settingsDestination.icon}
        />
      </div>
    </nav>
  );
}
