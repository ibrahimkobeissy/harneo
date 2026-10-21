export interface Destination {
  label: string;
  path: string;
}

export const places: readonly Destination[] = [
  { label: "Library", path: "/library" },
  { label: "Profiles", path: "/profiles" },
  { label: "Projects", path: "/projects" },
  { label: "Security", path: "/security" },
];

export const actions: readonly Destination[] = [
  { label: "Import", path: "/import" },
  { label: "Scaffold project", path: "/scaffold" },
  { label: "Refit project", path: "/refit" },
  { label: "Cleanup", path: "/cleanup" },
];

export const settingsDestination: Destination = {
  label: "Settings",
  path: "/settings",
};
