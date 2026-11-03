import { describe, expect, it } from "vitest";

import { dot, norm, normalise, scale, sub, vec3 } from "../../../geometry/vec3";
import { rotateToBody } from "../../../view/coords/rotation";
import { fieldCellAt } from "../../../view/terrain/coverage";
import { vertexDir } from "../../../view/terrain/cube";
import {
  approachDurationS,
  approachPart,
  cameraOnLineM,
  cellCentreDirection,
  closingTimeS,
  datumNormal,
  datumPointM,
  HALO_CRATER_CENTRE,
  HANDOVER_APPROACH,
  handoverApproachFor,
  HANDOVER_HALO_LOOK,
  HANDOVER_LOOKS,
  type HandoverLook,
  limbRatioAt,
  lookAnglesRad,
  lookGeometry,
  NAMED_HANDOVER_LOOKS,
  rayDatumHitM,
  texelMappingMismatches,
  texelOfDirection,
} from "./handoverGeometry";

/** `TERRAIN TEST`'s moon's datum (the terrain golden's figure, rounded). */
const FIGURE = { equatorialRadiusM: 1_737_400, polarRadiusM: 1_735_300 };

/** Its survey's middle and its star, along −x at 1 au. */
const INPUT = {
  figure: FIGURE,
  surveyMiddle: vec3(1, 1, 0.71),
  towardStarM: vec3(-149_597_870_700, 0, 0),
  rangeM: 1.5419e7,
};

const DEG = Math.PI / 180;

/** The look named `name`. */
function look(name: HandoverLook["name"]): HandoverLook {
  const found = HANDOVER_LOOKS.find((each) => each.name === name);
  if (found === undefined) {
    throw new Error(`no look ${name}`);
  }
  return found;
}

describe("the hand-over check's looks", () => {
  it("are 30°, 90° and the terminator's, the first two judged", () => {
    expect(
      HANDOVER_LOOKS.map((each) => [
        each.scene,
        Math.round(each.incidenceRad / DEG),
        Math.round(each.emissionRad / DEG),
        each.judged,
      ]),
    ).toEqual([
      ["HAND-OVER 30°", 15, 15, true],
      ["HAND-OVER 90°", 45, 45, true],
      ["HAND-OVER TERMINATOR", 78, 12, false],
    ]);
  });

  it("name a fourth, the halo's, at 30° about its crater, recorded, which no run takes unnamed (R10.T1.c)", () => {
    const secondSurvey = normalise(vec3(1, 1, 0.62));
    expect([
      NAMED_HANDOVER_LOOKS.map((each) => each.name),
      [HANDOVER_HALO_LOOK.scene, HANDOVER_HALO_LOOK.judged, HANDOVER_HALO_LOOK.middle],
      HANDOVER_LOOKS.map((each) => each.middle),
      // Inside the second survey's 3.1° (its cells' centres within a cosine of 0.9985).
      dot(normalise(HALO_CRATER_CENTRE), secondSurvey) > 0.9985,
    ]).toEqual([
      ["handover-30", "handover-90", "handover-terminator", "handover-halo"],
      ["HAND-OVER HALO", false, HALO_CRATER_CENTRE],
      [null, null, null],
      true,
    ]);
  });

  it("closes the halo's line past the hand-over to its limb some 1,000 km off, the crater resolved", () => {
    const halo = HANDOVER_APPROACH.halo;
    // TERRAIN TEST's high setting's hand-over range (its record of 2026-11-03), and its pixel.
    const rangeM = 15_337e3;
    const radPerPx = 1.031e-3;
    const limbM = halo.closestRatio * rangeM;
    // The camera's height over a sphere of the moon's radius at that distance to its limb.
    const radiusM = FIGURE.equatorialRadiusM;
    const heightM = Math.sqrt(radiusM * radiusM + limbM * limbM) - radiusM;
    expect([
      { ...halo, closestRatio: HANDOVER_APPROACH.full.closestRatio, ratePerS: 0.01 },
      Math.round(limbM / 1e3),
      Math.round(heightM / 1e3),
      Math.round(8_104 / (heightM * radPerPx)),
    ]).toEqual([HANDOVER_APPROACH.full, 997, 266, 30]);
  });

  it("flies a launch's looks along the halo's line for its look alone, else the run's", () => {
    expect([
      handoverApproachFor("handover-halo", false),
      handoverApproachFor("handover-halo", true),
      handoverApproachFor("handover-30", false),
      handoverApproachFor(undefined, true),
    ]).toEqual([
      HANDOVER_APPROACH.halo,
      HANDOVER_APPROACH.halo,
      HANDOVER_APPROACH.full,
      HANDOVER_APPROACH.smoke,
    ]);
  });

  it("sees the halo's crater from straight above, the star 15° from its normal, at every range", () => {
    const geometry = lookGeometry(HANDOVER_HALO_LOOK, {
      ...INPUT,
      surveyMiddle: HALO_CRATER_CENTRE,
    });
    const at = (ratio: number) => {
      const angles = lookAnglesRad(geometry, cameraOnLineM(geometry, ratio));
      return [angles.incidenceRad / DEG, angles.emissionRad / DEG];
    };
    // The line runs to the moon's centre, the crater's normal off it by the figure's flattening:
    // some 0.35°, a crater's 1.6 km at 266 km up.
    const [incidenceDeg, emissionDeg] = at(1);
    const [closeIncidenceDeg, closeEmissionDeg] = at(HANDOVER_APPROACH.halo.closestRatio);
    expect([
      incidenceDeg,
      (emissionDeg ?? Number.NaN) < 0.01,
      closeIncidenceDeg,
      (closeEmissionDeg ?? Number.NaN) < 0.5,
    ]).toEqual([expect.closeTo(15, 6), true, expect.closeTo(15, 6), true]);
  });
});

