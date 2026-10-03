/**
 * A body's photometry and figure from what the wire carries today (plan R07, T2.a; Design notes 5
 * and 19).
 *
 * @remarks
 * Plan 14 sends no photometric section and no flattening yet (R07.T1 asks for both), so every body
 * takes {@link PROVISIONAL_PHOTOMETRY}, labelled `BODY ALBEDO: NOT YET MODELLED`, and is a sphere of
 * its mean radius, `bulk.radius_m`. Its pole is the body-fixed z axis in the body frame from R02's
 * `bodyFixedRotation`, `null` while plan 14 sends no rotation. A `contact` entry of R03's frame has
 * no body summary here and stays R02's mark (decisions-r06-r07, item 4); R07.T2.b replaces the
 * provisional branch once the section is on the wire.
 */
import type { SystemBody } from "../../lib/system/model";
import { type Rotation3, rotateToBody } from "../coords/rotation";
import type { BodyFigure } from "../terrain/planet";
import type { Rgb } from "../photometry/toneCurve";
import type { PhotometricLaw } from "./law";
import { lawFor } from "./phase";

export type { BodyFigure } from "../terrain/planet";

/** A body's photometry, as R08 and R10 read it through `BodyAppearance` (Design note 5). */
export interface BodyPhotometry {
  /** p per display channel (r, g, b). */
  readonly geometricAlbedo: Rgb;
  /** q per display channel (r, g, b), from the law. */
  readonly phaseIntegral: Rgb;
  readonly law: PhotometricLaw;
  /** p_V q_V ÷ A_Bond as plan 14 states it, a check only; `null` until the section carries it. */
  readonly bondRatioCheck: number | null;
  readonly provenance: "modelled" | "provisional";
}

/** The label a body without a photometric section carries (a phrase for the owner, R07.T16). */
export type AppearanceLabel = "BODY ALBEDO: NOT YET MODELLED";

/** The provisional photometry's geometric albedo, every channel (Design note 5). */
const PROVISIONAL_ALBEDO = 0.2;

/** The provisional photometry's phase integral, a Lambert sphere's 3 ÷ 2. */
const PROVISIONAL_PHASE_INTEGRAL = 1.5;

/**
 * Design note 5's photometry for a body with no photometric section: a Lambert sphere of spherical
 * albedo 0.3 (p = 0.2, q = 1.5), brighter at large phase than any real body, labelled.
 */
export const PROVISIONAL_PHOTOMETRY: BodyPhotometry = {
  geometricAlbedo: [PROVISIONAL_ALBEDO, PROVISIONAL_ALBEDO, PROVISIONAL_ALBEDO],
  phaseIntegral: [
    PROVISIONAL_PHASE_INTEGRAL,
    PROVISIONAL_PHASE_INTEGRAL,
    PROVISIONAL_PHASE_INTEGRAL,
  ],
  law: lawFor(
    [PROVISIONAL_ALBEDO, PROVISIONAL_ALBEDO, PROVISIONAL_ALBEDO],
    [PROVISIONAL_PHASE_INTEGRAL, PROVISIONAL_PHASE_INTEGRAL, PROVISIONAL_PHASE_INTEGRAL],
    "lambert",
  ),
  bondRatioCheck: null,
  provenance: "provisional",
};

/** What the wire gives a body's shading today: its photometry, its figure and its labels. */
export interface WireAppearance {
  readonly photometry: BodyPhotometry;
  /** `null` where the body's radius is not granted: it stays R02's mark. */
  readonly figure: BodyFigure | null;
  readonly labels: ReadonlyArray<AppearanceLabel>;
}

/**
 * A body's photometry, figure and labels from its summary.
 *
 * @param rotation - The body's body-fixed rotation (`CameraOrigins.bodyFixedRotation`), or `null`
 *   where plan 14 sends none.
 */
export function appearanceFromWire(body: SystemBody, rotation: Rotation3 | null): WireAppearance {
  const radiusM = body.bulk.state === "ok" ? body.bulk.value.radiusM : null;
  const pole = rotation === null ? null : rotateToBody(rotation, { x: 0, y: 0, z: 1 });
  return {
    photometry: PROVISIONAL_PHOTOMETRY,
    figure:
      radiusM === null || radiusM <= 0
        ? null
        : { equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole },
    labels: ["BODY ALBEDO: NOT YET MODELLED"],
  };
}
