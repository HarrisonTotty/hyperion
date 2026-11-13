/**
 * The scale the lit terrain counts a rendered pixel at (perf-terrain-selection-scale, the owner's
 * Mac; `GeometrySettings.selectionScale` `rendered`): the internal scale snapped to the floor's
 * steps and held, and the factor its τ takes from it.
 *
 * @remarks
 * Under its frame-rate floor (or beside open instruments) a photorealistic primary's internal scale
 * is the resolution controller's, which moves a few percent at a time. A selection that followed it
 * frame by frame would reselect the terrain at every move, and a coarser selection's cheaper frames
 * would let the controller raise the scale it was made at: the two would chase each other. So the
 * selection takes the scale snapped up to the floor's steps ({@link selectionScaleStep}), the bounds'
 * max over 2^(i ÷ {@link SELECTION_SCALE_STEPS_PER_OCTAVE}) down to the bounds' min, its lowest step,
 * and takes a new step only once the controller's scale has stood on it for
 * {@link SELECTION_SCALE_HOLD_FRAMES} frames in a row ({@link SelectionScaleHold}). Snapped up, the
 * selection is never coarser than the pixel the frame is rendered at, except while a rise of the
 * scale waits out its hold.
 */

import type { Viewport } from "../camera/projection";
import type { ScaleBounds } from "./resolutionController";

/**
 * The floor's steps an octave of scale (a lean, 2026-11-10): 2^(1/8), about 9% of each axis, a
 * fifth of the lit triangles a step, so that a step reselects noticeably less than the controller's
 * moves while the selection stays within a step of the rendered pixel.
 */
export const SELECTION_SCALE_STEPS_PER_OCTAVE = 8;

/**
 * The primary's frames a new step must stand for before the selection takes it (a lean,
 * 2026-11-10): the resolution controller's first raise run (`CONTROLLER_TUNING.raiseHoldFrames`,
 * 30), five times its settle after a change from GPU time, so that the controller judges each scale
 * at the selection made before it, and a held step's reselection lands after the controller's own
 * change has settled.
 */
export const SELECTION_SCALE_HOLD_FRAMES = 30;

/**
 * The step of the floor's steps at or above `scale`: the least of `bounds`' max × 2^(−i ÷
 * {@link SELECTION_SCALE_STEPS_PER_OCTAVE}), whole i ≥ 0, and `bounds`' min, that is at least
 * `scale`; the max for a scale at or above it, the min for one at or below it.
 *
 * @param scale - The controller's scale, a fraction of the render resolution on each axis.
 */
export function selectionScaleStep(scale: number, bounds: ScaleBounds): number {
  const [min, max] = bounds;
  if (!(scale < max)) {
    return max;
  }
  if (!(scale > min)) {
    return min;
  }
  const i = Math.floor(SELECTION_SCALE_STEPS_PER_OCTAVE * Math.log2(max / scale));
  return Math.max(min, max * 2 ** (-i / SELECTION_SCALE_STEPS_PER_OCTAVE));
}

/**
 * A view's selection scale, held: a new step is taken once it has stood for
 * {@link SELECTION_SCALE_HOLD_FRAMES} updates in a row, the first at once.
 *
 * @example
 * ```ts
 * const hold = new SelectionScaleHold();
 * // Once a primary frame, with the controller's scale for it:
 * const selectionScale = hold.update(selectionScaleStep(scale, control.bounds));
 * ```
 */
export class SelectionScaleHold {
  #held: number | null = null;
  #candidate: number | null = null;
  #frames = 0;

  /** The step the selection holds, or `null` before the first update. */
  get held(): number | null {
    return this.#held;
  }

  /**
   * Takes one frame's step ({@link selectionScaleStep}) and returns the one the selection holds.
   */
  update(step: number): number {
    if (this.#held === null) {
      this.#held = step;
      return step;
    }
    if (step === this.#held) {
      this.#candidate = null;
      this.#frames = 0;
      return this.#held;
    }
    if (step === this.#candidate) {
      this.#frames += 1;
    } else {
      this.#candidate = step;
      this.#frames = 1;
    }
    if (this.#frames >= SELECTION_SCALE_HOLD_FRAMES) {
      this.#held = step;
      this.#candidate = null;
      this.#frames = 0;
    }
    return this.#held;
  }

  /** Forgets the held step: the next update's is taken at once. */
  reset(): void {
    this.#held = null;
    this.#candidate = null;
    this.#frames = 0;
  }
}

/**
 * The factor a view's lit τ takes from its rendered pixel (`GeometrySettings.selectionScale`
 * `rendered`), at least 1: its selection size's width (`selectionViewport`, CSS px) over the width
 * its photorealistic image is rendered at (`internalViewport` of its render resolution at the held
 * scale), so that τ CSS px times it is τ of the larger of a CSS and a rendered pixel.
 *
 * @param selectionPx - The view's selection size, CSS px.
 * @param renderedPx - The size its image is rendered at, device px, at least one pixel wide.
 */
export function renderedTauScale(selectionPx: Viewport, renderedPx: Viewport): number {
  return Math.max(1, selectionPx.widthPx / renderedPx.widthPx);
}
