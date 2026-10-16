import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { brdf, brdfFromTable } from "../appearance/brdf";
import {
  mcewenLunarProfile,
  PHASE_TABLE_SAMPLES,
  PHASE_TABLE_STEP_RAD,
  type PhaseFactorTable,
  phaseFactorTableOf,
  type PhotometricLaw,
} from "../appearance/law";
import { lawPhaseIntegral, withTemplateCurve } from "../appearance/phase";
import { oblateAlbedoScale } from "../bodies/oblate";
import { WGSL_CATALOGUE } from "../engine/catalogue";
import type { WgslMaterialSpec } from "../engine/types";
import { annulusEdges } from "../lighting/annuli";
import terrainLitWgsl from "./shaders/terrainLit.wgsl?raw";
import horizonWgsl from "./shadows/horizon.wgsl?raw";
import { CLASS_LAWS_BYTES, CLASS_LAYERS_BYTES, CLASS_WEIGHTS_BYTES } from "./slotLayout";
import {
  AIRLESS_ICE_EXPONENT,
  CLASS_LAW_ROWS,
  CLASS_LAW_TEMPLATES,
  classPhotometryOf,
  classRowLaw,
  classTexelLaw,
  dominantSlot,
  packClassLaws,
  packTerrainLighting,
  TERRAIN_CLASSES,
  TERRAIN_LIGHT_ROWS,
  TERRAIN_LIT_BINDINGS,
  TERRAIN_LIT_MATERIALS,
  PHASE_LAW,
  type TerrainLighting,
  TerrainPhaseRows,
  terrainLitMaterialSpec,
  terrainReflectance,
  terrainShares,
  texelLawOf,
  texelWeights,
} from "./terrainMaterial";

const RAD = Math.PI / 180;

/** A bake's photometry: each slot `[B, V, R, law, L(0)]`, the rest empty. */
function photometryOf(slots: ReadonlyArray<readonly [number, number, number, number, number]>) {
  const out = new Float32Array(TERRAIN_CLASSES * 5);
  slots.forEach((slot, k) => {
    out.set(slot, 5 * k);
  });
  return out;
}

/** A material's storage buffers' names. */
function storageNames(spec: WgslMaterialSpec): string[] {
  return (spec.storageBuffers ?? []).map((buffer) => buffer.name);
}

