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
  onMetering,
  type ExposureCommandResult,
  type ExposureControl,
  type ExposureTriple,
} from "../photometry/exposure";
import { binCentreLuminance, HISTOGRAM_BINS, type Histogram } from "./histogram";
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
 * How long without a histogram before `AUTO` reports no image to meter, s: well above the reader's
 * one to three frames, so a dropped frame does not inhibit it.
 */
export const METER_TIMEOUT_S = 0.5;

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

/**
 * The `AUTO` program's fixed aperture and shutter, from R06's `DEFAULT_VIEW_CAMERA` (N = 1.4,
 * t = 1/30 s), passed in where the controller is made; the sensitivity is solved (Design note 11).
 */
export interface ExposureProgram {
  readonly aperture: number;
  readonly shutterS: number;
}

/** A view's exposure as R02's panel, R06's `cameraLimitV` and wireframe instrument views read it. */
export interface ExposureReading {
  /** The applied EV100. */
  readonly ev100: number;
  /** R02's triple; under `AUTO` and `INHIBITED` from the sensitivity-priority program. */
  readonly triple: ExposureTriple;
  readonly control: ExposureControl;
  readonly meter: MeterMode;
  /** The photorealistic view whose image is metered. */
  readonly source: ViewId;
}

/**
 * The triple of `ev100` under a sensitivity-priority program: N and t fixed, S = 100 × (N² ÷ t) ×
 * 2^−EV100, from R02's EV100 = log₂(N² ÷ t) − log₂(S ÷ 100).
 */
export function programTriple(program: ExposureProgram, ev100: number): ExposureTriple {
  const { aperture, shutterS } = program;
  return { aperture, shutterS, iso: 100 * ((aperture * aperture) / shutterS) * 2 ** -ev100 };
}

/** What a controller starts from. */
export interface AutoExposureOptions {
  readonly source: ViewId;
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
 */
export class AutoExposure {
  readonly #source: ViewId;
  readonly #program: ExposureProgram;
  readonly #window: PercentileWindow;
  #control: ExposureControl;
  #meter: MeterMode;
  #targetEv: number | null = null;
  #sinceHistogramS = 0;

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

  /** Selects the operator's meter; the next histogram taken uses its weights. */
  setMeter(mode: MeterMode): void {
    this.#meter = mode;
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
   * @param h - The histogram that arrived since the last step, if one did.
   * @param dtS - The frame's duration, s.
   */
  step(h: Histogram | undefined, dtS: number): ExposureReading {
    if (h === undefined) {
      this.#sinceHistogramS += dtS;
      if (this.#sinceHistogramS > METER_TIMEOUT_S) {
        this.#targetEv = null;
      }
    } else {
      this.#sinceHistogramS = 0;
      const average = meteredLuminance(h, this.#window);
      this.#targetEv = average > 0 ? ev100FromAverageLuminance(average) : null;
    }
    const target = this.#targetEv;
    const control = this.#control;
    switch (control.kind) {
      case "auto":
        // A lost source inhibits `AUTO` at its last value (R02's system inhibit).
        this.#control = onMetering(
          control,
          target === null ? null : smoothEv(control.ev100, target, dtS),
        );
        break;
      case "inhibited":
        // A system inhibit resumes `AUTO` from where it was held, and smooths from there; an
        // operator's is untouched.
        this.#control = onMetering(control, target === null ? null : control.ev100);
        break;
      case "manual":
        break;
    }
    return this.reading();
  }

  /** The reading as it stands, without advancing. */
  reading(): ExposureReading {
    const ev100 = controlEv100(this.#control);
    const triple =
      this.#control.kind === "manual" ? this.#control.triple : programTriple(this.#program, ev100);
    return { ev100, triple, control: this.#control, meter: this.#meter, source: this.#source };
  }
}
