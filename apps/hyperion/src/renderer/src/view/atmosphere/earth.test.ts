import { describe, expect, it } from "vitest";

import bakeSpectra from "../../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import { photopicIlluminance } from "../lighting/illuminance";
import { AU_M } from "../scenes/kept";
import { absorberVerticalAmount, profileMixingRatio } from "./absorbers";
import {
  AEROSOL_ASYMMETRY,
  AEROSOL_OPTICAL_DEPTH_550,
  AFGL_US_STANDARD_OZONE_MB_PPMV,
  DOBSON_UNIT_PER_M2,
  EARTH_COLUMN,
  EARTH_DRY_AIR,
  EARTH_OZONE,
  EARTH_OZONE_LAYER_PA,
  EARTH_OZONE_PROFILE,
  EARTH_OZONE_SLAB,
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  EARTH_SKIN_K,
  EARTH_TOP_HEIGHT_M,
  HILLAIRE_REFERENCE,
  OZONE_ABSORPTION_PER_M,
  OZONE_COLUMN_DU,
  R05_EARTH_REFERENCE,
  R06_SOLAR_BAKE_SPECTRUM,
  R06_SOLAR_COLOUR_RGB,
  RAYLEIGH_SCALE_HEIGHT_M,
  RAYLEIGH_SCATTERING_PER_M,
  REFERENCE_SUN,
  referenceSunIlluminanceLx,
} from "./earth";
import {
  type AtmosphereMedium,
  CHANNEL_WAVELENGTHS_NM,
  columnLengthM,
  densityAt,
  extinction,
  layerEdgesM,
  type MediumTerm,
  termNamed,
} from "./medium";
import { molecularMixture } from "./rayleigh";
import { E490_ILLUMINANCE_LX } from "./solar";
import { deltaUv, luminanceOfRgb, sunWeights } from "./spectralColour";
import { checkAbsorberCurves, packMedium } from "./tables";

/** Boltzmann's constant, J K⁻¹ (SI 2019, exact). */
const BOLTZMANN_J_PER_K = 1.380_649e-23;

/** The atomic mass constant, kg (CODATA 2018). */
const ATOMIC_MASS_KG = 1.660_539_066_6e-27;

/** The US Standard Atmosphere 1976's sea level: 288.15 K and 101,325 Pa. */
const SEA_LEVEL_N_PER_M3 = 101_325 / (BOLTZMANN_J_PER_K * 288.15);

/** Bruneton's channel wavelengths, nm, at which R05's constants stand. */
const BRUNETON_NM = [680, 550, 440] as const;

/**
 * (n − 1) of standard air (15 °C, 101,325 Pa, 0.03% CO₂) at a vacuum wavelength: Peck and Reeder,
 * J. Opt. Soc. Am. 62 (1972) 958, as Bodhaine et al. 1999, eq. 4, quotes it.
 */
function refractivity(wavelengthUm: number): number {
  const s2 = 1 / (wavelengthUm * wavelengthUm);
  return (8_060.51 + 2_480_990 / (132.274 - s2) + 17_455.7 / (39.329_57 - s2)) * 1e-8;
}

/**
 * The King factor of air: Bates, Planet. Space Sci. 32 (1984) 785, N₂'s and O₂'s factors mixed
 * with Ar (1) and CO₂ (1.15) by volume, as Bodhaine et al., J. Atmos. Oceanic Technol. 16 (1999)
 * 1854, eqs. 5–6, quote them.
 */
function kingFactor(wavelengthUm: number, co2Percent = 0.03): number {
  const l2 = wavelengthUm * wavelengthUm;
  const n2 = 1.034 + 3.17e-4 / l2;
  const o2 = 1.096 + 1.385e-3 / l2 + 1.448e-4 / (l2 * l2);
  return (
    (78.084 * n2 + 20.946 * o2 + 0.934 * 1 + co2Percent * 1.15) /
    (78.084 + 20.946 + 0.934 + co2Percent)
  );
}

/** The Rayleigh cross-section of air, m²: 24π³ (n² − 1)² ÷ (λ⁴ N_s² (n² + 2)²) × F_K. */
function rayleighCrossSectionM2(wavelengthNm: number): number {
  const um = wavelengthNm / 1_000;
  const n = 1 + refractivity(um);
  const n2 = n * n;
  const lambdaM = wavelengthNm * 1e-9;
  return (
    ((24 * Math.PI ** 3 * (n2 - 1) ** 2) /
      (lambdaM ** 4 * SEA_LEVEL_N_PER_M3 ** 2 * (n2 + 2) ** 2)) *
    kingFactor(um)
  );
}

