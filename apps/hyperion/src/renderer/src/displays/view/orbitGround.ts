import type { BodyIdHex } from "@hyperion/protocol";

import { datumGroundUnder, heldGroundUnder } from "../../view/terrain/grounded";
import type { BodyFigure } from "../../view/terrain/planet";
import type { ResidentHeightsView } from "../../view/terrain/residentHeights";
import type { TerrainSession } from "../../view/terrain/session";
import type { OrbitGround, ViewRun } from "./viewRun";

/**
 * The drawn surface of `heights` over `body` of figure `figure` as an `ORBIT` camera's floor reads
 * it (plan R15, T4): under a body-fixed point, the drawn ground's height above the datum, the
 * datum's height of the point less its height above the held ground (`heldGroundUnder`, as the
 * descent's contacts read it), which is the resident ground's height over the point's parametric
 * direction, 0 where none is resident; or the sea's surface, `seaLevelM` above the datum on a world
 * with a sea, where the ground lies under it.
 */
export function orbitGroundOver(
  body: BodyIdHex,
  figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">,
  heights: Pick<ResidentHeightsView, "groundHeightAtM"> | null,
  seaLevelM: number | null,
): OrbitGround {
  const held = heldGroundUnder(figure, heights);
  const datum = datumGroundUnder(figure);
  return {
    body,
    groundAboveDatumM: (p) => {
      const groundM = datum(p).heightM - held(p).heightM;
      return seaLevelM === null ? groundM : Math.max(groundM, seaLevelM);
    },
  };
}

/**
 * The drawn ground an `ORBIT` camera's floor stands over this frame (plan R15, T4), for the run's
 * `FrameInput`: the ground the terrain session's pass of its body holds resident
 * and the sea's surface over it ({@link orbitGroundOver}); none, the datum, outside `ORBIT`, with no
 * session, or before the body's pass is handed over.
 */
export function orbitGroundOf(
  run: ViewRun,
  terrain: Pick<TerrainSession, "pass" | "survey"> | null,
): OrbitGround | undefined {
  const frame = run.camera.pose.frame;
  if (run.camera.preset !== "orbit" || frame.kind !== "body" || terrain === null) {
    return undefined;
  }
  const pass = terrain.pass(frame.body);
  if (pass === null) {
    return undefined;
  }
  // A world with a sea has its main liquid's entry; on one with none, the header's sea level is its
  // lowest cell's, no surface (`frameTerrain.ts`' coastline).
  const header = terrain.survey(frame.body)?.header ?? null;
  const seaLevelM = header === null || header.main_liquid === null ? null : header.sea_level_m;
  return orbitGroundOver(frame.body, pass.planet.figure, pass.heights, seaLevelM);
}
