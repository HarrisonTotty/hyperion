import { describe, expect, it } from "vitest";

import {
  earthBody,
  saturnBody,
  saturnClassMedium,
  SATURN_SPHEROID,
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
import { R05_EARTH_REFERENCE, referenceSunIlluminanceLx } from "./earth";
import {
  AERIAL_PERSPECTIVE_KERNEL,
  AERIAL_PERSPECTIVE_THICK_KERNEL,
  type AtmosphereCamera,
  type AtmosphereScene,
  createAtmosphereFrameKernels,
  HillaireAtmosphere,
  RAY_MARCH_CENTRE_KERNEL,
  RAY_MARCH_DECK_KERNEL,
  RAY_MARCH_DECK_SPECIAL_KERNEL,
  RAY_MARCH_FILL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_KERNEL,
  RAY_MARCH_SLOTS_DECK_KERNEL,
  RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  RAY_MARCH_SLOTS_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_KERNEL,
  RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL,
  RAY_MARCH_SPECIAL_KERNEL,
  RAY_MARCH_THICK_KERNEL,
  RAY_MARCH_THICK_SPECIAL_KERNEL,
  SKY_VIEW_KERNEL,
  SKY_VIEW_THICK_KERNEL,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
  withDeckRead,
  withThickRead,
} from "./hillaire";
import deckReadWgsl from "./shaders/deckRead.wgsl?raw";
import thickReadWgsl from "./shaders/thickRead.wgsl?raw";
import { AtmosphereTables, kernelMedium } from "./tables";
import { upperMediumOf } from "./thick/deckRead";

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
 * is made beside them, and leaves each to the byte (main's ruling on the variants, 2026-10-27).
 */
const THIN_DIGESTS: ReadonlyArray<readonly [KernelPair, string]> = [
  [SKY_VIEW_KERNEL, "3041f0a7e55328c4"],
  [AERIAL_PERSPECTIVE_KERNEL, "21a417584d1b1791"],
  [RAY_MARCH_KERNEL, "9bf89a1fa995e8a3"],
  [RAY_MARCH_SLOTS_KERNEL, "25c2ee4c4dd2e21f"],
  [RAY_MARCH_CENTRE_KERNEL, "403cb2484fb9ab4f"],
  [RAY_MARCH_SLOTS_CENTRE_KERNEL, "aa25911b102bb013"],
  [RAY_MARCH_SPECIAL_KERNEL, "6138c6f9bc1f0b73"],
  [RAY_MARCH_SLOTS_SPECIAL_KERNEL, "e6d546ee780e86af"],
  [RAY_MARCH_FILL_KERNEL, "c4ece515222b45bc"],
];

/**
 * The thick kernels' sources as they stood before R08.T15.a.2 (1e949d8e), by digest: the deck read
 * is made beside them, and leaves each to the byte, so that a body with no deck draws as before.
 */
const THICK_DIGESTS: ReadonlyArray<readonly [KernelPair, string]> = [
  [SKY_VIEW_THICK_KERNEL, "2f0225028c051361"],
  [AERIAL_PERSPECTIVE_THICK_KERNEL, "1ebf84e4c2889818"],
  [RAY_MARCH_THICK_KERNEL, "36ae5050caf9561e"],
  [RAY_MARCH_SLOTS_THICK_KERNEL, "5e1a2b29be3e2d70"],
  [RAY_MARCH_THICK_SPECIAL_KERNEL, "7ac9ad838b192e4e"],
  [RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL, "6704cba1da82e660"],
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
      const appended = `\n${thickReadWgsl}`;
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
      reference: `${thickReadWgsl}\nfn main() {}`,
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
