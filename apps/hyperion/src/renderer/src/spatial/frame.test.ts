import { describe, expect, it } from "vitest";

import { between, seededRandom } from "../test/seededRandom";
import {
  cylindrical,
  fromLocal,
  type LocalFrame,
  localFrameAt,
  PARALLEL_TOLERANCE,
  planeFrame,
  toLocal,
} from "./frame";
import { add, cross, dot, norm, normalise, scale, sub, vec3, type Vec3 } from "./vec3";

/** How far apart two vectors are. */
function gap(actual: Vec3, expected: Vec3): number {
  return norm(sub(actual, expected));
}

describe("vec3", () => {
  it("adds, subtracts and scales componentwise", () => {
    expect(add(vec3(1, 2, 3), vec3(4, 5, 6))).toEqual(vec3(5, 7, 9));
    expect(sub(vec3(1, 2, 3), vec3(4, 5, 6))).toEqual(vec3(-3, -3, -3));
    expect(scale(vec3(1, -2, 3), 2)).toEqual(vec3(2, -4, 6));
  });

  it("takes dot and right-handed cross products", () => {
    expect(dot(vec3(1, 2, 3), vec3(4, -5, 6))).toBe(12);
    expect(cross(vec3(1, 0, 0), vec3(0, 1, 0))).toEqual(vec3(0, 0, 1));
  });

  it("normalises to unit length and refuses the zero vector", () => {
    expect(norm(normalise(vec3(3, 4, 12)))).toBeCloseTo(1, 15);
    expect(norm(vec3(3, 4, 12))).toBe(13);
    expect(() => normalise(vec3(0, 0, 0))).toThrow(RangeError);
  });
});

describe("localFrameAt", () => {
  it("points coreward along -x and spinward along +y on the +x axis", () => {
    const frame = localFrameAt(vec3(26_000, 0, 0));

    expect(gap(frame.coreward, vec3(-1, 0, 0))).toBeLessThan(1e-12);
    expect(gap(frame.spinward, vec3(0, 1, 0))).toBeLessThan(1e-12);
    expect(frame.north).toEqual(vec3(0, 0, 1));
    expect(frame.onAxis).toBe(false);
  });

  it("points coreward along +y and spinward along +x on the -y axis", () => {
    const frame = localFrameAt(vec3(0, -5, 0));

    expect(gap(frame.coreward, vec3(0, 1, 0))).toBeLessThan(1e-12);
    expect(gap(frame.spinward, vec3(1, 0, 0))).toBeLessThan(1e-12);
  });

  it("ignores height: the directions are parallel to the galactic plane", () => {
    const low = localFrameAt(vec3(3_000, 4_000, 0));
    const high = localFrameAt(vec3(3_000, 4_000, 900));

    expect(high.coreward).toEqual(low.coreward);
    expect(high.spinward).toEqual(low.spinward);
  });

  it("forms the right-handed triple spinward, coreward, north at 100 random points", () => {
    const random = seededRandom(0x05_09_a);
    for (let i = 0; i < 100; i += 1) {
      const position = vec3(
        between(random, -65_536, 65_536),
        between(random, -65_536, 65_536),
        between(random, -5_000, 5_000),
      );
      const frame = localFrameAt(position);

      expect(gap(cross(frame.spinward, frame.coreward), frame.north)).toBeLessThan(1e-12);
      expect(norm(frame.coreward)).toBeCloseTo(1, 12);
      expect(norm(frame.spinward)).toBeCloseTo(1, 12);
      expect(dot(frame.coreward, frame.spinward)).toBeCloseTo(0, 12);
    }
  });

  it("falls back to -x and +y on the axis and says so", () => {
    for (const position of [vec3(0, 0, 0), vec3(0, 0, 1_200), vec3(5e-7, -5e-7, 0)]) {
      const frame = localFrameAt(position);

      expect(frame.onAxis).toBe(true);
      expect(frame.coreward).toEqual(vec3(-1, 0, 0));
      expect(frame.spinward).toEqual(vec3(0, 1, 0));
      expect(gap(cross(frame.spinward, frame.coreward), frame.north)).toBeLessThan(1e-12);
    }
  });

  it("names the directions just outside the axis tolerance", () => {
    const frame = localFrameAt(vec3(0, 2e-6, 0));

    expect(frame.onAxis).toBe(false);
    expect(gap(frame.coreward, vec3(0, -1, 0))).toBeLessThan(1e-12);
  });
});

describe("toLocal", () => {
  it("resolves a vector into coreward, spinward and north components", () => {
    const frame = localFrameAt(vec3(26_000, 0, 0));

    const local = toLocal(frame, vec3(-3, 2, -1.5));

    expect(local.coreward).toBeCloseTo(3, 12);
    expect(local.spinward).toBeCloseTo(2, 12);
    expect(local.north).toBeCloseTo(-1.5, 12);
  });

  it("round-trips through fromLocal", () => {
    const random = seededRandom(7);
    for (let i = 0; i < 100; i += 1) {
      const frame = localFrameAt(
        vec3(between(random, -60_000, 60_000), between(random, -60_000, 60_000), 0),
      );
      const vector = vec3(
        between(random, -50, 50),
        between(random, -50, 50),
        between(random, -50, 50),
      );

      expect(gap(fromLocal(frame, toLocal(frame, vector)), vector)).toBeLessThan(1e-10);
    }
  });
});