describe("the hand-over check's line", () => {
  const approach = HANDOVER_APPROACH.full;

  it("closes from 3 to 0.85 of the range at 1% a second, holds 12 s, and backs out to 1.25", () => {
    const inS = closingTimeS(approach, 0.85);
    expect([
      limbRatioAt(approach, 0),
      limbRatioAt(approach, inS),
      limbRatioAt(approach, inS + 6),
      limbRatioAt(approach, approachDurationS(approach)),
    ]).toEqual([3, expect.closeTo(0.85, 12), 0.85, expect.closeTo(1.25, 12)]);
  });

  it("takes about three minutes a look, a smoke's about 36 s", () => {
    expect([approachDurationS(approach), approachDurationS(HANDOVER_APPROACH.smoke)]).toEqual([
      expect.closeTo(176.7, 1),
      expect.closeTo(35.9, 1),
    ]);
  });

  it("closes, holds and backs out in turn", () => {
    const inS = closingTimeS(approach, 0.85);
    expect([
      approachPart(approach, 10),
      approachPart(approach, inS + 1),
      approachPart(approach, inS + 13),
    ]).toEqual(["in", "hold", "out"]);
  });

  it("puts the camera at the limb distance its ratio of the range says", () => {
    const geometry = lookGeometry(look("handover-30"), INPUT);
    const centreM = norm(cameraOnLineM(geometry, 1.1));
    const limbM = Math.sqrt(centreM ** 2 - FIGURE.equatorialRadiusM ** 2);
    expect(limbM / (1.1 * INPUT.rangeM)).toBeCloseTo(1, 12);
  });
});

