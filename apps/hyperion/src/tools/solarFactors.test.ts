import { describe, expect, it } from "vitest";

import {
  interpolate,
  MAX_LUMINOUS_EFFICACY_LM_PER_W,
  type MatchingSample,
  parseCie,
  parseE490,
  photopicIlluminanceLx,
  type SpectralSample,
  spectralToLuminanceFactors,
  XYZ_TO_REC709,
} from "./solarFactors";

/** A flat spectrum of 1 W m⁻² nm⁻¹ from 300 to 900 nm. */
const FLAT: SpectralSample[] = [
  { wavelengthNm: 300, value: 1 },
  { wavelengthNm: 900, value: 1 },
];

/** A toy observer: ȳ = 1 at 550 nm only, x̄ = z̄ = 0, rows at 1 nm. */
function spike(): MatchingSample[] {
  const rows: MatchingSample[] = [];
  for (let w = 540; w <= 560; w += 1) {
    rows.push({ wavelengthNm: w, x: 0, y: w === 550 ? 1 : 0, z: 0 });
  }
  return rows;
}

describe("parsing", () => {
  it("turns the E-490 export's µm and per-µm into nm and per-nm", () => {
    expect(parseE490("0.55,1878.5\n0.56,1870\n")).toEqual([
      { wavelengthNm: 550, value: 1.8785 },
      { wavelengthNm: 560, value: 1.87 },
    ]);
  });

  it("refuses a malformed line and wavelengths out of order", () => {
    expect(() => parseE490("0.55,abc\n")).toThrow(/line 1/);
    expect(() => parseE490("0.56,1\n0.55,1\n")).toThrow(/not increasing/);
    expect(() => parseCie("360,1,2\n")).toThrow(/expected 4/);
  });

  it("reads the CIE rows", () => {
    expect(parseCie("360,0.1,0.2,0.3\n")).toEqual([{ wavelengthNm: 360, x: 0.1, y: 0.2, z: 0.3 }]);
  });
});

describe("interpolate", () => {
  it("is linear between samples and refuses wavelengths outside the spectrum", () => {
    const s = [
      { wavelengthNm: 500, value: 1 },
      { wavelengthNm: 600, value: 3 },
    ];
    expect(interpolate(s, 550)).toBe(2);
    expect(() => interpolate(s, 499)).toThrow(/outside/);
  });
});

describe("Bruneton's factors", () => {
  it("give 683 lm W⁻¹ × ȳ's area for a flat spectrum's luminance", () => {
    expect(photopicIlluminanceLx(FLAT, spike())).toBeCloseTo(MAX_LUMINOUS_EFFICACY_LM_PER_W, 9);
  });

  it("sum, for the Sun, to the Rec. 709 colour of a flat spectrum", () => {
    const k = spectralToLuminanceFactors(FLAT, spike(), 0);
    for (const c of [0, 1, 2] as const) {
      expect(k[c]).toBeCloseTo(XYZ_TO_REC709[c][1] * MAX_LUMINOUS_EFFICACY_LM_PER_W, 9);
    }
  });

  it("weight each row by (λ ÷ λ_c)^p", () => {
    const k = spectralToLuminanceFactors(FLAT, spike(), -3);
    expect(k[0]).toBeCloseTo(
      XYZ_TO_REC709[0][1] * MAX_LUMINOUS_EFFICACY_LM_PER_W * (550 / 680) ** -3,
      9,
    );
  });
});
