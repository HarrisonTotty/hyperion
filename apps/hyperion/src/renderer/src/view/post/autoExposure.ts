/**
 * The meter and the exposure controller: the metered average from a histogram, smoothed in EV,
 * under R02's three automation levels (plan R07, T13.a, Design notes 10–12).
 *
 * @remarks
 * Exposure is computed on the CPU from the histogram read back one to three frames late, so the
 * displayed value is exactly the applied one and the controller is tested without a GPU. The next
 * frame's pre-exposure is the latest applied exposure.
 */

import type { ViewId } from "../camera/state";
import {
  controlEv100,
  ev100FromAverageLuminance,
  MAN_EV100_MIN,
  METER_CALIBRATION_K,
  NO_IMAGE_INHIBIT,
  NO_IMAGE_TO_METER,
  NOTHING_WEIGHED_STATUS,
  onMetering,
  programTriple,
  type ExposureCommandResult,
  type ExposureControl,
  type ExposureProgram,
  type ExposureTriple,
} from "../photometry/exposure";
import {
  binCentreLuminance,
  HISTOGRAM_BINS,
  HISTOGRAM_MIN_LOG2,
  type Histogram,
} from "./histogram";
import type { MeterMode } from "./meter";

/** The fraction of the histogram's counts, from the darkest, that the mean takes: [0, 1] by default. */
export interface PercentileWindow {
  /** The fraction of counts dropped from the dark end, in [0, 1]. */
  readonly low: number;
  /** The fraction of counts kept up to, from the dark end, in [low, 1]. */
  readonly high: number;
}

/**
 * The meter's default window: every count. A reflected-light meter integrates flux, and a clip of
 * the top 1% would delete a small planet (Design note 11).
 */
export const FULL_WINDOW: PercentileWindow = { low: 0, high: 1 };

/**
 * The metered average luminance, cd/m²: the weighted arithmetic mean Σ nᵢ Lᵢ ÷ Σ nᵢ over the
 * counts inside `window`, with Lᵢ = 2^(bin centre) ÷ the pre-exposure and bin 0 at luminance 0
 * (Design note 11). The glare sources' veil is in it, of each source's whole light, as the optic
 * delivers it (`histogram.wgsl`); the host disc's own pixels are not, and nor is the bloom chain's
 * light, which only moves the weighed texels' light and leaves the disc's out.
 *
 * @returns 0 when the window holds no counts.
 */
export function meteredLuminance(h: Histogram, window: PercentileWindow = FULL_WINDOW): number {
  let total = 0;
  for (const n of h.bins) {
    total += n;
  }
  const from = window.low * total;
  const to = window.high * total;
  let below = 0;
  let weight = 0;
  let flux = 0;
  for (let bin = 0; bin < HISTOGRAM_BINS; bin += 1) {
    const n = h.bins[bin] ?? 0;
    const kept = Math.max(0, Math.min(below + n, to) - Math.max(below, from));
    below += n;
    if (kept > 0) {
      weight += kept;
      flux += kept * binCentreLuminance(bin);
    }
  }
  return weight > 0 ? flux / weight / h.preExposure : 0;
}

/**
 * The least average a frame whose counted pixels all fall below the histogram's range is metered
 * at, cd/m²: 2⁻¹⁷, so that `AUTO` goes no darker than EV100 −14 for it (decision-r07-t13d,
 * R07.T13.a's follow-up).
 *
 * @remarks
 * R02's {@link MAN_EV100_MIN}, −14, the foot of the brainstorm's scene span, taken back through
 * R02's EV100 = log₂(L̄ × S ÷ K) with S = 100 and K = {@link METER_CALIBRATION_K}, 12.5, that is
 * log₂(8 L̄): L̄ = 2⁻¹⁴ × 12.5 ÷ 100 = 2⁻¹⁷, exact in binary.
 */
export const EMPTY_FRAME_CD_M2 = (2 ** MAN_EV100_MIN * METER_CALIBRATION_K) / 100;

