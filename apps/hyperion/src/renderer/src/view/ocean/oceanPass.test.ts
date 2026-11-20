import { describe, expect, it } from "vitest";

import { add, norm, normalise, scale, sub, vec3 } from "../../geometry/vec3";
import { uniformClouds } from "../../test/cloudFixtures";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { waterWorldAnswer, waterWorldHeader } from "../../test/oceanFixtures";
import { R05_EARTH_REFERENCE } from "../atmosphere/earth";
import type { SunState } from "../atmosphere/hillaire";
import { NEAR_PLANE_M, perspectiveReversedInfinite } from "../camera/projection";
import { rotation3FromRows } from "../coords/rotation";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import { SUN_RADIUS_M } from "../lighting/hostDisc";
import { hostAnnuli } from "../lighting/hostLights";
import { AU_M } from "../scenes/kept";
import { aHostDisc } from "../../test/litFixtures";
import { IDENTITY_QUATERNION, lookAlong, rotate } from "../camera/quaternion";
import { BLOOM_THRESHOLD_EXPOSED } from "../post/bloom";
import { tileProjection } from "../stills/stillPlan";
import { specularPoint } from "./glint";
import { oceanSeaOf, STANDARD_GRAVITY_MPS2 } from "./oceanBody";
import { raySpheroidHit } from "./oceanFar";
import { packOceanFar } from "./oceanFarProbe";
import { oceanMapTextureSpec, packOceanTexels } from "./oceanMap";
import { oceanMaskOf } from "./oceanMask";
import {
  OCEAN_ECLIPSE_FIRST_OCCLUDER,
  OCEAN_ECLIPSE_ROWS,
  OCEAN_FAR_PASS_MATERIAL,
  type OceanDrawView,
  oceanDrawOptics,
  type OceanEclipse,
  OceanGlare,
  oceanPassDepth,
  oceanPassDraw,
  oceanPassRay,
  oceanPassUniforms,
  oceanSourcesOfRecord,
  oceanSourceVisible,
  packOceanEclipse,
} from "./oceanPass";

const EARTH = "0200080020000000.0300";

/** The Sun's illuminance at 1 au, per channel, lx: R06's disc's, near enough for the tests. */
const SUN_LX: readonly [number, number, number] = [1.27e5, 1.3e5, 1.25e5];

/** A water world's view from 3 × 10⁷ m out on +z, lit along `sun`, at `exposureScale`. */
async function viewOf(
  sun: ReturnType<typeof vec3>,
  exposureScale = 1e-4,
  suns: ReadonlyArray<SunState> = [
    { directionBodyFixed: normalise(sun), illuminanceLx: SUN_LX, angularRadiusRad: 4.65e-3 },
  ],
): Promise<OceanDrawView> {
  const engine = await countingRenderEngine();
  const answer = waterWorldAnswer(4000, 64, 152);
  const mask = oceanMaskOf(answer);
  const sea = oceanSeaOf(
    waterWorldHeader(EARTH),
    {
      answer,
      mask,
      texels: packOceanTexels(answer, mask, { from: 0, to: 0, weight: 0 }),
      texture: engine.createTexture(oceanMapTextureSpec(`ocean map ${EARTH}`, answer.level)),
    },
    [0.2, 0.18, 0.15],
  );
  if (sea === null) {
    throw new Error("the water world has a sea");
  }
  return {
    sea,
    camera: {
      positionM: vec3(0, 0, 3e7),
      orientation: lookAlong(vec3(0, 0, -1), vec3(0, 1, 0)),
    },
    gravityMps2: STANDARD_GRAVITY_MPS2,
    figureOnDatum: true,
    suns,
    exposureScale,
    thresholdPreExposed: BLOOM_THRESHOLD_EXPOSED,
    pixelAngleRad: 2e-4,
    whitecaps: true,
    eclipse: null,
  };
}

