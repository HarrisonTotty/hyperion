import { describe, expect, it } from "vitest";

import { aLitBody } from "../../test/litFixtures";
import { bodyAppearance, discSurfaceOf } from "./bodyAppearance";
import { PROVISIONAL_PHOTOMETRY, type WireAppearance } from "./fromWire";

const WIRE: WireAppearance = {
  photometry: PROVISIONAL_PHOTOMETRY,
  figure: { equatorialRadiusM: 6.4e6, polarRadiusM: 6.4e6, pole: null },
  labels: ["BODY ALBEDO: NOT YET MODELLED"],
};

describe("bodyAppearance", () => {
  it("gathers the wire's figure, photometry and labels with the view's regime", () => {
    expect(bodyAppearance("0200080020000000.0300", WIRE, "point")).toEqual({
      body: "0200080020000000.0300",
      figure: WIRE.figure,
      photometry: PROVISIONAL_PHOTOMETRY,
      regime: "point",
      labels: ["BODY ALBEDO: NOT YET MODELLED"],
    });
  });

  it("gives a body without a figure no appearance, so it stays R02's mark", () => {
    expect(bodyAppearance("0200080020000000.0300", { ...WIRE, figure: null }, "disc")).toBeNull();
  });

  it("shades a disc with its photometry's uniform law until R10's class map", () => {
    expect(discSurfaceOf(aLitBody())).toEqual({ kind: "uniform", law: PROVISIONAL_PHOTOMETRY.law });
  });
});
