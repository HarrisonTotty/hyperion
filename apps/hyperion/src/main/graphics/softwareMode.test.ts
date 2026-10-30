import { describe, expect, it } from "vitest";

import { SOFTWARE_WAIT_FACTOR, softwareModeRefusal } from "./softwareMode";

describe("the software mode's refusal", () => {
  it("lets a views check of an unpackaged build take the switch", () => {
    expect(
      softwareModeRefusal({ software: true, packaged: false, viewsCheck: true, out: false }),
    ).toBeNull();
  });

  it("refuses the switch without --views-check", () => {
    expect(
      softwareModeRefusal({ software: true, packaged: false, viewsCheck: false, out: false }),
    ).toBe(
      "error: --hyperion-graphics-software is an option of --views-check, which was not given",
    );
  });

  it("refuses the switch in a packaged build, a views check's included", () => {
    for (const viewsCheck of [false, true]) {
      expect(softwareModeRefusal({ software: true, packaged: true, viewsCheck, out: false })).toBe(
        "error: --hyperion-graphics-software is a views check's test switch, refused in a packaged build",
      );
    }
  });

  it("refuses --out with the switch, so that a software run writes under target/ alone", () => {
    expect(
      softwareModeRefusal({ software: true, packaged: false, viewsCheck: true, out: true }),
    ).toBe(
      "error: --out is refused with --hyperion-graphics-software, whose records go under target/ alone",
    );
  });

  it("refuses nothing without the switch", () => {
    for (const packaged of [false, true]) {
      for (const viewsCheck of [false, true]) {
        expect(
          softwareModeRefusal({ software: false, packaged, viewsCheck, out: true }),
        ).toBeNull();
      }
    }
  });
});

describe("the software mode's stretch", () => {
  it("is five times, fixed (main's ruling, 2026-10-29)", () => {
    expect(SOFTWARE_WAIT_FACTOR).toBe(5);
  });
});
