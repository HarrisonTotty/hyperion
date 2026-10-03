import { describe, expect, it } from "vitest";

import {
  AEROSOL_ASYMMETRY,
  AEROSOL_OPTICAL_DEPTH_550,
  EARTH_REFERENCE,
  HILLAIRE_REFERENCE,
  OZONE_ABSORPTION_PER_M,
  OZONE_COLUMN_DU,
  RAYLEIGH_SCALE_HEIGHT_M,
  RAYLEIGH_SCATTERING_PER_M,
} from "./earth";
import { CHANNEL_WAVELENGTHS_NM, densityAt, extinction, termNamed } from "./medium";

/** Boltzmann's constant, J K⁻¹ (SI 2019, exact). */
const BOLTZMANN_J_PER_K = 1.380_649e-23;

/** The US Standard Atmosphere 1976's sea level: 288.15 K and 101,325 Pa. */
const SEA_LEVEL_N_PER_M3 = 101_325 / (BOLTZMANN_J_PER_K * 288.15);

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
 * One Dobson unit, molecules m⁻²: WMO/UNEP 2018 (_Twenty Questions_, glossary), Bruneton 2017's
 * `kDobsonUnit`.
 */
const DOBSON_UNIT_PER_M2 = 2.687e20;

/**
 * Ozone's cross-sections at 680, 550 and 440 nm, m²: Serdyuchenko et al., Atmos. Meas. Tech. 7
 * (2014) 625, at 233 K, binned over 10 nm as Bruneton 2017's `kOzoneCrossSection` has them.
 */
const OZONE_CROSS_SECTION_M2 = [1.209e-25, 3.5e-25, 1.582e-26] as const;

describe("Earth's Rayleigh term", () => {
  it("recomputes the coefficients from Peck and Reeder's refractivity and Bates's King factor to 0.5%", () => {
    for (const [c, wavelength] of CHANNEL_WAVELENGTHS_NM.entries()) {
      const beta = rayleighCrossSectionM2(wavelength) * SEA_LEVEL_N_PER_M3;
      expect(Math.abs(beta / (RAYLEIGH_SCATTERING_PER_M[c] ?? 0) - 1)).toBeLessThan(0.005);
    }
  });

  it("gives Bucholtz 1995's 4.51 × 10⁻²⁷ cm² at 550 nm to 1%", () => {
    const cm2 = rayleighCrossSectionM2(550) * 1e4;
    expect(Math.abs(cm2 / 4.51e-27 - 1)).toBeLessThan(0.01);
  });

  it("is the term the reference medium carries", () => {
    expect(termNamed(EARTH_REFERENCE, "rayleigh").scattering).toEqual(RAYLEIGH_SCATTERING_PER_M);
  });
});

describe("Earth's ozone term", () => {
  it("recomputes the peak absorption from 300 DU over the 15 km tent and the cross-sections", () => {
    const peakPerM3 = (OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2) / 15_000;
    for (const [c, sigma] of OZONE_CROSS_SECTION_M2.entries()) {
      const expected = sigma * peakPerM3;
      expect(Math.abs(expected / (OZONE_ABSORPTION_PER_M[c] ?? 0) - 1)).toBeLessThan(0.005);
    }
  });

  it("absorbs only", () => {
    expect(termNamed(EARTH_REFERENCE, "ozone").scattering).toEqual([0, 0, 0]);
  });
});

describe("Earth's aerosol term", () => {
  it("integrates back to an optical depth of 0.1 at 550 nm", () => {
    const aerosol = termNamed(EARTH_REFERENCE, "aerosol");
    const groundExtinction = extinction(aerosol)[1];
    let tau = 0;
    for (let h = 0.5; h < EARTH_REFERENCE.topHeightM; h += 1) {
      tau += groundExtinction * densityAt(aerosol.density, h);
    }
    expect(tau).toBeCloseTo(AEROSOL_OPTICAL_DEPTH_550, 6);
  });

  it("follows the Ångström law with exponent 1.3 and scatters 92% of its extinction", () => {
    const aerosol = termNamed(EARTH_REFERENCE, "aerosol");
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

describe("Earth's Rayleigh column (decisions-r05.md item 3)", () => {
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

  it("is the aerosol term's phase, while Hillaire's medium keeps his 0.8", () => {
    expect(termNamed(EARTH_REFERENCE, "aerosol").phase).toEqual({
      kind: "cornette-shanks",
      asymmetry: AEROSOL_ASYMMETRY,
    });
    expect(termNamed(HILLAIRE_REFERENCE, "aerosol").phase).toEqual({
      kind: "cornette-shanks",
      asymmetry: 0.8,
    });
  });
});
