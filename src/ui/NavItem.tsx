import type { LucideIcon } from "lucide-react";
import { NavLink } from "react-router";
import styles from "./NavItem.module.css";

interface NavItemProps {
  to: string;
  label: string;
  icon: LucideIcon;
  end?: boolean;
  /** A quiet number at the right, such as how many artifacts a collection holds. */
  count?: number;
  /** Shown instead of the icon in the 60px rail, in a small tinted square. */
  initials?: string;
}

const tints = [
  "blue",
  "violet",
  "cyan",
  "teal",
  "orange",
  "pink",
  "indigo",
  "amber",
] as const;

/** A stable tint per name, so a collection keeps its colour in the rail. */
function tintFor(name: string): (typeof tints)[number] {
  let h = 0;
  for (const ch of name) h = (h * 31 + (ch.codePointAt(0) ?? 0)) % 997;
  return tints[h % tints.length] ?? "blue";
}

// A sidebar link. The name stays on the link (aria-label) and shows as a tooltip
// on hover and keyboard focus when the sidebar collapses to the icon rail.
export function NavItem({
  to,
  label,
  icon: Icon,
  end,
  count,
  initials,
}: NavItemProps) {
  return (
    <NavLink
      to={to}
      end={end}
      aria-label={label}
      data-tip={label}
      className={({ isActive }) =>
        [
          styles.nav,
          isActive ? styles.on : "",
          initials === undefined ? "" : styles.hasInitials,
        ]
          .filter(Boolean)
          .join(" ")
      }
    >
      <Icon size={16} aria-hidden="true" className={styles.icon} />
      {initials !== undefined && (
        <span
          className={styles.initials}
          data-tint={tintFor(label)}
          aria-hidden="true"
        >
          {initials}
        </span>
      )}
      <span className={styles.lbl}>{label}</span>
      {count !== undefined && <span className={styles.count}>{count}</span>}
    </NavLink>
  );
}