/**
 * The average a controller meters from a histogram, cd/m², or `null` when no pixel counts.
 *
 * @remarks
 * A frame whose counted pixels all fall below the histogram's range (bin 0) meters at that range's
 * floor, 2⁻¹⁴ ÷ the pre-exposure, an upper bound on its true mean, rather than at 0: the exposure
 * then steps darker, the next pre-exposure follows, and the frame comes into range. A mean of 0
 * would read as nothing to meter, and since the pre-exposure follows the held exposure the frame
 * would never come back into range.
 *
 * A frame of exact zeros never comes into range, so that floor alone would keep the metered value
 * 10.74 EV below the applied one and `AUTO` would darken at 1 EV/s without end, past −100 (wider
 * than the `MAN` field's 5ch fill) and toward an `f32` overflow of the pre-exposure below about
 * −128. It is bounded below by {@link EMPTY_FRAME_CD_M2}, EV100 −14. The range's floor is the
 * larger above an applied EV100 of −3.26 (log₂(2⁻³ ÷ 1.2)), so the exposure steps darker there as
 * before, and below it such a frame aims at −14. At −14 every counted pixel above about
 * 4.5 × 10⁻⁹ cd/m² (1.2 × 2⁻²⁸) is in range and meters by its light, below −14 included. Only a
 * frame whose counted pixels stay below the range on the way to −14 is held there: from above,
 * every counted pixel below 4.5 × 10⁻⁹ cd/m²; from below, every counted pixel below 2⁻¹⁴ ÷ the
 * pre-exposure. Those pixels are drawn black, and pixels the meter does not weigh are drawn at −14.
 */
export function meteredAverage(
  h: Histogram,
  window: PercentileWindow = FULL_WINDOW,
): number | null {
  let total = 0;
  for (const n of h.bins) {
    total += n;
  }
  if (total === 0) {
    return null;
  }
  const average = meteredLuminance(h, window);
  return average > 0
    ? average
    : Math.max(2 ** HISTOGRAM_MIN_LOG2 / h.preExposure, EMPTY_FRAME_CD_M2);
}

/**
 * The smoothing speed, EV/s, the same toward a brighter scene and a darker one: a camera's
 * automatic exposure, which settles in 0.1 to 0.5 s (fix-view-light's design value, 2026-11-01,
 * after a still camera's AE on a step of a few EV; no measurement read), linear while the
 * metered value is more than {@link SMOOTHING_BAND_EV} away and exponential inside the band at
 * speed ÷ band, 16 s⁻¹, with a continuous first derivative (Unreal's form, UE 4.27, "Auto
 * Exposure", `r.EyeAdaptation.ExponentialTransitionDistance`). A 10 EV step comes within 0.1 EV in
 * 0.52 s and a 3 EV step in 0.23 s.
 *
 * @remarks
 * It replaces Unreal's `FCameraExposureSettings` defaults, 3 EV/s brightening and 1 darkening (UE
 * 4.26 Python API), which left a sunlit body some 8 s overexposed after a starfield and took some
 * 23 s back. Every view is a camera, `VIEW_CAMERA`'s triple at every level, and the operator's own
 * eyes adapt to the display. The eye is not modelled: its dark adaptation takes minutes, the cones'
 * plateau near 10 min and the rods' at 30 to 40 min (Hecht, Haig and Chase 1937, "The influence of
 * light adaptation on subsequent dark adaptation of the eye", J. Gen. Physiol. 20, 831–850,
 * doi:10.1085/jgp.20.6.831; recalled, not read here), which would blind the console for minutes
 * after a star left the frame.
 */
export const SMOOTHING_SPEED_EV_PER_S = 24;

/** The half-width of the band, EV, inside which the approach is exponential: Unreal's default 1.5. */
export const SMOOTHING_BAND_EV = 1.5;

/**
 * The longest a cut holds the view's image back while it waits for a reading inside the
 * histogram's range, s: 0.15, nine frames at 60 Hz, three readings each the reader's three frames
 * late at most (`HISTOGRAM_RING`); each frame counts at most {@link METER_STEP_MAX_S}.
 */
export const CUT_HOLD_MAX_S = 0.15;

/**
 * The fraction of a reading's weighed counts in the histogram's top bin above which the reading is
 * clipped: 0.5%. Those texels hold at least 60,404 after pre-exposure (2^15.88, the top bin's
 * floor), up to `rgba16float`'s clamp at 65,504 and the veil's light beyond it, all metered at the
 * bin's centre, so the mean is only a lower bound on the light. At `AUTO` the top bin lies
 * 19.2 EV above the metered mean.
 */
export const CLIPPED_TOP_FRACTION = 0.005;

/**
 * Where a reading lies against the histogram's range: `inside` it, a value; `above` it, more than
 * {@link CLIPPED_TOP_FRACTION} of its weighed counts in the top bin, so that the light is at least
 * what it meters; or `below` it, every count in bin 0, so that the light is at most what it
 * meters.
 */
