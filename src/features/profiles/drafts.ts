import type { CategoryId } from "../../bindings/CategoryId";
import type { ProfileSpec } from "../../bindings/ProfileSpec";

/** Unsaved edits of a profile's builder, kept while the user looks at other screens. */
export interface Draft {
  name: string;
  spec: ProfileSpec;
  category?: CategoryId;
  categoryNote?: string | null;
}

const drafts = new Map<string, Draft>();

export const draftFor = (key: string): Draft | undefined => drafts.get(key);
export const keepDraft = (key: string, draft: Draft): void => {
  drafts.set(key, draft);
};
export const dropDraft = (key: string): void => {
  drafts.delete(key);
};
