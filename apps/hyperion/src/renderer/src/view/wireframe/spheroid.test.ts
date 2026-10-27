import { describe, expect, it } from "vitest";

import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { SATURN_OBLATE_SPHEROID } from "../atmosphere/reference/cases";
import { perpendicularPair } from "./bodies";
import { angularDistortion, datumSpheroidOf, stretched, unstretched } from "./spheroid";

/** `TEST SATURN`'s figure at 1 bar, m. */
const SATURN = SATURN_OBLATE_SPHEROID;

const POLE = normalise(vec3(1, 2, 3));

/** The angle between two directions, rad. */
function angle(u: Vec3, v: Vec3): number {
  return Math.acos(Math.min(1, dot(normalise(u), normalise(v))));
}

describe("the datum's spheroid (R02.T19)", () => {
  it("is a flattened figure's stretch a ÷ c about its unit pole", () => {
    const spheroid = datumSpheroidOf(SATURN, scale(POLE, 7));
    expect([norm(sub(spheroid?.pole ?? vec3(0, 0, 0), POLE)) < 1e-15, spheroid?.stretch]).toEqual([
      true,
      SATURN.equatorialRadiusM / SATURN.polarRadiusM,
    ]);
  });

  it.each([
    ["a sphere", { equatorialRadiusM: 6e6, polarRadiusM: 6e6 }, POLE],
    ["a prolate figure", { equatorialRadiusM: 5e6, polarRadiusM: 6e6 }, POLE],
    ["no polar radius", { equatorialRadiusM: 6e6, polarRadiusM: 0 }, POLE],
    ["a polar radius not a number", { equatorialRadiusM: 6e6, polarRadiusM: Number.NaN }, POLE],
    ["a pole of no length", SATURN, vec3(0, 0, 0)],
  ] as const)("is none for %s", (_, figure, pole) => {
    expect(datumSpheroidOf(figure, pole)).toBeNull();
  });

  it("stretches the spheroid onto the sphere of its equatorial radius, and back", () => {
    const spheroid = datumSpheroidOf(SATURN, POLE);
    // Points of the spheroid at several reduced latitudes and longitudes about the pole.
    const [e1, e2] = perpendicularPair(POLE);
    const points = [-80, -30, 0, 45, 89].flatMap((latDeg) =>
      [0, 100, 250].map((lonDeg) => {
        const lat = (latDeg * Math.PI) / 180;
        const lon = (lonDeg * Math.PI) / 180;
        const across = SATURN.equatorialRadiusM * Math.cos(lat);
        return add(
          add(scale(e1, across * Math.cos(lon)), scale(e2, across * Math.sin(lon))),
          scale(POLE, SATURN.polarRadiusM * Math.sin(lat)),
        );
      }),
    );
    const radii = points.map((p) => norm(stretched(p, spheroid)) / SATURN.equatorialRadiusM - 1);
    const back = points.map((p) => norm(sub(unstretched(stretched(p, spheroid), spheroid), p)));
    expect([
      Math.max(...radii.map(Math.abs)) < 1e-15,
      Math.max(...back) < 1e-8 * SATURN.equatorialRadiusM,
    ]).toEqual([true, true]);
  });

  it("is the identity for a sphere, returning the very vector it was given", () => {
    const v = vec3(1, -2, 3);
    expect([stretched(v, null) === v, unstretched(v, null) === v, angularDistortion(null)]).toEqual(
      [true, true, 1],
    );
  });

  it("turns lines of sight apart by at most its stretch", () => {
    const spheroid = datumSpheroidOf(SATURN, POLE);
    const s = angularDistortion(spheroid);
    const pairs = [
      [vec3(1, 0, 0), vec3(1, 1e-3, 0)],
      [POLE, normalise(vec3(1, 2, 3.01))],
      [vec3(0, 0, -1), vec3(1e-4, 0, -1)],
      [vec3(3, -1, 2), vec3(3, -1.001, 2)],
    ] as const;
    const ratios = pairs.map(
      ([u, v]) => angle(stretched(u, spheroid), stretched(v, spheroid)) / angle(u, v),
    );
    expect([s, ratios.every((ratio) => ratio <= s && ratio >= 1 / s)]).toEqual([
      SATURN.equatorialRadiusM / SATURN.polarRadiusM,
      true,
    ]);
  });
});
