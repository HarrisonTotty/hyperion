import { describe, expect, it } from "vitest";

import {
  controlEv100,
  DEFAULT_EXPOSURE,
  DEFAULT_MAN_EV100,
  DEFAULT_MAN_TRIPLE,
  enable,
  ev100FromAverageLuminance,
  ev100FromTriple,
  exposureLevelReading,
  exposureScale,
  type ExposureControl,
  type ExposureTriple,
  inhibit,
  MAN_EV100_MAX,
  MAN_EV100_MIN,
  METER_CALIBRATION_K,
  onMetering,
  programTriple,
  setAuto,
  setManual,
  setManualEv100,
  VIEW_CAMERA,
} from "./exposure";

const AUTO: ExposureControl = { kind: "auto", ev100: 3 };

function accepted(result: ReturnType<typeof inhibit>): ExposureControl {
  if (result.kind !== "accepted") {
    throw new Error(`expected the command to be accepted, got ${result.reason}`);
  }
  return result.control;
}

describe("exposure values", () => {
  it("makes f/1, 1 s, ISO 100 EV100 0", () => {
    expect(ev100FromTriple({ aperture: 1, shutterS: 1, iso: 100 })).toBe(0);
  });

  it("makes f/16, 1/100 s, ISO 100 about EV100 14.6", () => {
    expect(ev100FromTriple({ aperture: 16, shutterS: 0.01, iso: 100 })).toBeCloseTo(14.64, 2);
  });

  it("makes a metered average of 12.5 cd/m² EV100 6.64", () => {
    expect(ev100FromAverageLuminance(12.5)).toBeCloseTo(6.644, 3);
  });

  it("puts the white point at 9.6 times the metered average within 1%", () => {
    const average = 3.7;
    const whitePoint = 1 / exposureScale(ev100FromAverageLuminance(average));
    expect(whitePoint / (9.6 * average)).toBeCloseTo(1, 2);
  });

  it("sets the default MAN triple at EV100 −1", () => {
    expect(ev100FromTriple(DEFAULT_MAN_TRIPLE)).toBe(DEFAULT_MAN_EV100);
  });

  it("refuses a triple with a member that is not positive", () => {
    expect(() => ev100FromTriple({ aperture: 0, shutterS: 1, iso: 100 })).toThrow(RangeError);
  });

  it("adds an ND filter's attenuation in EV", () => {
    expect(ev100FromTriple({ aperture: 1, shutterS: 1, iso: 100, ndEv: 6.5 })).toBe(6.5);
  });

  it("refuses an ND that is negative or not finite", () => {
    for (const ndEv of [-0.1, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(() => ev100FromTriple({ aperture: 1, shutterS: 1, iso: 100, ndEv })).toThrow(
        RangeError,
      );
    }
  });
});

/** The program's three joins, EV100 (decision-r07-exposure-camera, (a)). */
const JOINS = {
  /** ISO 409,600 at f/1.4 and 1/30 s: the top gain, darker than which the picture is pushed. */
  push: Math.log2(5_880 / 409_600),
  /** ISO 100 at f/1.4 and 1/30 s: the shutter takes over. */
  shutter: Math.log2(1.96 * 30),
  /** 1/8,000 s at f/1.4 and ISO 100: the ND takes over. */
  nd: Math.log2(1.96 * 8_000),
} as const;

/** Whether two numbers agree to `relative` of the larger. */
function agree(a: number, b: number, relative: number): boolean {
  return Math.abs(a - b) <= relative * Math.max(Math.abs(a), Math.abs(b));
}

describe("the view camera's program", () => {
  it("joins at EV100 −6.12, 5.88 and 13.94", () => {
    expect(JOINS.push).toBeCloseTo(-6.1223, 4);
    expect(JOINS.shutter).toBeCloseTo(5.8777, 4);
    expect(JOINS.nd).toBeCloseTo(13.9366, 4);
  });

  it("stands at the top gain, base ISO and shortest shutter at its joins, the filter clear", () => {
    const push = programTriple(VIEW_CAMERA, JOINS.push);
    expect([push.shutterS, push.ndEv]).toEqual([1 / 30, undefined]);
    expect(agree(push.iso, 409_600, 1e-12)).toBe(true);
    const shutter = programTriple(VIEW_CAMERA, JOINS.shutter);
    expect([shutter.iso, shutter.ndEv]).toEqual([100, undefined]);
    expect(agree(shutter.shutterS, 1 / 30, 1e-12)).toBe(true);
    const nd = programTriple(VIEW_CAMERA, JOINS.nd);
    expect([nd.iso, nd.ndEv]).toEqual([100, undefined]);
    expect(agree(nd.shutterS, 1 / 8_000, 1e-12)).toBe(true);
  });

  it("gives back its EV100 to 1E-12 from −14 to 42", () => {
    for (const ev100 of [-14, -10, -6.12, -1, 0, 5.88, 10, 13.94, 15, 20, 34, 42]) {
      expect(Math.abs(ev100FromTriple(programTriple(VIEW_CAMERA, ev100)) - ev100)).toBeLessThan(
        1e-12,
      );
    }
  });

  it("is continuous at its joins to 1E-12", () => {
    const step = 1e-13;
    for (const join of Object.values(JOINS)) {
      const below = programTriple(VIEW_CAMERA, join - step);
      const above = programTriple(VIEW_CAMERA, join + step);
      expect(agree(below.aperture, above.aperture, 0)).toBe(true);
      expect(agree(below.shutterS, above.shutterS, 1e-12)).toBe(true);
      expect(agree(below.iso, above.iso, 1e-12)).toBe(true);
      expect(Math.abs((below.ndEv ?? 0) - (above.ndEv ?? 0))).toBeLessThan(1e-12);
    }
  });

  it("never sets less than ISO 100 nor a shutter outside 1/8,000 to 1/30 s", () => {
    for (let ev100 = -14; ev100 <= 42; ev100 += 0.01) {
      const triple = programTriple(VIEW_CAMERA, ev100);
      expect(triple.iso).toBeGreaterThanOrEqual(100);
      expect(triple.shutterS).toBeGreaterThanOrEqual(1 / 8_000);
      expect(triple.shutterS).toBeLessThanOrEqual(1 / 30);
      expect(triple.aperture).toBe(1.4);
    }
  });

  it("is the default MAN triple at EV100 −1: f/1.4, 1/30 s, ISO 11,760, the filter clear", () => {
    expect(programTriple(VIEW_CAMERA, DEFAULT_MAN_EV100)).toEqual(DEFAULT_MAN_TRIPLE);
    expect(DEFAULT_MAN_TRIPLE.aperture).toBe(1.4);
    expect(DEFAULT_MAN_TRIPLE.shutterS).toBe(1 / 30);
    expect(DEFAULT_MAN_TRIPLE.iso).toBeCloseTo(11_760, 9);
    expect(DEFAULT_MAN_TRIPLE.ndEv).toBeUndefined();
  });

  it("takes a sunlit planet at EV100 15 at 1/8,000 s and ISO 100 behind 1.063 EV of ND", () => {
    const triple = programTriple(VIEW_CAMERA, 15);
    expect([triple.aperture, triple.shutterS, triple.iso]).toEqual([1.4, 1 / 8_000, 100]);
    expect(triple.ndEv).toBeCloseTo(1.063, 3);
  });

  it("pushes a dark sky at EV100 −10 to ISO 6.02E6 at 1/30 s", () => {
    const triple = programTriple(VIEW_CAMERA, -10);
    expect(triple.shutterS).toBe(1 / 30);
    expect(triple.iso / 6.02e6).toBeCloseTo(1, 3);
    expect(triple.ndEv).toBeUndefined();
  });

  it("sets the sensitivity a meter of K = 12.5 calls for at f/1.4 and 1/30 s", () => {
    // ISO 2720: N² ÷ t = L̄ S ÷ K, with R02's metered L̄ = 2^EV100 ÷ 8.
    for (const ev100 of [-10, 0, 5]) {
      const meteredCdM2 = 2 ** ev100 / 8;
      const iso = (METER_CALIBRATION_K * 1.4 ** 2) / ((1 / 30) * meteredCdM2);
      expect(agree(programTriple(VIEW_CAMERA, ev100).iso, iso, 1e-12)).toBe(true);
    }
  });

  it("refuses an EV100 that is not finite", () => {
    expect(() => programTriple(VIEW_CAMERA, Number.NaN)).toThrow(RangeError);
  });

  it("is dense enough at its ND for EV100 42, MAN's top: 28.06 EV, optical density 8.45", () => {
    expect(VIEW_CAMERA.maxNdEv).toBeCloseTo(28.06, 2);
    expect(VIEW_CAMERA.maxNdEv / Math.log2(10)).toBeCloseTo(8.45, 2);
    expect(programTriple(VIEW_CAMERA, 42).ndEv).toBeCloseTo(VIEW_CAMERA.maxNdEv, 12);
  });
});

describe("the exposure's automation levels", () => {
  it("starts in MAN", () => {
    expect(exposureLevelReading(DEFAULT_EXPOSURE)).toBe("MAN");
  });

  it("refuses AUTO with no image to meter", () => {
    expect(setAuto(null)).toEqual({ kind: "refused", reason: "no_image_to_meter" });
  });

  it("takes AUTO with a metering source", () => {
    expect(accepted(setAuto(3))).toEqual(AUTO);
  });

  it("goes to MAN from any level", () => {
    const manual = { aperture: 2, shutterS: 1, iso: 100 };
    expect(accepted(setManual(manual))).toEqual({ kind: "manual", triple: manual });
  });

  it("holds AUTO at its value on the operator's INHIBIT and says who inhibited it", () => {
    const held = accepted(inhibit(AUTO));
    expect([held, exposureLevelReading(held)]).toEqual([
      { kind: "inhibited", ev100: 3, reason: "operator" },
      "INHIBITED · OPERATOR",
    ]);
  });

  it("refuses INHIBIT under MAN", () => {
    expect(inhibit(DEFAULT_EXPOSURE)).toEqual({ kind: "refused", reason: "not_automatic" });
  });

  it("refuses INHIBIT under the operator's own inhibit, whose effect already holds (decision-r07-owner-ux-signoff)", () => {
    const held = accepted(inhibit(AUTO));
    expect(inhibit(held)).toEqual({ kind: "refused", reason: "already_inhibited" });
  });

  it("still takes INHIBIT under AUTO and under INHIBITED · NO IMAGE TO METER", () => {
    expect([
      accepted(inhibit(AUTO)),
      accepted(inhibit({ kind: "inhibited", ev100: 2, reason: "no_image_to_meter" })),
    ]).toEqual([
      { kind: "inhibited", ev100: 3, reason: "operator" },
      { kind: "inhibited", ev100: 2, reason: "operator" },
    ]);
  });

  it("takes over a system inhibit as the operator's on INHIBIT, which then does not resume", () => {
    const taken = accepted(inhibit(onMetering(AUTO, null)));
    expect(onMetering(taken, 5)).toEqual({ kind: "inhibited", ev100: 3, reason: "operator" });
  });

  it("clears an inhibit to AUTO on ENABLE", () => {
    const held = accepted(inhibit(AUTO));
    expect(accepted(enable(held, 4))).toEqual({ kind: "auto", ev100: 4 });
  });

  it("refuses ENABLE with no image to meter", () => {
    const held = accepted(inhibit(AUTO));
    expect(enable(held, null)).toEqual({ kind: "refused", reason: "no_image_to_meter" });
  });

  it("inhibits AUTO on the loss of its source, saying so", () => {
    const lost = onMetering(AUTO, null);
    expect([lost, exposureLevelReading(lost)]).toEqual([
      { kind: "inhibited", ev100: 3, reason: "no_image_to_meter" },
      "INHIBITED · NO IMAGE TO METER",
    ]);
  });

  it("resumes AUTO by itself when the source returns after a system inhibit", () => {
    expect(onMetering(onMetering(AUTO, null), 5)).toEqual({ kind: "auto", ev100: 5 });
  });

  it("does not resume AUTO when the source reports after an operator's inhibit", () => {
    const held = accepted(inhibit(AUTO));
    expect(onMetering(held, 5)).toBe(held);
  });

  it("refuses MAN at a triple with a member that is not positive", () => {
    expect(setManual({ aperture: 1, shutterS: 0, iso: 100 })).toEqual({
      kind: "refused",
      reason: "invalid_triple",
    });
  });

  it("refuses MAN at a triple whose ND is negative or not finite", () => {
    for (const ndEv of [-1, Number.NaN, Number.NEGATIVE_INFINITY]) {
      expect(setManual({ aperture: 1.4, shutterS: 1 / 8_000, iso: 100, ndEv })).toEqual({
        kind: "refused",
        reason: "invalid_triple",
      });
    }
  });

  it("takes MAN at a triple with an ND", () => {
    const triple = { aperture: 1.4, shutterS: 1 / 8_000, iso: 100, ndEv: 6 };
    expect(accepted(setManual(triple))).toEqual({ kind: "manual", triple });
  });

  it("refuses ENABLE under AUTO", () => {
    expect(enable(AUTO, 3)).toEqual({ kind: "refused", reason: "already_auto" });
  });

  it("hands MAN to AUTO at the metered value on ENABLE, and only with an image to meter (R07.T8.a)", () => {
    expect(accepted(enable(DEFAULT_EXPOSURE, 6))).toEqual({ kind: "auto", ev100: 6 });
    expect(enable(DEFAULT_EXPOSURE, null)).toEqual({
      kind: "refused",
      reason: "no_image_to_meter",
    });
  });

  it("keeps a system inhibit while the source stays lost", () => {
    const lost = onMetering(AUTO, null);
    expect(onMetering(lost, null)).toBe(lost);
  });

  it("reads a MAN control's EV100 from its triple and the others' at their held value", () => {
    expect([
      controlEv100(DEFAULT_EXPOSURE),
      controlEv100(AUTO),
      controlEv100({ kind: "inhibited", ev100: 2, reason: "operator" }),
    ]).toEqual([-1, 3, 2]);
  });

  it("follows the metered value under AUTO and leaves MAN alone", () => {
    expect([onMetering(AUTO, 7), onMetering(DEFAULT_EXPOSURE, 7)]).toEqual([
      { kind: "auto", ev100: 7 },
      DEFAULT_EXPOSURE,
    ]);
  });
});

/** The triple an accepted command sets `MAN` at. */
function manualTriple(result: ReturnType<typeof setManualEv100>): ExposureTriple {
  const control = accepted(result);
  if (control.kind !== "manual") {
    throw new Error(`expected MAN, got ${control.kind}`);
  }
  return control.triple;
}

describe("the operator's MAN entry (R07.T13.d)", () => {
  it("spans the scene from the galactic band's faintest background to an O star's disc", () => {
    // BS's 10⁻⁵ to 3 × 10¹¹ cd/m², metered at R02's log₂(L̄ × 8), then rounded outward.
    expect(ev100FromAverageLuminance(1e-5)).toBeCloseTo(-13.61, 2);
    expect(ev100FromAverageLuminance(3e11)).toBeCloseTo(41.13, 2);
    expect([MAN_EV100_MIN, MAN_EV100_MAX]).toEqual([-14, 42]);
  });

  it("gives back the default MAN triple at EV100 −1 to 1E-12", () => {
    const triple = manualTriple(setManualEv100(DEFAULT_MAN_EV100));
    expect(triple.aperture).toBe(DEFAULT_MAN_TRIPLE.aperture);
    expect(agree(triple.shutterS, DEFAULT_MAN_TRIPLE.shutterS, 1e-12)).toBe(true);
    expect(agree(triple.iso, DEFAULT_MAN_TRIPLE.iso, 1e-12)).toBe(true);
    expect(triple.ndEv).toBeUndefined();
  });

  it("sets the view camera's triple at EV100 9.6: f/1.4, 1.96 × 2^−9.6 s, ISO 100", () => {
    const triple = manualTriple(setManualEv100(9.6));
    expect([triple.aperture, triple.iso, triple.ndEv]).toEqual([1.4, 100, undefined]);
    expect(agree(triple.shutterS, 1.96 * 2 ** -9.6, 1e-12)).toBe(true);
  });

  it("sets the program's triple to 1E-12 across the span, its EV100 given back", () => {
    for (const ev100 of [-14, -10, JOINS.push, -1, 0, JOINS.shutter, 10, JOINS.nd, 15, 34, 42]) {
      const triple = manualTriple(setManualEv100(ev100));
      const program = programTriple(VIEW_CAMERA, ev100);
      expect(triple.aperture).toBe(program.aperture);
      expect(agree(triple.shutterS, program.shutterS, 1e-12)).toBe(true);
      expect(agree(triple.iso, program.iso, 1e-12)).toBe(true);
      expect(Math.abs((triple.ndEv ?? 0) - (program.ndEv ?? 0))).toBeLessThan(1e-12);
      expect(Math.abs(ev100FromTriple(triple) - ev100)).toBeLessThan(1e-12);
    }
  });

  it("refuses an EV100 outside −14 to 42, or not finite", () => {
    for (const ev100 of [
      -14.1,
      42.1,
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.NEGATIVE_INFINITY,
    ]) {
      expect(setManualEv100(ev100)).toEqual({ kind: "refused", reason: "invalid_ev100" });
    }
  });
});
