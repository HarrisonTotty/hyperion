/**
 * What a `VIEW` asks of the sky and what it keeps of it (plan R06, T13.c, taking T12's view wiring;
 * Design notes 5, 18 and 20; R07.T13.e; R06.T13.i).
 *
 * @remarks
 * An eye view asks the eye's limits (the server sets the eye's cut and each band texel's limit)
 * and keeps a star by its texel's limit and its colour offset. A camera view keeps a star by the
 * view camera's limit over the background of the band texel it lies in, and asks the limit over a
 * black sky, which no texel's is deeper than (Design note 18's limit by texel; R06.T13.i,
 * decision-r06-camera-budget). Both are at the camera's deepest triple, at every exposure
 * (decision-r07-exposure-camera), so that no step of `AUTO` and no `MAN` entry asks for the sky
 * again or culls it again; its label states the darkest texel's limit at the exposure shown, never
 * deeper than the reply's cut.
 *
 * Wherever a texel's limit at the exposure shown is 0.05 mag or more shallower than its cull's
 * (from EV100 3.5 under μ 24 mag arcsec⁻²; from 3.3 over a black sky to 6.0 at μ 17), a
 * Sun-coloured star at the shown limit, centred in a pixel at the frame's centre, reaches the tone
 * curve below AgX's floor at 60°: 3.4–5.0 EV below it at 1080p and 1.4–3.0 EV at 4K under μ 24,
 * and at least 2.8 and 0.8 EV over any texel from μ 17 to a black sky. Off the axis, and for the
 * bluest stars, a 4K star can stand above the floor (about 0.3 EV under μ 24), in AgX's toe, where
 * it moves the output by under 0.01 of an 8-bit code over any texel from μ 17 to a black sky; so
 * the deeper cull changes nothing visible.
 * Brighter than μ 17, which T17 did not find near the Sun (μ 21.26–25.25), the margin narrows: at
 * μ 15 a centred 4K star stands 0.3 EV above the floor, and the bluest at a 4K corner moves the
 * output by 0.3 of a code (R06's Risks, T13.i's record).
 */

