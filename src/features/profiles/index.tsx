import { Layers } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function ProfilesScreen() {
  return (
    <Screen title="Profiles">
      <EmptyState icon={Layers} title="Not built yet">
        A profile is a named selection of library artifacts, like “Light” or
        “Heavy”, that gets installed into projects. Picking an artifact brings
        its whole bundle.
      </EmptyState>
    </Screen>
  );
}
