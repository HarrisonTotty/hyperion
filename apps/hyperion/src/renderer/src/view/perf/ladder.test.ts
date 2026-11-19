/**
 * The ladder's mechanical audit (plan R12, R12.T7.a): every row of the brainstorm's settings ladder
 * has a high and a low value in R05's `SETTINGS` (or R08's `settings.ts`, through it), each taken
 * by the feature that reads it; and, by test name, R02's label block states the setting and R05's
 * and R10's grounded-body tests run at both settings and in both styles.
 *
 * @remarks
 * The rows are the brainstorm's (Performance budget, "The ladder, stated as policy"), and each
 * row's fields R12.T0's inventory (`12-measurements-and-settings-ladder/records/R12.T0.md`). A field
 * is accepted where its feature's own function takes it and gives a result of the setting's form;
 * a boolean is accepted by its type, and the test holds its high and low to the ladder's. The
 * smoke page's `ladder` part draws each setting on SwiftShader (`smoke/ladder.ts`).
 */

import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { WATER_RECORD } from "../../test/oceanFixtures";
import { HillaireAtmosphere, type AtmosphereCamera, type SunState } from "../atmosphere/hillaire";
import { R05_EARTH_REFERENCE, referenceSunIlluminanceLx } from "../atmosphere/earth";
import { ResolutionController } from "../budget/resolutionController";
import { frameGpuBudgetMs } from "../budget/viewBudget";
import { quaternion } from "../camera/quaternion";
import { cloudFieldParams } from "../clouds/cloudDensity";
import { LAYER_OCTAVES } from "../clouds/cloudLayer";
import { pagePoolLayout } from "../clusters/pages";
import { instancePathOn } from "../clusters/particles";
import { edgeStandInForm } from "../clusters/terrain";
import { oceanLiquidOf } from "../ocean/liquids";
import { OCEAN_GRID_CELL_PX, oceanGridSize, oceanNearSet } from "../ocean/oceanNear";
import { EARTH_SEA } from "../ocean/seaState";
import { renderViewport } from "../photoreal/internalScale";
import { bloomKernel } from "../post/bloom";
import { histogramWorkgroups } from "../post/histogram";
import { QUALITY_SETTINGS, type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import { mipSizes } from "../sky/mips";
import { decorationAllowed } from "../surface/decoration/decorationSetting";
import { fittedTerrainLayout } from "../terrain/gpu/resources";
import { handoverDetailShare, handoverRangeM } from "../terrain/handover";
import { sessionMaterials } from "../terrain/materialSet";
import { selectionTolerancePx } from "../terrain/selectionTolerance";
import { terrainSlotLayout } from "../terrain/slotLayout";
import { wireframeToleranceRad } from "../wireframe/meshOccluders";

/** The brainstorm's ladder rows, in its order (Performance budget). */
const LADDER_ROWS = [
  "Clouds",
  "Terrain detail",
  "Ocean",
  "Shadows",
  "Atmosphere",
  "Scatter",
  "Rings",
  "Resolution",
] as const;

/** A row of the brainstorm's ladder. */
type LadderRow = (typeof LADDER_ROWS)[number];

/** A device's limits that hold either setting's own vertex path (`fittedTerrainLayout`). */
const ROOMY_LIMITS = {
  maxStorageBufferBindingSize: 2 ** 31 - 4,
  maxBufferSize: 2 ** 32,
  maxTextureDimension2D: 16_384,
} as const;

/** Whether `setting` draws the shadow cascades: the terrain's casters among its materials. */
function cascadesOn(setting: QualitySetting): boolean {
  return sessionMaterials(setting, ROOMY_LIMITS).some((key) => key.kind.startsWith("shadow"));
}

/**
 * Each row's values on a setting that its high and low must tell apart: the low form is another
 * form, not the high one under another name.
 */
const ROW_FORMS: Readonly<Record<LadderRow, (setting: QualitySetting) => unknown>> = {
  Clouds: (s) => [SETTINGS[s].clouds, LAYER_OCTAVES[s]],
  "Terrain detail": (s) => [SETTINGS[s].terrain, SETTINGS[s].decoration],
  Ocean: (s) => [
    SETTINGS[s].oceanComponents,
    SETTINGS[s].oceanRefraction,
    SETTINGS[s].whitecaps,
    OCEAN_GRID_CELL_PX[s],
  ],
  Shadows: (s) => [cascadesOn(s), SETTINGS[s].cloudShadows],
  Atmosphere: (s) => SETTINGS[s].atmosphere,
  Scatter: (s) => SETTINGS[s].scatterDensity,
  Rings: (s) => [SETTINGS[s].rings, SETTINGS[s].ringShadowOnBody],
  Resolution: (s) => [SETTINGS[s].terrain.renderHeightPx, SETTINGS[s].photoreal],
};

describe("the ladder's rows (plan R12, R12.T7.a)", () => {
  it.each(LADDER_ROWS)("gives the %s row a high and a low form that differ", (row) => {
    const forms = ROW_FORMS[row];
    expect(forms("high")).toBeDefined();
    expect(forms("low")).toBeDefined();
    expect(forms("low")).not.toEqual(forms("high"));
  });
});

/** A 1080p canvas, which high presents at its own size and low at 720 rows. */
const CANVAS_1080P = { widthPx: 1920, heightPx: 1080 } as const;

/** A pixel's angle on that canvas at a 60° field, rad. */
const RAD_PER_PX_1080P = (60 * Math.PI) / 180 / 1920;

/** The eye the bloom's spread is fitted for (`bloom.test.ts`'). */
const EYE = { ageYears: 25, pigmentation: 0.5 };

describe("the Clouds row: a raymarched volume high, a layer low", () => {
  it("draws the volume on high and the layer on low", () => {
    expect([SETTINGS.high.clouds, SETTINGS.low.clouds]).toEqual(["volume", "layer"]);
  });

  it.each(QUALITY_SETTINGS)("takes %s's layer octaves into the cloud field", (setting) => {
    const octaves = LAYER_OCTAVES[setting];
    const params = cloudFieldParams({
      seed: 7,
      condensate: { inCloudOpticalDepth550: 8 },
      radiusM: 6_371_000,
      time: { seconds: 0, nanos: 0 },
      rate: 1,
      windMps: 20,
      octaves,
    });
    expect([params.octaves, params.offsets.length, params.octaveFades.length]).toEqual([
      octaves,
      6 * octaves,
      octaves,
    ]);
  });
});

describe("the Terrain detail row: full depth and decoration high; shallower, none low", () => {
  it.each(QUALITY_SETTINGS)("takes %s's τ into the hand-over's range", (setting) => {
    const rangeM = handoverRangeM(8_000, SETTINGS[setting].terrain.tauPx, RAD_PER_PX_1080P);
    expect(Number.isFinite(rangeM) && rangeM > 0).toBe(true);
  });

  it.each(QUALITY_SETTINGS)("takes %s's τ into selection's tolerance", (setting) => {
    const { tauPx } = SETTINGS[setting].terrain;
    expect(selectionTolerancePx(tauPx)).toBeLessThan(tauPx);
  });

  it("puts low's hand-over nearer than high's: a shallower quadtree away from the camera", () => {
    const at = (setting: QualitySetting): number =>
      handoverRangeM(8_000, SETTINGS[setting].terrain.tauPx, RAD_PER_PX_1080P);
    expect(at("low")).toBeLessThan(at("high"));
  });

  it.each(QUALITY_SETTINGS)("lays out %s's cache of its normals into its slots", (setting) => {
    expect(terrainSlotLayout(SETTINGS[setting].terrain, "lit").slotCount).toBeGreaterThanOrEqual(6);
  });

  it.each(QUALITY_SETTINGS)("fits %s's own vertex path on a device of room", (setting) => {
    const terrain = SETTINGS[setting].terrain;
    expect(fittedTerrainLayout(terrain, ROOMY_LIMITS, "lit")?.vertexPath).toBe(terrain.vertexPath);
  });

  it.each(QUALITY_SETTINGS)("shares %s's sub-cell detail at the hand-over", (setting) => {
    const share = handoverDetailShare(2, 1, SETTINGS[setting].terrain.handoverDetail);
    expect(share >= 0 && share <= 1).toBe(true);
  });

  it.each(QUALITY_SETTINGS)("pools %s's mesh pages", (setting) => {
    const pool = pagePoolLayout(SETTINGS[setting].geometry.pageBytes);
    expect(Math.min(pool.rootSlots, pool.pageSlots)).toBeGreaterThan(0);
  });

  it.each(QUALITY_SETTINGS)("takes %s's wireframe occluder's tolerance as an angle", (setting) => {
    const toleranceRad = wireframeToleranceRad(
      SETTINGS[setting].geometry.meshTolerancePx.wireframeOccluder,
      Math.PI / 3,
      CANVAS_1080P,
    );
    expect(Number.isFinite(toleranceRad) && toleranceRad > 0).toBe(true);
  });

  it.each(QUALITY_SETTINGS)("forms %s's whole edge stand-in as its edge patch", (setting) => {
    expect(edgeStandInForm(SETTINGS[setting].geometry.edgeStandIns, 0, null, true)).toBe(
      "edge-patch",
    );
  });

  it("draws decoration on high and none on low, in the photorealistic style", () => {
    const frame = { style: "photorealistic", floorStepped: false } as const;
    expect(QUALITY_SETTINGS.map((s) => decorationAllowed(SETTINGS[s], frame))).toEqual([
      true,
      false,
    ]);
  });
});

describe("the Ocean row: 32 components, refraction and whitecaps high; about eight, none low", () => {
  const sea = { liquid: oceanLiquidOf(WATER_RECORD), air: EARTH_SEA.air };

  it.each(QUALITY_SETTINGS)("builds %s's Gerstner components on a windy sea", (setting) => {
    const count = SETTINGS[setting].oceanComponents;
    expect(oceanNearSet(sea, EARTH_SEA.gravityMps2, 12, count).components).toHaveLength(count);
  });

  it.each(QUALITY_SETTINGS)("grids %s's near sea at its cell", (setting) => {
    const { columns, rows } = oceanGridSize(1920, 1080, OCEAN_GRID_CELL_PX[setting]);
    expect(columns >= 2 && rows >= 2).toBe(true);
  });

  it("refracts and caps on high alone", () => {
    expect(
      QUALITY_SETTINGS.map((s) => [SETTINGS[s].oceanRefraction, SETTINGS[s].whitecaps]),
    ).toEqual([
      [true, true],
      [false, false],
    ]);
  });
});

describe("the Shadows row: cascades and cloud shadows high; the horizon map and the layer's shadow low", () => {
  it("draws the cascades' casters on high alone", () => {
    expect(QUALITY_SETTINGS.map(cascadesOn)).toEqual([true, false]);
  });

  it("lets clouds shadow the terrain on both settings (amended 2026-10-09)", () => {
    expect(QUALITY_SETTINGS.map((s) => SETTINGS[s].cloudShadows)).toEqual([true, true]);
  });
});

/** WGS 84's equatorial radius, m. */
const WGS84_A_M = 6_378_137;

/** WGS 84 (NIMA TR8350.2, Table 3.1), the body the atmosphere is drawn about. */
const WGS84 = { equatorialRadiusM: WGS84_A_M, polarRadiusM: WGS84_A_M * (1 - 1 / 298.257223563) };

/** A camera 400 km over the equator, looking north along the horizon (`hillaire.test.ts`'). */
const ORBIT: AtmosphereCamera = {
  positionM: { x: WGS84_A_M + 400_000, y: 0, z: 0 },
  orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
  fovXRad: Math.PI / 2,
  viewport: { widthPx: 64, heightPx: 32 },
};

/** The Sun overhead at the camera's point. */
const NOON: SunState = {
  directionBodyFixed: { x: 1, y: 0, z: 0 },
  illuminanceLx: referenceSunIlluminanceLx(),
  angularRadiusRad: 0.004_65,
};

/** The workgroups a frame's sky view dispatched, in all. */
function skyViewGroups(frame: { readonly skyView: ReadonlyArray<number> } | undefined): number {
  return (frame?.skyView ?? []).reduce((product, n) => product * n, 1);
}

describe("the Atmosphere row: full tables and perspective on everything high; smaller, terrain alone low", () => {
  /** The sky view's dispatch, and the aerial perspective the frame publishes, at `setting`. */
  async function frameAt(setting: QualitySetting): Promise<{
    readonly skyView: ReadonlyArray<number>;
    readonly aerial: boolean;
  }> {
    const engine = await countingRenderEngine();
    const atmosphere = new HillaireAtmosphere(
      engine,
      R05_EARTH_REFERENCE,
      SETTINGS[setting].atmosphere,
      WGS84,
    );
    try {
      atmosphere.drawFrame(ORBIT, [NOON], {
        colour: { kind: "texture", name: "terrain colour" },
        depth: { kind: "texture", name: "terrain depth" },
        nearM: 0.1,
        exposureScale: 1e-4,
      });
      // The frame's tables read, as the smoke's are: the sky view and the volume made for it.
      const tables = atmosphere.frameTables;
      expect(tables.skyView.kind).toBe("texture");
      const dispatch = engine.dispatched.find((each) => each.kernel === "atmosphere sky view");
      const workgroups = dispatch?.workgroups;
      return {
        skyView: Array.isArray(workgroups) ? [...workgroups] : [],
        aerial: atmosphere.aerialPerspective(ORBIT) !== null,
      };
    } finally {
      atmosphere.dispose();
    }
  }

  it.each(QUALITY_SETTINGS)("draws a frame at %s's table sizes", async (setting) => {
    const { skyView } = await frameAt(setting);
    expect(skyView).toHaveLength(3);
    expect(skyView.every((n) => n > 0)).toBe(true);
  });

  it("dispatches a smaller sky view on low than on high", async () => {
    const [high, low] = await Promise.all(QUALITY_SETTINGS.map(frameAt));
    expect(skyViewGroups(low)).toBeLessThan(skyViewGroups(high));
  });

  it("gives the scene its aerial perspective on high alone, the terrain alone taking it on low", async () => {
    const frames = await Promise.all(QUALITY_SETTINGS.map(frameAt));
    expect(frames.map((frame) => frame.aerial)).toEqual([true, false]);
  });
});

describe("the Scatter row: instanced high; a token density or none low", () => {
  it("draws scatter at its full density on high and none on low", () => {
    expect(QUALITY_SETTINGS.map((s) => SETTINGS[s].scatterDensity)).toEqual([1, 0]);
  });

  it("makes the instance path on high alone, which scatter and ring particles draw by", () => {
    expect(QUALITY_SETTINGS.map((s) => instancePathOn(SETTINGS[s]))).toEqual([true, false]);
  });
});

describe("the Rings row: particles and both shadows high; the annulus and the planet's shadow low", () => {
  it("draws particles with the ring's shadow on its planet on high, the annulus without on low", () => {
    expect(QUALITY_SETTINGS.map((s) => [SETTINGS[s].rings, SETTINGS[s].ringShadowOnBody])).toEqual([
      ["particles", true],
      ["annulus", false],
    ]);
  });
});

describe("the Resolution row: 1080p native high, 720p presented upscaled low", () => {
  it("renders a 1080p canvas at its own size on high and at 1280 × 720 on low", () => {
    expect(
      QUALITY_SETTINGS.map((s) => renderViewport(CANVAS_1080P, SETTINGS[s].terrain.renderHeightPx)),
    ).toEqual([CANVAS_1080P, { widthPx: 1280, heightPx: 720 }]);
  });

  it.each(QUALITY_SETTINGS)("holds %s's internal scale within its bounds and budget", (setting) => {
    const { internalScaleBounds, budget } = SETTINGS[setting];
    const controller = new ResolutionController(internalScaleBounds, {
      periodMs: 1_000 / budget.photorealisticRateHz,
      gpuBudgetMs: frameGpuBudgetMs(budget.photorealisticRateHz, 0),
    });
    expect(controller.scale).toBe(internalScaleBounds[1]);
  });

  it.each(QUALITY_SETTINGS)("reads %s's histogram at its stride", (setting) => {
    const { histogramStride } = SETTINGS[setting].photoreal;
    expect(histogramWorkgroups(CANVAS_1080P, histogramStride).every((n) => n > 0)).toBe(true);
  });

  it.each(QUALITY_SETTINGS)("blooms %s's frame over its levels, its weights a whole", (setting) => {
    const { bloom } = SETTINGS[setting].photoreal;
    const { weights } = bloomKernel(bloom, "eye", RAD_PER_PX_1080P, EYE);
    expect([weights.length, weights.every((w) => w >= 0)]).toEqual([bloom.levels, true]);
    expect(weights.reduce((sum, w) => sum + w, 0)).toBeCloseTo(1, 6);
  });
});

describe("beyond the ladder: the sky's cube on each setting (R06.T13.f)", () => {
  it.each(QUALITY_SETTINGS)("takes %s's face size into the cube's mips", (setting) => {
    expect(mipSizes(SETTINGS[setting].sky.faceSizePx).length).toBeGreaterThan(1);
  });
});

/** The tests the check by name reads, as text, by their paths from here. */
const TESTS: Readonly<Record<string, string>> = import.meta.glob<string>(
  [
    "../../displays/view/ViewDisplaySetting.test.tsx",
    "../../displays/view/ViewLabelBlock.test.tsx",
    "../terrain/grounded.test.ts",
    "../terrain/levelBounds.test.ts",
  ],
  { query: "?raw", import: "default", eager: true },
);

/** The text of the test at `path`. */
function testText(path: string): string {
  const text = TESTS[path];
  if (text === undefined) {
    throw new Error(`no test at ${path}`);
  }
  return text;
}

/**
 * The text of the call that a test titled `title` heads: from the `it` or `it.each` before its title
 * to the title itself, where an `it.each`'s table is.
 */
function callBefore(text: string, title: string): string {
  const at = text.indexOf(title);
  if (at < 0) {
    throw new Error(`no test titled ${title}`);
  }
  const calls = [...text.slice(0, at).matchAll(/\bit(?:\.each)?\(/gu)];
  const start = calls.at(-1)?.index ?? 0;
  return text.slice(start, at + title.length);
}

/** levelBounds.test.ts' `STYLES`, the styles its grounded-body test selects in. */
function levelBoundsStyles(): string {
  const text = testText("../terrain/levelBounds.test.ts");
  const at = text.indexOf("const STYLES");
  return text.slice(at, text.indexOf("];", at));
}

/** A `STYLES` row map over both settings at the τ `tau` names. */
function bothSettingsAt(tau: string): RegExp {
  return new RegExp(
    String.raw`(?:\["high", "low"\] as const|QUALITY_SETTINGS)\)?\.map\(\(setting\) => \(\{\s+setting,\s+tauPx: ${tau},`,
    "u",
  );
}

describe("the check by test name (plan R12, R12.T7.a)", () => {
  it("has R02's label block state the setting it is given, QUALITY HIGH and LOW", () => {
    const call = callBefore(
      testText("../../displays/view/ViewDisplaySetting.test.tsx"),
      "reads QUALITY $words on the primary's label block",
    );
    expect(call).toContain(`{ setting: "high", words: "HIGH" }`);
    expect(call).toContain(`{ setting: "low", words: "LOW" }`);
  });

  it("has R02's label block show the terrain's annunciation as a steady statement", () => {
    expect(testText("../../displays/view/ViewLabelBlock.test.tsx")).toContain(
      "shows a terrain annunciation as a steady statement, not an alert or a fault",
    );
  });

  it("runs R05's grounded-body selection on both settings in both styles", () => {
    const call = callBefore(
      testText("../terrain/grounded.test.ts"),
      "selects the finest level under the body and the ramp's margin on %s",
    );
    // Each case's setting and τ: the setting's own (the photorealistic style) or 4 px (the
    // wireframe's, `WIREFRAME_TERRAIN_TAU_PX`).
    const cases = [...call.matchAll(/\["[^"]+", "(high|low)" as const, ([^\]]+)\]/gu)].map(
      ([, setting, tau]) => `${setting ?? ""} ${tau?.trim() === "4" ? "wireframe" : "lit"}`,
    );
    expect(cases.toSorted()).toEqual(["high lit", "high wireframe", "low lit", "low wireframe"]);
  });

  it("has R10's grounded-body test select on every world, setting and style", () => {
    expect(testText("../terrain/levelBounds.test.ts")).toContain(
      "draws the ground under a contact at the finest level on every world, setting and style",
    );
  });

  it("runs R10's grounded-body test's lit terrain at each setting's own τ", () => {
    expect(levelBoundsStyles()).toMatch(
      bothSettingsAt(String.raw`SETTINGS\[setting\]\.terrain\.tauPx`),
    );
  });

  it("runs R10's grounded-body test's wireframe at its τ on both settings", () => {
    expect(levelBoundsStyles()).toMatch(bothSettingsAt("WIREFRAME_TERRAIN_TAU_PX"));
  });
});
