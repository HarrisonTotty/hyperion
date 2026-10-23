/**
 * The orbit map in a body's frame: a planet focused with `FOCUS BODY`, its moons and rings drawn
 * about it (plan 14, P14.T42.b).
 *
 * @remarks
 * Pure functions of the model, beside `orbitMap.ts`'s system frame. The scene is still in
 * astronomical units, now about the planet's centre along the galactic axes, since a satellite's
 * elements on the wire are in its planet's body frame, a translation of the system frame (plan 14,
 * phase D): a moon's offset from its planet is its own orbit's position, and nothing is rotated.
 * The reference plane is the planet's equatorial plane, normal to its rotation section's pole, or
 * to its orbit's normal without the section (plan R02, Design note 14; plan 14, P14.T56.b). Each
 * ring lies in its own plane, the one its normal gives (P14.T56.a), or its planet's orbital plane
 * where an older server sends no normal.
 */
import type { UniverseTime } from "@hyperion/protocol";

import { orbitPolyline, positionAt } from "../../lib/orbit";
import { bodySymbol } from "../../lib/system/bodySymbols";
import { bodyKindLabel, bodyStateLabel, sectionStateLabel } from "../../lib/system/bodyWords";
import { orbitNormal } from "../../lib/system/hierarchy";
import type { BodyOrbit, SystemBody } from "../../lib/system/model";
import { type LocalFrame, planeFrame } from "../../geometry/frame";
import type {
  AnnulusMark,
  PathMark,
  PlaneRing,
  PointMark,
  SpatialScene,
} from "../../spatial/marks";
import { gridSpacing } from "../../spatial/scale";
import { scale, type Vec3 } from "../../geometry/vec3";
import { populationName } from "./bodyMap";
import type { OrbitPlane } from "./orbitMap";
import { METRES_PER_AU } from "./orbitScale";

/** Points of a moon's drawn orbit, every 2° of eccentric anomaly, as a planet's are. */
const ORBIT_SEGMENTS = 180;

/**
 * The radius a focused body with no moon or ring fits, in its own radii, or in metres when its
 * bulk is withheld: enough to show it alone on a grid for scale.
 */
export const LONE_BODY_FIT_RADII = 10;

/** The radius a focused body with no moon or ring fits when its bulk is withheld, m: 100 Mm. */
export const LONE_BODY_FIT_M = 1e8;

/** The name of a body's frame, as the view's `FRAME` reads it: `BODY <designation>`. */
export function bodyFrameName(body: SystemBody): string {
  return `BODY ${body.designation}`;
}

/** Whether a body can be focused: a planet or a dwarf planet, present, on an orbit. */
function canFocus(body: SystemBody): boolean {
  return (
    (body.kind.kind === "planet" || body.kind.kind === "dwarf_planet") &&
    body.state.kind === "present" &&
    body.orbit.state === "ok"
  );
}

/**
 * The body `FOCUS BODY` focuses for a selection: the planet or dwarf planet selected, or the one
 * a selected moon or ring belongs to; `null` for any other selection, or a body not present or on
 * no orbit.
 */
export function focusTarget(
  selected: SystemBody | null,
  bodies: ReadonlyArray<SystemBody>,
): SystemBody | null {
  if (selected === null) {
    return null;
  }
  if (canFocus(selected)) {
    return selected;
  }
  const parent = selected.parent;
  if ((selected.kind.kind === "moon" || selected.kind.kind === "ring") && parent?.kind === "body") {
    const planet = bodies.find((body) => body.id === parent.id);
    return planet !== undefined && canFocus(planet) ? planet : null;
  }
  return null;
}

/**
 * Why `FOCUS BODY` is held back for a selection, in words, or `null` when it is not: `KIND NOT
 * RESOLVED` for an unresolved contact, which may be a planet (ruling 113.5); `NO PLANET SELECTED`
 * for a selection that is not a planet, a dwarf planet, or a moon's or ring's; the planet by its
 * kind and state when it is not present (`PLANET DESTROYED`, `DWARF PLANET NOT YET FORMED`); the
 * orbit's section state, `ORBIT NOT RESOLVED` or `ORBIT NOT YET MODELLED`, when its orbit is
 * withheld or not computed (the UX advisor's ruling for ui11; ruling 113.5).
 */
export function focusHeldReason(
  selected: SystemBody | null,
  bodies: ReadonlyArray<SystemBody>,
): string | null {
  if (focusTarget(selected, bodies) !== null) {
    return null;
  }
  if (selected?.kind.kind === "unresolved") {
    return `KIND ${sectionStateLabel("not_resolved")}`;
  }
  let planet: SystemBody | null = null;
  if (selected !== null) {
    const parent = selected.parent;
    if (selected.kind.kind === "planet" || selected.kind.kind === "dwarf_planet") {
      planet = selected;
    } else if (
      (selected.kind.kind === "moon" || selected.kind.kind === "ring") &&
      parent?.kind === "body"
    ) {
      planet = bodies.find((body) => body.id === parent.id) ?? null;
    }
  }
  if (planet === null) {
    return "NO PLANET SELECTED";
  }
  if (planet.state.kind !== "present") {
    return `${bodyKindLabel(planet.kind)} ${bodyStateLabel(planet.state)}`;
  }
  const orbit = planet.orbit.state;
  return orbit === "not_resolved" || orbit === "not_modelled"
    ? `ORBIT ${sectionStateLabel(orbit)}`
    : "NO PLANET SELECTED";
}

