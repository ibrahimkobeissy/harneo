import { AboutPanel } from "../../app/AboutPanel";
import { Screen } from "../../ui";

export function SettingsScreen() {
  return (
    <Screen title="Settings" narrow>
      <AboutPanel />
    </Screen>
  );
}
