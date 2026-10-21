import { LayoutGrid } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function LibraryScreen() {
  return (
    <Screen title="Library">
      <EmptyState icon={LayoutGrid} title="Not built yet">
        Every skill, subagent, command, settings template and MCP server you
        trust, kept outside any project with its source, trust score and usage.
      </EmptyState>
    </Screen>
  );
}
