import { Eraser } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function CleanupScreen() {
  return (
    <Screen title="Cleanup">
      <EmptyState icon={Eraser} title="Not built yet">
        Find what is unused, outdated, repeated or untrusted across your library
        and profiles, and prune it.
      </EmptyState>
    </Screen>
  );
}
