import { readdirSync, readFileSync } from "node:fs";
import { builtinModules } from "node:module";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const SRC = fileURLToPath(new URL(".", import.meta.url));

/**
 * The package's library sources: everything under `src/` but its tests, their helpers in
 * `src/test/` and the command-line tools in `src/cli/`, which run under Node and read files.
 */
function librarySources(dir: string): ReadonlyArray<string> {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      return entry.name === "cli" || entry.name === "test" ? [] : librarySources(path);
    }
    return entry.name.endsWith(".ts") && !entry.name.endsWith(".test.ts") ? [path] : [];
  });
}

/** Every module specifier a source imports or re-exports, statically or dynamically. */
function specifiers(text: string): ReadonlyArray<string> {
  const found: string[] = [];
  for (const match of text.matchAll(/(?:\bfrom|\bimport|\brequire)\s*\(?\s*["']([^"']+)["']/g)) {
    found.push(...match.slice(1, 2));
  }
  return found;
}

/** A source with its comments and string literals blanked, so that a word in prose is not code. */
function code(text: string): string {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .replace(/\/\/[^\n]*/g, " ")
    .replace(/(["'`])(?:\\.|(?!\1)[^\\\n])*\1/g, '""');
}

const NODE = new Set(builtinModules);
const DOM_GLOBALS =
  /\b(window|document|navigator|localStorage|sessionStorage|HTMLElement|requestAnimationFrame)\b/;

describe("the package's boundary", () => {
  const sources = librarySources(SRC);

  it("has library sources to check", () => {
    expect(sources.map((path) => relative(SRC, path))).toEqual(
      expect.arrayContaining(["index.ts", "record.ts", "rows.ts", "scenes.ts"]),
    );
  });

  it("imports nothing from Node, Electron or outside the package", () => {
    const offending = sources.flatMap((path) =>
      specifiers(readFileSync(path, "utf8"))
        .filter(
          (specifier) =>
            specifier.startsWith("node:") ||
            NODE.has(specifier) ||
            specifier === "electron" ||
            specifier.startsWith("electron/") ||
            !(specifier.startsWith("./") && specifier.endsWith(".ts")),
        )
        .map((specifier) => `${relative(SRC, path)} imports ${specifier}`),
    );
    expect(offending).toEqual([]);
  });

  it("uses no DOM global", () => {
    const offending = sources.flatMap((path) => {
      const found = code(readFileSync(path, "utf8")).match(DOM_GLOBALS);
      return found === null ? [] : [`${relative(SRC, path)} uses ${found[0]}`];
    });
    expect(offending).toEqual([]);
  });

  it("catches an import from Node and a DOM global", () => {
    expect(
      specifiers('import { readFileSync } from "node:fs";\nexport * from "./rows.ts";'),
    ).toEqual(["node:fs", "./rows.ts"]);
    expect(
      code("// the window's frames\nconst w = window.innerWidth;").match(DOM_GLOBALS)?.[0],
    ).toBe("window");
  });
});
