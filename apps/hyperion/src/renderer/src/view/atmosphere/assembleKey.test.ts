/**
 * What the assembly's two halves load at run time (plan R08, R08.T10.a): the render thread
 * computes the cache's key on every reply, so `atmosphereInput.ts` takes no optics data; and the
 * optics worker runs `assemble.ts`, which takes no engine (`tsconfig.worker.json`, no DOM). Read
 * from the sources by `test/runTimeGraph.ts`.
 */
import { describe, expect, it } from "vitest";

import { RENDERER_ROOT, runTimeGraph } from "../../test/runTimeGraph";

describe("the cache's key on the render thread", () => {
  const graph = [...runTimeGraph(`${RENDERER_ROOT}view/atmosphere/atmosphereInput.ts`)];

  it("is read from the sources it walks", () => {
    expect(graph).toContain(`${RENDERER_ROOT}view/terrain/workers/f32Digest.ts`);
  });

  it("loads no optics data, tables or engine at run time", () => {
    const heavy = graph.filter(
      (path) =>
        path.endsWith(".json") ||
        /\/view\/atmosphere\/(?!atmosphereInput\.ts$)/u.test(path) ||
        /\/view\/(engine|lighting)\//u.test(path),
    );
    expect(heavy).toEqual([]);
  });
});

describe("the assembly in the optics worker", () => {
  const graph = [...runTimeGraph(`${RENDERER_ROOT}view/atmosphere/assemble.ts`)];

  it("is read from the sources it walks", () => {
    expect(graph).toContain(`${RENDERER_ROOT}view/atmosphere/absorbers.ts`);
  });

  it("loads no engine, hillaire.ts or tables.ts at run time", () => {
    const engine = graph.filter(
      (path) =>
        path.includes("/view/engine/") ||
        path.endsWith("/view/atmosphere/hillaire.ts") ||
        path.endsWith("/view/atmosphere/tables.ts"),
    );
    expect(engine).toEqual([]);
  });
});
