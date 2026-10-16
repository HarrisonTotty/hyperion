import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { type PhaseTemplateId, type PhotometricLaw } from "../appearance/law";
import { lawFor, lawPhaseIntegral, withTemplateCurve } from "../appearance/phase";
import { packPhaseFactorRows } from "../appearance/litBodyProbe";
import { discReflectance } from "../bodies/discShading";
import { type LitRegime, REGIME_HYSTERESIS } from "../bodies/regime";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { ProjectionCamera } from "../camera/projection";
import type { Rgb } from "../photometry/toneCurve";
import { SETTINGS, WIREFRAME_TERRAIN_TAU_PX } from "../quality/qualitySetting";
import { CLASS_MAP_LEVEL, classMapLawsOf } from "./classMap";
import { CoverageView } from "./coverage";
import {
  handoverRangeM,
  handoverRangeRatio,
  handoverReliefM,
  handOverRegimes,
  handoverTauPx,
  handsOver,
  type HandoverSphere,
  limbDistanceM,
  pixelAngleRad,
  storeReliefSource,
} from "./handover";
import {
  classRowLaw,
  PHASE_LAW,
  type PhaseLawCode,
  TERRAIN_CLASSES,
  TerrainPhaseRows,
  terrainReflectance,
  terrainTexelLaws,
} from "./terrainMaterial";
import { PHOTOMETRY_STRIDE } from "./workers/messages";

const RAD = Math.PI / 180;

/** Design note 13's pixel: 1080p across 60° by width ÷ field, rad. */
const NOTE_RAD_PER_PX = 5.45e-4;

/** Design note 13's relief: an Earth's 10 km, m. */
const EARTH_RELIEF_M = 1e4;

/** A 1080p view across 60°, as R07's regimes measure it. */
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: 60 * RAD };
const HD = { widthPx: 1920, heightPx: 1080 };

const BODY: BodyIdHex = "0200080020000000.0300";

/** A level-6 coverage with the given cells surveyed at resolution code 40, or none. */
function coverageOf(cells: ReadonlyArray<number>): CoverageView {
  const empty = CoverageView.empty(6);
  if (cells.length === 0) {
    return empty;
  }
  const result = empty.merged(
    { ranges: cells.map((start) => ({ start, end: start + 1, resolution: 40 })) },
    1,
  );
  if (!result.ok) {
    throw new Error(result.message);
  }
  return result.coverage;
}

/** `x` within a relative `tolerance` of `expected`. */
function near(x: number, expected: number, tolerance: number): boolean {
  return Math.abs(x / expected - 1) <= tolerance;
}

