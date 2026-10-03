/**
 * The style switch of a view (plan R07, T7; Design note 8): which styles the adapter offers, the
 * reason one is refused, and the switch itself, which changes the pass list and nothing else.
 */
import type { CameraState, RenderStyle } from "../camera/state";
import type { StyleAvailability } from "../engine/platform";

/** The styles in the control's order. */
export const RENDER_STYLES: ReadonlyArray<RenderStyle> = ["wireframe", "photorealistic"];

/** A style's name, as the label block, the canvas's accessible name and the control show it. */
export function styleName(style: RenderStyle): string {
  let name: string;
  switch (style) {
    case "wireframe":
      name = "WIREFRAME";
      break;
    case "photorealistic":
      name = "PHOTOREALISTIC";
      break;
  }
  return name;
}

/**
 * The view's single key that toggles its style: `4`, beside the presets' `1` to `3` (digits and
 * brackets being the view's single keys, letters the flight keys). Provisional (the orchestrator's
 * ruling, 2026-10-03): checked against the guide's key rules and the keymaps before T8.a mounts it.
 */
export const STYLE_TOGGLE_KEY = "4";

/**
 * Why a style is refused on this adapter, as the guide words it, or `null` where it is offered.
 *
 * @remarks
 * R01's `styleAvailability` refuses the photorealistic style on a fallback (software) adapter; the
 * reason is the guide's `GRAPHICS SOFTWARE ADAPTER` cause (`photorealistic style not available`).
 */
export function styleRefusal(style: RenderStyle, availability: StyleAvailability): string | null {
  let offered: boolean;
  switch (style) {
    case "wireframe":
      offered = availability.wireframe;
      break;
    case "photorealistic":
      offered = availability.photorealistic;
      break;
  }
  if (offered) {
    return null;
  }
  return style === "photorealistic"
    ? "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available"
    : "NOT AVAILABLE";
}

/** The other style, the one the toggle key selects. */
export function otherStyle(style: RenderStyle): RenderStyle {
  return style === "wireframe" ? "photorealistic" : "wireframe";
}

/**
 * The camera with another style: a style owns no scene, camera or projection, so only the style
 * changes; a refused style leaves the camera as it was.
 */
export function withStyle(
  camera: CameraState,
  style: RenderStyle,
  availability: StyleAvailability,
): CameraState {
  if (styleRefusal(style, availability) !== null || camera.style === style) {
    return camera;
  }
  return { ...camera, style };
}
