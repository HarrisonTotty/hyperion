/**
 * The sky's harness checks (plan R06): the band's HDR draw on a target the check makes itself,
 * since R07.T7 makes the views' scene target (decision record item 1 of 2026-10-02).
 */

import type { SkyBand } from "@hyperion/protocol";

import type { RenderEngine } from "../view/engine/types";
import { BandLayer } from "../view/sky/band";
import { type Checks, frameOf, halfTexels, show, texel } from "./harness";

/** The band's face side in the check, texels. */
const FACE = 4;

/** A white band whose face k (WebGPU's layer order) holds (k + 1) × 10⁻⁴ cd/m². */
function steppedBand(): SkyBand {
  const count = 6 * FACE * FACE;
  return {
    count,
    luminanceCdM2: Float32Array.from(
      { length: count },
      (_, i) => (Math.floor(i / (FACE * FACE)) + 1) * 1e-4,
    ),
    chroma: new Float32Array(count * 2).fill(1 / 3),
    eyeLimitMag: new Float32Array(count).fill(Number.NaN),
    spRatio: new Float32Array(count).fill(2.26),
  };
}

/**
 * R06.T13.d: the band drawn first into an `rgba16float` target, at the view's direction's face,
 * pre-exposed, the target's alpha (R07's meter class) kept.
 */
export async function checkSkyBand(engine: RenderEngine, checks: Checks): Promise<void> {
  const layer = new BandLayer(engine);
  layer.update(steppedBand(), FACE, new Float64Array(6 * FACE * FACE * 3));
  const target = engine.createRenderTarget({
    name: "sky band check",
    size: { widthPx: 8, heightPx: 8 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const draw = layer.draw(1_000);
  if (draw === null) {
    throw new Error("the band layer gave no draw after its update");
  }
  // The identity view looks along −z, face 5, which holds 6 × 10⁻⁴ cd/m²: 0.6 at 1,000.
  target.render(frameOf("sky band check", [draw]));
  const texels = halfTexels(await engine.readTexture(target.colour));
  const centre = texel(texels, 8, 4, 4);
  checks.check(
    "R06.T13.d the band reads its face's luminance, pre-exposed, at the view's centre",
    Math.abs(centre[0] - 0.6) < 0.01 && Math.abs(centre[1] - 0.6) < 0.01,
    show(centre),
  );
  checks.check(
    "R06.T13.d the band keeps the target's alpha, the meter class",
    centre[3] === 1,
    show(centre),
  );
  checks.check(
    "R06.T13.d every band texel is finite",
    texels.every(Number.isFinite),
    `${texels.length / 4} texels`,
  );
  target.dispose();
  layer.dispose();
}
