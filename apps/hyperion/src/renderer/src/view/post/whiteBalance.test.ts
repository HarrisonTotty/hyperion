import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { SUN_COLOUR_RGB, sunLikeHostDisc } from "../lighting/hostDisc";
import type { PlacedLight } from "../lighting/hostLights";
import { starColour } from "../photometry/starColour";
import { applyRows, type Rgb, SRGB_TO_XYZ, spriteToneCurve } from "../photometry/toneCurve";
import { srgbEncode, tonemapTexel, tonemapUniforms } from "./tonemap";
import { glareSpreadTerms } from "./glare";
import {
  brightestLight,
  discWhite,
  frameWhiteBalance,
  whiteBalanceName,
  whiteBalanceRows,
} from "./whiteBalance";

const luminance = (rgb: Rgb): number => applyRows(SRGB_TO_XYZ, rgb)[1];

/** The hosts the census meets: an L dwarf's edge to a B star's, and R06's Sun. */
const WHITES: ReadonlyArray<readonly [string, Rgb]> = [
  ["2,300 K", starColour(2_300)],
  ["3,000 K", starColour(3_000)],
  ["3,444 K", starColour(3_444)],
  ["4,500 K", starColour(4_500)],
  ["the Sun", SUN_COLOUR_RGB],
  ["10,000 K", starColour(10_000)],
  ["20,000 K", starColour(20_000)],
];

describe("whiteBalanceRows", () => {
  it.each(WHITES)("maps a %s host's own white to a neutral grey of its luminance", (_, white) => {
    const y = luminance(white);
    const balanced = applyRows(whiteBalanceRows(white), white);
    for (const channel of balanced) {
      expect(Math.abs(channel - y)).toBeLessThan(1e-6);
    }
  });

  it("leaves light of D65's own white as it is, to Filament's two matrices' 2 × 10⁻⁶", () => {
    const rows = whiteBalanceRows([1, 1, 1]);
    rows.forEach((row, i) => {
      row.forEach((cell, j) => {
        expect(Math.abs(cell - (i === j ? 1 : 0))).toBeLessThan(2e-6);
      });
    });
  });

  it("takes an M dwarf's orange cast off a grey surface it lights", () => {
    const light = starColour(3_000);
    const grey: Rgb = [0.12 * light[0], 0.12 * light[1], 0.12 * light[2]];
    expect(grey[0] / grey[2]).toBeGreaterThan(3);
    const [r, g, b] = applyRows(whiteBalanceRows(light), grey);
    expect([Math.abs(r - g), Math.abs(g - b)].every((d) => d < 1e-6)).toBe(true);
  });
});

describe("discWhite", () => {
  it("reads a disc's channels, B, V and R on the wire, as a colour of unit luminance", () => {
    const white = discWhite(sunLikeHostDisc());
    SUN_COLOUR_RGB.forEach((channel, i) => {
      expect(Math.abs((white[i] ?? Number.NaN) - channel)).toBeLessThan(1e-4);
    });
    expect(Math.abs(luminance(white) - 1)).toBeLessThan(1e-12);
  });
});

/** Star `star`'s light at `distanceM` along +x, of the colour `colourRgb`, the Sun's luminance. */
function placed(star: number, distanceM: number, colourRgb: Rgb, teffK: number): PlacedLight {
  return {
    disc: sunLikeHostDisc({ star, colourRgb, teff_k: teffK }),
    centreM: vec3(distanceM, 0, 0),
  };
}

describe("frameWhiteBalance", () => {
  const near = placed(0, 1.5e11, starColour(3_000), 3_000);
  const far = placed(1, 1.5e13, starColour(6_000), 6_000);

  it("applies nothing under D65", () => {
    expect(frameWhiteBalance("d65", [near, far])).toBeNull();
  });

  it("applies nothing under HOST with no light", () => {
    expect(frameWhiteBalance("host", [])).toBeNull();
  });

  it("adapts under HOST to the star brightest at the camera, whatever its order", () => {
    expect(brightestLight([far, near])).toBe(near);
    const balanced = frameWhiteBalance("host", [far, near]);
    expect(balanced?.teffK).toBe(3_000);
    const neutral = applyRows(balanced?.rows ?? whiteBalanceRows([1, 1, 1]), starColour(3_000));
    expect(Math.abs(neutral[0] - neutral[2])).toBeLessThan(1e-6);
  });

  it("keeps the star it adapted to while another is less than 1.25 times as bright", () => {
    // Nearer by √1.2 and by √1.3, the second star gives 1.2 and 1.3 times the first one's light.
    const kept = frameWhiteBalance("host", [near]);
    const close = placed(1, 1.5e11 / Math.sqrt(1.2), starColour(6_000), 6_000);
    const brighter = placed(1, 1.5e11 / Math.sqrt(1.3), starColour(6_000), 6_000);
    expect(frameWhiteBalance("host", [near, close], kept)).toBe(kept);
    expect(frameWhiteBalance("host", [near, brighter], kept)?.teffK).toBe(6_000);
    expect(frameWhiteBalance("host", [near, close])?.teffK).toBe(6_000);
  });

  it("names its settings as the panel and the label block show them", () => {
    expect([whiteBalanceName("d65"), whiteBalanceName("host")]).toEqual(["D65", "HOST"]);
  });
});

describe("the tone step's white balance", () => {
  const light: Rgb = [0.4, 0.2, 0.05];

  it("draws D65 as before, to the bit: no balance is no step", () => {
    const display = spriteToneCurve(light);
    const before = display.map(srgbEncode);
    expect(tonemapTexel(light, 1, null)).toEqual(before);
    expect(tonemapTexel(light, 1, null, null)).toEqual(before);
  });

  it("tells the pass to skip its step under D65 and to take the rows under HOST", () => {
    const terms = glareSpreadTerms("camera", { ageYears: 25, pigmentation: 0.5 });
    const base = {
      exposure: 1,
      threshold: 1,
      levelZeroWeight: 0,
      levelOneWeight: 1,
      sourceCount: 0,
      terms,
      dither: true,
    };
    const d65 = tonemapUniforms(base);
    expect(Array.from(d65["balance"] ?? [])).toEqual([0]);
    const rows = whiteBalanceRows(starColour(3_000));
    const host = tonemapUniforms({ ...base, balance: rows });
    expect(Array.from(host["balance"] ?? [])).toEqual([1]);
    expect(Array.from(host["balanceRow1"] ?? [])).toEqual(
      Array.from(new Float32Array([...rows[1], 0])),
    );
  });

  it("draws a host-lit grey grey under HOST, within a tenth of a code", () => {
    const sun = starColour(3_000);
    const grey: Rgb = [0.18 * sun[0], 0.18 * sun[1], 0.18 * sun[2]];
    const [r, g, b] = tonemapTexel(grey, 1, null, whiteBalanceRows(sun));
    expect(Math.max(Math.abs(r - g), Math.abs(g - b))).toBeLessThan(0.1 / 255);
  });
});
