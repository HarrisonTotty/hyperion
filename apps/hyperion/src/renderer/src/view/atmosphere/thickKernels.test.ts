import { describe, expect, it } from "vitest";

import {
  earthBody,
  saturnBody,
  saturnClassMedium,
  SATURN_SPHEROID,
  venusClassMedium,
} from "../../test/atmosphereFixtures";
import { wgslFunctions, wgslReached } from "../../test/atmosphereWgsl";
import { boundNames, countingRenderEngine } from "../../test/countingRenderEngine";
import {
  patternedCorrection,
  patternedDeckTables,
  patternedThickBake,
} from "../../test/thickTables";
import { lookAlong } from "../camera/quaternion";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import type { KernelPair } from "../engine/kernels";
import { kernelBindings } from "../engine/webgpu/compute";
import { fnv1a64 } from "../terrain/workers/f32Digest";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
  referenceSunIlluminanceLx,
} from "./earth";
import {
  AERIAL_PERSPECTIVE_BELOW_DECK_KERNEL,
  AERIAL_PERSPECTIVE_KERNEL,
  AERIAL_PERSPECTIVE_THICK_KERNEL,
  AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL,
  type AtmosphereCamera,
  type AtmosphereScene,
  createAtmosphereFrameKernels,
  HillaireAtmosphere,
  RAY_MARCH_BELOW_DECK_KERNEL,
  RAY_MARCH_CENTRE_KERNEL,
  RAY_MARCH_DECK_KERNEL,
  RAY_MARCH_DECK_SPECIAL_KERNEL,
  RAY_MARCH_DECK_THERMAL_KERNEL,
  RAY_MARCH_FILL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_KERNEL,
  RAY_MARCH_SLOTS_DECK_KERNEL,
  RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  RAY_MARCH_SLOTS_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_KERNEL,
  RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL,
  RAY_MARCH_SPECIAL_KERNEL,
  RAY_MARCH_THICK_KERNEL,
  RAY_MARCH_THICK_SPECIAL_KERNEL,
  RAY_MARCH_THICK_THERMAL_KERNEL,
  SKY_VIEW_BELOW_DECK_KERNEL,
  SKY_VIEW_KERNEL,
  SKY_VIEW_THICK_KERNEL,
  SKY_VIEW_THICK_THERMAL_KERNEL,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
  THICK_READ_WGSL,
  withDeckRead,
  withThickRead,
} from "./hillaire";
import deckReadWgsl from "./shaders/deckRead.wgsl?raw";
import thermalSourceWgsl from "./shaders/thermalSource.wgsl?raw";
import { AtmosphereTables, kernelMedium } from "./tables";
import { upperMediumOf } from "./thick/deckRead";
import { classifyRegime } from "./thick/regime";
import { type MediumTemperature, selfLuminousOf } from "./thermalSource";

/** WGS 84 (NIMA TR8350.2): a = 6,378,137 m, f = 1 ÷ 298.257223563. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
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

/** From 10⁹ m over the equator, looking at the centre: the air a few texels across. */
const DISTANT: AtmosphereCamera = {
  positionM: { x: 1e9, y: 0, z: 0 },
  orientation: lookAlong({ x: -1, y: 0, z: 0 }, { x: 0, y: 0, z: 1 }),
  fovXRad: Math.PI / 3,
  viewport: { widthPx: 320, heightPx: 180 },
};

/** From 2 km over the equator, looking north along the horizon: the sky view and aerial perspective drawn. */
const NEAR: AtmosphereCamera = {
  positionM: { x: 6_378_137 + 2_000, y: 0, z: 0 },
  orientation: lookAlong({ x: 0, y: 0, z: 1 }, { x: 1, y: 0, z: 0 }),
  fovXRad: Math.PI / 2,
  viewport: { widthPx: 64, heightPx: 32 },
};

/** A kernel's source as one line-end form: a checkout's line ends do not move its digest. */
function digestOf(pair: KernelPair): string {
  return fnv1a64(new TextEncoder().encode(pair.reference.replaceAll("\r\n", "\n"))).toString(16);
}

/**
 * The thin kernels' sources as they stood before R08.T14.c (e8b5c6f3), by digest: the thick read
 * is made beside them, and leaves each to the byte (main's ruling on the variants, 2026-10-27). The
 * ray march's are R08.T16.a.2's, whose shares by the light each piece returns and crossings on their
 * edges' levels moved `rayMarch.wgsl` (2026-11-02), and the aerial perspective's and every march's
 * (its fill's too) R08.T9.c's, which stores their transmittance per channel in a second texture
 * (2026-11-02).
 */
const THIN_DIGESTS: ReadonlyArray<readonly [KernelPair, string]> = [
  [SKY_VIEW_KERNEL, "3041f0a7e55328c4"],
  [AERIAL_PERSPECTIVE_KERNEL, "3b76a8ef6a2d9e5"],
  [RAY_MARCH_KERNEL, "60b666946c8ec3bf"],
  [RAY_MARCH_SLOTS_KERNEL, "2fbc833e37385f93"],
  [RAY_MARCH_CENTRE_KERNEL, "2dbea9b9b4e562fb"],
  [RAY_MARCH_SLOTS_CENTRE_KERNEL, "a8338a1299025ef"],
  [RAY_MARCH_SPECIAL_KERNEL, "ff5816a4102485ab"],
  [RAY_MARCH_SLOTS_SPECIAL_KERNEL, "7124fffbe8942acf"],
  [RAY_MARCH_FILL_KERNEL, "f2a36f005c4cf5bd"],
];

