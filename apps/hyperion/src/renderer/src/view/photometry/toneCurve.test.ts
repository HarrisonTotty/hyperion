import { describe, expect, it } from "vitest";

import wgsl from "../shaders/toneCurve.wgsl?raw";
import { vec3 } from "../../geometry/vec3";
import { pixelSolidAngle } from "../camera/projection";
import { DEFAULT_MAN_EV100, exposureScale } from "./exposure";
import { illuminanceLx, pixelLuminance, PSF_QUAD_PX, psfPixelWeights } from "./magnitude";
import {
  AGX_INSET,
  AGX_MAX_EV,
  AGX_MIN_EV,
  AGX_OUTSET,
  agxSigmoid,
  HALF_FLOAT_MAX,
  HDR_COLOUR_FORMAT,
  preExpose,
  REC2020_TO_REC709,
  REC709_TO_REC2020,
  type Rgb,
  spriteToneCurve,
  TONE_CURVE_BLACK,
  toneCurve,
} from "./toneCurve";

const grey = (value: number): Rgb => [value, value, value];

/** The centre pixel of a 1920 × 1080, 60° view, sr. */
const CENTRE_PIXEL_SR = pixelSolidAngle(
  vec3(0, 0, -1),
  { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 3 },
  { widthPx: 1920, heightPx: 1080 },
);

/** A centred star's peak pixel, display-linear, at an exposure. */
function centredPeak(v: number, ev100: number): number {
  const weight = psfPixelWeights({ xPx: 0, yPx: 0 })[(PSF_QUAD_PX * PSF_QUAD_PX - 1) / 2] ?? 0;
  const exposed = pixelLuminance(illuminanceLx(v), weight, CENTRE_PIXEL_SR) * exposureScale(ev100);
  return toneCurve(grey(exposed))[1];
}

/** The literal matrices of `const <name> = mat3x3f(…);` in the WGSL, as rows. */
function wgslMatrix(name: string): number[][] {
  const match = new RegExp(`const ${name} = mat3x3f\\(([^)]*)\\);`).exec(wgsl);
  if (match === null) {
    throw new Error(`no matrix ${name} in toneCurve.wgsl`);
  }
  const columns = (match[1] ?? "")
    .split(",")
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
    .map(Number);
  return [0, 1, 2].map((row) => [0, 1, 2].map((column) => columns[column * 3 + row] ?? NaN));
}

/**
 * The WGSL sigmoid's Horner steps, highest power first: the first literal, then the constant of
 * each `p * t ± c` that follows.
 */
function wgslSigmoid(): { first: number; steps: number[] } {
  const body = /fn agxSigmoid[^{]*\{([^}]*)\}/.exec(wgsl)?.[1] ?? "";
  const first = Number(/var p = vec3f\((-?[\d.]+)\);/.exec(body)?.[1]);
  const steps = [...body.matchAll(/p \* t ([+-]) ([\d.]+);/g)].map(
    ([, sign, value]) => (sign === "-" ? -1 : 1) * Number(value),
  );
  return { first, steps };
}

