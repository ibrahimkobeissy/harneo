import { Sparkles } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function CleanupScreen() {
  return (
    <Screen title="Cleanup">
      <EmptyState icon={Sparkles} title="Not built yet">
        Cleanup arrives with the cleanup module.
      </EmptyState>
    </Screen>
  );
}