/**
 * The thick kernels' sources by digest, as R08.T14.d.4's thick read made them (they stood before
 * R08.T15.a.2 at 1e949d8e, and that read moved them), the ray march's as R08.T16.a.2's march moved
 * them, and the aerial perspective's and the marches' as R08.T9.c's second texture moved them: the
 * deck read is made beside them, and leaves each to the byte, so that a body with no deck draws as
 * before.
 */
const THICK_DIGESTS: ReadonlyArray<readonly [KernelPair, string]> = [
  [SKY_VIEW_THICK_KERNEL, "8d9564b785fc26f6"],
  [AERIAL_PERSPECTIVE_THICK_KERNEL, "1f27c90c96e77121"],
  [RAY_MARCH_THICK_KERNEL, "2f7ca8c342b7f933"],
  [RAY_MARCH_SLOTS_THICK_KERNEL, "1388c6e2294e6ecd"],
  [RAY_MARCH_THICK_SPECIAL_KERNEL, "cb8cb913143f7e87"],
  [RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL, "5e8e8644a2589f71"],
];

/** Each thick kernel beside the thin one it is made from. */
const PAIRS: ReadonlyArray<readonly [string, KernelPair, KernelPair]> = [
  ["the sky view", SKY_VIEW_KERNEL, SKY_VIEW_THICK_KERNEL],
  ["the aerial perspective", AERIAL_PERSPECTIVE_KERNEL, AERIAL_PERSPECTIVE_THICK_KERNEL],
  ["the ray march", RAY_MARCH_KERNEL, RAY_MARCH_THICK_KERNEL],
  ["the ray march by slot", RAY_MARCH_SLOTS_KERNEL, RAY_MARCH_SLOTS_THICK_KERNEL],
];

/** The lines of `thick` that differ from `thin`'s, each as [thin's, thick's], trimmed. */
function linesDiffering(thin: string, thick: string): ReadonlyArray<readonly [string, string]> {
  const ours = thick.split("\n");
  return thin.split("\n").flatMap((line, i) => {
    const theirs = ours[i] ?? "";
    return line === theirs ? [] : [[line.trim(), theirs.trim()] as const];
  });
}

/** The bindings `wgsl` declares that neither its entry point nor any function it reaches names. */
function unreachedBindings(wgsl: string): string[] {
  const functions = wgslFunctions(wgsl);
  const bodies = [...wgslReached(wgsl, ["main"])].map((name) => functions.get(name)?.body ?? "");
  return [...kernelBindings(wgsl).keys()].filter(
    (name) => !bodies.some((body) => new RegExp(`\\b${name}\\b`, "u").test(body)),
  );
}

const SOURCE_AT = "return sourceAt(body, start, light, muSun, slot, local, phased);";
const THICK_SOURCE_AT =
  "return thickSourceAt(body, start, light, muSun, slot, local, phased, ray, dir, up);";
const LOBES = "phased += lobeOrdersOf(local, angle, light.depth);";
const THICK_LOBES =
  "phased += thickAnalyticWeight(muSun) * lobeOrdersOf(local, angle, light.depth);";
const SLOT_SOURCE_AT = "let one = sourceAt(body, start, light, muSun, slot, local, phased);";
const THICK_SLOT_SOURCE_AT =
  "let one = thickSourceAt(body, start, light, muSun, slot, local, phased, ray, dir, up);";

