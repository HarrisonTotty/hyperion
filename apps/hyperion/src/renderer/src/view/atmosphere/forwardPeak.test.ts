import { beforeAll, describe, expect, it } from "vitest";

import { quaternion } from "../camera/quaternion";
import { R05_EARTH_REFERENCE } from "./earth";
import {
  FORWARD_PEAK_COSINE,
  FORWARD_PEAK_ENTRIES,
  FORWARD_PEAK_GRID_ENTRIES,
  FORWARD_PEAK_INTEGRAL,
  FORWARD_PEAK_SHAPE,
  FORWARD_PEAK_U,
  forwardPeakEntry,
  phaseTableMoment,
} from "./forwardPeak";
import { type AtmosphereCamera, type SpheroidFigure, tableRadiusM } from "./hillaire";
import { type AtmosphereMedium, type MediumTerm, type PhaseTable, phaseTable } from "./medium";
import { PHASE_TABLE_U, PHASE_TABLE_SIZE } from "./sizeDistribution";
import { kernelMedium, packMedium, PHASE_TABLE_ENTRIES, withoutForwardDelta } from "./tables";
import {
  frameTwinView,
  kernelOpticalDepth,
  type PlanetTwin,
  planetTwin,
  skyViewRayTwin,
  skyViewTexelsTwin,
} from "./tablesCpu";

/** WGS 84 (NIMA TR8350.2), and the tables' ground R₁. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};
const R1 = tableRadiusM(WGS84);

/** Each channel's delta share: as a large crystal's or dust's, 4% to 40% (Design note 6's range). */
const SHARES = [0.3, 0.2, 0.1] as const;

/** Henyey–Greenstein's phase at cosine μ, sr⁻¹. */
function henyeyGreenstein(g: number, mu: number): number {
  return (1 - g * g) / (4 * Math.PI * (1 + g * g - 2 * g * mu) ** 1.5);
}

/** A channel of Henyey–Greenstein on the kernels' grid, normalised to integrate to 1 as read. */
function hgChannel(g: number): Float64Array {
  const raw = FORWARD_PEAK_U.map((u) => henyeyGreenstein(g, Math.cos(Math.PI * u * u)));
  const integral = phaseTableMoment(FORWARD_PEAK_U, raw, () => 1);
  return raw.map((v) => v / integral);
}

/** The resolved lobe of each channel: Henyey–Greenstein at g 0.8, 0.75 and 0.7. */
const LOBES = [hgChannel(0.8), hgChannel(0.75), hgChannel(0.7)] as const;

/** A table as `aerosolPhaseTable` lays it: (1 − f) of the resolved lobe and the delta of f. */
function tableWithDelta(): PhaseTable {
  const values = LOBES.map((lobe, c) => {
    const share = SHARES[c] ?? 0;
    return lobe.map((v, k) => v * (1 - share) + forwardPeakEntry(share, k));
  });
  const element = (scale: number, delta: boolean): [Float64Array, Float64Array, Float64Array] => {
    const [r, g, b] = LOBES.map((lobe, c) => {
      const share = SHARES[c] ?? 0;
      return lobe.map((v, k) => v * (1 - share) * scale + (delta ? forwardPeakEntry(share, k) : 0));
    });
    if (r === undefined || g === undefined || b === undefined) {
      throw new Error("three channels");
    }
    return [r, g, b];
  };
  const [r, g, b] = values;
  if (r === undefined || g === undefined || b === undefined) {
    throw new Error("three channels");
  }
  return phaseTable(
    FORWARD_PEAK_U,
    [r, g, b],
    {
      a2: element(1, true),
      a3: element(0.9, true),
      a4: element(0.8, true),
      b1: element(-0.1, false),
      b2: element(0.05, false),
    },
    [...SHARES],
  );
}

/** A thin aerosol term with the delta, at 1.2 km scale height. */
function deltaTerm(table: PhaseTable): MediumTerm {
  return {
    name: "aerosol with a forward delta",
    density: { kind: "exponential", scaleHeightM: 1_200 },
    scattering: [4e-6, 4.2e-6, 4.4e-6],
    absorption: [3e-7, 3e-7, 3e-7],
    phase: { kind: "tabulated", table },
  };
}

