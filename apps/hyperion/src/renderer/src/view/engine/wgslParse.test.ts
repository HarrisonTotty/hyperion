/**
 * Every WGSL module the client ships, parsed and validated by naga as the engine hands it to the
 * device, so that a module the device's compiler would refuse fails here and not first in
 * `just test-render` (plan R14's CI health, 2026-11-05).
 *
 * @remarks
 * No other test compiles WGSL: `set` as a parameter's name, a reserved word (W3C WGSL, §15.5),
 * passed tsc, oxlint and vitest, and only test-render's catalogue refused it (part 1,
 * r14-t3e-triangle-budget). naga is wgpu's own front end (naga 30.0.1, the version
 * `tools/gpu-replay` pins), built to WebAssembly by the `naga-wasm` package: its parse reads, lowers
 * and resolves every name, and its validator types every expression. Each module is the text the
 * engine compiles, after the project's own assembly of its `.wgsl` files (concatenation, and the
 * `//NAME//` placeholders a source fills).
 */

import { NagaError, parseWgsl, validate } from "naga-wasm";
import { describe, expect, it } from "vitest";

import { type CatalogueEntry, WGSL_CATALOGUE } from "./catalogue";
import { CLUSTER_OVER_KERNEL } from "../clusters/overCount";
import { TERRAIN_CLUSTER_ID_PRIMITIVE_MATERIALS } from "../terrain/terrainMaterial";
import type { ProbedTargetFormat } from "./status";
import type { WgslMaterialSpec } from "./types";
import { FULL_SCREEN_WGSL } from "./webgpu/materials";
import { DOWNSAMPLE_WGSL } from "./webgpu/mipmaps";
import { PROBE_VALUES, probeWgsl } from "./webgpu/roundingProbe";

/** One module as the device receives it, named for its failure. */
interface ShippedModule {
  readonly name: string;
  readonly wgsl: string;
}

/** A material's modules: one where both stages share the source, as the engine makes one. */
function materialModules(spec: WgslMaterialSpec): ShippedModule[] {
  return spec.vertexWgsl === spec.fragmentWgsl
    ? [{ name: spec.name, wgsl: spec.vertexWgsl }]
    : [
        { name: `${spec.name}: vertex`, wgsl: spec.vertexWgsl },
        { name: `${spec.name}: fragment`, wgsl: spec.fragmentWgsl },
      ];
}

/** The modules an entry of the catalogue makes, each kernel's in both its forms. */
function entryModules(entry: CatalogueEntry): ShippedModule[] {
  let modules: ShippedModule[];
  switch (entry.kind) {
    case "material":
      modules = materialModules(entry.spec);
      break;
    case "point-splat":
      modules = [
        { name: `${entry.spec.name}: vertex`, wgsl: entry.spec.vertexWgsl },
        { name: `${entry.spec.name}: fragment`, wgsl: entry.spec.fragmentWgsl },
      ];
      break;
    case "post-process":
      modules = [{ name: entry.spec.name, wgsl: entry.spec.fragmentWgsl }];
      break;
    case "compute":
      modules = [
        { name: `${entry.spec.name}: reference`, wgsl: entry.spec.reference },
        ...(entry.spec.subgroup === null
          ? []
          : [{ name: `${entry.spec.name}: subgroup`, wgsl: entry.spec.subgroup }]),
      ];
      break;
  }
  return modules;
}

/** Whether `format` is one the rounding probe draws. */
function isProbed(format: string): format is ProbedTargetFormat {
  return Object.hasOwn(PROBE_VALUES, format);
}

/**
 * Every module the engine can compile, each source once: the catalogue's, and those the engine makes
 * outside it (the post-processes' full-screen stage, the mips' downsampling, the rounding probe's
 * passes, the ID raster's `primitive_index` forms, which SwiftShader cannot compile, and the
 * clusters' over-count copy).
 */
const MODULES: ReadonlyArray<ShippedModule> = [
  ...WGSL_CATALOGUE.flatMap(entryModules),
  ...TERRAIN_CLUSTER_ID_PRIMITIVE_MATERIALS.flatMap(materialModules),
  { name: `${CLUSTER_OVER_KERNEL.name}: reference`, wgsl: CLUSTER_OVER_KERNEL.reference },
  { name: "full screen", wgsl: FULL_SCREEN_WGSL },
  { name: "mip downsampling", wgsl: DOWNSAMPLE_WGSL },
  ...Object.keys(PROBE_VALUES)
    .filter(isProbed)
    .map((format) => ({ name: `rounding probe ${format}`, wgsl: probeWgsl(format) })),
].filter(({ wgsl }, index, all) => all.findIndex((other) => other.wgsl === wgsl) === index);