describe("the thick read's kernels (R08.T14.c)", () => {
  it.each(THIN_DIGESTS.map(([pair, digest]) => [pair.name, pair, digest] as const))(
    "leaves %s as it was, to the byte",
    (_name, pair, digest) => {
      expect(digestOf(pair)).toBe(digest);
    },
  );

  it.each(PAIRS)(
    "makes %s's thick form from its own sources, differing only at the multiple scattering and the lobes, with the read appended",
    (_name, thin, thick) => {
      expect(thick.name).toBe(`${thin.name}, thick`);
      const appended = `\n${THICK_READ_WGSL}`;
      expect(thick.reference.endsWith(appended)).toBe(true);
      const head = thick.reference.slice(0, -appended.length);
      const bySlot = thin === RAY_MARCH_SLOTS_KERNEL;
      expect(linesDiffering(thin.reference, head)).toEqual([
        [LOBES, THICK_LOBES],
        [SOURCE_AT, THICK_SOURCE_AT],
        ...(bySlot
          ? [
              [LOBES, THICK_LOBES],
              [SLOT_SOURCE_AT, THICK_SLOT_SOURCE_AT],
            ]
          : []),
      ]);
      // The appended read declares the table alone; every thin binding stays.
      expect([...kernelBindings(thick.reference).keys()]).toEqual([
        ...kernelBindings(thin.reference).keys(),
        "thickTable",
      ]);
      expect(kernelBindings(thick.reference).get("thickTable")).toMatchObject({
        group: 0,
        binding: 24,
        kind: "texture",
        viewDimension: "2d-array",
      });
    },
  );

  it.each([
    ["the ray march", RAY_MARCH_THICK_KERNEL, RAY_MARCH_THICK_SPECIAL_KERNEL],
    ["the ray march by slot", RAY_MARCH_SLOTS_THICK_KERNEL, RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL],
  ] as const)(
    "specialises %s's thick form as the thin one, at the same three lines",
    (_name, general, special) => {
      expect(special.name).toBe(`${general.name} for one band, edge-free`);
      expect(linesDiffering(general.reference, special.reference)).toEqual([
        ["return clamp(u32(body.counts.x), 1u, MAX_SLICES);", "return 1u;"],
        ["return clamp(u32(body.counts.y), 1u, MAX_BANDS);", "return 1u;"],
        [
          "let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);",
          "let edgeCount = min(u32(layerEdges.count.x), 0u);",
        ],
      ]);
    },
  );

  // The engine's compute layouts are "auto": a declared binding the entry point never reaches drops
  // out of the layout, and the bind group, which takes every declared binding, fails.
  it.each([
    ["the sky view", SKY_VIEW_THICK_KERNEL],
    ["the aerial perspective", AERIAL_PERSPECTIVE_THICK_KERNEL],
    ["the ray march", RAY_MARCH_THICK_KERNEL],
    ["the ray march by slot", RAY_MARCH_SLOTS_THICK_KERNEL],
    ["the specialised ray march", RAY_MARCH_THICK_SPECIAL_KERNEL],
    ["the specialised ray march by slot", RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL],
  ] as const)("reaches every binding the thick form of %s declares", (_name, pair) => {
    expect(unreachedBindings(pair.reference)).toEqual([]);
  });

  it("takes the table's bands' count through oblateBandCount, which the specialisation folds", () => {
    const functions = wgslFunctions(RAY_MARCH_THICK_KERNEL.reference);
    expect(functions.get("thickBandsAt")?.body).toMatch(/oblateBandCount\(body\)/u);
    expect(functions.get("thickBandsAt")?.body).not.toMatch(/counts\.y/u);
  });

  it("refuses sources that hold its lines nowhere, or too often", () => {
    const empty = { ...RAY_MARCH_KERNEL, name: "no march", reference: "fn main() {}" };
    expect(() => withThickRead(empty)).toThrow(/^kernel no march holds 0 places for/u);
    const twice = {
      ...SKY_VIEW_KERNEL,
      name: "two skies",
      reference: `${SKY_VIEW_KERNEL.reference}\n${SKY_VIEW_KERNEL.reference}`,
    };
    expect(() => withThickRead(twice)).toThrow(/^kernel two skies holds 2 places for/u);
  });

  it("is catalogued, each thick form beside its thin one", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(
      expect.arrayContaining(
        [
          SKY_VIEW_THICK_KERNEL,
          AERIAL_PERSPECTIVE_THICK_KERNEL,
          RAY_MARCH_THICK_KERNEL,
          RAY_MARCH_SLOTS_THICK_KERNEL,
          RAY_MARCH_THICK_SPECIAL_KERNEL,
          RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL,
        ].map((pair) => pair.name),
      ),
    );
  });
});

const GROUND_LIGHT = "luminance += viewThroughputOf(&march, slot) * source.weight.rgb * toSun";
const DECK_GROUND_LIGHT = "luminance += viewThroughputOf(&march, slot) * source.weight.rgb";
const ALBEDO = "* max(muSun, 0.0) * medium.groundAlbedo.rgb / PI_VIEW;";
const DECK = "* deckGroundAt(body, s, datum.normal, dir, source.direction.xyz, slot, toSun);";

describe("the deck read's kernels (R08.T15.a.2)", () => {
  it.each(THICK_DIGESTS.map(([pair, digest]) => [pair.name, pair, digest] as const))(
    "leaves %s as it was, to the byte",
    (_name, pair, digest) => {
      expect(digestOf(pair)).toBe(digest);
    },
  );

  it.each([
    ["the ray march", RAY_MARCH_THICK_KERNEL, RAY_MARCH_DECK_KERNEL],
    ["the ray march by slot", RAY_MARCH_SLOTS_THICK_KERNEL, RAY_MARCH_SLOTS_DECK_KERNEL],
  ] as const)(
    "makes %s's deck form from its thick one, differing only at the bare ground's light, with the read appended",
    (_name, thick, deck) => {
      expect(deck.name).toBe(`${thick.name}, over a deck`);
      const appended = `\n${deckReadWgsl}`;
      expect(deck.reference.endsWith(appended)).toBe(true);
      const head = deck.reference.slice(0, -appended.length);
      expect(linesDiffering(thick.reference, head)).toEqual([
        [GROUND_LIGHT, DECK_GROUND_LIGHT],
        [ALBEDO, DECK],
      ]);
      expect([...kernelBindings(deck.reference).keys()]).toEqual([
        ...kernelBindings(thick.reference).keys(),
        "deckTable",
      ]);
      expect(kernelBindings(deck.reference).get("deckTable")).toMatchObject({
        group: 0,
        binding: 25,
        kind: "texture",
        viewDimension: "2d-array",
      });
    },
  );

  it.each([
    ["the ray march", RAY_MARCH_DECK_KERNEL, RAY_MARCH_DECK_SPECIAL_KERNEL],
    ["the ray march by slot", RAY_MARCH_SLOTS_DECK_KERNEL, RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL],
  ] as const)(
    "specialises %s's deck form as the thick one, at the same three lines",
    (_name, general, special) => {
      expect(special.name).toBe(`${general.name} for one band, edge-free`);
      expect(linesDiffering(general.reference, special.reference)).toHaveLength(3);
    },
  );

  it.each([
    ["the ray march", RAY_MARCH_DECK_KERNEL],
    ["the ray march by slot", RAY_MARCH_SLOTS_DECK_KERNEL],
    ["the specialised ray march", RAY_MARCH_DECK_SPECIAL_KERNEL],
    ["the specialised ray march by slot", RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL],
  ] as const)("reaches every binding the deck form of %s declares", (_name, pair) => {
    expect(unreachedBindings(pair.reference)).toEqual([]);
  });

  it("takes the deck's bands' count through oblateBandCount, which the specialisation folds", () => {
    const functions = wgslFunctions(RAY_MARCH_DECK_KERNEL.reference);
    expect(functions.get("deckGroundAt")?.body).toMatch(/oblateBandCount\(body\)/u);
  });

  it("refuses a kernel that reads no thick table, or holds the bare ground's light other than once", () => {
    expect(() => withDeckRead(RAY_MARCH_KERNEL)).toThrow(/reads no thick table/u);
    const empty = {
      ...RAY_MARCH_THICK_KERNEL,
      name: "no ground",
      reference: `${THICK_READ_WGSL}\nfn main() {}`,
    };
    expect(() => withDeckRead(empty)).toThrow(/^kernel no ground holds 0 places for/u);
  });

  it("is catalogued, each deck form beside its thick one", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(
      expect.arrayContaining(
        [
          RAY_MARCH_DECK_KERNEL,
          RAY_MARCH_SLOTS_DECK_KERNEL,
          RAY_MARCH_DECK_SPECIAL_KERNEL,
          RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL,
        ].map((pair) => pair.name),
      ),
    );
  });
});

