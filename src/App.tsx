import { HashRouter, Navigate, Route, Routes } from "react-router";
import { Layout } from "./app/Layout";
import { CleanupScreen } from "./features/cleanup";
import { ImportScreen } from "./features/import";
import { LibraryScreen } from "./features/library";
import { ProfilesScreen } from "./features/profiles";
import { ProjectsScreen } from "./features/projects";
import { RefitScreen } from "./features/refit";
import { ScaffoldScreen } from "./features/scaffold";
import { SecurityScreen } from "./features/security";
import { SettingsScreen } from "./features/settings";

export function AppRoutes() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Navigate to="/library" replace />} />
        <Route path="library" element={<LibraryScreen />} />
        <Route path="profiles" element={<ProfilesScreen />} />
        <Route path="projects" element={<ProjectsScreen />} />
        <Route path="security" element={<SecurityScreen />} />
        <Route path="import" element={<ImportScreen />} />
        <Route path="scaffold" element={<ScaffoldScreen />} />
        <Route path="refit" element={<RefitScreen />} />
        <Route path="cleanup" element={<CleanupScreen />} />
        <Route path="settings" element={<SettingsScreen />} />
        <Route path="*" element={<Navigate to="/library" replace />} />
      </Route>
    </Routes>
  );
}

// Hash history: a Tauri webview has no server to rewrite paths.
export function App() {
  return (
    <HashRouter>
      <AppRoutes />
    </HashRouter>
  );
}