export type ReadingRange = "inside" | "above" | "below";

/** Where a histogram's reading lies against its range; `inside` for one that weighs nothing. */
export function readingRange(h: Histogram): ReadingRange {
  let total = 0;
  for (const n of h.bins) {
    total += n;
  }
  const top = h.bins[HISTOGRAM_BINS - 1] ?? 0;
  const zero = h.bins[0] ?? 0;
  let range: ReadingRange = "inside";
  if (total > 0 && top > CLIPPED_TOP_FRACTION * total) {
    range = "above";
  } else if (total > 0 && zero === total) {
    range = "below";
  }
  return range;
}

/**
 * How long the meter holds its value without a histogram that weighs a pixel before it reports
 * why it has none, s: well above the reader's one to three frames, so a dropped frame does not
 * inhibit `AUTO`. It is also the `acquiring` window (R07.T16.b).
 */
export const METER_TIMEOUT_S = 0.5;

/**
 * The most of one step that the meter's timers count, s: a tenth of a second, three frames at the
 * slowest rate a view is paced at (30 Hz). A stall of the page, in which no read-back can be
 * delivered, then cannot time the meter out by itself and say there is no image beside a drawn one
 * (decision-r07-t8a-meter, reason 1); a source that stops while frames run still does.
 */
export const METER_STEP_MAX_S = 0.1;

/**
 * Why a controller holds no metered value (plan R07, R07.T16.b; decision-r07-t8a-meter, item 1).
 *
 * @remarks
 * - `no-image`: no photorealistic image is drawn, or no histogram has arrived for
 *   {@link METER_TIMEOUT_S} (`NO IMAGE TO METER`).
 * - `nothing-weighed`: for that long the histograms of a drawn image weighed no pixel under
 *   `meter`, the meter in force (`NO LIT SIDE`, `NO DARK SIDE`, `STAR DISC ONLY`).
 * - `acquiring`: under that long since the image came to be drawn, or since the meter changed,
 *   with no value held: the transient before the first histogram, which gets no word.
 */
export type MeterCause =
  | { readonly kind: "no-image" }
  | { readonly kind: "nothing-weighed"; readonly meter: MeterMode }
  | { readonly kind: "acquiring" };

/** The meter as it stands: its value, EV100, which `ENABLE` takes, or why it has none. */
export type Metering = { readonly kind: "metered"; readonly ev100: number } | MeterCause;

const NO_IMAGE: MeterCause = { kind: "no-image" };
const ACQUIRING: MeterCause = { kind: "acquiring" };

/**
 * The metering as the panels show it beside the operator's chosen `meter`: a meter's own status is
 * cleared at once when another is chosen, which is `acquiring` until the controller's own window
 * ends (decision-r07-t8a-meter, item 1), so that the status never describes a meter no longer
 * chosen while the controller has yet to take the change.
 */
export function meteringFor(metering: Metering, meter: MeterMode): Metering {
  return metering.kind === "nothing-weighed" && metering.meter !== meter ? ACQUIRING : metering;
}

/**
 * The meter's status while it has no value, bare, or `null` where it has none to say: while it
 * meters, and while it is `acquiring` (decision-r07-t8a-meter, item 1).
 */
export function meterStatus(metering: Metering): string | null {
  let status: string | null;
  switch (metering.kind) {
    case "metered":
    case "acquiring":
      status = null;
      break;
    case "no-image":
      status = NO_IMAGE_TO_METER;
      break;
    case "nothing-weighed":
      status = NOTHING_WEIGHED_STATUS[metering.meter];
      break;
  }
  return status;
}

/**
 * One step of the applied EV toward the metered one, frame-rate independent: linear at
 * {@link SMOOTHING_SPEED_EV_PER_S} down to the band's edge, then exponential with the rate that
 * makes the speed continuous there (speed ÷ band), each in closed form over the step.
 *
 * @param currentEv - The applied EV100.
 * @param targetEv - The metered EV100.
 * @param dtS - The step, s, not negative.
 */
