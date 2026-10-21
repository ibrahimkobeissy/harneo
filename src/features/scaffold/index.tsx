import { FolderPlus } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function ScaffoldScreen() {
  return (
    <Screen title="Scaffold project">
      <EmptyState icon={FolderPlus} title="Not built yet">
        Scaffolding arrives with the scaffold module.
      </EmptyState>
    </Screen>
  );
}
