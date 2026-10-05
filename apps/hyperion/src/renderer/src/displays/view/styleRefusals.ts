/**
 * Why a view's style is held back, from the graphics' condition and the photorealistic view's own
 * standing (plan R07, T8.a): the reason the style control shows, in the guide's words, or `null`
 * where the style is offered.
 */
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
