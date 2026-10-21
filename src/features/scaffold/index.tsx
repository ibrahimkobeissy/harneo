import { FolderPlus } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function ScaffoldScreen() {
  return (
    <Screen title="Scaffold project">
      <EmptyState icon={FolderPlus} title="Not built yet">
        Install a profile into a new, empty project. You see every file before
        anything is written.
      </EmptyState>
    </Screen>
  );
}
