/**
 * The smoke page's photorealistic-style checks (plan R07, T7): a view's HDR scene target, made by
 * `createSceneTarget`, renders an empty photorealistic frame whose texels are finite, black and
 * of alpha `METER_CLASS.other`, then resizes and is disposed.
 */

import type { RenderEngine, RenderTarget } from "../view/engine/types";
import { createSceneTarget } from "../view/photoreal/sceneTarget";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { METER_CLASS } from "../view/post/meter";
import { type Checks, frameOf, halfTexels } from "./harness";

/** The scene target's size in the check, px: the display's 16 : 9 at a small size. */
const SIZE = { widthPx: 64, heightPx: 36 } as const;

/** T7: an empty photorealistic frame. */
export async function checkPhotoreal(engine: RenderEngine, checks: Checks): Promise<void> {
  const target = createSceneTarget(engine, "smoke view", SIZE);
  try {
    await checkFrames(engine, checks, target);
  } finally {
    target.dispose();
  }
}

/** The empty frame and the resize, on a scene target the caller disposes of. */
async function checkFrames(
  engine: RenderEngine,
  checks: Checks,
  target: RenderTarget,
): Promise<void> {
  target.render(frameOf(PHOTOREAL_PASS_LABELS.bodies, [], SIZE.widthPx / SIZE.heightPx));
  const texels = halfTexels(await engine.readTexture(target.colour));
  let finite = true;
  let black = true;
  let metered = true;
  for (let i = 0; i < texels.length; i += 4) {
    const [r = Number.NaN, g = Number.NaN, b = Number.NaN, a = Number.NaN] = texels.subarray(
      i,
      i + 4,
    );
    finite &&= [r, g, b, a].every(Number.isFinite);
    black &&= r === 0 && g === 0 && b === 0;
    metered &&= a === METER_CLASS.other;
  }
  checks.check(
    "T7 an empty photorealistic frame is finite and black, its alpha METER_CLASS.other",
    finite && black && metered && texels.length === SIZE.widthPx * SIZE.heightPx * 4,
    `${String(texels.length / 4)} texels; finite ${String(finite)}, black ${String(black)}, alpha other ${String(metered)}`,
  );
  target.resize({ widthPx: 32, heightPx: 18 });
  target.render(frameOf(PHOTOREAL_PASS_LABELS.bodies, [], 32 / 18));
  const resized = halfTexels(await engine.readTexture(target.colour));
  checks.check(
    "T7 the scene target resizes with the view's internal scale",
    resized.length === 32 * 18 * 4 && resized.every(Number.isFinite),
    `${String(resized.length / 4)} texels after resizing to 32 × 18`,
  );
}
