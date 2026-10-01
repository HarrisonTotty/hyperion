import { describe, expect, it } from "vitest";

/**
 * Only a worker compiles the surface module (R04.T10.a's ruling): the page's Content Security
 * Policy refuses WebAssembly on the render thread, and stays as it is. So the generated module and
 * its glue may be imported only by a `*.worker.ts` file and by tests, which run in Node. Checked by
 * reading every renderer source file, as `engineBoundary.test.ts` checks the engine's boundary. The
 * fixture strings are assembled at run time, so that this file's own text never matches the rule.
 */

/** Where the renderer's sources lie, from the app's root, which is Vite's. */
const ROOT = "/src/renderer/src/";

/** Every `.ts` and `.tsx` file under `src/renderer/src`, keyed by its path from the app's root. */
const SOURCES: Readonly<Record<string, string>> = import.meta.glob<string>(
  ["/src/renderer/src/**/*.ts", "/src/renderer/src/**/*.tsx"],
  { query: "?raw", import: "default", eager: true },
);

const GENERATED_SURFACE = ["generated", "/", "surface", "/"].join("");

/** An import, static or dynamic, of anything under `generated/surface/`. */
const SURFACE_IMPORT = new RegExp(
  String.raw`(?:from\s+|import\s*\(\s*|import\s+)["'][^"']*${GENERATED_SURFACE}`,
);

/** A static or dynamic import of a `*.worker` module, which runs it on the importing thread. */
const WORKER_IMPORT = /(?:from\s+|import\s*\(\s*|import\s+)["'][^"']*\.worker(?:\.ts)?["']/;

/** Whether `path` may import the generated module: a worker, or a test. */
function mayImportSurface(path: string): boolean {
  return /\.worker\.ts$|\.test\.tsx?$/.test(path);
}

/** The rule `path` with `text` breaks, if any. */
function violation(path: string, text: string): string | null {
  if (!mayImportSurface(path) && SURFACE_IMPORT.test(text)) {
    return `${path} imports ${GENERATED_SURFACE}, which only a worker may load`;
  }
  if (WORKER_IMPORT.test(text)) {
    return `${path} imports a worker module, which starts only as new Worker(new URL(…))`;
  }
  return null;
}

describe("the surface module's imports", () => {
  it("are only in workers and tests on the tree", () => {
    const found = Object.entries(SOURCES)
      .map(([key, text]) => violation(key.slice(ROOT.length), text))
      .filter((v) => v !== null);
    expect(found).toEqual([]);
    expect(Object.keys(SOURCES)).toContain(`${ROOT}wasm/surface.worker.ts`);
  });

  it("refuses a static or dynamic import outside a worker", () => {
    const glue = `"../${GENERATED_SURFACE}hyperion_surface"`;
    expect(violation("view/scene.ts", `import init from ${glue};`)).not.toBeNull();
    expect(violation("view/scene.ts", `const m = await import(${glue});`)).not.toBeNull();
    expect(violation("wasm/loadSurfaceModule.ts", `import { x } from ${glue};`)).not.toBeNull();
  });

  it("refuses an import of a worker module, which would run it on the importer's thread", () => {
    expect(violation("view/scene.ts", `import "./wasm/surface.worker";`)).not.toBeNull();
    expect(violation("view/scene.ts", `await import("./surface.worker.ts");`)).not.toBeNull();
    const start = `new Worker(new URL("./surface.worker.ts", import.meta.url), { type: "module" });`;
    expect(violation("wasm/loadSurfaceModule.ts", start)).toBeNull();
  });

  it("allows a worker and a test", () => {
    const glue = `import init from "../${GENERATED_SURFACE}hyperion_surface";`;
    expect(violation("wasm/surface.worker.ts", glue)).toBeNull();
    expect(violation("wasm/handleRequest.test.ts", glue)).toBeNull();
  });
});