import {
  type GalacticPosition,
  MAX_CUT_V,
  type SkyBand,
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
import { cubeTexelOf } from "./cube";
import type { ViewStarLimit } from "./cull";
import { DEFAULT_EYE_OBSERVER, eyeDto } from "./eye";
import { eyeLimitAt } from "./limits";
import type { SkyModel } from "./model";

/**
 * A camera's deepest triple: its program's where the digital push starts, the frame's shutter at
 * the top gain (decision-r07-exposure-camera).
 *
 * @remarks
 * At EV100 = log₂(100 × N² ÷ (t_frame × S_max)), −6.12 for {@link VIEW_CAMERA}: f/1.4, 1/30 s,
 * ISO 409,600. Darker, the push adds no gain, so the limit holds; brighter, the gain falls, then
 * the light, and the limit with them. Its limit is therefore the deepest of the program's triples
 * over any background: V 10.15 at 60°, 11.82 at 30° and 14.25 at 10° over a black sky (10.06,
 * 11.72 and 14.16 under μ 24 mag arcsec⁻²). That holds while the program's top gain is the
 * sensor's (R06's `DEFAULT_VIEW_CAMERA.maxIso`, which `cameraLimitParts` clamps the gain to), and
 * only for the program's triples: a `setManual` triple with a shutter beyond 1/30 s would reach
 * deeper, which no console control sets (decision-r07-exposure-camera).
 */
export function deepestTriple(program: ExposureProgram): ExposureTriple {
  const { aperture, frameShutterS, maxIso } = program;
  return programTriple(program, Math.log2((100 * aperture * aperture) / (frameShutterS * maxIso)));
}

/** The view camera's deepest triple, which a camera view's request and cull take. */
const VIEW_DEEPEST_TRIPLE = deepestTriple(VIEW_CAMERA);

/** A camera view's limit, V, at a triple, field of view and background (Design note 18). */
function cameraViewLimitV(triple: ExposureTriple, fovDeg: number, backgroundCdM2: number): number {
  return cameraLimitV(DEFAULT_VIEW_CAMERA, triple, fovDeg, backgroundCdM2);
}

/**
 * A band texel's background as a camera's limit reads it, cd/m²: its luminance as the reply sent
 * it, or a black sky's 0 where the band lacks the texel or its luminance is not finite or is below
 * 0, which the server never sends; a black texel gives the deepest limit, as the eye's missing
 * limit keeps every star.
 */
function texelBackgroundCdM2(luminanceCdM2: number | undefined): number {
  return luminanceCdM2 !== undefined && Number.isFinite(luminanceCdM2) && luminanceCdM2 > 0
    ? luminanceCdM2
    : 0;
}

/** The least of a band's texel backgrounds, cd/m²: a black sky's 0 for a band of no texels. */
function darkestBackgroundCdM2(band: SkyBand): number {
  let darkest = Number.POSITIVE_INFINITY;
  for (let texel = 0; texel < band.count; texel += 1) {
    darkest = Math.min(darkest, texelBackgroundCdM2(band.luminanceCdM2[texel]));
  }
  return Number.isFinite(darkest) ? darkest : 0;
}

/** What a view's request is made from. */
export interface ViewSkyInput {
  readonly universe: UniverseIdHex;
  /**
   * The sky's observer: the camera's galactic position now, its system's barycentre plus its offset
   * (R06.T12), from which the parallax rule measures the camera.
   */
  readonly observer: GalacticPosition;
  readonly system: string;
  readonly time: UniverseTime;
  readonly role: ViewRole;
  readonly fovDeg: number;
  /** The setting's N_max. */
  readonly nMax: number;
}

/**
 * The `sky` request a view makes (Design note 5): a camera's at its black-sky limit, clamped at
 * `MAX_CUT_V`, whatever the exposure (R07.T13.e; R06.T13.i).
 *
 * @remarks
 * The black-sky limit is the view camera's over a background of 0 at its deepest triple: V 10.15
 * at 60°, 10.87 at 45°, 8.96 at 90° and 7.77 at 120°, and 11.0 at 30° and narrower once clamped. A
 * camera's limit falls as its background brightens, so no band texel's is deeper than the black
 * sky's, and no texel's cull is deeper than its census (`decision-r06-t7b-brackets.md`'s
 * invariant).
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
      : Math.min(MAX_CUT_V, cameraViewLimitV(VIEW_DEEPEST_TRIPLE, input.fovDeg, 0)),
    n_max: input.nMax,
    cone: null,
    exclude_system: input.system,
  };
}

/**
 * A camera view's limit in each band texel of a sky it holds, V, never deeper than the reply's cut
 * (Design note 18's limit by texel; R06.T13.i).
 *
 * @remarks
 * Each is the view camera's at its deepest triple and the view's field of view over the texel's
 * luminance as the reply sent it, before the view adds the stars it culls, whose photons fall on
 * their own pixels. They run face after face (+X, −X, +Y, −Y, +Z, −Z), rows from the top, as the
 * band's texels do, and are computed once per held reply and field of view: 6 × 64² of
 * `cameraLimitV` for a 64² band.
 *
 * @param fovDeg - The view's horizontal field of view, degrees, in (0, 180).
 * @throws RangeError for a field of view out of range, as `cameraLimitV` does.
 */
export function cameraTexelLimitsV(model: SkyModel, fovDeg: number): Float64Array {
  const { band } = model;
  const cutV = model.response.cut_v;
  const limits = new Float64Array(band.count);
  for (let texel = 0; texel < band.count; texel += 1) {
    const backgroundCdM2 = texelBackgroundCdM2(band.luminanceCdM2[texel]);
    limits[texel] = Math.min(cutV, cameraViewLimitV(VIEW_DEEPEST_TRIPLE, fovDeg, backgroundCdM2));
  }
  return limits;
}

/**
 * A view's limit on a sky it holds, by which it culls: the eye's per texel, or the view camera's
 * per texel at its deepest triple, whatever the exposure (R07.T13.e; R06.T13.i).
 */
export function viewSkyLimit(model: SkyModel, role: ViewRole, fovDeg: number): ViewStarLimit {
  const faceTexels = model.response.band.face_texels;
  if (role === "camera") {
    const limits = cameraTexelLimitsV(model, fovDeg);
    // A texel the band lacks reads a black sky's limit, as `texelBackgroundCdM2` has it.
    const unbandedV = Math.min(
      model.response.cut_v,
      cameraViewLimitV(VIEW_DEEPEST_TRIPLE, fovDeg, 0),
    );
    return {
      kind: "camera",
      limitAt: (x, y, z) => {
        const { face, column, row } = cubeTexelOf(x, y, z, faceTexels);
        return limits[(face * faceTexels + row) * faceTexels + column] ?? unbandedV;
      },
    };
  }
  const source = {
    band: model.band,
    faceTexels,
    requestFieldFactor: model.request.eye?.field_factor ?? null,
  };
  return {
    kind: "eye",
    limitAt: (x, y, z) => eyeLimitAt(source, [x, y, z], DEFAULT_EYE_OBSERVER.fieldFactor),
  };
}

/**
 * The limit a view's label states, V, quoted at its deepest over the whole sky (the guide's `EYE`,
 * `CAM`): the eye's deepest texel's, or the view camera's over the sky's darkest band texel at the
 * exposure shown, never deeper than the reply's cut (R07.T13.e; R06.T13.i).
 *
 * @remarks
 * A camera's limit falls as its background brightens, so the darkest texel's is the deepest of all
 * six faces', and the figure does not move as the camera turns. The cut caps it, since the sky
 * holds its stars to that: at 30° near the Sun the camera's limit is V 11.2–11.8 and a camera's
 * sky is cut at 11.0, and an instrument's camera culling the eye's sky reads the eye's cut.
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
    return Math.min(
      model.response.cut_v,
      cameraViewLimitV(shownTriple(exposure), fovDeg, darkestBackgroundCdM2(model.band)),
    );
  }
  let deepest = Number.NEGATIVE_INFINITY;
  for (const limit of model.band.eyeLimitMag) {
    if (Number.isFinite(limit) && limit > deepest) {
      deepest = limit;
    }
  }
  return Number.isFinite(deepest) ? deepest : model.response.cut_v;
}