describe("the hand-over range (R10.T10.e, Design note 13)", () => {
  it("is about 1.8 × 10⁷ m for 10 km of relief at 1080p and 1 px, and 4.6 × 10⁶ m at the wireframe's 4 px", () => {
    const atOnePx = handoverRangeM(EARTH_RELIEF_M, 1, NOTE_RAD_PER_PX);
    const atFourPx = handoverRangeM(EARTH_RELIEF_M, WIREFRAME_TERRAIN_TAU_PX, NOTE_RAD_PER_PX);
    expect(near(atOnePx, 1.83e7, 3e-3)).toBe(true);
    expect(near(atFourPx, 4.59e6, 3e-3)).toBe(true);
    expect(atOnePx / atFourPx).toBe(4);
  });

  it("falls at 1.66 × 10⁷ m and 4.16 × 10⁶ m at a live 1080p view's centre pixel across 60°, as the 3 px switch counts pixels", () => {
    const radPerPx = pixelAngleRad(CAMERA, HD);
    expect(near(radPerPx, 6.01e-4, 1e-3)).toBe(true);
    // The inverse of the regimes' scale (`angularDiameterPx`'s width ÷ 2 tan(fov ÷ 2)).
    expect(radPerPx * (HD.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2)))).toBeCloseTo(1, 15);
    expect(near(handoverRangeM(EARTH_RELIEF_M, 1, radPerPx), 1.663e7, 1e-3)).toBe(true);
    expect(near(handoverRangeM(EARTH_RELIEF_M, 4, radPerPx), 4.157e6, 1e-3)).toBe(true);
  });

  it("takes the style's τ: 1 px high and 2 px low in the photorealistic style, 4 px in the wireframe on both", () => {
    expect(handoverTauPx("photorealistic", "high")).toBe(1);
    expect(handoverTauPx("photorealistic", "low")).toBe(2);
    expect(handoverTauPx("photorealistic", "high")).toBe(SETTINGS.high.terrain.tauPx);
    expect(handoverTauPx("photorealistic", "low")).toBe(SETTINGS.low.terrain.tauPx);
    expect(handoverTauPx("wireframe", "high")).toBe(4);
    expect(handoverTauPx("wireframe", "low")).toBe(4);
  });

  it("refuses a relief that is negative or not finite, and a τ or pixel that is not positive", () => {
    expect(handoverRangeM(0, 1, NOTE_RAD_PER_PX)).toBe(0);
    expect(() => handoverRangeM(-1, 1, NOTE_RAD_PER_PX)).toThrow(RangeError);
    expect(() => handoverRangeM(Number.NaN, 1, NOTE_RAD_PER_PX)).toThrow(RangeError);
    expect(() => handoverRangeM(1e4, 0, NOTE_RAD_PER_PX)).toThrow(RangeError);
    expect(() => handoverRangeM(1e4, 1, 0)).toThrow(RangeError);
  });

  it("is a view's range ratio for the field's demand, unbounded with no relief known", () => {
    const rangeM = handoverRangeM(EARTH_RELIEF_M, 2, NOTE_RAD_PER_PX);
    expect(handoverRangeRatio(rangeM / 2, EARTH_RELIEF_M, 2, NOTE_RAD_PER_PX)).toBeCloseTo(0.5, 12);
    expect(handoverRangeRatio(1e3, null, 2, NOTE_RAD_PER_PX)).toBe(Number.POSITIVE_INFINITY);
    expect(handoverRangeRatio(1e3, 0, 2, NOTE_RAD_PER_PX)).toBe(Number.POSITIVE_INFINITY);
  });
});

describe("the hand-over's relief", () => {
  const header = { realised_relief_m: 18_000 };

  it("is the field header's realised relief where the survey covers ground", () => {
    expect(handoverReliefM(header, coverageOf([7, 8]))).toBe(18_000);
  });

  it("never hands a body over with no survey, no coverage or no header, however near", () => {
    for (const [held, coverage] of [
      [header, coverageOf([])],
      [header, null],
      [null, coverageOf([7])],
    ] as const) {
      const reliefM = handoverReliefM(held, coverage);
      expect(reliefM).toBeNull();
      for (const distanceM of [0, 1, 1e3, 1e7]) {
        expect(handsOver(distanceM, reliefM, 1, NOTE_RAD_PER_PX, false)).toBe(false);
        expect(handsOver(distanceM, reliefM, 1, NOTE_RAD_PER_PX, true)).toBe(false);
      }
    }
    expect(handoverReliefM({ realised_relief_m: 0 }, coverageOf([7]))).toBeNull();
  });

  it("reads a store's header and coverage of each body", () => {
    const store = {
      header: (body: BodyIdHex) => (body === BODY ? header : null),
      coverage: (body: BodyIdHex) => (body === BODY ? coverageOf([3]) : null),
    };
    const source = storeReliefSource(store);
    expect(source.reliefM(BODY)).toBe(18_000);
    expect(source.reliefM("0200080020000000.0301")).toBeNull();
  });
});

/** A body of 1 m `distanceM` ahead, whose limb is its centre's distance, and its relief. */
function sphere(id: BodyIdHex, distanceM: number, reliefM?: number): HandoverSphere {
  return { id, centreM: vec3(0, 0, -distanceM), radiusM: 1, reliefM };
}

