import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Fact } from "../../bindings/Fact";
import type { MapDoc } from "../../bindings/MapDoc";
import type { Zone } from "../../bindings/Zone";

/** What the canvas needs to know about an artifact on the map. */
export interface ArtifactInfo {
  name: string;
  kind: ArtifactKind;
}

/** One hook of the settings template, for the rail along the line. */
export interface HookInfo {
  event: string;
  matcher: string;
}

/** Which of the three layers are shown. */
export interface Layers {
  line: boolean;
  toolbelt: boolean;
  floor: boolean;
}

/** The canvas's view state, owned by the screen and passed in. */
export interface MapViewState {
  /** 0.4 to 1.5. */
  zoom: number;
  layers: Layers;
  /** Artifacts the user hid. */
  hidden: string[];
  /** Artifact types the user hid with the Types menu. */
  hiddenKinds: ArtifactKind[];
}

/** Everything the map canvas draws; it has no IPC of its own. */
export interface MapData {
  doc: MapDoc;
  /** Name and kind by artifact id; a placement whose artifact isn't here is not drawn. */
  artifacts: Record<string, ArtifactInfo>;
  /** Facts from the files, by artifact id. */
  facts: Record<string, Fact[]>;
  /** Artifact ids per bundle. */
  bundles: string[][];
  hooks: HookInfo[];
}

export const ZOOM_MIN = 0.4;
export const ZOOM_MAX = 1.5;
export const ZOOM_STEP = 0.1;
/** Below this zoom the map turns into plain boxes. */
export const QUIET_BELOW = 0.75;

export const DEFAULT_VIEW: MapViewState = {
  zoom: 1,
  layers: { line: true, toolbelt: true, floor: true },
  hidden: [],
  hiddenKinds: [],
};

export function clampZoom(zoom: number): number {
  return Math.round(Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom)) * 100) / 100;
}

/** Drag and drop onto the map: the drop targets are the stage frames, the toolbelt and the floor. */
export interface Dnd {
  /** True while an artifact is being dragged. */
  active: boolean;
  onStart: (artifact: string) => void;
  onEnd: () => void;
  onDrop: (zone: Zone) => void;
}

/** The candidate Harnfit proposes, drawn as a dashed "NEW" node over a faded map. */
export interface Ghost {
  name: string;
  kind: ArtifactKind;
  zone: Zone;
  /** Artifact ids on the map the candidate clashes with: red lines run to them. */
  conflicts: string[];
}