/** The Sun at 1 au along `sun` from a body of radius `radiusM`, a moon of `moonRadii` radii out there. */
function eclipseOf(
  sun: ReturnType<typeof vec3>,
  radiusM: number,
  moonAtRadii: number,
  moonRadii: number,
): OceanEclipse {
  const towards = normalise(sun);
  return {
    lights: [
      {
        direction: towards,
        distance: AU_M / radiusM,
        radius: SUN_RADIUS_M / radiusM,
        illuminance: SUN_LX,
        annuli: hostAnnuli(aHostDisc(), DISC_ANNULI_HIGH),
      },
    ],
    occluders: [{ centre: scale(towards, moonAtRadii), radius: moonRadii }],
    radiusM,
  };
}

describe("the ocean pass's material (R11.T8.b)", () => {
  it("is opaque, writes its depth, and binds the ocean map and R08's nine tables", () => {
    expect(OCEAN_FAR_PASS_MATERIAL).toMatchObject({
      displayName: "OCEAN",
      depthWrite: true,
      colourWrites: true,
      blend: "none",
      cullMode: "none",
    });
    expect(OCEAN_FAR_PASS_MATERIAL.textures?.map((t) => t.binding)).toEqual([
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
    ]);
    expect(OCEAN_FAR_PASS_MATERIAL.textures?.[0]).toEqual({
      name: "oceanMap",
      binding: 0,
      viewDimension: "2d-array",
    });
  });
});

describe("a sea's draw (R11.T8.b)", () => {
  it("takes its optics in cd/m²: the cap the threshold over the pre-exposure, the brightest source's light, no sky", async () => {
    const view = await viewOf(vec3(1, 0, 0));
    const optics = oceanDrawOptics(view);
    expect(optics.glintCapCdM2).toBeCloseTo(BLOOM_THRESHOLD_EXPOSED / 1e-4, 6);
    expect(optics.sunLux).toEqual(SUN_LX);
    expect(optics.skyLux).toEqual([0, 0, 0]);
    expect(optics.sunDirection).toEqual(vec3(1, 0, 0));
    expect(optics.equatorialRadiusM).toBe(6_378_137 - 120);
  });

  it("packs its camera, the view's axes, the datum and its sources pre-exposed, each with its angular radius", async () => {
    const view = await viewOf(vec3(1, 0, 0));
    const u = oceanPassUniforms(view);
    expect(Array.from(u["camera"] ?? [])).toEqual([0, 0, 3e7, 1]);
    // Looking down −z with +y up: the view's z is the body's +z, its x the body's +x.
    const axes = ["viewX", "viewY", "viewZ"].map((name) =>
      Array.from(u[name] ?? [], (v) => Math.round(v * 1e6) / 1e6 + 0),
    );
    expect(axes).toEqual([
      [1, 0, 0, 0],
      [0, 1, 0, 0],
      [0, 0, 1, 0],
    ]);
    expect(Array.from(u["datum"] ?? [])).toEqual(
      Array.from(new Float32Array([6_378_137, 6_356_752.314245, -120, 1])),
    );
    expect(Array.from(u["source0"] ?? [])).toEqual([1, 0, 0, 0]);
    // Its light's w its angular radius, which the cloudy forms' tap takes (R11.T7.c).
    expect(Array.from(u["sourceLight0"] ?? [])).toEqual(
      Array.from(new Float32Array([...SUN_LX.map((x) => x * 1e-4), 4.65e-3])),
    );
    // No second source: a dark one along +z.
    expect(Array.from(u["sourceLight1"] ?? [])).toEqual([0, 0, 0, 0]);
  });

  it("packs its OceanFar with its cap and light pre-exposed, row by row", async () => {
    const view = await viewOf(vec3(1, 0, 0));
    const u = oceanPassUniforms(view);
    const optics = oceanDrawOptics(view);
    const far = packOceanFar({
      ...optics,
      glintCapCdM2: BLOOM_THRESHOLD_EXPOSED,
      sunLux: [SUN_LX[0] * 1e-4, SUN_LX[1] * 1e-4, SUN_LX[2] * 1e-4],
      skyLux: [0, 0, 0],
    });
    const rows = [
      "farFigure",
      "farSun",
      "farSunLux",
      "farSkyLux",
      "farIndex",
      "farAttenuation",
      "farDeep",
      "farBed",
    ].flatMap((name) => Array.from(u[name] ?? []));
    expect(rows).toEqual(Array.from(far));
    expect(u["farSun"]?.[3]).toBeCloseTo(BLOOM_THRESHOLD_EXPOSED, 4);
  });

  it("draws its two brightest sources at most, each with its light slot", async () => {
    const source = (x: number, slot: number): SunState => ({
      directionBodyFixed: normalise(vec3(x, 0, 1)),
      illuminanceLx: SUN_LX,
      angularRadiusRad: 4.65e-3,
      curveSlot: slot,
    });
    const view = await viewOf(vec3(1, 0, 0), 1e-4, [source(1, 0), source(-1, 1), source(0, 2)]);
    const u = oceanPassUniforms(view);
    expect(u["datum"]?.[3]).toBe(2);
    expect(u["source1"]?.[3]).toBe(1);
    expect(u["source1"]?.[0]).toBeLessThan(0);
  });

  it("binds the map under its name and the tables under theirs", async () => {
    const view = await viewOf(vec3(1, 0, 0));
    const engine = await countingRenderEngine();
    const tables = {
      body: engine.createTexture(oceanMapTextureSpec("body", 5)),
      transmittance: engine.createTexture(oceanMapTextureSpec("transmittance", 5)),
      absorberColumns: engine.createTexture(oceanMapTextureSpec("columns", 5)),
      absorberPressureColumns: engine.createTexture(oceanMapTextureSpec("pressure", 5)),
      lastAbsorberPressureColumn: engine.createTexture(oceanMapTextureSpec("last", 5)),
      absorberCurves: engine.createTexture(oceanMapTextureSpec("curves", 5)),
      irradiance: engine.createTexture(oceanMapTextureSpec("irradiance", 5)),
      belowDeck: engine.createTexture(oceanMapTextureSpec("deck", 5)),
      sourceTransmittance: engine.createTexture(oceanMapTextureSpec("sources", 5)),
    };
    const rows = { kind: "buffer", name: "eclipse rows", bytes: 16 * OCEAN_ECLIPSE_ROWS } as const;
    const draw = oceanPassDraw(
      { kind: "mesh", name: "triangle" },
      { kind: "material", name: "ocean far pass" },
      view.sea.map.texture,
      tables,
      rows,
      oceanPassUniforms(view),
    );
    expect(draw.storageBuffers?.["oceanEclipse"]).toBe(rows);
    expect(draw.textures["oceanMap"]).toBe(view.sea.map.texture);
    expect(draw.textures["atmosphereBody"]).toBe(tables.body);
    expect(draw.textures["atmosphereSourceTable"]).toBe(tables.sourceTransmittance);
  });
});