describe("a look's geometry", () => {
  it.each(HANDOVER_LOOKS.map((each) => [each.name, each] as const))(
    "turns %s's survey's middle to its normal in the x–y plane, its north along +z",
    (_name, each) => {
      const geometry = lookGeometry(each, INPUT);
      const normal = rotateToBody(geometry.rotation, geometry.surveyNormal);
      const north = normalise(
        sub(vec3(0, 0, 1), scale(geometry.surveyNormal, geometry.surveyNormal.z)),
      );
      // At i from the star along −x, to the star's 10⁻⁵ rad of parallax across the moon's radius,
      // which tilts the plane out of the x–y plane by its own part along z, under 10⁻⁷ rad.
      const far = vec3(-Math.cos(each.incidenceRad), Math.sin(each.incidenceRad), 0);
      expect([
        normal.z,
        norm(sub(normal, far)),
        norm(sub(rotateToBody(geometry.rotation, north), vec3(0, 0, 1))),
      ]).toEqual([expect.closeTo(0, 7), expect.closeTo(0, 4), expect.closeTo(0, 7)]);
    },
  );

  it.each(HANDOVER_LOOKS.map((each) => [each.name, each] as const))(
    "sees %s at its own angles from a star far out of the x–y plane, as a served world's is",
    (_name, each) => {
      // A served world's star where the server's scene puts it: 65° out of the plane, as the wet
      // world's was (R10.T15.b.2's runs, whose looks stood at 66° and 72° for their 15° and 45°).
      const elevation = 65 * DEG;
      const auM = 149_597_870_700;
      const input = {
        ...INPUT,
        towardStarM: vec3(
          -auM * Math.cos(elevation) * Math.cos(0.4),
          -auM * Math.cos(elevation) * Math.sin(0.4),
          auM * Math.sin(elevation),
        ),
      };
      const geometry = lookGeometry(each, input);
      const angles = lookAnglesRad(geometry, cameraOnLineM(geometry, 1));
      expect([angles.incidenceRad / DEG, angles.emissionRad / DEG, angles.phaseRad / DEG]).toEqual([
        expect.closeTo(each.incidenceRad / DEG, 6),
        expect.closeTo(each.emissionRad / DEG, 6),
        expect.closeTo((each.incidenceRad + each.emissionRad) / DEG, 4),
      ]);
    },
  );

  it.each(HANDOVER_LOOKS.map((each) => [each.name, each] as const))(
    "sees %s's survey's middle at its incidence and emission from the range, in one plane",
    (_name, each) => {
      const geometry = lookGeometry(each, INPUT);
      const angles = lookAnglesRad(geometry, cameraOnLineM(geometry, 1));
      expect([angles.incidenceRad / DEG, angles.emissionRad / DEG, angles.phaseRad / DEG]).toEqual([
        expect.closeTo(each.incidenceRad / DEG, 6),
        expect.closeTo(each.emissionRad / DEG, 6),
        expect.closeTo((each.incidenceRad + each.emissionRad) / DEG, 4),
      ]);
    },
  );

  it("tilts the line toward the star from the far-off direction, by the parallax at the range", () => {
    const each = look("handover-90");
    const geometry = lookGeometry(each, INPUT);
    const fromSun = Math.acos(-geometry.towardCamera.x) / DEG;
    // R sin e ÷ D: 1,737 km × sin 45° ÷ 15,516 km is 4.5°.
    expect(90 - fromSun).toBeCloseTo(4.5, 0);
  });

  it("takes the datum's normal, not the radial direction, at the survey's middle", () => {
    const point = datumPointM(INPUT.surveyMiddle, FIGURE);
    const normal = datumNormal(point, FIGURE);
    const tilt = Math.acos(dot(normal, normalise(point))) / DEG;
    // The oblate datum's normal leans poleward of the radial, by e² sin 2φ ÷ 2, 0.056° at 26.7°.
    expect([normal.z > normalise(point).z, tilt]).toEqual([true, expect.closeTo(0.056, 2)]);
  });
});

describe("the cells", () => {
  it("take each cell's centre at level 6 back to its own cell", () => {
    const cells = Array.from({ length: 6 * 4 ** 6 }, (_, i) => i);
    expect(texelMappingMismatches(cells, 6)).toBe(0);
  });

  it("number a cell as the coverage mask does: face, then row j, then column i", () => {
    const level = 6;
    const n = 2 ** level;
    const cells = [0, 4_000, 4_095, 6_800, 24_575].map((index) => fieldCellAt(level, index));
    expect(
      cells.map((cell) => {
        const [x, y, z] = vertexDir(cell, 32, 32);
        return texelOfDirection(vec3(x, y, z), level);
      }),
    ).toEqual(cells.map((cell) => (cell.face * n + cell.j) * n + cell.i));
  });

  it("give no centre for a texel outside the level's faces", () => {
    expect([cellCentreDirection(-1, 6), cellCentreDirection(6 * 4 ** 6, 6)]).toEqual([null, null]);
  });
});

describe("a ray to the datum", () => {
  it("meets it where it first enters it", () => {
    const hit = rayDatumHitM(vec3(0, 0, 1e7), vec3(0, 0, -1), FIGURE);
    expect(hit?.z).toBeCloseTo(FIGURE.polarRadiusM, 3);
  });

  it("misses beside it and behind it", () => {
    expect([
      rayDatumHitM(vec3(0, 0, 1e7), vec3(1, 0, 0), FIGURE),
      rayDatumHitM(vec3(0, 0, 1e7), vec3(0, 0, 1), FIGURE),
    ]).toEqual([null, null]);
  });
});
