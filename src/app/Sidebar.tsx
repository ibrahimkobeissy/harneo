import { Bookmark, Keyboard, ListFilter, Plus, Search } from "lucide-react";
import { Link } from "react-router";
import { Kbd, Logo, NavItem } from "../ui";
import { useCollections } from "./collections";
import { searchShortcut } from "./platform";
import { actions, places, settingsDestination } from "./routes";
import styles from "./Sidebar.module.css";

interface SidebarProps {
  onShortcuts: () => void;
  onSearch: () => void;
}

export function Sidebar({ onShortcuts, onSearch }: SidebarProps) {
  const search = searchShortcut();
  const { collections, openNew } = useCollections();
  return (
    <nav className={styles.side} aria-label="Main">
      {/* The brand goes home but is not a nav item, so only Library carries aria-current. */}
      <Link
        to="/library"
        aria-label="Harneo, go to Library"
        className={styles.brand}
      >
        <Logo />
        <span className={styles.brandName}>Harneo</span>
      </Link>
      {places.map((d) => (
        <NavItem key={d.path} to={d.path} label={d.label} icon={d.icon} />
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
