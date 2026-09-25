/**
 * The pure model behind the orbit map: a system at the display time as a scene for the general
 * spatial view (plan 14, P14.T42).
 *
 * @remarks
 * Nothing here touches the DOM or the server. The scene's unit is the astronomical unit, its
 * positions offsets from the system's barycentre along the galactic axes, and its frame the
 * system's reference plane (D21). Hosts are drawn with the registry's symbols at their mass layer's
 * size, each where its chain of orbits puts it at the display time; each star's path about its
 * pair's barycentre is drawn through it, and the selected body's path in `--text` 2 px wide. The
 * bodies `system_bodies` sends, and their zones, join the same scene through `bodyMap.ts`.
 */
import type { SystemIdHex, UniverseTime } from "@hyperion/protocol";

import { starSizeClass, starSymbol } from "../../lib/galaxy/starSymbols";
import type { LayerIndex } from "../../lib/galaxy/model";
import { composePosition, orbitPolyline } from "../../lib/orbit";
import { type HierarchyLayout, orbitNormal } from "../../lib/system/hierarchy";
import type { HostBody, SystemBodies, SystemPlane } from "../../lib/system/model";
import { type LocalFrame, localFrameAt, planeFrame } from "../../spatial/frame";
import type { PathMark, PlaneRing, PointMark, SpatialScene } from "../../spatial/marks";
import { gridSpacing } from "../../spatial/scale";
import { add, scale, type Vec3 } from "../../spatial/vec3";
import type { LayerBand } from "../galaxy/chartModel";
import {
  type BodiesLayout,
  bodyMarks,
  bodyPaths,
  populationAnnuli,
  type ZoneLayers,
  zoneAnnuli,
} from "./bodyMap";
import { METRES_PER_AU } from "./orbitScale";

/** A zoom preset of the orbit map, which sets the radius the view fits. */
export type ZoomPreset = "inner" | "all" | "belts";

/** A zoom preset's control: the preset, its label and its single key. */
export interface ZoomPresetControl {
  readonly name: ZoomPreset;
  readonly label: string;
  readonly key: string;
}

/**
 * The orbit map's zoom presets in the order they are offered: `INNER`, `ALL` and `BELTS` (plan 14,
 * P14.T42.b). `BELTS` is offered only for a system with a belt to fit.
 */
