import { AboutPanel } from "../../app/AboutPanel";
import { Screen } from "../../ui";
import { ClaudeSettings } from "./ClaudeSettings";

export function SettingsScreen() {
  return (
    <Screen title="Settings" narrow>
      <ClaudeSettings />
      <AboutPanel />
    </Screen>
  );
}
