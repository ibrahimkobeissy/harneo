import { useEffect, useState } from "react";
import type { CategoryId } from "../../bindings/CategoryId";
import type { Template } from "../../bindings/Template";
import { templatesList } from "../../lib/ipc";

/** The one line under each category's name, from the design board. */
export const categoryHint: Record<CategoryId, string> = {
  software: "A line, with a loop from Review back to Build.",
  research: "Read and analyse in a loop until the question is answered.",
  writing:
    "A line, with an editorial loop: Edit and Fact-check go back to Draft.",
  video: "Pre, production, post. Picture and sound run in parallel in post.",
  social: "A cycle that fans out per platform and comes back to Plan.",
  data: "A line, with a back-and-forth between Clean and Explore.",
  design: "Double diamond: open up, narrow down, twice.",
  custom:
    "Describe your work. Claude picks one of the shapes above and names the stages; you edit them.",
};

/** What the preview says for Custom, which has no template stages yet. */
export const customPreview = "Proposed by Claude from your description";

/** The category templates, once loaded; empty until then or when they cannot be read. */
export function useTemplates(): Template[] {
  const [templates, setTemplates] = useState<Template[]>([]);
  useEffect(() => {
    let stale = false;
    templatesList().then(
      (all) => {
        if (!stale && Array.isArray(all)) setTemplates(all);
      },
      () => undefined,
    );
    return () => {
      stale = true;
    };
  }, []);
  return templates;
}

/** The display name of a category (`Software development`), or its id until templates load. */
export function labelOf(templates: Template[], id: CategoryId): string {
  return templates.find((t) => t.id === id)?.label ?? id;
}