describe("the hand-over's decision", () => {
  const rangeM = handoverRangeM(EARTH_RELIEF_M, 1, NOTE_RAD_PER_PX);
  const h = REGIME_HYSTERESIS;

  it("hands over once the relief subtends τ (1 + h) px, and back once under τ (1 − h), R07's hysteresis", () => {
    const at = (distanceM: number, was: boolean): boolean =>
      handsOver(distanceM, EARTH_RELIEF_M, 1, NOTE_RAD_PER_PX, was);
    // Inside the band either side of the range, a body keeps what it was.
    for (const distanceM of [rangeM * 0.95, rangeM, rangeM * 1.05]) {
      expect(at(distanceM, false)).toBe(false);
      expect(at(distanceM, true)).toBe(true);
    }
    expect(at(rangeM / (1 + h), false)).toBe(true);
    expect(at((rangeM / (1 + h)) * 1.001, false)).toBe(false);
    expect(at(rangeM / (1 - h), true)).toBe(true);
    expect(at((rangeM / (1 - h)) * 1.001, true)).toBe(false);
  });

  it("measures from the camera to the limb, so that a body of little relief beside its radius hands over as the camera nears its ground", () => {
    expect(limbDistanceM(5, 3)).toBe(4);
    expect(limbDistanceM(3, 3)).toBe(0);
    expect(limbDistanceM(1, 3)).toBe(0);
    // A Venus of 6,051.8 km and 13 km of relief in the wireframe at 1080p: its range, 5.4 × 10⁶ m,
    // is inside its radius, so the centre's distance would never hand it over.
    const radPerPx = pixelAngleRad(CAMERA, HD);
    const venusM = 6.0518e6;
    const reliefM = 1.3e4;
    const wire = handoverRangeM(reliefM, WIREFRAME_TERRAIN_TAU_PX, radPerPx);
    expect(wire).toBeLessThan(venusM);
    const at = (altitudeM: number): boolean =>
      handsOver(
        limbDistanceM(venusM + altitudeM, venusM),
        reliefM,
        WIREFRAME_TERRAIN_TAU_PX,
        radPerPx,
        false,
      );
    expect(at(1e3)).toBe(true);
    expect(at(2e6)).toBe(false);
    // Its limb is at the range (less the hysteresis) some 2,000 km up.
    const altitudeM = Math.sqrt((wire / (1 + REGIME_HYSTERESIS)) ** 2 + venusM ** 2) - venusM;
    expect(at(altitudeM * 0.999)).toBe(true);
    expect(at(altitudeM * 1.001)).toBe(false);
    expect(altitudeM).toBeGreaterThan(1.5e6);
    expect(altitudeM).toBeLessThan(2.5e6);
  });

  it("hands over at the wireframe's 4 px a quarter as far", () => {
    const wire = handoverRangeM(EARTH_RELIEF_M, WIREFRAME_TERRAIN_TAU_PX, NOTE_RAD_PER_PX);
    const tau = handoverTauPx("wireframe", "high");
    expect(handsOver(wire / 1.11, EARTH_RELIEF_M, tau, NOTE_RAD_PER_PX, false)).toBe(true);
    expect(handsOver(wire / 1.09, EARTH_RELIEF_M, tau, NOTE_RAD_PER_PX, false)).toBe(false);
    expect(handsOver(rangeM / 2, EARTH_RELIEF_M, tau, NOTE_RAD_PER_PX, false)).toBe(false);
  });

  it("hands over discs alone, by each body's own relief, whatever the input order", () => {
    const inside = rangeM / 2;
    const bodies = [
      sphere("0200080020000000.0300", inside, EARTH_RELIEF_M),
      sphere("0200080020000000.0301", inside),
      sphere("0200080020000000.0302", inside, EARTH_RELIEF_M),
      sphere("0200080020000000.0303", rangeM * 2, EARTH_RELIEF_M),
    ];
    const regimes = new Map<BodyIdHex, LitRegime>([
      ["0200080020000000.0300", "disc"],
      ["0200080020000000.0301", "disc"],
      // A point is the point regime's however near: R10.T10.f's flux.
      ["0200080020000000.0302", "point"],
      ["0200080020000000.0303", "disc"],
    ]);
    const handed = handOverRegimes(regimes, bodies, 1, NOTE_RAD_PER_PX, new Map());
    expect([...handed]).toEqual([
      ["0200080020000000.0300", "terrain"],
      ["0200080020000000.0301", "disc"],
      ["0200080020000000.0302", "point"],
      ["0200080020000000.0303", "disc"],
    ]);
    const reversed = handOverRegimes(regimes, bodies.toReversed(), 1, NOTE_RAD_PER_PX, new Map());
    expect(new Map(reversed)).toEqual(new Map(handed));
    // Handed over before, a body just past the range stays handed over.
    const held = handOverRegimes(
      new Map([[BODY, "disc"]]),
      [sphere(BODY, rangeM * 1.05, EARTH_RELIEF_M)],
      1,
      NOTE_RAD_PER_PX,
      new Map([[BODY, "terrain"]]),
    );
    expect(held.get(BODY)).toBe("terrain");
  });
});

