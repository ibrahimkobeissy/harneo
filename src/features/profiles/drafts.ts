import type { CategoryId } from "../../bindings/CategoryId";
import type { ProfileSpec } from "../../bindings/ProfileSpec";

/** Unsaved edits of a profile's builder, kept while the user looks at other screens. */
export interface Draft {
  /** The saved profile this draft was kept for; a draft never opens on another one. */
  base: string;
  name: string;
  spec: ProfileSpec;
  category?: CategoryId;
  categoryNote?: string | null;
}

const drafts = new Map<string, Draft>();

export const draftFor = (key: string, base: string): Draft | undefined => {
  const kept = drafts.get(key);
  return kept?.base === base ? kept : undefined;
};
export const keepDraft = (key: string, draft: Draft): void => {
  drafts.set(key, draft);
};
export const dropDraft = (key: string): void => {
  drafts.delete(key);
};
