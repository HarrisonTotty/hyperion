import { describe, expect, it } from "vitest";

import { patternedThickBake } from "../../test/thickTables";
import { packDiscAir } from "./discReflectance";
import { fromHalfBits } from "../sky/half";
import {
  DISC_LAYER_BYTES,
  DISC_LAYER_VALUES,
  DISC_SUN_COSINES,
  DISC_VIEW_COSINES,
  discTableBytes,
  discTableHalves,
  discTableLayers,
  discTexelIndex,
  discThermalAt,
  discViewCosineAt,
} from "./discTable";
import type { AtmosphereMedium } from "./medium";
import { tablesPlanOf } from "./tables";
import { blackBodyLuminance, type ThermalSource, thermalSourceOf } from "./thermalSource";
import { bakeDiscReflectance, discThermalMarcher } from "./thick/discBake";
import { packThickTable } from "./thick/read";

/** A grey column over 100 km of vertical optical depth `tau` and albedo `omega`. */
function grey(tau: number, omega: number): AtmosphereMedium {
  const scaleHeightM = 8_000;
  const extinction = tau / (scaleHeightM * -Math.expm1(-100_000 / scaleHeightM));
  return {
    name: "grey",
    topHeightM: 100_000,
    groundAlbedo: [0, 0, 0],
    terms: [
      {
        name: "grey",
        density: { kind: "exponential", scaleHeightM },
        scattering: [omega * extinction, omega * extinction, omega * extinction],
        absorption: [(1 - omega) * extinction, (1 - omega) * extinction, (1 - omega) * extinction],
        phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
      },
    ],
  };
}

const PLAN = tablesPlanOf(null, 6_371_000);

/** The thermal source of a 100 km column isothermal at `temperatureK`. */
function isothermalSource(temperatureK: number): ThermalSource {
  return thermalSourceOf(
    {
      heightsM: Float64Array.of(0, 100_000),
      temperaturesK: Float64Array.of(temperatureK, temperatureK),
    },
    100_000,
  );
}

/** A grey column's disc-bake input, at 2,000 K by default, J of 0 everywhere (its own light alone). */
function inputOf(medium: AtmosphereMedium, thermal: ThermalSource = isothermalSource(2_000)) {
  const zero = patternedThickBake(PLAN.bands, medium, 1);
  for (const band of zero) {
    for (const slot of band.slots) {
      slot.values.fill(0);
    }
  }
  return {
    medium,
    curves: [],
    bands: PLAN.bands,
    source: packThickTable(zero, medium),
    thermal,
  };
}

describe("the disc table's own light (R08.T18.b)", () => {
  it("bakes an opaque isothermal column's own light as π B̃ at every view cosine", () => {
    const march = discThermalMarcher(inputOf(grey(60, 0)), 0);
    const b = blackBodyLuminance(2_000);
    for (const muView of [1, 0.5, 0.1]) {
      const glow = march(muView);
      for (const c of [0, 1, 2] as const) {
        expect(glow[c] / (Math.PI * Math.fround(b[c])) - 1).toBeCloseTo(0, 6);
      }
    }
  });

  /** A thin grey column's input, its table and its own march. */
  const thin = (() => {
    const input = inputOf(grey(0.5, 0.3));
    return { input, table: bakeDiscReflectance(input), march: discThermalMarcher(input, 0) };
  })();

  it("holds a thermal layer a band past the light slots'", () => {
    const { table } = thin;
    expect([table.bands, table.slots, discTableLayers(table), table.values.length]).toEqual([
      1,
      1,
      2,
      2 * DISC_LAYER_VALUES,
    ]);
    expect(discTableBytes(table)).toBe(2 * DISC_LAYER_BYTES);
  });

  it("reads its thermal layer back as the march's π L_th at a node", () => {
    const { table, march } = thin;
    // At a node, the read is the march's to the half floats' rounding.
    for (let v = 0; v < DISC_VIEW_COSINES; v += 5) {
      const muView = discViewCosineAt(v);
      const got = discThermalAt(table, [0], 1, muView);
      const want = march(muView);
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(got[c] / want[c] - 1)).toBeLessThan(1e-3);
      }
    }
  });

  it("scales its thermal layer by its brightest, the limb's, and gives the scale to the air block", () => {
    const { table, march } = thin;
    // The brightest is the scale: the limb's, the longest path.
    expect(table.thermalScale).toBeCloseTo(Math.max(...march(0)), 6);
    expect(
      packDiscAir({
        table,
        texture: { kind: "texture", name: "t" },
        lnBands: [0],
        gravity: null,
      })[2],
    ).toBe(Math.fround(table.thermalScale ?? Number.NaN));
  });

  it("holds no thermal layer for a medium that is not self-luminous", () => {
    const { thermal: _thermal, ...cold } = inputOf(grey(0.5, 0.3));
    const table = bakeDiscReflectance(cold);
    expect([table.thermalScale, discTableLayers(table)]).toEqual([undefined, 1]);
    expect(discThermalAt(table, [0], 1, 0.5)).toEqual([0, 0, 0]);
  });
});

describe("the disc table's signed own light (R08.T18.b)", () => {
  it("keeps a cool glow's negative blue through the halves it uploads and its read", () => {
    // At 1,500 K B̃'s blue is negative (Rec. 709's matching functions' negative lobe).
    const input = inputOf(grey(0.5, 0.3), isothermalSource(1_500));
    const table = bakeDiscReflectance(input);
    const march = discThermalMarcher(input, 0);
    const halves = discTableHalves(table);
    const layer = table.bands * table.slots;
    for (let v = 0; v < DISC_VIEW_COSINES; v += 5) {
      const muView = discViewCosineAt(v);
      const want = march(muView);
      const at = discTexelIndex(layer, DISC_SUN_COSINES - 1, v, 0) + 2;
      expect([want[2] < 0, (table.values[at] ?? 0) < 0]).toEqual([true, true]);
      expect(fromHalfBits(halves[at] ?? 0)).toBe(table.values[at]);
      expect(Math.abs(discThermalAt(table, [0], 1, muView)[2] / want[2] - 1)).toBeLessThan(1e-3);
    }
  });

  it("scales a glow negative in every channel by its largest magnitude, keeping its layer", () => {
    const warm = isothermalSource(2_000);
    const input = inputOf(grey(0.5, 0.3), { ...warm, planck: warm.planck.map((value) => -value) });
    const table = bakeDiscReflectance(input);
    const march = discThermalMarcher(input, 0);
    expect(table.thermalScale).toBeCloseTo(
      Math.max(...march(0).map((value) => Math.abs(value))),
      6,
    );
    for (let v = 0; v < DISC_VIEW_COSINES; v += 5) {
      const muView = discViewCosineAt(v);
      const got = discThermalAt(table, [0], 1, muView);
      const want = march(muView);
      for (const c of [0, 1, 2] as const) {
        expect(want[c]).toBeLessThan(0);
        expect(Math.abs(got[c] / want[c] - 1)).toBeLessThan(1e-3);
      }
    }
  });
});
