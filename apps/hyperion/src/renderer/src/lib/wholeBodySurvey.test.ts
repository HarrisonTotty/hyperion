import { describe, expect, it } from "vitest";

import { coarseCellCount, coarseLevel, figureLevel, wholeBodySurveyPass } from "./wholeBodySurvey";

describe("a whole body's survey", () => {
  it("finds a body's coarse level as R09's coarse_level does: 5 for a Ceres to 8 for an Earth", () => {
    // Volumetric mean radii: Ceres's 469.7 km (Dawn), the Moon's 1,737.4 km, Mars's 3,389.5 km and
    // Earth's 6,371.0 km (NASA's fact sheets); a world of 2 R⊕ where the cap at 8 binds.
    expect([469_700, 1_737_400, 3_389_500, 6_371_000, 12_742_000].map(coarseLevel)).toEqual([
      5, 6, 7, 8, 8,
    ]);
    // A level changes where (R √(4π ÷ 6)) ÷ 2ᴸ crosses 40 km: here between levels 5 and 6.
    const switchM = (40_000 * 2 ** 5) / Math.sqrt((4 * Math.PI) / 6);
    expect([coarseLevel(switchM * (1 - 1e-12)), coarseLevel(switchM * (1 + 1e-12))]).toEqual([
      5, 6,
    ]);
    expect(coarseCellCount(6)).toBe(24_576);
    expect(() => coarseLevel(0)).toThrow(RangeError);
  });

  it("finds the level from the figure's volumetric radius, ∛(a² c)", () => {
    // Mars's 3,396.2 km by 3,376.2 km (NASA's fact sheet): ∛(a² c) is 3,389.5 km, level 7.
    expect(figureLevel({ equatorialRadiusM: 3_396_200, polarRadiusM: 3_376_200 })).toBe(7);
    // A spheroid whose equatorial radius alone is past the switch to level 6, its volume's short.
    const switchM = (40_000 * 2 ** 5) / Math.sqrt((4 * Math.PI) / 6);
    expect(figureLevel({ equatorialRadiusM: switchM * 1.001, polarRadiusM: switchM * 0.99 })).toBe(
      5,
    );
  });

  it("asks one pass of every cell of the field's level, at one instant", () => {
    const time = { seconds: 3_600, nanos: 0 };
    expect(
      wholeBodySurveyPass({
        universe: "00000000000000aa",
        body: "61ffd967fe000023.0100",
        level: 8,
        survey: { source: "orbital", resolutionM: 50 },
        time,
      }),
    ).toEqual({
      kind: "survey_pass",
      universe: "00000000000000aa",
      body: "61ffd967fe000023.0100",
      from: time,
      to: time,
      source: "orbital",
      resolution_m: 50,
      cells: [{ start: 0, end: 393_216 }],
    });
  });
});
