import { describe, expect, it } from "vitest";

import { viewId } from "../camera/state";
import {
  ev100FromAverageLuminance,
  ev100FromTriple,
  exposureScale,
  inhibit,
  setAuto,
  type ExposureControl,
} from "../photometry/exposure";
import { AGX_MAX_EV } from "../photometry/toneCurve";
import { glareSourceVeil, type GlareSource } from "./glare";
import { binCentreLuminance, cpuHistogram, HISTOGRAM_BINS, type Histogram } from "./histogram";
import {
  AutoExposure,
  meteredLuminance,
  METER_TIMEOUT_S,
  programTriple,
  smoothEv,
  type ExposureProgram,
} from "./autoExposure";
import { METER_CLASS, type MeterMode } from "./meter";

/** R06's `DEFAULT_VIEW_CAMERA`: N = 1.4, t = 1/30 s (R06 Design note 18). */
const PROGRAM: ExposureProgram = { aperture: 1.4, shutterS: 1 / 30 };
const SOURCE = viewId("main");
const AUTO: ExposureControl = { kind: "auto", ev100: 0 };

/** A histogram of `counts` at bins, pre-exposed by `preExposure`. */
function histogram(counts: ReadonlyArray<readonly [number, number]>, preExposure = 1): Histogram {
  const bins = new Uint32Array(HISTOGRAM_BINS);
  for (const [bin, n] of counts) {
    bins[bin] = (bins[bin] ?? 0) + n;
  }
  return { bins, preExposure };
}

/** A histogram of texels as the GPU kernel would bin them. */
function frameHistogram(
  texels: ReadonlyArray<readonly [number, number]>,
  mode: MeterMode,
  preExposure = 1,
): Histogram {
  const rgba = new Float32Array(4 * texels.length);
  texels.forEach(([luminance, meterClass], i) => {
    rgba.set([luminance, luminance, luminance, meterClass], 4 * i);
  });
  const size = { widthPx: texels.length, heightPx: 1 };
  return { bins: cpuHistogram(rgba, size, mode, 1), preExposure };
}

/** Steps a controller at `hz` for `seconds` with the same histogram every frame. */
function run(exposure: AutoExposure, h: Histogram, hz: number, seconds: number): number {
  let ev = exposure.reading().ev100;
  for (let frame = 0; frame < Math.round(seconds * hz); frame += 1) {
    ev = exposure.step(h, 1 / hz).ev100;
  }
  return ev;
}

function controller(control: ExposureControl = AUTO, meter: MeterMode = "average"): AutoExposure {
  return new AutoExposure({ source: SOURCE, program: PROGRAM, control, meter });
}

describe("meteredLuminance", () => {
  it("meters a uniform 100 cd/m² field at EV100 log₂(800) = 9.644", () => {
    // Lagarde and de Rousiers 2014, eq. 69: EV100 = log₂(L̄ S ÷ K), S = 100, K = 12.5.
    const bin = 160;
    const preExposure = binCentreLuminance(bin) / 100;
    const h = histogram([[bin, 1000]], preExposure);
    expect(meteredLuminance(h)).toBeCloseTo(100, 9);
    expect(ev100FromAverageLuminance(meteredLuminance(h))).toBeCloseTo(9.644, 3);
  });

  it("is the arithmetic mean, bin 0 counting as black", () => {
    const h = histogram([
      [0, 3],
      [200, 1],
    ]);
    expect(meteredLuminance(h)).toBeCloseTo(binCentreLuminance(200) / 4, 9);
  });

  it("drops the counts outside a percentile window", () => {
    const h = histogram([
      [10, 50],
      [100, 49],
      [250, 1],
    ]);
    const clipped = meteredLuminance(h, { low: 0, high: 0.99 });
    expect(clipped).toBeCloseTo(
      (50 * binCentreLuminance(10) + 49 * binCentreLuminance(100)) / 99,
      6,
    );
  });

  it("is zero for an empty histogram", () => {
    expect(meteredLuminance(histogram([]))).toBe(0);
  });
});

