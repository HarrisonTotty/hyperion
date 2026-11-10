import { beforeAll, describe, expect, it } from "vitest";

import {
  HOT_GIANT,
  HOT_GIANT_DAYSIDE_K,
  HOT_GIANT_ORBIT_M,
  HOT_GIANT_RADIUS_M,
  HOT_GIANT_STAR_RADIUS_M,
  HOT_GIANT_STAR_TEMPERATURE_K,
  HOT_GIANT_TOP_M,
  isothermal,
} from "../../test/glowingAirs";
import { EARTH_REFERENCE } from "./earth";
import { marchTwin, planetTwin, type PlanetTwin, type TwinRay, tableSources } from "./tablesCpu";
import { REFLECTED_FLOOR_CD_M2, THERMAL_CUT } from "./thermal";
import {
  blackBodyLuminance,
  selfLuminousOf,
  STELLAR_EFFICACY_LM_PER_W,
  thermalSourceOf,
} from "./thermalSource";
import { classifyRegime, CLEAR_COVER } from "./thick/regime";

/** The nominal solar constant, W m⁻² (IAU 2015 Resolution B3). */
const SOLAR_CONSTANT_W_M2 = 1_361;

describe("the thermal cut and selfLuminous (R08.T18.b)", () => {
  it("leaves a 300 K world's medium unset, its emission under 10⁻⁶ of its reflected light in every channel", () => {
    const temperature = isothermal(300, EARTH_REFERENCE.topHeightM);
    expect(selfLuminousOf(EARTH_REFERENCE, temperature, SOLAR_CONSTANT_W_M2)).toBe(false);
    expect(
      classifyRegime(EARTH_REFERENCE, CLEAR_COVER, {
        temperature,
        stellarFluxWM2: SOLAR_CONSTANT_W_M2,
      }).selfLuminous,
    ).toBeUndefined();
    // Its black body bounds whatever it emits; its ground reflects A E ÷ π of the Sun at 1 au.
    const lux = SOLAR_CONSTANT_W_M2 * STELLAR_EFFICACY_LM_PER_W;
    const emitted = blackBodyLuminance(300);
    for (const c of [0, 1, 2] as const) {
      const reflected = (EARTH_REFERENCE.groundAlbedo[c] * lux) / Math.PI;
      expect(emitted[c]).toBeLessThan(THERMAL_CUT * reflected);
    }
  });

  it("sets an ultra-hot giant's selfLuminous", () => {
    const temperature = isothermal(HOT_GIANT_DAYSIDE_K, HOT_GIANT.topHeightM);
    expect(
      classifyRegime(HOT_GIANT, CLEAR_COVER, { temperature, stellarFluxWM2: 1e6 }).selfLuminous,
    ).toBe(true);
  });

  it("leaves a 300 K world no star lights unset, under 10⁻⁶ of the reflected floor's 10⁻⁵ cd m⁻²", () => {
    expect(THERMAL_CUT * REFLECTED_FLOOR_CD_M2).toBeGreaterThan(
      Math.max(...blackBodyLuminance(300)),
    );
    expect(selfLuminousOf(EARTH_REFERENCE, isothermal(300, 100_000), 0)).toBe(false);
  });
});

// WASP-121b: `test/glowingAirs.ts`' `HOT_GIANT`, its figures and their sources there.
describe("an ultra-hot giant's dayside (R08.T18.b)", () => {
  let planet: PlanetTwin | undefined;
  beforeAll(() => {
    planet = planetTwin(HOT_GIANT, HOT_GIANT_RADIUS_M);
  }, 120_000);

  it("glows in the red brighter than it reflects its star there, at its substellar point", () => {
    if (planet === undefined) {
      throw new Error("the giant's tables are built before the test");
    }
    // The star's illuminance per channel: its black body's luminance over its disc's solid angle.
    const starDisc = Math.PI * (HOT_GIANT_STAR_RADIUS_M / HOT_GIANT_ORBIT_M) ** 2;
    const starLuminance = blackBodyLuminance(HOT_GIANT_STAR_TEMPERATURE_K);
    const scale = 1e-6;
    const weight = starLuminance.map((l) => l * starDisc * scale);
    const zenith = { x: 0, y: 0, z: 1 };
    const steps = 192;
    const dtM = HOT_GIANT_TOP_M / steps;
    const ray: TwinRay = {
      originM: { x: 0, y: 0, z: HOT_GIANT_RADIUS_M + HOT_GIANT_TOP_M },
      direction: { x: 0, y: 0, z: -1 },
      sun: zenith,
      tStartM: 0,
      tEndM: HOT_GIANT_TOP_M,
      fromCamera: false,
      sources: [
        { direction: zenith, weight: [weight[0] ?? 0, weight[1] ?? 0, weight[2] ?? 0], slot: 0 },
      ],
    };
    const placed = Array.from({ length: steps }, (_, i) => ({ tM: (i + 0.5) * dtM, dtM }));
    const geometry = { kind: "sphere", bottomRadiusM: HOT_GIANT_RADIUS_M } as const;
    const reflected = marchTwin(HOT_GIANT, tableSources(planet), geometry, ray, placed);
    const glowing = marchTwin(
      HOT_GIANT,
      tableSources({
        ...planet,
        thermal: {
          source: thermalSourceOf(
            isothermal(HOT_GIANT_DAYSIDE_K, HOT_GIANT_TOP_M),
            HOT_GIANT_TOP_M,
          ),
          scale,
        },
      }),
      geometry,
      ray,
      placed,
    );
    const emittedRed = glowing.luminance[0] - reflected.luminance[0];
    expect(reflected.luminance[0]).toBeGreaterThan(0);
    expect(emittedRed).toBeGreaterThan(reflected.luminance[0]);
    // Opaque, it emits its black body at the share of its extinction that absorbs, 20 ÷ 20.03 in red.
    const want = blackBodyLuminance(HOT_GIANT_DAYSIDE_K)[0] * scale * (20 / 20.03);
    expect(emittedRed / want).toBeCloseTo(1, 6);
  });
});
