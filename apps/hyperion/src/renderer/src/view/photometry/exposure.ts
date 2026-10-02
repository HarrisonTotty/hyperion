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
 * A camera's aperture, shutter and sensitivity: the triple a `MAN` exposure is set from (R06's
 * `cameraLimitV` takes it).
 */
export interface ExposureTriple {
  /** The f-number N, positive. */
  readonly aperture: number;
  /** The shutter time t, s, positive. */
  readonly shutterS: number;
  /** The sensitivity S, ISO, positive. */
  readonly iso: number;
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

/** A triple that gives {@link DEFAULT_MAN_EV100}: f/1, 2 s, ISO 100. */
export const DEFAULT_MAN_TRIPLE: ExposureTriple = { aperture: 1, shutterS: 2, iso: 100 };

/**
 * The exposure value at ISO 100 of a triple: EV100 = log₂(N² ÷ t) − log₂(S ÷ 100).
 *
 * @throws RangeError if a member of the triple is not finite and positive.
 */
export function ev100FromTriple(triple: ExposureTriple): number {
  const { aperture, shutterS, iso } = triple;
  for (const value of [aperture, shutterS, iso]) {
    if (!(Number.isFinite(value) && value > 0)) {
      throw new RangeError("an exposure triple's members must be finite and positive");
    }
  }
  return Math.log2((aperture * aperture) / shutterS) - Math.log2(iso / 100);
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
 * `manual` is set from a triple by the operator. `auto` follows the photorealistic view the view
 * accompanies (R07), holding its last metered value. `inhibited` means the automatic function is
 * prevented from acting: the exposure is held at its last metered value, with who inhibited it and
 * why. A system inhibit (the source closed or faulted) returns to `auto` by itself when the source
 * returns; an operator's does not.
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
      readonly reason: "no_image_to_meter" | "not_automatic" | "not_inhibited" | "invalid_triple";
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
 * `MAN` at a triple, from any level, by the operator; refused where a member of the triple is not
 * finite and positive.
 */
export function setManual(triple: ExposureTriple): ExposureCommandResult {
  const valid = [triple.aperture, triple.shutterS, triple.iso].every(
    (value) => Number.isFinite(value) && value > 0,
  );
  if (!valid) {
    return { kind: "refused", reason: "invalid_triple" };
  }
  return { kind: "accepted", control: { kind: "manual", triple } };
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
 * as the operator's, so that it no longer resumes by itself when the source returns.
 */
export function inhibit(control: ExposureControl): ExposureCommandResult {
  if (control.kind === "manual") {
    return { kind: "refused", reason: "not_automatic" };
  }
  return {
    kind: "accepted",
    control: { kind: "inhibited", ev100: control.ev100, reason: "operator" },
  };
}

/**
 * The operator's `ENABLE`: clears an inhibit, of either origin, back to `AUTO`; refused with
 * `NO IMAGE TO METER` while there is no source.
 *
 * @param meteredEv100 - The source's current metered value, or `null` where there is no source.
 */
export function enable(
  control: ExposureControl,
  meteredEv100: number | null,
): ExposureCommandResult {
  if (control.kind !== "inhibited") {
    return { kind: "refused", reason: "not_inhibited" };
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
