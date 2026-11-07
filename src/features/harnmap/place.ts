import type { MapDoc } from "../../bindings/MapDoc";
import type { Zone } from "../../bindings/Zone";

/** A zone as a `<select>` value and a drop target key: `stage:<id>`, `toolbelt` or `floor`. */
export function zoneKey(zone: Zone): string {
  return zone.type === "stage" ? `stage:${zone.id}` : zone.type;
}

export function zoneOfKey(key: string): Zone {
  if (key === "toolbelt") return { type: "toolbelt" };
  if (key === "floor") return { type: "floor" };
  return { type: "stage", id: key.replace(/^stage:/, "") };
}

export function sameZone(a: Zone, b: Zone): boolean {
  return zoneKey(a) === zoneKey(b);
}

/** The zone's name for the user: the stage's name, "Toolbelt" or "Floor". */
export function zoneName(doc: MapDoc, zone: Zone): string {
  if (zone.type === "toolbelt") return "Toolbelt";
  if (zone.type === "floor") return "Floor";
  return doc.stages.find((s) => s.id === zone.id)?.name ?? zone.id;
}

/** Where a toast says an artifact went: "in Test", "on the toolbelt", "on the floor". */
export function zonePhrase(doc: MapDoc, zone: Zone): string {
  if (zone.type === "toolbelt") return "on the toolbelt";
  if (zone.type === "floor") return "on the floor";
  return `in ${zoneName(doc, zone)}`;
}

/** The order that puts an artifact last in `zone`. */
export function nextOrder(doc: MapDoc, zone: Zone): number {
  const orders = doc.placements
    .filter((p) => sameZone(p.zone, zone))
    .map((p) => p.order);
  return orders.length === 0 ? 0 : Math.max(...orders) + 1;
}

/** Placements in map order: stages in order, then the toolbelt, then the floor. */
export function inMapOrder(doc: MapDoc): MapDoc["placements"] {
  const rank = (zone: Zone) =>
    zone.type === "stage"
      ? doc.stages.findIndex((s) => s.id === zone.id)
      : zone.type === "toolbelt"
        ? doc.stages.length
        : doc.stages.length + 1;
  return [...doc.placements].sort(
    (a, b) => rank(a.zone) - rank(b.zone) || a.order - b.order,
  );
}

/** "3 Nov, 14:12" for the builder card and the history. */
export function stamp(createdAt: number): string {
  const d = new Date(createdAt * 1000);
  return `${d.toLocaleDateString(undefined, { day: "numeric", month: "short" })}, ${d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}`;
}
