import { describe, expect, it } from "vitest";

import { add, scale, vec3 } from "../geometry/vec3";
import { TABLE_SIZES } from "../view/atmosphere/hillaire";
import {
  BOTTOM_M,
  drawnSurfaceRays,
  type GateRay,
  marchKindOf,
  type Placement,
  type Rgb3,
  runGate,
  sphereHits,
  sunZenithAtDeg,
} from "./atmosphereGate";

/** A sky ray of the gate's shape, named, that only a stand-in pixel reads. */
function aRay(name: string): GateRay {
  return {
    family: "sky",
    name,
    o: vec3(0, 0, 0),
    d: vec3(0, 0, 1),
    s: vec3(0, 0, 1),
    tStartM: 0,
    tEndM: 1,
    ground: false,
    drawnHeightM: null,
    sunPoint: vec3(0, 0, 0),
    sunZenithDeg: 0,
  };
}

/**
 * Each ray's pixel at n steps, L + c ÷ n in every channel: its reference has converged, against
 * twice its steps, once c ÷ (2 n) falls under 0.05% of L or of 10⁻³ of the brightest reference.
 */
const PIXELS: Readonly<Record<string, (n: number) => number>> = {
  // Converged from 2,048 on.
  steady: (n) => 1 + 1e-9 / n,
  // c ÷ (2 n L) is 7.3 × 10⁻⁴ at 2,048 and 3.7 × 10⁻⁴ at 4,096.
  "at 4,096": (n) => 10 + 30 / n,
  // 1.5 × 10⁻³, 7.3 × 10⁻⁴, then 3.7 × 10⁻⁴ at 8,192.
  "at 8,192": (n) => 10 + 60 / n,
  // Never within 0.05%: it stops at 8,192.
  never: (n) => 10 + 1_000 / n,
  // The brightest reference, 1,000 at 2,048, which halves with each doubling and never converges,
  // so that the brightest the second level judges by is half the first's.
  bright: (n) => (1_000 * 2_048) / n,
  // Under the floor of 10⁻³ of the brightest: its 4 × 10⁻⁴ between 2,048 and 4,096 steps is within
  // 0.05% of the first level's floor (1) and not of the second's (0.5).
  "dim, judged again": (n) => 0.01 + 1.6384 / n,
};

/** A stand-in for a twin's pixel; with no schemes, the gate asks it only for placed steps. */
const pixelOf = (ray: GateRay, placement: Placement, n: number): Rgb3 => {
  if (placement !== "placed") {
    throw new Error(`a reference of ${placement} steps`);
  }
  const value = PIXELS[ray.name]?.(n) ?? Number.NaN;
  return [value, value, value];
};

describe("the gate's reference steps (decision-test-coverage R3(b))", () => {
  const gate = runGate(Object.keys(PIXELS).map(aRay), pixelOf, []);

  it("takes the coarsest of 2,048, 4,096 and 8,192 steps within 0.05% of twice its steps", () => {
    expect(gate.results.map(({ ray, referenceSteps }) => [ray.name, referenceSteps])).toEqual([
      ["steady", 2_048],
      ["at 4,096", 4_096],
      ["at 8,192", 8_192],
      ["never", 8_192],
      ["bright", 8_192],
      // Within 0.05% at the first level, not at the second, whose brightest is dimmer.
      ["dim, judged again", 4_096],
    ]);
  });

  it("gives each ray the pixels of its own steps and of twice them", () => {
    const mislabelled = gate.results.filter(
      ({ ray, referenceSteps, reference, confirm }) =>
        reference[1] !== pixelOf(ray, "placed", referenceSteps)[1] ||
        confirm[1] !== pixelOf(ray, "placed", 2 * referenceSteps)[1],
    );
    expect(mislabelled.map(({ ray }) => ray.name)).toEqual([]);
  });

  it("states the brightest of the references it ends with", () => {
    expect(gate.lMax.get("skyView")).toEqual([250, 250, 250]);
  });
});

/** A gate ray's end's height above the gate's sphere, m. */
function heightOf(ray: GateRay): number {
  const end = add(ray.o, scale(ray.d, ray.tEndM));
  return Math.hypot(end.x, end.y, end.z) - BOTTOM_M;
}

describe("the drawn surfaces' family (R08.T11.b)", () => {
  const rays = drawnSurfaceRays();
  const rule = TABLE_SIZES.high.marchDepthSteps;

  it("draws 188 surfaces, 168 from orbit and 20 from inside the air", () => {
    expect([rays.length, rays.filter(({ tStartM }) => tStartM > 0).length]).toEqual([188, 168]);
  });

  it("puts each surface on the ground or within the band's 25 km, on a ray that meets the ground", () => {
    const misplaced = rays.filter((ray) => {
      const ground = sphereHits(ray.o, ray.d, BOTTOM_M);
      const onGround = ground !== null && ray.tEndM === ground[0];
      const drawn = ray.drawnHeightM ?? Number.NaN;
      return (
        ground === null ||
        !(ground[0] > 0 && ray.tEndM <= ground[0]) ||
        !ray.ground ||
        (onGround ? drawn !== 0 : Math.abs(drawn - heightOf(ray)) > 1e-9 * ray.tEndM)
      );
    });
    expect(misplaced.map(({ name }) => name)).toEqual([]);
  });

  it("stands half the orbit's surfaces at the band's 25 km edge, and the inside ones no higher", () => {
    const edges = rays.filter(({ drawnHeightM }) => drawnHeightM !== 0);
    expect(edges.every(({ drawnHeightM }) => (drawnHeightM ?? Number.NaN) <= 25_000)).toBe(true);
    expect(
      edges.filter(({ tStartM, drawnHeightM }) => tStartM > 0 && drawnHeightM === 25_000),
    ).toHaveLength(104);
  });

  it("puts every surface past the aerial perspective's reach from inside the air, where the march draws it", () => {
    const inside = rays.filter(({ tStartM }) => tStartM <= 0);
    expect(inside).toHaveLength(20);
    expect(
      inside.filter(({ tEndM }) => !(tEndM > TABLE_SIZES.high.aerialPerspectiveReachM)),
    ).toEqual([]);
  });

  it("counts every drawn surface as the bare ground under high's rule, and as a drawn surface under low's", () => {
    expect(new Set(rays.map((ray) => marchKindOf(ray, rule)))).toEqual(new Set(["ground"]));
    expect(new Set(rays.map((ray) => marchKindOf(ray, null)))).toEqual(new Set(["surface"]));
  });

  it("puts each ray's sun at its zenith angle at the drawn surface", () => {
    const misplaced = rays.filter(
      (ray) => !(Math.abs(sunZenithAtDeg(ray) - ray.sunZenithDeg) < 1e-9),
    );
    expect(misplaced.map(({ name }) => name)).toEqual([]);
  });
});