describe("a sea's eclipses (R11.T8.b)", () => {
  const R = 6.371e6;

  it("packs a header, each light's place, size and annuli, then each occluder", () => {
    const eclipse = eclipseOf(vec3(1, 0, 0), R, 3, 0.3);
    const rows = packOceanEclipse(eclipse);
    expect(rows).toHaveLength(4 * OCEAN_ECLIPSE_ROWS);
    expect(Array.from(rows.subarray(0, 4))).toEqual(
      Array.from(new Float32Array([1, 1, DISC_ANNULI_HIGH, R])),
    );
    expect(Array.from(rows.subarray(4, 8))).toEqual(
      Array.from(new Float32Array([1, 0, 0, AU_M / R])),
    );
    expect(rows[11]).toBeCloseTo(SUN_RADIUS_M / R, 3);
    const annuli = hostAnnuli(aHostDisc(), DISC_ANNULI_HIGH);
    expect(Array.from(rows.subarray(12, 16))).toEqual(
      Array.from(new Float32Array([1, 2, 3, 4].map((k) => annuli[0].edges[k] ?? 1))),
    );
    const occluder = 4 * OCEAN_ECLIPSE_FIRST_OCCLUDER;
    expect(Array.from(rows.subarray(occluder, occluder + 4))).toEqual(
      Array.from(new Float32Array([3, 0, 0, 0.3])),
    );
    // None: no light, so no source is eclipsed; the radius 1, so no place is divided by 0.
    expect(Array.from(packOceanEclipse(null).subarray(0, 4))).toEqual([0, 0, 0, 1]);
  });

  it("hides the whole source in a moon's umbra, none of it clear of the moon, and part in its penumbra", () => {
    // The moon of 0.3 radii at 3 radii sunward: its umbra on the sub-solar point.
    const eclipse = eclipseOf(vec3(1, 0, 0), R, 3, 0.3);
    expect(oceanSourceVisible(eclipse, 0, vec3(R, 0, 0))).toEqual([0, 0, 0]);
    expect(oceanSourceVisible(eclipse, 0, vec3(0, R, 0))).toEqual([1, 1, 1]);
    // At the shadow's edge, 0.3 radii off the axis: part of the disc shows.
    const edge = oceanSourceVisible(
      eclipse,
      0,
      vec3(Math.sqrt(R * R - (0.3 * R) ** 2), 0.3 * R, 0),
    );
    expect(edge[1]).toBeGreaterThan(0.05);
    expect(edge[1]).toBeLessThan(0.95);
    // A light the eclipse does not hold, and no eclipse, are whole.
    expect(oceanSourceVisible(eclipse, 1, vec3(R, 0, 0))).toEqual([1, 1, 1]);
    expect(oceanSourceVisible(null, 0, vec3(R, 0, 0))).toEqual([1, 1, 1]);
  });

  it("takes the record's lights and occluders into the body's axes, each light's slot its rank, past the air's slots none", () => {
    const eclipse = eclipseOf(vec3(1, 0, 0), R, 3, 0.3);
    // Body-fixed to galactic: a quarter turn about z, +x body-fixed along galactic +y.
    const quarter = rotation3FromRows([vec3(0, -1, 0), vec3(1, 0, 0), vec3(0, 0, 1)]);
    const record = { lights: [eclipse.lights[0], eclipse.lights[0]], occluders: eclipse.occluders };
    const lights = record.lights.flatMap((light) => (light === undefined ? [] : [light]));
    const taken = oceanSourcesOfRecord({ lights, occluders: record.occluders }, quarter, R, 1);
    expect(taken.suns).toHaveLength(1);
    expect(taken.suns[0]?.curveSlot).toBe(0);
    const d = taken.suns[0]?.directionBodyFixed ?? vec3(9, 9, 9);
    expect([d.x, d.y, d.z].map((v) => Math.round(v * 1e9) / 1e9 + 0)).toEqual([0, -1, 0]);
    expect(taken.eclipse.occluders[0]?.centre.y).toBeCloseTo(-3, 12);
    expect(taken.suns[0]?.angularRadiusRad).toBeCloseTo(Math.asin(SUN_RADIUS_M / AU_M), 12);
    expect(oceanSourcesOfRecord({ lights, occluders: [] }, quarter, R, 4).suns).toHaveLength(2);
  });
});

