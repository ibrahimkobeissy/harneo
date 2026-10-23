import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import {
  artifactProfiles,
  profileGet,
  profileResolve,
  profileSave,
} from "../../lib/ipc";

export interface AddResult {
  /** The line to show and announce. */
  message: string;
  /** False when nothing was saved (already in the profile, or it cannot hold it). */
  saved: boolean;
}

/** The spec with the artifact added where its kind belongs; null when it cannot be added. */
function withArtifact(
  spec: ProfileSpec,
  id: string,
  kind: ArtifactKind,
): ProfileSpec | null {
  if (kind === "settings") {
    return spec.settings === null ? { ...spec, settings: id } : null;
  }
  if (kind === "mcp") {
    return { ...spec, mcp: [...spec.mcp, { artifact: id, env: {} }] };
  }
  return { ...spec, items: [...spec.items, id] };
}

/**
 * Adds an artifact to a profile's current spec and saves the next version. Bundle members it
 * pulls in are counted by resolving the spec before and after.
 */
export async function addToProfile(
  profile: { id: bigint; name: string },
  artifact: { id: string; name: string; kind: ArtifactKind },
): Promise<AddResult> {
  const using = await artifactProfiles(artifact.id);
  if (using.some((p) => Number(p.id) === Number(profile.id))) {
    return { message: `Already in ${profile.name}`, saved: false };
  }
  const view = await profileGet(profile.id);
  const next = withArtifact(view.spec, artifact.id, artifact.kind);
  if (next === null) {
    return {
      message: `${profile.name} already has a Settings template. Change it in the profile.`,
      saved: false,
    };
  }
  const [before, after] = await Promise.all([
    profileResolve(view.spec),
    profileResolve(next),
  ]);
  const bundled = Math.max(
    0,
    after.artifacts.length - before.artifacts.length - 1,
  );
  const version = await profileSave(profile.id, next);
  const plus = bundled > 0 ? ` + ${bundled} bundled` : "";
  return {
    message: `Added ${artifact.name}${plus} to ${profile.name} (now v${version}).`,
    saved: true,
  };
}
