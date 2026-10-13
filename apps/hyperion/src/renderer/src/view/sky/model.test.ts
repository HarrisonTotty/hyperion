import { describe, expect, it } from "vitest";

import type { SkyResponse } from "@hyperion/protocol";

import { vec3 } from "../../geometry/vec3";
import { galacticTranslated } from "../coords/position";
import {
  decodedSky,
  nearestFirstCensus,
  skyPayload,
  skyRequest,
  skyResponse,
} from "../../test/skyFixtures";
import {
  asCompleteAs,
  bakedBeyondM,
  keepsHeldSky,
  PARALLAX_BASELINE_M,
  PARALLAX_HOLD_PX,
  PARALLAX_THRESHOLD_PX,
  parallaxShiftPx,
  type SkyCamera,
  type SkyModel,
  skyRequestReason,
} from "./model";

const LIGHT_YEAR_M = 9_460_730_472_580_800;

/** A sky asked at `years` with stars at `distancesLy`. */
function heldSky(years: number, distancesLy: ReadonlyArray<number>): SkyModel {
  const stars = distancesLy.map((distanceLy) => ({
    direction: [1, 0, 0] as const,
    distanceLy,
    vMag: 5,
  }));
  const payload = skyPayload(stars, 2, null);
  const request = skyRequest(years);
  return {
    request,
    response: skyResponse(request, payload, stars.length, 2),
    ...decodedSky(payload, stars.length),
    stale: false,
  };
}

/**
 * A reply near the Sun whose layers C, D and E are complete to `edgesLy`, final where `null`, and
 * the reply final only where all three are (R06.T11.d).
 */
function replyAt(edgesLy: readonly [number | null, number | null, number | null]): SkyResponse {
  const request = skyRequest(0);
  return {
    ...skyResponse(request, skyPayload([], 2, null), 0, 2),
    census: nearestFirstCensus(edgesLy),
    final: edgesLy.every((edge) => edge === null),
  };
}

/** A sky held at a reply complete to `edgesLy`, with a star 20 ly away. */
function heldAt(edgesLy: readonly [number | null, number | null, number | null]): SkyModel {
  return { ...heldSky(0, [20]), response: replyAt(edgesLy) };
}

const OBSERVER = skyRequest(0).observer;

const CAMERA: SkyCamera = { position: OBSERVER, fovDeg: 60, widthPx: 1_920 };

/** The camera moved `m` metres along +x from the sky's observer. */
function movedBy(m: number): SkyCamera {
  return { ...CAMERA, position: galacticTranslated(OBSERVER, vec3(m, 0, 0)) };
}

describe("skyRequestReason", () => {
  it("asks on arrival, with no sky held", () => {
    expect(skyRequestReason(null, { request: skyRequest(0), cameras: [CAMERA] })).toBe("arrival");
  });

  it("asks on arrival in another system", () => {
    const held = heldSky(0, [100]);
    expect(
      skyRequestReason(held, { request: skyRequest(0, "0200080020000001"), cameras: [CAMERA] }),
    ).toBe("arrival");
  });

  it("asks again for a sky held at a reply that is not final", () => {
    const held = heldSky(0, [100]);
    const partial = { ...held, response: { ...held.response, final: false } };
    expect(skyRequestReason(partial, { request: skyRequest(0), cameras: [CAMERA] })).toBe(
      "partial",
    );
    expect(skyRequestReason(held, { request: skyRequest(0), cameras: [CAMERA] })).toBeNull();
  });

  it("holds the sky within its validity and asks again past it", () => {
    const held = heldSky(0, [100]);
    expect(skyRequestReason(held, { request: skyRequest(0.5), cameras: [CAMERA] })).toBeNull();
    expect(skyRequestReason(held, { request: skyRequest(1.01), cameras: [CAMERA] })).toBe(
      "expired",
    );
  });

  it("asks again for a deeper camera limit, a larger N_max or the eye, never for less", () => {
    const held = heldSky(0, [100]);
    const ask = (request: ReturnType<typeof skyRequest>) =>
      skyRequestReason(held, { request, cameras: [CAMERA] });
    expect(ask({ ...skyRequest(0), camera_limit_v: 10.5 })).toBe("limits");
    expect(ask({ ...skyRequest(0), camera_limit_v: 9.52 })).toBeNull();
    expect(ask({ ...skyRequest(0), camera_limit_v: 8 })).toBeNull();
    expect(ask({ ...skyRequest(0), n_max: 5_000 })).toBe("limits");
    expect(ask({ ...skyRequest(0), n_max: 10 })).toBeNull();
    expect(
      ask({ ...skyRequest(0), eye: { field_factor: 2, age_years: 25, pigmentation: 0.5 } }),
    ).toBe("limits");
  });

  it("asks again on a jump back before the sky's time", () => {
    const held = heldSky(10, [100]);
    expect(skyRequestReason(held, { request: skyRequest(9), cameras: [CAMERA] })).toBe("jump");
  });

  it("asks again when a camera's move shifts the nearest baked star by a tenth of a pixel", () => {
    const held = heldSky(0, [20, 400]);
    const pixelRad = Math.PI / 3 / 1_920;
    // The 20 ly star is baked (beyond the parallax sprites' 9 ly or so); a tenth of a pixel of it.
    const shiftM = 0.1 * pixelRad * 20 * LIGHT_YEAR_M;
    expect(
      skyRequestReason(held, {
        request: skyRequest(0),
        cameras: [movedBy(shiftM * 0.9)],
      }),
    ).toBeNull();
    expect(
      skyRequestReason(held, {
        request: skyRequest(0),
        cameras: [movedBy(shiftM * 1.1)],
      }),
    ).toBe("parallax");
  });

  it("leaves the parallax sprites, within 30 au's tenth of a pixel, out of the rule", () => {
    // At 1080p and 60°, 30 au moves a star a tenth of a pixel at about 9 ly.
    const beyondLy = bakedBeyondM(CAMERA) / LIGHT_YEAR_M;
    expect(beyondLy).toBeGreaterThan(8);
    expect(beyondLy).toBeLessThan(10);
    const held = heldSky(0, [1, 400]);
    expect(
      skyRequestReason(held, {
        request: skyRequest(0),
        cameras: [movedBy(PARALLAX_BASELINE_M)],
      }),
    ).toBeNull();
  });

  it("asks a reply that is not final, and stands for another time, again for that time", () => {
    const held = heldSky(10, [100]);
    const partial = { ...held, response: { ...held.response, final: false } };
    expect([
      skyRequestReason(partial, { request: skyRequest(11.5), cameras: [CAMERA] }),
      skyRequestReason(partial, { request: skyRequest(9), cameras: [CAMERA] }),
      skyRequestReason(partial, { request: skyRequest(10.5), cameras: [CAMERA] }),
    ]).toEqual(["expired", "jump", "partial"]);
  });
});