/**
 * Ozone's cross-sections at 680, 550 and 440 nm, m²: Serdyuchenko et al., Atmos. Meas. Tech. 7
 * (2014) 625, at 233 K, binned over 10 nm as Bruneton 2017's `kOzoneCrossSection` has them.
 */
const OZONE_CROSS_SECTION_M2 = [1.209e-25, 3.5e-25, 1.582e-26] as const;

describe("R05's Rayleigh term", () => {
  it("recomputes the coefficients from Peck and Reeder's refractivity and Bates's King factor to 0.5%", () => {
    for (const [c, wavelength] of BRUNETON_NM.entries()) {
      const beta = rayleighCrossSectionM2(wavelength) * SEA_LEVEL_N_PER_M3;
      expect(Math.abs(beta / (RAYLEIGH_SCATTERING_PER_M[c] ?? 0) - 1)).toBeLessThan(0.005);
    }
  });

  it("gives Bucholtz 1995's 4.51 × 10⁻²⁷ cm² at 550 nm to 1%", () => {
    const cm2 = rayleighCrossSectionM2(550) * 1e4;
    expect(Math.abs(cm2 / 4.51e-27 - 1)).toBeLessThan(0.01);
  });

  it("is the term R05's medium carries", () => {
    expect(termNamed(R05_EARTH_REFERENCE, "rayleigh").scattering).toEqual(
      RAYLEIGH_SCATTERING_PER_M,
    );
  });
});

describe("R05's ozone term", () => {
  it("recomputes the peak absorption from 300 DU over the 15 km tent and the cross-sections", () => {
    const peakPerM3 = (OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2) / 15_000;
    for (const [c, sigma] of OZONE_CROSS_SECTION_M2.entries()) {
      const expected = sigma * peakPerM3;
      expect(Math.abs(expected / (OZONE_ABSORPTION_PER_M[c] ?? 0) - 1)).toBeLessThan(0.005);
    }
  });

  it("absorbs only", () => {
    expect(termNamed(R05_EARTH_REFERENCE, "ozone").scattering).toEqual([0, 0, 0]);
  });
});

describe("R05's aerosol term", () => {
  it("integrates back to an optical depth of 0.1 at 550 nm", () => {
    const aerosol = termNamed(R05_EARTH_REFERENCE, "aerosol");
    const groundExtinction = extinction(aerosol)[1];
    let tau = 0;
    for (let h = 0.5; h < R05_EARTH_REFERENCE.topHeightM; h += 1) {
      tau += groundExtinction * densityAt(aerosol.density, h);
    }
    expect(tau).toBeCloseTo(AEROSOL_OPTICAL_DEPTH_550, 6);
  });

  it("follows the Ångström law with exponent 1.3 and scatters 92% of its extinction", () => {
    const aerosol = termNamed(R05_EARTH_REFERENCE, "aerosol");
    const [red, green, blue] = extinction(aerosol);
    expect(Math.log(red / green) / Math.log(550 / 680)).toBeCloseTo(1.3, 10);
    expect(Math.log(blue / green) / Math.log(550 / 440)).toBeCloseTo(1.3, 10);
    expect(aerosol.scattering[1] / green).toBeCloseTo(0.92, 12);
  });
});

describe("Hillaire's reference medium", () => {
  it("carries sebh's coefficients in m⁻¹", () => {
    expect(termNamed(HILLAIRE_REFERENCE, "rayleigh").scattering).toEqual([
      5.802e-6, 13.558e-6, 33.1e-6,
    ]);
    expect(extinction(termNamed(HILLAIRE_REFERENCE, "aerosol"))[0]).toBeCloseTo(4.44e-6, 12);
  });
});

