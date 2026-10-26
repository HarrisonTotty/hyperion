/**
 * The renderer modules a source loads at run time, read from the sources themselves (plan R08,
 * R08.T10.a): what a render-thread module's bundle takes, as `view/atmosphere/assembleKey.test.ts`
 * reads it for the cache's key, so that a test can hold the optics' tables and data off the render
 * thread.
 */

/** Where the renderer's sources lie, from the app's root, which is Vite's. */
export const RENDERER_ROOT = "/src/renderer/src/";

/** Every `.ts` file under `src/renderer/src` but the tests, keyed by its path from the app's root. */
const SOURCES: Readonly<Record<string, string>> = import.meta.glob<string>(
  ["/src/renderer/src/**/*.ts", "!/src/renderer/src/**/*.test.ts"],
  { query: "?raw", import: "default", eager: true },
);

/**
 * An import or re-export that loads its module at run time, not `import type` or `export type`,
 * whose specifier is relative.
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

/**
 * Every module `start` loads at run time, itself included, and the data files among them, by
 * path from the app's root; a `.tsx` module or a package is not followed.
 *
 * @param start - A source's path from the app's root, under {@link RENDERER_ROOT}.
 */
export function runTimeGraph(start: string): ReadonlySet<string> {
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
