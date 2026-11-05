import { afterEach, describe, expect, it, vi } from "vitest";

import {
  earthBody,
  saturnBody,
  saturnClassMedium,
  SATURN_SPHEROID,
} from "../../test/atmosphereFixtures";
import { wgslFunctions, wgslReached } from "../../test/atmosphereWgsl";
import { boundNames, countingRenderEngine } from "../../test/countingRenderEngine";
import { patternedDeckTables, patternedThickBake } from "../../test/thickTables";
import { lookAlong } from "../camera/quaternion";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import type { KernelPair } from "../engine/kernels";
import { kernelBindings } from "../engine/webgpu/compute";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
  referenceSunIlluminanceLx,
} from "./earth";
import {
  type AtmosphereCamera,
  type AtmosphereScene,
  type AtmosphereSettings,
  createAtmosphereFrameKernels,
  HillaireAtmosphere,
  LIMB_DISPATCH_KERNEL,
  LIMB_RESOLVE_KERNEL,
  PER_PIXEL_LEVERS,
  RAY_MARCH_DECK_SPECIAL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_LIMB_KERNEL,
  RAY_MARCH_PACKED_KERNEL,
  RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL,
  RAY_MARCH_THICK_KERNEL,
  RAY_MARCH_THICK_ROW_KERNEL,
  RAY_MARCH_THICK_ROW_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_ROW_KERNEL,
  RAY_MARCH_SLOTS_THICK_ROW_SPECIAL_KERNEL,
  RAY_MARCH_THICK_SPECIAL_KERNEL,
  RAY_MARCH_THICK_TABLE_KERNEL,
  RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
  THICK_LIMB_KERNELS,
  withThickRead,
} from "./hillaire";
import thickSampleRowWgsl from "./shaders/thickSampleRow.wgsl?raw";
import { SOURCE_TABLE_KERNEL_TEXTURE } from "./surfaceLighting";
import { AtmosphereTables, kernelMedium } from "./tables";
import { upperMediumOf } from "./thick/deckRead";

/** WGS 84 (NIMA TR8350.2): a = 6,378,137 m, f = 1 ÷ 298.257223563. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};

/** From 10⁸ m over the equator, looking at the centre: the Earth's limb a few texels across. */
const DISTANT: AtmosphereCamera = {
  positionM: { x: 1e8, y: 0, z: 0 },
  orientation: lookAlong({ x: -1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 }),
  fovXRad: Math.PI / 3,
  viewport: { widthPx: 320, heightPx: 180 },
};

/** From 2 km over the equator, looking north along the horizon: inside the air. */
const NEAR: AtmosphereCamera = {
  positionM: { x: WGS84.equatorialRadiusM + 2_000, y: 0, z: 0 },
  orientation: lookAlong({ x: 0, y: 0, z: 1 }, { x: 1, y: 0, z: 0 }),
  fovXRad: Math.PI / 2,
  viewport: { widthPx: 64, heightPx: 32 },
};

const SCENE: AtmosphereScene = {
  colour: { kind: "texture", name: "terrain colour" },
  depth: { kind: "texture", name: "terrain depth" },
  nearM: 0.1,
  exposureScale: 1e-4,
};

const NOON: SunState = {
  directionBodyFixed: { x: 1, y: 0, z: 0 },
  illuminanceLx: referenceSunIlluminanceLx(),
  angularRadiusRad: 0.004_65,
};

/** The deck's split over the datum, m: 3 km, as the deck read's tests stand it. */
const DECK_ALTITUDE_M = 3_000;

/** The sources' table's stub's name (`AtmosphereTables.perFragmentSourceTransmittance`). */
const STUB = "atmosphere: sources' transmittance per fragment";