/** R05's Earth over a deck 3 km up: its upper medium, as the kernels read it over the deck. */
const DECK_ALTITUDE_M = 3_000;
const EARTH_ABOVE_DECK = upperMediumOf(R05_EARTH_REFERENCE, DECK_ALTITUDE_M);

describe("AtmosphereTables' deck table (R08.T15.a.2)", () => {
  it("holds a deck and its J above as the thick table, counts their bytes, and gives both up on a rebuild or a release", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_ABOVE_DECK, tableRadiusM(WGS84));
    const before = tables.rowBytes;
    expect(tables.deckTable).toBeNull();
    const deck = patternedDeckTables(
      tables.plan.bands,
      kernelMedium(tables.medium),
      2,
      DECK_ALTITUDE_M,
    );
    tables.setDeckTables(deck);
    expect([tables.deckBytes, tables.thickBytes]).toEqual([2 * 159_744, 2 * 388_864]);
    expect(tables.rowBytes).toBe(before + 2 * 159_744 + 2 * 388_864);
    expect(tables.deckPacked?.slots).toBe(2);
    expect(tables.thickPacked?.altitudesM[0]).toBe(0);
    expect(
      engine.textureWritten.findLast((w) => w.texture === "atmosphere deck table"),
    ).toBeDefined();
    tables.setDeckTables(null);
    expect([tables.deckTable, tables.thickTable, tables.deckBytes]).toEqual([null, null, 0]);
    tables.setDeckTables(deck);
    engine.loseDevice();
    engine.restore();
    expect(tables.deckTable).not.toBeNull();
    expect(tables.deckBytes).toBe(2 * 159_744);
    tables.setMedium({ ...EARTH_ABOVE_DECK, groundAlbedo: [0.2, 0.2, 0.2] }, tableRadiusM(WGS84));
    expect([tables.deckTable, tables.thickTable]).toEqual([null, null]);
    tables.dispose();
    expect(() => tables.setDeckTables(null)).toThrow(/disposed/u);
  });

  it("refuses a deck of other bands than the plan's, keeping what it held", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_ABOVE_DECK, tableRadiusM(WGS84));
    const bands = tables.plan.bands.map((band) => ({
      ...band,
      gravityRatio: band.gravityRatio * 1.01,
    }));
    expect(() =>
      tables.setDeckTables(
        patternedDeckTables(bands, kernelMedium(tables.medium), 1, DECK_ALTITUDE_M),
      ),
    ).toThrow(/is not the tables'/u);
    expect(tables.deckTable).toBeNull();
    tables.dispose();
  });
});

describe("HillaireAtmosphere over a deck (R08.T15.a.2)", () => {
  it("draws its march by the deck's kernels, binding the deck's table, and its sky by the thick ones", async () => {
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const tables = new AtmosphereTables(
      engine,
      EARTH_ABOVE_DECK,
      tableRadiusM(WGS84),
      [],
      earthBody(),
    );
    tables.setDeckTables(
      patternedDeckTables(tables.plan.bands, kernelMedium(tables.medium), 1, DECK_ALTITUDE_M),
    );
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    for (const [cover, march] of [
      ["area", RAY_MARCH_DECK_SPECIAL_KERNEL],
      ["whole frame", RAY_MARCH_DECK_KERNEL],
    ] as const) {
      view.setMarchCover(cover);
      engine.dispatched.length = 0;
      view.drawFrame(NEAR, [NOON], SCENE);
      const atmosphere = engine.dispatched.filter((d) => d.kernel.startsWith("atmosphere "));
      const names = atmosphere.map((d) => d.kernel);
      expect(names.filter((name) => name.startsWith("atmosphere ray march"))).toEqual([march.name]);
      expect(names).toEqual(
        expect.arrayContaining([SKY_VIEW_THICK_KERNEL.name, AERIAL_PERSPECTIVE_THICK_KERNEL.name]),
      );
      const marched = atmosphere.find((d) => d.kernel === march.name);
      expect(
        boundNames(marched?.bindings ?? { buffers: {}, sampled: {}, uniforms: {}, storage: {} }),
      ).toEqual([...kernelBindings(march.reference).keys()].toSorted());
    }
  });
});

