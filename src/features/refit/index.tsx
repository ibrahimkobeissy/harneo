import { RefreshCw } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function RefitScreen() {
  return (
    <Screen title="Refit project">
      <EmptyState icon={RefreshCw} title="Not built yet">
        Install or switch a profile in a project that already has a harness:
        keep, replace or delete each item, with every replaced file backed up.
      </EmptyState>
    </Screen>
  );
}