/** R05's Rayleigh with a term. */
function mediumWith(term: MediumTerm): AtmosphereMedium {
  return {
    ...R05_EARTH_REFERENCE,
    name: `R05's Rayleigh with ${term.name}`,
    terms: [...R05_EARTH_REFERENCE.terms.filter((t) => t.phase.kind === "rayleigh"), term],
  };
}

describe("the forward delta's shape (R08.T5.c, R08.T7)", () => {
  it("stands on the kernels' grid, the aerosol tables' own", () => {
    expect([FORWARD_PEAK_GRID_ENTRIES, PHASE_TABLE_ENTRIES, PHASE_TABLE_SIZE]).toEqual([
      256, 256, 256,
    ]);
    expect(FORWARD_PEAK_U).toEqual(PHASE_TABLE_U);
    expect(FORWARD_PEAK_SHAPE.slice(0, FORWARD_PEAK_ENTRIES + 1)).toEqual(
      Float64Array.from([1, 0.75, 0.5, 0.25, 0]),
    );
    // Its integral as read lays a share f at f, and its mean cosine is within 0.044° of forward.
    expect(
      phaseTableMoment(
        FORWARD_PEAK_U,
        FORWARD_PEAK_U.map((_, k) => forwardPeakEntry(0.3, k)),
        () => 1,
      ),
    ).toBeCloseTo(0.3, 12);
    expect(FORWARD_PEAK_INTEGRAL).toBeGreaterThan(0);
    expect(1 - FORWARD_PEAK_COSINE).toBeLessThan(1 - Math.cos((0.044 * Math.PI) / 180));
  });
});

