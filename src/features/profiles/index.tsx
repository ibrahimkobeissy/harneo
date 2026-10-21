import { Layers } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function ProfilesScreen() {
  return (
    <Screen title="Profiles">
      <EmptyState icon={Layers} title="Not built yet">
        Profiles arrive with the profiles module.
      </EmptyState>
    </Screen>
  );
}