/** The kernels beneath a deck (R08.T15.b.2), each with its name in the checks. */
const BELOW_DECK_KERNELS: ReadonlyArray<readonly [string, KernelPair]> = [
  ["the sky view", SKY_VIEW_BELOW_DECK_KERNEL],
  ["the aerial perspective", AERIAL_PERSPECTIVE_BELOW_DECK_KERNEL],
  ["the ray march", RAY_MARCH_BELOW_DECK_KERNEL],
];

describe("the kernels beneath a deck (R08.T15.b.2)", () => {
  it.each(BELOW_DECK_KERNELS)(
    "reads %s's light from the table below the deck, declaring no medium and reaching every binding",
    (_name, pair) => {
      const bindings = kernelBindings(pair.reference);
      expect([
        bindings.get("belowDeckTable"),
        bindings.has("medium"),
        unreachedBindings(pair.reference),
      ]).toEqual([
        expect.objectContaining({ kind: "texture", viewDimension: "2d-array" }),
        false,
        [],
      ]);
    },
  );

  it("is catalogued beside the deck's marches", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(expect.arrayContaining(BELOW_DECK_KERNELS.map(([, pair]) => pair.name)));
  });
});

describe("AtmosphereTables' table below a deck (R08.T15.b.2)", () => {
  it("holds a deck over a surface's table below, of its whole column, counts its bytes, and gives it up with the deck", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_ABOVE_DECK, tableRadiusM(WGS84));
    const before = tables.rowBytes;
    const stub = tables.belowDeckLighting;
    const over = patternedDeckTables(
      tables.plan.bands,
      kernelMedium(tables.medium),
      2,
      DECK_ALTITUDE_M,
      true,
    );
    const column = kernelMedium(R05_EARTH_REFERENCE);
    expect(() => tables.setDeckTables(over)).toThrow(/reads its whole column/u);
    tables.setDeckTables(over, column);
    expect([tables.belowDeckBytes, tables.rowBytes - before]).toEqual([
      2 * 436_800,
      2 * (436_800 + 159_744 + 388_864),
    ]);
    expect(tables.belowDeckLighting).toBe(tables.belowDeckTable);
    expect(
      engine.textureWritten.findLast((w) => w.texture === "atmosphere table below a deck"),
    ).toBeDefined();
    engine.loseDevice();
    engine.restore();
    expect([tables.belowDeckTable === null, tables.belowDeckBytes]).toEqual([false, 2 * 436_800]);
    // An envelope's deck has none, and the lit passes bind the stub.
    tables.setDeckTables(
      patternedDeckTables(tables.plan.bands, kernelMedium(tables.medium), 2, DECK_ALTITUDE_M),
    );
    expect([tables.belowDeckTable, tables.belowDeckBytes]).toEqual([null, 0]);
    expect(tables.belowDeckLighting.name).toBe(stub.name);
    tables.setDeckTables(over, column);
    tables.setDeckTables(null);
    expect([tables.belowDeckTable, tables.belowDeckBytes]).toEqual([null, 0]);
    tables.dispose();
  });
});

/** From 1 km under the tables' datum, the deck's, looking north along the horizon. */
const BENEATH: AtmosphereCamera = {
  ...NEAR,
  positionM: { x: 6_378_137 - 1_000, y: 0, z: 0 },
};

describe("HillaireAtmosphere beneath a deck (R08.T15.b.2)", () => {
  it("draws a camera under the deck by the kernels that read the table below, binding it, and one above it by the deck's", async () => {
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const tables = new AtmosphereTables(
      engine,
      EARTH_ABOVE_DECK,
      tableRadiusM(WGS84),
      [],
      earthBody(),
    );
    tables.setDeckTables(
      patternedDeckTables(tables.plan.bands, kernelMedium(tables.medium), 1, DECK_ALTITUDE_M, true),
      kernelMedium(R05_EARTH_REFERENCE),
    );
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    const drawn = (camera: AtmosphereCamera): ReadonlyArray<string> => {
      engine.dispatched.length = 0;
      view.drawFrame(camera, [NOON], SCENE);
      return engine.dispatched
        .filter((d) => d.kernel.startsWith("atmosphere ") && !d.kernel.includes("fill"))
        .map((d) => d.kernel);
    };
    expect(drawn(BENEATH).toSorted()).toEqual(
      BELOW_DECK_KERNELS.map(([, pair]) => pair.name).toSorted(),
    );
    for (const [, pair] of BELOW_DECK_KERNELS) {
      const dispatch = engine.dispatched.find((d) => d.kernel === pair.name);
      expect(
        boundNames(dispatch?.bindings ?? { buffers: {}, sampled: {}, uniforms: {}, storage: {} }),
      ).toEqual([...kernelBindings(pair.reference).keys()].toSorted());
    }
    expect(drawn(NEAR)).toEqual(
      expect.arrayContaining([SKY_VIEW_THICK_KERNEL.name, AERIAL_PERSPECTIVE_THICK_KERNEL.name]),
    );
    expect(drawn(NEAR).some((name) => name.includes("beneath a deck"))).toBe(false);
  });
});

/** Each deck march's thermal form beside the deck form it is made from (R08.T15.b.2). */
const DECK_THERMAL_PAIRS: ReadonlyArray<readonly [string, KernelPair, KernelPair]> = [
  ["the ray march", RAY_MARCH_DECK_KERNEL, RAY_MARCH_DECK_THERMAL_KERNEL],
  ["the ray march by slot", RAY_MARCH_SLOTS_DECK_KERNEL, RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL],
];

