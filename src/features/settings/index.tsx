import { Info } from "lucide-react";
import { AboutPanel } from "../../app/AboutPanel";
import { Screen } from "../../ui";
import { ClaudeSettings } from "./ClaudeSettings";
import styles from "./Settings.module.css";
import { LiveRescan } from "./LiveRescan";
import { UpdatesPanel } from "./UpdatesPanel";
import { UsageSettings } from "./UsageSettings";

export function SettingsScreen() {
  return (
    <Screen title="Settings" narrow>
      <div className={styles.page}>
        <UsageSettings />
        <ClaudeSettings />
        <UpdatesPanel />
        <LiveRescan />
        {/* About keeps its own content; this wrapper gives it the group look. */}
        <div className={styles.about}>
          <Info size={15} aria-hidden="true" />
          <AboutPanel />
        </div>
      </div>
    </Screen>
  );
}
