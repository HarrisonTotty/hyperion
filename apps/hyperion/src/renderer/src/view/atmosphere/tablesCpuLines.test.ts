import { describe, expect, it } from "vitest";

import { fiveLineSlotMedium, lineAbsorberMedium } from "../../test/atmosphereFixtures";
import { absorberTransmittance, scaledAmount } from "./absorbers";
import { ABSORBER_PRESSURE_REFERENCE_PA } from "./medium";
import {
  absorberAmounts,
  absorberAmountsToTop,
  absorberPressuresPa,
  curvesOpticalDepth,
  maxDistanceM,
  opticalDepth,
  transmittanceUvToRMu,
  unitFromTexelUv,
} from "./opticalDepth";
import { absorberSlots, lineSlotMask, pressureChannelOf } from "./tables";
import {
  curveOpticalDepth,
  kernelRayToTop,
  marchTwin,
  placedSteps,
  type PlanetTwin,
  tableSources,
  transmittanceTablesTwin,
  type TwinRay,
} from "./tablesCpu";

/** The twin's channels. */
const CHANNELS = [0, 1, 2] as const;

/** The tables' ground: WGS 84's mean radius, m (IUGG's R₁). */
const R1 = 6_371_008.8;

const { medium, curves } = lineAbsorberMedium();
/** The line slot: the fixture puts its pair first, so that the slot's pressure channel is not its own. */
const LINE_SLOT = 1;
const lines = curves[0]?.[LINE_SLOT];
/** Each absorber slot's pressure channel, −1 for the pair, which has no lines. */
const channels = absorberSlots(medium).map(({ slot }) =>
  pressureChannelOf(lineSlotMask(medium), slot),
);
const shell = { bottomRadiusM: R1, topRadiusM: R1 + medium.topHeightM };
/** A small transmittance table, enough texels to span the heights and angles. */
const SIZE = { widthTexels: 12, heightTexels: 8 };

describe("a line absorber's curve as the kernels read it (R08.T4.d)", () => {
  it("has more pressure rows than the read's stencil", () => {
    expect(lines?.logPressures.length ?? 0).toBeGreaterThan(4);
  });

  it("stands on a medium with a pressure profile", () => {
    expect(medium.pressureBar).toBeDefined();
  });

  it("takes pressure channel 0 in absorber slot 1, its pair in slot 0 taking none", () => {
    expect(channels).toEqual([-1, 0]);
  });

  it("reads by the kernels' rule exactly as absorbers.ts's absorberTransmittance reads it, at each pressure", () => {
    const slot = absorberSlots(medium)[LINE_SLOT];
    if (lines === undefined || slot === undefined) {
      throw new Error("the fixture has a line absorber");
    }
    let worst = 0;
    // Below the first row, between rows where they halve, at the ground's, past the read's top.
    for (const pressurePa of [3, 300, 4.2e4, 1e5, 3e5, 2e7]) {
      for (let i = 0; i <= 600; i += 3) {
        const x = 10 ** (-4 + i / 100);
        const rule = absorberTransmittance(lines, scaledAmount(slot.verticalAmount, x), pressurePa);
        const read = curveOpticalDepth(lines, x, pressurePa);
        for (const c of CHANNELS) {
          worst = Math.max(worst, Math.abs(read[c] + Math.log(rule[c])));
        }
      }
    }
    expect(worst).toBeLessThan(1e-12);
  });

  it("refuses a read with no pressure, as absorberTransmittance does", () => {
    if (lines === undefined) {
      throw new Error("the fixture has a line absorber");
    }
    expect(() => curveOpticalDepth(lines, 1, undefined)).toThrow(RangeError);
  });
});