export function smoothEv(currentEv: number, targetEv: number, dtS: number): number {
  const gap = targetEv - currentEv;
  if (gap === 0 || dtS <= 0) {
    return gap === 0 ? targetEv : currentEv;
  }
  const speed = SMOOTHING_SPEED_EV_PER_S;
  const sign = Math.sign(gap);
  let remaining = Math.abs(gap);
  let time = dtS;
  if (remaining > SMOOTHING_BAND_EV) {
    const linearS = (remaining - SMOOTHING_BAND_EV) / speed;
    if (linearS >= time) {
      return currentEv + sign * speed * time;
    }
    remaining = SMOOTHING_BAND_EV;
    time -= linearS;
  }
  remaining *= Math.exp(-(speed / SMOOTHING_BAND_EV) * time);
  return targetEv - sign * remaining;
}

/** A view's exposure as R02's panel, R06's `cameraLimitV` and wireframe instrument views read it. */
export interface ExposureReading {
  /** The applied EV100. */
  readonly ev100: number;
  /** R02's triple: the view camera's program at every level, or `MAN`'s own (R07.T13.c). */
  readonly triple: ExposureTriple;
  readonly control: ExposureControl;
  readonly meter: MeterMode;
  /** The photorealistic view whose image is metered. */
  readonly source: ViewId;
}

/** What a controller starts from. */
export interface AutoExposureOptions {
  readonly source: ViewId;
  /** The view's camera, R02's `VIEW_CAMERA` where the controller is made, which sets the triple. */
  readonly program: ExposureProgram;
  readonly control: ExposureControl;
  readonly meter: MeterMode;
  /** {@link FULL_WINDOW} unless an operator's meter says otherwise. */
  readonly window?: PercentileWindow;
  /**
   * Whether it opens at the first metered value: `AUTO` at it at once, not smoothed toward it,
   * from whatever level it starts at, unless an operator's command comes first (a kept scene's
   * `startExposure`, the body inspection's; the owner's request, 2026-11-01).
   */
  readonly opensAtMeter?: boolean;
}

/**
 * One photorealistic view's exposure controller: it meters each histogram as it arrives, smooths
 * the applied EV toward it, and keeps R02's `ExposureControl` through `onMetering`.
 *
 * @remarks
 * Under `MAN` and `INHIBITED` the applied value does not move; the meter still reads, so that
 * `setAuto` and `enable` can take its value. One that opens at its meter
 * ({@link AutoExposureOptions.opensAtMeter}) takes `AUTO` at its first metered value at once. Exposure adaptation is not motion, so
 * `prefers-reduced-motion` leaves it alone. No dark adaptation of the eye is modelled.
 *
 * A cut ({@link AutoExposure.cut}) is a new scene or framing, and the exposure snaps to it, as a
 * camera's does: the next histogram that weighs a pixel while the control follows the meter sets
 * the applied value with no smoothing, and while the readings are clipped
 * ({@link readingRange}) each sets it to its bound, never back past an earlier one, until one lies
 * inside the range. A reading above the range moves the exposure at least 19.2 EV, so `MAN`'s
 * 56 EV needs at most three toward a brighter scene; one below it moves 10.74 EV, so toward a
 * darker one up to six. The frames drawn before a reading inside the range are held back
 * ({@link AutoExposure.holds}).
 *
 * Each frame the owner calls {@link AutoExposure.step} with the histogram that arrived, then draws
 * at the reading's control, then tells the controller whether that frame drew the photorealistic
 * image ({@link AutoExposure.noteImage}), so that what it says of the meter ({@link
 * AutoExposure.metering}) is true of the frame just drawn. Without a value the meter says why
 * (R07.T16.b): the status and the system inhibit are raised together, only after
 * {@link METER_TIMEOUT_S} with nothing weighed, and the window before it, `acquiring`, leaves the
 * control as it stands.
 */
export class AutoExposure {
  readonly #source: ViewId;
  readonly #program: ExposureProgram;
  readonly #window: PercentileWindow;
  #control: ExposureControl;
  #meter: MeterMode;
  #targetEv: number | null = null;
  // Since the last histogram that weighed a pixel, or since the acquiring window opened, s.
  #unweighedS = 0;
  // Since the last histogram of any kind, or since the acquiring window opened, s.
  #sinceHistogramS = 0;
  #imageDrawn = false;
  // Whether the window since the image came to be drawn, or the meter changed, with no value held
  // is still open.
  #acquiring = false;
  // Whether `AUTO` is still to be taken at the first metered value at once.
  #opensAtMeter: boolean;
  // Whether a cut waits for a reading inside the histogram's range, which ends it.
  #cut = false;
  // Whether the cut's frames are still held back (`holds`), and for how long they have been, s.
  #holding = false;
  #heldS = 0;