describe("the deck's marches' thermal forms (R08.T15.b.2)", () => {
  it.each(DECK_THERMAL_PAIRS)(
    "makes %s's from its deck form, its thermal source at binding 26 beside the deck's table at 25",
    (_name, deck, thermal) => {
      const bindings = kernelBindings(thermal.reference);
      expect([
        thermal.name,
        thermal.reference.endsWith(
          `\n${thermalSourceWgsl.replace("@binding(25)", "@binding(26)")}`,
        ),
        [...bindings.keys()],
        bindings.get("deckTable")?.binding,
        bindings.get("thermalSource")?.binding,
        unreachedBindings(thermal.reference),
      ]).toEqual([
        `${deck.name}, thermal`,
        true,
        [...kernelBindings(deck.reference).keys(), "thermalSource"],
        25,
        26,
        [],
      ]);
    },
  );

  it("leaves every other thermal form's source at binding 25", () => {
    expect(
      kernelBindings(RAY_MARCH_THICK_THERMAL_KERNEL.reference).get("thermalSource"),
    ).toMatchObject({ group: 0, binding: 25 });
  });

  it("is catalogued beside the deck's marches", () => {
    const names = WGSL_CATALOGUE.flatMap((entry) =>
      entry.kind === "compute" ? [entry.spec.name] : [],
    );
    expect(names).toEqual(
      expect.arrayContaining(DECK_THERMAL_PAIRS.map(([, , thermal]) => thermal.name)),
    );
  });
});

/** Venus's solar irradiance, W m⁻²: 2,601.3 (NASA's Venus Fact Sheet). */
const VENUS_SOLAR_IRRADIANCE_W_M2 = 2_601.3;

/**
 * A Venus-class column's temperature, hottest at its ground, `groundK`, falling linearly to 230 K at
 * 100 km: a test's profile, not fitted to Venus, of which only the peak at the ground decides the
 * cut.
 */
function venusClassTemperature(groundK: number): MediumTemperature {
  return { heightsM: Float64Array.of(0, 100_000), temperaturesK: Float64Array.of(groundK, 230) };
}

describe("HillaireAtmosphere over a deck, self-luminous (R08.T15.b.2)", () => {
  const venus = venusClassMedium();

  it("takes a Venus-class world at 1,000 K for self-luminous, and the Venus-class fixture at its 730 K not", () => {
    expect(
      [730, 1_000].map((groundK) =>
        selfLuminousOf(venus.medium, venusClassTemperature(groundK), VENUS_SOLAR_IRRADIANCE_W_M2),
      ),
    ).toEqual([false, true]);
  });

  it.each([
    ["made at its first frame", {}],
    ["made ahead", { thick: true, thermal: true }],
  ] as const)(
    "draws a Venus-class world at 1,000 K over its deck by the deck's thermal march under every cover, its sky by the thick read's thermal forms, the kernels %s",
    async (_made, ahead) => {
      const engine = await countingRenderEngine();
      const kernels = await createAtmosphereFrameKernels(engine, ahead);
      const deckCover = {
        cloudFraction: 1,
        decks: [{ opticalDepth550: 30, baseAltitudeM: 45_000, topAltitudeM: 73_000 }],
        envelope: false,
      };
      const regime = classifyRegime(venus.medium, deckCover);
      const splitM = regime.kind === "cloudDeck" ? regime.deckAltitudeM : Number.NaN;
      const upper = upperMediumOf(venus.medium, splitM);
      const tables = new AtmosphereTables(engine, upper, venus.radiusM + splitM);
      tables.setDeckTables(patternedDeckTables(tables.plan.bands, kernelMedium(upper), 1, splitM));
      tables.setThermalSource(venusClassTemperature(1_000));
      const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
      const camera: AtmosphereCamera = {
        ...NEAR,
        positionM: { x: venus.radiusM + splitM + 2_000, y: 0, z: 0 },
      };
      const march = RAY_MARCH_DECK_THERMAL_KERNEL;
      for (const cover of ["area", "whole frame", "tiles"] as const) {
        view.setMarchCover(cover);
        engine.dispatched.length = 0;
        view.drawFrame(camera, [NOON], SCENE);
        const atmosphere = engine.dispatched.filter((d) => d.kernel.startsWith("atmosphere "));
        const names = atmosphere.map((d) => d.kernel);
        const marched = atmosphere.find((d) => d.kernel === march.name);
        expect([
          names.filter((name) => name.startsWith("atmosphere ray march")),
          names,
          names.includes(SKY_VIEW_THICK_KERNEL.name),
          boundNames(marched?.bindings ?? { buffers: {}, sampled: {}, uniforms: {}, storage: {} }),
        ]).toEqual([
          [march.name],
          expect.arrayContaining([
            SKY_VIEW_THICK_THERMAL_KERNEL.name,
            AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL.name,
          ]),
          false,
          [...kernelBindings(march.reference).keys()].toSorted(),
        ]);
      }
    },
  );

  it("draws the measured Earth's air over a deck at 3,000 K, whose ozone's curves give it slot layers, by the deck's thermal march by slot", async () => {
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const upper = upperMediumOf(EARTH_REFERENCE, DECK_ALTITUDE_M);
    const tables = new AtmosphereTables(
      engine,
      upper,
      tableRadiusM(WGS84),
      EARTH_REFERENCE_CURVES,
      earthBody(),
    );
    tables.setDeckTables(
      patternedDeckTables(
        tables.plan.bands,
        kernelMedium(upper),
        EARTH_REFERENCE_CURVES.length,
        DECK_ALTITUDE_M,
      ),
    );
    tables.setThermalSource({
      heightsM: Float64Array.of(0, 1e6),
      temperaturesK: Float64Array.of(3_000, 3_000),
    });
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    engine.dispatched.length = 0;
    view.drawFrame(NEAR, [NOON], SCENE);
    const marches = engine.dispatched
      .map((d) => d.kernel)
      .filter((name) => name.startsWith("atmosphere ray march"));
    expect([tables.slotDepths === null, marches]).toEqual([
      false,
      [RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL.name],
    ]);
  });
});

