/**
 * What a `VIEW` asks of the sky and what it keeps of it (plan R06, T13.c, taking T12's view wiring;
 * Design notes 5, 18 and 20; R07.T13.e).
 *
 * @remarks
 * An eye view asks the eye's limits (the server sets the eye's cut and each band texel's limit)
 * and keeps a star by its texel's limit and its colour offset. A camera view asks and keeps the
 * view camera's deepest limit, at every exposure (decision-r07-exposure-camera), so that no step of
 * `AUTO` and no `MAN` entry asks for the sky again or culls it again; its label states the limit
 * at the exposure shown. Wherever the two differ by 0.05 mag or more (from EV100 3.5), a
 * Sun-coloured star at the label's limit, centred in a pixel at the frame's centre, reaches the
 * tone curve 3.4–5.0 EV below AgX's floor at 1080p and 1.4–3.0 EV below it at 4K (60°). Off the
 * axis, and for the bluest stars, a 4K star can stand up to about 0.3 EV above the floor, in AgX's
 * toe, where it moves the output by under 0.01 of an 8-bit code; so the deeper cull changes nothing
 * visible.
 * Until the band layer (T13.d) gives a texel's background to the camera's limit, the camera reads
 * a dark sky of μ 24 mag arcsec⁻², Leinert et al. 1998's integrated starlight at the galactic pole,
 * which asks the deepest limit.
 */

import {
  type GalacticPosition,
  MAX_CUT_V,
  type SkyRequest,
  type UniverseIdHex,
  type UniverseTime,
} from "@hyperion/protocol";

import type { ViewRole } from "../camera/state";
import {
  type ExposureControl,
  type ExposureProgram,
  type ExposureTriple,
  programTriple,
  shownTriple,
  VIEW_CAMERA,
} from "../photometry/exposure";
import { cameraLimitV, DEFAULT_VIEW_CAMERA } from "./cameraLimit";
import type { ViewStarLimit } from "./cull";
import { DEFAULT_EYE_OBSERVER, eyeDto } from "./eye";
import { eyeLimitAt } from "./limits";
import type { SkyModel } from "./model";

/** The dark sky a camera's limit reads until the band gives its own, cd/m²: μ 24 (Leinert 1998). */
export const DARK_SKY_CD_M2 = 10 ** ((12.58 - 24) / 2.5);

/**
 * A camera's deepest triple: its program's where the digital push starts, the frame's shutter at
 * the top gain (decision-r07-exposure-camera).
 *
 * @remarks
 * At EV100 = log₂(100 × N² ÷ (t_frame × S_max)), −6.12 for {@link VIEW_CAMERA}: f/1.4, 1/30 s,
 * ISO 409,600. Darker, the push adds no gain, so the limit holds; brighter, the gain falls, then
 * the light, and the limit with them. Its limit is therefore the deepest of the program's
 * triples: V 10.06 at 60°, 11.72 at 30° and 13.58 at 13° under μ 24 mag arcsec⁻²
 * ({@link DARK_SKY_CD_M2}). That holds while the program's top gain is the sensor's (R06's
 * `DEFAULT_VIEW_CAMERA.maxIso`, which `cameraLimitParts` clamps the gain to), and only for the
 * program's triples: a `setManual` triple with a shutter beyond 1/30 s would reach deeper, which no
 * console control sets (decision-r07-exposure-camera).
 */
export function deepestTriple(program: ExposureProgram): ExposureTriple {
  const { aperture, frameShutterS, maxIso } = program;
  return programTriple(program, Math.log2((100 * aperture * aperture) / (frameShutterS * maxIso)));
}

/** The view camera's deepest triple, which a camera view's request and cull take. */
const VIEW_DEEPEST_TRIPLE = deepestTriple(VIEW_CAMERA);

/** A camera view's limit at its field of view over the dark sky, V, at a triple. */
function cameraViewLimitV(triple: ExposureTriple, fovDeg: number): number {
  return cameraLimitV(DEFAULT_VIEW_CAMERA, triple, fovDeg, DARK_SKY_CD_M2);
}

/** What a view's request is made from. */
export interface ViewSkyInput {
  readonly universe: UniverseIdHex;
  /** The scene system's barycentre now, the sky's observer. */
  readonly observer: GalacticPosition;
  readonly system: string;
  readonly time: UniverseTime;
  readonly role: ViewRole;
  readonly fovDeg: number;
  /** The setting's N_max. */
  readonly nMax: number;
}

/**
 * The `sky` request a view makes (Design note 5): a camera's at the view camera's deepest limit,
 * whatever the exposure (R07.T13.e).
 */
export function viewSkyRequest(input: ViewSkyInput): SkyRequest {
  const eye = input.role === "eye";
  return {
    universe: input.universe,
    observer: input.observer,
    time: input.time,
    eye: eye ? eyeDto(DEFAULT_EYE_OBSERVER) : null,
    camera_limit_v: eye
      ? null
      : Math.min(MAX_CUT_V, cameraViewLimitV(VIEW_DEEPEST_TRIPLE, input.fovDeg)),
    n_max: input.nMax,
    cone: null,
    exclude_system: input.system,
  };
}

/**
 * A view's limit on a sky it holds, by which it culls: the eye's per texel, or the view camera's
 * deepest, whatever the exposure (R07.T13.e).
 */
export function viewSkyLimit(model: SkyModel, role: ViewRole, fovDeg: number): ViewStarLimit {
  if (role === "camera") {
    return { kind: "camera", limitV: cameraViewLimitV(VIEW_DEEPEST_TRIPLE, fovDeg) };
  }
  const source = {
    band: model.band,
    faceTexels: model.response.band.face_texels,
    requestFieldFactor: model.request.eye?.field_factor ?? null,
  };
  return {
    kind: "eye",
    limitAt: (x, y, z) => eyeLimitAt(source, [x, y, z], DEFAULT_EYE_OBSERVER.fieldFactor),
  };
}

/**
 * The limit a view's label states, V: the eye's deepest over the sky, or the view camera's at the
 * exposure shown (R07.T13.e; the guide's `CAM`, "from its exposure").
 *
 * @param exposure - The control as the readout shows it, whose setting ({@link shownTriple}) sets a
 *   camera's limit; an eye's does not read it.
 */
export function viewSkyLabelV(
  model: SkyModel,
  role: ViewRole,
  exposure: ExposureControl,
  fovDeg: number,
): number {
  if (role === "camera") {
    return cameraViewLimitV(shownTriple(exposure), fovDeg);
  }
  let deepest = Number.NEGATIVE_INFINITY;
  for (const limit of model.band.eyeLimitMag) {
    if (Number.isFinite(limit) && limit > deepest) {
      deepest = limit;
    }
  }
  return Number.isFinite(deepest) ? deepest : model.response.cut_v;
}