export const ZOOM_PRESETS: ReadonlyArray<ZoomPresetControl> = [
  { name: "inner", label: "INNER", key: "I" },
  { name: "all", label: "ALL", key: "A" },
  { name: "belts", label: "BELTS", key: "B" },
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
 * The orbit map's reference plane, its coreward the galactic coreward laid onto it (D21;
 * `planeFrame`): the system's plane as the server gives it with the bodies; before there are bodies,
 * the plane of the innermost pair that holds the primary, its normal along the pair's angular
 * momentum; or, for a single star, the galactic plane at the system.
 *
 * @remarks
 * D21's plane is the primary's planetary plane, or a close binary's, which the server chooses
 * (`SystemBodiesDto.system_plane`). Until the bodies arrive, or for a system with no zone, the
 * stars' innermost orbit about the primary is the only plane the system has; a single star has
 * none, and its map is drawn on the galactic plane, which the legend names as such (the
 * orchestrator's ruling 59.2).
 *
 * @param positionLy - The system's position in the `GALACTIC` frame.
 * @param systemPlane - The server's system plane, or `null` before the bodies or without a zone.
 */
export function orbitPlane(
  layout: HierarchyLayout,
  positionLy: Vec3,
  systemPlane: SystemPlane | null = null,
): OrbitPlane {
  const galactic = localFrameAt(positionLy);
  if (systemPlane !== null) {
    return {
      frame: planeFrame(orbitNormal(systemPlane), galactic.coreward),
      name: "SYSTEM PLANE",
      isSystemPlane: true,
    };
  }
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

/** The count of bodies `INNER` fits when there is no habitable zone to fit: the fifth body. */
export const INNER_BODY_COUNT = 5;

/**
 * What the zoom presets fit besides the stars: the bodies' reaches, the habitable zone's and the
 * outermost belt's.
 */
export interface BodyFit {
  /** The farthest each drawn body other than a moon can be from the barycentre, AU, nearest first. */
  readonly bodyReachesAu: ReadonlyArray<number>;
  /** How far out the primary's habitable zone reaches from the barycentre, AU; `null` for none. */
  readonly habitableOuterAu: number | null;
  /** How far out the outermost belt proper reaches from the barycentre, AU; `null` for none. */
  readonly beltsOuterAu: number | null;
}

/**
 * The radius each zoom preset fits (plan 14, P14.T42.b).
 *
 * @remarks
 * A reach is the sum of each orbit's apoapsis distance up a body's chain, the star's share of it
 * for a star, which bounds where it can be at any time, so that neither preset moves as the display
 * time does.
 *
 * With bodies, `INNER` fits the outer limit of the primary's habitable zone or the fifth body out,
 * whichever is nearer, so that it shows the inner system however the planets fall; with neither, it
 * fits the stars as below. `ALL` fits the farthest of the bodies, the stars and `INNER`'s radius,
 * so that everything drawn is in it. `BELTS` fits the outermost belt proper, and `ALL`'s radius in
 * a system with no belt, where it is not offered.
 *
 * Without bodies, as before `system_bodies` answers, `ALL` fits the farthest any drawn star's orbits
 * can take it from the barycentre and `INNER` the nearest such reach that is not zero, so that a
 * binary's primary fills `INNER` and both stars fit `ALL` (the orchestrator's ruling 59.4); a system
 * with nothing off its barycentre fits {@link LONE_STAR_FIT_AU}.
 */
export function fitRadiiAu(
  layout: HierarchyLayout,
  hosts: ReadonlyArray<HostBody>,
  bodies: BodyFit | null = null,
): FitRadii {
  const reaches = hosts
    .filter((host) => starSymbol(host.kind) !== null)
    .map((host) => (layout.reachM.get(host.id) ?? 0) / METRES_PER_AU)
    .filter((reach) => reach > 0);
  const bodyReaches = bodies?.bodyReachesAu ?? [];
  const innerCandidates = [
    bodies?.habitableOuterAu ?? null,
    bodyReaches[INNER_BODY_COUNT - 1] ?? bodyReaches.at(-1) ?? null,
  ].filter((radius): radius is number => radius !== null && radius > 0);
  let inner: number;
  if (innerCandidates.length > 0) {
    inner = Math.min(...innerCandidates);
  } else if (reaches.length > 0) {
    inner = Math.min(...reaches);
  } else {
    inner = LONE_STAR_FIT_AU;
  }
  const farthest = [...reaches, ...bodyReaches];
  const all =
    farthest.length === 0 && innerCandidates.length === 0
      ? LONE_STAR_FIT_AU
      : Math.max(inner, ...farthest);
  return { inner, all, belts: bodies?.beltsOuterAu ?? all };
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

/** The bodies an orbit map's scene draws beside its hosts, and which of their zones. */
export interface OrbitSceneBodies {
  /** The system the bodies are of, whose ID their hosts' IDs extend. */
  readonly system: SystemIdHex;
  readonly bodies: SystemBodies;
  readonly layout: BodiesLayout;
  readonly zoneLayers: ZoneLayers;
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
  /** The system's bodies, from `system_bodies`; `null` before them, or while the kind is unserved. */
  readonly bodies: OrbitSceneBodies | null;
}

/**
 * The orbit map at the display time, as the scene the spatial view draws: the hosts, the bodies,
 * their paths, the zones' and the belts' annuli, the cometary halo when it is inside the fitted
 * radius, and the reference plane's grid out to that radius.
 */
export function orbitScene(input: OrbitSceneInput): SpatialScene {
  const { hosts, layout, plane, time, selectedId, bands, fitRadiusAu, bodies } = input;
  const bodyScene =
    bodies === null
      ? null
      : {
          points: bodyMarks(bodies.bodies.bodies, bodies.layout, time),
          paths: bodyPaths(bodies.bodies.bodies, bodies.layout, time, selectedId),
          annuli: [
            ...zoneAnnuli(bodies.bodies.zones, bodies.system, layout, time, bodies.zoneLayers),
            ...populationAnnuli(bodies.bodies.bodies, bodies.system, layout, time, fitRadiusAu),
          ],
        };
  const points = [...hostMarks(hosts, layout, time, bands), ...(bodyScene?.points ?? [])];
  const spacingAu = gridSpacing(fitRadiusAu);
  return {
    frame: plane.frame,
    points,
    spheres: [],
    plane: { spacing: spacingAu, extent: fitRadiusAu, rings: planeRings(fitRadiusAu, spacingAu) },
    paths: [...orbitPaths(hosts, layout, time, selectedId), ...(bodyScene?.paths ?? [])],
    ...(bodyScene === null ? {} : { annuli: bodyScene.annuli }),
    selectedId: points.some((mark) => mark.id === selectedId) ? selectedId : null,
    destinationId: null,
  };
}
