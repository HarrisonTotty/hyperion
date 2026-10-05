/**
 * Why a view's style is held back, from the graphics' condition and the photorealistic view's own
 * standing (plan R07, T8.a): the reason the style control shows, in the guide's words, or `null`
 * where the style is offered.
 */
import type { PhotorealisticPermission } from "../../view/budget/viewBudget";
import type { RenderStyle } from "../../view/camera/state";
import type { StyleAvailability } from "../../view/engine/platform";
import { type GraphicsStatus, graphicsAnnunciation } from "../../view/engine/status";
import type { PhotorealStatus } from "../../view/photoreal/renderer";
import { styleRefusal } from "../../view/photoreal/style";

/** Each style's refusal, `null` where it is offered. */
export type StyleRefusals = Readonly<Record<RenderStyle, string | null>>;

/**
 * The refusal where the photorealistic view's pipelines could not be made: a fault of the
 * console's own graphics, shown in `--status-caution` while it lasts (drafted for the owner).
 */
export const PHOTOREAL_NOT_CREATED =
  "GRAPHICS STYLE REFUSED: photorealistic style not created, relaunch to retry";

/**
 * Why both styles are held back while no view can be drawn: the adapter being acquired, the views
 * not made, a graphics fault. The style panel stands then too, so that the panels under it never
 * move when the adapter answers; the stage states the cause itself (decision-r07-t19b-exposure-fit,
 * item 2; the guide's `NOT AVAILABLE` form, decision-r07-t18 item 6).
 */
export const NO_VIEW_DRAWN = "NOT AVAILABLE: no view is drawn";

/** Both styles' refusals while no view can be drawn ({@link NO_VIEW_DRAWN}). */
export const NO_VIEW_REFUSALS: StyleRefusals = {
  wireframe: NO_VIEW_DRAWN,
  photorealistic: NO_VIEW_DRAWN,
};

/** The reason a state without a drawable view gives: the graphics' own annunciation. */
const NO_VIEWS = "GRAPHICS NOT AVAILABLE";

/**
 * The styles' refusals: the adapter's styles once it has answered (R01's `styleAvailability`), the
 * software adapter's refusal of the photorealistic style, `GRAPHICS ACQUIRING ADAPTER` before the
 * answer, the graphics' annunciation for both styles where no view can be drawn, and
 * {@link PHOTOREAL_NOT_CREATED} where the photorealistic view failed to make its pipelines.
 */
export function styleRefusals(status: GraphicsStatus, photoreal: PhotorealStatus): StyleRefusals {
  const { condition } = status;
  let refusals: StyleRefusals;
  switch (condition.kind) {
    case "nominal":
      refusals = {
        wireframe: styleRefusal("wireframe", condition.styles),
        photorealistic:
          photoreal === "failed"
            ? PHOTOREAL_NOT_CREATED
            : styleRefusal("photorealistic", condition.styles),
      };
      break;
    case "software-adapter":
      refusals = {
        wireframe: null,
        photorealistic: styleRefusal("photorealistic", { wireframe: true, photorealistic: false }),
      };
      break;
    case "acquiring":
      refusals = { wireframe: null, photorealistic: "GRAPHICS ACQUIRING ADAPTER" };
      break;
    case "no-webgpu":
    case "no-adapter":
    case "safe-mode":
    case "disabled": {
      const reason = graphicsAnnunciation(status)?.text ?? NO_VIEWS;
      refusals = { wireframe: reason, photorealistic: reason };
      break;
    }
  }
  return refusals;
}

/** The styles a view may switch to: those with no refusal. */
export function availabilityOf(refusals: StyleRefusals): StyleAvailability {
  return {
    wireframe: refusals.wireframe === null,
    photorealistic: refusals.photorealistic === null,
  };
}

/**
 * The refusals with the several views' budget's (plan R07, T19): the photorealistic style held
 * back with `photorealisticAllowed`'s reason where the setting allows no more photorealistic views,
 * the adapter's own refusal shown first where both hold.
 */
export function withPermission(
  refusals: StyleRefusals,
  permission: PhotorealisticPermission,
): StyleRefusals {
  return {
    ...refusals,
    photorealistic: refusals.photorealistic ?? (permission.allowed ? null : permission.reason),
  };
}
