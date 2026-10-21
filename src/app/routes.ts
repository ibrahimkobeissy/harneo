import {
  Download,
  Eraser,
  FolderGit2,
  FolderPlus,
  Layers,
  LayoutGrid,
  RefreshCw,
  Settings,
  ShieldCheck,
  type LucideIcon,
} from "lucide-react";

export interface Destination {
  label: string;
  path: string;
  icon: LucideIcon;
}

export const places: readonly Destination[] = [
  { label: "Library", path: "/library", icon: LayoutGrid },
  { label: "Profiles", path: "/profiles", icon: Layers },
  { label: "Projects", path: "/projects", icon: FolderGit2 },
  { label: "Security", path: "/security", icon: ShieldCheck },
];

export const actions: readonly Destination[] = [
  { label: "Import", path: "/import", icon: Download },
  { label: "Scaffold project", path: "/scaffold", icon: FolderPlus },
  { label: "Refit project", path: "/refit", icon: RefreshCw },
  { label: "Cleanup", path: "/cleanup", icon: Eraser },
];

export const settingsDestination: Destination = {
  label: "Settings",
  path: "/settings",
  icon: Settings,
};
