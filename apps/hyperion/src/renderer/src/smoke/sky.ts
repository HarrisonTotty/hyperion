/**
 * The sky's harness checks (plan R06): the band's HDR draw on a target the check makes itself,
 * since R07.T7 makes the views' scene target (decision record item 1 of 2026-10-02).
 */

import type { HostDiscDto, SkyBand } from "@hyperion/protocol";

import type { RenderEngine } from "../view/engine/types";
import { BandLayer } from "../view/sky/band";
import { HostDiscLayer } from "../view/sky/disc";
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

/** A host whose centre is `centralRgb` cd/m² in red, green and blue, darkened by c 0.5, α 1. */
function testHost(centralRgb: readonly [number, number, number]): HostDiscDto {
  const law = { c: 0.5, alpha: 1 };
  // The law's disc average is 1 − cα ÷ (α + 2) = 5 ÷ 6.
  const mean = 5 / 6;
  return {
    star: 0,
    radius_m: Math.sin((15 * Math.PI) / 180),
    teff_k: 5_772,
    log_g: 4.438,
    mean_luminance_cd_m2: [centralRgb[2] * mean, centralRgb[1] * mean, centralRgb[0] * mean],
    central_luminance_cd_m2: [centralRgb[2], centralRgb[1], centralRgb[0]],
    limb: [law, law, law],
    chroma: [1, 1],
    lux_per_v0: 1,
    bake_spectrum: [
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
      1 / 15,
    ],
  };
}

/**
 * R06.T13.e: a host disc 30° across lights its pixels by its law, pre-exposed and clamped at
 * 65,504, writes R07's meter class `hostDisc` (0) in their alpha, and the band drawn over it
 * keeps that class.
 */
export async function checkSkyDisc(engine: RenderEngine, checks: Checks): Promise<void> {
  const discs = new HostDiscLayer(engine);
  const band = new BandLayer(engine);
  band.update(steppedBand(), FACE, new Float64Array(6 * FACE * FACE * 3));
  const target = engine.createRenderTarget({
    name: "sky disc check",
    size: { widthPx: 64, heightPx: 64 },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const look = { orientation: { w: 1, x: 0, y: 0, z: 0 }, fovXRad: Math.PI / 2 };
  const viewport = { widthPx: 64, heightPx: 64 };
  const ahead = { x: 0, y: 0, z: -1 };
  const frame = discs.frame(
    [{ host: testHost([1, 2, 3]), direction: ahead, distanceM: 1 }],
    look,
    viewport,
    1_000,
  );
  const bandDraw = band.draw(1);
  if (frame.draws.length !== 1 || bandDraw === null) {
    throw new Error("the disc check made no disc draw or no band draw");
  }
  target.render(frameOf("sky disc check", [...frame.draws.map((d) => d.item), bandDraw]));
  const texels = halfTexels(await engine.readTexture(target.colour));
  const centre = texel(texels, 64, 32, 32);
  const corner = texel(texels, 64, 2, 2);
  // The centre: μ ≈ 1, (1, 2, 3) × 1,000 pre-exposed, plus the band's 6 × 10⁻⁴.
  checks.check(
    "R06.T13.e the disc's centre is its central luminance, pre-exposed",
    Math.abs(centre[0] - 1_000) < 15 &&
      Math.abs(centre[1] - 2_000) < 30 &&
      Math.abs(centre[2] - 3_000) < 45,
    show(centre),
  );
  checks.check(
    "R06.T13.e the disc's pixels carry the meter class hostDisc, kept under the band drawn over it",
    centre[3] === 0 && corner[3] === 1,
    `centre ${show(centre)}, corner ${show(corner)}`,
  );
  checks.check("R06.T13.e the disc lights nothing outside it", corner[0] < 1e-2, show(corner));
  const bright = discs.frame(
    [{ host: testHost([1e9, 1e9, 1e9]), direction: ahead, distanceM: 1 }],
    look,
    viewport,
    1,
  );
  target.render(
    frameOf(
      "sky disc clamp check",
      bright.draws.map((d) => d.item),
    ),
  );
  const clamped = texel(halfTexels(await engine.readTexture(target.colour)), 64, 32, 32);
  checks.check(
    "R06.T13.e a disc above a half's range is clamped at 65,504",
    clamped[0] === 65_504 && clamped.every(Number.isFinite),
    show(clamped),
  );
  target.dispose();
  band.dispose();
}
