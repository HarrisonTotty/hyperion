import { symbolOutline } from "../../spatial/symbols";

/**
 * The Greek cross's outline about the origin at `radiusPx`, as an SVG path's data: the feature
 * mark the galaxy map's overlays draw a feature and a dwarf core with.
 */
export function featureCrossPath(radiusPx: number): string {
  const outline = symbolOutline("greek-cross");
  const points = outline.kind === "polygon" ? outline.points : [];
  return `${points
    .map((point, index) => `${index === 0 ? "M" : "L"} ${point.x * radiusPx} ${point.y * radiusPx}`)
    .join(" ")} Z`;
}
