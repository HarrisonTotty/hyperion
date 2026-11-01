import { describe, expect, it } from "vitest";

import { SOFTWARE_WAIT_FACTOR, softwareModeRefusal, type SoftwareModeRun } from "./softwareMode";

/** The runs that may take the switch. */
const RUNS: ReadonlyArray<SoftwareModeRun> = ["views check", "descent spike"];

describe("the software mode's refusal", () => {
  it("lets a views check or a descent spike of an unpackaged build take the switch", () => {
    for (const run of RUNS) {
      expect(softwareModeRefusal({ software: true, packaged: false, run, out: false })).toBeNull();
    }
  });

  it("refuses the switch without --views-check or --descent-spike", () => {
    expect(softwareModeRefusal({ software: true, packaged: false, run: null, out: false })).toBe(
      "error: --hyperion-graphics-software is an option of --views-check and --descent-spike, neither of which was given",
    );
  });

  it("refuses the switch in a packaged build, a test run's included", () => {
    for (const run of [null, ...RUNS]) {
      expect(softwareModeRefusal({ software: true, packaged: true, run, out: false })).toBe(
        "error: --hyperion-graphics-software is a test switch of the views check and the descent spike, refused in a packaged build",
      );
    }
  });

  it("refuses --out with the switch, so that a software run writes under target/ alone", () => {
    for (const run of RUNS) {
      expect(softwareModeRefusal({ software: true, packaged: false, run, out: true })).toBe(
        "error: --out is refused with --hyperion-graphics-software, whose records go under target/ alone",
      );
    }
  });

  it("refuses nothing without the switch", () => {
    for (const packaged of [false, true]) {
      for (const run of [null, ...RUNS]) {
        expect(softwareModeRefusal({ software: false, packaged, run, out: true })).toBeNull();
      }
    }
  });
});

describe("the software mode's stretch", () => {
  it("is five times, fixed (main's ruling, 2026-10-29)", () => {
    expect(SOFTWARE_WAIT_FACTOR).toBe(5);
  });
});