describe("R05's Rayleigh column (decisions-r05.md item 3)", () => {
  it("takes the US Standard Atmosphere's sea-level scale height, R*T₀ ÷ (M₀g₀), to 0.1 m", () => {
    const heightM = (8.314_32 * 288.15) / (0.028_964_4 * 9.806_65);
    expect(Math.abs(RAYLEIGH_SCALE_HEIGHT_M - heightM)).toBeLessThan(0.1);
  });

  it("gives Bodhaine et al. 1999's τ_R(550 nm) ≈ 0.097 to 1%", () => {
    const h = RAYLEIGH_SCALE_HEIGHT_M;
    const tau = RAYLEIGH_SCATTERING_PER_M[1] * h * -Math.expm1(-100_000 / h);
    expect(Math.abs(tau / 0.097 - 1)).toBeLessThan(0.01);
  });
});

describe("Earth's aerosol phase (decisions-r05.md item 1)", () => {
  it("has a Cornette–Shanks mean cosine of 0.650, AERONET's continental asymmetry", () => {
    const g = AEROSOL_ASYMMETRY;
    const meanCosine = (3 * g * (4 + g * g)) / (5 * (2 + g * g));
    expect(Math.abs(meanCosine - 0.65)).toBeLessThan(0.002);
  });

  it("is both Earths' aerosol phase, while Hillaire's medium keeps his 0.8", () => {
    for (const medium of [EARTH_REFERENCE, R05_EARTH_REFERENCE]) {
      expect(termNamed(medium, "aerosol").phase).toEqual({
        kind: "cornette-shanks",
        asymmetry: AEROSOL_ASYMMETRY,
      });
    }
    expect(termNamed(HILLAIRE_REFERENCE, "aerosol").phase).toEqual({
      kind: "cornette-shanks",
      asymmetry: 0.8,
    });
  });
});

// --- Earth rebuilt (plan R08, R08.T6.d) -----------------------------------------------------------

describe("Earth's column, rebuilt", () => {
  it("has a scale height of about 8.4 km at the ground, kT ÷ (μ m_u g_ref)", () => {
    const scaleHeightM =
      (BOLTZMANN_J_PER_K * 288.15) /
      (EARTH_COLUMN.meanMolarMassGPerMol * ATOMIC_MASS_KG * EARTH_COLUMN.referenceGravityMS2);
    expect(scaleHeightM).toBeGreaterThan(8_400);
    expect(scaleHeightM).toBeLessThan(8_460);
    // Its column length, ∫ρ dz, is the ground's scale height to the spherical excess's 2H ÷ R.
    const lengthM = columnLengthM(EARTH_COLUMN.density, EARTH_COLUMN.topHeightM);
    expect(lengthM / scaleHeightM - 1).toBeGreaterThan(0);
    expect(lengthM / scaleHeightM - 1).toBeLessThan(0.004);
  });

  it("thins faster than R05's exponential in the stratosphere, its 214 K skin's", () => {
    expect(densityAt(EARTH_COLUMN.density, 30_000)).toBeLessThan(
      Math.exp(-30_000 / RAYLEIGH_SCALE_HEIGHT_M),
    );
    expect(
      densityAt(EARTH_COLUMN.density, 60_000) / Math.exp(-60_000 / RAYLEIGH_SCALE_HEIGHT_M),
    ).toBeLessThan(0.5);
  });

  it("stands above the medium's 100 km top, so that no density is held up to it", () => {
    expect(EARTH_COLUMN.topHeightM).toBeGreaterThan(EARTH_TOP_HEIGHT_M);
    expect(() => packMedium(EARTH_REFERENCE)).not.toThrow();
  });

  it("gives Bodhaine et al. 1999's τ_R(550 nm) = 0.097 to 1% through dry air, as R05's exponential does", () => {
    const tau =
      molecularMixture(EARTH_DRY_AIR).crossSectionM2(550) *
      EARTH_COLUMN.surfaceNumberDensityPerM3 *
      columnLengthM(EARTH_COLUMN.density, EARTH_TOP_HEIGHT_M);
    expect(Math.abs(tau / 0.097 - 1)).toBeLessThan(0.01);
  });
});

