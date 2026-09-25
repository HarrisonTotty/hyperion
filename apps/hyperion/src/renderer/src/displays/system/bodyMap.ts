/**
 * The orbit map's bodies and zones: where each body is at the display time, the paths their orbits
 * trace, and the zones' annuli (plan 14, P14.T42.a).
 *
 * @remarks
 * Pure functions of the model, beside `orbitMap.ts`'s hosts. Positions are in metres in the
 * galactic axes about the system's barycentre, as `lib/orbit.ts` gives them, and the marks in the
 * scene's astronomical units. A body is placed on its orbit about what it orbits: a star, a pair's
 * barycentre, the system's, or another body; a population (a belt, a ring, a disc, the halo) has no
 * single position, so a member of one orbits what the population orbits. A body that is not present
 * is not drawn, and stays in the list.
 */
import { formatBodyId, type SystemIdHex, type UniverseTime } from "@hyperion/protocol";

import { type BodyPlacement, composePosition, orbitPolyline } from "../../lib/orbit";
import { bodySymbol } from "../../lib/system/bodySymbols";
import { bodyKindLabel, ringKindLabel } from "../../lib/system/bodyWords";
import type { HierarchyLayout } from "../../lib/system/hierarchy";
import type { BodyKind, OrbitHost, SystemBody, Zone } from "../../lib/system/model";
import type { AnnulusMark, PathMark, PointMark } from "../../spatial/marks";
import { add, norm, scale, type Vec3 } from "../../spatial/vec3";
import { METRES_PER_AU } from "./orbitScale";

/** Points of a body's drawn orbit, every 2° of eccentric anomaly, as the stars' are. */
const ORBIT_SEGMENTS = 180;

/** Earth masses in a solar mass, by the nominal GM of each: 332,946. */
const MEARTH_PER_MSUN = 332_946;

/** Where the map places every body it can, beside the hosts. */
export interface BodiesLayout {
  /** The hosts' placements and every placed body's, for `composePosition`. */
  readonly placements: ReadonlyMap<string, BodyPlacement>;
  /** The placement key of what each placed body orbits, by the body's ID. */
  readonly parentKeys: ReadonlyMap<string, string>;
  /** The largest distance from the barycentre each placed body's chain of orbits allows, m. */
  readonly reachM: ReadonlyMap<string, number>;
}

/** Whether a kind of body is a population, which has no single position and takes no symbol. */
export function isPopulation(kind: BodyKind): boolean {
  let result: boolean;
  switch (kind.kind) {
    case "ring":
    case "belt":
    case "cometary_halo":
    case "protoplanetary_disc":
    case "debris_disc":
      result = true;
      break;
    case "planet":
    case "dwarf_planet":
    case "moon":
    case "unresolved":
      result = false;
      break;
  }
  return result;
}

/**
 * The placement key of a star, a pair or the barycentre, as the hierarchy's layout names it, or
 * `null` for one it does not hold.
 */
export function hostKeyOf(
  host: OrbitHost,
  system: SystemIdHex,
  layout: HierarchyLayout,
): string | null {
  let key: string | null;
  switch (host.kind) {
    case "star": {
      const id = formatBodyId({ system, bodyIndex: host.bodyIndex });
      key = layout.placements.has(id) ? id : null;
      break;
    }
    case "pair":
      key = layout.pairKeyedBy.get(formatBodyId({ system, bodyIndex: host.keyBodyIndex })) ?? null;
      break;
    case "barycentre":
      key = layout.rootKey;
      break;
    case "body":
      key = null;
      break;
  }
  return key;
}

/**
 * Places every body that is present and has an orbit on its orbit about what it orbits.
 *
 * @remarks
 * What a body orbits is its orbit's parent. A parent that is a population is passed through to
 * what the population orbits, since a belt's member orbits the belt's star; a parent that is a
 * body not placed leaves its child unplaced. The chain of parents is the one `toSystemBodiesModel`
 * accepted, which ends at a host; the walk is bounded all the same.
 */