describe("a line absorber through the transmittance tables (R08.T4.d)", () => {
  const tables = transmittanceTablesTwin(medium, R1, curves, SIZE);

  it("keep the fifth channel's table to one texel, the medium having one line slot", () => {
    const last = tables.lastAbsorberPressureColumn;
    expect([last.widthTexels, last.heightTexels, [...last.texels]]).toEqual([1, 1, [0, 0, 0, 0]]);
  });
  const texels = Array.from({ length: SIZE.widthTexels * SIZE.heightTexels }, (_, i) => {
    const x = i % SIZE.widthTexels;
    const y = Math.floor(i / SIZE.widthTexels);
    return {
      i,
      ...transmittanceUvToRMu(
        shell,
        unitFromTexelUv((x + 0.5) / SIZE.widthTexels, SIZE.widthTexels),
        unitFromTexelUv((y + 0.5) / SIZE.heightTexels, SIZE.heightTexels),
      ),
    };
  });

  it("stores each line slot's pressure-weighted column in its channel, 10⁵ Pa × w ÷ x the oracle's p̄", () => {
    let worst = 0;
    for (const { rM, mu } of texels) {
      const ray = kernelRayToTop(medium, R1, rM, mu);
      const oracle = absorberPressuresPa(medium, R1, rM, mu, true);
      for (const [k, expected] of oracle.entries()) {
        const c = channels[k] ?? -1;
        const x = ray.columns[k] ?? Number.NaN;
        const w = ray.pressureColumns[c] ?? Number.NaN;
        if (c >= 0 && expected !== undefined && x > 1e-6) {
          worst = Math.max(
            worst,
            Math.abs((ABSORBER_PRESSURE_REFERENCE_PA * w) / x / expected - 1),
          );
        }
      }
    }
    // The midpoint rule over 256 steps against Simpson's over the oracle's: both resolve the
    // column's levels to a fraction of their steps.
    expect(worst).toBeLessThan(1e-3);
  });

  /** The tables read as the per-frame kernels read them, light slot 0. */
  const sources = tableSources({
    medium,
    bottomRadiusM: R1,
    ...tables,
    multiScattering: { widthTexels: 1, heightTexels: 1, texels: new Float64Array(4) },
  });

  it("read a path whose pressure-weighted column underflowed to 0 at the curve's first row", () => {
    if (lines === undefined) {
      throw new Error("the fixture has a line absorber");
    }
    const x = 0.3;
    const columns = new Float64Array(5);
    columns[LINE_SLOT] = x;
    const out: [number, number, number] = [0, 0, 0];
    sources.absorberPathDepth(columns, new Float64Array(5), out);
    const first = curveOpticalDepth(lines, x, Math.exp(lines.logPressures[0] ?? Number.NaN));
    expect(Math.max(...CHANNELS.map((c) => Math.abs(out[c] - first[c])))).toBeLessThan(1e-12);
  });

  /** A texel's ray as the twin integrates it, and its curves read at its own columns and p̄. */
  function ownAbsorbed(
    rM: number,
    mu: number,
  ): {
    readonly tau: readonly [number, number, number];
    readonly absorbed: readonly [number, number, number];
  } {
    const ray = kernelRayToTop(medium, R1, rM, mu);
    const slots = absorberSlots(medium);
    const absorbed = curvesOpticalDepth(
      curves[0] ?? [],
      slots.map((slot, k) => scaledAmount(slot.verticalAmount, ray.columns[k] ?? 0)),
      slots.map((_, k) => {
        const c = channels[k] ?? -1;
        return c < 0
          ? undefined
          : (ABSORBER_PRESSURE_REFERENCE_PA * (ray.pressureColumns[c] ?? 0)) /
              (ray.columns[k] ?? 1);
      }),
    );
    return { tau: ray.opticalDepth, absorbed };
  }

  it("give the sun's transmittance at each texel's own (h, μ) as its own ray's, each curve at its p̄", () => {
    const out: [number, number, number] = [0, 0, 0];
    let worst = 0;
    for (const { rM, mu } of texels) {
      sources.sunTransmittance(rM - R1, mu, out);
      const { tau, absorbed } = ownAbsorbed(rM, mu);
      for (const c of CHANNELS) {
        const own = Math.exp(-(tau[c] + absorbed[c]));
        worst = Math.max(worst, Math.abs(out[c] / own - 1));
      }
    }
    expect(worst).toBeLessThan(1e-9);
  });

  it("absorb at each texel what the oracle's columns and p̄ give, to 10⁻³ of its depth", () => {
    let worst = 0;
    for (const { rM, mu } of texels) {
      const { absorbed } = ownAbsorbed(rM, mu);
      const oracle = curvesOpticalDepth(
        curves[0] ?? [],
        absorberAmountsToTop(medium, R1, rM, mu),
        absorberPressuresPa(medium, R1, rM, mu, true),
      );
      for (const c of CHANNELS) {
        worst = Math.max(worst, Math.abs(absorbed[c] - oracle[c]) / Math.max(oracle[c], 1e-6));
      }
    }
    expect(worst).toBeLessThan(1e-3);
  });

  it("march a view ray's own path by its whole column and p̄", () => {
    const planet: PlanetTwin = {
      medium,
      bottomRadiusM: R1,
      ...tables,
      multiScattering: { widthTexels: 1, heightTexels: 1, texels: new Float64Array(4) },
    };
    const rM = R1 + 2_000;
    const mu = 0.2;
    const ray: TwinRay = {
      originM: { x: 0, y: 0, z: rM },
      direction: { x: Math.sqrt(1 - mu * mu), y: 0, z: mu },
      sun: { x: 0, y: 0, z: 1 },
      tStartM: 0,
      tEndM: maxDistanceM(shell, rM, mu),
      fromCamera: true,
    };
    const marched = marchTwin(
      medium,
      tableSources(planet),
      { kind: "sphere", bottomRadiusM: R1 },
      ray,
      placedSteps(ray, 4_096),
    );
    const tau = opticalDepth(medium, R1, rM, mu);
    const absorbed = curvesOpticalDepth(
      curves[0] ?? [],
      absorberAmounts(medium, R1, rM, mu),
      absorberPressuresPa(medium, R1, rM, mu, false),
    );
    for (const c of CHANNELS) {
      const oracle = Math.exp(-(tau[c] + absorbed[c]));
      expect(Math.abs(marched.transmittance[c] / oracle - 1)).toBeLessThan(1e-3);
    }
  }, 60_000);
});