describe("Earth rebuilt", () => {
  it("takes R08.T3.c's molecular term on its column, named rayleigh, with air's depolarisation", () => {
    const rayleigh = termNamed(EARTH_REFERENCE, "rayleigh");
    expect(rayleigh.density).toBe(EARTH_COLUMN.density);
    if (rayleigh.phase.kind !== "rayleigh") {
      throw new Error("the molecular term's phase is Rayleigh's");
    }
    for (const rho of rayleigh.phase.depolarisation) {
      expect(rho).toBeGreaterThan(0.026);
      expect(rho).toBeLessThan(0.03);
    }
  });

  it("evaluates its molecular term at the fitted channels, its scattering λ⁻⁴ and a little steeper", () => {
    const [red, green, blue] = termNamed(EARTH_REFERENCE, "rayleigh").scattering;
    const [lr, lg, lb] = CHANNEL_WAVELENGTHS_NM;
    expect(Math.log(red / green) / Math.log(lg / lr)).toBeGreaterThan(4);
    expect(Math.log(blue / green) / Math.log(lg / lb)).toBeLessThan(4.3);
    expect(Math.log(blue / green) / Math.log(lg / lb)).toBeGreaterThan(4);
  });

  it("keeps R05's aerosol, at the fitted channels: 0.1 at 550 nm under the Ångström law", () => {
    const aerosol = termNamed(EARTH_REFERENCE, "aerosol");
    const [, green] = extinction(aerosol);
    const tau550 =
      green * 1_200 * (CHANNEL_WAVELENGTHS_NM[1] / 550) ** 1.3 * -Math.expm1(-100_000 / 1_200);
    expect(tau550).toBeCloseTo(AEROSOL_OPTICAL_DEPTH_550, 6);
    expect(aerosol.scattering[1] / green).toBeCloseTo(0.92, 12);
  });

  it("carries its ozone as an absorber term, absorbing by its curves alone", () => {
    const ozone = termNamed(EARTH_REFERENCE, "absorber:O3");
    expect(ozone.absorber?.spec).toBe(EARTH_OZONE);
    expect(extinction(ozone)).toEqual([0, 0, 0]);
  });

  it("gives its ozone 300 DU", () => {
    const amount = absorberVerticalAmount(EARTH_OZONE);
    expect(amount.kind === "column" ? amount.columnPerM2 : Number.NaN).toBeCloseTo(
      OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2,
      -15,
    );
  });

  it("is built with the reference Sun's curve of its ozone, which its tables accept", () => {
    expect(EARTH_REFERENCE_CURVES).toHaveLength(1);
    expect(() => {
      checkAbsorberCurves(EARTH_REFERENCE, EARTH_REFERENCE_CURVES);
    }).not.toThrow();
  });

  it("keeps R05's top and ground", () => {
    expect(EARTH_REFERENCE.topHeightM).toBe(R05_EARTH_REFERENCE.topHeightM);
    expect(EARTH_REFERENCE.groundAlbedo).toEqual(R05_EARTH_REFERENCE.groundAlbedo);
  });
});

// --- Earth's measured ozone (plan R08, R08.T6.h) ------------------------------------------------

/**
 * The U.S. Standard Atmosphere 1976 (NOAA-S/T 76-1562), Table 4: the base geopotential heights,
 * m′, of its layers to 84.852 km′, and their lapse rates, K m′⁻¹.
 */
const US76_BASE_HEIGHTS_M = [0, 11_000, 20_000, 32_000, 47_000, 51_000, 71_000, 84_852] as const;
const US76_LAPSE_K_PER_M = [-6.5e-3, 0, 1e-3, 2.8e-3, 0, -2.8e-3, -2e-3] as const;

/** g₀ M₀ ÷ R\*, K m⁻¹: the 1976 standard's 9.80665 m s⁻², 28.9644 g mol⁻¹ and 8.31432 J mol⁻¹ K⁻¹. */
const US76_GM_OVER_R = (9.806_65 * 0.028_964_4) / 8.314_32;

/** The 1976 standard's effective Earth radius r₀ for geopotential height, m: 6,356,766. */
const US76_RADIUS_M = 6_356_766;

/** T and p at each layer's base, from 288.15 K and 101,325 Pa at sea level. */
const US76_BASES: ReadonlyArray<{ readonly temperatureK: number; readonly pressurePa: number }> =
  US76_LAPSE_K_PER_M.reduce<{ temperatureK: number; pressurePa: number }[]>(
    (bases, lapse, b) => {
      const { temperatureK: t, pressurePa: p } = bases[b] ?? { temperatureK: NaN, pressurePa: NaN };
      const dh = (US76_BASE_HEIGHTS_M[b + 1] ?? NaN) - (US76_BASE_HEIGHTS_M[b] ?? NaN);
      bases.push({
        temperatureK: t + lapse * dh,
        pressurePa:
          lapse === 0
            ? p * Math.exp((-US76_GM_OVER_R * dh) / t)
            : p * (t / (t + lapse * dh)) ** (US76_GM_OVER_R / lapse),
      });
      return bases;
    },
    [{ temperatureK: 288.15, pressurePa: 101_325 }],
  );

