import { FolderGit2 } from "lucide-react";
import { EmptyState, Screen } from "../../ui";

export function ProjectsScreen() {
  return (
    <Screen title="Projects">
      <EmptyState icon={FolderGit2} title="Not built yet">
        Projects arrive with the projects module.
      </EmptyState>
    </Screen>
  );
}