/**
 * The normal of a body's equatorial plane, a unit vector along the galactic axes: its rotation
 * section's pole; or, where the server does not model its rotation, its orbit's normal, which
 * stands for its pole (plan R02, Design note 14); `null` where its rotation is withheld or does
 * not apply, or where the orbit that would stand for it is not given.
 *
 * @remarks
 * The pole is the spin axis by the right-hand rule, so a body spinning retrograde has its
 * equator's north on the far side from its orbit's, and its moons, prograde about its spin, are
 * filled above it (plan 14, P14.T56.a). A withheld rotation is not stood in for, so that no
 * reading is taken to a plane the display does not have.
 */
export function equatorNormal(body: SystemBody): Vec3 | null {
  let normal: Vec3 | null;
  switch (body.rotation.state) {
    case "ok":
      normal = body.rotation.value.pole;
      break;
    case "not_modelled":
      normal = body.orbit.state === "ok" ? orbitNormal(body.orbit.value.orbit) : null;
      break;
    case "not_resolved":
    case "not_applicable":
      normal = null;
      break;
  }
  return normal;
}

/**
 * A focused body's reference plane: its equatorial plane ({@link equatorNormal}), with the
 * galactic coreward laid onto it, as the system's plane is (D21).
 *
 * @remarks
 * Where the body's rotation is withheld the map still needs a plane, and its orbital plane stands
 * in, as for a body whose rotation is not modelled. No server withholds it yet, since the display
 * asks for every level; how the plane is then labelled waits for the knowledge overlay (plan 14,
 * P14.T56.b's record).
 *
 * @param galactic - The galactic directions at the system, whose coreward the plane's follows.
 */
export function bodyPlane(body: SystemBody, galactic: LocalFrame): OrbitPlane {
  const north =
    equatorNormal(body) ?? (body.orbit.state === "ok" ? orbitNormal(body.orbit.value.orbit) : null);
  if (north === null) {
    // focusTarget chooses only a body on an orbit.
    throw new Error(`the focused body ${body.id} has no orbit`);
  }
  return {
    frame: planeFrame(north, galactic.coreward),
    name: "EQUATORIAL PLANE",
    isSystemPlane: true,
  };
}

/** The satellites of a body: what names it as its parent, moons and rings alike. */
function satellitesOf(
  body: SystemBody,
  bodies: ReadonlyArray<SystemBody>,
): ReadonlyArray<SystemBody> {
  return bodies.filter(
    (candidate) => candidate.parent?.kind === "body" && candidate.parent.id === body.id,
  );
}

/** A moon's orbit about the focused body, when it is present and its orbit is on show. */
function moonOrbit(moon: SystemBody, body: SystemBody): BodyOrbit | null {
  if (moon.kind.kind !== "moon" || moon.state.kind !== "present" || moon.orbit.state !== "ok") {
    return null;
  }
  const orbit = moon.orbit.value;
  return orbit.parent.kind === "body" && orbit.parent.id === body.id ? orbit : null;
}

/**
 * The radius the view fits about a focused body, AU: the farthest its moons' apoapses and its
 * rings' outer edges reach, or, with neither, {@link LONE_BODY_FIT_RADII} of its radius, or
 * {@link LONE_BODY_FIT_M} with its bulk withheld.
 */
export function bodyFitAu(body: SystemBody, bodies: ReadonlyArray<SystemBody>): number {
  let farthestM = 0;
  for (const satellite of satellitesOf(body, bodies)) {
    const orbit = moonOrbit(satellite, body);
    if (orbit !== null) {
      farthestM = Math.max(farthestM, orbit.orbit.semiMajorAxisM * (1 + orbit.orbit.eccentricity));
    }
    const population = satellite.population.state === "ok" ? satellite.population.value : null;
    if (population?.kind === "ring" && satellite.state.kind === "present") {
      farthestM = Math.max(farthestM, population.outerEdgeM);
    }
  }
  if (farthestM > 0) {
    return farthestM / METRES_PER_AU;
  }
  const loneM =
    body.bulk.state === "ok" ? LONE_BODY_FIT_RADII * body.bulk.value.radiusM : LONE_BODY_FIT_M;
  return loneM / METRES_PER_AU;
}

/** A position in metres as scene units, astronomical units. */
function toAu(positionM: Vec3): Vec3 {
  return scale(positionM, 1 / METRES_PER_AU);
}