/** The 1976 standard's T and p at a geometric height up to 86 km, by its eqs. 18, 23 and 33a–b. */
function us76At(heightM: number): { readonly temperatureK: number; readonly pressurePa: number } {
  const h = (US76_RADIUS_M * heightM) / (US76_RADIUS_M + heightM);
  const b = Math.max(
    0,
    US76_BASE_HEIGHTS_M.findLastIndex((base, i) => i < US76_LAPSE_K_PER_M.length && base <= h),
  );
  const { temperatureK: t, pressurePa: p } = US76_BASES[b] ?? {
    temperatureK: NaN,
    pressurePa: NaN,
  };
  const lapse = US76_LAPSE_K_PER_M[b] ?? NaN;
  const dh = h - (US76_BASE_HEIGHTS_M[b] ?? NaN);
  return {
    temperatureK: t + lapse * dh,
    pressurePa:
      lapse === 0
        ? p * Math.exp((-US76_GM_OVER_R * dh) / t)
        : p * (t / (t + lapse * dh)) ** (US76_GM_OVER_R / lapse),
  };
}

/** The 1976 standard's geometric height at a pressure, m, by bisection to 86 km. */
function us76HeightM(pressurePa: number): number {
  let low = 0;
  let high = 86_000;
  for (let i = 0; i < 80; i += 1) {
    const mid = 0.5 * (low + high);
    if (us76At(mid).pressurePa > pressurePa) {
      low = mid;
    } else {
      high = mid;
    }
  }
  return 0.5 * (low + high);
}

/** {@link EARTH_COLUMN}'s height at a pressure, m: linear in ln p between its levels. */
function columnHeightM(pressurePa: number): number {
  const { pressuresPa, altitudesM } = EARTH_COLUMN;
  const i = Math.max(
    0,
    pressuresPa.findLastIndex((p) => p >= pressurePa),
  );
  const p0 = pressuresPa[i] ?? NaN;
  const p1 = pressuresPa[i + 1] ?? NaN;
  const z0 = altitudesM[i] ?? NaN;
  const z1 = altitudesM[i + 1] ?? NaN;
  return z0 + ((z1 - z0) * Math.log(p0 / pressurePa)) / Math.log(p0 / p1);
}

/** The report's 50 levels, km: 1 km apart to 25 km, 2.5 km to 50 km, 5 km to 120 km. */
const AFGL_LEVELS_KM = [
  ...Array.from({ length: 26 }, (_, i) => i),
  ...Array.from({ length: 10 }, (_, i) => 27.5 + 2.5 * i),
  ...Array.from({ length: 14 }, (_, i) => 55 + 5 * i),
];

/** The tropopause of {@link EARTH_COLUMN}, m: its first level at the skin's temperature. */
const EARTH_TROPOPAUSE_M =
  EARTH_COLUMN.altitudesM[EARTH_COLUMN.temperaturesK.findIndex((t) => t <= EARTH_SKIN_K)] ?? NaN;

/** An ozone's figures on its column, from 1 m midpoint sums to the medium's top. */
interface OzoneFigures {
  /** Its vertical column, molecules m⁻². */
  readonly columnPerM2: number;
  /** Its column-weighted mean height, m. */
  readonly meanHeightM: number;
  /** The shares of its column below 18.6 km and below {@link EARTH_TROPOPAUSE_M}. */
  readonly belowBase: number;
  readonly belowTropopause: number;
  /** Its peak number density, m⁻³, and the peak's height, m. */
  readonly peakPerM3: number;
  readonly peakHeightM: number;
}