/** Every `.wgsl` file under `view/`, by its path from here. */
const FILES = import.meta.glob<string>("../**/*.wgsl", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** A placeholder that a source fills before the engine compiles it (`clusters/compact.ts`). */
const PLACEHOLDER = /\/\/[A-Z_]+\/\//gu;

/**
 * The enable-extensions naga 30 refuses by name though it implements their functions: `subgroups`
 * (W3C WGSL, §4.1.1), whose builtins its parse takes without the directive and its validator under
 * its own subgroup capability.
 */
const NAGA_REFUSES: ReadonlySet<string> = new Set(["subgroups"]);

/**
 * `wgsl` with {@link NAGA_REFUSES}' names taken out of its `enable` directives, a directive left
 * with none blanked to spaces, so that naga's line and column are the module's.
 */
function forNaga(wgsl: string): string {
  return wgsl.replaceAll(/\benable\s+([^;]+);/gu, (directive, list: string) => {
    const names = list
      .split(",")
      .map((name) => name.trim())
      .filter((name) => name.length > 0);
    const kept = names.filter((name) => !NAGA_REFUSES.has(name));
    if (kept.length === names.length) {
      return directive;
    }
    if (kept.length === 0) {
      return directive.replaceAll(/[^\n]/gu, " ");
    }
    return `enable ${kept.join(", ")};`.padEnd(directive.length, " ");
  });
}

/** The first lines of naga's message: its summary, and the line of the module it points at. */
function excerpt(error: unknown): string {
  if (error instanceof NagaError) {
    return error.formatted.split("\n").slice(0, 6).join("\n");
  }
  return error instanceof Error ? error.message : String(error);
}

/** Why naga refuses `module`, parsing or validating it, or `null` where it takes it. */
function refusal(module: ShippedModule): string | null {
  let parsed: ReturnType<typeof parseWgsl>;
  try {
    parsed = parseWgsl(forNaga(module.wgsl));
  } catch (error: unknown) {
    return `${module.name} (parse): ${excerpt(error)}`;
  }
  try {
    validate(parsed).free();
    return null;
  } catch (error: unknown) {
    return `${module.name} (validation): ${excerpt(error)}`;
  } finally {
    parsed.free();
  }
}

describe("the client's WGSL modules, by naga", () => {
  it("assemble every `.wgsl` file under view/ into a module, piece by piece about its placeholders", () => {
    const sources = MODULES.map(({ wgsl }) => wgsl);
    const unshipped = Object.entries(FILES).flatMap(([path, text]) =>
      text
        .split(PLACEHOLDER)
        .map((piece) => piece.trim())
        .every((piece) => piece.length === 0 || sources.some((wgsl) => wgsl.includes(piece)))
        ? []
        : [path],
    );
    expect(unshipped).toEqual([]);
    expect(Object.keys(FILES).length).toBeGreaterThan(100);
  });

  // The parse and validation of some 240 modules, 42 MB of WGSL, took 4.1 s alone on a pod
  // (2026-11-05); 10 times that, as CPU-bound work, rounded up.
  it(
    "parse and validate, every one, as the engine hands it to the device",
    { timeout: 60_000 },
    () => {
      expect(MODULES.map(refusal).filter((reason) => reason !== null)).toEqual([]);
      expect(MODULES.length).toBeGreaterThan(200);
    },
  );

  it("refuse a shipped module with a reserved word for a parameter's name", () => {
    const [first] = MODULES;
    if (first === undefined) {
      throw new Error("the client ships no WGSL module");
    }
    expect(refusal(first)).toBeNull();
    const planted = {
      ...first,
      wgsl: `${first.wgsl}\nfn planted(set : u32) -> u32 { return set; }\n`,
    };
    expect(refusal(planted)).toMatch(/\(parse\): error: name `set` is a reserved keyword/u);
  });

  it("take the subgroups directive out for naga, keeping every other extension and every column", () => {
    expect(MODULES.some(({ wgsl }) => /\benable\s+subgroups\s*;/u.test(wgsl))).toBe(true);
    const directives = "enable f16, subgroups;\nenable subgroups;\nenable primitive_index;\n";
    const taken = forNaga(directives);
    expect(taken.split("\n")).toEqual([
      "enable f16;".padEnd(22, " "),
      " ".repeat(17),
      "enable primitive_index;",
      "",
    ]);
  });
});