describe("AtmosphereTables' thick table (R08.T14.c)", () => {
  it("holds a bake of the plan's bands, counts its bytes, and gives it up on a rebuild or a release", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const before = tables.rowBytes;
    expect(tables.thickTable).toBeNull();
    tables.setThickTables(patternedThickBake(tables.plan.bands, kernelMedium(tables.medium), 2));
    expect(tables.thickBytes).toBe(2 * 388_864);
    expect(tables.rowBytes).toBe(before + 2 * 388_864);
    expect(tables.thickTable).not.toBeNull();
    expect(tables.thickPacked?.slots).toBe(2);
    const written = engine.textureWritten.findLast((w) => w.texture === "atmosphere thick table");
    expect(written).toBeDefined();
    tables.setThickTables(null);
    expect([tables.thickTable, tables.thickBytes]).toEqual([null, 0]);
    tables.setThickTables(patternedThickBake(tables.plan.bands, kernelMedium(tables.medium), 1));
    // Another medium's build takes it away: it belongs to the medium it was baked for.
    tables.setMedium(
      { ...R05_EARTH_REFERENCE, groundAlbedo: [0.2, 0.2, 0.2] },
      tableRadiusM(WGS84),
    );
    expect(tables.thickTable).toBeNull();
    tables.dispose();
    expect(() => tables.setThickTables(null)).toThrow(/disposed/u);
  });

  it("refuses a bake of other bands than the plan's", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(
      engine,
      saturnClassMedium(),
      tableRadiusM(SATURN_SPHEROID),
      [],
      saturnBody(),
    );
    const bands = tables.plan.bands;
    expect(bands.length).toBeGreaterThan(1);
    const medium = kernelMedium(tables.medium);
    expect(() => tables.setThickTables(patternedThickBake(bands.slice(1), medium, 1))).toThrow(
      /is not the tables'/u,
    );
    expect(() =>
      tables.setThickTables(
        patternedThickBake(
          bands.map((band) => ({
            gravityRatio: band.gravityRatio * 1.01,
            latitudeRad: band.latitudeRad,
            bottomRadiusM: band.bottomRadiusM,
            coefficientFactor: band.coefficientFactor,
          })),
          medium,
          1,
        ),
      ),
    ).toThrow(/is not the tables'/u);
    expect(tables.thickTable).toBeNull();
    tables.dispose();
  });

  it("uploads the table anew after a restore", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    tables.setThickTables(patternedThickBake(tables.plan.bands, kernelMedium(tables.medium), 1));
    const held = tables.thickTable;
    engine.loseDevice();
    engine.restore();
    expect(tables.thickTable).not.toBeNull();
    expect(tables.thickTable).not.toBe(held);
    expect(tables.thickBytes).toBe(388_864);
    tables.dispose();
  });
});

describe("AtmosphereTables' polarisation correction (R08.T14.f)", () => {
  it("holds a correction alone as a correction table, and summed into a bake as a thick one, of the bake's bytes", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const before = tables.rowBytes;
    const bands = tables.plan.bands;
    expect(tables.holdsPolarisationCorrection).toBe(false);
    tables.setPolarisationCorrection(patternedCorrection(bands, tables.medium, 2));
    expect(tables.thickPacked?.kind).toBe("correction");
    expect(tables.holdsPolarisationCorrection).toBe(true);
    expect(tables.rowBytes).toBe(before + 2 * 388_864);
    tables.setThickTables(patternedThickBake(bands, kernelMedium(tables.medium), 2));
    expect(tables.thickPacked?.kind).toBe("thick");
    expect(tables.thickBytes).toBe(2 * 388_864);
    // The bake taken away leaves the correction alone; the correction taken away, nothing.
    tables.setThickTables(null);
    expect(tables.thickPacked?.kind).toBe("correction");
    tables.setPolarisationCorrection(null);
    expect([tables.thickTable, tables.thickBytes, tables.holdsPolarisationCorrection]).toEqual([
      null,
      0,
      false,
    ]);
    tables.dispose();
    expect(() => tables.setPolarisationCorrection(null)).toThrow(/disposed/u);
  });

  it("refuses a correction of other bands or light slots, keeping what it held", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const bands = tables.plan.bands;
    tables.setThickTables(patternedThickBake(bands, kernelMedium(tables.medium), 2));
    const held = tables.thickTable;
    expect(() =>
      tables.setPolarisationCorrection(
        patternedCorrection(
          bands.map((band) => ({ ...band, gravityRatio: band.gravityRatio * 1.01 })),
          tables.medium,
          2,
        ),
      ),
    ).toThrow(/is not the tables'/u);
    expect(() =>
      tables.setPolarisationCorrection(patternedCorrection(bands, tables.medium, 1)),
    ).toThrow(/light slots/u);
    expect(tables.thickTable).toBe(held);
    expect(tables.holdsPolarisationCorrection).toBe(false);
    tables.dispose();
  });

  it("forgets both on a rebuild for another medium, and keeps both across a restore", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, tableRadiusM(WGS84));
    const bands = tables.plan.bands;
    tables.setThickTables(patternedThickBake(bands, kernelMedium(tables.medium), 1));
    tables.setPolarisationCorrection(patternedCorrection(bands, tables.medium, 1));
    engine.loseDevice();
    engine.restore();
    expect(tables.thickPacked?.kind).toBe("thick");
    // The bake is still held after the restore: taking the correction away leaves it.
    tables.setPolarisationCorrection(null);
    expect(tables.thickPacked?.kind).toBe("thick");
    tables.setPolarisationCorrection(patternedCorrection(bands, tables.medium, 1));
    tables.setMedium(
      { ...R05_EARTH_REFERENCE, groundAlbedo: [0.2, 0.2, 0.2] },
      tableRadiusM(WGS84),
    );
    expect([tables.thickTable, tables.holdsPolarisationCorrection]).toEqual([null, false]);
    tables.dispose();
  });
});

