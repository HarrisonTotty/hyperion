import { describe, expect, it } from "vitest";

import {
  AIRLESS_ENTRIES,
  LEAN_BODY_ENTRIES,
  type CatalogueEntry,
  TABLE_ONLY_ENTRIES,
  WGSL_CATALOGUE,
} from "./catalogue";
import { airlessMaterial } from "../atmosphere/surfaceLighting";
import { fragmentOutputAt } from "./webgpu/fragmentOutput";
import {
  assertOutputsFit,
  ENTRY_POINTS,
  materialBindings,
  storageBuffersPerStage,
} from "./webgpu/materials";
import { TERRAIN_CLUSTER_ID_PRIMITIVE_MATERIALS } from "../terrain/terrainMaterial";
import { pipelineOutputs } from "./webgpu/pipelineOutputs";

/**
 * An effect's display name (decided 2026-10-02): upper case, at most three words, no colon and no
 * camelCase. The guide's `GRAPHICS SHADER REFUSED` row makes the catalogue the register of these
 * names.
 */
const DISPLAY_NAME = /^[A-Z][A-Z0-9]*( [A-Z0-9][A-Z0-9-]*){0,2}$/;

/** The display name of an entry that carries one: kernels and splats dispatch no refusal. */
function displayNameOf(entry: CatalogueEntry): string | null {
  let name: string | null;
  switch (entry.kind) {
    case "material":
    case "post-process":
      name = entry.spec.displayName;
      break;
    case "compute":
    case "point-splat":
      name = null;
      break;
  }
  return name;
}

/**
 * Every material's and post-process's display name but the table-only twins' (R08.T9.b's lever),
 * the airless twins' (plan R07, perf-bodies-pass) and the lean forms' (perf-bodies-pass-2), which a
 * refusal names by their originals' effects.
 */
const DISPLAY_NAMES: ReadonlyArray<string> = WGSL_CATALOGUE.flatMap((entry) => {
  const name =
    TABLE_ONLY_ENTRIES.includes(entry) ||
    AIRLESS_ENTRIES.includes(entry) ||
    LEAN_BODY_ENTRIES.includes(entry)
      ? null
      : displayNameOf(entry);
  return name === null ? [] : [name];
});

describe("the catalogue's kinds", () => {
  it("holds a point splat in the splat's format and blend, with no display name", () => {
    const splats = WGSL_CATALOGUE.filter((entry) => entry.kind === "point-splat");
    expect(splats.length).toBeGreaterThan(0);
    for (const entry of splats) {
      expect(displayNameOf(entry)).toBeNull();
      expect(entry.spec).toMatchObject({ format: "rgba32float", blend: "additive" });
    }
  });
});

describe("the catalogue's display names", () => {
  it("names R02's wireframe materials as decided", () => {
    expect(DISPLAY_NAMES).toEqual(
      expect.arrayContaining(["WIREFRAME LINES", "BODY OCCLUDER", "HULL OCCLUDER", "STAR SPRITES"]),
    );
  });

  it("names both terrain vertex paths' materials on the console", () => {
    expect(DISPLAY_NAMES).toEqual(expect.arrayContaining(["TERRAIN", "TERRAIN OFFSETS"]));
  });

  it("gives every effect a name in the console's form", () => {
    expect(DISPLAY_NAMES.filter((name) => !DISPLAY_NAME.test(name))).toEqual([]);
  });

  it("gives no two effects the same name", () => {
    expect(DISPLAY_NAMES.filter((name, index) => DISPLAY_NAMES.indexOf(name) !== index)).toEqual(
      [],
    );
  });

  it("rejects the forms the rule excludes", () => {
    expect(
      ["wireframe:occluderSphere", "BODY OCCLUDER: SPHERE", "THE BODY SPHERE OCCLUDER", ""].map(
        (name) => DISPLAY_NAME.test(name),
      ),
    ).toEqual([false, false, false, false]);
  });
});

describe("the catalogue's compute kernels", () => {
  it("registers R05.T12.b's atmosphere tables", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(
      expect.arrayContaining(["atmosphere transmittance", "atmosphere multiple scattering"]),
    );
  });

  it("registers R07.T8.d's cell pass as BODY DISC CELLS", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toContain("BODY DISC CELLS");
  });
});

describe("the catalogue's materials' outputs (plan R14, R14.T2.b's finding)", () => {
  const materials = WGSL_CATALOGUE.flatMap((entry) => (entry.kind === "material" ? [entry] : []));

  it("reads every material's fragment output from its WGSL", () => {
    expect(
      materials
        .filter(
          ({ spec }) =>
            fragmentOutputAt(spec.fragmentWgsl, ENTRY_POINTS.fragment, 0).kind === "unread",
        )
        .map(({ spec }) => spec.name),
    ).toEqual([]);
  });

  it("writes into each of its targets an output the target's format takes, on either canvas format", () => {
    const refused: string[] = [];
    for (const { spec, targets } of materials) {
      // `getPreferredCanvasFormat` returns one of these two (W3C WebGPU, §4.2).
      for (const preferred of ["bgra8unorm", "rgba8unorm"] as const) {
        const outputs = (targets ?? ["rgba16float"]).flatMap((target) =>
          pipelineOutputs(target, preferred),
        );
        try {
          assertOutputsFit(
            spec,
            outputs.map(([format]) => format),
          );
        } catch (error: unknown) {
          refused.push(error instanceof Error ? error.message : String(error));
        }
      }
    }
    expect(refused).toEqual([]);
  });
});