export function layoutBodies(
  system: SystemIdHex,
  bodies: ReadonlyArray<SystemBody>,
  layout: HierarchyLayout,
): BodiesLayout {
  const byId = new Map(bodies.map((body) => [body.id, body]));
  const placements = new Map(layout.placements);
  const parentKeys = new Map<string, string>();
  const reachM = new Map<string, number>();

  const keyOf = (host: OrbitHost, depth: number): string | null => {
    if (host.kind !== "body") {
      return hostKeyOf(host, system, layout);
    }
    const parent = byId.get(host.id);
    if (parent === undefined || depth > bodies.length) {
      return null;
    }
    if (isPopulation(parent.kind)) {
      return parent.parent === null ? null : keyOf(parent.parent, depth + 1);
    }
    return place(parent, depth + 1) ? parent.id : null;
  };

  function place(body: SystemBody, depth: number): boolean {
    if (placements.has(body.id)) {
      return true;
    }
    if (body.state.kind !== "present" || body.orbit.state !== "ok") {
      return false;
    }
    const { parent, orbit } = body.orbit.value;
    const parentKey = keyOf(parent, depth);
    if (parentKey === null) {
      return false;
    }
    placements.set(body.id, { kind: "orbit", parentId: parentKey, orbit });
    parentKeys.set(body.id, parentKey);
    const parentReach = layout.reachM.get(parentKey) ?? reachM.get(parentKey) ?? 0;
    reachM.set(body.id, parentReach + orbit.semiMajorAxisM * (1 + orbit.eccentricity));
    return true;
  }

  for (const body of bodies) {
    place(body, 0);
  }
  return { placements, parentKeys, reachM };
}

/**
 * Where a body is at the display time, m, or `null` where it is not drawn: on its orbit when it is
 * placed, or, with its orbit withheld, where the server put it at the answer's time.
 *
 * @remarks
 * At the `contact` detail level only a position crosses, so a contact stands where it was when the
 * system was last asked about, until the display asks again.
 */
export function bodyPositionM(
  body: SystemBody,
  bodiesLayout: BodiesLayout,
  time: UniverseTime,
): Vec3 | null {
  if (body.state.kind !== "present") {
    return null;
  }
  if (bodiesLayout.parentKeys.has(body.id)) {
    return composePosition(bodiesLayout.placements, body.id, time);
  }
  return body.positionM;
}

/** A position in metres as scene units, astronomical units. */
function toAu(positionM: Vec3): Vec3 {
  return scale(positionM, 1 / METRES_PER_AU);
}

/**
 * Every drawn body as a point mark at the display time: its symbol, labelled with its
 * designation, heavier bodies labelled first, after every star.
 *
 * @remarks
 * Moons are left to the frame of their planet, which `FOCUS BODY` draws (P14.T42.b): at the
 * system's scale a moon lies under its planet's symbol, where it would take the planet's label and
 * its pick.
 */
export function bodyMarks(
  bodies: ReadonlyArray<SystemBody>,
  bodiesLayout: BodiesLayout,
  time: UniverseTime,
): ReadonlyArray<PointMark> {
  const marks: PointMark[] = [];
  for (const body of bodies) {
    const symbol = body.kind.kind === "moon" ? null : bodySymbol(body);
    const positionM = symbol === null ? null : bodyPositionM(body, bodiesLayout, time);
    if (symbol === null || positionM === null) {
      continue;
    }
    marks.push({
      id: body.id,
      position: toAu(positionM),
      shape: symbol.shape,
      sizeClass: symbol.sizeClass,
      status: "plain",
      label: body.designation,
      labelPriority: body.massMearth.state === "ok" ? body.massMearth.value / MEARTH_PER_MSUN : 0,
    });
  }
  return marks;
}

/**
 * The path each placed body's orbit traces about what it orbits, as it is at the display time: the
 * selected body's is the one `selected` path, and the rest are `reference` (plan 14, P14.T42.a).
 *
 * @remarks
 * A moon's orbit is left to the body frame that `FOCUS BODY` draws (P14.T42.b), since at the
 * system's scale it would lie under its planet's symbol. Paths are not labelled: the marks on them
 * are.
 */