describe("the lit terrain's material (R10.T10.b)", () => {
  it("registers both vertex paths' lit materials in the catalogue", () => {
    for (const spec of TERRAIN_LIT_MATERIALS) {
      expect(WGSL_CATALOGUE.some((entry) => entry.kind === "material" && entry.spec === spec)).toBe(
        true,
      );
    }
    expect(TERRAIN_LIT_MATERIALS.map((spec) => spec.displayName)).toEqual([
      "LIT TERRAIN OFFSETS",
      "LIT TERRAIN",
    ]);
  });

  it("declares each binding its WGSL reads, the offsets on the baked-offsets path alone", () => {
    const offsets = terrainLitMaterialSpec("baked-offsets");
    const differences = terrainLitMaterialSpec("face-differences");
    expect(storageNames(offsets)).toContain("offsets");
    expect(storageNames(differences)).not.toContain("offsets");
    expect(storageNames(differences)).toEqual([
      "heights",
      "slots",
      "instances",
      "contacts",
      "classLayers",
      "terrainLights",
      "horizonMap",
      "surveyMask",
    ]);
    // Nine storage buffers on the baked-offsets path, five read by the vertex stage alone, three by
    // the fragment stage alone, and the survey mask by both (decision-r10-t10b-r09-t2e §5;
    // R10.T10.c).
    expect(offsets.storageBuffers).toHaveLength(9);
    expect(
      (offsets.storageBuffers ?? []).map((buffer) => [buffer.name, buffer.stages?.join(" ")]),
    ).toEqual([
      ["heights", "vertex"],
      ["slots", "vertex"],
      ["instances", "vertex"],
      ["contacts", "vertex"],
      ["offsets", "vertex"],
      ["classLayers", "fragment"],
      ["terrainLights", "fragment"],
      ["horizonMap", "fragment"],
      ["surveyMask", "vertex fragment"],
    ]);
    const textures = differences.textures ?? [];
    expect(textures.map((t) => t.binding)).toEqual([4, 7, 9, 10, 11, 12, 13, 14, 15]);
    for (const [name, binding] of [
      ["class_layers", TERRAIN_LIT_BINDINGS.classLayers],
      ["phase_factor_table", TERRAIN_LIT_BINDINGS.phaseFactorTable],
      ["terrain_lights", TERRAIN_LIT_BINDINGS.terrainLights],
      ["atmosphere_body", TERRAIN_LIT_BINDINGS.atmosphere],
      ["horizon_map", TERRAIN_LIT_BINDINGS.horizonMap],
      ["survey_mask", TERRAIN_LIT_BINDINGS.surveyMask],
    ] as const) {
      expect(terrainLitWgsl + horizonWgsl).toMatch(
        new RegExp(`@binding\\(${String(binding)}\\) var[^\\n]* ${name}`, "u"),
      );
    }
  });

  it("states the hooks R11 fills, each with no effect", () => {
    for (const hook of [
      "fn terrain_decoration(p : vec3f, normal : vec3f) -> TerrainDecoration",
      "fn terrain_shadow_factor(p : vec3f) -> f32",
      "fn terrain_ring_shadow(p : vec3f, sun : vec3f) -> vec3f",
      "fn terrain_sky_factor(p : vec3f) -> vec3f",
    ]) {
      expect(terrainLitWgsl).toContain(hook);
    }
  });

  it("carries the layout's sizes: two vec4f a class after the weights, 25 rows of lights", () => {
    expect(CLASS_LAW_ROWS * 16).toBe(CLASS_LAWS_BYTES);
    expect(terrainLitWgsl).toContain(
      `const CLASS_LAW_WORD : u32 = ${String(CLASS_WEIGHTS_BYTES / 4)}u;`,
    );
    expect(terrainLitWgsl).toContain(
      `const CLASS_LAYER_WORDS : u32 = ${String(CLASS_LAYERS_BYTES / 4)}u;`,
    );
    expect(terrainLitWgsl).toContain("const TERRAIN_CLASSES : u32 = 8u;");
    expect(terrainLitWgsl).toContain("const FIRST_TERRAIN_SECONDARY_ROW : u32 = 21u;");
    expect(TERRAIN_LIGHT_ROWS).toBe(21 + 2 * 2);
  });
});

