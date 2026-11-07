import {
  createHashRouter,
  Navigate,
  Route,
  RouterProvider,
  Routes,
} from "react-router";
import { Layout } from "./app/Layout";
import { CleanupScreen } from "./features/cleanup";
import { HarnmapScreen } from "./features/harnmap";
import { ImportScreen } from "./features/import";
import { CollectionScreen, LibraryScreen } from "./features/library";
import { ProfileBuilder, ProfilesScreen } from "./features/profiles";
import { ProjectScreen, ProjectsScreen } from "./features/projects";
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
        <Route path="collections/:id" element={<CollectionScreen />} />
        <Route path="profiles" element={<ProfilesScreen />} />
        <Route path="profiles/:id" element={<ProfileBuilder />} />
        <Route path="profiles/:id/harnmap" element={<HarnmapScreen />} />
        <Route path="projects" element={<ProjectsScreen />} />
        <Route path="projects/:id" element={<ProjectScreen />} />
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

// Hash history: a Tauri webview has no server to rewrite paths. A data router, so screens can
// block navigation while they hold unsaved changes (`useBlocker`).
const router = createHashRouter([{ path: "*", element: <AppRoutes /> }]);

export function App() {
  return <RouterProvider router={router} />;
}