export function bodyPaths(
  bodies: ReadonlyArray<SystemBody>,
  bodiesLayout: BodiesLayout,
  time: UniverseTime,
  selectedId: string | null,
): ReadonlyArray<PathMark> {
  const paths: PathMark[] = [];
  for (const body of bodies) {
    const parentKey = bodiesLayout.parentKeys.get(body.id);
    if (parentKey === undefined || body.orbit.state !== "ok" || body.kind.kind === "moon") {
      continue;
    }
    const aboutM = composePosition(bodiesLayout.placements, parentKey, time);
    const points = orbitPolyline(body.orbit.value.orbit, ORBIT_SEGMENTS).map((pointM) =>
      toAu(add(aboutM, pointM)),
    );
    const [first] = points;
    if (first === undefined) {
      continue;
    }
    paths.push({
      id: body.id,
      points,
      role: body.id === selectedId ? "selected" : "reference",
      label: "",
      labelAt: first,
    });
  }
  return paths;
}

/**
 * The farthest each drawn body other than a moon can be from the barycentre, AU, nearest first: a
 * placed body's reach along its chain of orbits, or the distance of a contact that stands where
 * the server put it.
 */
export function bodyReachesAu(
  bodies: ReadonlyArray<SystemBody>,
  bodiesLayout: BodiesLayout,
): ReadonlyArray<number> {
  const reaches: number[] = [];
  for (const body of bodies) {
    if (bodySymbol(body) === null || body.kind.kind === "moon" || body.state.kind !== "present") {
      continue;
    }
    const reachM =
      bodiesLayout.reachM.get(body.id) ?? (body.positionM === null ? null : norm(body.positionM));
    if (reachM !== null && reachM > 0) {
      reaches.push(reachM / METRES_PER_AU);
    }
  }
  return reaches.toSorted((a, b) => a - b);
}

/** Which of the zones' annuli the map draws: each can be switched off (plan 14, P14.T42.a). */
export interface ZoneLayers {
  readonly stable: boolean;
  readonly snowLine: boolean;
  readonly habitable: boolean;
  /** The optimistic habitable zone, from recent Venus to early Mars (ruling 65.4). */
  readonly optimistic: boolean;
}

/** Every zone annulus drawn, as the map opens. */
export const ALL_ZONE_LAYERS: ZoneLayers = {
  stable: true,
  snowLine: true,
  habitable: true,
  optimistic: true,
};

/**
 * The zones as annuli on the reference plane about their hosts at the display time, labelled: each
 * zone's stable limits (`STABLE ZONE`), its snow line (`SNOW LINE`), its conservative habitable
 * zone (`HABITABLE ZONE`) and its optimistic one (`OPTIMISTIC`), drawn in `--text-muted` as the
 * orchestrator's ruling 35.6 has it.
 *
 * @remarks
 * A stable zone is drawn where its companions bound it: an edge the disc bounds (`null`) is not
 * drawn, and a zone bounded on neither side, a single star's, draws nothing. A habitable zone runs
 * from the moist greenhouse to the maximum greenhouse; an inner edge beyond every orbit leaves no
 * zone, an outer one beyond every orbit leaves its inner edge alone, and a host with no light, whose
 * limits are all zero, draws none. The optimistic zone runs from recent Venus to early Mars, the
 * same fit's other pair of limits (Kopparapu et al. 2013, 2014), which the server sends with the
 * conservative pair, by the same rules; its edges carry short ticks into the band, so that it
 * differs from the conservative pair by shape and not by colour (ruling 65.4). The annuli lie in the system's plane, which is a companion's own
 * planets' plane only approximately.
 */
