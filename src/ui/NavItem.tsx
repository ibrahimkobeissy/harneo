import type { LucideIcon } from "lucide-react";
import { NavLink } from "react-router";
import styles from "./NavItem.module.css";

interface NavItemProps {
  to: string;
  label: string;
  icon: LucideIcon;
  end?: boolean;
}

// A sidebar link. The name stays on the link (aria-label) and shows as a tooltip
// on hover and keyboard focus when the sidebar collapses to the icon rail.
export function NavItem({ to, label, icon: Icon, end }: NavItemProps) {
  return (
    <NavLink
      to={to}
      end={end}
      aria-label={label}
      data-tip={label}
      className={({ isActive }) =>
        isActive ? `${styles.nav} ${styles.on}` : styles.nav
      }
    >
      <Icon size={16} aria-hidden="true" />
      <span className={styles.lbl}>{label}</span>
    </NavLink>
  );
}
