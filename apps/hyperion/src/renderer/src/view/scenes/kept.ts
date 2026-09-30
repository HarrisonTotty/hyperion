import {
  type BodyIdHex,
  galacticPositionFromLy,
  type SystemIdHex,
  type UniverseTime,
} from "@hyperion/protocol";

import type { CameraPose } from "../camera/pose";
import type { ViewScene } from "../scene/model";

/**
 * A kept test scene: a scripted scene and camera path the view can play, and the automatic tests
 * run on (plan R02, R02.T11.b and c).
 */
export interface KeptScene {
  /** The scene's name as the `SCENE` selector and the label block show it. */
  readonly name: string;
  /** How long the script runs, s. */
  readonly durationS: number;
  /** The scene at `tS` seconds into the script. */
  sceneAt(tS: number): ViewScene;
  /** The scripted camera's pose at `tS` seconds into the script. */
  cameraAt(tS: number): CameraPose;
}

/** The kept scenes' system: a made-up ID, labelled as a test wherever it is shown. */
export const KEPT_SYSTEM: SystemIdHex = "0200080020000000";

/** A body of the kept system by its index. */
export function keptBody(index: number): BodyIdHex {
  return `${KEPT_SYSTEM}.${index.toString(16).padStart(4, "0")}`;
}

/** The kept system's barycentre: Design note 19's Sun-like point, [0, 26,000, 0] ly. */
export const KEPT_BARYCENTRE = galacticPositionFromLy([0, 26_000, 0]);

/** One astronomical unit, m (IAU 2012 Resolution B2). */
export const AU_M = 149_597_870_700;

/** The kept system's tidal radius, m: the Sun's, some 2.7 × 10⁵ au (R03's design note 7). */
export const KEPT_TIDAL_RADIUS_M = 2.7e5 * AU_M;

/** A script time as a universe time, from the epoch. */
export function keptTime(tS: number): UniverseTime {
  const seconds = Math.floor(tS);
  return { seconds, nanos: Math.min(999_999_999, Math.round((tS - seconds) * 1e9)) };
}
