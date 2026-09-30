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
  inhibit,
  onMetering,
  setAuto,
  setManual,
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

  it("refuses ENABLE outside INHIBITED", () => {
    expect(enable(AUTO, 3)).toEqual({ kind: "refused", reason: "not_inhibited" });
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
