/**
 * The light half of the assembly, `atmosphereInput.ts` (plan R08, R08.T10.a): the render thread
 * computes the cache's key on every reply, so the modules it loads at run time take no optics
 * data. Checked by reading the renderer's sources, as `wasm/surfaceImports.test.ts` checks the
 * surface module's importers.
 */
import { describe, expect, it } from "vitest";

/** Where the renderer's sources lie, from the app's root, which is Vite's. */
const ROOT = "/src/renderer/src/";

/** Every `.ts` file under `src/renderer/src`, keyed by its path from the app's root. */
const SOURCES: Readonly<Record<string, string>> = import.meta.glob<string>(
  ["/src/renderer/src/**/*.ts", "!/src/renderer/src/**/*.test.ts"],
  { query: "?raw", import: "default", eager: true },
);

/**
 * An import or re-export that loads its module at run time: not `import type` or `export type`.
 * Its specifier is relative.
 */
const VALUE_IMPORT = /^(?:import|export)\s+(?!type\s)[^;]*?from\s+"(\.{1,2}\/[^"]+)"/gmu;

/** A path's directory, with its trailing slash. */
function directoryOf(path: string): string {
  return path.slice(0, path.lastIndexOf("/") + 1);
}

/** A relative specifier resolved against a directory, its `.` and `..` segments folded. */
function joined(directory: string, specifier: string): string {
  const parts: string[] = [];
  for (const part of `${directory}${specifier}`.split("/")) {
    if (part === "..") {
      parts.pop();
    } else if (part !== ".") {
      parts.push(part);
    }
  }
  return parts.join("/");
}

/** The source a specifier names, as the bundler resolves it, or the specifier for a data file. */
function resolved(from: string, specifier: string): string {
  const base = joined(directoryOf(from), specifier.split("?")[0] ?? specifier);
  return [`${base}.ts`, `${base}/index.ts`].find((path) => path in SOURCES) ?? base;
}

/** Every module a file loads at run time, itself included, and the data files among them. */
function runTimeGraph(start: string): ReadonlySet<string> {
  const seen = new Set<string>();
  const stack = [start];
  for (let path = stack.pop(); path !== undefined; path = stack.pop()) {
    if (seen.has(path)) {
      continue;
    }
    seen.add(path);
    const text = SOURCES[path];
    if (text === undefined) {
      continue;
    }
    for (const match of text.matchAll(VALUE_IMPORT)) {
      const specifier = match[1];
      if (specifier !== undefined) {
        stack.push(resolved(path, specifier));
      }
    }
  }
  return seen;
}

describe("the cache's key on the render thread", () => {
  const graph = runTimeGraph(`${ROOT}view/atmosphere/atmosphereInput.ts`);

  it("is read from the sources it walks", () => {
    expect([...graph]).toContain(`${ROOT}view/terrain/workers/f32Digest.ts`);
  });

  it("loads no optics data, tables or engine at run time", () => {
    const heavy = [...graph].filter(
      (path) =>
        path.endsWith(".json") ||
        /\/view\/atmosphere\/(?!atmosphereInput\.ts$)/u.test(path) ||
        /\/view\/(engine|lighting)\//u.test(path),
    );
    expect(heavy).toEqual([]);
  });

  it("would find a value import of the absorbers beside its type imports", () => {
    const text = 'import type { A } from "./a";\nimport { b, type C } from "./absorbers";\n';
    expect([...text.matchAll(VALUE_IMPORT)].map((m) => m[1])).toEqual(["./absorbers"]);
  });
});
