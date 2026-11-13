/**
 * Plan R14, R14.T2.b's check on the run's adapter: the engine's ID target, its one test frame.
 *
 * @remarks
 * A 32 × 32 scene target draws a quad over its left half 2 m out, writing its depth. An `r32uint`
 * target drawn with that depth (`depthOf`), its own colour cleared to none (`ID_NONE`), then draws
 * two IDs by `ID_WRITE_MATERIAL`: 7 over the top half 1 m out, nearer than the scene's quad, and 9
 * over the whole 3 m out, which its depth test refuses behind both. Read back, the IDs are 7 above,
 * none below left, behind the scene's quad, and 9 below right, and the scene's depth holds each
 * nearest surface's, the ID pass's writes stored into it. `ID_READ_MATERIAL` then reads the IDs as a
 * `texture_2d<u32>` by `textureLoad` into a colour target, a grey of id ÷ 255 a pixel, none black.
 */

import { ID_NONE, ID_READ_MATERIAL, ID_WRITE_MATERIAL } from "../view/engine/idTarget";
import type { RenderEngine } from "../view/engine/types";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  NEAR_M,
  rectangleMesh,
  texel,
} from "./harness";

/** The frame's side, px. */
const SIDE_PX = 32;

/**
 * Where the check reads: a point in each quarter, px from the top left, what each holds and the
 * grey the reader writes of it.
 */
const READS = [
  { x: 8, y: 8, id: 7, depthM: 1, grey: 7 / 255 },
  { x: 24, y: 8, id: 7, depthM: 1, grey: 7 / 255 },
  { x: 8, y: 24, id: ID_NONE, depthM: 2, grey: 0 },
  { x: 24, y: 24, id: 9, depthM: 3, grey: 9 / 255 },
] as const;

/** The ID writer's uniforms: `id`, which the engine packs as the `u32` it declares. */
function idOf(value: number): Readonly<Record<string, Float32Array>> {
  return { id: Float32Array.of(value) };
}

/** R14.T2.b's check (see the module's remarks). */
export async function checkIdTarget(engine: RenderEngine, checks: Checks): Promise<void> {
  const size = { widthPx: SIDE_PX, heightPx: SIDE_PX };
  const scene = engine.createRenderTarget({
    name: "R14 ID scene",
    size,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const ids = engine.createRenderTarget({
    name: "R14 IDs",
    size,
    format: "r32uint",
    mips: 1,
    depth: false,
    depthOf: scene,
    category: "render-targets",
  });
  const read = engine.createRenderTarget({
    name: "R14 IDs read",
    size,
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
  try {
    const flat = engine.createMaterial(flatSpec("R14 ID scene"));
    const writer = engine.createMaterial(ID_WRITE_MATERIAL);
    const reader = engine.createMaterial(ID_READ_MATERIAL);
    // Corners at z = −d scaled by d: the harness's projection puts them at the same view place.
    const left = rectangleMesh(engine, "R14 ID scene's quad", [-2, -2, 0, 2], -2);
    const top = rectangleMesh(engine, "R14 ID 7's quad", [-1, 0, 1, 1], -1);
    const whole = rectangleMesh(engine, "R14 ID 9's quad", [-3, -3, 3, 3], -3);
    scene.render(frameOf("R14 ID scene", [drawOf(left, flat, [0.5, 0.5, 0.5, 1])]));
    ids.render(
      frameOf("R14 IDs", [
        drawOf(top, writer, [0, 0, 0, 0], { uniforms: idOf(7) }),
        drawOf(whole, writer, [0, 0, 0, 0], { uniforms: idOf(9) }),
      ]),
    );
    read.render(
      frameOf("R14 IDs read", [
        drawOf(fullScreenMesh(engine, "R14 IDs read's triangle"), reader, [0, 0, 0, 0], {
          uniforms: {},
          textures: { ids: ids.colour },
        }),
      ]),
    );
    if (scene.depth === null) {
      throw new Error(`${scene.name} was made with depth and has none`);
    }
    const [idBytes, depthBytes, greyBytes] = await Promise.all([
      engine.readTexture(ids.colour),
      engine.readTexture(scene.depth),
      engine.readTexture(read.colour),
    ]);
    const seenIds = READS.map(({ x, y }) => new Uint32Array(idBytes)[y * SIDE_PX + x] ?? -1);
    const seenDepths = READS.map(({ x, y }) => new Float32Array(depthBytes)[y * SIDE_PX + x] ?? -1);
    const greys = halfTexels(greyBytes);
    const seenGreys = READS.map(({ x, y }) => texel(greys, SIDE_PX, x, y)[0]);
    checks.check(
      "R14.T2.b an r32uint ID target drawn with the scene's depth holds the ID of each draw its depth test passes, none elsewhere, and stores its depth into the scene's",
      READS.every((r, k) => seenIds[k] === r.id) &&
        READS.every((r, k) => Math.abs((seenDepths[k] ?? 0) - NEAR_M / r.depthM) <= 1e-7),
      `IDs ${seenIds.join(", ")} (want ${READS.map((r) => r.id).join(", ")}); depths ${seenDepths.map((d) => d.toExponential(4)).join(", ")} (want ${READS.map((r) => (NEAR_M / r.depthM).toExponential(4)).join(", ")})`,
    );
    checks.check(
      "R14.T2.b a material reads the ID target as a texture_2d<u32> by textureLoad",
      READS.every((r, k) => Math.abs((seenGreys[k] ?? -1) - r.grey) <= 1e-3),
      `greys ${seenGreys.map((g) => g.toFixed(4)).join(", ")} (want ${READS.map((r) => r.grey.toFixed(4)).join(", ")})`,
    );
  } finally {
    ids.dispose();
    read.dispose();
    scene.dispose();
  }
}
