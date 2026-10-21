import { Keyboard, Search } from "lucide-react";
import { NavLink } from "react-router";
import { Kbd, Logo, NavItem } from "../ui";
import { actions, places, settingsDestination } from "./routes";
import styles from "./Sidebar.module.css";

interface SidebarProps {
  onShortcuts: () => void;
  onSearch: () => void;
}

export function Sidebar({ onShortcuts, onSearch }: SidebarProps) {
  return (
    <nav className={styles.side} aria-label="Main">
      <NavLink to="/library" end aria-label="Harneo" className={styles.brand}>
        <Logo />
        <span className={styles.brandName}>Harneo</span>
      </NavLink>
      {places.map((d) => (
        <NavItem key={d.path} to={d.path} label={d.label} icon={d.icon} />
      ))}
      <h2 className={styles.heading}>Collections</h2>
      <p className={styles.none}>None yet</p>
      <h2 className={styles.heading}>Actions</h2>
      {actions.map((d) => (
        <NavItem key={d.path} to={d.path} label={d.label} icon={d.icon} />
      ))}
      <div className={styles.foot}>
        <button
          type="button"
          className={styles.kbdbar}
          title="Keyboard shortcuts (?)"
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
          title="Search and commands (Ctrl K)"
          onClick={onSearch}
        >
          <Search size={16} aria-hidden="true" />
          <span className={styles.lbl}>Search</span>
          <span className={styles.hint}>
            <Kbd>Ctrl K</Kbd>
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