/** WebGPU's default `maxStorageBuffersPerShaderStage`, which the engine never raises. */
const DEFAULT_STORAGE_BUFFERS_PER_STAGE = 8;

describe("the catalogue's materials' storage bindings (plan R14, R14.T2.c)", () => {
  it("count at most WebGPU's default eight storage buffers in each stage of every material", () => {
    const specs = [
      ...WGSL_CATALOGUE.flatMap((entry) => (entry.kind === "material" ? [entry.spec] : [])),
      // The ID raster's `primitive_index` forms, which the catalogue leaves out (SwiftShader has no
      // `primitive-index`).
      ...TERRAIN_CLUSTER_ID_PRIMITIVE_MATERIALS,
    ];
    const over = specs.flatMap((spec) => {
      const counts = storageBuffersPerStage(materialBindings(spec));
      return Math.max(counts.vertex, counts.fragment) > DEFAULT_STORAGE_BUFFERS_PER_STAGE
        ? [`${spec.name}: ${String(counts.vertex)} vertex, ${String(counts.fragment)} fragment`]
        : [];
    });
    expect(over).toEqual([]);
    expect(specs.length).toBeGreaterThan(100);
  });
});

describe("the airless twins (plan R07, perf-bodies-pass)", () => {
  it("twins the figure, the disc, the cells and the lit terrain, in the plain and emissive forms alone", () => {
    const names = AIRLESS_ENTRIES.map((entry) => entry.spec.name);
    expect(names.every((name) => name.endsWith(" (airless)"))).toBe(true);
    expect(names).toEqual(
      expect.arrayContaining([
        "bodies:smoothMesh (airless)",
        "bodies:disc (airless)",
        "BODY DISC CELLS (airless)",
        "bodies:smoothMeshEmissive (airless)",
        // Plan R11's lit passes: the clouds, the lit terrain's cloudy forms, the seas and the rocks.
        "cloud layer (airless)",
        "cloud volume (airless)",
        "terrain-lit-baked-offsets-clouds (airless)",
        "terrain-lit-fin-face-differences-clouds (airless)",
        "ocean far pass (airless)",
        "ocean near pass (airless)",
        // Plan R11's rocks, forward and by the visibility buffer.
        "rock-lit (airless)",
        "rock-resolve (airless)",
        // Perf-bodies-pass-2's lean forms, plain and emissive.
        "bodies:smoothMeshLean (airless)",
        "bodies:discEmissiveLean (airless)",
        "BODY DISC CELLS, packed, lean (airless)",
        // Plan R11's decorative scatter, on each vertex path.
        "scatter-pebbles-baked-offsets (airless)",
        "scatter-pebbles-face-differences (airless)",
      ]),
    );
    expect(names.some((name) => name.startsWith("terrain"))).toBe(true);
    expect(names.filter((name) => name.includes("Air") || name.includes("table only"))).toEqual([]);
    expect(new Set(names).size).toBe(names.length);
  });

  it("twins every lit material of the catalogue that a body without air may be drawn by", () => {
    // Every material the engine would draw by its airless twin where a body's airless tables are
    // bound: all but the air forms, whose records carry their air, and the table-only twins, bound
    // to a body of one band's sources' table, wherever the catalogue holds them (the decorative
    // scatter's beside its pebbles): the engine twins a material as written, never a twin.
    const twinned = new Set(AIRLESS_ENTRIES.map((entry) => entry.spec.name));
    const missing = WGSL_CATALOGUE.flatMap((entry) => {
      if (
        entry.kind !== "material" ||
        AIRLESS_ENTRIES.includes(entry) ||
        TABLE_ONLY_ENTRIES.includes(entry) ||
        entry.spec.name.endsWith(" (table only)") ||
        entry.spec.name.includes("Air")
      ) {
        return [];
      }
      const twin = airlessMaterial(entry.spec);
      return twin === null || twinned.has(twin.name) ? [] : [entry.spec.name];
    });
    expect(missing).toEqual([]);
  });
});

describe("the lit bodies' lean forms (perf-bodies-pass-2)", () => {
  it("catalogues each lean form, its airless twin and its table-only twin", () => {
    const names = LEAN_BODY_ENTRIES.map((entry) => entry.spec.name);
    expect(names).toHaveLength(12);
    expect(names.every((name) => name.includes("Lean") || name.endsWith(", lean"))).toBe(true);
    expect(LEAN_BODY_ENTRIES.every((entry) => WGSL_CATALOGUE.includes(entry))).toBe(true);
    const airless = new Set(AIRLESS_ENTRIES.map((entry) => entry.spec.name));
    const tableOnly = new Set(TABLE_ONLY_ENTRIES.map((entry) => entry.spec.name));
    expect(names.filter((name) => !airless.has(`${name} (airless)`))).toEqual([]);
    expect(names.filter((name) => !tableOnly.has(`${name} (table only)`))).toEqual([]);
    // Each the same effect on the console as its full form, as a twin is.
    const displayed = new Set(DISPLAY_NAMES);
    expect(
      LEAN_BODY_ENTRIES.flatMap((entry) => {
        const name = displayNameOf(entry);
        return name === null || displayed.has(name) ? [] : [name];
      }),
    ).toEqual([]);
  });
});