describe("the forward delta taken out of the kernels' medium (R08.T7, Design note 6)", () => {
  const full = tableWithDelta();
  const term = deltaTerm(full);
  const direct = withoutForwardDelta(term);

  it("multiplies the term's scattering by 1 − f and leaves its absorption", () => {
    expect(direct.scattering).toEqual(term.scattering.map((s, c) => s * (1 - (SHARES[c] ?? 0))));
    expect(direct.absorption).toBe(term.absorption);
  });

  it("leaves a table that integrates to 1, with mean cosine (g − f μ_δ) ÷ (1 − f), to 10⁻⁶", () => {
    if (direct.phase.kind !== "tabulated") {
      throw new Error("the term stays tabulated");
    }
    const kept = direct.phase.table;
    expect(kept.forwardPeak).toEqual([0, 0, 0]);
    for (const c of [0, 1, 2] as const) {
      const f = SHARES[c];
      const g = phaseTableMoment(full.u, full.values[c], (mu) => mu);
      expect(phaseTableMoment(kept.u, kept.values[c], () => 1)).toBeCloseTo(1, 6);
      expect(
        phaseTableMoment(kept.u, kept.values[c], (mu) => mu) -
          (g - f * FORWARD_PEAK_COSINE) / (1 - f),
      ).toBeCloseTo(0, 6);
      // a₂, a₃ and a₄ lose the delta; b₁ and b₂, which never carried it, are only rescaled.
      const lobe = LOBES[c];
      for (const k of [0, 3, 10, 100]) {
        expect(kept.matrix?.a2[c][k] ?? Number.NaN).toBeCloseTo(lobe[k] ?? Number.NaN, 9);
        expect(kept.matrix?.a3[c][k] ?? Number.NaN).toBeCloseTo(0.9 * (lobe[k] ?? Number.NaN), 9);
        expect(kept.matrix?.b1[c][k] ?? Number.NaN).toBeCloseTo(-0.1 * (lobe[k] ?? Number.NaN), 12);
      }
    }
  });

  it("leaves a term with no delta unchanged, bit for bit", () => {
    for (const each of R05_EARTH_REFERENCE.terms) {
      expect(withoutForwardDelta(each)).toBe(each);
    }
    const noDelta = deltaTerm(phaseTable(FORWARD_PEAK_U, [LOBES[0], LOBES[1], LOBES[2]]));
    expect(withoutForwardDelta(noDelta)).toBe(noDelta);
    expect(packMedium(kernelMedium(mediumWith(noDelta)))).toEqual(packMedium(mediumWith(noDelta)));
  });

  it("refuses a delta on a table off the kernels' grid, where its shape is not defined", () => {
    const u = Float64Array.of(0, 0.5, 1);
    const flat = Float64Array.of(1, 1, 1).map((v) => v / (4 * Math.PI));
    const off = deltaTerm(phaseTable(u, [flat, flat, flat], undefined, [0.1, 0, 0]));
    expect(() => withoutForwardDelta(off)).toThrow(/forward delta stands on a table of 3 entries/u);
  });

  it.each([
    [1, 0, 0],
    [0, -0.1, 0],
    [0, 0, Number.NaN],
  ] as const)("refuses a delta's share outside [0, 1) a channel: %s, %s, %s", (r, g, b) => {
    expect(() =>
      phaseTable(FORWARD_PEAK_U, [LOBES[0], LOBES[1], LOBES[2]], undefined, [r, g, b]),
    ).toThrow(/forward delta takes a share in \[0, 1\)/u);
  });

  it("dims a source through the term by e^(−τ_ext + f τ_sca) in each channel", () => {
    const medium = mediumWith(term);
    const alone: AtmosphereMedium = {
      ...medium,
      name: "the term's scattering alone",
      terms: [{ ...term, absorption: [0, 0, 0] }],
    };
    for (const [r, mu] of [
      [R1, 1],
      [R1 + 2_000, 0.2],
      [R1 + 500, 0.02],
    ] as const) {
      const kept = kernelOpticalDepth(kernelMedium(medium), R1, r, mu);
      const whole = kernelOpticalDepth(medium, R1, r, mu);
      const scattering = kernelOpticalDepth(alone, R1, r, mu);
      for (const c of [0, 1, 2] as const) {
        const expected = whole[c] - SHARES[c] * scattering[c];
        expect(kept[c] / expected - 1).toBeCloseTo(0, 9);
      }
    }
  });

  describe("on a sky-view texel straight at the source", () => {
    const medium = mediumWith(term);
    const byHand = mediumWith({
      ...term,
      scattering: [4e-6 * 0.7, 4.2e-6 * 0.8, 4.4e-6 * 0.9],
      phase: {
        kind: "tabulated",
        table: phaseTable(FORWARD_PEAK_U, [LOBES[0], LOBES[1], LOBES[2]]),
      },
    });
    const at: AtmosphereCamera = {
      positionM: { x: WGS84.equatorialRadiusM + 2, y: 0, z: 0 },
      orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
      fovXRad: Math.PI / 2,
      viewport: { widthPx: 8, heightPx: 4 },
    };
    const size = { widthTexels: 24, heightTexels: 16 };
    let kernelTwin: PlanetTwin;
    let handTwin: PlanetTwin;

    beforeAll(() => {
      kernelTwin = planetTwin(kernelMedium(medium), R1);
      handTwin = planetTwin(byHand, R1);
    }, 120_000);

    it("equals the texel the medium gives with the delta removed by hand, to 10⁻³", () => {
      // Row 4's ray in the sky frame, its sun's own azimuth (column 0), turned into the body frame:
      // the camera's normal is +x and the sky axis north, +z.
      const probe = frameTwinView(
        at,
        [
          {
            directionBodyFixed: { x: 0, y: 0, z: 1 },
            illuminanceLx: [1, 1, 1],
            angularRadiusRad: 0,
          },
        ],
        WGS84,
        0.1,
        1,
        false,
        0,
      );
      const ray = skyViewRayTwin(medium, probe, 0, 4, size);
      const d = ray.direction;
      const sun = { x: d.z, y: 0, z: d.x };
      const suns = [
        { directionBodyFixed: sun, illuminanceLx: [1, 1, 1] as const, angularRadiusRad: 0 },
      ];
      const view = frameTwinView(at, suns, WGS84, 0.1, 1, false, 0);
      const kernel = skyViewTexelsTwin(kernelTwin, view, size, 32)(0, 4);
      const hand = skyViewTexelsTwin(handTwin, view, size, 32)(0, 4);
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(kernel[c] / hand[c] - 1)).toBeLessThan(1e-3);
      }
    });
  });
});