/** The bindings `wgsl` declares that neither its entry point nor any function it reaches names. */
function unreachedBindings(wgsl: string): string[] {
  const functions = wgslFunctions(wgsl);
  const bodies = [...wgslReached(wgsl, ["main"])].map((name) => functions.get(name)?.body ?? "");
  return [...kernelBindings(wgsl).keys()].filter(
    (name) => !bodies.some((body) => new RegExp(`\\b${name}\\b`, "u").test(body)),
  );
}

/** The calls of `call` in the functions `main` reaches. */
function callsReached(wgsl: string, call: string): number {
  const functions = wgslFunctions(wgsl);
  const pattern = new RegExp(`\\b${call}\\(`, "gu");
  return [...wgslReached(wgsl, ["main"])]
    .map((name) => [...(functions.get(name)?.body ?? "").matchAll(pattern)].length)
    .reduce((sum, calls) => sum + calls, 0);
}

/** The lines of `b` that differ from `a`'s, each as [a's, b's], trimmed. */
function linesDiffering(a: string, b: string): ReadonlyArray<readonly [string, string]> {
  const theirs = b.split("\n");
  return a.split("\n").flatMap((line, i) => {
    const other = theirs[i] ?? "";
    return line === other ? [] : [[line.trim(), other.trim()] as const];
  });
}

/** Every thick march the levers make: the footprints from the table, and the packed kernels. */
const LEVER_MARCHES: ReadonlyArray<KernelPair> = [
  RAY_MARCH_THICK_TABLE_KERNEL,
  RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL,
  ...THICK_LIMB_KERNELS,
];

/** The table-only line of `marchSourceTable.wgsl`, unset and set. */
const TABLE_BOTH_WAYS = "const SOURCE_TABLE_ONLY : bool = false;";
const TABLE_ALONE = "const SOURCE_TABLE_ONLY : bool = true;";

