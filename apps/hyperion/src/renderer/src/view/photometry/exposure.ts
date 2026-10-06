/**
 * The camera's exposure: a value in EV100 with three automation levels (plan R02, Design note 11;
 * brainstorm, "Luminance in physical units, and an exposure model").
 *
 * @remarks
 * After Filament, "Physically based camera" ("Exposure value", "Exposure settings", "Exposure"),
 * and bruop.github.io/exposure, both after Lagarde and de Rousiers, _Moving Frostbite to PBR_
 * (SIGGRAPH 2014).
 */

/**
 * A camera's aperture, shutter, sensitivity and neutral-density filter: the view camera's setting
 * at an exposure, or the triple a `MAN` exposure is set from (R06's `cameraLimitV` takes it).
 */
export interface ExposureTriple {
  /** The f-number N, positive. */
  readonly aperture: number;
  /** The shutter time t, s, positive. */
  readonly shutterS: number;
  /** The sensitivity S, ISO, positive. */
  readonly iso: number;
  /**
   * The neutral-density filter's attenuation, EV, not negative; absent while the filter is clear
   * (decision-r07-exposure-camera).
   */
  readonly ndEv?: number;
}

/**
 * The reflected-light meter's calibration constant K, cd s m⁻² ISO: 12.5 (Filament, "Exposure
 * settings"; the value Canon, Nikon and Sekonic use under ISO 2720).
 */
export const METER_CALIBRATION_K = 12.5;

/**
 * The lens and vignetting attenuation q of the saturation-based sensitivity: 0.65 (Filament,
 * "Exposure"; ISO 12232 takes q = 0.65 for a typical lens).
 */
export const LENS_ATTENUATION_Q = 0.65;

/** ISO 12232's saturation-based sensitivity constant: 78 (Filament, "Exposure"). */
export const SATURATION_CONSTANT = 78;

/**
 * The default `MAN` exposure, EV100: −1 (plan R02, Design note 11).
 *
 * @remarks
 * Computed through the full AgX and the point-spread function at 1080p and 60°: Sirius's peak pixel
 * reaches display-linear 0.957 (sRGB code 250) and a mag 6.5 star's 5.2 × 10⁻³ (code 16), visible,
 * where +1 would leave the mag 6.5 star invisible in a lit room and −2 would flatten Sirius's peak
 * at the curve's log clamp. Stated for 1080p; the recorded run (R02.T18) confirms it.
 */
export const DEFAULT_MAN_EV100 = -1;

/**
 * The least `MAN` exposure the operator may enter, EV100: −14 (decision-r07-man-exposure).
 *
 * @remarks
 * The foot of the brainstorm's scene span, the galactic band's faintest background at about
 * 10⁻⁵ cd/m², metered at R02's EV100 = log₂(L̄ × 8) (−13.6), rounded outward.
 */
export const MAN_EV100_MIN = -14;

/**
 * The greatest `MAN` exposure the operator may enter, EV100: 42 (decision-r07-man-exposure).
 *
 * @remarks
 * The top of the brainstorm's scene span, an O star's disc centre at about 3 × 10¹¹ cd/m², metered
 * at log₂(L̄ × 8) (41.1), rounded outward. The view camera's ND is dense enough for it.
 */
export const MAN_EV100_MAX = 42;

/**
 * A view camera's exposure program: its fixed lens and its ranges, which set its triple as a
 * function of EV100 alone, under `AUTO`, `INHIBITED` and `MAN` alike (decision-r07-exposure-camera).
 */
export interface ExposureProgram {
  /** The fixed f-number N. */
  readonly aperture: number;
  /** The longest shutter, the live frame's, s. */
  readonly frameShutterS: number;
  /** The shortest shutter, s. */
  readonly minShutterS: number;
  /** The sensor's base sensitivity, ISO: the program never sets less. */
  readonly baseIso: number;
  /** The sensor's top gain, ISO; beyond it the sensitivity is a digital push. */
  readonly maxIso: number;
  /** The neutral-density filter's densest, EV. */
  readonly maxNdEv: number;
}

/** The view camera's fixed f-number (R06 Design note 18's N). */
const VIEW_APERTURE = 1.4;

/** The view camera's shortest shutter, s: 1/8,000 s. */
const VIEW_MIN_SHUTTER_S = 1 / 8_000;