describe("the class laws (decision-r10-limb; decision-r07-t8b)", () => {
  it("takes R10.T1.a's three laws onto R07's moon, airless-ice and lambert templates", () => {
    expect(CLASS_LAW_TEMPLATES).toEqual({
      1: { template: "moon", phaseExponent: 1 },
      2: { template: "airless-ice", phaseExponent: AIRLESS_ICE_EXPONENT },
      3: { template: "lambert", phaseExponent: 1 },
    });
  });

  it("solves the Ice law's exponent to Ganymede's q of 0.80 at a constant L of 1, as the generator does", () => {
    const law: PhotometricLaw = {
      a: [1, 1, 1],
      lommelSeeligerShare: 1,
      template: "airless-ice",
      phaseExponent: [AIRLESS_ICE_EXPONENT, AIRLESS_ICE_EXPONENT, AIRLESS_ICE_EXPONENT],
    };
    expect(lawPhaseIntegral(law)[1]).toBeCloseTo(0.8, 6);
  });

  it("tabulates the particulate rows' L(α) as L(0) times McEwen's profile, and Lambert's f ≡ 1, L ≡ 0", () => {
    const ice = phaseFactorTableOf(classRowLaw(2, 0.38));
    const lambert = phaseFactorTableOf(classRowLaw(3, 0));
    for (const i of [0, 60, 180, 300, 360]) {
      const alpha = i * PHASE_TABLE_STEP_RAD;
      expect(ice.alpha[i]).toBe(Math.fround(0.38 * mcewenLunarProfile(alpha)));
      expect(lambert.alpha[i]).toBe(0);
      expect([lambert.rgb[3 * i], lambert.rgb[3 * i + 1], lambert.rgb[3 * i + 2]]).toEqual([
        1, 1, 1,
      ]);
    }
  });

  it("reads a slot's photometry as a law, R, V and B as r, g and b, and none for law 0", () => {
    const photometry = photometryOf([
      [0.1, 0.12, 0.14, 1, 1],
      [0.9, 0.92, 0.94, 3, 0],
      [0, 0, 0, 0, 0],
    ]);
    expect(classPhotometryOf(photometry, 0)).toEqual({
      a: [Math.fround(0.14), Math.fround(0.12), Math.fround(0.1)],
      l0: 1,
      code: 1,
    });
    expect(classPhotometryOf(photometry, 1)?.code).toBe(3);
    expect(classPhotometryOf(photometry, 2)).toBeNull();
    expect(classPhotometryOf(photometry, 7)).toBeNull();
  });

  it("refuses a slot naming a law no law has", () => {
    expect(() => classPhotometryOf(photometryOf([[0.1, 0.1, 0.1, 9, 1]]), 0)).toThrow(/law 9/u);
  });

  it("keys the rows by law and L(0): every material at L(0) = 1 shares its law's row, a bright frost takes its own", () => {
    const elsewhere = withTemplateCurve({
      a: [0.1, 0.1, 0.1],
      lommelSeeligerShare: 1,
      template: "moon",
      phaseExponent: [1, 1, 1],
    });
    const rows = new TerrainPhaseRows(elsewhere);
    expect(rows.laws).toEqual([elsewhere]);
    const regolith = rows.rowOf(PHASE_LAW.regolith, 1);
    expect(rows.rowOf(PHASE_LAW.regolith, 1)).toBe(regolith);
    const frost = rows.rowOf(PHASE_LAW.ice, 0.57789);
    const ice = rows.rowOf(PHASE_LAW.ice, 1);
    const lambert = rows.rowOf(PHASE_LAW.lambert, 0);
    expect([regolith, frost, ice, lambert]).toEqual([1, 2, 3, 4]);
    expect(rows.laws[frost]?.lommelSeeligerShare).toBe(0.57789);
  });

  it("counts a change of row 0's law as a version, and no change for the same law", () => {
    const elsewhere = classRowLaw(PHASE_LAW.regolith, 1);
    const rows = new TerrainPhaseRows(elsewhere);
    const before = rows.version;
    expect(rows.setElsewhere(elsewhere)).toBe(false);
    expect(rows.version).toBe(before);
    expect(rows.setElsewhere({ ...elsewhere })).toBe(true);
    expect(rows.version).toBe(before + 1);
  });

  it("packs a slot's class laws: A and L(0), then the row, −1 for a slot with none", () => {
    const rows = new TerrainPhaseRows(classRowLaw(3, 0));
    const packed = packClassLaws(
      photometryOf([
        [0.1, 0.12, 0.14, 1, 1],
        [0, 0, 0, 0, 0],
        [0.9, 0.92, 0.94, 3, 0],
      ]),
      rows,
      1,
    );
    expect(packed).toHaveLength(CLASS_LAW_ROWS * 4);
    expect([...packed.subarray(0, 5)]).toEqual([
      Math.fround(0.14),
      Math.fround(0.12),
      Math.fround(0.1),
      1,
      1,
    ]);
    expect(packed[8 + 4]).toBe(-1);
    expect([...packed.subarray(16, 21)]).toEqual([
      Math.fround(0.94),
      Math.fround(0.92),
      Math.fround(0.9),
      0,
      2,
    ]);
    for (let k = 3; k < TERRAIN_CLASSES; k += 1) {
      expect(packed[8 * k + 4]).toBe(-1);
    }
    expect(rows.laws).toHaveLength(3);
  });

  it("scales each class's A for an oblate figure as law 0's and R07's disc's laws are (R10.T10.e)", () => {
    const cOverA = 0.9;
    const photometry = photometryOf([
      [0.1, 0.12, 0.14, 1, 1],
      [0.8, 0.85, 0.9, 2, 0.6],
      [0.9, 0.92, 0.94, 3, 0],
    ]);
    const packed = packClassLaws(photometry, new TerrainPhaseRows(classRowLaw(3, 0)), cOverA);
    const expected = [
      [0.14, 0.12, 0.1, 1],
      [0.9, 0.85, 0.8, 0.6],
      [0.94, 0.92, 0.9, 0],
    ] as const;
    expected.forEach(([r, g, b, l0], k) => {
      const factor = oblateAlbedoScale(Math.fround(l0), cOverA);
      // A factor of order the flattening wherever the law limb-darkens, 1 under Lommel–Seeliger's
      // (L(0) = 1), whose disc reaches p on any figure.
      expect(factor === 1).toBe(l0 === 1);
      expect([...packed.subarray(8 * k, 8 * k + 4)]).toEqual([
        Math.fround(Math.fround(r) * factor),
        Math.fround(Math.fround(g) * factor),
        Math.fround(Math.fround(b) * factor),
        Math.fround(l0),
      ]);
    });
  });
});