/** A class map's answer holding the given classes' photometry and no texel. */
function answerOf(
  photometry: ReadonlyArray<readonly [number, number, number, number, number]>,
): Parameters<typeof classMapLawsOf>[0] {
  const values = new Float32Array(photometry.length * PHOTOMETRY_STRIDE);
  photometry.forEach((slot, k) => {
    values.set(slot, PHOTOMETRY_STRIDE * k);
  });
  return {
    level: CLASS_MAP_LEVEL,
    fieldLevel: 6,
    classes: new Uint8Array(photometry.length * 2),
    photometry: values,
    texels: new Uint32Array(0),
    fractions: new Float32Array(0),
    weights: new Uint8Array(0),
    complete: true,
  };
}

/** A bake's photometry holding the given slots, the rest empty. */
function bakePhotometryOf(
  photometry: ReadonlyArray<readonly [number, number, number, number, number]>,
): Float32Array {
  const values = new Float32Array(TERRAIN_CLASSES * PHOTOMETRY_STRIDE);
  photometry.forEach((slot, k) => {
    values.set(slot, PHOTOMETRY_STRIDE * k);
  });
  return values;
}

/** A body's uniform law on a template from its p and q, display order. */
function uniformLaw(p: Rgb, q: Rgb, template: PhaseTemplateId): PhotometricLaw {
  return lawFor(p, q, template);
}