/**
 * The view camera (decision-r07-exposure-camera): R06's sensor (`DEFAULT_VIEW_CAMERA`) behind a
 * fixed f/1.4 lens, a shutter of 1/30 to 1/8,000 s, ISO 100 to 409,600 and a variable ND filter to
 * 28.06 EV (optical density 8.45).
 *
 * @remarks
 * f/1.4 and 1/30 s are R06 Design note 18's N and t, the live frame at 30 fps. 1/8,000 s is the
 * shortest movie shutter of Sony's α7S III and FX6. ISO 409,600 is the high gain of Design note 18,
 * 12 stops above base, the expanded top of the α7S III and the FX6. The ND's densest is what
 * {@link MAN_EV100_MAX}, 42, needs at 1/8,000 s and ISO 100, so that no `MAN` exposure is beyond
 * it; the FX6 carries a variable ND, and a camera imaging a star's disc an OD 5.0 solar filter, as
 * MER's Pancam and MSL's Mastcam do.
 */
export const VIEW_CAMERA: ExposureProgram = {
  aperture: VIEW_APERTURE,
  frameShutterS: 1 / 30,
  minShutterS: VIEW_MIN_SHUTTER_S,
  baseIso: 100,
  maxIso: 409_600,
  maxNdEv: MAN_EV100_MAX - Math.log2((VIEW_APERTURE * VIEW_APERTURE) / VIEW_MIN_SHUTTER_S),
};

/**
 * A camera's triple at an exposure under its program (decision-r07-exposure-camera): from dark to
 * bright, gain at the frame's shutter, then the shutter at base sensitivity, then the ND filter.
 *
 * @remarks
 * With EV100 = log₂(N² ÷ t) − log₂(S ÷ 100) + ND (ISO 2720, APEX): the frame's shutter with
 * S = 100 × (N² ÷ t) × 2^−EV100 while S is at least the base, beyond the top gain included, where
 * the picture is pushed digitally (darker than EV100 −6.12 for {@link VIEW_CAMERA}); then base
 * sensitivity with t = N² × 100 ÷ (S_base × 2^EV100) down to the shortest shutter (to 13.94); then
 * the shortest shutter with ND = EV100 − (log₂(N² ÷ t_min) − log₂(S_base ÷ 100)). At f/1.4 and
 * 1/30 s the first is S = 5,880 × 2^−EV100, which is K N² ÷ (t L̄) at the metered L̄ = 2^EV100 ÷ 8.
 * The sensitivity never falls below base, and the shutter never leaves its range; the ND is absent
 * while the filter is clear. The triple's {@link ev100FromTriple} is `ev100`.
 *
 * @throws RangeError if `ev100` is not finite.
 */
export function programTriple(program: ExposureProgram, ev100: number): ExposureTriple {
  if (!Number.isFinite(ev100)) {
    throw new RangeError(`an exposure program needs a finite EV100, not ${ev100}`);
  }
  const { aperture, frameShutterS, minShutterS, baseIso } = program;
  const squared = aperture * aperture;
  const iso = 100 * (squared / frameShutterS) * 2 ** -ev100;
  if (iso >= baseIso) {
    return { aperture, shutterS: frameShutterS, iso };
  }
  const shutterS = ((squared * 100) / baseIso) * 2 ** -ev100;
  if (shutterS >= minShutterS) {
    return { aperture, shutterS, iso: baseIso };
  }
  const ndEv = ev100 - (Math.log2(squared / minShutterS) - Math.log2(baseIso / 100));
  // At the join itself rounding can leave no attenuation: the filter is then clear, and absent.
  return ndEv > 0
    ? { aperture, shutterS: minShutterS, iso: baseIso, ndEv }
    : { aperture, shutterS: minShutterS, iso: baseIso };
}

/**
 * The view camera's triple at {@link DEFAULT_MAN_EV100}: f/1.4, 1/30 s, ISO 11,760
 * (decision-r07-exposure-camera).
 */
export const DEFAULT_MAN_TRIPLE: ExposureTriple = programTriple(VIEW_CAMERA, DEFAULT_MAN_EV100);

/** Whether a triple's members are finite and positive, and its ND, if any, finite and not negative. */
function isValidTriple(triple: ExposureTriple): boolean {
  const { aperture, shutterS, iso, ndEv } = triple;
  return (
    [aperture, shutterS, iso].every((value) => Number.isFinite(value) && value > 0) &&
    (ndEv === undefined || (Number.isFinite(ndEv) && ndEv >= 0))
  );
}

/**
 * The exposure value at ISO 100 of a triple: EV100 = log₂(N² ÷ t) − log₂(S ÷ 100) + ND, the ND in
 * EV.
 *
 * @throws RangeError if a member of the triple is not finite and positive, or its ND is negative
 *   or not finite.
 */
