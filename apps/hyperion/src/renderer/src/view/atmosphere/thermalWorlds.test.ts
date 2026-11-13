import { beforeAll, describe, expect, it } from "vitest";

import { EARTH_REFERENCE } from "./earth";
import type { AtmosphereMedium } from "./medium";
import { marchTwin, planetTwin, type PlanetTwin, type TwinRay, tableSources } from "./tablesCpu";
import { REFLECTED_FLOOR_CD_M2, THERMAL_CUT } from "./thermal";
import {
  blackBodyLuminance,
  type MediumTemperature,
  selfLuminousOf,
  STELLAR_EFFICACY_LM_PER_W,
  thermalSourceOf,
} from "./thermalSource";
import { classifyRegime, CLEAR_COVER } from "./thick/regime";

/** A column isothermal at `temperatureK` up to `topM`. */
function isothermal(temperatureK: number, topM: number): MediumTemperature {
  return {
    heightsM: Float64Array.of(0, topM),
    temperaturesK: Float64Array.of(temperatureK, temperatureK),
  };
}

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
    const temperature = isothermal(2_700, HOT_GIANT.topHeightM);
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

// WASP-121b, its figures of the order L. Delrez et al. give (MNRAS 458 (2016) 4025; not re-read in
// the pod, which reached no source): an F6V star of about 6,460 K and 1.46 R☉, 0.0254 au away, the
// planet of about 1.8 R_J. Its dayside is taken isothermal at 2,700 K, the order of its dayside
// brightness temperature (T. M. Evans et al., Nature 548 (2017) 58), its continuum and molecular
// opacity as a grey absorber, opaque at τ 20, under H₂'s Rayleigh, and its scale height about
// kT ÷ (μ m_H g) with μ 2.3 and g about 9 m s⁻², some 1,000 km. The conclusion holds wherever its
// red reflectance is under about 0.075 at 2,700 K (the science check), as ultra-hot Jupiters'
// optical albedos are.
const STAR_TEMPERATURE_K = 6_460;
const STAR_RADIUS_M = 1.458 * 6.957e8;
const ORBIT_M = 0.025_44 * 1.495_978_707e11;
const PLANET_RADIUS_M = 1.807 * 7.1492e7;
const GIANT_SCALE_HEIGHT_M = 1.0e6;
const GIANT_TOP_M = 10 * GIANT_SCALE_HEIGHT_M;

/** A grey term of vertical optical depth `scattering` and `absorption` per channel. */
function greyTerm(
  name: string,
  scattering: readonly [number, number, number],
  absorption: number,
): AtmosphereMedium["terms"][number] {
  const column = GIANT_SCALE_HEIGHT_M * -Math.expm1(-GIANT_TOP_M / GIANT_SCALE_HEIGHT_M);
  const a = absorption / column;
  return {
    name,
    density: { kind: "exponential", scaleHeightM: GIANT_SCALE_HEIGHT_M },
    scattering: [scattering[0] / column, scattering[1] / column, scattering[2] / column],
    absorption: [a, a, a],
    phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
  };
}

const HOT_GIANT: AtmosphereMedium = {
  name: "ultra-hot giant",
  topHeightM: GIANT_TOP_M,
  groundAlbedo: [0, 0, 0],
  terms: [greyTerm("H2 Rayleigh", [0.03, 0.07, 0.17], 0), greyTerm("continuum", [0, 0, 0], 20)],
};

describe("an ultra-hot giant's dayside (R08.T18.b)", () => {
  let planet: PlanetTwin | undefined;
  beforeAll(() => {
    planet = planetTwin(HOT_GIANT, PLANET_RADIUS_M);
  }, 120_000);

  it("glows in the red brighter than it reflects its star there, at its substellar point", () => {
    if (planet === undefined) {
      throw new Error("the giant's tables are built before the test");
    }
    // The star's illuminance per channel: its black body's luminance over its disc's solid angle.
    const starDisc = Math.PI * (STAR_RADIUS_M / ORBIT_M) ** 2;
    const starLuminance = blackBodyLuminance(STAR_TEMPERATURE_K);
    const scale = 1e-6;
    const weight = starLuminance.map((l) => l * starDisc * scale);
    const zenith = { x: 0, y: 0, z: 1 };
    const steps = 192;
    const dtM = GIANT_TOP_M / steps;
    const ray: TwinRay = {
      originM: { x: 0, y: 0, z: PLANET_RADIUS_M + GIANT_TOP_M },
      direction: { x: 0, y: 0, z: -1 },
      sun: zenith,
      tStartM: 0,
      tEndM: GIANT_TOP_M,
      fromCamera: false,
      sources: [
        { direction: zenith, weight: [weight[0] ?? 0, weight[1] ?? 0, weight[2] ?? 0], slot: 0 },
      ],
    };
    const placed = Array.from({ length: steps }, (_, i) => ({ tM: (i + 0.5) * dtM, dtM }));
    const geometry = { kind: "sphere", bottomRadiusM: PLANET_RADIUS_M } as const;
    const reflected = marchTwin(HOT_GIANT, tableSources(planet), geometry, ray, placed);
    const glowing = marchTwin(
      HOT_GIANT,
      tableSources({
        ...planet,
        thermal: { source: thermalSourceOf(isothermal(2_700, GIANT_TOP_M), GIANT_TOP_M), scale },
      }),
      geometry,
      ray,
      placed,
    );
    const emittedRed = glowing.luminance[0] - reflected.luminance[0];
    expect(reflected.luminance[0]).toBeGreaterThan(0);
    expect(emittedRed).toBeGreaterThan(reflected.luminance[0]);
    // Opaque, it emits its black body at the share of its extinction that absorbs, 20 ÷ 20.03 in red.
    const want = blackBodyLuminance(2_700)[0] * scale * (20 / 20.03);
    expect(emittedRed / want).toBeCloseTo(1, 6);
  });
});