describe("the lit terrain's lights", () => {
  const annuli = annulusEdges(0.6, 0.2, 2);
  const lighting: TerrainLighting = {
    lights: [
      {
        direction: vec3(0, 0.6, 0.8),
        distance: 2.3e4,
        radius: 109,
        illuminance: [1.2e5, 1.1e5, 1.0e5],
        annuli: [annuli, annuli, annuli],
      },
    ],
    occluders: [{ centre: vec3(60, 0, 0), radius: 0.27 }],
    secondaries: [
      { direction: vec3(1, 0, 0), distance: 60, radius: 0.27, illuminance: [0.2, 0.2, 0.2] },
    ],
    elsewhere: withTemplateCurve({
      a: [0.12, 0.12, 0.12],
      lommelSeeligerShare: 1,
      template: "moon",
      phaseExponent: [1, 1, 1],
    }),
    exposureScale: 1e-3,
  };

  it("packs them as terrainLit.wgsl reads them, law 0's A scaled for the figure", () => {
    const rows = packTerrainLighting(lighting, 0.9);
    expect(rows).toHaveLength(TERRAIN_LIGHT_ROWS * 4);
    const row = (i: number) => [...rows.subarray(4 * i, 4 * i + 4)];
    expect(row(0)).toEqual([1, 1, 1, 2]);
    const k = oblateAlbedoScale(1, 0.9);
    expect(row(1)).toEqual([0.12 * k, 0.12 * k, 0.12 * k, 1].map(Math.fround));
    expect(row(2)).toEqual([0, Math.fround(1e-3 / Math.PI), 0, 0]);
    expect(row(3)).toEqual([0, 0.6, 0.8, 2.3e4].map(Math.fround));
    expect(row(4)).toEqual([1.2e5, 1.1e5, 1.0e5, 109].map(Math.fround));
    expect(row(5)).toEqual([...annuli.edges.subarray(1, 3), 1, 1].map(Math.fround));
    expect(row(19)).toEqual([60, 0, 0, 0.27].map(Math.fround));
    expect(row(21)).toEqual([1, 0, 0, 60]);
    expect(row(22)).toEqual([0.2, 0.2, 0.2, 0.27].map(Math.fround));
  });

  it("draws at most two stars, occluders and planetshine sources", () => {
    const star = lighting.lights[0];
    const occluder = lighting.occluders[0];
    const source = lighting.secondaries[0];
    if (star === undefined || occluder === undefined || source === undefined) {
      throw new Error("the fixture has one of each");
    }
    const many = packTerrainLighting(
      {
        ...lighting,
        lights: [star, star, star],
        occluders: [occluder, occluder, occluder],
        secondaries: [source, source, source],
      },
      1,
    );
    expect([...many.subarray(0, 3)]).toEqual([2, 2, 2]);
  });
});

