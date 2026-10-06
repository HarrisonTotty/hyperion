import { describe, expect, it } from "vitest";

import { viewId } from "../camera/state";
import {
  ev100FromAverageLuminance,
  ev100FromTriple,
  exposureLevelReading as exposureLevel,
  exposureScale,
  inhibit,
  MAN_EV100_MIN,
  programTriple,
  setAuto,
  type ExposureControl,
  VIEW_CAMERA,
} from "../photometry/exposure";
import { AGX_MAX_EV } from "../photometry/toneCurve";
import { glareSourceVeil, type GlareSource } from "./glare";
import { binCentreLuminance, cpuHistogram, HISTOGRAM_BINS, type Histogram } from "./histogram";
import {
  AutoExposure,
  EMPTY_FRAME_CD_M2,
  meteredAverage,
  meteredLuminance,
  type Metering,
  meteringFor,
  meterStatus,
  METER_STEP_MAX_S,
  METER_TIMEOUT_S,
  smoothEv,
} from "./autoExposure";
import { METER_CLASS, type MeterMode } from "./meter";

const SOURCE = viewId("main");
const AUTO: ExposureControl = { kind: "auto", ev100: 0 };
const DEFAULT_MAN: ExposureControl = { kind: "manual", triple: programTriple(VIEW_CAMERA, -1) };

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

/** A frame of exact zeros as the kernel bins it, pre-exposed at the applied `ev100`. */
function zerosAt(ev100: number): Histogram {
  const preExposure = exposureScale(ev100);
  return frameHistogram(
    Array.from({ length: 16 }, (): readonly [number, number] => [0, METER_CLASS.other]),
    "average",
    preExposure,
  );
}

/** The bottom of the histogram's range at the applied `ev100`, cd/m²: 2⁻¹⁴ ÷ the pre-exposure. */
function rangeFloorAt(ev100: number): number {
  return 2 ** -14 / exposureScale(ev100);
}