describe("the AgX tone curve", () => {
  it("is monotone for grey above the toe's recovery at 3.4 × 10⁻⁴", () => {
    let previous = toneCurve(grey(3.4e-4))[1];
    const falls: number[] = [];
    for (let stops = -11.5; stops <= 8; stops += 0.01) {
      const value = toneCurve(grey(0.18 * 2 ** stops))[1];
      if (0.18 * 2 ** stops > 3.4e-4 && value < previous) {
        falls.push(stops);
      }
      previous = value;
    }
    expect(falls).toEqual([]);
  });

  it("maps black to 2.53 × 10⁻⁶ per channel", () => {
    expect(TONE_CURVE_BLACK.map((channel) => Number(channel.toPrecision(3)))).toEqual([
      2.53e-6, 2.53e-6, 2.53e-6,
    ]);
  });

  it("writes 0 for black on the sprite path", () => {
    expect(spriteToneCurve(grey(0))).toEqual([0, 0, 0]);
  });

  it("clamps the toe's dip below black to 0 on the sprite path", () => {
    expect(spriteToneCurve(grey(2.35e-4))).toEqual([0, 0, 0]);
  });

  it("saturates at 0.961 for grey", () => {
    expect(toneCurve(grey(1e6))[1]).toBeCloseTo(0.961, 3);
  });

  it("maps the log-encoded 0.18 through the sigmoid alone to 0.4971, display-linear 0.2148", () => {
    const x = (Math.log2(0.18) - AGX_MIN_EV) / (AGX_MAX_EV - AGX_MIN_EV);
    expect([x, agxSigmoid(x), agxSigmoid(x) ** 2.2].map((v) => Number(v.toFixed(4)))).toEqual([
      Number((10 / 16.5).toFixed(4)),
      0.4971,
      0.2148,
    ]);
  });

  it("maps linear grey 0.18 to 0.2148 per channel within 10⁻⁴", () => {
    const out = toneCurve(grey(0.18));
    expect(Math.max(...out.map((channel) => Math.abs(channel - 0.2148)))).toBeLessThan(1e-4);
  });

  it("has inset and outset rows summing to 1 within 10⁻⁶, so grey stays grey", () => {
    const sums = [...AGX_INSET, ...AGX_OUTSET].map((row) => row[0] + row[1] + row[2]);
    expect(Math.max(...sums.map((sum) => Math.abs(sum - 1)))).toBeLessThan(1e-6);
  });

  it("has Rec. 709 ↔ 2020 rows summing to 1 within 2 × 10⁻⁴", () => {
    const sums = [...REC709_TO_REC2020, ...REC2020_TO_REC709].map(
      (row) => row[0] + row[1] + row[2],
    );
    expect(Math.max(...sums.map((sum) => Math.abs(sum - 1)))).toBeLessThan(2e-4);
  });

  it("has the WGSL twin's matrices equal to the port's within 10⁻⁹", () => {
    const pairs = [
      ["AGX_REC709_TO_REC2020", REC709_TO_REC2020],
      ["AGX_INSET", AGX_INSET],
      ["AGX_OUTSET", AGX_OUTSET],
      ["AGX_REC2020_TO_REC709", REC2020_TO_REC709],
    ] as const;
    const gaps = pairs.flatMap(([name, rows]) =>
      wgslMatrix(name).flatMap((row, i) =>
        row.map((value, j) => Math.abs(value - (rows[i]?.[j] ?? NaN))),
      ),
    );
    expect(Math.max(...gaps)).toBeLessThan(1e-9);
  });

  it("has the WGSL twin's sigmoid, re-expanded about 0.5, equal to the port's within 10⁻¹²", () => {
    const { first, steps } = wgslSigmoid();
    const sigmoid = (x: number): number => steps.reduce((p, c) => p * (x - 0.5) + c, first);
    const gaps = Array.from({ length: 101 }, (_, i) => i / 100).map((x) =>
      Math.abs(sigmoid(x) - agxSigmoid(x)),
    );
    expect([steps.length, Math.max(...gaps) < 1e-12]).toEqual([7, true]);
  });

  it("evaluates the WGSL twin's sigmoid in f32 within 10⁻⁶ of the port", () => {
    const { first, steps } = wgslSigmoid();
    const f = Math.fround;
    const sigmoid = (x: number): number => {
      const t = f(f(x) - 0.5);
      return steps.reduce((p, c) => f(f(p * t) + f(c)), f(first));
    };
    const gaps = Array.from({ length: 1001 }, (_, i) => i / 1000).map((x) =>
      Math.abs(sigmoid(x) - agxSigmoid(Math.fround(x))),
    );
    expect(Math.max(...gaps)).toBeLessThan(1e-6);
  });
});

describe("the default MAN exposure through the curve", () => {
  it("puts Sirius's centred peak at 0.957 ± 0.002", () => {
    expect(Math.abs(centredPeak(-1.46, DEFAULT_MAN_EV100) - 0.957)).toBeLessThan(0.002);
  });

  it("puts a mag 6.5 star's centred peak at 5.2 × 10⁻³ ± 2%", () => {
    expect(Math.abs(centredPeak(6.5, DEFAULT_MAN_EV100) / 5.2e-3 - 1)).toBeLessThan(0.02);
  });
});

describe("pre-exposure", () => {
  it("clamps a Sun's disc at 2 × 10⁹ cd/m² under a dark-sky exposure to 65,504", () => {
    expect(preExpose(2e9, exposureScale(DEFAULT_MAN_EV100))).toBe(HALF_FLOAT_MAX);
  });

  it("passes a value inside the window through", () => {
    expect(preExpose(10, 0.5)).toBe(5);
  });

  it("stores into rgba16float, whose largest value is 65,504", () => {
    expect([HDR_COLOUR_FORMAT, HALF_FLOAT_MAX]).toEqual(["rgba16float", 65_504]);
  });
});
