import { add } from "../../geometry/vec3";
import { differenceM } from "../coords/position";
import { type CameraOrigins, frameOrigin } from "../coords/relative";
import type { CameraFrame, CameraPose } from "./pose";

/**
 * A change of the camera's frame: the one event on which every cached camera-relative quantity is
 * dropped at once (plan R02, R02.T8.b).
 *
 * @remarks
 * Nothing is shifted incrementally in `f32`; whatever was computed against the old frame is
 * recomputed against the new one from the `f64` positions.
 */
export interface FrameChange {
  /** The frame the camera left. */
  readonly from: CameraFrame;
  /** The frame the camera is now in. */
  readonly to: CameraFrame;
}

/** Whether two camera frames are the same frame. */
export function sameCameraFrame(a: CameraFrame, b: CameraFrame): boolean {
  let same: boolean;
  switch (a.kind) {
    case "galactic":
      same =
        b.kind === "galactic" &&
        a.origin.cell_ly.every((cell, axis) => cell === b.origin.cell_ly[axis]) &&
        a.origin.offset_m.every((offset, axis) => offset === b.origin.offset_m[axis]);
      break;
    case "system":
      same = b.kind === "system" && a.system === b.system;
      break;
    case "body":
      same = b.kind === "body" && a.body === b.body;
      break;
    case "craft":
      same = b.kind === "craft" && a.craft === b.craft;
      break;
  }
  return same;
}

/**
 * Re-expresses a camera pose in another frame, in `f64`, as one operation (plan R02, R02.T8.b).
 *
 * @remarks
 * The new position is (old origin − new origin) + the old offset, the difference taken in the
 * innermost frame the two origins share, so that the vector from the camera to any point is
 * unchanged but for the rounding of that one difference. Every camera frame is non-rotating and
 * along the galactic axes, so the orientation carries over unchanged.
 *
 * @returns The re-expressed pose, and the {@link FrameChange} that invalidates cached
 * camera-relative quantities, or `null` where `next` is the frame the pose is already in (the pose
 * is then returned as it was).
 */
export function rebase(
  pose: CameraPose,
  next: CameraFrame,
  origins: CameraOrigins,
): { readonly pose: CameraPose; readonly change: FrameChange | null } {
  if (sameCameraFrame(pose.frame, next)) {
    return { pose, change: null };
  }
  const originShiftM = differenceM(
    frameOrigin(pose.frame, origins),
    frameOrigin(next, origins),
    origins,
  );
  return {
    pose: {
      frame: next,
      positionM: add(originShiftM, pose.positionM),
      orientation: pose.orientation,
    },
    change: { from: pose.frame, to: next },
  };
}
