import { describe, expect, it } from "vitest";

import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import type { SystemBody } from "../../lib/system/model";
import { FIXTURE_EARTH, sliceBodies } from "../../test/planetaryFixture";
import { rotation3FromRows } from "../coords/rotation";
import { appearanceFromWire, PROVISIONAL_PHOTOMETRY } from "./fromWire";
import { discIntegratedPhase, geometricAlbedo } from "./phase";
import { lambertPhase, phaseIntegral } from "./shapes";

/** The fixture's Earth as the client models today's `BodySummaryDto`. */
function fixtureEarth(): SystemBody {
  const result = toSystemBodiesModel(sliceBodies(), "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  const earth = result.bodies.bodies.find((body) => body.id === FIXTURE_EARTH);
  if (earth === undefined) {
    throw new Error("no Earth in the fixture");
  }
  return earth;
}

describe("PROVISIONAL_PHOTOMETRY", () => {
  it("is a sphere of geometric albedo 0.2", () => {
    for (const p of geometricAlbedo(PROVISIONAL_PHOTOMETRY.law)) {
      expect(p).toBeCloseTo(0.2, 12);
    }
  });

  it("has a Lambert sphere's phase integral 3 ÷ 2", () => {
    const q = phaseIntegral((alpha) => discIntegratedPhase(PROVISIONAL_PHOTOMETRY.law, alpha)[1]);
    expect(Math.abs(q / 1.5 - 1)).toBeLessThan(5e-4);
  });

  it("follows Lambert's phase function", () => {
    for (const alpha of [0.3, 1, 2, 2.8]) {
      expect(discIntegratedPhase(PROVISIONAL_PHOTOMETRY.law, alpha)[1]).toBeCloseTo(
        lambertPhase(alpha),
        4,
      );
    }
  });

  it("is labelled provisional", () => {
    expect(PROVISIONAL_PHOTOMETRY.provenance).toBe("provisional");
    expect(PROVISIONAL_PHOTOMETRY.bondRatioCheck).toBeNull();
  });
});

describe("appearanceFromWire", () => {
  it("gives a body from today's summary the provisional law and label", () => {
    const appearance = appearanceFromWire(fixtureEarth(), null);
    expect(appearance.photometry).toBe(PROVISIONAL_PHOTOMETRY);
    expect(appearance.labels).toEqual(["BODY PHOTOMETRY: NOT YET MODELLED"]);
  });

  it("makes its figure a sphere of its mean radius with a null pole", () => {
    const earth = fixtureEarth();
    const radiusM = earth.bulk.state === "ok" ? earth.bulk.value.radiusM : Number.NaN;
    expect(appearanceFromWire(earth, null).figure).toEqual({
      equatorialRadiusM: radiusM,
      polarRadiusM: radiusM,
      pole: null,
    });
  });

  it("takes the pole from the body-fixed rotation where there is one", () => {
    const tilted = rotation3FromRows([
      { x: 1, y: 0, z: 0 },
      { x: 0, y: 0, z: -1 },
      { x: 0, y: 1, z: 0 },
    ]);
    expect(appearanceFromWire(fixtureEarth(), tilted).figure?.pole).toEqual({ x: 0, y: -1, z: 0 });
  });

  it("gives a body without a granted radius no figure", () => {
    const earth: SystemBody = { ...fixtureEarth(), bulk: { state: "not_resolved" } };
    expect(appearanceFromWire(earth, null).figure).toBeNull();
  });
});
