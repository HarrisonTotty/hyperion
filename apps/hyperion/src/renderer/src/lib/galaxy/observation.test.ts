import { describe, expect, it } from "vitest";

import { AU_PER_LY } from "../format";
import {
  formatCurvatureBound,
  formatCurvatureBoundArcsec,
  formatLightAgeYr,
  LIGHT_AGE_WIDTH_CH,
} from "./observation";

describe("formatLightAgeYr", () => {
  it("writes a light age to universe time's 0.01 yr, grouped from five digits", () => {
    expect(formatLightAgeYr(4_210)).toBe("4210.00");
    expect(formatLightAgeYr(4.243)).toBe("4.24");
    expect(formatLightAgeYr(26_000)).toBe("26,000.00");
  });

  it("fits the source horizon's oldest light in its slot", () => {
    expect(formatLightAgeYr(228_023.45)).toHaveLength(LIGHT_AGE_WIDTH_CH);
  });
});

describe("formatCurvatureBound", () => {
  it("reads a bound of zero, an orbit followed in full, as 0", () => {
    expect(formatCurvatureBound(0, null)).toEqual({ value: "0", unit: "ly" });
  });

  it("scales a small bound into a planetary system's units", () => {
    // About Design note 3's bound scaled to a thousand years out: 1E-5 ly is 0.632 AU.
    expect(formatCurvatureBound(1e-5, null)).toEqual({ value: "0.632", unit: "AU" });
    expect(formatCurvatureBound(1e-6, null)).toEqual({ value: "9.46", unit: "Gm" });
    expect(formatCurvatureBound(1e-7, null)).toEqual({ value: "946", unit: "Mm" });
  });

  it("writes a bound of a hundredth of a light-year or more in ly", () => {
    expect(formatCurvatureBound(0.53, null)).toEqual({ value: "0.530", unit: "ly" });
    expect(formatCurvatureBound(0.01, null)).toEqual({ value: "0.0100", unit: "ly" });
    expect(formatCurvatureBound(73.4, null)).toEqual({ value: "73.4", unit: "ly" });
  });

  it("keeps its unit within 5% of the switch to ly, either way", () => {
    expect(formatCurvatureBound(0.0102, "AU").unit).toBe("AU");
    expect(formatCurvatureBound(0.0106, "AU").unit).toBe("ly");
    expect(formatCurvatureBound(0.0097, "ly").unit).toBe("ly");
    expect(formatCurvatureBound(0.0094, "ly")).toEqual({
      value: String(Math.round(0.0094 * AU_PER_LY)),
      unit: "AU",
    });
  });
});

describe("formatCurvatureBoundArcsec", () => {
  it("writes three significant figures, E notation below a thousandth, and 0 for none", () => {
    expect(formatCurvatureBoundArcsec(13.05)).toBe("13.1");
    expect(formatCurvatureBoundArcsec(0.48)).toBe("0.480");
    expect(formatCurvatureBoundArcsec(2.06e-4)).toBe("2.06E-4");
    expect(formatCurvatureBoundArcsec(0)).toBe("0");
  });
});