  constructor(options: AutoExposureOptions) {
    this.#source = options.source;
    this.#program = options.program;
    this.#window = options.window ?? FULL_WINDOW;
    this.#control = options.control;
    this.#meter = options.meter;
    this.#opensAtMeter = options.opensAtMeter === true;
  }

  /** The operator's meter. */
  get meter(): MeterMode {
    return this.#meter;
  }

  /** The control as it stands. */
  get control(): ExposureControl {
    return this.#control;
  }

  /**
   * The latest metered EV100, or `null` when there is nothing to meter: what R02's `setAuto` and
   * `enable` take.
   */
  get meteredEv100(): number | null {
    return this.#targetEv;
  }

  /** The meter as it stands: its value, or why it has none (R07.T16.b). */
  get metering(): Metering {
    let metering: Metering;
    if (this.#targetEv !== null) {
      metering = { kind: "metered", ev100: this.#targetEv };
    } else if (!this.#imageDrawn) {
      metering = NO_IMAGE;
    } else if (this.#acquiring) {
      metering = ACQUIRING;
    } else if (this.#sinceHistogramS > METER_TIMEOUT_S) {
      metering = NO_IMAGE;
    } else {
      metering = { kind: "nothing-weighed", meter: this.#meter };
    }
    return metering;
  }

  /**
   * Selects the operator's meter; the next histogram taken uses its weights. A change clears the
   * old meter's status at once and restarts the window (decision-r07-t8a-meter, item 1): with no
   * value held the meter is `acquiring` again, and a value held is kept for as long again.
   */
  setMeter(mode: MeterMode): void {
    if (mode === this.#meter) {
      return;
    }
    this.#meter = mode;
    this.#openWindow();
  }

  /**
   * Notes whether the frame just drawn drew the photorealistic image this controller meters: its
   * coming to be drawn opens the `acquiring` window where no value is held, and is a cut, the style
   * turning photorealistic; while it is not drawn the meter has no image to meter and takes no
   * histogram.
   */
  noteImage(drawn: boolean): void {
    if (drawn && !this.#imageDrawn) {
      this.#openWindow();
      this.#startCut();
    }
    this.#imageDrawn = drawn;
  }

  /**
   * Whether the view holds back the frame it draws now, presenting nothing new, so that no frame of
   * a new scene or framing is shown at the exposure from before it (the UX guide's flash rule over
   * the brief's frames at the old exposure; fix-view-light, 2026-11-02).
   *
   * @remarks
   * While the control follows the meter (`AUTO`, a system inhibit, or a view that opens at its
   * meter), it holds the image's first frame, and every frame of a cut until a reading lies inside
   * the histogram's range: the frame is drawn into the scene target, which the histogram reads, but
   * not presented, so the view shows the image it last showed. A cut toward a brighter scene costs
   * the read-back's one to three frames, a clipped one up to {@link CUT_HOLD_MAX_S}, past which the
   * frame is shown as it is: so too a cut whose readings weigh nothing (`NO LIT SIDE`), since a
   * new framing's first reading may weigh nothing before its bodies are drawn. `MAN`
   * the operator has set and the operator's `INHIBIT` hold nothing; the display's default `MAN`
   * holds where the view opens at its meter.
   */
  get holds(): boolean {
    const control = this.#control;
    const follows =
      this.#opensAtMeter ||
      control.kind === "auto" ||
      (control.kind === "inhibited" && control.reason !== "operator");
    return follows && (this.#holding || !this.#imageDrawn) && this.#heldS < CUT_HOLD_MAX_S;
  }

  /** Begins a cut: the next readings snap, and its frames are held from the first. */
  #startCut(): void {
    this.#cut = true;
    this.#holding = true;
    this.#heldS = 0;
  }

  /**
   * Marks a cut: a new scene, or a camera change that is not an eased move. The exposure snaps to
   * the next histogram that weighs a pixel while the control follows the meter, and to each one
   * after it until a reading lies inside the histogram's range (the class's remarks).
   */
  cut(): void {
    this.#startCut();
  }

  /**
   * Takes an operator's command's outcome from R02's `setManual`, `setAuto`, `inhibit` or
   * `enable`; one that puts the exposure under `AUTO` from another level is a cut, so that it
   * snaps to the meter.
   *
   * @returns Whether it was accepted.
   */
  apply(result: ExposureCommandResult): boolean {
    if (result.kind === "refused") {
      return false;
    }
    if (result.control.kind === "auto" && this.#control.kind !== "auto") {
      this.#startCut();
    }
    this.#control = result.control;
    // The operator's command decides the exposure from here.
    this.#opensAtMeter = false;
    return true;
  }

  /**
   * Advances one frame.
   *
   * @param h - The histogram that arrived since the last step, if one did; one taken of an image
   *   no longer drawn is not metered.
   * @param dtS - The frame's duration, s.
   */
  step(h: Histogram | undefined, dtS: number): ExposureReading {
    if (this.#imageDrawn && this.holds) {
      this.#heldS += Math.min(dtS, METER_STEP_MAX_S);
    }
    const histogram = this.#imageDrawn ? h : undefined;
    const average = histogram === undefined ? null : meteredAverage(histogram, this.#window);
    // The range of this step's reading where it weighs a pixel: a frame below the range metered at
    // EMPTY_FRAME_CD_M2 is at the value the exposure stops at, as good as one inside it.
    const range =
      histogram === undefined || average === null
        ? null
        : average === EMPTY_FRAME_CD_M2
          ? "inside"
          : readingRange(histogram);
    if (average === null) {
      // No histogram, or one with no pixel of a class the meter weighs (`LIT` with no lit body):
      // the meter holds its value until the timeout, then says why it has none.
      const countedS = Math.min(dtS, METER_STEP_MAX_S);
      this.#unweighedS += countedS;
      this.#sinceHistogramS = histogram === undefined ? this.#sinceHistogramS + countedS : 0;
      if (this.#unweighedS > METER_TIMEOUT_S) {
        this.#targetEv = null;
        this.#acquiring = false;
      }
    } else {
      this.#unweighedS = 0;
      this.#sinceHistogramS = 0;
      this.#acquiring = false;
      this.#targetEv = ev100FromAverageLuminance(average);
    }
    const metering = this.metering;
    const control = this.#control;
    if (this.#opensAtMeter && metering.kind === "metered") {
      // Opened at the first metered value, as `ENABLE` sets it, not walked to it: a cut's first
      // reading, so that a clipped one is a bound the next readings snap past, and one inside the
      // range ends the cut, AUTO smoothing from there.
      this.#opensAtMeter = false;
      this.#cut = true;
      this.#control = {
        kind: "auto",
        ev100:
          range === null ? metering.ev100 : this.#snapped(metering.ev100, metering.ev100, range),
      };
    } else if (control.kind !== "manual") {
      // `AUTO` follows the value, and a system inhibit resumes from where it was held and smooths
      // from there, or takes the new cause; an operator's inhibit is untouched (R02's
      // `onMetering`). While acquiring, the control stands as it is.
      switch (metering.kind) {
        case "metered":
          this.#control = onMetering(
            control,
            this.#cut && range !== null
              ? this.#snapped(control.ev100, metering.ev100, range)
              : smoothEv(control.ev100, metering.ev100, dtS),
          );
          break;
        case "acquiring":
          break;
        case "no-image":
          this.#control = onMetering(control, NO_IMAGE_INHIBIT);
          break;
        case "nothing-weighed":
          this.#control = onMetering(control, {
            reason: "nothing_weighed",
            meter: metering.meter,
          });
          break;
      }
    }
    return this.reading();
  }

  /**
   * The applied EV100 a cut takes from a reading: its value where it lies inside the range, which
   * ends the cut; else its bound, never back past the applied value, since a reading taken before
   * the last snap is a looser bound of the same side.
   */
  #snapped(appliedEv: number, meteredEv: number, range: ReadingRange): number {
    let ev: number;
    switch (range) {
      case "inside":
        this.#cut = false;
        this.#holding = false;
        ev = meteredEv;
        break;
      case "above":
        ev = Math.max(appliedEv, meteredEv);
        break;
      case "below":
        ev = Math.min(appliedEv, meteredEv);
        break;
    }
    return ev;
  }

  /** Opens the window in which the meter, with no value held, is `acquiring`. */
  #openWindow(): void {
    this.#unweighedS = 0;
    this.#sinceHistogramS = 0;
    this.#acquiring = this.#targetEv === null;
  }

  /** The reading as it stands, without advancing. */
  reading(): ExposureReading {
    const ev100 = controlEv100(this.#control);
    const triple =
      this.#control.kind === "manual" ? this.#control.triple : programTriple(this.#program, ev100);
    return { ev100, triple, control: this.#control, meter: this.#meter, source: this.#source };
  }
}