describe("the disc and the terrain draw one law a class (R10.T10.e)", () => {
  // Each class law, [B, V, R, code, L(0)] as the worker resolves it: a regolith at L(0) 1, an ice
  // frost at 0.9 (L(0) = 0.6696 ÷ 0.9 − 0.1160, decision-r10-limb), and a consolidated class under
  // Lambert's law.
  const CLASSES = [
    [0.105, 0.142, 0.172, PHASE_LAW.regolith, 1],
    [0.86, 0.9, 0.92, PHASE_LAW.ice, 0.6696 / 0.9 - 0.116],
    [0.78, 0.8, 0.81, PHASE_LAW.lambert, 0],
  ] as const;
  // Law 0, the body's own: Mercury's template at the Moon-like world's p (R10.T10.a's table).
  const ELSEWHERE = uniformLaw([0.172, 0.142, 0.105], [0.48, 0.48, 0.48], "mercury");
  const GEOMETRIES = [1, 0.7, 0.4, 0.1].flatMap((mu0) =>
    [1, 0.6, 0.2, 0.05].map((mu) => [mu0, mu] as const),
  );

  it.each([
    ["a sphere", 1],
    ["an oblate body", 0.9],
  ])(
    "agree to 10⁻⁶ at 30° and 90° of phase on %s, per class law and law 0, each law's one row read by both",
    (_, cOverA) => {
      const discLaws = [ELSEWHERE, ...classMapLawsOf(answerOf(CLASSES))];
      const terrainLaws = terrainTexelLaws(bakePhotometryOf(CLASSES), ELSEWHERE, cOverA);
      const rows = new TerrainPhaseRows(ELSEWHERE);
      const names = ["law 0", "Regolith", "Ice", "Lambert"];
      let worst = 0;
      let checked = 0;
      discLaws.forEach((discLaw, m) => {
        if (discLaw === null) {
          throw new Error(`class ${String(m)} has a law`);
        }
        // The row each side reads: the disc's frame's from its law, the terrain's from its rows.
        const code = CLASSES[m - 1]?.[3];
        const l0 = CLASSES[m - 1]?.[4];
        const terrainRow =
          code === undefined || l0 === undefined
            ? rows.laws[0]
            : rows.laws[rows.rowOf(code satisfies PhaseLawCode, Math.fround(l0))];
        if (terrainRow === undefined) {
          throw new Error(`${names[m] ?? ""} has no row`);
        }
        expect(packPhaseFactorRows([discLaw])).toEqual(packPhaseFactorRows([terrainRow]));
        const shares = discLaws.map((_law, k) => (k === m ? 1 : 0));
        for (const alphaDeg of [30, 90]) {
          for (const [mu0, mu] of GEOMETRIES) {
            const disc = discReflectance(
              discLaws.map((law) => law ?? ELSEWHERE),
              cOverA,
              shares,
              mu0,
              mu,
              alphaDeg * RAD,
            );
            const terrain = terrainReflectance(terrainLaws, shares, mu0, mu, alphaDeg * RAD);
            for (const c of [0, 1, 2] as const) {
              expect(disc[c]).toBeGreaterThan(0);
              worst = Math.max(worst, Math.abs(terrain[c] - disc[c]) / disc[c]);
              checked += 1;
            }
          }
        }
      });
      expect(checked).toBe(4 * 2 * GEOMETRIES.length * 3);
      expect(worst).toBeLessThan(1e-6);
    },
  );
});

/** A template's own q in V at s = 1 and L `share`, under McEwen's L(α) where it takes one. */
function templateQ(template: PhaseTemplateId, share: number): number {
  return lawPhaseIntegral(
    withTemplateCurve({
      a: [0.1, 0.1, 0.1],
      lommelSeeligerShare: share,
      template,
      phaseExponent: [1, 1, 1],
    }),
  )[1];
}

describe("the classes' q against plan 14's (R10.T10.a's residual, Design note 8)", () => {
  // The orientation-averaged phase integral of a patterned sphere is Σ aᵢ pᵢ qᵢ ÷ Σ aᵢ pᵢ, as its
  // p is the area average (R10.T10.a's module documentation): on a world whose ground takes one
  // class law, the classes' q is that law's, whatever their albedos.
  const regolithQ = lawPhaseIntegral(classRowLaw(PHASE_LAW.regolith, 1))[1];

  it("states the Regolith row's q and its residual against Mercury's and Mars's templates", () => {
    // Plan 14's `templates.golden` at each template's own L: Mercury's 0.47980, Mars's 1.08465.
    const mercury = templateQ("mercury", 1);
    const mars = templateQ("mars", 0.5);
    expect(regolithQ).toBeCloseTo(0.626, 3);
    expect(mercury).toBeCloseTo(0.4798, 4);
    expect(mars).toBeCloseTo(1.0846, 3);
    // The airless rocks' (the Moon- and Ceres-like worlds, the reference Moon and Ceres) and the
    // reference Mars's dust: +30.5% and −42.3%.
    expect(regolithQ / mercury - 1).toBeCloseTo(0.305, 3);
    expect(regolithQ / mars - 1).toBeCloseTo(-0.423, 3);
  });
});
