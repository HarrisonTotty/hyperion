/**
 * A scene and the views' reports the terrain session's tests judge fields by (plan R10, R10.T16.a):
 * the fixtures' planet and its moon, with Hill radii small enough that the moon's hand-over range
 * reaches beyond its frame.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { add, type Vec3, vec3 } from "../geometry/vec3";
import { handoverRangeM } from "../view/terrain/handover";
import type { TerrainViewReport } from "../view/terrain/session";
import type { RenderStyle } from "../view/camera/state";
import type { ViewScene } from "../view/scene/model";
import {
  aBody,
  aViewScene,
  FIXTURE_MOON,
  FIXTURE_MOON_CENTRE_M,
  FIXTURE_PLANET,
  FIXTURE_SYSTEM,
  NO_TURN,
} from "./viewFixtures";

/** The scene's moon, whose field the tests want. */
export const SESSION_MOON: BodyIdHex = FIXTURE_MOON;

/** The scene's planet. */
export const SESSION_PLANET: BodyIdHex = FIXTURE_PLANET;

/**
 * The moon's and the planet's Hill radius here, m: a sixth of the Moon's 5.8 × 10⁷ m, so that the
 * moon's hand-over range at 1 px (3 × 10⁷ m for `moonLikeHeader`'s 18 km of relief) reaches
 * beyond its frame, and the planet's frame does not hold a camera by the moon.
 */
export const SESSION_HILL_M = 1e7;

/** The moon's equatorial radius, m: `moonLikeHeader`'s 1,738.1 km (NASA's Moon fact sheet). */
export const SESSION_MOON_RADIUS_M = 1_738_100;

/** The tests' scene: the planet 1 au out and its moon, each with a Hill radius of 10⁷ m. */
export const SESSION_SCENE: ViewScene = aViewScene({
  bodies: [
    aBody({ hillRadiusM: SESSION_HILL_M }),
    aBody({
      id: FIXTURE_MOON,
      parent: FIXTURE_PLANET,
      kind: "moon",
      designation: "TEST MOON",
      radiusM: SESSION_MOON_RADIUS_M,
      hillRadiusM: SESSION_HILL_M,
      centreM: FIXTURE_MOON_CENTRE_M,
    }),
  ],
});

/** The views' horizontal field of view, rad: R02's default 60°. */
export const SESSION_FOV_X_RAD = Math.PI / 3;

/** The views' presented size, device px: 1080p. */
export const SESSION_PRESENTED = { widthPx: 1920, heightPx: 1080 } as const;

/** What {@link viewAt} may vary beyond the distance and style. */
export interface ViewAtOptions {
  /** The centre the camera stands off from, m from the barycentre: the moon's by default. */
  readonly centreM?: Vec3;
  /** The scene: {@link SESSION_SCENE} by default. */
  readonly scene?: ViewScene;
}

/**
 * A view's report from `distanceM` off the moon's centre (or `options.centreM`) along +z, in the
 * system frame, drawn in `style` on the high setting at 60° across 1080p.
 */
export function viewAt(
  distanceM: number,
  style: RenderStyle = "photorealistic",
  options: ViewAtOptions = {},
): TerrainViewReport {
  return {
    scene: options.scene ?? SESSION_SCENE,
    pose: {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: add(options.centreM ?? FIXTURE_MOON_CENTRE_M, vec3(0, 0, distanceM)),
      orientation: NO_TURN,
    },
    fovXRad: SESSION_FOV_X_RAD,
    presentedPx: SESSION_PRESENTED,
    style,
    setting: "high",
    streamPriority: "primary",
    regimes: new Map(),
  };
}

/**
 * The moon's hand-over range at `tauPx` for `moonLikeHeader`'s 18 km, m, at the views' centre
 * pixel: 2.99 × 10⁷ m at 1 px.
 */
export function moonRangeM(tauPx: number): number {
  const radPerPx = (2 * Math.tan(SESSION_FOV_X_RAD / 2)) / SESSION_PRESENTED.widthPx;
  return handoverRangeM(18_000, tauPx, radPerPx);
}