function ozoneFigures(medium: AtmosphereMedium): OzoneFigures {
  const ozone: MediumTerm = termNamed(medium, "absorber:O3");
  const n = ozone.absorber?.kind === "column" ? ozone.absorber.numberDensityPerM3 : Number.NaN;
  let column = 0;
  let moment = 0;
  let belowBase = 0;
  let belowTropopause = 0;
  let peakPerM3 = 0;
  let peakHeightM = 0;
  for (let h = 0.5; h < medium.topHeightM; h += 1) {
    const d = n * densityAt(ozone.density, h);
    column += d;
    moment += d * h;
    belowBase += h < 18_600 ? d : 0;
    belowTropopause += h < EARTH_TROPOPAUSE_M ? d : 0;
    if (d > peakPerM3) {
      peakPerM3 = d;
      peakHeightM = h;
    }
  }
  return {
    columnPerM2: column,
    meanHeightM: moment / column,
    belowBase: belowBase / column,
    belowTropopause: belowTropopause / column,
    peakPerM3,
    peakHeightM,
  };
}

/** Boltzmann's constant, J K⁻¹, as the 1976 standard takes it: 1.380622 × 10⁻²³. */
const US76_BOLTZMANN_J_PER_K = 1.380_622e-23;

describe("the AFGL U.S. Standard ozone, as read from AFGL-TR-86-0110's Table 1f", () => {
  it("has the report's 50 levels, from the 1976 standard's sea level, descending in pressure", () => {
    expect(AFGL_US_STANDARD_OZONE_MB_PPMV).toHaveLength(AFGL_LEVELS_KM.length);
    expect(AFGL_US_STANDARD_OZONE_MB_PPMV[0]).toEqual([1_013.25, 2.66e-2]);
    for (const [i, [pressureMb]] of AFGL_US_STANDARD_OZONE_MB_PPMV.entries()) {
      expect(pressureMb).toBeLessThan(AFGL_US_STANDARD_OZONE_MB_PPMV[i - 1]?.[0] ?? Infinity);
    }
  });

  it("prints the 1976 model's pressure at each level to 0.5%, but 32.5 and 37.5 km's, 3% off", () => {
    const off: string[] = [];
    for (const [i, [pressureMb]] of AFGL_US_STANDARD_OZONE_MB_PPMV.entries()) {
      const heightM = (AFGL_LEVELS_KM[i] ?? NaN) * 1_000;
      if (heightM <= 86_000) {
        const e = (pressureMb * 100) / us76At(heightM).pressurePa - 1;
        off.push(...(Math.abs(e) < 5e-3 ? [] : [`${heightM / 1_000} km ${e.toFixed(4)}`]));
      }
    }
    expect(off).toEqual(["32.5 km -0.0300", "37.5 km 0.0269"]);
  });

  it("holds 345.6 DU on the report's own heights, the 1976 standard's about 345", () => {
    // n = p ÷ (kT) at the 1976 model's T, trapezoids to 85 km, above which lies under 10⁻⁵ of it.
    let column = 0;
    let moment = 0;
    let below11 = 0;
    let below186 = 0;
    const atLevel = (i: number): { readonly z: number; readonly n: number } => {
      const z = (AFGL_LEVELS_KM[i] ?? NaN) * 1_000;
      const [pressureMb = NaN, ppmv = NaN] = AFGL_US_STANDARD_OZONE_MB_PPMV[i] ?? [];
      return {
        z,
        n: ((pressureMb * 100) / (US76_BOLTZMANN_J_PER_K * us76At(z).temperatureK)) * ppmv * 1e-6,
      };
    };
    for (let i = 1; AFGL_LEVELS_KM[i] !== undefined && (AFGL_LEVELS_KM[i] ?? NaN) <= 85; i += 1) {
      const a = atLevel(i - 1);
      const b = atLevel(i);
      for (let k = 0; k < 1_000; k += 1) {
        const f = (k + 0.5) / 1_000;
        const z = a.z + f * (b.z - a.z);
        const d = (a.n + f * (b.n - a.n)) * ((b.z - a.z) / 1_000);
        column += d;
        moment += d * z;
        below11 += z < 11_000 ? d : 0;
        below186 += z < 18_600 ? d : 0;
      }
    }
    expect(column / DOBSON_UNIT_PER_M2).toBeCloseTo(345.6, 1);
    // Recorded beside the measured Earth's: its mean height, and its shares below 18.6 km and the
    // 1976 standard's 11 km tropopause (`decision-r08-t6d-findings.md` §3: 22.4 km, 31% and 9%).
    expect(moment / column / 1_000).toBeCloseTo(22.55, 2);
    expect(below186 / column).toBeCloseTo(0.314, 3);
    expect(below11 / column).toBeCloseTo(0.088, 3);
  });
});

