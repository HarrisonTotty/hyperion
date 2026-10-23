import type { ResponseFor, UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { positionAt } from "../../lib/orbit";
import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { MOON_SIZE_CLASS } from "../../lib/system/bodySymbols";
import { orbitNormal } from "../../lib/system/hierarchy";
import type { SystemBody } from "../../lib/system/model";
import { localFrameAt } from "../../geometry/frame";
import { dot, norm, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import {
  FIXTURE_RING_NORMAL,
  FIXTURE_SYSTEM,
  populatedBodies,
  populatedBodiesWith,
  tiltedPole,
  withBodyRotation,
  withoutRingNormal,
  withRingNormal,
} from "../../test/planetaryFixture";
import {
  bodyFitAu,
  bodyFrameName,
  bodyPlane,
  bodyScene,
  equatorNormal,
  focusHeldReason,
  focusTarget,
  LONE_BODY_FIT_RADII,
} from "./bodyFrame";
import { METRES_PER_AU } from "./orbitScale";

const TIME: UniverseTime = { seconds: 3_155_760_000, nanos: 0 };
const GALACTIC = localFrameAt(vec3(26_000, 0, 12));

/** The populated answer's planet `A b`, its moon `A b I` and its massive ring. */
const PLANET = `${FIXTURE_SYSTEM}.0100`;
const MOON = `${FIXTURE_SYSTEM}.0101`;
const RING = `${FIXTURE_SYSTEM}.0180`;
const BELT = `${FIXTURE_SYSTEM}.e000`;
const DESTROYED_PLANET = `${FIXTURE_SYSTEM}.0200`;

function bodiesOf(
  response: ResponseFor<"system_bodies"> = populatedBodies(),
): ReadonlyArray<SystemBody> {
  const result = toSystemBodiesModel(response, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return result.bodies.bodies;
}

function body(id: string, bodies = bodiesOf()): SystemBody {
  const found = bodies.find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error(`the populated answer lists ${id}`);
  }
  return found;
}

/** The tilt of {@link tiltedPlanet}'s pole from its planet's orbit normal, rad. */
const OBLIQUITY_RAD = 0.4;

/**
 * A close-in giant's ring's tilt from its planet's equator, rad: the local Laplace plane of
 * P14.T56.a's `laplace_angle`, tan 2φ = sin 2ε′ ÷ (cos 2ε′ + 2 (r_L ÷ a)⁵), with ε′ the acute
 * angle between the pole and the orbit's normal, for a ring at half its planet's Laplace radius;
 * about 5.5 × 10⁻³ rad at {@link OBLIQUITY_RAD}.
 */
function ringTiltRad(obliquityRad: number): number {
  const acute = Math.min(obliquityRad, Math.PI - obliquityRad);
  return 0.5 * Math.atan2(Math.sin(2 * acute), Math.cos(2 * acute) + 2 * 2 ** 5);
}

/** What {@link tiltedPlanet} builds. */
interface TiltedPlanet {
  readonly bodies: ReadonlyArray<SystemBody>;
  readonly pole: Vec3;
  readonly orbitNormal: Vec3;
  /** The ring's normal on the wire; `null` where it is left out, as below version 23. */
  readonly ringNormal: Vec3 | null;
}

/**
 * The populated answer with its planet `A b` spinning about a pole `obliquityRad` from its orbit's
 * normal (`tiltedPole`), and, with `ring`, its ring given the plane's normal a server from the
 * 22 → 23 batch sends for it: the local Laplace pole on the spin's side, turned
 * {@link ringTiltRad} from the pole towards the end of the orbit normal's axis within 90° of it;
 * without, its ring's normal left out, as an older server sends it.
 */
function tiltedPlanet({
  obliquityRad = OBLIQUITY_RAD,
  ring = false,
}: { readonly obliquityRad?: number; readonly ring?: boolean } = {}): TiltedPlanet {
  const tilted = tiltedPole(populatedBodies(), PLANET, obliquityRad);
  // Towards the orbit normal about a prograde spin, towards its other end about a retrograde one.
  const towardsAxis = obliquityRad <= Math.PI / 2 ? -1 : 1;
  const ringNormal = ring
    ? tilted.turned(obliquityRad + towardsAxis * ringTiltRad(obliquityRad))
    : null;
  const rotate = withBodyRotation(PLANET, tilted.rotation);
  const tiltRing =
    ringNormal === null
      ? withoutRingNormal
      : withRingNormal([ringNormal.x, ringNormal.y, ringNormal.z]);
  return {
    bodies: bodiesOf(populatedBodiesWith((each) => tiltRing(rotate(each)))),
    pole: tilted.pole,
    orbitNormal: tilted.orbitNormal,
    ringNormal,
  };
}

function sceneOf(selectedId: string | null = null, bodies = bodiesOf()) {
  const planet = body(PLANET, bodies);
  return bodyScene({
    body: planet,
    bodies,
    plane: bodyPlane(planet, GALACTIC),
    time: TIME,
    selectedId,
    fitRadiusAu: bodyFitAu(planet, bodies),
  });
}

describe("focusTarget", () => {
  it("focuses a planet selected, and the planet of a selected moon or ring", () => {
    const bodies = bodiesOf();

    for (const id of [PLANET, MOON, RING]) {
      expect(focusTarget(body(id, bodies), bodies)?.id).toBe(PLANET);
    }
  });

  it("focuses nothing for a belt, a body not present, or no selection", () => {
    const bodies = bodiesOf();

    expect(focusTarget(body(BELT, bodies), bodies)).toBeNull();
    expect(focusTarget(body(DESTROYED_PLANET, bodies), bodies)).toBeNull();
    expect(focusTarget(null, bodies)).toBeNull();
  });
});

describe("focusHeldReason", () => {
  it("holds nothing back while there is a planet to focus", () => {
    const bodies = bodiesOf();

    for (const id of [PLANET, MOON, RING]) {
      expect(focusHeldReason(body(id, bodies), bodies)).toBeNull();
    }
  });

  it("says no planet is selected for a belt or no selection", () => {
    const bodies = bodiesOf();

    expect(focusHeldReason(body(BELT, bodies), bodies)).toBe("NO PLANET SELECTED");
    expect(focusHeldReason(null, bodies)).toBe("NO PLANET SELECTED");
  });

  it("names a planet not present by its kind and state", () => {
    const bodies = bodiesOf();

    expect(focusHeldReason(body(DESTROYED_PLANET, bodies), bodies)).toBe("PLANET DESTROYED");
  });

  it("says the orbit is withheld for a planet, or a moon's planet, whose orbit is", () => {
    const withOrbit = bodiesOf();
    const withheld: SystemBody = { ...body(PLANET, withOrbit), orbit: { state: "not_resolved" } };
    const bodies = withOrbit.with(
      withOrbit.findIndex((candidate) => candidate.id === PLANET),
      withheld,
    );

    expect(focusHeldReason(withheld, bodies)).toBe("ORBIT NOT RESOLVED");
    expect(focusHeldReason(body(MOON, bodies), bodies)).toBe("ORBIT NOT RESOLVED");
  });

  it("says the orbit is not yet modelled for a planet whose orbit is not computed", () => {
    const withOrbit = bodiesOf();
    const unmodelled: SystemBody = { ...body(PLANET, withOrbit), orbit: { state: "not_modelled" } };
    const bodies = withOrbit.with(
      withOrbit.findIndex((candidate) => candidate.id === PLANET),
      unmodelled,
    );

    expect(focusHeldReason(unmodelled, bodies)).toBe("ORBIT NOT YET MODELLED");
    expect(focusHeldReason(body(RING, bodies), bodies)).toBe("ORBIT NOT YET MODELLED");
  });

  it("says a selected contact's kind is not resolved, since it may be a planet", () => {
    const bodies = bodiesOf();
    const contact: SystemBody = { ...body(PLANET, bodies), kind: { kind: "unresolved" } };

    expect(focusHeldReason(contact, bodies)).toBe("KIND NOT RESOLVED");
  });
});

describe("bodyFrameName", () => {
  it("names the frame BODY and the body's designation of record", () => {
    expect(bodyFrameName(body(PLANET))).toBe("BODY H7K 4C0RFZ D-7 /256");
  });
});

describe("bodyPlane", () => {
  it("lies in the planet's equatorial plane, normal to its rotation section's pole", () => {
    const { bodies, pole } = tiltedPlanet();
    const plane = bodyPlane(body(PLANET, bodies), GALACTIC);

    expect(plane.name).toBe("EQUATORIAL PLANE");
    expect(norm(sub(plane.frame.north, pole))).toBeLessThan(1e-12);
  });

  it("lies in the planet's orbital plane without a rotation section, its pole the orbit's normal", () => {
    const planet = body(PLANET);
    if (planet.orbit.state !== "ok") {
      throw new Error("the planet has an orbit");
    }
    const plane = bodyPlane(planet, GALACTIC);

    expect(plane.name).toBe("EQUATORIAL PLANE");
    expect(dot(plane.frame.north, orbitNormal(planet.orbit.value.orbit))).toBeCloseTo(1, 12);
  });

  it("stands on the planet's orbital plane where its rotation is withheld", () => {
    const { bodies, orbitNormal: normal } = tiltedPlanet();
    const withheld: SystemBody = { ...body(PLANET, bodies), rotation: { state: "not_resolved" } };

    expect(dot(bodyPlane(withheld, GALACTIC).frame.north, normal)).toBeCloseTo(1, 12);
  });
});

describe("equatorNormal", () => {
  it("is a body's pole", () => {
    const { bodies, pole } = tiltedPlanet();

    expect(equatorNormal(body(PLANET, bodies))).toEqual(pole);
  });

  it("is a body's orbit normal where its rotation is not modelled", () => {
    const planet = body(PLANET);
    if (planet.orbit.state !== "ok") {
      throw new Error("the planet has an orbit");
    }

    expect(equatorNormal(planet)).toEqual(orbitNormal(planet.orbit.value.orbit));
  });

  it("is null where a body's rotation is withheld, which its orbit does not stand in for", () => {
    const { bodies } = tiltedPlanet();

    expect(
      equatorNormal({ ...body(PLANET, bodies), rotation: { state: "not_resolved" } }),
    ).toBeNull();
  });

  it("is null for a body with neither a rotation section nor an orbit", () => {
    expect(equatorNormal({ ...body(PLANET), orbit: { state: "not_resolved" } })).toBeNull();
  });
});

describe("bodyFitAu", () => {
  it("fits the farther of the moon's apoapsis and the ring's outer edge", () => {
    const bodies = bodiesOf();
    const moon = body(MOON, bodies);
    if (moon.orbit.state !== "ok") {
      throw new Error("the moon has an orbit");
    }
    const { semiMajorAxisM, eccentricity } = moon.orbit.value.orbit;

    expect(bodyFitAu(body(PLANET, bodies), bodies)).toBe(
      (semiMajorAxisM * (1 + eccentricity)) / METRES_PER_AU,
    );
  });

  it("fits ten of its radii about a body with no moon or ring", () => {
    const member = body(`${FIXTURE_SYSTEM}.e001`);
    if (member.bulk.state !== "ok") {
      throw new Error("the member's bulk is in the populated answer");
    }

    expect(bodyFitAu(member, bodiesOf())).toBe(
      (LONE_BODY_FIT_RADII * member.bulk.value.radiusM) / METRES_PER_AU,
    );
  });
});

describe("bodyScene", () => {
  it("draws the planet at the centre and its moon where its orbit about the planet puts it", () => {
    const moon = body(MOON);
    if (moon.orbit.state !== "ok") {
      throw new Error("the moon has an orbit");
    }
    const scene = sceneOf();
    const expected = scale(positionAt(moon.orbit.value.orbit, TIME), 1 / METRES_PER_AU);

    expect(scene.points.map((mark) => [mark.id, mark.shape])).toEqual([
      [PLANET, "triangle-down"],
      [MOON, "pentagon"],
    ]);
    expect(norm(scene.points[0]?.position ?? vec3(1, 1, 1))).toBe(0);
    const moonMark = scene.points[1];
    expect(moonMark?.sizeClass).toBe(MOON_SIZE_CLASS);
    expect(norm(sub(moonMark?.position ?? vec3(0, 0, 0), expected))).toBeLessThan(
      1e-12 * norm(expected),
    );
  });

  it("draws the moon's orbit, as the selected path once the moon is selected", () => {
    expect(sceneOf().paths?.map((path) => [path.id, path.role])).toEqual([[MOON, "reference"]]);
    expect(sceneOf(MOON).paths?.map((path) => [path.id, path.role])).toEqual([[MOON, "selected"]]);
    expect(sceneOf(MOON).selectedId).toBe(MOON);
  });

  it("draws the ring as a ticked annulus on the equatorial plane, its edges from the planet's centre", () => {
    expect(
      sceneOf().annuli?.map((annulus) => [
        annulus.label,
        annulus.innerRadius * METRES_PER_AU,
        annulus.outerRadius * METRES_PER_AU,
        annulus.ticks,
        annulus.centre,
      ]),
    ).toEqual([["MASSIVE RING", 66_000_000, 136_800_000, true, undefined]]);
  });

  it("draws the ring on the reference plane without a pole or a ring normal, as before", () => {
    const scene = sceneOf(null, bodiesOf(populatedBodiesWith(withoutRingNormal)));
    const [ring] = scene.annuli ?? [];

    expect(dot(ring?.normal ?? vec3(0, 0, 0), scene.frame.north)).toBeCloseTo(1, 12);
  });

  it("draws the fixture's ring in the plane its normal gives, without a pole", () => {
    const [ring] = sceneOf().annuli ?? [];

    expect(ring?.normal).toEqual(FIXTURE_RING_NORMAL);
  });

  it("draws a ring under a tilted pole in the plane its normal gives, not the equator's", () => {
    const { bodies, ringNormal } = tiltedPlanet({ ring: true });
    const scene = sceneOf(null, bodies);
    const normal = scene.annuli?.[0]?.normal ?? vec3(0, 0, 0);

    expect(normal).toEqual(ringNormal);
    expect(Math.acos(dot(normal, scene.frame.north))).toBeCloseTo(ringTiltRad(OBLIQUITY_RAD), 12);
  });

  it("keeps a retrograde spinner's ring normal on the spin's side, never turned to the orbit's", () => {
    const {
      bodies,
      pole,
      orbitNormal: normalOfOrbit,
      ringNormal,
    } = tiltedPlanet({
      obliquityRad: 2.6,
      ring: true,
    });
    const normal = sceneOf(null, bodies).annuli?.[0]?.normal ?? vec3(0, 0, 0);

    expect(normal).toEqual(ringNormal);
    expect([dot(normal, pole) > 0, dot(normal, normalOfOrbit) < 0]).toEqual([true, true]);
  });

  it("draws a ring without a normal in its planet's orbital plane, tilted to a tilted pole's equator", () => {
    const { bodies } = tiltedPlanet();
    const planet = body(PLANET, bodies);
    if (planet.orbit.state !== "ok") {
      throw new Error("the planet has an orbit");
    }
    const scene = sceneOf(null, bodies);
    const normal = scene.annuli?.[0]?.normal ?? vec3(0, 0, 0);

    expect(normal).toEqual(orbitNormal(planet.orbit.value.orbit));
    expect(Math.acos(dot(normal, scene.frame.north))).toBeCloseTo(OBLIQUITY_RAD, 12);
  });

  it("draws the ring as selected once it is selected, as a selected orbit is", () => {
    expect(sceneOf().annuli?.map((annulus) => annulus.selected)).toEqual([false]);
    expect(sceneOf(RING).annuli?.map((annulus) => annulus.selected)).toEqual([true]);
  });

  it("draws every ring about the planet's mark, selected or not (plan 14, P14.T42.g)", () => {
    expect([
      sceneOf().points[0]?.id,
      sceneOf().annuli?.map((annulus) => annulus.aboutId),
      sceneOf(RING).annuli?.map((annulus) => annulus.aboutId),
    ]).toEqual([PLANET, [PLANET], [PLANET]]);
  });

  it("draws no reticle for a selection outside the body's frame", () => {
    expect(sceneOf(BELT).selectedId).toBeNull();
  });
});