/** An airless body's law of normal albedo `a` and L(0) `l` on a law's row. */
function rowLaw(
  code: 1 | 2 | 3,
  a: number,
  l: number,
): { law: PhotometricLaw; table: PhaseFactorTable } {
  const row = classRowLaw(code, l);
  return { law: { ...row, a: [a, a, a] }, table: phaseFactorTableOf(row) };
}

describe("a texel's blend (R10.T10.b; Design note 8)", () => {
  const regolith = rowLaw(1, 0.12, 1);
  const frost = rowLaw(2, 0.85, 0.65);
  const geometries = [
    [0.9, 0.7, 20],
    [0.5, 0.1, 60],
    [0.2, 0.95, 110],
  ] as const;

  it("reflects w r₁ + (1 − w) r₂ for two classes in weights w and 1 − w, to 10⁻⁶", () => {
    for (const w of [0, 0.25, 0.5, 0.9, 1]) {
      for (const [mu0, mu, deg] of geometries) {
        const ours = terrainReflectance(
          [null, texelLawOf(regolith.law), texelLawOf(frost.law)],
          [0, w, 1 - w],
          mu0,
          mu,
          deg * RAD,
        );
        const one = brdf(regolith.law, mu0, mu, deg * RAD);
        const two = brdf(frost.law, mu0, mu, deg * RAD);
        for (const c of [0, 1, 2] as const) {
          expect(Math.abs(ours[c] - (w * one[c] + (1 - w) * two[c]))).toBeLessThan(1e-6);
        }
      }
    }
  });

  it("sums an even blend of regolith (0.12, L 1) and snow (0.9, L 0) under one f at μ₀ 0.5, μ 0.1, not the 1.7× of the mixed law", () => {
    // The test's own construction: one f for both, each its own L, as Design note 8's example.
    const f = phaseFactorTableOf(classRowLaw(1, 1)).rgb;
    const tableAt = (l: number): PhaseFactorTable => ({
      rgb: f,
      alpha: new Float32Array(PHASE_TABLE_SAMPLES).fill(l),
    });
    const laws = [
      null,
      { a: [0.12, 0.12, 0.12] as const, table: tableAt(1) },
      { a: [0.9, 0.9, 0.9] as const, table: tableAt(0) },
    ];
    const alpha = 30 * RAD;
    const summed = terrainReflectance(laws, [0, 0.5, 0.5], 0.5, 0.1, alpha)[1];
    const parts =
      0.5 * brdfFromTable({ a: [0.12, 0.12, 0.12] }, tableAt(1), 0.5, 0.1, alpha)[1] +
      0.5 * brdfFromTable({ a: [0.9, 0.9, 0.9] }, tableAt(0), 0.5, 0.1, alpha)[1];
    const mixed = brdfFromTable({ a: [0.51, 0.51, 0.51] }, tableAt(0.5), 0.5, 0.1, alpha)[1];
    expect(summed).toBeCloseTo(parts, 12);
    expect(mixed / summed).toBeCloseTo(1.7, 2);
  });

  it("adds nothing for a zero weight, whatever the law", () => {
    const broken = { a: [Number.NaN, Number.NaN, Number.NaN] as const, table: regolith.table };
    const without = terrainReflectance([null, texelLawOf(regolith.law)], [0, 1], 0.6, 0.5, 0.4);
    const with0 = terrainReflectance(
      [null, texelLawOf(regolith.law), broken],
      [0, 1, 0],
      0.6,
      0.5,
      0.4,
    );
    expect(with0).toEqual(without);
  });

  it("names the class of the largest weight, the lowest on a tie, none without weight", () => {
    expect(dominantSlot([0.1, 0.5, 0.4, 0, 0, 0, 0, 0])).toBe(1);
    expect(dominantSlot([0.4, 0.4, 0.2, 0, 0, 0, 0, 0])).toBe(0);
    expect(dominantSlot([0, 0, 0, 0, 0, 0, 0, 0.2])).toBe(7);
    expect(dominantSlot([0, 0, 0, 0, 0, 0, 0, 0])).toBeNull();
  });

  it("draws a one-class palette's texel as the per-body law of that class", () => {
    const photometry = { a: [0.14, 0.12, 0.1] as const, l0: 0.84, code: 2 as const };
    const perBody = withTemplateCurve({
      a: photometry.a,
      lommelSeeligerShare: photometry.l0,
      template: "airless-ice",
      phaseExponent: [AIRLESS_ICE_EXPONENT, AIRLESS_ICE_EXPONENT, AIRLESS_ICE_EXPONENT],
    });
    for (const [mu0, mu, deg] of geometries) {
      expect(
        terrainReflectance([null, classTexelLaw(photometry, 1)], [0, 1], mu0, mu, deg * RAD),
      ).toEqual(brdf(perBody, mu0, mu, deg * RAD));
    }
  });

  it("gives law 0 what the named slots leave, and scales them down past 1", () => {
    const named = [true, false, true, false, false, false, false, false];
    expect(terrainShares([0.5, 0.3, 0.2, 0, 0, 0, 0, 0], named)).toEqual([
      0.30000000000000004, 0.5, 0, 0.2, 0, 0, 0, 0, 0,
    ]);
    const over = terrainShares([0.75, 0, 0.5, 0, 0, 0, 0, 0], named);
    expect(over[0]).toBe(0);
    expect((over[1] ?? 0) + (over[3] ?? 0)).toBeCloseTo(1, 15);
    expect(terrainShares([0, 0, 0, 0, 0, 0, 0, 0], named)[0]).toBe(1);
  });

  it("reads a texel's weights bilinearly between its quad's vertices, each byte ÷ 255", () => {
    const weights = new Uint8Array(65 * 65 * 8);
    // Vertex (3, 4) all slot 0, vertex (4, 4) all slot 1.
    weights[8 * (4 * 65 + 3)] = 255;
    weights[8 * (4 * 65 + 4) + 1] = 255;
    const at = (x: number, y: number) => texelWeights(weights, x / 64, y / 64);
    expect(at(3, 4).slice(0, 2)).toEqual([1, 0]);
    expect(at(4, 4).slice(0, 2)).toEqual([0, 1]);
    const middle = at(3.25, 4);
    expect(middle[0]).toBeCloseTo(0.75, 12);
    expect(middle[1]).toBeCloseTo(0.25, 12);
    expect(texelWeights(new Uint8Array(), 0.5, 0.5)).toEqual([0, 0, 0, 0, 0, 0, 0, 0]);
  });
});

