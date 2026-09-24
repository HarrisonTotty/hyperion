/**
 * The pure model behind the orbit map: a system at the display time as a scene for the general
 * spatial view (plan 14, P14.T42).
 *
 * @remarks
 * Nothing here touches the DOM or the server. The scene's unit is the astronomical unit, its
 * positions offsets from the system's barycentre along the galactic axes, and its frame the
 * system's reference plane (D21). Hosts are drawn with the registry's symbols at their mass layer's
 * size, each where its chain of orbits puts it at the display time; each star's path about its
 * pair's barycentre is drawn through it, and the selected star's path in `--text` 2 px wide. The
 * planets, moons, belts and zones join the same scene with `system_bodies`.
 */
import type { UniverseTime } from "@hyperion/protocol";

import { starSizeClass, starSymbol } from "../../lib/galaxy/starSymbols";
import type { LayerIndex } from "../../lib/galaxy/model";
import { composePosition, orbitPolyline } from "../../lib/orbit";
import type { HierarchyLayout } from "../../lib/system/hierarchy";
import type { HostBody } from "../../lib/system/model";
import { type LocalFrame, localFrameAt, planeFrame } from "../../spatial/frame";
import type { PathMark, PlaneRing, PointMark, SpatialScene } from "../../spatial/marks";
import { gridSpacing } from "../../spatial/scale";
import { add, scale, type Vec3 } from "../../spatial/vec3";
import type { LayerBand } from "../galaxy/chartModel";
import { METRES_PER_AU } from "./orbitScale";

/** A zoom preset of the orbit map, which sets the radius the view fits. */
export type ZoomPreset = "inner" | "all";

/** A zoom preset's control: the preset, its label and its single key. */
export interface ZoomPresetControl {
  readonly name: ZoomPreset;
  readonly label: string;
  readonly key: string;
}

/**
 * The orbit map's zoom presets in the order they are offered: `INNER` and `ALL` (plan 14,
 * P14.T42.b). `BELTS` joins them with the belts.
 */
export const ZOOM_PRESETS: ReadonlyArray<ZoomPresetControl> = [
  { name: "inner", label: "INNER", key: "I" },
  { name: "all", label: "ALL", key: "A" },
];

/**
 * The radius both presets fit, in astronomical units, for a system with nothing on an orbit yet: a
 * single star, whose map shows the star at its barycentre on a grid for scale.
 */
export const LONE_STAR_FIT_AU = 1;

/** Points of an orbit's drawn ellipse, every 2° of eccentric anomaly. */
const ORBIT_SEGMENTS = 180;

/** The orbit map's reference plane: its frame, and its name as the legend reads it. */
export interface OrbitPlane {
  readonly frame: LocalFrame;
  /** `SYSTEM PLANE`, or `GALACTIC PLANE` for a single star, which has no plane of its own yet. */
  readonly name: string;
  /** Whether the plane is the system's own: its fill is then above and below, not north and south. */
  readonly isSystemPlane: boolean;
}

/**
 * The orbit map's reference plane: the plane of the innermost pair that holds the primary, its
 * normal along the pair's angular momentum and its coreward the galactic coreward laid onto it (D21;
 * `planeFrame`); or, for a single star, the galactic plane at the system.
 *
 * @remarks
 * D21's plane is the primary's planetary plane, or a close binary's. Until `system_bodies` brings
 * the planets, the stars' innermost orbit about the primary is the only plane the system has; a
 * single star has none, and its map is drawn on the galactic plane, which the legend names as such.
 *
 * @param positionLy - The system's position in the `GALACTIC` frame.
 */
export function orbitPlane(layout: HierarchyLayout, positionLy: Vec3): OrbitPlane {
  const galactic = localFrameAt(positionLy);
  if (layout.primaryPairNormal === null) {
    return { frame: galactic, name: "GALACTIC PLANE", isSystemPlane: false };
  }
  return {
    frame: planeFrame(layout.primaryPairNormal, galactic.coreward),
    name: "SYSTEM PLANE",
    isSystemPlane: true,
  };
}

/** The radius each zoom preset fits, in astronomical units. */
export type FitRadii = Readonly<Record<ZoomPreset, number>>;

/**
 * The radius each zoom preset fits, until there are planets: `ALL` the farthest any drawn star's
 * orbits can take it from the barycentre, `INNER` the nearest such reach that is not zero, so that a
 * binary's primary fills `INNER` and both stars fit `ALL` (plan 14, P14.T42.b).
 *
 * @remarks
 * A reach is the sum of the star's share of each orbit's apoapsis distance up its chain, which
 * bounds where it can be at any time, so that neither preset moves as the display time does. A
 * system with no drawn star off its barycentre fits {@link LONE_STAR_FIT_AU}.
 */
export function fitRadiiAu(layout: HierarchyLayout, hosts: ReadonlyArray<HostBody>): FitRadii {
  const reaches = hosts
    .filter((host) => starSymbol(host.kind) !== null)
    .map((host) => (layout.reachM.get(host.id) ?? 0) / METRES_PER_AU)
    .filter((reach) => reach > 0);
  if (reaches.length === 0) {
    return { inner: LONE_STAR_FIT_AU, all: LONE_STAR_FIT_AU };
  }
  return { inner: Math.min(...reaches), all: Math.max(...reaches) };
}

