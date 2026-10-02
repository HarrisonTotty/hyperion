/**
 * The smoke page's offline catalogue check (T9.b): every `WGSL_CATALOGUE` entry is made once, with
 * the network refused by the harness's main process, and must compile; a post-process is also run
 * over a drawn frame whose texels must stay finite.
 */

import type { CatalogueEntry } from "../view/engine/catalogue";
import type { GraphicsStatusStore } from "../view/engine/status";
import type { RenderEngine } from "../view/engine/types";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  show,
} from "./harness";

/** The fixtures a run may add, named by `--smoke-fixture`. */
export type SmokeFixture = "none" | "broken-wgsl" | "external-fetch";

/** A material whose WGSL does not compile, which must fail the run naming it. */
export const BROKEN_ENTRY: CatalogueEntry = {
  kind: "material",
  spec: flatSpec("broken fixture", {
    displayName: "TEST FIXTURE",
    fragmentWgsl: "@fragment fn fragmentMain() -> @location(0) vec4f { return oops; }",
  }),
};

/** Makes every entry once and checks it compiled, and that the shader fault stayed clear. */
export async function checkCatalogue(
  engine: RenderEngine,
  status: GraphicsStatusStore,
  entries: ReadonlyArray<CatalogueEntry>,
  checks: Checks,
): Promise<void> {
  const full = fullScreenMesh(engine, "catalogue full");
  const flat = engine.createMaterial(flatSpec("catalogue flat"));
  for (const entry of entries) {
    const settings = entry.settings ?? ["default"];
    for (const setting of settings) {
      const name = `T9.b ${entry.kind} ${entry.spec.name} (${setting})`;
      // The harness's checks run in order: each reads the GPU back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      await checks.group(name, async () => {
        switch (entry.kind) {
          case "material":
            await engine.createMaterialAsync(entry.spec, ["rgba16float"]);
            checks.check(name, true, "compiled");
            break;
          case "compute": {
            const kernel = await engine.createComputeAsync(entry.spec);
            checks.check(name, true, `compiled on the ${kernel.path} path`);
            break;
          }
          case "post-process": {
            const postProcess = engine.createPostProcess(entry.spec);
            const target = engine.createRenderTarget({
              name: entry.spec.name,
              size: { widthPx: 8, heightPx: 8 },
              format: "rgba16float",
              mips: 1,
              depth: true,
              category: "render-targets",
            });
            target.render(
              frameOf(entry.spec.name, [drawOf(full, flat, [0.5, 0.25, 0.125, 1])], 1, [
                { postProcess, uniforms: {} },
              ]),
            );
            const texels = halfTexels(await engine.readTexture(target.colour));
            checks.check(
              name,
              texels.every(Number.isFinite),
              `first texel ${show(texels.subarray(0, 4))}`,
            );
            target.dispose();
            break;
          }
        }
      });
    }
  }
  const { fault } = status.getSnapshot();
  checks.check(
    "T9.b no shader was refused",
    fault?.kind !== "shader-refused",
    JSON.stringify(fault),
  );
}

/** The external-fetch fixture: requests that the harness's main process must cancel and name. */
export async function makeExternalRequests(): Promise<void> {
  const image = new Image();
  image.src = "https://example.invalid/smoke.png";
  await fetch("https://example.invalid/smoke.json").catch(() => undefined);
}
