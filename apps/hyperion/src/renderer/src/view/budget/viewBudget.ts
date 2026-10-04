/**
 * The several views' budgets (plan R07, T18; Design note 14): a pure policy from the views `VIEW`
 * shows and the quality setting to each view's scale, rate, style and streaming priority.
 *
 * @remarks
 * The primary view (the work area's full-window view) is budgeted at the setting's photorealistic
 * rate where it is photorealistic (60 Hz, or 30 Hz on the low setting) and at 60 Hz in the
 * wireframe, and renders at its internal scale. Each instrument view (a slot over its right edge,
 * Design note 15) renders at 30 Hz at its full scale, so that a wireframe's strokes stay sharp, and
 * streams terrain at R05's secondary priority. On the low setting at most one view is
 * photorealistic: the first that asks keeps it, the primary before the instruments, and the others
 * draw the wireframe, their style controls held back with {@link ONE_PHOTOREALISTIC_VIEW}. The
 * policy keeps no state, so a switch to the photorealistic style (its control or its key) asks
 * {@link photorealisticAllowed} first. While instruments are open a photorealistic primary's scale
 * is the {@link ResolutionController}'s, between
 * `ViewSettings.internalScaleBounds`, holding the frame's GPU time, every view's included, to
 * {@link GPU_FRAME_SHARE} of the period less {@link PER_CANVAS_OVERHEAD_MS} for each instrument's
 * canvas; the instruments' cost so comes out of the primary's margin. With no instrument open the
 * primary renders at the bounds' max. A wireframe view draws straight to its canvas (R02 Design
 * note 12) and has no internal scale: it renders at 1. The outputs are data that R12's runs
 * record.
 */