/** One pixel's angle at {@link CAMERA}, rad. */
const PIXEL_RAD = Math.PI / 3 / 1_920;

/** The camera moved so that a star 20 ly away shifts by `px` pixels. */
function shiftedBy(px: number): SkyCamera {
  return movedBy(px * PIXEL_RAD * 20 * LIGHT_YEAR_M);
}

describe("parallaxShiftPx", () => {
  it("measures the nearest baked star's shift in pixels for a camera's move", () => {
    const held = heldSky(0, [20, 400]);
    expect(parallaxShiftPx(held, shiftedBy(0.5))).toBeCloseTo(0.5, 9);
    expect(parallaxShiftPx(held, CAMERA)).toBe(0);
  });
});

describe("asCompleteAs (R06.T12.b)", () => {
  it("counts a final reply as complete as any sky", () => {
    expect(asCompleteAs(replyAt([null, null, null]), replyAt([null, null, null]))).toBe(true);
    expect(asCompleteAs(replyAt([null, null, null]), replyAt([500, 500, 500]))).toBe(true);
  });

  it("counts a reply complete at least as far in every layer as complete", () => {
    const held = replyAt([500, 500, 500]);
    expect([
      asCompleteAs(replyAt([500, 500, 500]), held),
      asCompleteAs(replyAt([1_000, 500, 2_000]), held),
      asCompleteAs(replyAt([null, 500, 500]), held),
    ]).toEqual([true, true, true]);
  });

  it("counts a reply short in any one layer as less complete", () => {
    const held = replyAt([500, 500, 500]);
    expect([
      asCompleteAs(replyAt([250, 250, 250]), held),
      asCompleteAs(replyAt([1_000, 250, 1_000]), held),
    ]).toEqual([false, false]);
  });

  it("counts a layer the held sky has final as short in a reply where it is not", () => {
    expect(asCompleteAs(replyAt([4_000, 4_000, 4_000]), replyAt([null, 500, 500]))).toBe(false);
  });

  it("counts a layer the held sky states and the reply does not as short", () => {
    const reply = replyAt([1_000, 1_000, 1_000]);
    const withoutE = { ...reply, census: reply.census.filter((layer) => layer.layer !== "e") };
    expect(asCompleteAs(withoutE, replyAt([500, 500, 500]))).toBe(false);
  });
});

describe("keepsHeldSky (R06.T12.b)", () => {
  const FINAL = heldAt([null, null, null]);
  const FIRST = replyAt([125, 125, 125]);

  it("holds to one pixel, ten times the re-ask threshold", () => {
    expect([PARALLAX_HOLD_PX, PARALLAX_HOLD_PX / PARALLAX_THRESHOLD_PX]).toEqual([1, 10]);
  });

  it.each(["partial", "limits", "parallax"] as const)(
    "keeps the held sky through a re-ask for %s until a reply is final",
    (reason) => {
      expect([
        keepsHeldSky(reason, FINAL, FIRST, [CAMERA]),
        keepsHeldSky(reason, FINAL, replyAt([1_000, 1_000, 1_000]), [CAMERA]),
        keepsHeldSky(reason, FINAL, replyAt([null, null, null]), [CAMERA]),
      ]).toEqual([true, true, false]);
    },
  );

  it("gives way to a reply as complete as the held sky in every layer, before the final", () => {
    const held = heldAt([500, 500, 500]);
    expect([
      keepsHeldSky("partial", held, replyAt([250, 250, 250]), [CAMERA]),
      keepsHeldSky("partial", held, replyAt([500, 500, 500]), [CAMERA]),
    ]).toEqual([true, false]);
  });

  it("gives way when the held sky's parallax for any open camera reaches a pixel", () => {
    expect([
      keepsHeldSky("parallax", FINAL, FIRST, [shiftedBy(0.5)]),
      keepsHeldSky("parallax", FINAL, FIRST, [shiftedBy(0.999)]),
      keepsHeldSky("parallax", FINAL, FIRST, [shiftedBy(1.001)]),
      keepsHeldSky("limits", FINAL, FIRST, [CAMERA, shiftedBy(1.001)]),
    ]).toEqual([true, true, false, false]);
  });

  it.each(["expired", "jump", "arrival"] as const)(
    "holds each reply as it comes through a re-ask for %s",
    (reason) => {
      expect(keepsHeldSky(reason, FINAL, FIRST, [CAMERA])).toBe(false);
    },
  );
});
