import { describe, expect, it } from "vitest";

import {
  handOverDistance,
  instancedPerSquareMetre,
  instancedRadiusAt,
  instancedShare,
  pixelAngleRad,
  RING_PARTICLE_RADII_M,
} from "./ringSplit";

const [S_MIN_M, S_MAX_M] = RING_PARTICLE_RADII_M;
const SIXTY_DEG = Math.PI / 3;
const MASSIVE = { ringKind: "massive" } as const;

describe("a ring's hand-over to instances (R11.T9.d)", () => {
  it.each([
    ["1080p", 1920, 1830],
    ["720p", 1280, 1220],
    ["4K", 3840, 3670],
  ])(
    "lies at %s at its largest particle's diameter times the plan's figure, to 1%",
    (_, widthPx, times) => {
      const distanceM = handOverDistance(MASSIVE, pixelAngleRad(SIXTY_DEG, widthPx));
      expect(Math.abs(distanceM / (2 * S_MAX_M) / times - 1)).toBeLessThan(0.01);
    },
  );

  it("never hands a dusty ring's grains over", () => {
    expect(handOverDistance({ ringKind: "dusty" }, pixelAngleRad(SIXTY_DEG, 1920))).toBe(0);
  });
});

describe("the instanced share of a ring's optical depth (R11.T9.d)", () => {
  it("is 0 at a 10 m footprint, 0.371 at 1 m and 1 below 2 cm, over radii of 1 cm–5 m", () => {
    expect(instancedShare(10, 0.01, 5)).toBe(0);
    expect(instancedShare(1, 0.01, 5)).toBeCloseTo(0.371, 3);
    expect(instancedShare(0.019, 0.01, 5)).toBe(1);
  });

  it("rises with approach without a step", () => {
    let previous = instancedShare(20, S_MIN_M, S_MAX_M);
    for (let footprintM = 20; footprintM > 0.005; footprintM *= 0.99) {
      const share = instancedShare(footprintM, S_MIN_M, S_MAX_M);
      expect(share).toBeGreaterThanOrEqual(previous);
      expect(share - previous).toBeLessThan(0.002);
      previous = share;
    }
  });

  it("is the optical depth the instanced particles hold, by their number and size", () => {
    const tau = 1.5;
    const footprintM = 1;
    const perSquareMetre = instancedPerSquareMetre(tau, footprintM, S_MIN_M, S_MAX_M);
    // Their cross-sections' sum by quadrature over the instanced radii, s⁻³ weighted.
    const low = footprintM / 2;
    const c = tau / (Math.PI * Math.log(S_MAX_M / S_MIN_M));
    const steps = 20_000;
    let crossSection = 0;
    let number = 0;
    for (let k = 0; k < steps; k += 1) {
      const s = low * (S_MAX_M / low) ** ((k + 0.5) / steps);
      const ds = s * (Math.log(S_MAX_M / low) / steps);
      crossSection += Math.PI * s * s * c * s ** -3 * ds;
      number += c * s ** -3 * ds;
    }
    expect(crossSection / tau).toBeCloseTo(instancedShare(footprintM, S_MIN_M, S_MAX_M), 6);
    expect(number / perSquareMetre).toBeCloseTo(1, 5);
  });

  it("draws instanced radii from the s⁻³ law between its least and largest", () => {
    expect(instancedRadiusAt(0, 0.5, S_MAX_M)).toBeCloseTo(0.5, 12);
    expect(instancedRadiusAt(1, 0.5, S_MAX_M)).toBeCloseTo(S_MAX_M, 12);
    // Half the number lies below the radius whose s⁻² is the mean of the ends'.
    const median = instancedRadiusAt(0.5, 0.5, S_MAX_M);
    expect(median ** -2).toBeCloseTo((0.5 ** -2 + S_MAX_M ** -2) / 2, 9);
  });
});