export function ev100FromTriple(triple: ExposureTriple): number {
  if (!isValidTriple(triple)) {
    throw new RangeError(
      "an exposure triple's members must be finite and positive, and its ND finite and not negative",
    );
  }
  const { aperture, shutterS, iso, ndEv } = triple;
  return Math.log2((aperture * aperture) / shutterS) - Math.log2(iso / 100) + (ndEv ?? 0);
}

/**
 * The exposure value at ISO 100 that a metered average luminance calls for: EV100 = log₂(L̄ × S ÷
 * K) with S = 100 and K = {@link METER_CALIBRATION_K}.
 *
 * @param averageCdPerM2 - The metered average luminance, cd/m², positive.
 */
export function ev100FromAverageLuminance(averageCdPerM2: number): number {
  return Math.log2((averageCdPerM2 * 100) / METER_CALIBRATION_K);
}

/**
 * The scale that multiplies luminance at an exposure: 1 ÷ L_max, with L_max the saturation-based
 * maximum (N² ÷ t) × 78 ÷ (q S) = 1.2 × 2^EV100.
 *
 * @remarks
 * 78 ÷ (0.65 × 100) = 1.2 exactly. Under `AUTO` it puts the white point at 9.6 times the metered
 * average, and the average at 1 ÷ 9.6 = 0.104, 0.79 stops below AgX's 0.18 middle grey: a
 * calibration R07 accepts without compensation (its design note 9). Filament, "Exposure";
 * bruop.github.io/exposure.
 */
export function exposureScale(ev100: number): number {
  return 1 / ((SATURATION_CONSTANT / (LENS_ATTENUATION_Q * 100)) * 2 ** ev100);
}

/** Who inhibited automatic exposure, and so why. */
export type InhibitReason = "operator" | "no_image_to_meter";

/**
 * The exposure's automation level and value (Design note 11).
 *
 * @remarks
 * `manual` is set by the operator, from an EV100 entered, at the view camera's triple
 * ({@link setManualEv100}), or from a triple ({@link setManual}). `auto` follows the photorealistic
 * view the view accompanies (R07), holding its last metered value. `inhibited` means the automatic
 * function is prevented from acting: the exposure is held at its last metered value, with who
 * inhibited it and why. A system inhibit (the source closed or faulted) returns to `auto` by itself
 * when the source returns; an operator's does not.
 */
export type ExposureControl =
  | { readonly kind: "manual"; readonly triple: ExposureTriple }
  | { readonly kind: "auto"; readonly ev100: number }
  | { readonly kind: "inhibited"; readonly ev100: number; readonly reason: InhibitReason };

/** A new view's exposure: `MAN` at the default triple. */
export const DEFAULT_EXPOSURE: ExposureControl = { kind: "manual", triple: DEFAULT_MAN_TRIPLE };

/** An exposure command's outcome: the new control, or why it was refused. */
export type ExposureCommandResult =
  | { readonly kind: "accepted"; readonly control: ExposureControl }
  | {
      readonly kind: "refused";
      readonly reason:
        | "no_image_to_meter"
        | "not_automatic"
        | "already_auto"
        | "already_inhibited"
        | "invalid_triple"
        | "invalid_ev100";
    };

/** The exposure value a control stands at, EV100. */
export function controlEv100(control: ExposureControl): number {
  let ev100: number;
  switch (control.kind) {
    case "manual":
      ev100 = ev100FromTriple(control.triple);
      break;
    case "auto":
    case "inhibited":
      ev100 = control.ev100;
      break;
  }
  return ev100;
}

/**
 * The view camera's setting at a control: `MAN`'s own triple, else {@link VIEW_CAMERA}'s at the
 * control's EV100 (decision-r07-exposure-camera).
 *
 * @remarks
 * The exposure panel's `APERTURE`, `SHUTTER`, `ND` and `ISO` rows show it (R07.T13.c), and the
 * sky label's `STARS V … mag CAM` states the camera's limit at it (R07.T13.e), so that the two
 * read one setting.
 */
export function shownTriple(control: ExposureControl): ExposureTriple {
  return control.kind === "manual"
    ? control.triple
    : programTriple(VIEW_CAMERA, controlEv100(control));
}

/**
 * `MAN` at a triple, from any level, by the operator; refused where a member of the triple is not
 * finite and positive, or its ND is negative or not finite.
 */
export function setManual(triple: ExposureTriple): ExposureCommandResult {
  if (!isValidTriple(triple)) {
    return { kind: "refused", reason: "invalid_triple" };
  }
  return { kind: "accepted", control: { kind: "manual", triple } };
}

