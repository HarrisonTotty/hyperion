import { describe, expect, it } from "vitest";

import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lookAlong } from "../camera/quaternion";
import { type QualitySetting, TERRAIN_SETTINGS } from "../quality/qualitySetting";
import { planetGeometry } from "./planet";
import { screenSpaceErrorPx, selectionErrorM, type ViewSelectionInput } from "./select";
import {
  RESELECT_MOVE_FRACTION,
  SELECTION_MARGIN,
  selectionTolerancePx,
} from "./selectionTolerance";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/** A 1080p view, 60° wide, at a setting's τ. */
function viewOf(setting: QualitySetting): ViewSelectionInput {
  return {
    camera: {
      positionM: { x: 0, y: 0, z: PLANET.figure.polarRadiusM + 1_000 },
      orientation: lookAlong({ x: 0, y: 0, z: -1 }, { x: 1, y: 0, z: 0 }),
    },
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 1920, heightPx: 1080 },
    weight: 1,
    tauPx: TERRAIN_SETTINGS[setting].tauPx,
  };
}

/**
 * The ρ a leaf of `level` draws after the camera moves `fraction` of d_min straight at its box,
 * pixels, for the worst-placed leaf selection keeps: at d_min itself, with ρ exactly τ_sel there.
 */
function drawnAfterMovePx(setting: QualitySetting, level: number, fraction: number): number {
  const view = viewOf(setting);
  const errorM = selectionErrorM(PLANET, level);
  // The distance at which the level's error subtends τ_sel: the nearest a leaf of it is kept.
  const dMinM =
    (errorM * view.viewport.widthPx) /
    (2 * Math.tan(view.fovXRad / 2) * selectionTolerancePx(view.tauPx));
  expect(screenSpaceErrorPx(errorM, dMinM, view)).toBeCloseTo(selectionTolerancePx(view.tauPx), 12);
  return screenSpaceErrorPx(errorM, dMinM - fraction * dMinM, view);
}

/** The relative rounding the f64 arithmetic of a few divisions can leave, well below a 1% change. */
const ROUNDING = 1e-12;

/**
 * The finest level but one, whose leaves are the nearest coarse ones. The ratio of ρ to τ after
 * the move is the same at every level.
 */
const LEVEL = PLANET.finestLevel - 1;

describe("the terrain pass's re-selection move", () => {
  it.each(["high", "low"] as const)(
    "keeps the worst-placed leaf within τ on the %s setting",
    (setting) => {
      const tauPx = TERRAIN_SETTINGS[setting].tauPx;
      // Exactly τ in exact arithmetic: τ_sel ÷ (1 − m ÷ (1 + m)) = τ_sel × (1 + m).
      const drawnPx = drawnAfterMovePx(setting, LEVEL, RESELECT_MOVE_FRACTION);
      expect(drawnPx).toBeLessThanOrEqual(tauPx * (1 + ROUNDING));
      expect(drawnPx).toBeGreaterThan(tauPx * (1 - ROUNDING));
    },
  );

  it.each(["high", "low"] as const)(
    "would let that leaf reach τ ÷ (1 − m²) at the former m of d_min on the %s setting",
    (setting) => {
      const tauPx = TERRAIN_SETTINGS[setting].tauPx;
      const drawnPx = drawnAfterMovePx(setting, LEVEL, SELECTION_MARGIN);
      expect(drawnPx).toBeGreaterThan(tauPx * 1.01);
      expect(drawnPx / (tauPx / (1 - SELECTION_MARGIN ** 2))).toBeCloseTo(1, 12);
    },
  );
});
