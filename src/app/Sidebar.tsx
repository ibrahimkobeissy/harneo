import { NavLink } from "react-router";
import {
  actions,
  places,
  settingsDestination,
  type Destination,
} from "./routes";

function NavItem({ to, label }: { to: string; label: string }) {
  return (
    <li>
      <NavLink to={to}>{label}</NavLink>
    </li>
  );
}

function NavList({ items }: { items: readonly Destination[] }) {
  return (
    <ul>
      {items.map((d) => (
        <NavItem key={d.path} to={d.path} label={d.label} />
      ))}
    </ul>
  );
}

interface SidebarProps {
  onShortcuts: () => void;
  onSearch: () => void;
}

export function Sidebar({ onShortcuts, onSearch }: SidebarProps) {
  return (
    <nav aria-label="Main">
      <NavLink to="/library" end>
        Harneo
      </NavLink>
      <NavList items={places} />
      <h2>Collections</h2>
      <p>None yet</p>
      <h2>Actions</h2>
      <NavList items={actions} />
      <div>
        <button type="button" onClick={onShortcuts}>
          Keyboard shortcuts <kbd>?</kbd>
        </button>
        <button type="button" onClick={onSearch}>
          Search <kbd>Ctrl K</kbd>
        </button>
        <ul>
          <NavItem
            to={settingsDestination.path}
            label={settingsDestination.label}
          />
        </ul>
      </div>
    </nav>
  );
}