import type { RenderStyle, ViewId } from "../camera/state";
import { type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import type { ResolutionTarget, ScaleBounds } from "./resolutionController";

/** Where a view sits in `VIEW`: the work area's full-window view, or an instrument slot. */
export type ViewSlot = "primary" | "instrument";

/** A view `VIEW` shows (the primary, or an open instrument), as the budget reads it. */
export interface ViewSpec {
  readonly id: ViewId;
  readonly slot: ViewSlot;
  /** The style the view's camera asks for (`CameraState.style`). */
  readonly style: RenderStyle;
}

/** A view whose internal scale the resolution controller holds: its bounds and target. */
export interface ScaleControl {
  readonly bounds: ScaleBounds;
  readonly target: ResolutionTarget;
}

/** What one view may spend. */
export interface ViewBudget {
  /**
   * The internal scale, a fraction of the view's render resolution on each axis (the canvas's
   * size, or the setting's `terrain.renderHeightPx` rows): for a controlled view the bounds' max,
   * from which its controller starts.
   */
  readonly renderScale: number;
  /** The rate the view renders at. */
  readonly rateHz: 60 | 30;
  /** The style it draws: the one it asks for, unless the setting allows no more photorealistic. */
  readonly style: RenderStyle;
  /** Its terrain streaming priority, R05's primary or secondary weight. */
  readonly streamPriority: "primary" | "secondary";
  /** Where its scale is the resolution controller's, the controller's bounds and target. */
  readonly control: ScaleControl | null;
}

/**
 * The GPU cost of presenting one more canvas, ms (Design note 14): provisional, since two extra
 * small canvases cost below the probe's resolution; R07.T20 measures it.
 */
export const PER_CANVAS_OVERHEAD_MS = 0.3;

/**
 * The share of the frame period the GPU's passes may take: R05 Design note 21's headroom row, the
 * GPU pass sum within 0.8 T at the 95th percentile.
 */
export const GPU_FRAME_SHARE = 0.8;

/** The rate of every instrument view (Design note 14: secondary views at 30 Hz). */
export const INSTRUMENT_RATE_HZ = 30;

/**
 * The rate of a wireframe primary view on both settings: the brainstorm's budget gives a wireframe
 * view its own 16.7 ms frame even on the UHD 620 (Performance budget, the station wireframe row).
 */
export const WIREFRAME_PRIMARY_RATE_HZ = 60;

/** Why a view's photorealistic style is held back on the low setting (the guide's row, T16). */
export const ONE_PHOTOREALISTIC_VIEW = "ONE PHOTOREALISTIC VIEW ON LOW SETTING";

/** Whether a view may be photorealistic, and the reason where it may not. */
export type PhotorealisticPermission =
  | { readonly allowed: true }
  | { readonly allowed: false; readonly reason: typeof ONE_PHOTOREALISTIC_VIEW };

/**
 * The GPU time a frame's passes may take, ms: {@link GPU_FRAME_SHARE} of the period at the rate,
 * less {@link PER_CANVAS_OVERHEAD_MS} for each canvas presented beside the primary's.
 */
export function frameGpuBudgetMs(rateHz: 60 | 30, extraCanvases: number): number {
  return (GPU_FRAME_SHARE * 1000) / rateHz - extraCanvases * PER_CANVAS_OVERHEAD_MS;
}

/**
 * Each view's budget, by its identity.
 *
 * @param views - The views shown: one primary, and the open instruments in their slots' order,
 *   whether or not an instrument at 30 Hz draws in a given frame. Instruments are open exactly
 *   when some are listed.
 * @throws RangeError unless there is exactly one primary view and every identity is distinct.
 */
export function viewBudgets(
  views: ReadonlyArray<ViewSpec>,
  setting: QualitySetting,
): ReadonlyMap<ViewId, ViewBudget> {
  const settings = SETTINGS[setting];
  const bounds = settings.internalScaleBounds;
  const styles = drawnStyles(views, setting);
  const instruments = views.filter((view) => view.slot === "instrument").length;
  const budgets = new Map<ViewId, ViewBudget>();
  for (const view of views) {
    const style = styles.get(view.id);
    if (style === undefined) {
      throw new Error(`drawnStyles gave view ${view.id} no style`);
    }
    const photorealistic = style === "photorealistic";
    const renderScale = photorealistic ? bounds[1] : 1;
    let budget: ViewBudget;
    switch (view.slot) {
      case "primary": {
        const rateHz = photorealistic
          ? settings.budget.photorealisticRateHz
          : WIREFRAME_PRIMARY_RATE_HZ;
        budget = {
          renderScale,
          rateHz,
          style,
          streamPriority: "primary",
          control:
            photorealistic && instruments > 0
              ? {
                  bounds,
                  target: {
                    periodMs: 1000 / rateHz,
                    gpuBudgetMs: frameGpuBudgetMs(rateHz, instruments),
                  },
                }
              : null,
        };
        break;
      }
      case "instrument":
        budget = {
          renderScale,
          rateHz: INSTRUMENT_RATE_HZ,
          style,
          streamPriority: "secondary",
          control: null,
        };
        break;
    }
    budgets.set(view.id, budget);
  }
  return budgets;
}

/**
 * Whether `candidate` may draw the photorealistic style beside the other views: on a setting that
 * limits photorealistic views, only while fewer others draw it. The adapter's own refusal is
 * `styleRefusal`'s, not this.
 *
 * @throws RangeError as {@link viewBudgets} does, or where `candidate` is not among the views.
 */
export function photorealisticAllowed(
  views: ReadonlyArray<ViewSpec>,
  setting: QualitySetting,
  candidate: ViewId,
): PhotorealisticPermission {
  const styles = drawnStyles(views, setting);
  if (!styles.has(candidate)) {
    throw new RangeError(`view ${candidate} is not among the views`);
  }
  const limit = SETTINGS[setting].budget.photorealisticViews;
  if (limit === null) {
    return { allowed: true };
  }
  let others = 0;
  for (const [id, style] of styles) {
    if (id !== candidate && style === "photorealistic") {
      others += 1;
    }
  }
  return others < limit ? { allowed: true } : { allowed: false, reason: ONE_PHOTOREALISTIC_VIEW };
}

/**
 * The style each view draws: the photorealistic style goes to those asking for it, the primary
 * first and then the instruments in order, while the setting allows more.
 */
function drawnStyles(
  views: ReadonlyArray<ViewSpec>,
  setting: QualitySetting,
): Map<ViewId, RenderStyle> {
  const primaries = views.filter((view) => view.slot === "primary");
  if (primaries.length !== 1) {
    throw new RangeError(`VIEW draws exactly one primary view, not ${primaries.length}`);
  }
  const limit = SETTINGS[setting].budget.photorealisticViews;
  const ordered = [...primaries, ...views.filter((view) => view.slot === "instrument")];
  const styles = new Map<ViewId, RenderStyle>();
  let photorealistic = 0;
  for (const view of ordered) {
    if (styles.has(view.id)) {
      throw new RangeError(`view ${view.id} is listed twice`);
    }
    const granted = view.style === "photorealistic" && (limit === null || photorealistic < limit);
    if (granted) {
      photorealistic += 1;
    }
    styles.set(view.id, granted ? "photorealistic" : "wireframe");
  }
  return styles;
}