describe("the exposure", () => {
  it("displays log₂(mean × 8) for a two-level histogram once settled", () => {
    const h = histogram([
      [80, 700],
      [180, 300],
    ]);
    const mean = (700 * binCentreLuminance(80) + 300 * binCentreLuminance(180)) / 1000;
    const exposure = controller();
    expect(run(exposure, h, 60, 30)).toBeCloseTo(Math.log2(mean * 8), 6);
  });

  it("puts the white point of a linear clip at 9.6 × L̄", () => {
    // Lagarde and de Rousiers 2014, eq. 75 and Listing 28: L_max = 1.2 × 2^EV100 = 9.6 L̄.
    const average = 37;
    expect(1 / exposureScale(ev100FromAverageLuminance(average))).toBeCloseTo(9.6 * average, 9);
  });

  it("exposes a 5% disc 25 stops above a black frame 4–5 stops above the average, unclipped", () => {
    const disc = 1000;
    const texels: Array<readonly [number, number]> = [];
    for (let i = 0; i < 100; i += 1) {
      texels.push(i < 5 ? [disc, METER_CLASS.litBody] : [disc * 2 ** -25, METER_CLASS.other]);
    }
    const average = meteredLuminance(frameHistogram(texels, "average"));
    const above = Math.log2(disc / average);
    expect(above).toBeGreaterThan(4);
    expect(above).toBeLessThan(5);
    const exposed = disc * exposureScale(ev100FromAverageLuminance(average));
    expect(exposed).toBeLessThan(2 ** AGX_MAX_EV);
  });

  it("moves under 1% when a Sun-like star and its glare enter a frame with a sunlit planet", () => {
    const widthPx = 480;
    const heightPx = 270;
    const planet = 5000;
    const sky = 1e-3;
    // 60° across 480 px: the Sun at 1 au is 2.1 px in radius, the planet 15% of the frame.
    const radPerPx = (60 * Math.PI) / 180 / widthPx;
    const sunPx = (0.267 * Math.PI) / 180 / radPerPx;
    const before: Array<readonly [number, number]> = [];
    const after: Array<readonly [number, number]> = [];
    const centre = { x: 400, y: 60 };
    let veil = 0;
    const source: GlareSource = {
      direction: { x: 0, y: 0, z: -1 },
      angularRadiusRad: sunPx * radPerPx,
      excessLuminance: [2e9, 2e9, 2e9],
    };
    for (let y = 0; y < heightPx; y += 1) {
      for (let x = 0; x < widthPx; x += 1) {
        const onPlanet = Math.hypot(x - 160, y - 150) < 80;
        const fromStarPx = Math.hypot(x - centre.x, y - centre.y);
        const texel: readonly [number, number] = onPlanet
          ? [planet, METER_CLASS.litBody]
          : [sky, METER_CLASS.other];
        before.push(texel);
        after.push(fromStarPx <= sunPx ? [65_504, METER_CLASS.hostDisc] : texel);
        // The veil the tone-mapping pass draws after the histogram is taken.
        veil += glareSourceVeil(source, Math.max(fromStarPx, sunPx) * radPerPx, "eye", {
          ageYears: 25,
          pigmentation: 0.5,
        })[1];
      }
    }
    const metered = (texels: ReadonlyArray<readonly [number, number]>): number =>
      meteredLuminance(frameHistogram(texels, "average"));
    const change = Math.abs(metered(after) / metered(before) - 1);
    expect(change).toBeLessThan(0.01);
    // The veil, were it metered, would have moved the mean more than 4 stops (Design note 12).
    expect(Math.log2(veil / (widthPx * heightPx) / metered(before))).toBeGreaterThan(4);
  });

  it("reaches the same EV at 30 Hz and 60 Hz", () => {
    const dark = histogram([[60, 100]]);
    const bright = histogram([[220, 100]]);
    for (const [from, to] of [
      [dark, bright],
      [bright, dark],
    ] as const) {
      const at30 = controller();
      const at60 = controller();
      run(at30, from, 30, 30);
      run(at60, from, 60, 30);
      for (const seconds of [0.2, 0.5, 1, 2, 4]) {
        const ev30 = run(at30, to, 30, seconds);
        const ev60 = run(at60, to, 60, seconds);
        expect(Math.abs(ev30 - ev60)).toBeLessThan(0.05);
      }
    }
  });

  it("meters a half-lit body's lit side under LIT and its night side under DARK", () => {
    const lit = binCentreLuminance(200);
    const night = binCentreLuminance(40);
    const texels: Array<readonly [number, number]> = [];
    for (let i = 0; i < 200; i += 1) {
      texels.push(
        i < 50
          ? [lit, METER_CLASS.litBody]
          : i < 100
            ? [night, METER_CLASS.unlitBody]
            : [0, METER_CLASS.other],
      );
    }
    expect(meteredLuminance(frameHistogram(texels, "lit"))).toBeCloseTo(lit, 6);
    expect(meteredLuminance(frameHistogram(texels, "dark"))).toBeCloseTo(night, 6);
    expect(meteredLuminance(frameHistogram(texels, "average"))).toBeCloseTo((lit + night) / 4, 6);
  });

  it("holds under INHIBITED", () => {
    const exposure = controller();
    run(exposure, histogram([[100, 10]]), 60, 10);
    const held = exposure.reading().ev100;
    expect(exposure.apply(inhibit(exposure.control))).toBe(true);
    expect(run(exposure, histogram([[230, 10]]), 60, 10)).toBe(held);
    expect(exposure.control.kind).toBe("inhibited");
  });

  it("gives under AUTO a triple whose EV100 equals the reading's", () => {
    const exposure = controller();
    run(exposure, histogram([[137, 10]]), 60, 3);
    const reading = exposure.reading();
    expect(reading.control.kind).toBe("auto");
    expect(Math.abs(ev100FromTriple(reading.triple) - reading.ev100)).toBeLessThan(1e-9);
    expect(reading.triple.aperture).toBe(1.4);
    expect(reading.triple.shutterS).toBe(1 / 30);
    expect(reading.source).toBe(SOURCE);
  });

  it("reports no image to meter when histograms stop, and resumes when they return", () => {
    const exposure = controller();
    const h = histogram([[120, 10]]);
    run(exposure, h, 60, 5);
    for (let t = 0; t <= METER_TIMEOUT_S + 0.1; t += 1 / 60) {
      exposure.step(undefined, 1 / 60);
    }
    expect(exposure.control).toEqual(
      expect.objectContaining({ kind: "inhibited", reason: "no_image_to_meter" }),
    );
    expect(exposure.meteredEv100).toBeNull();
    exposure.step(h, 1 / 60);
    expect(exposure.control.kind).toBe("auto");
  });

  it("recovers from a frame entirely below the histogram's range", () => {
    // A cut from a sunlit planet to a dark sky: every pixel pre-exposes below 2⁻¹⁴.
    const exposure = controller({ kind: "auto", ev100: 15 });
    const skyCdM2 = 1e-3;
    let reading = exposure.reading();
    for (let frame = 0; frame < 60 * 40; frame += 1) {
      const preExposure = 1 / (1.2 * 2 ** reading.ev100);
      const h = frameHistogram(
        [[skyCdM2 * preExposure, METER_CLASS.other]],
        "average",
        preExposure,
      );
      reading = exposure.step(h, 1 / 60);
    }
    expect(reading.control.kind).toBe("auto");
    expect(reading.ev100).toBeCloseTo(ev100FromAverageLuminance(skyCdM2), 1);
  });

  it("holds AUTO through a histogram with nothing to meter, until the timeout", () => {
    const exposure = controller();
    run(exposure, histogram([[120, 10]]), 60, 5);
    const empty = histogram([]);
    exposure.step(empty, 1 / 60);
    expect(exposure.control.kind).toBe("auto");
    for (let t = 0; t <= METER_TIMEOUT_S; t += 1 / 60) {
      exposure.step(empty, 1 / 60);
    }
    expect(exposure.control).toEqual(
      expect.objectContaining({ kind: "inhibited", reason: "no_image_to_meter" }),
    );
  });

  it("is accepted by R02's setAuto once it meters", () => {
    const exposure = controller({ kind: "manual", triple: programTriple(PROGRAM, 3) });
    expect(exposure.apply(setAuto(exposure.meteredEv100))).toBe(false);
    exposure.step(histogram([[120, 10]]), 1 / 60);
    expect(exposure.apply(setAuto(exposure.meteredEv100))).toBe(true);
    expect(exposure.control.kind).toBe("auto");
  });
});

describe("smoothEv", () => {
  it("brightens linearly at 3 EV/s and darkens at 1 EV/s outside the band", () => {
    expect(smoothEv(0, 10, 0.5)).toBeCloseTo(1.5, 12);
    expect(smoothEv(10, 0, 0.5)).toBeCloseTo(9.5, 12);
  });

  it("approaches exponentially inside the band and never overshoots", () => {
    const one = smoothEv(0, 1, 0.1);
    expect(one).toBeGreaterThan(0);
    expect(one).toBeLessThan(1);
    expect(smoothEv(0, 1, 100)).toBeCloseTo(1, 9);
  });
});
