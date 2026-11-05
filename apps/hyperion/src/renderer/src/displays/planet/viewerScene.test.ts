import { describe, expect, it } from "vitest";

import { dot, normalise, sub } from "../../geometry/vec3";
import { sliceEarthInspection } from "../../test/inspectionFixture";
import { FIXTURE_EARTH } from "../../test/planetaryFixture";
import { PHASE_GIANT, phaseScene } from "../../view/scenes/phaseScene";
import {
  heldSunStatement,
  ORBITAL_SUN,
  type SunHolder,
  sunnedScene,
  viewerScene,
} from "./viewerScene";

describe("the viewer's sun", () => {
  it("leaves the scene as its records put it while ORBITAL", () => {
    const scene = phaseScene().sceneAt(0);
    expect(sunnedScene(scene, PHASE_GIANT, ORBITAL_SUN)).toBe(scene);
  });

  it("stands every star over the held point, at its own distance", () => {
    const scene = phaseScene().sceneAt(0);
    const held = sunnedScene(scene, PHASE_GIANT, {
      kind: "held",
      latitudeDeg: 0,
      longitudeDeg: 90,
    });
    const planet = held.bodies.find((body) => body.id === PHASE_GIANT);
    const star = held.bodies.find((body) => body.kind === "star");
    const before = scene.bodies.find((body) => body.kind === "star");
    if (planet === undefined || star === undefined || before === undefined) {
      throw new Error("the scene holds its planet and its star");
    }
    // The kept planet has no rotation: its galactic axes stand for its body-fixed ones.
    const direction = normalise(sub(star.centreM, planet.centreM));
    expect(dot(direction, { x: 0, y: 1, z: 0 })).toBeCloseTo(1, 12);
    const distance = (centre: typeof star.centreM): number =>
      Math.hypot(
        centre.x - planet.centreM.x,
        centre.y - planet.centreM.y,
        centre.z - planet.centreM.z,
      );
    expect(distance(star.centreM) / distance(before.centreM)).toBeCloseTo(1, 12);
    expect(star.retarded?.centreM).toEqual(star.centreM);
  });
});

describe("the viewer's scene", () => {
  it("offers ORBIT about its planet and opens in it", () => {
    const kept = viewerScene(sliceEarthInspection(), { current: ORBITAL_SUN });
    expect([kept.startPreset, kept.sceneAt(0).orbitHome]).toEqual(["orbit", FIXTURE_EARTH]);
  });

  it("moves the light at its next frame once the panel holds the sun, the scene kept", () => {
    const holder: SunHolder = { current: ORBITAL_SUN };
    const kept = viewerScene(sliceEarthInspection(), holder);
    const starOf = (tS: number) => kept.sceneAt(tS).bodies.find((body) => body.kind === "star");
    const before = starOf(0)?.centreM;
    holder.current = { kind: "held", latitudeDeg: 30, longitudeDeg: 120 };
    expect(starOf(0)?.centreM).not.toEqual(before);
  });
});

describe("the label block's statement of the sun", () => {
  it("says where the sun is held, and nothing while it is where its orbit puts it", () => {
    expect([
      heldSunStatement({ kind: "held", latitudeDeg: -12.25, longitudeDeg: 120 }),
      heldSunStatement(ORBITAL_SUN),
    ]).toEqual(["SUN HELD: LATITUDE -12.3° · LONGITUDE 120.0°", null]);
  });
});