describe("Earth's measured ozone (R08.T6.h)", () => {
  const ozone = termNamed(EARTH_REFERENCE, "absorber:O3");
  const measured = ozoneFigures(EARTH_REFERENCE);

  it("is the AFGL template laid on Earth's column from the datum to the column's top", () => {
    expect(EARTH_OZONE.kind === "column" ? EARTH_OZONE.profile : undefined).toBe(
      EARTH_OZONE_PROFILE,
    );
    expect(EARTH_OZONE.basePa).toBe(EARTH_COLUMN.pressuresPa[0]);
    expect(EARTH_OZONE.topPa).toBe(EARTH_COLUMN.pressuresPa.at(-1));
    // x(p) n(p) at a level inside, against the template's mixing ratio there.
    const i = 300;
    const ratio = (h: number, p: number): number =>
      densityAt(ozone.density, h) /
      densityAt(EARTH_COLUMN.density, h) /
      profileMixingRatio(EARTH_OZONE_PROFILE, p);
    const reference = ratio(EARTH_COLUMN.altitudesM[i] ?? NaN, EARTH_COLUMN.pressuresPa[i] ?? NaN);
    for (const j of [10, 150, 500, 800]) {
      expect(
        ratio(EARTH_COLUMN.altitudesM[j] ?? NaN, EARTH_COLUMN.pressuresPa[j] ?? NaN) / reference,
      ).toBeCloseTo(1, 12);
    }
  });

  it("holds 300 DU to 10⁻⁶ up to the medium's top", () => {
    expect(
      Math.abs(measured.columnPerM2 / (OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2) - 1),
    ).toBeLessThan(1e-6);
  });

  it("peaks in number density between 20 and 25 km, at 4.35 × 10¹² cm⁻³", () => {
    expect(measured.peakHeightM).toBeGreaterThan(20_000);
    expect(measured.peakHeightM).toBeLessThan(25_000);
    expect(measured.peakPerM3 / 1e18).toBeCloseTo(4.35, 2);
  });

  it("has no layer edge", () => {
    expect(layerEdgesM(ozone.density)).toEqual([]);
  });

  it("takes its cross-sections at its column-weighted 217.8 K, within Serdyuchenko's 193–293 K", () => {
    expect(EARTH_OZONE.temperatureK).toBeCloseTo(217.79, 2);
  });

  it("records its mean height and its shares below 18.6 km and the tropopause, beside the slab's", () => {
    const slab = ozoneFigures(EARTH_SLAB_REFERENCE);
    expect(EARTH_TROPOPAUSE_M / 1_000).toBeCloseTo(12.585, 3);
    expect(measured.meanHeightM / 1_000).toBeCloseTo(22.39, 2);
    expect(measured.belowBase).toBeCloseTo(0.31, 3);
    expect(measured.belowTropopause).toBeCloseTo(0.117, 3);
    // The slab's: its base at 18.59 km, so its 78 m ramp and the 9 m above it lie below 18.6 km.
    expect(slab.meanHeightM / 1_000).toBeCloseTo(23.99, 2);
    expect(slab.belowBase).toBeCloseTo(0.0078, 4);
    expect(slab.belowTropopause).toBe(0);
  });

  it("records its column's heights at 100, 30, 10, 3 and 1 hPa beside the 1976 standard's", () => {
    const heightsKm = [100, 30, 10, 3, 1].map((hPa) => [
      Number((columnHeightM(hPa * 100) / 1_000).toFixed(2)),
      Number((us76HeightM(hPa * 100) / 1_000).toFixed(2)),
    ]);
    // Design note 3's skin has no ozone heating, so the column runs low above 30 km
    // (`decision-r08-t6d-findings.md` §7).
    expect(heightsKm).toEqual([
      [16.34, 16.22],
      [23.94, 23.94],
      [30.9, 31.21],
      [38.54, 39.68],
      [45.52, 48.18],
    ]);
  });
});

