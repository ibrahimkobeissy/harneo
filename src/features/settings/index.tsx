import { AboutPanel } from "../../app/AboutPanel";
import { Screen } from "../../ui";
import { ClaudeSettings } from "./ClaudeSettings";
import { UpdatesPanel } from "./UpdatesPanel";

export function SettingsScreen() {
  return (
    <Screen title="Settings" narrow>
      <ClaudeSettings />
      <UpdatesPanel />
      <AboutPanel />
    </Screen>
  );
}
