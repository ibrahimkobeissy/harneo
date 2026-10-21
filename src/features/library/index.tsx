import { LayoutGrid } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function LibraryScreen() {
  return (
    <Screen title="Library">
      <EmptyState icon={LayoutGrid} title="Not built yet">
        The library arrives with the library module.
      </EmptyState>
    </Screen>
  );
}
