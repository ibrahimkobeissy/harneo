import { Download } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function ImportScreen() {
  return (
    <Screen title="Import">
      <EmptyState icon={Download} title="Not built yet">
        Importing artifacts arrives with the import module.
      </EmptyState>
    </Screen>
  );
}
