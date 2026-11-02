import {
  type BodyIdHex,
  galacticPositionFromLy,
  type SystemIdHex,
  type UniverseTime,
} from "@hyperion/protocol";

import { AU_M } from "../../lib/system/constants";
import type { CameraPose } from "../camera/pose";
import type { RenderStyle } from "../camera/state";
import type { MeterMode } from "../post/meter";
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
  /**
   * The free camera's start pose framed for a view `aspect` wide for each unit high, which a view
   * takes in its default pose's place at its first frame with a size: `VIEW`'s body inspection's,
   * which frames its body's disc on the view's smaller side (the owner's request, 2026-11-01).
   * Absent, as for every other kept scene, the camera starts at the scene's default pose.
   */
  readonly framedPoseFor?: (aspect: number) => CameraPose;
  /**
   * The style a view of it starts in: `photorealistic` for `VIEW`'s body inspection (the owner's
   * request, 2026-11-01), drawn so once the view may draw it; the wireframe where absent, as every
   * other kept scene starts.
   */
  readonly startStyle?: RenderStyle;
  /**
   * How a view of it opens its exposure: `metered`, at `AUTO` at its image's first metered value at
   * once, not walked to it from where the exposure stood, as `ENABLE` would set it, for `VIEW`'s
   * body inspection (the owner's request, 2026-11-01); as the display's exposure stands where
   * absent, as every other kept scene opens.
   */
  readonly startExposure?: "metered";
  /**
   * The meter a view of it opens on, until the operator chooses one in that opening: `incident`
   * for `VIEW`'s body inspection (ux-calls-view, item 3b, the owner's delegated call, 2026-11-01);
   * the display's meter as it stands where absent, as every other kept scene opens.
   */
  readonly startMeter?: MeterMode;
}

/** The kept scenes' system: a made-up ID, labelled as a test wherever it is shown. */
export const KEPT_SYSTEM: SystemIdHex = "0200080020000000";

/** A body of the kept system by its index. */
export function keptBody(index: number): BodyIdHex {
  return `${KEPT_SYSTEM}.${index.toString(16).padStart(4, "0")}`;
}

/** The kept system's barycentre: Design note 19's Sun-like point, [0, 26,000, 0] ly. */
export const KEPT_BARYCENTRE = galacticPositionFromLy([0, 26_000, 0]);

/** One astronomical unit, m: `lib/system/constants.ts`'s, which a worker can take. */
export { AU_M } from "../../lib/system/constants";

/** The kept system's tidal radius, m: the Sun's, some 2.7 × 10⁵ au (R03's design note 7). */
export const KEPT_TIDAL_RADIUS_M = 2.7e5 * AU_M;

/** A script time as a universe time, from the epoch. */
export function keptTime(tS: number): UniverseTime {
  const seconds = Math.floor(tS);
  return { seconds, nanos: Math.min(999_999_999, Math.round((tS - seconds) * 1e9)) };
}
