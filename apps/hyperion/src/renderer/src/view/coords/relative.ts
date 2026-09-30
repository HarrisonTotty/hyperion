import { sub, type Vec3 } from "../../geometry/vec3";
import type { CameraFrame, CameraPose, CraftId } from "../camera/pose";
import { narrow } from "./narrow";
import { differenceM, type FrameOrigins, type ViewPosition } from "./position";

/** {@link FrameOrigins} with the positions of the scene's craft, for cameras held about a craft. */
export interface CameraOrigins extends FrameOrigins {
  /** Where the craft is at the frame time, in its own frame. */
  craftPosition(craft: CraftId): ViewPosition;
}

/** The position of `frame`'s origin: the point a camera's `positionM` is measured from. */
function frameOrigin(frame: CameraFrame, origins: CameraOrigins): ViewPosition {
  const zero = { x: 0, y: 0, z: 0 };
  let origin: ViewPosition;
  switch (frame.kind) {
    case "galactic":
      origin = { kind: "galactic", position: frame.origin };
      break;
    case "system":
      origin = { kind: "system", system: frame.system, m: zero };
      break;
    case "body":
      origin = { kind: "body", body: frame.body, m: zero };
      break;
    case "craft":
      origin = origins.craftPosition(frame.craft);
      break;
  }
  return origin;
}

/**
 * The `f64` vector from the camera to `p`, m along the galactic axes: the one place the view
 * differences a position against the camera (plan R02, Design note 1).
 *
 * @remarks
 * Computed as (p − the camera frame's origin) − the camera's offset, each difference in the
 * innermost frame the two share (`differenceM`). For a camera in the `craft` frame this is
 * (p − craft) − offset, so the craft's own hull is exact however far the craft is from its frame's
 * origin, and only far objects carry the frame's rounding, where it is below a pixel (Design note
 * 22). The result is not required to be small; {@link narrow} it for the GPU.
 */
export function relativeToCamera(
  p: ViewPosition,
  camera: CameraPose,
  origins: CameraOrigins,
): Vec3 {
  return sub(differenceM(p, frameOrigin(camera.frame, origins), origins), camera.positionM);
}

/**
 * A mesh whose vertices lie far from its owner's centre, as an `f64` origin and `f32` offsets no
 * larger than the mesh: the one convention for anything large (plan R02, Design note 2).
 *
 * @remarks
 * The offsets are along the origin's own axes: the body-fixed axes for a `body_fixed` origin, whose
 * rotation the draw applies, the galactic axes otherwise. Graticule meshes and hulls use it here;
 * R05 and R10 use it for terrain patches.
 */
export interface OriginRelative {
  /** The mesh's origin, in `f64`. */
  readonly origin: ViewPosition;
  /** The vertices' offsets from the origin, m, three `f32` per vertex. */
  readonly offsetsF32: Float32Array;
}

/**
 * The vector from the camera to a mesh's origin, narrowed to `f32`: the one per-draw offset a
 * draw receives (R01's `DrawItem.offsetFromCameraM`).
 */
export function originMinusCamera(
  mesh: OriginRelative,
  camera: CameraPose,
  origins: CameraOrigins,
): Float32Array {
  return narrow(relativeToCamera(mesh.origin, camera, origins));
}
