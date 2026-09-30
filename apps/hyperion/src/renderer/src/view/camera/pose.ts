import type { BodyIdHex, GalacticPosition, SystemIdHex } from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";

/**
 * A unit quaternion `w + xi + yj + zk`: the camera's orientation, rotating camera axes into its
 * frame's axes.
 *
 * @remarks
 * The camera looks down its own −z with +y up and +x right, the right-handed view space of
 * `perspectiveReversedInfinite` (plan R02, Design note 4).
 */
export interface Quaternion {
  /** The scalar part. */
  readonly w: number;
  /** The i component. */
  readonly x: number;
  /** The j component. */
  readonly y: number;
  /** The k component. */
  readonly z: number;
}

/** A craft's identity within the scene: the key its position is looked up by. */
export type CraftId = string;

/**
 * The frame a camera's position is held in.
 *
 * @remarks
 * `system` and `body` are the scene's non-rotating frames; `craft` holds a pose as an offset from
 * a craft, along the galactic axes, so that a seat or chase camera near its hull is exact however
 * far the craft is from its frame's origin (plan R02, Design note 22); `galactic`, with its own
 * origin, only where the scene itself is galactic.
 */
export type CameraFrame =
  | { readonly kind: "galactic"; readonly origin: GalacticPosition }
  | { readonly kind: "system"; readonly system: SystemIdHex }
  | { readonly kind: "body"; readonly body: BodyIdHex }
  | { readonly kind: "craft"; readonly craft: CraftId };

/** Where a camera is and which way it looks. */
export interface CameraPose {
  /** The frame `positionM` is measured in. */
  readonly frame: CameraFrame;
  /** The camera's position, m from the frame's origin (or the craft) along the galactic axes. */
  readonly positionM: Vec3;
  /** The camera's orientation in its frame. */
  readonly orientation: Quaternion;
}
