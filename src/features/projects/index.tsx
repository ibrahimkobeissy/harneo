import { FolderGit2 } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function ProjectsScreen() {
  return (
    <Screen title="Projects">
      <EmptyState icon={FolderGit2} title="Not built yet">
        The projects Harneo has scaffolded or refitted, the profile and version
        each one runs, and any drift from it.
      </EmptyState>
    </Screen>
  );
}
