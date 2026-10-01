import { galacticPositionFromLy } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { aStellarBrief, aSystemsInRange } from "../../test/galaxyFixtures";
import {
  INTERIM_LIMIT,
  INTERIM_QUERIES,
  interimCountLine,
  interimField,
  interimRequest,
  retryRadiusLy,
} from "./interim";

const CENTRE_LY = [0, 26_000, 0] as const;
const CENTRE = galacticPositionFromLy(CENTRE_LY);
const LY_M = 9_460_730_472_580_800;

describe("the interim queries", () => {
  it("are e to 620 ly, d to 360, c to 210 and a to 60, each at the limit of 20,000", () => {
    const requests = INTERIM_QUERIES.map((query) =>
      interimRequest("00000000000000a1", CENTRE, { seconds: 0, nanos: 0 }, query),
    );
    expect(requests.map((r) => [r.min_layer, r.radius_ly, r.limit, r.include_stellar])).toEqual([
      ["e", 620, INTERIM_LIMIT, true],
      ["d", 360, INTERIM_LIMIT, true],
      ["c", 210, INTERIM_LIMIT, true],
      ["a", 60, INTERIM_LIMIT, true],
    ]);
  });

  it("shrink by the cube root of 0.9 of the limit over the expected count from E to the floor", () => {
    const answer = aSystemsInRange({ minLayer: "c", overLimit: ["c"], limit: INTERIM_LIMIT });
    const expected = answer.census.layers
      .filter((layer) => ["e", "d", "c"].includes(layer.layer))
      .reduce((sum, layer) => sum + layer.expected, 0);
    expect(retryRadiusLy(answer, { layer: "c", radiusLy: 210 })).toBeCloseTo(
      210 * Math.cbrt((0.9 * INTERIM_LIMIT) / expected),
      9,
    );
  });

  it("are not asked again where the floor is in", () => {
    const answer = aSystemsInRange({ minLayer: "c", limit: INTERIM_LIMIT });
    expect(retryRadiusLy(answer, { layer: "c", radiusLy: 210 })).toBeNull();
  });
});

describe("the interim field", () => {
  const brief = aStellarBrief("a");
  const answer = aSystemsInRange({
    centreLy: CENTRE_LY,
    systems: [
      { relLy: [10, 0, 0], layer: "a", stellar: { ...brief, absolute_v_mag: 4.8 } },
      { relLy: [0, 20, 0], layer: "a", stellar: { ...brief, kind: "white_dwarf" } },
    ],
  });

  it("keeps one row per system across the answers", () => {
    expect(interimField([answer, answer], CENTRE, null).stars).toHaveLength(1);
  });

  it("draws a star by its direction, distance and absolute V", () => {
    const star = interimField([answer], CENTRE, null).stars[0];
    expect([star?.direction.x, Math.round((star?.distanceM ?? 0) / LY_M), star?.absoluteV]).toEqual(
      [1, 10, 4.8],
    );
  });

  it("counts and leaves out a row without an absolute V", () => {
    expect(interimField([answer], CENTRE, null).withoutV).toBe(1);
  });

  it("leaves out the scene's own system", () => {
    const own = answer.systems[0]?.id ?? null;
    expect(interimField([answer], CENTRE, own).stars).toHaveLength(0);
  });

  it("states the count and the radii used", () => {
    expect(interimCountLine(interimField([answer], CENTRE, null), [620, 360, 210, 54.3])).toBe(
      "STARS 1 DRAWN · 1 WITHOUT V · RADII 620/360/210/54 ly",
    );
  });
});
