import { ShieldCheck } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function SecurityScreen() {
  return (
    <Screen title="Security">
      <EmptyState icon={ShieldCheck} title="Not built yet">
        Every artifact is audited on import: a trust score, its findings, and
        exactly which files, commands, hosts and env vars it can reach.
      </EmptyState>
    </Screen>
  );
}