describe("a medium of five line slots through the transmittance tables (R08.T4.d)", () => {
  const five = fiveLineSlotMedium();
  const tables = transmittanceTablesTwin(five.medium, R1, five.curves, SIZE);
  const fiveShell = { bottomRadiusM: R1, topRadiusM: R1 + five.medium.topHeightM };

  it("takes a channel a line slot, the fifth in the last table", () => {
    const mask = lineSlotMask(five.medium);
    expect(absorberSlots(five.medium).map(({ slot }) => pressureChannelOf(mask, slot))).toEqual([
      0, 1, 2, 3, 4,
    ]);
  });

  it("keeps the fifth channel's table at the transmittance's size", () => {
    const last = tables.lastAbsorberPressureColumn;
    expect([last.widthTexels, last.heightTexels]).toEqual([SIZE.widthTexels, SIZE.heightTexels]);
  });

  it("stores the fifth line slot's pressure-weighted column in its r", () => {
    let worst = 0;
    for (let y = 0; y < SIZE.heightTexels; y += 1) {
      for (let x = 0; x < SIZE.widthTexels; x += 1) {
        const { rM, mu } = transmittanceUvToRMu(
          fiveShell,
          unitFromTexelUv((x + 0.5) / SIZE.widthTexels, SIZE.widthTexels),
          unitFromTexelUv((y + 0.5) / SIZE.heightTexels, SIZE.heightTexels),
        );
        const ray = kernelRayToTop(five.medium, R1, rM, mu);
        const stored = tables.lastAbsorberPressureColumn.texels[(y * SIZE.widthTexels + x) * 4];
        const own = ray.pressureColumns[4] ?? Number.NaN;
        worst = Math.max(worst, Math.abs((stored ?? Number.NaN) - own) / Math.max(own, 1e-30));
      }
    }
    // The table's own rays at its texels, the same midpoint rule: equal but for the stored order.
    expect(worst).toBeLessThan(1e-12);
  });

  it("give the sun's transmittance at a texel as its own ray's, the fifth slot at its p̄", () => {
    const sources = tableSources({
      medium: five.medium,
      bottomRadiusM: R1,
      ...tables,
      multiScattering: { widthTexels: 1, heightTexels: 1, texels: new Float64Array(4) },
    });
    const { rM, mu } = transmittanceUvToRMu(
      fiveShell,
      unitFromTexelUv(5.5 / SIZE.widthTexels, SIZE.widthTexels),
      unitFromTexelUv(2.5 / SIZE.heightTexels, SIZE.heightTexels),
    );
    const ray = kernelRayToTop(five.medium, R1, rM, mu);
    const slots = absorberSlots(five.medium);
    const mask = lineSlotMask(five.medium);
    const absorbed = curvesOpticalDepth(
      five.curves[0] ?? [],
      slots.map((slot, k) => scaledAmount(slot.verticalAmount, ray.columns[k] ?? 0)),
      slots.map(
        ({ slot }, k) =>
          (ABSORBER_PRESSURE_REFERENCE_PA *
            (ray.pressureColumns[pressureChannelOf(mask, slot)] ?? 0)) /
          (ray.columns[k] ?? 1),
      ),
    );
    const out: [number, number, number] = [0, 0, 0];
    sources.sunTransmittance(rM - R1, mu, out);
    for (const c of CHANNELS) {
      expect(Math.abs(out[c] / Math.exp(-(ray.opticalDepth[c] + absorbed[c])) - 1)).toBeLessThan(
        1e-9,
      );
    }
  });
});