describe("the thick read's march levers' kernels (perf-thick-march)", () => {
  it.each(LEVER_MARCHES.map((pair) => [pair.name, pair.reference]))(
    "reaches every binding %s declares from its entry point",
    (_name, source) => {
      expect(unreachedBindings(source)).toEqual([]);
    },
  );

  it("makes the march from the table from the thin one with it, differing only in its table alone and where it finds its row", () => {
    const appended = `\n${thickSampleRowWgsl}`;
    const tabled = RAY_MARCH_THICK_TABLE_KERNEL.reference;
    const cleared = "  thickSampleRowFound = false;\n";
    const searched = tabled
      .slice(0, -appended.length)
      .replace(cleared, "")
      .replace(
        "let row = thickSampleRowAt(start.scaledHeightM);",
        "let row = thickRowAt(max(start.scaledHeightM, 0.0));",
      )
      .replace(TABLE_ALONE, TABLE_BOTH_WAYS);
    expect(linesDiffering(withThickRead(RAY_MARCH_KERNEL).reference, searched)).toEqual([]);
  });

  it("appends the row's search once to the march from the table, which clears it once a step", () => {
    const tabled = RAY_MARCH_THICK_TABLE_KERNEL.reference;
    expect([
      tabled.endsWith(`\n${thickSampleRowWgsl}`),
      tabled.split("  thickSampleRowFound = false;\n").length,
    ]).toEqual([true, 2]);
  });

  it("leaves the thick read's march per step without the row once a sample", () => {
    expect(RAY_MARCH_THICK_KERNEL.reference).not.toContain("thickSampleRow");
  });

  it.each(
    [
      ...LEVER_MARCHES,
      RAY_MARCH_THICK_ROW_KERNEL,
      RAY_MARCH_SLOTS_THICK_ROW_KERNEL,
      RAY_MARCH_THICK_ROW_SPECIAL_KERNEL,
      RAY_MARCH_SLOTS_THICK_ROW_SPECIAL_KERNEL,
    ].map((pair) => [pair.name, pair.reference]),
  )(
    "finds the altitude row in %s through the step's one search, which each step clears",
    (_name, source) => {
      const functions = wgslFunctions(source);
      expect([
        functions.get("thickMultipleAt")?.body.includes("thickSampleRowAt(start.scaledHeightM)"),
        functions.get("thickMultipleAt")?.body.includes("thickRowAt("),
        functions.get("thickSampleRowAt")?.body.includes("thickRowAt("),
        functions.get("rayMarchStep")?.body.includes("thickSampleRowFound = false;"),
        callsReached(source, "thickRowAt"),
      ]).toEqual([true, false, true, true, 1]);
    },
  );

  // SwiftShader inlines every call, and its GPU process crashed on a march emitted twice in one
  // entry point (R08.T8.d): the thick read's packed kernels march through one call each.
  it("marches one ray through one call site in the thick read's packed entry points and node passes", () => {
    expect(THICK_LIMB_KERNELS).toHaveLength(12);
    expect(THICK_LIMB_KERNELS.map((pair) => callsReached(pair.reference, "marchRay"))).toEqual(
      THICK_LIMB_KERNELS.map(() => 1),
    );
    expect(THICK_LIMB_KERNELS.map((pair) => pair.name)).toEqual(
      expect.arrayContaining([
        `${RAY_MARCH_PACKED_KERNEL.name}, thick`,
        `${RAY_MARCH_LIMB_KERNEL.name}, thick`,
      ]),
    );
  });

  // SwiftShader's GPU process crashed compiling the shared read's general thick march with both
  // ways (perf-thick-march): the thick read's marches take the table alone, or none.
  it("takes each step's sources from the table alone in every thick march from it", () => {
    const fromTable = LEVER_MARCHES.filter((pair) => pair.name.includes("from the sources' table"));
    expect([
      fromTable.length,
      fromTable.every(
        (pair) => pair.reference.includes(TABLE_ALONE) && !pair.reference.includes(TABLE_BOTH_WAYS),
      ),
    ]).toEqual([6, true]);
  });

  it("declares no table in the thick read's packed kernels per step", () => {
    const perStep = LEVER_MARCHES.filter((pair) => !pair.name.includes("from the sources' table"));
    expect([
      perStep.length,
      perStep.some((pair) => kernelBindings(pair.reference).has(SOURCE_TABLE_KERNEL_TEXTURE)),
    ]).toEqual([8, false]);
  });

  it("names every lever kernel apart", () => {
    expect(new Set(LEVER_MARCHES.map((pair) => pair.name)).size).toBe(LEVER_MARCHES.length);
  });

  it("is catalogued, so that the smoke page compiles each lever's kernel", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(expect.arrayContaining(LEVER_MARCHES.map((pair) => pair.name)));
  });
});