describe("the pass's ray and depth, as its shader forms them (R11.T8.b)", () => {
  const size = { widthPx: 65, heightPx: 33 };

  it("is the camera's forward at the centre pixel of a view", () => {
    const projection = perspectiveReversedInfinite(1.1, size.widthPx / size.heightPx, NEAR_PLANE_M);
    const orientation = lookAlong(vec3(0.3, -1, 0.2), vec3(0, 0, 1));
    const ray = oceanPassRay(32, 16, size, projection, orientation);
    const forward = rotate(orientation, vec3(0, 0, -1));
    expect(norm(sub(ray.direction, forward))).toBeLessThan(1e-12);
    expect(ray.viewLength).toBeCloseTo(1, 12);
  });

  it.each([
    ["a view's projection", perspectiveReversedInfinite(1.1, 65 / 33, NEAR_PLANE_M)],
    [
      "a still's tile, off its axis",
      tileProjection(
        1.1,
        { widthPx: 130, heightPx: 66 },
        {
          column: 1,
          row: 0,
          rect: { x0Px: 65, y0Px: 0, x1Px: 130, y1Px: 33 },
          texture: { widthPx: 65, heightPx: 33 },
          bytesPerRow: 4 * 65,
        },
      ),
    ],
  ] as const)(
    "lands back on its pixel's centre through %s, its depth the projected hit's",
    (_, projection) => {
      const orientation = IDENTITY_QUATERNION;
      for (const [x, y] of [
        [0, 0],
        [64, 32],
        [10, 25],
      ] as const) {
        const ray = oceanPassRay(x, y, size, projection, orientation);
        const t = 1e7;
        const point = scale(ray.direction, t);
        const p = (k: number): number => projection[k] ?? Number.NaN;
        const clip = [
          p(0) * point.x + p(8) * point.z,
          p(5) * point.y + p(9) * point.z,
          p(14),
          -point.z,
        ] as const;
        const xPx = ((clip[0] / clip[3] + 1) / 2) * size.widthPx;
        const yPx = ((1 - clip[1] / clip[3]) / 2) * size.heightPx;
        expect(xPx).toBeCloseTo(x + 0.5, 6);
        expect(yPx).toBeCloseTo(y + 0.5, 6);
        expect(oceanPassDepth(t, ray.viewLength, projection)).toBeCloseTo(clip[2] / clip[3], 15);
      }
    },
  );

  it("meets the sea's spheroid where its twin's ray does, from the draw's camera", async () => {
    const view = await viewOf(vec3(1, 0, 0));
    const projection = perspectiveReversedInfinite(0.6, 1, NEAR_PLANE_M);
    const ray = oceanPassRay(
      8,
      8,
      { widthPx: 16, heightPx: 16 },
      projection,
      view.camera.orientation,
    );
    const optics = oceanDrawOptics(view);
    const hit = raySpheroidHit(
      view.camera.positionM,
      ray.direction,
      optics.equatorialRadiusM,
      optics.polarRadiusM,
    );
    expect(hit).not.toBeNull();
    // Straight down from 3 × 10⁷ m onto the pole: the polar radius raised by the sea level.
    expect(hit?.tM).toBeGreaterThan(3e7 - 6_356_752.314245 - 1e5);
  });
});

