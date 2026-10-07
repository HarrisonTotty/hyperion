/**
 * A photorealistic view's internal resolution (plan R07, T19; Design note 14): its scene target is
 * made at its render resolution (the canvas's, or the setting's rows, T17) times its scale, and the
 * tone-mapping pass presents it upscaled onto the canvas (T15).
 *
 * @remarks
 * Everything drawn into the scene target is drawn at the internal viewport: R06's sky and host
 * discs, the lit bodies, and the stars. The stars come from the wireframe's draw list, built at the
 * canvas's viewport for the symbology over the image, so {@link spritesAtScale} carries them to the
 * internal one. The internal viewport keeps the canvas's field of view; its height follows its
 * width's factor, so a point projects to the canvas's position times that factor about the
 * centre, within half an internal pixel of rounding at the edges once upscaled.
 */
import type { Viewport } from "../camera/projection";
import type { StarSprite } from "../wireframe/drawList";

/**
 * A view's render resolution: its canvas's viewport, or, where the setting renders fewer rows than
 * the canvas has, `renderHeightPx` rows at the canvas's aspect, presented upscaled by the
 * tone-mapping pass (R07 Design note 18: 720p on the low setting; R07.T17).
 *
 * @remarks
 * R05's `renderSizeOf` gives the descent spike's lit view its size by it, from the same setting.
 *
 * @param renderHeightPx - The setting's `terrain.renderHeightPx`, device px, or `null` for the
 *   canvas's own size.
 */
export function renderViewport(viewport: Viewport, renderHeightPx: number | null): Viewport {
  if (renderHeightPx === null || renderHeightPx >= viewport.heightPx) {
    return viewport;
  }
  return {
    widthPx: Math.max(1, Math.round((viewport.widthPx * renderHeightPx) / viewport.heightPx)),
    heightPx: renderHeightPx,
  };
}

/**
 * The viewport the scene target is drawn at: the render resolution ({@link renderViewport}) times
 * `scale` on each axis, at least one pixel; the render resolution itself at a scale of 1.
 *
 * @param viewport - The render resolution.
 * @param scale - The budget's or the resolution controller's scale, in (0, 1].
 */
export function internalViewport(viewport: Viewport, scale: number): Viewport {
  if (scale === 1) {
    return viewport;
  }
  const widthPx = Math.max(1, Math.round(viewport.widthPx * scale));
  const factor = widthPx / viewport.widthPx;
  return { widthPx, heightPx: Math.max(1, Math.round(viewport.heightPx * factor)) };
}

/**
 * The draw list's star sprites placed for the internal viewport: each position moved about the
 * centre by the width's factor k, and its light per unit of point-spread weight scaled by k², the
 * ratio of the two pixels' solid angles, so that each star's total is kept.
 */
export function spritesAtScale(
  sprites: ReadonlyArray<StarSprite>,
  from: Viewport,
  to: Viewport,
): ReadonlyArray<StarSprite> {
  if (from.widthPx === to.widthPx && from.heightPx === to.heightPx) {
    return sprites;
  }
  const k = to.widthPx / from.widthPx;
  const light = k * k;
  return sprites.map((sprite) => ({
    ...sprite,
    xPx: sprite.xPx * k,
    yPx: to.heightPx / 2 + (sprite.yPx - from.heightPx / 2) * k,
    exposedRgb: [
      sprite.exposedRgb[0] * light,
      sprite.exposedRgb[1] * light,
      sprite.exposedRgb[2] * light,
    ],
  }));
}