/** decision-r10-limb's particulate rule L_p(x) (`material/properties.rs`' `limb_parameter`). */
function particulateLimb(x: number): number {
  const onset = 0.6;
  const enceladus = 1.35;
  const limb = 0.38;
  if (!(x > onset)) {
    return 1;
  }
  if (x >= enceladus) {
    return limb;
  }
  return (limb * enceladus * (x - onset) + onset * (enceladus - x)) / (x * (enceladus - onset));
}

/** Eight-point Gauss–Legendre on [0, 1]. */
const GL_NODES = [
  0.019855071751231856, 0.10166676129318664, 0.2372337950418355, 0.4082826787521751,
  0.5917173212478249, 0.7627662049581645, 0.8983332387068134, 0.9801449282487681,
];
const GL_WEIGHTS = [
  0.05061426814518813, 0.11119051722668724, 0.15685332293894363, 0.181341891689181,
  0.181341891689181, 0.15685332293894363, 0.11119051722668724, 0.05061426814518813,
];

/** The emission cosines' subintervals: [0, 10⁻⁶], then 40 geometric steps to 1. */
const MU_EDGES = [0, ...Array.from({ length: 41 }, (_, k) => 10 ** (-6 + (6 * k) / 40))];

/** Midpoints in the relative azimuth over [0, π], doubled for its other half. */
const AZIMUTHS = 180;

