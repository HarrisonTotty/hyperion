/**
 * What a `VIEW` asks of the sky and what it keeps of it (plan R06, T13.c, taking T12's view wiring;
 * Design notes 5, 18 and 20).
 *
 * @remarks
 * An eye view asks the eye's limits (the server sets the eye's cut and each band texel's limit)
 * and keeps a star by its texel's limit and its colour offset; a camera view asks its noise-floor
 * limit and keeps a star by its band term. Until the band layer (T13.d) gives a texel's background
 * to the camera's limit, the camera reads a dark sky of μ 24 mag arcsec⁻², Leinert et al. 1998's
 * integrated starlight at the galactic pole, which asks the deepest limit (the cull is the view's
 * and only shallower).
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
  DEFAULT_MAN_TRIPLE,
  type ExposureControl,
  type ExposureTriple,
} from "../photometry/exposure";
import { cameraLimitV, DEFAULT_VIEW_CAMERA } from "./cameraLimit";
import type { ViewStarLimit } from "./cull";
import { DEFAULT_EYE_OBSERVER, eyeDto } from "./eye";
import { eyeLimitAt } from "./limits";
import type { SkyModel } from "./model";

/** The dark sky a camera's limit reads until the band gives its own, cd/m²: μ 24 (Leinert 1998). */
export const DARK_SKY_CD_M2 = 10 ** ((12.58 - 24) / 2.5);

/**
 * The exposure triple a camera's limit takes: a manual exposure's, else the default manual one
 * until R07's metering states its triple (`ExposureReading.triple`).
 */
export function limitTriple(exposure: ExposureControl): ExposureTriple {
  return exposure.kind === "manual" ? exposure.triple : DEFAULT_MAN_TRIPLE;
}

/** What a view's request is made from. */
export interface ViewSkyInput {
  readonly universe: UniverseIdHex;
  /** The scene system's barycentre now, the sky's observer. */
  readonly observer: GalacticPosition;
  readonly system: string;
  readonly time: UniverseTime;
  readonly role: ViewRole;
  readonly exposure: ExposureControl;
  readonly fovDeg: number;
  /** The setting's N_max. */
  readonly nMax: number;
}

/** The `sky` request a view makes (Design note 5). */
export function viewSkyRequest(input: ViewSkyInput): SkyRequest {
  const eye = input.role === "eye";
  return {
    universe: input.universe,
    observer: input.observer,
    time: input.time,
    eye: eye ? eyeDto(DEFAULT_EYE_OBSERVER) : null,
    camera_limit_v: eye
      ? null
      : Math.min(
          MAX_CUT_V,
          cameraLimitV(
            DEFAULT_VIEW_CAMERA,
            limitTriple(input.exposure),
            input.fovDeg,
            DARK_SKY_CD_M2,
          ),
        ),
    n_max: input.nMax,
    cone: null,
    exclude_system: input.system,
  };
}

/** A view's limit on the sky, and the magnitude its label states. */
export interface ViewSkyLimit {
  readonly limit: ViewStarLimit;
  /** The label's limit: a camera's, or the eye's deepest over the sky. */
  readonly labelV: number;
}

/** A view's limit on a sky it holds. */
export function viewSkyLimit(
  model: SkyModel,
  role: ViewRole,
  exposure: ExposureControl,
  fovDeg: number,
): ViewSkyLimit {
  if (role === "camera") {
    const limitV = cameraLimitV(DEFAULT_VIEW_CAMERA, limitTriple(exposure), fovDeg, DARK_SKY_CD_M2);
    return { limit: { kind: "camera", limitV }, labelV: limitV };
  }
  const source = {
    band: model.band,
    faceTexels: model.response.band.face_texels,
    requestFieldFactor: model.request.eye?.field_factor ?? null,
  };
  let deepest = Number.NEGATIVE_INFINITY;
  for (const limit of model.band.eyeLimitMag) {
    if (Number.isFinite(limit) && limit > deepest) {
      deepest = limit;
    }
  }
  return {
    limit: {
      kind: "eye",
      limitAt: (x, y, z) => eyeLimitAt(source, [x, y, z], DEFAULT_EYE_OBSERVER.fieldFactor),
    },
    labelV: Number.isFinite(deepest) ? deepest : model.response.cut_v,
  };
}
