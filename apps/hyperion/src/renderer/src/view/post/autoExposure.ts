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
 * (Design note 11). The veil is never in it: the histogram is taken before the glare is drawn.
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
 * Smoothing speeds, EV/s: Unreal's `FCameraExposureSettings` defaults, Speed Up 3 and Speed Down 1
 * f-stops per second (UE 4.26 Python API, `CameraExposureSettings`), linear while the metered value
 * is more than {@link SMOOTHING_BAND_EV} away and exponential inside the band with a continuous
 * first derivative (UE 4.27, "Auto Exposure", `r.EyeAdaptation.ExponentialTransitionDistance`);
 * to be settled by eye (plan R07, Risks).
 */
export const SMOOTHING_SPEED_EV_PER_S = { brighten: 3, darken: 1 } as const;

/** The half-width of the band, EV, inside which the approach is exponential: Unreal's default 1.5. */
export const SMOOTHING_BAND_EV = 1.5;

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
 * One step of the applied EV toward the metered one, frame-rate independent: linear at the
 * direction's speed down to the band's edge, then exponential with the rate that makes the speed
 * continuous there (speed ÷ band), each in closed form over the step.
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
  // A higher EV100 is a brighter scene.
  const speed = gap > 0 ? SMOOTHING_SPEED_EV_PER_S.brighten : SMOOTHING_SPEED_EV_PER_S.darken;
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
}

/**
 * One photorealistic view's exposure controller: it meters each histogram as it arrives, smooths
 * the applied EV toward it, and keeps R02's `ExposureControl` through `onMetering`.
 *
 * @remarks
 * Under `MAN` and `INHIBITED` the applied value does not move; the meter still reads, so that
 * `setAuto` and `enable` can take its value. Exposure adaptation is not motion, so
 * `prefers-reduced-motion` leaves it alone. No dark adaptation of the eye is modelled.
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

  constructor(options: AutoExposureOptions) {
    this.#source = options.source;
    this.#program = options.program;
    this.#window = options.window ?? FULL_WINDOW;
    this.#control = options.control;
    this.#meter = options.meter;
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
   * coming to be drawn with no value held opens the `acquiring` window, and while it is not drawn
   * the meter has no image to meter and takes no histogram.
   */
  noteImage(drawn: boolean): void {
    if (drawn && !this.#imageDrawn) {
      this.#openWindow();
    }
    this.#imageDrawn = drawn;
  }

  /**
   * Takes an operator's command's outcome from R02's `setManual`, `setAuto`, `inhibit` or
   * `enable`.
   *
   * @returns Whether it was accepted.
   */
  apply(result: ExposureCommandResult): boolean {
    if (result.kind === "refused") {
      return false;
    }
    this.#control = result.control;
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
    const histogram = this.#imageDrawn ? h : undefined;
    const average = histogram === undefined ? null : meteredAverage(histogram, this.#window);
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
    if (control.kind !== "manual") {
      // `AUTO` follows the value, and a system inhibit resumes from where it was held and smooths
      // from there, or takes the new cause; an operator's inhibit is untouched (R02's
      // `onMetering`). While acquiring, the control stands as it is.
      switch (metering.kind) {
        case "metered":
          this.#control = onMetering(control, smoothEv(control.ev100, metering.ev100, dtS));
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