/**
 * η(μ₀) = A_h ÷ A_N of a row: (1 ÷ π μ₀) ∫ (I/F at A = 1) μ dΩ over the hemisphere, the I/F the
 * twin's (`brdfFromTable`, the row's f and L(α)), cos α = μ₀ μ + sin i sin e cos φ.
 */
function eta(table: PhaseFactorTable, mu0: number): number {
  const sinI = Math.sqrt(1 - mu0 * mu0);
  const dPhi = Math.PI / AZIMUTHS;
  const one = { a: [1, 1, 1] as const };
  let sum = 0;
  for (let s = 0; s + 1 < MU_EDGES.length; s += 1) {
    const lo = MU_EDGES[s] ?? 0;
    const hi = MU_EDGES[s + 1] ?? 1;
    for (let g = 0; g < GL_NODES.length; g += 1) {
      const mu = lo + (hi - lo) * (GL_NODES[g] ?? 0);
      const w = (hi - lo) * (GL_WEIGHTS[g] ?? 0);
      const sinE = Math.sqrt(Math.max(0, 1 - mu * mu));
      let ring = 0;
      for (let p = 0; p < AZIMUTHS; p += 1) {
        const cosAlpha = mu0 * mu + sinI * sinE * Math.cos((p + 0.5) * dPhi);
        const alpha = Math.acos(Math.min(1, Math.max(-1, cosAlpha)));
        ring += brdfFromTable(one, table, mu0, mu, alpha)[1];
      }
      sum += w * mu * ring * 2 * dPhi;
    }
  }
  return sum / (Math.PI * mu0);
}

/** The incidences of the bound's check (decision-r10-limb §3). */
const MU0S = [1, 0.5, 0.2, 0.1, 0.05, 0.02, 0.01, 0.005];

describe("the rows' energy bound, with their f and L(α) (decision-r10-limb §3)", () => {
  it("keeps every A_h at most 1 on each particulate row, on an A_N grid up to its ceiling of 1.35", () => {
    let worst = { code: 0, normalAlbedo: 0, mu0: 0, ah: 0 };
    for (const code of [1, 2] as const) {
      // L(0) is 1 up to A_N 0.60, where A_h = A_N η rises with A_N: its top, 0.60, stands for the
      // grid below it; above it each A_N has an L(0) of its own.
      for (let k = 12; k <= 27; k += 1) {
        const normalAlbedo = 0.05 * k;
        const table = phaseFactorTableOf(classRowLaw(code, particulateLimb(normalAlbedo)));
        for (const mu0 of MU0S) {
          const ah = normalAlbedo * eta(table, mu0);
          if (ah > worst.ah) {
            worst = { code, normalAlbedo, mu0, ah };
          }
        }
      }
    }
    // The Ice row at its ceiling, 1.35 × 0.720 = 0.97 by the ruling's figures.
    expect(worst).toMatchObject({ code: 2, normalAlbedo: expect.closeTo(1.35, 9) });
    expect(worst.ah).toBeLessThanOrEqual(1);
    expect(worst.ah).toBeCloseTo(0.97, 1);
  });

  it("gives the Ice row at L(0) 0.38 its sup η of 0.720 and its η(1) of 0.636, to 0.01", () => {
    const table = phaseFactorTableOf(classRowLaw(2, 0.38));
    const etas = MU0S.map((mu0) => eta(table, mu0));
    expect(Math.abs((etas[0] ?? 0) - 0.636)).toBeLessThan(0.01);
    expect(Math.abs(Math.max(...etas) - 0.72)).toBeLessThan(0.01);
  });

  it("holds the Lambert row's η at 1 at every incidence, to 10⁻⁶", () => {
    const table = phaseFactorTableOf(classRowLaw(3, 0));
    for (const mu0 of MU0S) {
      expect(Math.abs(eta(table, mu0) - 1)).toBeLessThan(1e-6);
    }
  });
});