describe("HillaireAtmosphere over a thick table (R08.T14.c)", () => {
  /** The march's and the sky's dispatches, by kernel name. */
  async function drawn(
    withThick: boolean | "correction",
    cover: "area" | "whole frame" | "tiles",
    camera: AtmosphereCamera = DISTANT,
  ): Promise<{ kernels: string[]; bound: string[][] }> {
    const engine = await countingRenderEngine();
    const kernels = await createAtmosphereFrameKernels(engine);
    const tables = new AtmosphereTables(
      engine,
      R05_EARTH_REFERENCE,
      tableRadiusM(WGS84),
      [],
      earthBody(),
    );
    if (withThick === "correction") {
      tables.setPolarisationCorrection(patternedCorrection(tables.plan.bands, tables.medium, 1));
    } else if (withThick) {
      tables.setThickTables(patternedThickBake(tables.plan.bands, kernelMedium(tables.medium), 1));
    }
    const view = HillaireAtmosphere.over(engine, tables, TABLE_SIZES.high, kernels);
    view.setMarchCover(cover);
    engine.dispatched.length = 0;
    view.drawFrame(camera, [NOON], SCENE);
    const atmosphere = engine.dispatched.filter((d) => d.kernel.startsWith("atmosphere "));
    return {
      kernels: atmosphere.map((d) => d.kernel),
      bound: atmosphere.map((d) => boundNames(d.bindings)),
    };
  }

  it("draws a body without one by the thin kernels, as before", async () => {
    const { kernels } = await drawn(false, "area");
    expect(kernels).not.toContain(SKY_VIEW_THICK_KERNEL.name);
    expect(kernels).toContain(RAY_MARCH_SPECIAL_KERNEL.name);
  });

  it("draws a body with one by the thick kernels, the specialised march over its area, binding the table", async () => {
    const { kernels, bound } = await drawn(true, "area", NEAR);
    const march = kernels.filter((name) => name.startsWith("atmosphere ray march"));
    expect(march).toEqual([RAY_MARCH_THICK_SPECIAL_KERNEL.name]);
    expect(kernels).toEqual(
      expect.arrayContaining([SKY_VIEW_THICK_KERNEL.name, AERIAL_PERSPECTIVE_THICK_KERNEL.name]),
    );
    const thick = [
      SKY_VIEW_THICK_KERNEL,
      AERIAL_PERSPECTIVE_THICK_KERNEL,
      RAY_MARCH_THICK_SPECIAL_KERNEL,
    ];
    const declared = kernels.flatMap((name, k) => {
      const pair = thick.find((p) => p.name === name);
      return pair === undefined
        ? []
        : [[bound[k], [...kernelBindings(pair.reference).keys()].toSorted()] as const];
    });
    expect(declared).toHaveLength(3);
    expect(declared.map(([given]) => given)).toEqual(declared.map(([, wanted]) => wanted));
  });

  it("draws a thin body with a correction alone by the thick kernels too, binding the table (R08.T14.f)", async () => {
    const { kernels, bound } = await drawn("correction", "area", NEAR);
    expect(kernels.filter((name) => name.startsWith("atmosphere ray march"))).toEqual([
      RAY_MARCH_THICK_SPECIAL_KERNEL.name,
    ]);
    expect(kernels).toEqual(
      expect.arrayContaining([SKY_VIEW_THICK_KERNEL.name, AERIAL_PERSPECTIVE_THICK_KERNEL.name]),
    );
    expect(
      bound.every(
        (names, k) => !(kernels[k] ?? "").includes("thick") || names.includes("thickTable"),
      ),
    ).toBe(true);
  });

  it("takes no tile cull for a thick body, and the general thick march on the reference path", async () => {
    const tiles = await drawn(true, "tiles");
    expect(tiles.kernels.filter((name) => name.startsWith("atmosphere ray march"))).toEqual([
      RAY_MARCH_THICK_KERNEL.name,
    ]);
    const whole = await drawn(true, "whole frame");
    expect(whole.kernels.filter((name) => name.startsWith("atmosphere ray march"))).toEqual([
      RAY_MARCH_THICK_KERNEL.name,
    ]);
  });
});