export function zoneAnnuli(
  zones: ReadonlyArray<Zone>,
  system: SystemIdHex,
  layout: HierarchyLayout,
  time: UniverseTime,
  layers: ZoneLayers,
): ReadonlyArray<AnnulusMark> {
  const annuli: AnnulusMark[] = [];
  for (const [index, zone] of zones.entries()) {
    const key = hostKeyOf(zone.host, system, layout);
    if (key === null) {
      continue;
    }
    const centre = toAu(composePosition(layout.placements, key, time));
    const band = (id: string, innerM: number, outerM: number, label: string): AnnulusMark => ({
      id: `zone:${index}:${id}`,
      centre,
      innerRadius: innerM / METRES_PER_AU,
      outerRadius: outerM / METRES_PER_AU,
      ticks: false,
      label,
    });
    if (layers.stable && (zone.innerM !== null || zone.outerM !== null)) {
      const innerM = zone.innerM ?? 0;
      annuli.push(band("stable", innerM, zone.outerM ?? innerM, "STABLE ZONE"));
    }
    if (layers.snowLine) {
      annuli.push(band("snow", zone.snowLineM, zone.snowLineM, "SNOW LINE"));
    }
    const habitable = zone.habitableZone;
    const innerM = habitable?.moistGreenhouseM ?? null;
    if (layers.habitable && innerM !== null && innerM > 0) {
      const outerM = habitable?.maximumGreenhouseM ?? innerM;
      annuli.push(band("habitable", innerM, Math.max(innerM, outerM), "HABITABLE ZONE"));
    }
    const optimisticInnerM = habitable?.recentVenusM ?? null;
    if (layers.optimistic && optimisticInnerM !== null && optimisticInnerM > 0) {
      const outerM = habitable?.earlyMarsM ?? optimisticInnerM;
      annuli.push({
        ...band("optimistic", optimisticInnerM, Math.max(optimisticInnerM, outerM), "OPTIMISTIC"),
        edgeTicks: true,
        // Its outer edge lies just beyond the conservative zone's, whose label stands rimward.
        labelSpinward: true,
      });
    }
  }
  return annuli;
}

/**
 * The zone that holds the primary, the innermost first: the primary's own, else the innermost
 * pair's that holds it, else the barycentre's; `null` when there is none. Its habitable zone is the
 * one `INNER` fits, and its class the system's (plan 14, P14.T42.b and T43.b).
 */
export function primaryZone(
  zones: ReadonlyArray<Zone>,
  system: SystemIdHex,
  layout: HierarchyLayout,
): Zone | null {
  const primaryId = formatBodyId({ system, bodyIndex: 0 });
  const holds = (zone: Zone): boolean => {
    let result: boolean;
    switch (zone.host.kind) {
      case "star":
        result = zone.host.bodyIndex === 0;
        break;
      case "pair": {
        const key = hostKeyOf(zone.host, system, layout);
        result = key !== null && (layout.pairStars.get(key)?.includes(primaryId) ?? false);
        break;
      }
      case "barycentre":
        result = true;
        break;
      case "body":
        result = false;
        break;
    }
    return result;
  };
  return zones.find(holds) ?? null;
}

/**
 * How far out the primary's zone's habitable zone reaches from the barycentre, AU: its host's reach
 * and the maximum greenhouse limit; `null` without one, or beyond every orbit.
 */
export function habitableOuterAu(
  zone: Zone | null,
  system: SystemIdHex,
  layout: HierarchyLayout,
): number | null {
  const outerM = zone?.habitableZone?.maximumGreenhouseM ?? null;
  if (zone === null || outerM === null || !(outerM > 0)) {
    return null;
  }
  const key = hostKeyOf(zone.host, system, layout);
  const hostReachM = key === null ? 0 : (layout.reachM.get(key) ?? 0);
  return (hostReachM + outerM) / METRES_PER_AU;
}

/**
 * What a population is called on the map: its label for people when it has one (`BELT 1`), and
 * its kind in words otherwise (`ASTEROID BELT`, `MASSIVE RING`), so that two rings unlabelled
 * still read apart.
 */
export function populationName(body: SystemBody): string {
  if (body.label.state === "ok") {
    return body.label.value;
  }
  const population = body.population.state === "ok" ? body.population.value : null;
  return population?.kind === "ring"
    ? `${ringKindLabel(population.ringKind)} RING`
    : bodyKindLabel(body.kind);
}

/** Where a belt's or the halo's host is at the display time, AU, or `null` for one not held. */
function hostCentreAu(
  host: OrbitHost,
  system: SystemIdHex,
  layout: HierarchyLayout,
  time: UniverseTime,
): Vec3 | null {
  const key = hostKeyOf(host, system, layout);
  return key === null ? null : toAu(composePosition(layout.placements, key, time));
}

