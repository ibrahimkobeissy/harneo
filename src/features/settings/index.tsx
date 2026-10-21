import { AboutPanel } from "../../app/AboutPanel";

export function SettingsScreen() {
  return (
    <section aria-labelledby="settings-title">
      <h1 id="settings-title">Settings</h1>
      <AboutPanel />
    </section>
  );
}
