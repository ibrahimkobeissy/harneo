import { Download } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function ImportScreen() {
  return (
    <Screen title="Import">
      <EmptyState icon={Download} title="Not built yet">
        Bring artifacts into the library from ~/.claude, a git URL or a folder.
        Claude suggests bundles and tags; nothing in your projects changes.
      </EmptyState>
    </Screen>
  );
}