describe("T6.d's ozone slab, kept as a test medium of layer edges (R08.T6.h)", () => {
  const slab = termNamed(EARTH_SLAB_REFERENCE, "absorber:O3");

  it("lays 300 DU between 70 and 3 hPa, of constant mixing ratio", () => {
    expect([EARTH_OZONE_SLAB.basePa, EARTH_OZONE_SLAB.topPa]).toEqual(EARTH_OZONE_LAYER_PA);
    expect(EARTH_OZONE_SLAB.kind === "column" ? EARTH_OZONE_SLAB.profile : "none").toBeUndefined();
    const amount = absorberVerticalAmount(EARTH_OZONE_SLAB);
    expect(amount.kind === "column" ? amount.columnPerM2 : Number.NaN).toBeCloseTo(
      OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2,
      -15,
    );
  });

  it("has one layer of four edges, between 18 and 39 km", () => {
    const edges = layerEdgesM(slab.density);
    expect(edges).toHaveLength(4);
    expect(edges[0]).toBeGreaterThan(18_000);
    expect(edges[3]).toBeLessThan(39_000);
  });

  it("takes its cross-sections at the skin's 214.4 K", () => {
    expect(EARTH_OZONE_SLAB.temperatureK).toBeCloseTo(214.4, 6);
  });

  it("puts its column-weighted mean height within 1.5 km of R05's tent's 25 km", () => {
    expect(Math.abs(ozoneFigures(EARTH_SLAB_REFERENCE).meanHeightM - 25_000)).toBeLessThan(1_500);
  });

  it("is the measured Earth, its name and top and ground, term for term but its ozone", () => {
    expect(EARTH_SLAB_REFERENCE.name).toBe("earth (ozone slab)");
    expect({ ...EARTH_SLAB_REFERENCE, name: "earth", terms: [] }).toEqual({
      ...EARTH_REFERENCE,
      terms: [],
    });
    expect(EARTH_SLAB_REFERENCE.terms.slice(0, 2)).toEqual(EARTH_REFERENCE.terms.slice(0, 2));
    expect(slab.absorber?.spec).toBe(EARTH_OZONE_SLAB);
  });

  it("is built with its own curve of the slab's ozone, which its tables accept", () => {
    expect(EARTH_SLAB_REFERENCE_CURVES[0]?.[0]).not.toEqual(EARTH_REFERENCE_CURVES[0]?.[0]);
    expect(() => {
      checkAbsorberCurves(EARTH_SLAB_REFERENCE, EARTH_SLAB_REFERENCE_CURVES);
    }).not.toThrow();
  });
});

describe("the reference Sun (R06's, through R07's starIlluminance)", () => {
  it("carries the bake spectrum R06's table gives the Sun, as the client's fixture holds it", () => {
    const sun = bakeSpectra.suns.find((s) => s.teff_k === 5_772 && s.grid === "MainSequence");
    expect(sun?.bake_spectrum).toEqual([...R06_SOLAR_BAKE_SPECTRUM]);
    expect(REFERENCE_SUN.bake_spectrum).toEqual([...R06_SOLAR_BAKE_SPECTRUM]);
  });

  it("is R06's colour, of unit luminance, within 2 × 10⁻⁴ in u′v′ of its own bake spectrum's", () => {
    expect(luminanceOfRgb(R06_SOLAR_COLOUR_RGB)).toBeCloseTo(1, 4);
    expect(photopicIlluminance(R06_SOLAR_COLOUR_RGB)).toBeCloseTo(1, 12);
    expect(R06_SOLAR_COLOUR_RGB[2]).toBeCloseTo(0.934_006_3, 6);
    const bins = sunWeights(R06_SOLAR_BAKE_SPECTRUM).rgb;
    expect(deltaUv(R06_SOLAR_COLOUR_RGB, bins)).toBeLessThan(2e-4);
  });

  it("lights a body at 1 au with E-490's own photopic illuminance to 1%", () => {
    const lux = photopicIlluminance(referenceSunIlluminanceLx(AU_M));
    expect(Math.abs(lux / E490_ILLUMINANCE_LX - 1)).toBeLessThan(0.01);
  });

  it("splits its illuminance over the channels by R06's colour", () => {
    const [r, g, b] = referenceSunIlluminanceLx();
    const lux = photopicIlluminance([r, g, b]);
    expect(r / lux).toBeCloseTo(R06_SOLAR_COLOUR_RGB[0], 6);
    expect(g / lux).toBeCloseTo(R06_SOLAR_COLOUR_RGB[1], 6);
    expect(b / lux).toBeCloseTo(R06_SOLAR_COLOUR_RGB[2], 6);
  });
});