/** A controller beside a drawn photorealistic image. */
function controller(control: ExposureControl = AUTO, meter: MeterMode = "average"): AutoExposure {
  const exposure = new AutoExposure({ source: SOURCE, program: VIEW_CAMERA, control, meter });
  exposure.noteImage(true);
  return exposure;
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

describe("meteredAverage", () => {
  it("floors a frame of zeros at 2⁻¹⁷ cd/m², EV100 −14 under log₂(8 L̄)", () => {
    expect(EMPTY_FRAME_CD_M2).toBe(2 ** -17);
    expect(ev100FromAverageLuminance(EMPTY_FRAME_CD_M2)).toBe(MAN_EV100_MIN);
  });

  it("meters a frame of zeros at the range's floor, but no darker than EV100 −14", () => {
    // Above an applied −3.26 the range's floor, 2⁻¹⁴ ÷ the pre-exposure, is the larger, as built.
    expect(meteredAverage(zerosAt(9.6))).toBe(rangeFloorAt(9.6));
    expect(meteredAverage(zerosAt(-3.2))).toBe(rangeFloorAt(-3.2));
    // They cross at log₂(2⁻³ ÷ 1.2) = −3.263.
    expect(rangeFloorAt(-3 - Math.log2(1.2)) / EMPTY_FRAME_CD_M2).toBeCloseTo(1, 12);
    expect(meteredAverage(zerosAt(-3.3))).toBe(EMPTY_FRAME_CD_M2);
    expect(meteredAverage(zerosAt(-20))).toBe(EMPTY_FRAME_CD_M2);
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

  it("gives the view camera's triple under AUTO and INHIBITED, the ND in at a sunlit planet", () => {
    const exposure = controller({ kind: "auto", ev100: 15 });
    expect(exposure.reading().triple).toEqual(programTriple(VIEW_CAMERA, 15));
    expect(exposure.apply(inhibit(exposure.control))).toBe(true);
    const held = exposure.reading();
    expect(held.triple).toEqual(programTriple(VIEW_CAMERA, 15));
    expect(Math.abs(ev100FromTriple(held.triple) - 15)).toBeLessThan(1e-12);
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

  it.each([9.6, -20])(
    "settles a frame of exact zeros from EV100 %s at −14 and holds it",
    (start) => {
      // Black ground filling the view: every pixel exactly 0, so in bin 0 at any exposure.
      const exposure = controller({ kind: "auto", ev100: start });
      let reading = exposure.reading();
      let darkest = reading.ev100;
      let worstHeld = 0;
      for (let frame = 0; frame < 60 * (60 + 120); frame += 1) {
        reading = exposure.step(zerosAt(reading.ev100), 1 / 60);
        darkest = Math.min(darkest, reading.ev100);
        if (frame >= 60 * 60) {
          worstHeld = Math.max(worstHeld, Math.abs(reading.ev100 - MAN_EV100_MIN));
        }
      }
      expect(reading.control.kind).toBe("auto");
      // Settled within the first 60 s, then within 0.05 EV of −14 over the next 120 s at 60 Hz.
      expect(worstHeld).toBeLessThan(0.05);
      // Never darker than the floor, from above or below.
      expect(darkest).toBeGreaterThanOrEqual(Math.min(start, MAN_EV100_MIN));
      expect(exposure.meteredEv100).toBeCloseTo(MAN_EV100_MIN, 12);
    },
  );

  it("meters a frame of zeros with one faint pixel by its light, below EV100 −14", () => {
    // One pixel at 10⁻⁸ cd/m² among 99 at 0: a mean of 10⁻¹⁰ cd/m², EV100 log₂(8 × 10⁻¹⁰) = −30.2.
    // The pixel comes into the histogram's range below about EV100 −12.8, above the floor.
    const pixelCdM2 = 1e-8;
    const meanCdM2 = pixelCdM2 / 100;
    const exposure = controller({ kind: "auto", ev100: 9.6 });
    let reading = exposure.reading();
    for (let frame = 0; frame < 60 * 120; frame += 1) {
      const preExposure = exposureScale(reading.ev100);
      const texels: Array<readonly [number, number]> = [
        [pixelCdM2 * preExposure, METER_CLASS.other],
      ];
      for (let i = 1; i < 100; i += 1) {
        texels.push([0, METER_CLASS.other]);
      }
      reading = exposure.step(frameHistogram(texels, "average", preExposure), 1 / 60);
    }
    expect(reading.control.kind).toBe("auto");
    expect(reading.ev100).toBeLessThan(MAN_EV100_MIN - 15);
    // Off the true mean by at most the bin's half-width, 0.059 stop; 0.1 leaves a margin.
    expect(Math.abs(reading.ev100 - ev100FromAverageLuminance(meanCdM2))).toBeLessThan(0.1);
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
    // `AVG` weighs everything but a star's disc: a frame of which it weighs nothing is a disc's.
    expect(exposure.control).toEqual(
      expect.objectContaining({ kind: "inhibited", reason: "nothing_weighed", meter: "average" }),
    );
  });

  it("is accepted by R02's setAuto once it meters", () => {
    const exposure = controller({ kind: "manual", triple: programTriple(VIEW_CAMERA, 3) });
    expect(exposure.apply(setAuto(exposure.meteredEv100))).toBe(false);
    exposure.step(histogram([[120, 10]]), 1 / 60);
    expect(exposure.apply(setAuto(exposure.meteredEv100))).toBe(true);
    expect(exposure.control.kind).toBe("auto");
  });
});

/** A frame of `texels` of one meter class each, as `mode`'s kernel weighs it. */
function frameOf(meterClass: number, mode: MeterMode, count = 64): Histogram {
  return frameHistogram(
    Array.from({ length: count }, (): readonly [number, number] => [
      binCentreLuminance(140),
      meterClass,
    ]),
    mode,
  );
}

/** Steps a controller for `seconds` at 60 Hz with `h`, giving the metering after each frame. */
function meteringOver(
  exposure: AutoExposure,
  h: Histogram | undefined,
  seconds: number,
): Metering[] {
  const seen: Metering[] = [];
  for (let frame = 0; frame < Math.round(seconds * 60); frame += 1) {
    exposure.step(h, 1 / 60);
    seen.push(exposure.metering);
  }
  return seen;
}

/** The kinds of a run of meterings, each run of the same kind given once, in order. */
function kindsOf(seen: ReadonlyArray<Metering>): string[] {
  const kinds: string[] = [];
  for (const metering of seen) {
    if (kinds.at(-1) !== metering.kind) {
      kinds.push(metering.kind);
    }
  }
  return kinds;
}

describe("the meter's causes (R07.T16.b)", () => {
  // A drawn image with no lit body: a star field and a body drawn black, both `other`.
  const noLitBody = frameOf(METER_CLASS.other, "lit");

  it("reads NO LIT SIDE under LIT with no lit body after 0.5 s and not before, never NO IMAGE TO METER", () => {
    const exposure = controller(AUTO, "lit");
    const before = meteringOver(exposure, noLitBody, METER_TIMEOUT_S - 0.05);
    const after = meteringOver(exposure, noLitBody, 0.2);
    expect([kindsOf(before), kindsOf(after), meterStatus(exposure.metering)]).toEqual([
      ["acquiring"],
      ["acquiring", "nothing-weighed"],
      "NO LIT SIDE",
    ]);
    expect(exposure.control).toEqual({
      kind: "inhibited",
      ev100: 0,
      reason: "nothing_weighed",
      meter: "lit",
    });
  });

  it("raises the status with the inhibit, after the timeout, and gives the window no word", () => {
    const exposure = controller(AUTO, "lit");
    const states: Array<[string, string]> = [];
    for (let frame = 0; frame < 60; frame += 1) {
      exposure.step(noLitBody, 1 / 60);
      states.push([exposure.metering.kind, exposure.control.kind]);
    }
    expect([...new Set(states.map((state) => state.join(" ")))]).toEqual([
      "acquiring auto",
      "nothing-weighed inhibited",
    ]);
  });

  it("reads NO DARK SIDE under DARK and STAR DISC ONLY under AVG", () => {
    const dark = controller(AUTO, "dark");
    meteringOver(dark, frameOf(METER_CLASS.litBody, "dark"), 1);
    const average = controller(AUTO, "average");
    // A field filled by a star's disc, which no meter weighs.
    meteringOver(average, frameOf(METER_CLASS.hostDisc, "average"), 1);
    expect([dark.metering, average.metering].map(meterStatus)).toEqual([
      "NO DARK SIDE",
      "STAR DISC ONLY",
    ]);
  });

  it("clears the old meter's status at once on a meter change, and restarts the window", () => {
    const exposure = controller(AUTO, "lit");
    meteringOver(exposure, noLitBody, 1);
    const held = exposure.control;
    exposure.setMeter("dark");
    const changed = exposure.metering;
    // DARK finds nothing either: its own status, after a whole window.
    const window = meteringOver(exposure, frameOf(METER_CLASS.other, "dark"), 0.45);
    const then = meteringOver(exposure, frameOf(METER_CLASS.other, "dark"), 0.1);
    expect([changed, kindsOf(window), kindsOf(then), meterStatus(exposure.metering)]).toEqual([
      { kind: "acquiring" },
      ["acquiring"],
      ["acquiring", "nothing-weighed"],
      "NO DARK SIDE",
    ]);
    // The inhibit's words follow the meter that found nothing until the new one has.
    expect([exposureLevel(held), exposureLevel(exposure.control)]).toEqual([
      "INHIBITED · NO LIT SIDE",
      "INHIBITED · NO DARK SIDE",
    ]);
  });

  it("holds the control as it stands while acquiring after a meter change", () => {
    const exposure = controller(AUTO, "lit");
    meteringOver(exposure, noLitBody, 1);
    const held = exposure.control;
    exposure.setMeter("average");
    exposure.step(noLitBody, 1 / 60);
    expect([exposure.metering.kind, exposure.control]).toEqual(["acquiring", held]);
  });

  it("resumes AUTO when a lit body is metered", () => {
    const exposure = controller(AUTO, "lit");
    meteringOver(exposure, noLitBody, 1);
    exposure.step(frameOf(METER_CLASS.litBody, "lit"), 1 / 60);
    expect([exposure.metering.kind, exposure.control.kind]).toEqual(["metered", "auto"]);
  });

  it("has no image to meter while none is drawn, and acquires when one comes to be drawn", () => {
    const exposure = new AutoExposure({
      source: SOURCE,
      program: VIEW_CAMERA,
      control: AUTO,
      meter: "average",
    });
    // A histogram of an image no longer drawn is not metered.
    const undrawn = meteringOver(exposure, histogram([[120, 10]]), 0.1);
    exposure.noteImage(true);
    const drawn = exposure.metering;
    exposure.step(histogram([[120, 10]]), 1 / 60);
    expect([kindsOf(undrawn), drawn, exposure.metering.kind]).toEqual([
      ["no-image"],
      { kind: "acquiring" },
      "metered",
    ]);
  });

  it("reads NO IMAGE TO METER when histograms stop beside a drawn image, after the timeout", () => {
    const exposure = controller(AUTO, "lit");
    meteringOver(exposure, noLitBody, 1);
    const stopped = meteringOver(exposure, undefined, 0.6);
    expect([kindsOf(stopped), exposureLevel(exposure.control)]).toEqual([
      ["nothing-weighed", "no-image"],
      "INHIBITED · NO IMAGE TO METER",
    ]);
  });

  it("has no image to meter at once when the image goes with nothing metered", () => {
    const exposure = controller(AUTO, "lit");
    meteringOver(exposure, noLitBody, 1);
    exposure.noteImage(false);
    const gone = exposure.metering;
    exposure.step(undefined, 1 / 60);
    expect([gone, exposureLevel(exposure.control)]).toEqual([
      { kind: "no-image" },
      "INHIBITED · NO IMAGE TO METER",
    ]);
  });

  it("acquires again when the image comes back with nothing metered", () => {
    const exposure = controller(AUTO, "lit");
    meteringOver(exposure, noLitBody, 1);
    exposure.noteImage(false);
    exposure.step(undefined, 1 / 60);
    exposure.noteImage(true);
    const back = meteringOver(exposure, noLitBody, 0.4);
    // The reading stands as it is until the window ends: the inhibit is the system's, and its
    // cause is then the meter's again.
    expect([kindsOf(back), exposureLevel(exposure.control)]).toEqual([
      ["acquiring"],
      "INHIBITED · NO IMAGE TO METER",
    ]);
    meteringOver(exposure, noLitBody, 0.2);
    expect(exposureLevel(exposure.control)).toBe("INHIBITED · NO LIT SIDE");
  });

  it("does not time the meter out over one stalled frame, but does over frames without a histogram", () => {
    const exposure = controller(AUTO, "average");
    run(exposure, histogram([[120, 10]]), 60, 1);
    // A stall of the page, longer than the timeout, in which no read-back could be delivered: it
    // counts as METER_STEP_MAX_S.
    exposure.step(undefined, METER_TIMEOUT_S + 10 * METER_STEP_MAX_S);
    const stalled = exposure.metering.kind;
    const after = meteringOver(exposure, undefined, METER_TIMEOUT_S + 0.1);
    expect([stalled, kindsOf(after)]).toEqual(["metered", ["metered", "no-image"]]);
  });

  it("leaves MAN and the operator's inhibit alone while it says why it has no value", () => {
    const manual = controller(DEFAULT_MAN, "lit");
    meteringOver(manual, noLitBody, 1);
    const operator = controller(AUTO, "lit");
    run(operator, frameOf(METER_CLASS.litBody, "lit"), 60, 1);
    expect(operator.apply(inhibit(operator.control))).toBe(true);
    const held = operator.control;
    meteringOver(operator, noLitBody, 1);
    expect([
      manual.control,
      manual.metering.kind,
      operator.control,
      operator.metering.kind,
    ]).toEqual([DEFAULT_MAN, "nothing-weighed", held, "nothing-weighed"]);
  });
});

describe("meteringFor (R07.T16.b)", () => {
  it("clears a meter's own status at once when another meter is chosen", () => {
    const lit: Metering = { kind: "nothing-weighed", meter: "lit" };
    expect([meteringFor(lit, "dark"), meteringFor(lit, "lit")]).toEqual([
      { kind: "acquiring" },
      lit,
    ]);
  });

  it("leaves a value, the want of an image and the window as they are", () => {
    const kept: Metering[] = [
      { kind: "metered", ev100: 3 },
      { kind: "no-image" },
      { kind: "acquiring" },
    ];
    expect(kept.map((metering) => meteringFor(metering, "dark"))).toEqual(kept);
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