describe("cylindrical", () => {
  it.each([
    [vec3(26_000, 0, 12), 26_000, 0, 12],
    [vec3(0, 100, 0), 100, 90, 0],
    [vec3(-100, 0, -3.2), 100, 180, -3.2],
    [vec3(0, -100, 0), 100, 270, 0],
    [vec3(100, 100, 0), Math.SQRT2 * 100, 45, 0],
    [vec3(0, 0, 50), 0, 0, 50],
    [vec3(100, -0, 0), 100, 0, 0],
  ])("gives %o as radius %f, angle %f and height %f", (position, radiusLy, angleDeg, heightLy) => {
    const result = cylindrical(position);

    expect(result.radiusLy).toBeCloseTo(radiusLy, 9);
    expect(result.angleDeg).toBeCloseTo(angleDeg, 9);
    expect(Object.is(result.angleDeg, -0)).toBe(false);
    expect(result.heightLy).toBe(heightLy);
  });

  it("keeps the angle in [0, 360) just below the +x axis", () => {
    const { angleDeg } = cylindrical(vec3(1e6, -1e-10, 0));

    expect(angleDeg).toBeGreaterThanOrEqual(0);
    expect(angleDeg).toBeLessThan(360);
  });
});

/** The largest departure of a frame from an orthonormal right-handed triple, in any test. */
function frameError(frame: LocalFrame): number {
  return Math.max(
    Math.abs(norm(frame.coreward) - 1),
    Math.abs(norm(frame.spinward) - 1),
    Math.abs(norm(frame.north) - 1),
    Math.abs(dot(frame.coreward, frame.spinward)),
    Math.abs(dot(frame.coreward, frame.north)),
    Math.abs(dot(frame.spinward, frame.north)),
    gap(cross(frame.spinward, frame.coreward), frame.north),
  );
}

/** A direction drawn uniformly on the sphere, scaled to a length in [0.001, 1000). */
function randomDirection(random: () => number): Vec3 {
  const z = between(random, -1, 1);
  const angle = between(random, 0, 2 * Math.PI);
  const across = Math.sqrt(1 - z * z);
  const length = 10 ** between(random, -3, 3);
  return scale(vec3(across * Math.cos(angle), across * Math.sin(angle), z), length);
}

describe("planeFrame", () => {
  it("is orthonormal and right-handed, as localFrameAt's is, for 1,000 random normals", () => {
    const random = seededRandom(0x14_40_b);
    let worst = 0;
    for (let i = 0; i < 1_000; i += 1) {
      const frame = planeFrame(randomDirection(random), randomDirection(random));
      worst = Math.max(worst, frameError(frame));
    }

    expect(worst).toBeLessThan(1e-12);
  });

  it("takes the normal as north and the reference laid onto the plane as coreward", () => {
    const normal = vec3(0.3, -0.2, 0.9);
    const reference = vec3(-1, 0.4, 0.2);

    const frame = planeFrame(normal, reference);

    const north = normalise(normal);
    const onPlane = sub(reference, scale(north, dot(reference, north)));
    expect(gap(frame.north, north)).toBeLessThan(1e-15);
    expect(gap(frame.coreward, normalise(onPlane))).toBeLessThan(1e-15);
    expect(frame.onAxis).toBe(false);
  });

  it("equals localFrameAt's with the galactic north as normal and the point's coreward as reference", () => {
    const random = seededRandom(0x14_40_c);
    for (let i = 0; i < 100; i += 1) {
      const position = vec3(
        between(random, -65_536, 65_536),
        between(random, -65_536, 65_536),
        between(random, -5_000, 5_000),
      );
      const galactic = localFrameAt(position);

      const frame = planeFrame(vec3(0, 0, 1), galactic.coreward);

      expect(gap(frame.coreward, galactic.coreward)).toBeLessThan(1e-15);
      expect(gap(frame.spinward, galactic.spinward)).toBeLessThan(1e-15);
      expect(frame.north).toEqual(galactic.north);
      expect(frame.onAxis).toBe(galactic.onAxis);
    }
  });

  it.each([
    ["along the normal", vec3(0, 0, 3)],
    ["against it", vec3(0, 0, -1)],
    ["within 1e-6 of it", vec3(0.5 * PARALLEL_TOLERANCE, 0, 1)],
    ["of no length", vec3(0, 0, 0)],
  ])("falls back to a perpendicular for a reference %s, still orthonormal", (_, reference) => {
    const frame = planeFrame(vec3(0, 0, 1), reference);

    expect(frameError(frame)).toBeLessThan(1e-12);
    expect(dot(frame.coreward, vec3(0, 0, 1))).toBeCloseTo(0, 15);
  });

  it("still projects a reference just over 1e-6 off the normal", () => {
    const frame = planeFrame(vec3(0, 0, 1), vec3(2 * PARALLEL_TOLERANCE, 0, 1));

    expect(gap(frame.coreward, vec3(1, 0, 0))).toBeLessThan(1e-9);
  });

  it("refuses a normal of no length, which has no plane", () => {
    expect(() => planeFrame(vec3(0, 0, 0), vec3(1, 0, 0))).toThrow(RangeError);
  });
});
