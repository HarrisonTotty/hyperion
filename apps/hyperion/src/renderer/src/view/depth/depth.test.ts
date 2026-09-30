import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  type DepthLayer,
  OCCLUDER_MARGIN,
  occluderRadius,
  separable,
  transparentLayerOrder,
} from "./depth";

const AU_M = 1.495_978_707e11;
const EARTH_RADIUS_M = 6.371e6;
const EARTH: BodyIdHex = "0200080020000000.0103";
const MARS: BodyIdHex = "0200080020000000.0104";

describe("separable", () => {
  it("orders surfaces 1 m apart to 10⁶ m and 1 km apart to 10⁹ m", () => {
    expect(separable(1e6, 1e6 + 1, 1e6)).toBe(true);
    expect(separable(1e6, 1e6 + 1, 2e6)).toBe(false);
    expect(separable(1e9, 1e9 + 1_000, 1e9)).toBe(true);
    expect(separable(1e9, 1e9 + 1_000, 2e9)).toBe(false);
  });
});

/**
 * The smallest gap, over points of the front hemisphere visible from a camera `distanceM` from the
 * centre of a sphere of `radiusM`, between the point and the occluder behind it along the ray,
 * as a fraction of the point's distance.
 */
function smallestGap(radiusM: number, distanceM: number): number {
  const occluder = occluderRadius(radiusM, distanceM);
  const camera = [0, 0, distanceM] as const;
  let smallest = Infinity;
  for (let i = 0; i <= 200; i += 1) {
    // Points from the sub-camera point towards the limb.
    const polar = (i / 200) * Math.acos(radiusM / distanceM);
    const p = [radiusM * Math.sin(polar), 0, radiusM * Math.cos(polar)] as const;
    const ray = [p[0] - camera[0], p[1] - camera[1], p[2] - camera[2]] as const;
    const t = Math.hypot(...ray);
    const u = [ray[0] / t, ray[1] / t, ray[2] / t] as const;
    // The ray from the camera meets the occluder where |c + s u| = r_occ.
    const b = camera[0] * u[0] + camera[1] * u[1] + camera[2] * u[2];
    const c = distanceM * distanceM - occluder * occluder;
    const discriminant = b * b - c;
    if (discriminant >= 0) {
      const hit = -b - Math.sqrt(discriminant);
      smallest = Math.min(smallest, (hit - t) / t);
    }
  }
  return smallest;
}

describe("occluderRadius", () => {
  it.each([
    ["400 km up", EARTH_RADIUS_M + 4e5],
    ["10⁸ m out", 1e8],
    ["1 au out", AU_M],
  ])("keeps the front hemisphere 4e-6 d in front of the occluder %s", (_, distanceM) => {
    expect(smallestGap(EARTH_RADIUS_M, distanceM)).toBeGreaterThanOrEqual(OCCLUDER_MARGIN);
  });

  it("never shrinks an occluder below half the body's radius", () => {
    expect(occluderRadius(1_000, 1e12)).toBe(500);
  });
});

describe("transparentLayerOrder", () => {
  const layers: DepthLayer[] = [
    { kind: "shell", id: "earth-high-cloud", body: EARTH, radiusM: EARTH_RADIUS_M + 1.2e4 },
    { kind: "plane", id: "earth-rings", body: EARTH },
    { kind: "shell", id: "mars-air", body: MARS, radiusM: 3.4e6 + 1e5 },
    { kind: "opaque", id: "terrain" },
    { kind: "shell", id: "earth-low-cloud", body: EARTH, radiusM: EARTH_RADIUS_M + 2e3 },
    { kind: "shell", id: "earth-air", body: EARTH, radiusM: EARTH_RADIUS_M + 1e5 },
    { kind: "shell", id: "earth-haze", body: EARTH, radiusM: EARTH_RADIUS_M + 500 },
  ];
  // Between the low and high cloud decks, 5 km up; Mars far behind.
  const camera = {
    bodyDistanceM: (body: BodyIdHex): number => (body === EARTH ? EARTH_RADIUS_M + 5e3 : 7.8e10),
  };

  it("draws opaque first, the farther body next, and a camera between cloud decks as stated", () => {
    expect(transparentLayerOrder(layers, camera).map((l) => l.id)).toEqual([
      "terrain",
      "mars-air",
      "earth-haze",
      "earth-low-cloud",
      "earth-rings",
      "earth-air",
      "earth-high-cloud",
    ]);
  });

  it("depends on the layers and the camera alone, not their order", () => {
    const forward = transparentLayerOrder(layers, camera).map((l) => l.id);
    const backward = transparentLayerOrder(layers.toReversed(), camera).map((l) => l.id);
    expect(backward).toEqual(forward);
  });
});
