import type { ResponseFor, UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { positionAt } from "../../lib/orbit";
import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { MOON_SIZE_CLASS } from "../../lib/system/bodySymbols";
import { orbitNormal } from "../../lib/system/hierarchy";
import type { SystemBody } from "../../lib/system/model";
import { localFrameAt } from "../../spatial/frame";
import { dot, norm, scale, sub, vec3 } from "../../spatial/vec3";
import { FIXTURE_SYSTEM, populatedBodies } from "../../test/planetaryFixture";
import {
  bodyFitAu,
  bodyFrameName,
  bodyPlane,
  bodyScene,
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

function sceneOf(selectedId: string | null = null) {
  const bodies = bodiesOf();
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

describe("bodyFrameName", () => {
  it("names the frame BODY and the body's designation of record", () => {
    expect(bodyFrameName(body(PLANET))).toBe("BODY H7K 4C0RFZ D-7 /256");
  });
});

describe("bodyPlane", () => {
  it("lies in the planet's orbital plane, its normal the orbit's", () => {
    const planet = body(PLANET);
    if (planet.orbit.state !== "ok") {
      throw new Error("the planet has an orbit");
    }
    const plane = bodyPlane(planet, GALACTIC);

    expect(plane.name).toBe("EQUATORIAL PLANE");
    expect(dot(plane.frame.north, orbitNormal(planet.orbit.value.orbit))).toBeCloseTo(1, 12);
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

  it("draws no reticle for a selection outside the body's frame", () => {
    expect(sceneOf(BELT).selectedId).toBeNull();
  });
});