describe("a sea's glare source (R11.T8.b)", () => {
  it("is none without a source, and none where the glint stays under the cap", async () => {
    const glare = new OceanGlare();
    expect(
      glare.sourceOf(await viewOf(vec3(1, 0, 0), 1e-4, []), IDENTITY_QUATERNION, null),
    ).toBeNull();
    // At 10⁻⁴ the cap is 1.6 × 10⁵ cd/m², over the glint's peak.
    expect(glare.sourceOf(await viewOf(vec3(1, 0, 1)), IDENTITY_QUATERNION, null)).toBeNull();
  });

  it("lies at the specular point, its light dimmed by the air both ways", async () => {
    const view = await viewOf(vec3(1, 0, 1), 1e-2);
    const bare = new OceanGlare().sourceOf(view, IDENTITY_QUATERNION, null);
    expect(bare).not.toBeNull();
    const aired = new OceanGlare().sourceOf(view, IDENTITY_QUATERNION, {
      medium: R05_EARTH_REFERENCE,
      curves: [],
    });
    expect(aired).not.toBeNull();
    // Between the nadir and the Sun, along the direction from the camera.
    expect(bare?.direction.x).toBeGreaterThan(0);
    expect(bare?.direction.z).toBeLessThan(0);
    // Through Earth's air at 45° each way, the blue most of all.
    const ratio = [0, 1, 2].map((k) => (aired?.luminance[k] ?? 0) / (bare?.luminance[k] ?? 1));
    expect(ratio[2]).toBeLessThan(ratio[0] ?? 0);
    expect(ratio[0]).toBeLessThan(1);
    expect(ratio[2]).toBeGreaterThan(0.2);
  });

  it("turns its direction into the scene's axes", async () => {
    const view = await viewOf(vec3(1, 0, 1), 1e-2);
    const glare = new OceanGlare();
    const own = glare.sourceOf(view, IDENTITY_QUATERNION, null);
    // A body whose body-fixed axes are the scene's turned a quarter about z.
    const quarter = { w: Math.SQRT1_2, x: 0, y: 0, z: Math.SQRT1_2 };
    const turned = glare.sourceOf(view, quarter, null);
    const expected = own === null ? null : rotate({ ...quarter, z: -quarter.z }, own.direction);
    expect(norm(sub(turned?.direction ?? vec3(0, 0, 0), expected ?? vec3(9, 9, 9)))).toBeLessThan(
      1e-12,
    );
  });

  it("holds a source while its air's parts stand, though the frame names them afresh", async () => {
    const view = await viewOf(vec3(1, 0, 1), 1e-2);
    const glare = new OceanGlare();
    const curves: ReadonlyArray<never> = [];
    const first = glare.sourceOf(view, IDENTITY_QUATERNION, {
      medium: R05_EARTH_REFERENCE,
      curves,
    });
    const again = glare.sourceOf(view, IDENTITY_QUATERNION, {
      medium: R05_EARTH_REFERENCE,
      curves,
    });
    // Held: the very values it formed; another medium forms it afresh.
    expect(again?.luminance).toBe(first?.luminance);
    const other = glare.sourceOf(view, IDENTITY_QUATERNION, {
      medium: { ...R05_EARTH_REFERENCE },
      curves,
    });
    expect(other?.luminance).not.toBe(first?.luminance);
  });

  it("lets every sea's source go but those it is told to keep", async () => {
    const view = await viewOf(vec3(1, 0, 1), 1e-2);
    const glare = new OceanGlare();
    const first = glare.sourceOf(view, IDENTITY_QUATERNION, null);
    expect(glare.sourceOf(view, IDENTITY_QUATERNION, null)?.luminance).toBe(first?.luminance);
    glare.keepOnly(new Set([view.sea.body]));
    expect(glare.sourceOf(view, IDENTITY_QUATERNION, null)?.luminance).toBe(first?.luminance);
    glare.keepOnly(new Set());
    const formed = glare.sourceOf(view, IDENTITY_QUATERNION, null);
    expect(formed).not.toBeNull();
    expect(formed?.luminance).not.toBe(first?.luminance);
  });

  it("dims its light by its clouds' direct share at the specular point, formed again as they clear (R11.T7.c)", async () => {
    const view = await viewOf(vec3(1, 0, 1), 1e-2);
    const [a, c] = [6_378_137, 6_356_752.314245];
    const bare = new OceanGlare().sourceOf(view, IDENTITY_QUATERNION, null);
    expect(bare).not.toBeNull();
    // A clear sky's clouds dim nothing.
    const glare = new OceanGlare();
    const clear = glare.sourceOf(view, IDENTITY_QUATERNION, null, uniformClouds(0, a, c));
    expect(clear?.luminance).toEqual(bare?.luminance);
    expect(clear?.excessLuminance).toEqual(bare?.excessLuminance);
    // Held while they stand, though the frame names them afresh.
    expect(glare.sourceOf(view, IDENTITY_QUATERNION, null, uniformClouds(0, a, c))?.luminance).toBe(
      clear?.luminance,
    );
    // A whole τ̄ 8 deck lets under 10⁻⁴ of the beam through: the glint stays under the cap.
    expect(glare.sourceOf(view, IDENTITY_QUATERNION, null, uniformClouds(1, a, c))).toBeNull();
    // Formed again once they clear.
    expect(
      glare.sourceOf(view, IDENTITY_QUATERNION, null, uniformClouds(0, a, c))?.luminance,
    ).toEqual(bare?.luminance);
  });

  it("is none where the specular point lies in a moon's umbra", async () => {
    const view = await viewOf(vec3(1, 0, 1), 1e-2);
    const bare = new OceanGlare().sourceOf(view, IDENTITY_QUATERNION, null);
    expect(bare).not.toBeNull();
    // A moon two radii from the specular point towards the Sun, 0.6 radii across, hiding the Sun.
    const optics = oceanDrawOptics(view);
    const point = specularPoint(
      view.camera.positionM,
      optics.sunDirection,
      optics.equatorialRadiusM,
    );
    if (point === null) {
      throw new Error("the specular point is seen");
    }
    const a = 6_378_137;
    const at = vec3(point.pointM.x / a, point.pointM.y / a, point.pointM.z / a);
    const eclipse: OceanEclipse = {
      ...eclipseOf(vec3(1, 0, 1), a, 3, 0.3),
      occluders: [{ centre: add(at, scale(optics.sunDirection, 2)), radius: 0.6 }],
    };
    expect(new OceanGlare().sourceOf({ ...view, eclipse }, IDENTITY_QUATERNION, null)).toBeNull();
  });
});