/**
 * `MAN` at an exposure the operator enters, from any level (decision-r07-man-exposure): the view
 * camera's triple at that EV100 (decision-r07-exposure-camera); refused with `invalid_ev100` where
 * it is not finite or lies outside {@link MAN_EV100_MIN} to {@link MAN_EV100_MAX}.
 *
 * @remarks
 * The level it is entered from does not change the triple: one camera serves every level, so the
 * entry's setting is the one `AUTO` would show at that EV100. It is not smoothed and not pending,
 * since the exposure is the view's own, a display control.
 */
export function setManualEv100(ev100: number): ExposureCommandResult {
  if (!(Number.isFinite(ev100) && ev100 >= MAN_EV100_MIN && ev100 <= MAN_EV100_MAX)) {
    return { kind: "refused", reason: "invalid_ev100" };
  }
  return {
    kind: "accepted",
    control: { kind: "manual", triple: programTriple(VIEW_CAMERA, ev100) },
  };
}

/**
 * `AUTO`, by the operator, only with a metering source: a wireframe view has no image to meter, so
 * until R07's photorealistic view accompanies it this is refused with `NO IMAGE TO METER`.
 *
 * @param meteredEv100 - The source's current metered value, or `null` where there is no source.
 */
export function setAuto(meteredEv100: number | null): ExposureCommandResult {
  if (meteredEv100 === null) {
    return { kind: "refused", reason: "no_image_to_meter" };
  }
  return { kind: "accepted", control: { kind: "auto", ev100: meteredEv100 } };
}

/**
 * The operator's `INHIBIT`: holds an `AUTO` exposure where it stands, or takes over a system inhibit
 * as the operator's, so that it no longer resumes by itself when the source returns; refused where
 * it cannot act, under `MAN` (`not_automatic`), and where its effect already holds, under the
 * operator's own inhibit (`already_inhibited`, decision-r07-owner-ux-signoff, item 1).
 */
export function inhibit(control: ExposureControl): ExposureCommandResult {
  if (control.kind === "manual") {
    return { kind: "refused", reason: "not_automatic" };
  }
  if (control.kind === "inhibited" && control.reason === "operator") {
    return { kind: "refused", reason: "already_inhibited" };
  }
  return {
    kind: "accepted",
    control: { kind: "inhibited", ev100: control.ev100, reason: "operator" },
  };
}

/**
 * The operator's `ENABLE`: sets the exposure to `AUTO` at the metered value, from `MAN` or from an
 * inhibit of either origin (the guide's `ENABLE` rows; R07.T8.a, decision-r07-t8a-meter);
 * refused with `NO IMAGE TO METER` while there is no source, and under `AUTO`.
 *
 * @param meteredEv100 - The source's current metered value, or `null` where there is no source.
 */
export function enable(
  control: ExposureControl,
  meteredEv100: number | null,
): ExposureCommandResult {
  if (control.kind === "auto") {
    return { kind: "refused", reason: "already_auto" };
  }
  return setAuto(meteredEv100);
}

/**
 * The control after its metering source reports: a new metered value, or `null` when the source
 * closed or faulted.
 *
 * @remarks
 * `AUTO` follows the value, and on the loss of the source becomes a system inhibit
 * (`INHIBITED · NO IMAGE TO METER`) held at the last value; a system inhibit resumes `AUTO` when the
 * source returns; an operator's inhibit and `MAN` are not touched.
 */
export function onMetering(control: ExposureControl, meteredEv100: number | null): ExposureControl {
  let next: ExposureControl;
  switch (control.kind) {
    case "manual":
      next = control;
      break;
    case "auto":
      next =
        meteredEv100 === null
          ? { kind: "inhibited", ev100: control.ev100, reason: "no_image_to_meter" }
          : { kind: "auto", ev100: meteredEv100 };
      break;
    case "inhibited":
      next =
        control.reason === "no_image_to_meter" && meteredEv100 !== null
          ? { kind: "auto", ev100: meteredEv100 }
          : control;
      break;
  }
  return next;
}

/** The automation level's reading: `MAN`, `AUTO`, or `INHIBITED` with who inhibited it. */
export function exposureLevelReading(control: ExposureControl): string {
  let reading: string;
  switch (control.kind) {
    case "manual":
      reading = "MAN";
      break;
    case "auto":
      reading = "AUTO";
      break;
    case "inhibited":
      reading =
        control.reason === "operator" ? "INHIBITED · OPERATOR" : "INHIBITED · NO IMAGE TO METER";
      break;
  }
  return reading;
}