/**
 * The mass layer an initial mass falls in, by the chart's census bands: the band whose edges hold
 * it, the lightest below the lightest band and the heaviest above the heaviest.
 *
 * @param bands - The census's five bands, lightest first.
 */
export function layerOfMass(initialMassMsun: number, bands: ReadonlyArray<LayerBand>): LayerIndex {
  let layer: LayerIndex = 0;
  for (const band of bands) {
    if (initialMassMsun >= band.minMsun) {
      layer = band.index;
    }
  }
  return layer;
}

/** A position in metres as scene units, astronomical units. */
function toAu(positionM: Vec3): Vec3 {
  return scale(positionM, 1 / METRES_PER_AU);
}

/**
 * Every drawn host as a point mark at the display time: its registry symbol at its mass layer's
 * size, labelled with its designation, the heaviest labelled first. A star that left no remnant is
 * not drawn (it stays in the list).
 */
export function hostMarks(
  hosts: ReadonlyArray<HostBody>,
  layout: HierarchyLayout,
  time: UniverseTime,
  bands: ReadonlyArray<LayerBand>,
): ReadonlyArray<PointMark> {
  const marks: PointMark[] = [];
  for (const host of hosts) {
    const shape = starSymbol(host.kind);
    if (shape === null) {
      continue;
    }
    marks.push({
      id: host.id,
      position: toAu(composePosition(layout.placements, host.id, time)),
      shape,
      sizeClass: starSizeClass(shape, layerOfMass(host.initialMassMsun, bands)),
      status: "plain",
      label: host.designation,
      labelPriority: host.initialMassMsun,
    });
  }
  return marks;
}

/**
 * The path each drawn star, and each inner pair's barycentre, traces about its pair's barycentre as
 * it is at the display time: the pair's relative orbit scaled by the member's share (plan 14,
 * P14.T42.a). The selected star's is the one `selected` path; the rest are `reference`.
 *
 * @remarks
 * A member's path is drawn about where its pair's barycentre is now, which is where the member
 * lies on it now. An inner pair's barycentre is drawn too, since at the outer orbit's scale its two
 * stars sit on it. A star that left no remnant has no path. Paths are not labelled: the marks on
 * them are.
 */
export function orbitPaths(
  hosts: ReadonlyArray<HostBody>,
  layout: HierarchyLayout,
  time: UniverseTime,
  selectedId: string | null,
): ReadonlyArray<PathMark> {
  const undrawn = new Set(
    hosts.filter((host) => starSymbol(host.kind) === null).map((host) => host.id),
  );
  const paths: PathMark[] = [];
  for (const member of layout.members) {
    if (member.hostId !== null && undrawn.has(member.hostId)) {
      continue;
    }
    const barycentreM = composePosition(layout.placements, member.pairKey, time);
    const points = orbitPolyline(member.orbit, ORBIT_SEGMENTS).map((pointM) =>
      toAu(add(barycentreM, scale(pointM, member.share))),
    );
    const [first] = points;
    if (first === undefined) {
      continue;
    }
    paths.push({
      id: member.key,
      points,
      role: member.hostId !== null && member.hostId === selectedId ? "selected" : "reference",
      label: "",
      labelAt: first,
    });
  }
  return paths;
}

/** The reference grid's rings out to the fitted radius, unlabelled: the scale bar reads the scale. */
function planeRings(fitRadiusAu: number, spacingAu: number): ReadonlyArray<PlaneRing> {
  const rings: PlaneRing[] = [];
  for (let ring = 1; ring * spacingAu <= fitRadiusAu * (1 + 1e-12); ring += 1) {
    rings.push({ radius: ring * spacingAu, label: "" });
  }
  return rings;
}

/** What an orbit map's scene is built from. */
export interface OrbitSceneInput {
  readonly hosts: ReadonlyArray<HostBody>;
  readonly layout: HierarchyLayout;
  readonly plane: OrbitPlane;
  readonly time: UniverseTime;
  /** The selected body; a selection that names no drawn body draws no reticle. */
  readonly selectedId: string | null;
  readonly bands: ReadonlyArray<LayerBand>;
  /** The radius the view fits, which the grid covers. */
  readonly fitRadiusAu: number;
}

/**
 * The orbit map at the display time, as the scene the spatial view draws: the hosts, their paths,
 * and the reference plane's grid out to the fitted radius.
 */
export function orbitScene(input: OrbitSceneInput): SpatialScene {
  const { hosts, layout, plane, time, selectedId, bands, fitRadiusAu } = input;
  const points = hostMarks(hosts, layout, time, bands);
  const spacingAu = gridSpacing(fitRadiusAu);
  return {
    frame: plane.frame,
    points,
    spheres: [],
    plane: { spacing: spacingAu, extent: fitRadiusAu, rings: planeRings(fitRadiusAu, spacingAu) },
    paths: orbitPaths(hosts, layout, time, selectedId),
    selectedId: points.some((mark) => mark.id === selectedId) ? selectedId : null,
    destinationId: null,
  };
}