/** The reference grid's rings out to the fitted radius, unlabelled, as the system frame's are. */
function planeRings(fitRadiusAu: number, spacingAu: number): ReadonlyArray<PlaneRing> {
  const rings: PlaneRing[] = [];
  for (let ring = 1; ring * spacingAu <= fitRadiusAu * (1 + 1e-12); ring += 1) {
    rings.push({ radius: ring * spacingAu, label: "" });
  }
  return rings;
}

/** What a body frame's scene is built from. */
export interface BodySceneInput {
  /** The focused body, which {@link focusTarget} chose. */
  readonly body: SystemBody;
  /** Every body of the system, among which its moons and rings are found. */
  readonly bodies: ReadonlyArray<SystemBody>;
  readonly plane: OrbitPlane;
  readonly time: UniverseTime;
  /** The selected body; one not drawn here draws no reticle. */
  readonly selectedId: string | null;
  /** The radius the view fits, which the grid covers. */
  readonly fitRadiusAu: number;
}

/**
 * The orbit map in a focused body's frame at the display time: the body at the centre, its moons
 * on their orbits, each orbit a path, the selected moon's `selected`, and its rings as annuli,
 * each in its own plane, labelled, their two edges joined by radial ticks every 10° as the guide
 * draws a belt and a ring, the selected ring's drawn as a selected orbit is (plan 14, P14.T42.a–b),
 * each drawn about the body, whose bracket's box its label need not keep outside while the body is
 * not the selection (P14.T42.g). A ring's plane is the one its normal gives, close to the
 * equatorial plane well inside the planet's Laplace radius (P14.T56.a), or the planet's orbital
 * plane where an older server sends no normal, so that each is drawn where the server places it.
 *
 * @remarks
 * A moon or ring not present is not drawn; it stays in the list. Nothing beyond the body's own
 * satellites is drawn: the rest of the system lies far outside the view.
 */
export function bodyScene(input: BodySceneInput): SpatialScene {
  const { body, bodies, plane, time, selectedId, fitRadiusAu } = input;
  // Where a ring's normal is not sent, the ring lies in its planet's orbital plane.
  const orbitalNormal = body.orbit.state === "ok" ? orbitNormal(body.orbit.value.orbit) : undefined;
  const points: PointMark[] = [];
  const paths: PathMark[] = [];
  const annuli: AnnulusMark[] = [];
  const symbol = bodySymbol(body);
  if (symbol !== null) {
    points.push({
      id: body.id,
      position: { x: 0, y: 0, z: 0 },
      shape: symbol.shape,
      sizeClass: symbol.sizeClass,
      status: "plain",
      label: body.designation,
      labelPriority: Number.MAX_VALUE,
    });
  }
  for (const satellite of satellitesOf(body, bodies)) {
    const orbit = moonOrbit(satellite, body);
    const moonSymbol = bodySymbol(satellite);
    if (orbit !== null && moonSymbol !== null) {
      points.push({
        id: satellite.id,
        position: toAu(positionAt(orbit.orbit, time)),
        shape: moonSymbol.shape,
        sizeClass: moonSymbol.sizeClass,
        status: "plain",
        label: satellite.designation,
        labelPriority: satellite.massMearth.state === "ok" ? satellite.massMearth.value : 0,
      });
      const line = orbitPolyline(orbit.orbit, ORBIT_SEGMENTS).map(toAu);
      const [first] = line;
      if (first !== undefined) {
        paths.push({
          id: satellite.id,
          points: line,
          role: satellite.id === selectedId ? "selected" : "reference",
          label: "",
          labelAt: first,
        });
      }
    }
    const population = satellite.population.state === "ok" ? satellite.population.value : null;
    if (population?.kind === "ring" && satellite.state.kind === "present") {
      annuli.push({
        id: `ring:${satellite.id}`,
        innerRadius: population.innerEdgeM / METRES_PER_AU,
        outerRadius: population.outerEdgeM / METRES_PER_AU,
        normal: population.normal ?? orbitalNormal,
        ticks: true,
        label: populationName(satellite),
        // A second ring lies about the first, whose label stands rimward.
        labelSpinward: annuli.length % 2 === 1,
        selected: satellite.id === selectedId,
        // Its label names a ring (`populationName`), never the planet, which is why it may stand
        // inside the planet's bracket's box while no bracket is drawn there (plan 14, P14.T42.g).
        aboutId: body.id,
      });
    }
  }
  const spacingAu = gridSpacing(fitRadiusAu);
  return {
    frame: plane.frame,
    points,
    spheres: [],
    plane: { spacing: spacingAu, extent: fitRadiusAu, rings: planeRings(fitRadiusAu, spacingAu) },
    paths,
    annuli,
    selectedId: points.some((mark) => mark.id === selectedId) ? selectedId : null,
    destinationId: null,
  };
}
