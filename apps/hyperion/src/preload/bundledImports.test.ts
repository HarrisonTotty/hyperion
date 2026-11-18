import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import config from "../../electron.vite.config.mts";

/** A static import or re-export and its specifier; `import type` and `export type` are not. */
const RUNTIME_IMPORT =
  /^\s*(?:import|export)\s+(?!type\s)(?:[^"'`;]*?\sfrom\s+)?["']([^"']+)["']/gmu;

/** The package a specifier names, or `null` for a relative path or one of Node's own modules. */
function packageOf(specifier: string): string | null {
  if (specifier.startsWith(".") || specifier.startsWith("/") || specifier.startsWith("node:")) {
    return null;
  }
  const parts = specifier.split("/");
  return (specifier.startsWith("@") ? parts.slice(0, 2) : parts.slice(0, 1)).join("/");
}

/** The packages the sources under `dir` import at runtime, its tests' aside. */
function runtimePackages(dir: string): ReadonlySet<string> {
  const packages = new Set<string>();
  for (const file of readdirSync(dir, { recursive: true, encoding: "utf8" })) {
    if (!/\.tsx?$/u.test(file) || /\.test\.tsx?$/u.test(file)) {
      continue;
    }
    for (const [, specifier] of readFileSync(join(dir, file), "utf8").matchAll(RUNTIME_IMPORT)) {
      const name = specifier === undefined ? null : packageOf(specifier);
      if (name !== null) {
        packages.add(name);
      }
    }
  }
  return packages;
}

/** The packages a build bundles rather than leaves to `require` (`externalizeDeps.exclude`). */
function bundled(externalizeDeps: boolean | { readonly exclude?: string[] } | undefined): string[] {
  return typeof externalizeDeps === "object" ? (externalizeDeps.exclude ?? []) : [];
}

describe("the main process's and the preload's builds", () => {
  it("bundle every package the preload imports at runtime, since a sandboxed preload can require none", () => {
    const needed = [...runtimePackages(__dirname)].filter((name) => name !== "electron");
    expect(needed.length).toBeGreaterThan(0);
    expect(bundled(config.preload?.build?.externalizeDeps)).toEqual(expect.arrayContaining(needed));
  });

  it("bundle every workspace package the main process imports at runtime, whose sources are TypeScript", () => {
    const needed = [...runtimePackages(join(__dirname, "../main"))].filter((name) =>
      name.startsWith("@hyperion/"),
    );
    expect(needed.length).toBeGreaterThan(0);
    expect(bundled(config.main?.build?.externalizeDeps)).toEqual(expect.arrayContaining(needed));
  });
});