/**
 * The belts as ticked annuli on the reference plane about their hosts at the display time, and the
 * cometary halo as a labelled ring when it is inside the view (plan 14, P14.T42.a and T38.a).
 *
 * @remarks
 * A belt proper is drawn as its two edges joined by radial ticks every 10°, labelled with its name;
 * a Kuiper-like belt's scattered component, which runs on beyond it, as its two edges alone,
 * labelled `SCATTERED DISC`, so that the two differ by shape. Both lie in the reference plane, which
 * is the host's zone's plane for the primary's belts and a companion's own only approximately, as
 * the zones' annuli do. The halo is a shell, drawn as the circles where it meets the plane: its
 * inner edge when that lies inside `viewRadiusAu`, the radius the view fits, labelled as its inner
 * edge, and its outer edge too when that does, labelled as the halo; beyond the view it draws nothing, since a ring off the map says nothing. A body
 * not present, or whose section is not `ok`, draws nothing.
 *
 * @param viewRadiusAu - The radius the view fits, from the barycentre.
 */
export function populationAnnuli(
  bodies: ReadonlyArray<SystemBody>,
  system: SystemIdHex,
  layout: HierarchyLayout,
  time: UniverseTime,
  viewRadiusAu: number,
): ReadonlyArray<AnnulusMark> {
  const annuli: AnnulusMark[] = [];
  for (const body of bodies) {
    const population = body.population.state === "ok" ? body.population.value : null;
    if (population === null || population.kind === "ring" || body.state.kind !== "present") {
      continue;
    }
    const centre = hostCentreAu(population.host, system, layout, time);
    if (centre === null) {
      continue;
    }
    const name = populationName(body);
    if (population.kind === "belt") {
      annuli.push({
        id: `belt:${body.id}`,
        centre,
        innerRadius: population.main.innerEdgeM / METRES_PER_AU,
        outerRadius: population.main.outerEdgeM / METRES_PER_AU,
        ticks: true,
        label: name,
        // Spinward, clear of the snow line's and the zones' labels on the rimward ray, which a belt
        // near the snow line would meet.
        labelSpinward: true,
      });
      if (population.scattered !== null) {
        annuli.push({
          id: `belt:${body.id}:scattered`,
          centre,
          innerRadius: population.scattered.innerEdgeM / METRES_PER_AU,
          outerRadius: population.scattered.outerEdgeM / METRES_PER_AU,
          ticks: false,
          label: "SCATTERED DISC",
        });
      }
      continue;
    }
    const hostKey = hostKeyOf(population.host, system, layout);
    const hostReachAu = (hostKey === null ? 0 : (layout.reachM.get(hostKey) ?? 0)) / METRES_PER_AU;
    const innerAu = population.innerEdgeM / METRES_PER_AU;
    const outerAu = population.outerEdgeM / METRES_PER_AU;
    if (hostReachAu + innerAu > viewRadiusAu) {
      continue;
    }
    const whole = hostReachAu + outerAu <= viewRadiusAu;
    annuli.push({
      id: `halo:${body.id}`,
      centre,
      innerRadius: innerAu,
      outerRadius: whole ? outerAu : innerAu,
      ticks: false,
      // A lone circle is the halo's inner edge, and says so.
      label: whole ? name : `${name} INNER EDGE`,
    });
  }
  return annuli;
}

/**
 * How far the outermost belt proper reaches from the barycentre, AU: its host's reach and its outer
 * edge; `null` for a system with no belt drawn. The radius `BELTS` fits (P14.T42.b).
 *
 * @remarks
 * A Kuiper-like belt's scattered component is left out, since it runs on to many times the belt's
 * radius and would leave the belt proper a small ring in the middle of the view.
 */
export function beltsOuterAu(
  bodies: ReadonlyArray<SystemBody>,
  system: SystemIdHex,
  layout: HierarchyLayout,
): number | null {
  let outerAu: number | null = null;
  for (const body of bodies) {
    const population = body.population.state === "ok" ? body.population.value : null;
    if (population?.kind !== "belt" || body.state.kind !== "present") {
      continue;
    }
    const key = hostKeyOf(population.host, system, layout);
    if (key === null) {
      continue;
    }
    const reachAu = ((layout.reachM.get(key) ?? 0) + population.main.outerEdgeM) / METRES_PER_AU;
    outerAu = Math.max(outerAu ?? 0, reachAu);
  }
  return outerAu;
}