describe("HillaireAtmosphere's thick read under the march's levers (perf-thick-march)", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /**
   * R05's Earth over a thick table (or a deck), the measured Earth (by slot) or the Saturn-class
   * figure over one, drawn from
   * `camera` under `levers` over the
   * device's shared kernels, its first frame drawn and its background kernels made under the fake
   * clock; the second frame's march: each dispatch's kernel, its bindings' names, and the sources'
   * table it bound.
   */
  async function marched(
    levers: Partial<AtmosphereSettings>,
    camera: AtmosphereCamera = DISTANT,
    body: "Earth" | "by slot" | "deck" | "Saturn" = "Earth",
  ): Promise<{
    readonly kernels: string[];
    readonly bound: string[][];
    readonly table: Array<string | undefined>;
    readonly made: string[];
    readonly sources: string[];
  }> {
    vi.useFakeTimers();
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const tables =
      body === "Saturn"
        ? new AtmosphereTables(
            engine,
            saturnClassMedium(),
            tableRadiusM(SATURN_SPHEROID),
            [],
            saturnBody(),
          )
        : body === "by slot"
          ? new AtmosphereTables(
              engine,
              EARTH_REFERENCE,
              tableRadiusM(WGS84),
              EARTH_REFERENCE_CURVES,
              earthBody(),
            )
          : new AtmosphereTables(
              engine,
              body === "deck"
                ? upperMediumOf(R05_EARTH_REFERENCE, DECK_ALTITUDE_M)
                : R05_EARTH_REFERENCE,
              tableRadiusM(WGS84),
              [],
              earthBody(),
            );
    if (body === "deck") {
      tables.setDeckTables(
        patternedDeckTables(tables.plan.bands, kernelMedium(tables.medium), 1, DECK_ALTITUDE_M),
      );
    } else {
      tables.setThickTables(
        patternedThickBake(
          tables.plan.bands,
          kernelMedium(tables.medium),
          Math.max(1, tables.curves.length),
        ),
      );
    }
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels, {
      ...PER_PIXEL_LEVERS,
      ...levers,
    });
    view.drawFrame(camera, [NOON], SCENE);
    await vi.advanceTimersByTimeAsync(0);
    engine.dispatched.length = 0;
    view.drawFrame(camera, [NOON], SCENE);
    const march = engine.dispatched.filter((d) => d.kernel.startsWith("atmosphere ray march"));
    return {
      kernels: march.map((d) => d.kernel),
      bound: march.map((d) => boundNames(d.bindings)),
      table: march.map((d) => d.bindings.sampled[SOURCE_TABLE_KERNEL_TEXTURE]?.name),
      made: engine.kernelPairs.map((pair) => pair.name),
      sources: march.map(
        (d) => engine.kernelPairs.findLast((pair) => pair.name === d.kernel)?.reference ?? "",
      ),
    };
  }

  /** The thick read's packed kernels per step in the specialised form, by their names. */
  const packedSpecial = THICK_LIMB_KERNELS.filter(
    (pair) => pair.name.endsWith("edge-free") && !pair.name.includes("table"),
  ).map((pair) => pair.name);

  it("marches per step by the thick read's march with its row once a sample, its base's name", async () => {
    const { kernels, sources } = await marched({});
    expect([kernels, sources[0] === RAY_MARCH_THICK_ROW_SPECIAL_KERNEL.reference]).toEqual([
      [RAY_MARCH_THICK_SPECIAL_KERNEL.name],
      true,
    ]);
  });

  it("marches per step and per texel by the thick read's own march on the way back, binding no table", async () => {
    const { kernels, table } = await marched({});
    expect(kernels).toEqual([RAY_MARCH_THICK_SPECIAL_KERNEL.name]);
    expect(table).toEqual([undefined]);
  });

  it("packs its far limb's node rays from above the top, per step, binding no table", async () => {
    const { kernels, bound, table } = await marched({ limbRays: "packed" });
    expect(kernels).toEqual([
      packedSpecial.find((name) => name.includes("its limb packed")),
      LIMB_DISPATCH_KERNEL.name,
      packedSpecial.find((name) => name.includes("limb rays")),
      LIMB_RESOLVE_KERNEL.name,
    ]);
    expect(table).toEqual([undefined, undefined, undefined, undefined]);
    const pairs = [...THICK_LIMB_KERNELS, LIMB_DISPATCH_KERNEL, LIMB_RESOLVE_KERNEL];
    expect(bound).toEqual(
      kernels.map((name) =>
        [...kernelBindings(pairs.find((p) => p.name === name)?.reference ?? "").keys()].toSorted(),
      ),
    );
  });

  it("takes each step's sources from the table per texel", async () => {
    const { kernels } = await marched({ marchSourceTransmittance: "table" });
    expect(kernels).toEqual([RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL.name]);
  });

  it("binds the sources' table, not its stub, to the march from it", async () => {
    const { table } = await marched({ marchSourceTransmittance: "table" });
    expect([
      table[0] === STUB,
      (table[0] ?? "").startsWith("atmosphere sources' transmittance"),
    ]).toEqual([false, true]);
  });

  it("binds the march from the table as it declares", async () => {
    const { bound } = await marched({ marchSourceTransmittance: "table" });
    expect(bound[0]).toEqual(
      [...kernelBindings(RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL.reference).keys()].toSorted(),
    );
  });

  it("packs its limb from the table under both levers", async () => {
    const { kernels } = await marched({
      limbRays: "packed",
      marchSourceTransmittance: "table",
    });
    expect(kernels).toEqual([
      `${RAY_MARCH_PACKED_KERNEL.name}, thick, from the sources' table for one band, edge-free`,
      LIMB_DISPATCH_KERNEL.name,
      `${RAY_MARCH_LIMB_KERNEL.name}, thick, from the sources' table for one band, edge-free`,
      LIMB_RESOLVE_KERNEL.name,
    ]);
  });

  it("binds the packed limb from the table one sources' table, not its stub, in both passes", async () => {
    const { table } = await marched({
      limbRays: "packed",
      marchSourceTransmittance: "table",
    });
    expect([table[0] === STUB, table[2] === table[0]]).toEqual([false, true]);
  });

  it("marches a thick body by slot per step under the table, binding none", async () => {
    const { kernels, table } = await marched(
      { marchSourceTransmittance: "table" },
      DISTANT,
      "by slot",
    );
    expect([kernels, table]).toEqual([[RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL.name], [undefined]]);
  });

  it("marches a thick body of several bands per step under the table, binding none", async () => {
    const { kernels, table } = await marched(
      { marchSourceTransmittance: "table" },
      DISTANT,
      "Saturn",
    );
    expect(kernels).toEqual([RAY_MARCH_THICK_KERNEL.name]);
    expect(table).toEqual([undefined]);
  });

  it("marches from the table per texel inside the air, where no footprint blends", async () => {
    const { kernels } = await marched(
      { limbRays: "packed", marchSourceTransmittance: "table" },
      NEAR,
    );
    expect(kernels).toEqual([RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL.name]);
  });

  it("makes no packed kernel inside the air", async () => {
    const { made } = await marched({ limbRays: "packed", marchSourceTransmittance: "table" }, NEAR);
    expect(made.filter((name) => name.includes("limb"))).toEqual([]);
  });

  it("leaves a body over a deck to the deck's march, per step and per texel, under both levers", async () => {
    const { kernels, table } = await marched(
      { limbRays: "packed", marchSourceTransmittance: "table" },
      DISTANT,
      "deck",
    );
    expect(kernels).toEqual([RAY_MARCH_DECK_SPECIAL_KERNEL.name]);
    expect(table).toEqual([undefined]);
  });

  it.each([
    ["from the table", RAY_MARCH_THICK_TABLE_KERNEL],
    ["from the table, specialised", RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL],
    ["per step", RAY_MARCH_THICK_KERNEL],
  ] as const)("makes the thick read's march %s ahead with the table", async (_way, pair) => {
    const engine = await countingRenderEngine();
    await createAtmosphereFrameKernels(engine, { thick: "with the table" });
    expect(engine.kernelPairs.some((made) => made.name === pair.name)).toBe(true);
  });

  it("makes no thick march from the table ahead where not asked", async () => {
    const engine = await countingRenderEngine();
    await createAtmosphereFrameKernels(engine, { thick: true });
    expect(engine.kernelPairs.some((pair) => pair.name.includes("from the sources' table"))).toBe(
      false,
    );
  });

  it("makes the thick read's packed kernels ahead in the forms asked, both ways", async () => {
    const engine = await countingRenderEngine();
    await createAtmosphereFrameKernels(engine, { reads: "shared read", thickPacked: ["special"] });
    const made = engine.kernelPairs.map((pair) => pair.name);
    const wanted = THICK_LIMB_KERNELS.filter(
      (pair) => pair.name.endsWith("edge-free") && !pair.name.includes("by slot"),
    ).map((pair) => pair.name);
    expect([
      wanted.length,
      made.filter((name) => THICK_LIMB_KERNELS.some((pair) => pair.name === name)).toSorted(),
    ]).toEqual([4, wanted.toSorted()]);
  });
});
