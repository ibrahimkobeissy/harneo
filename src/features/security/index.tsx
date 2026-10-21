import { ShieldCheck } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function SecurityScreen() {
  return (
    <Screen title="Security">
      <EmptyState icon={ShieldCheck} title="Not built yet">
        The security overview arrives with the security module.
      </EmptyState>
    </Screen>
  );
}
