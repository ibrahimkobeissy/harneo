import { RefreshCw } from "lucide-react";
import { AboutPanel } from "../../app/AboutPanel";
import { Screen } from "../../ui";
import { ClaudeSettings } from "./ClaudeSettings";
import { DangerZone } from "./DangerZone";
import { Group } from "./Group";
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
        <Group
          id="background"
          title="Background"
          icon={<RefreshCw size={15} aria-hidden="true" />}
        >
          <UpdatesPanel />
          <LiveRescan />
        </Group>
        <AboutPanel grouped dataFolder={false} />
        <DangerZone />
      </div>
    </Screen>
  );
}
