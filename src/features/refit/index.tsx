import { RefreshCw } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function RefitScreen() {
  return (
    <Screen title="Refit project">
      <EmptyState icon={RefreshCw} title="Not built yet">
        Refitting arrives with the refit module.
      </EmptyState>
    </Screen>
  );
}
