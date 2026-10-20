/**
 * A view's live terrain in its frame (plan R10, R10.T16.c): the lights from its body's disc
 * record, its view from the plan's body, the draws and the mask through the session's seams, and
 * the survey edges every view's draw list marks, in both styles; and a wireframe view's terrain
 * (R10.T11), its contours at one interval and its bodies' graticules.
 */

import type { BodyIdHex, SurfaceHeaderDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  FIXTURE_MOON,
  FIXTURE_MOON_CENTRE_M,
  FIXTURE_PLANET,
  FIXTURE_SYSTEM,
  NO_TURN,
} from "../../test/viewFixtures";
import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { SESSION_SCENE } from "../../test/terrainSessionFixtures";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import { lawFor } from "../appearance/phase";
import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import {
  type BodyFrameOptions,
  type BodyFramePlan,
  type LitBodyInput,
  planLitBodies,
} from "../bodies/draw";
import { type DiscRecord, discReflectance } from "../bodies/discShading";
import { discSurfaceLaws } from "../bodies/discSurface";
import { rotationColumns } from "../bodies/smoothMesh";
import type { CameraPose } from "../camera/pose";
import type { ProjectionCamera } from "../camera/projection";
import { IDENTITY_QUATERNION, lookAlong, rotationRows } from "../camera/quaternion";
import { viewId } from "../camera/state";
import {
  IDENTITY_ROTATION,
  type Rotation3,
  rotateToBodyFixed,
  rotation3FromRows,
} from "../coords/rotation";
import type { DrawItem, MaterialHandle, TextureHandle } from "../engine/types";
import { DISC_ANNULI_HIGH } from "../lighting/annuli";
import { sunLikeHostDisc } from "../lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../lighting/planetshine";
import type { ViewScene } from "../scene/model";
import { AU_M } from "../scenes/kept";
import type { DrawCamera } from "../wireframe/drawList";
import { linearColour } from "../wireframe/submit";
import type { TerrainAnnunciation } from "./annunciation";
import { CLASS_MAP_LEVEL, classMapLawsOf } from "./classMap";
import { CoverageView, fieldCellIndex } from "./coverage";
import { uvToSt } from "./cube";
import {
  type FrameTerrainPass,
  LiveFrameTerrain,
  morePressingAnnunciation,
  surveyEdgeViewsOf,
  terrainLightingOf,
  terrainViewOf,
  type ViewTerrainSource,
  type WireframeTerrainInput,
  wireframeTerrainOf,
  type WireTerrainSource,
} from "./frameTerrain";
import { handoverTauPx, limbDistanceM, pixelAngleRad } from "./handover";
import type { PatchKey } from "./patchKey";
import { ResidentHeights } from "./residentHeights";
import { surveyEdgeMark } from "./surveyEdge";
import {
  PHASE_LAW,
  TERRAIN_CLASSES,
  TERRAIN_LAWS,
  terrainReflectance,
  terrainTexelLaws,
} from "./terrainMaterial";
import type { TerrainViewFrame, TerrainViewFrameInput } from "./terrainPass";
import { PoseIntervals } from "./poseInterval";
import { contourInterval } from "./terrainWire";
import { type BakedPatch, PALETTE_NONE, PHOTOMETRY_STRIDE } from "./workers/messages";

const BODY: BodyIdHex = "0200080020000000.0301";
const VIEW = viewId("primary");
const INSTRUMENT = viewId("instrument 1");

/** A Moon-sized sphere, m. */
const A = 1.7381e6;
const FIGURE = { equatorialRadiusM: A, polarRadiusM: A, pole: vec3(0, 0, 1) };

/** A 1080p view across 60°, looking along −z. */
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const VIEWPORT = { widthPx: 1920, heightPx: 1080 };
const EXPOSURE = 1e-4;

/** A rotation with no axis in common with the galactic ones. */
const ROTATION: Rotation3 = rotation3FromRows(
  rotationRows(lookAlong(normalise(vec3(1, 2, 3)), vec3(0, 0, 1))),
);

/** `v` under {@link CYCLIC}'s inverse, Rᵀ v = (v.y, v.z, v.x), exact. */
function cyclic(v: Vec3): Vec3 {
  return { x: v.y, y: v.z, z: v.x };
}

/** The cyclic permutation x → y → z → x: Rᵀ v = (v.y, v.z, v.x), exactly. */
const CYCLIC: Rotation3 = rotation3FromRows([vec3(0, 0, 1), vec3(1, 0, 0), vec3(0, 1, 0)]);

/** The Sun's direction from the body, towards the camera's side. */
const SUN = normalise(vec3(0.3, 0.2, 1));

/** The Regolith row's class, [B, V, R, code, L(0)], as the worker resolves it. */
const REGOLITH = [0.105, 0.142, 0.172, PHASE_LAW.regolith, 1] as const;

/** The body's uniform law: Mercury's template at a Moon-like p. */
const ELSEWHERE = lawFor([0.172, 0.142, 0.105], [0.48, 0.48, 0.48], "mercury");

const TEXTURE: TextureHandle = { kind: "texture", name: "class map" };

/** A one-class map of the Regolith row, unsurveyed ground under `ELSEWHERE`. */
function oneClassMap(): ClassMapDiscSurface {
  const photometry = new Float32Array(PHOTOMETRY_STRIDE);
  photometry.set(REGOLITH);
  const [law] = classMapLawsOf({
    level: CLASS_MAP_LEVEL,
    fieldLevel: 6,
    classes: new Uint8Array(2),
    photometry,
    texels: new Uint32Array(0),
    fractions: new Float32Array(0),
    weights: new Uint8Array(0),
    complete: true,
  });
  if (law === null || law === undefined) {
    throw new Error("the class has a law");
  }
  return {
    kind: "class-map",
    weights: TEXTURE,
    texels: { faceTexels: 64, classes: 1, data: new Uint8Array(0) },
    laws: [law],
    elsewhere: ELSEWHERE,
  };
}

/** The body `distanceM` ahead, turning by `rotation`, handed over by a relief of 4 px at its limb. */
function bodyAt(distanceM: number, extra: Partial<LitBodyInput> = {}): LitBodyInput {
  const limbM = limbDistanceM(distanceM, A);
  return {
    id: BODY,
    centreM: vec3(0.25, -0.5, -distanceM),
    figure: FIGURE,
    photometry: PROVISIONAL_PHOTOMETRY,
    rotation: ROTATION,
    handoverReliefM:
      4 * handoverTauPx("photorealistic", "high") * limbM * pixelAngleRad(CAMERA, VIEWPORT),
    lighting: undefined,
    ...extra,
  };
}

const OPTIONS: BodyFrameOptions = {
  camera: CAMERA,
  viewport: VIEWPORT,
  exposureScale: EXPOSURE,
  annuli: DISC_ANNULI_HIGH,
  planetshine: PLANETSHINE_SOURCES_HIGH,
  setting: "high",
};

/** The plan of `body` under the Sun 1 au along {@link SUN}. */
function planOf(body: LitBodyInput): BodyFramePlan {
  return planLitBodies(
    [body],
    [{ disc: sunLikeHostDisc(), centreM: add(body.centreM, scale(SUN, AU_M)) }],
    OPTIONS,
    new Map(),
  );
}

/** The record of `body`'s plan. */
function recordOf(body: LitBodyInput): DiscRecord {
  const record = planOf(body).discs[0];
  if (record === undefined) {
    throw new Error("the body has a record");
  }
  return record;
}

/** `body` with no rotation known. */
function unturned(body: LitBodyInput): LitBodyInput {
  const copy = { ...body };
  delete copy.rotation;
  return copy;
}

/** A plan handing the body over, 3 a from the camera, and its body. */
function handed(extra: Partial<LitBodyInput> = {}): {
  readonly plan: BodyFramePlan;
  readonly body: LitBodyInput;
} {
  const body = bodyAt(3 * A, extra);
  const plan = planOf(body);
  if (plan.meshes[0]?.role.kind !== "handed-over") {
    throw new Error("the body was not handed over");
  }
  return { plan, body };
}

describe("a body's terrain lights from its disc record (R10.T16.c; decision D5)", () => {
  const record: DiscRecord = {
    ...recordOf(bodyAt(3 * A)),
    occluders: [{ centre: vec3(0.5, -2, 30), radius: 0.27 }],
    secondaries: [
      { direction: normalise(vec3(-1, 4, 2)), distance: 220, radius: 3.7, illuminance: [2, 3, 4] },
    ],
  };

  it("gives the record's lights, occluders and planetshine sources unchanged under the identity", () => {
    const { lighting } = terrainLightingOf(record, IDENTITY_ROTATION, EXPOSURE);
    expect([lighting.lights, lighting.occluders, lighting.secondaries]).toEqual([
      record.lights,
      record.occluders,
      record.secondaries,
    ]);
  });

  it("gives their directions rotated into the body-fixed axes, exactly, and the rest as they stand", () => {
    const { lighting } = terrainLightingOf(record, CYCLIC, EXPOSURE);
    expect([lighting.lights, lighting.occluders, lighting.secondaries]).toEqual([
      record.lights.map((light) => ({ ...light, direction: cyclic(light.direction) })),
      record.occluders.map((occluder) => ({ ...occluder, centre: cyclic(occluder.centre) })),
      record.secondaries.map((source) => ({ ...source, direction: cyclic(source.direction) })),
    ]);
  });

  it("takes the record's uniform law, or its class map's unsurveyed ground's, and the pre-exposure", () => {
    const uniform = terrainLightingOf(record, ROTATION, EXPOSURE).lighting;
    const mapped = terrainLightingOf(
      recordOf(bodyAt(3 * A, { surface: oneClassMap() })),
      ROTATION,
      EXPOSURE,
    ).lighting;
    expect([uniform.elsewhere, mapped.elsewhere, mapped.exposureScale]).toEqual([
      PROVISIONAL_PHOTOMETRY.law,
      ELSEWHERE,
      EXPOSURE,
    ]);
    expect(uniform.atmosphere).toBeUndefined();
  });

  it("takes the brightest light as the sun, and none without a light", () => {
    const lit = terrainLightingOf(record, ROTATION, EXPOSURE);
    const brightest = record.lights[0];
    const dark = terrainLightingOf({ ...record, lights: [] }, ROTATION, EXPOSURE);
    expect([lit.sunDirectionBodyFixed, lit.sunIlluminanceLx]).toEqual([
      brightest === undefined ? null : rotateToBodyFixed(ROTATION, brightest.direction),
      brightest === undefined ? null : [...brightest.illuminance],
    ]);
    expect([dark.sunDirectionBodyFixed, dark.sunIlluminanceLx]).toEqual([vec3(0, 0, 1), [0, 0, 0]]);
  });

  it("lights a one-class palette at the sub-solar point to the disc record's I/F, to 10⁻⁶ (T10.e's agreement, through the live path)", () => {
    const { plan, body } = handed({ surface: oneClassMap() });
    const mesh = plan.meshes[0];
    const disc = mesh === undefined ? undefined : plan.discs[mesh.index];
    const rotation = body.rotation;
    if (disc === undefined || rotation === undefined) {
      throw new Error("the plan holds the body's record");
    }
    const view = terrainViewOf(body, CAMERA, VIEWPORT);
    const { lighting } = terrainLightingOf(disc, rotation, EXPOSURE);
    const light = lighting.lights[0];
    const recordLight = disc.lights[0];
    if (view === null || light === undefined || recordLight === undefined) {
      throw new Error("the body is lit and turns");
    }
    // The terrain's side, body-fixed: the sub-solar point, its normal the sun's direction.
    const pointBf = scale(light.direction, A);
    const towardsBf = normalise(sub(rotateToBodyFixed(rotation, view.cameraM), pointBf));
    const mu = dot(light.direction, towardsBf);
    const photometry = new Float32Array(TERRAIN_CLASSES * PHOTOMETRY_STRIDE);
    photometry.set(REGOLITH);
    const shares = Array.from({ length: TERRAIN_LAWS }, (_, m) => (m === 1 ? 1 : 0));
    const terrain = terrainReflectance(
      terrainTexelLaws(photometry, lighting.elsewhere, 1),
      shares,
      1,
      mu,
      Math.acos(Math.min(1, mu)),
    );
    // The disc's side, along the galactic axes, from the same record.
    const pointG = add(body.centreM, scale(recordLight.direction, A));
    const muG = dot(recordLight.direction, normalise(scale(pointG, -1)));
    const reflected = discReflectance(
      discSurfaceLaws(disc.surface),
      disc.polarOverEquatorial,
      [0, 1],
      1,
      muG,
      Math.acos(Math.min(1, muG)),
    );
    const worst = Math.max(
      ...[0, 1, 2].map((c) => Math.abs((terrain[c] ?? 0) / (reflected[c] ?? 1) - 1)),
    );
    const radiance = (light.illuminance[1] * lighting.exposureScale) / Math.PI;
    expect(Math.min(...reflected)).toBeGreaterThan(0);
    expect(worst).toBeLessThan(1e-6);
    expect(radiance / (recordLight.illuminance[1] * disc.exposureOverPi) - 1).toBeCloseTo(0, 12);
  });
});

describe("a body's terrain view from the plan's body (R10.T16.c)", () => {
  it("is the camera's offset from its centre and its rotation, bit for bit", () => {
    const body = bodyAt(3 * A, { centreM: vec3(-0.1, 1 / 3, -5.2e6) });
    const view = terrainViewOf(body, CAMERA, VIEWPORT);
    const c = body.centreM;
    expect(view?.rotation).toBe(ROTATION);
    expect([
      Object.is(view?.cameraM.x, -c.x),
      Object.is(view?.cameraM.y, -c.y),
      Object.is(view?.cameraM.z, -c.z),
    ]).toEqual([true, true, true]);
    expect([view?.orientation, view?.fovXRad, view?.viewport]).toEqual([
      CAMERA.orientation,
      CAMERA.fovXRad,
      VIEWPORT,
    ]);
  });

  it("is none for a body whose rotation is not known", () => {
    expect(terrainViewOf(unturned(bodyAt(3 * A)), CAMERA, VIEWPORT)).toBeNull();
  });
});

/** A draw the session's fake gives. */
const DRAW: DrawItem = {
  mesh: { kind: "mesh", name: "terrain patches" },
  material: { kind: "material", name: "terrain-lit-baked-offsets" },
  offsetFromCameraM: new Float32Array(3),
  uniforms: { bodyRotation: new Float32Array(16) },
  textures: {},
};

const DEPTH: MaterialHandle = { kind: "material", name: "terrain-depth-baked-offsets" };
const MASK: TextureHandle = { kind: "texture", name: "coverage mask" };

/** A source holding one pass of `BODY`, recording what each view asks it to draw. */
function fakeSource(
  options: {
    readonly ready?: boolean;
    readonly depth?: MaterialHandle | null;
    readonly annunciation?: TerrainAnnunciation | null;
    readonly hasPass?: boolean;
  } = {},
): {
  readonly source: ViewTerrainSource;
  readonly drawn: Array<{ view: string; body: string; input: TerrainViewFrameInput }>;
  readonly askedReady: string[];
} {
  const drawn: Array<{ view: string; body: string; input: TerrainViewFrameInput }> = [];
  const askedReady: string[] = [];
  const pass: FrameTerrainPass = {
    maskReadyFor: (view) => {
      askedReady.push(view);
      return options.ready ?? true;
    },
    symbologyDepthMaterial: options.depth === undefined ? DEPTH : options.depth,
    heights: new ResidentHeights(),
    viewIds: new Set([VIEW]),
    faulted: false,
  };
  const frame: TerrainViewFrame = {
    draw: DRAW,
    conditions: { streaming: false, detailLimited: false },
    annunciation: options.annunciation ?? null,
    fault: null,
    missing: false,
    standingIn: false,
    maskReady: true,
  };
  return {
    drawn,
    askedReady,
    source: {
      wanted: new Set(),
      survey: () => null,
      coverageMask: (body) => (body === BODY ? MASK : null),
      wirePass: () => null,
      drawWireTerrain: () => null,
      pass: (body) => (body === BODY && options.hasPass !== false ? pass : null),
      drawTerrain: (view, body, input) => {
        drawn.push({ view, body, input });
        return body === BODY ? frame : null;
      },
    },
  };
}

/** A frame terrain of `VIEW` over `source`. */
function frameTerrain(source: ViewTerrainSource): LiveFrameTerrain {
  return new LiveFrameTerrain(source, {
    view: VIEW,
    camera: CAMERA,
    presentedPx: VIEWPORT,
    exposureScale: EXPOSURE,
    occluderSlopePx: 3,
    nowMs: 1_234,
  });
}

describe("a view's frame terrain (R10.T16.c)", () => {
  it("draws each body its plan hands over, from the plan's body and its record's lights", () => {
    const { plan, body } = handed();
    const { source, drawn } = fakeSource();
    const draws = frameTerrain(source).draws(plan, [body]);
    const mesh = plan.meshes[0];
    const record = mesh === undefined ? undefined : plan.discs[mesh.index];
    if (record === undefined) {
      throw new Error("the plan holds the body's record");
    }
    const { lighting, sunDirectionBodyFixed, sunIlluminanceLx } = terrainLightingOf(
      record,
      ROTATION,
      EXPOSURE,
    );
    expect(draws).toEqual([DRAW]);
    expect(drawn).toEqual([
      {
        view: VIEW,
        body: BODY,
        input: {
          view: terrainViewOf(body, CAMERA, VIEWPORT),
          lighting,
          sunDirectionBodyFixed,
          sunIlluminanceLx,
          exposureScale: EXPOSURE,
          nowMs: 1_234,
        },
      },
    ]);
  });

  it("draws no terrain of a body its plan draws as a disc", () => {
    const far = bodyAt(200 * A, { handoverReliefM: 1 });
    const { source, drawn } = fakeSource();
    frameTerrain(source).draws(planOf(far), [far]);
    expect(drawn).toEqual([]);
  });

  it("draws no terrain of a body whose rotation is not known", () => {
    const { plan, body } = handed();
    const { source, drawn } = fakeSource();
    frameTerrain(source).draws(plan, [unturned(body)]);
    expect(drawn).toEqual([]);
  });

  it("gives a body's mask only while its pass says the view's draw set leaves nothing missing", () => {
    const ready = fakeSource({ ready: true });
    const waiting = fakeSource({ ready: false });
    const none = fakeSource({ hasPass: false });
    expect([
      frameTerrain(ready.source).surveyCoverage(BODY),
      frameTerrain(waiting.source).surveyCoverage(BODY),
      frameTerrain(none.source).surveyCoverage(BODY),
      ready.askedReady,
    ]).toEqual([MASK, null, null, [VIEW]]);
  });

  it("keeps the annunciation of the bodies it drew, none before its draws", () => {
    const { plan, body } = handed();
    const terrain = frameTerrain(fakeSource({ annunciation: "TERRAIN: STREAMING" }).source);
    const before = terrain.annunciation;
    terrain.draws(plan, [body]);
    expect([before, terrain.annunciation]).toEqual([null, "TERRAIN: STREAMING"]);
  });

  it("takes STREAMING before DETAIL LIMITED over its bodies", () => {
    expect([
      morePressingAnnunciation(null, null),
      morePressingAnnunciation(null, "TERRAIN: DETAIL LIMITED"),
      morePressingAnnunciation("TERRAIN: DETAIL LIMITED", "TERRAIN: STREAMING"),
      morePressingAnnunciation("TERRAIN: STREAMING", null),
    ]).toEqual([null, "TERRAIN: DETAIL LIMITED", "TERRAIN: STREAMING", "TERRAIN: STREAMING"]);
  });

  it("gives the terrain it drew as depth alone, with the depth material and the overlay's slope term", () => {
    const { plan, body } = handed();
    const withDepth = frameTerrain(fakeSource().source);
    const withoutDepth = frameTerrain(fakeSource({ depth: null }).source);
    withDepth.draws(plan, [body]);
    withoutDepth.draws(plan, [body]);
    const [depth] = withDepth.symbologyDepth();
    expect(depth).toEqual({
      ...DRAW,
      material: DEPTH,
      uniforms: { ...DRAW.uniforms, occluderSlopePx: new Float32Array([3]) },
    });
    expect([withDepth.symbologyDepth().length, withoutDepth.symbologyDepth()]).toEqual([1, []]);
  });
});

/** The scene's moon turning, its rotation the identity. */
const MOON: BodyIdHex = FIXTURE_MOON;
const SCENE: ViewScene = {
  ...SESSION_SCENE,
  bodies: SESSION_SCENE.bodies.map((body) =>
    body.id === MOON ? { ...body, rotation: IDENTITY_ROTATION } : body,
  ),
};

/** A camera 300 km over the moon's north pole, looking straight down (−z). */
const POSE: CameraPose = {
  frame: { kind: "system", system: FIXTURE_SYSTEM },
  positionM: add(FIXTURE_MOON_CENTRE_M, vec3(0, 0, 1_736_000 + 300_000)),
  orientation: NO_TURN,
};
const DRAW_CAMERA = { pose: POSE, fovXRad: Math.PI / 2 };

/** Two cells of face +z at level 5, apart: 15 and 17 of row 15. */
const CELL_A: PatchKey = { face: 2, level: 5, i: 15, j: 15 };
const CELL_B: PatchKey = { face: 2, level: 5, i: 17, j: 15 };

function coverageOf(cells: ReadonlyArray<PatchKey>): CoverageView {
  const merged = CoverageView.empty(5).merged(
    {
      ranges: cells.map((cell) => {
        const start = fieldCellIndex(cell);
        return { start, end: start + 1, resolution: 40 };
      }),
    },
    1,
  );
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

/** A bake of `key` at `heightM` everywhere, its mask holding the edge (one vertex outside). */
function edgeBake(key: PatchKey, heightM: number): BakedPatch {
  const heights = new Float32Array(65 * 65 * 2);
  for (let n = 0; n < 65 * 65; n += 1) {
    heights[2 * n] = heightM;
  }
  const mask = new Uint8Array(65 * 65).fill(255);
  mask[0] = 0;
  return {
    key,
    generation: 1,
    originM: vec3(0, 0, 0),
    heights,
    offsets: null,
    normals: new Float16Array(65 * 65 * 2),
    heightRangeM: [heightM, heightM],
    boundingRadiusM: 1,
    originHeightM: heightM,
    skirtDepthM: 1,
    layers: {
      palette: new Uint8Array(16).fill(PALETTE_NONE),
      weights: new Uint8Array(),
      surveyMask: mask,
      fieldLevel: 5,
      boundM: 1,
      sigmaM: 0,
      complete: true,
      unsurveyedCells: new Uint32Array(),
      photometry: new Float32Array(40),
      horizonNear: new Uint32Array(),
      horizonFar: new Uint32Array(),
      horizonComplete: true,
      horizonRefusedCells: new Uint32Array(),
    },
  };
}

/** A session-like source wanting the moon, its survey `cells`, its pass's heights `heights`. */
function edgeSource(
  heights: ResidentHeights | null,
  overrides: Partial<ViewTerrainSource> = {},
): ViewTerrainSource {
  return {
    wanted: new Set([MOON]),
    survey: (body) =>
      body === MOON
        ? { coverage: coverageOf([CELL_A, CELL_B]), header: moonLikeHeader(MOON) }
        : null,
    coverageMask: () => null,
    wirePass: () => null,
    drawWireTerrain: () => null,
    drawTerrain: () => null,
    pass: (body) =>
      body === MOON && heights !== null
        ? {
            maskReadyFor: () => true,
            symbologyDepthMaterial: null,
            heights,
            viewIds: new Set([VIEW]),
            faulted: false,
          }
        : null,
    ...overrides,
  };
}

/** Whether every height of `heights` lies within a metre of `heightM`, and there are some. */
function allNear(heights: ReadonlyArray<number>, heightM: number): boolean {
  return heights.length > 8 && heights.every((h) => Math.abs(h - heightM) < 1);
}

describe("the survey edges a view marks (R10.T16.c)", () => {
  it("marks each wanted body's coverage in view, on its header's figure, about its rotation and centre", () => {
    const [edge, ...rest] = surveyEdgeViewsOf(
      edgeSource(new ResidentHeights()),
      VIEW,
      SCENE,
      DRAW_CAMERA,
      VIEWPORT,
      "wireframe",
    );
    expect(rest).toEqual([]);
    expect([edge?.body, edge?.figure, edge?.rotation]).toEqual([
      MOON,
      { equatorialRadiusM: 1_738_100, polarRadiusM: 1_736_000 },
      IDENTITY_ROTATION,
    ]);
    // The moon's centre from the camera, as the draw list places it.
    expect(edge?.centreM.x).toBeCloseTo(0, 6);
    expect(edge?.centreM.y).toBeCloseTo(0, 6);
    expect(edge?.centreM.z).toBeCloseTo(-(1_736_000 + 300_000), 6);
  });

  describe("marks no body", () => {
    const heights = new ResidentHeights();
    const behind = {
      ...DRAW_CAMERA,
      pose: { ...POSE, orientation: lookAlong(vec3(0, 0, 1), vec3(1, 0, 0)) },
    };
    it.each<[string, () => ViewTerrainSource, ViewScene, typeof DRAW_CAMERA]>([
      ["not wanted", () => edgeSource(heights, { wanted: new Set() }), SCENE, DRAW_CAMERA],
      [
        "without a surveyed coverage",
        () =>
          edgeSource(heights, {
            survey: () => ({ coverage: CoverageView.empty(5), header: moonLikeHeader(MOON) }),
          }),
        SCENE,
        DRAW_CAMERA,
      ],
      [
        "without a header",
        () =>
          edgeSource(heights, { survey: () => ({ coverage: coverageOf([CELL_A]), header: null }) }),
        SCENE,
        DRAW_CAMERA,
      ],
      ["without a rotation", () => edgeSource(heights), SESSION_SCENE, DRAW_CAMERA],
      ["out of view", () => edgeSource(heights), SCENE, behind],
    ])("%s", (_case, source, scene, camera) => {
      expect(surveyEdgeViewsOf(source(), VIEW, scene, camera, VIEWPORT, "photorealistic")).toEqual(
        [],
      );
    });
  });

  it("hides points behind the drawn ground only in a photorealistic view its body's lit pass draws, with no wireframe pass", () => {
    const heights = new ResidentHeights();
    const ground = (style: "photorealistic" | "wireframe", view = VIEW): boolean =>
      surveyEdgeViewsOf(edgeSource(heights), view, SCENE, DRAW_CAMERA, VIEWPORT, style)[0]
        ?.ground !== undefined;
    expect([
      ground("photorealistic"),
      ground("wireframe"),
      ground("photorealistic", INSTRUMENT),
    ]).toEqual([true, false, false]);
  });

  /** A source whose lit pass's resident bakes stand the edge on cell A at 500 m. */
  function litAt500(): ViewTerrainSource {
    const heights = new ResidentHeights();
    heights.take(edgeBake(CELL_A, 500));
    return edgeSource(heights);
  }

  /**
   * The heights over the datum of the moon's edge's line points on cells A and B, in `view`, over
   * `source`.
   */
  function rimHeights(
    style: "photorealistic" | "wireframe",
    view = VIEW,
    source: ViewTerrainSource = litAt500(),
  ): { readonly onA: number[]; readonly onB: number[] } {
    const [edge] = surveyEdgeViewsOf(source, view, SCENE, DRAW_CAMERA, VIEWPORT, style);
    if (edge === undefined) {
      throw new Error("the moon's edge is marked");
    }
    const { line } = surveyEdgeMark(
      edge,
      { orientation: POSE.orientation, fovXRad: DRAW_CAMERA.fovXRad },
      VIEWPORT,
      { remPx: 16, keepClear: [], rangeFromM: null },
    );
    const a = edge.figure.equatorialRadiusM;
    const c = edge.figure.polarRadiusM;
    // Each line point's height over the datum, by the cell whose sides it lies on: A's columns
    // span s from 15 to 16 thirty-seconds of face +z, B's from 17 to 18.
    const onA: number[] = [];
    const onB: number[] = [];
    for (let n = 0; n + 2 < line.length; n += 3) {
      const q = sub(vec3(line[n] ?? 0, line[n + 1] ?? 0, line[n + 2] ?? 0), edge.centreM);
      const d = normalise(vec3(q.x / a, q.y / a, q.z / c));
      const datum = vec3(a * d.x, a * d.y, c * d.z);
      const height = dot(sub(q, datum), normalise(vec3(d.x / a, d.y / a, d.z / c)));
      // Face +z's u is −x ÷ z (`xyzToFaceUv`), its s = uvToSt(u); 32 cells a side.
      const column = 32 * uvToSt(-d.x / d.z);
      (column <= 16.5 ? onA : onB).push(height);
    }
    return { onA, onB };
  }

  it("stands the rim at a resident bake's height where one holds the edge, and on the datum elsewhere, where the view draws the terrain", () => {
    const { onA, onB } = rimHeights("photorealistic");
    expect([
      onA.length > 8 && onA.every((h) => Math.abs(h - 500) < 1),
      onB.length > 8 && onB.every((h) => Math.abs(h) < 1),
    ]).toEqual([true, true]);
  });

  it.each([
    ["the wireframe, with no wireframe pass", "wireframe", VIEW],
    ["a view its pass does not draw", "photorealistic", INSTRUMENT],
  ] as const)(
    "stands the rim on the datum, the figure drawn, where the view draws no terrain of the body: %s",
    (_name, style, view) => {
      const { onA, onB } = rimHeights(style, view);
      expect([...onA, ...onB].every((h) => Math.abs(h) < 1)).toBe(true);
    },
  );

  it("stands the rim on the datum where the body's pass is faulted, which draws the figure (R10.T16.e)", () => {
    const lit = litAt500();
    const { onA, onB } = rimHeights("photorealistic", VIEW, {
      ...lit,
      pass: (body) => {
        const pass = lit.pass(body);
        return pass === null ? null : { ...pass, faulted: true };
      },
    });
    expect([onA.length > 8, allNear([...onA, ...onB], 0)]).toEqual([true, true]);
  });

  /**
   * A source whose lit pass's bakes stand the edge on cell A at 900 m, taking `VIEW`, and whose
   * wireframe pass's at 500 m, taking `wireViews` (R10.T11).
   */
  function bothPasses(wireViews: ReadonlySet<string>): ViewTerrainSource {
    const lit = new ResidentHeights();
    lit.take(edgeBake(CELL_A, 900));
    const wire = new ResidentHeights();
    wire.take(edgeBake(CELL_A, 500));
    return edgeSource(lit, {
      wirePass: (body) =>
        body === MOON
          ? {
              maskReadyFor: () => true,
              symbologyDepthMaterial: null,
              heights: wire,
              viewIds: wireViews,
              faulted: false,
            }
          : null,
    });
  }

  it("stands the rim on the wireframe pass's bakes in a wireframe view it takes, not the lit pass's (R10.T11)", () => {
    const { onA, onB } = rimHeights("wireframe", VIEW, bothPasses(new Set([VIEW])));
    expect([allNear(onA, 500), allNear(onB, 0)]).toEqual([true, true]);
  });

  it("stands the rim on the lit pass's bakes in the photorealistic style, not the wireframe pass's", () => {
    const { onA } = rimHeights("photorealistic", VIEW, bothPasses(new Set([VIEW])));
    expect(allNear(onA, 900)).toBe(true);
  });

  it("stands the rim on the datum in a wireframe view its wireframe pass did not take, the lit pass's taking it", () => {
    const { onA, onB } = rimHeights("wireframe", VIEW, bothPasses(new Set([INSTRUMENT])));
    expect(allNear([...onA, ...onB], 0)).toBe(true);
  });

  it("hides points behind the drawn ground in a wireframe view only where its wireframe pass takes it", () => {
    const ground = (wireViews: ReadonlySet<string>): boolean =>
      surveyEdgeViewsOf(bothPasses(wireViews), VIEW, SCENE, DRAW_CAMERA, VIEWPORT, "wireframe")[0]
        ?.ground !== undefined;
    expect([ground(new Set([VIEW])), ground(new Set([INSTRUMENT]))]).toEqual([true, false]);
  });
});

/** Stand-in colours of the wireframe's terrain, one distinct colour per token. */
const WIRE_TOKENS = { textMuted: "#909090", text: "#e0e0e0", surface0: "#101010" } as const;

/** A draw the wireframe's fake pass gives. */
const WIRE_DRAW: DrawItem = {
  mesh: { kind: "mesh", name: "terrain patches" },
  material: { kind: "material", name: "terrain-wire-baked-offsets" },
  offsetFromCameraM: new Float32Array(3),
  uniforms: {},
  textures: {},
};

/** The moon's datum's polar radius, m (`moonLikeHeader`'s). */
const MOON_POLAR_M = 1_736_000;

/** A camera `distanceM` from the moon's centre over its north pole, looking down, across 90°. */
function cameraAt(distanceM: number): DrawCamera {
  return {
    pose: { ...POSE, positionM: add(FIXTURE_MOON_CENTRE_M, vec3(0, 0, distanceM)) },
    fovXRad: Math.PI / 2,
  };
}

/** A camera `heightM` above the moon's datum at its north pole. */
function abovePole(heightM: number): DrawCamera {
  return cameraAt(MOON_POLAR_M + heightM);
}

/**
 * A camera from which the moon's sphere of its scene radius spans `diameterPx` across
 * {@link VIEWPORT} at 90°: 2 asin(r ÷ d) at 960 px a radian.
 */
function seeingMoonAt(diameterPx: number): DrawCamera {
  return cameraAt(1_738_100 / Math.sin(diameterPx / (2 * 960)));
}

/** A body as the wireframe's fake source holds it. */
interface WireBody {
  /** The views its wireframe pass took: `VIEW` by default. */
  readonly viewIds?: ReadonlySet<string>;
  /** Its pass's resident heights: none by default. */
  readonly heights?: ResidentHeights;
  /** Its survey's header: `moonLikeHeader`'s of the body by default. */
  readonly header?: SurfaceHeaderDto | null;
  /** What its draw gives beyond a drawn frame, its mask ready. */
  readonly frame?: Partial<TerrainViewFrame>;
  /** Its coverage mask: {@link MASK} by default. */
  readonly mask?: TextureHandle | null;
}

/** A wireframe source holding a wireframe pass of each body of `bodies`, recording what it draws. */
function wireSource(bodies: ReadonlyMap<BodyIdHex, WireBody>): {
  readonly source: WireTerrainSource;
  readonly drawn: Array<{ view: string; body: string; input: TerrainViewFrameInput }>;
} {
  const drawn: Array<{ view: string; body: string; input: TerrainViewFrameInput }> = [];
  return {
    drawn,
    source: {
      wirePass: (body) => {
        const each = bodies.get(body);
        return each === undefined
          ? null
          : {
              maskReadyFor: () => true,
              symbologyDepthMaterial: null,
              heights: each.heights ?? new ResidentHeights(),
              viewIds: each.viewIds ?? new Set([VIEW]),
              faulted: false,
            };
      },
      drawWireTerrain: (view, body, input) => {
        drawn.push({ view, body, input });
        const each = bodies.get(body);
        return each === undefined
          ? null
          : {
              draw: WIRE_DRAW,
              conditions: { streaming: false, detailLimited: false },
              annunciation: null,
              fault: null,
              missing: false,
              standingIn: false,
              maskReady: true,
              ...each.frame,
            };
      },
      coverageMask: (body) => {
        const mask = bodies.get(body)?.mask;
        return mask === undefined ? MASK : mask;
      },
      survey: (body) => {
        const each = bodies.get(body);
        if (each === undefined) {
          return null;
        }
        return {
          coverage: null,
          header: each.header === undefined ? moonLikeHeader(body) : each.header,
        };
      },
    },
  };
}

/** The moon alone, as `wireSource` holds it with `moon`. */
function theMoon(moon: WireBody = {}): ReadonlyMap<BodyIdHex, WireBody> {
  return new Map([[MOON, moon]]);
}

/** A wireframe frame's input over {@link SCENE} from 300 km over the moon's pole. */
function wireInput(overrides: Partial<WireframeTerrainInput> = {}): WireframeTerrainInput {
  return {
    view: VIEW,
    scene: SCENE,
    camera: abovePole(300_000),
    viewport: VIEWPORT,
    strokeScale: 1,
    lowSetting: false,
    tokens: WIRE_TOKENS,
    occluderSlopePx: 3,
    nowMs: 1_234,
    previousIntervalM: null,
    ...overrides,
  };
}

/** The scene with its planet and its moon both turning, the planet first. */
const BOTH_TURNING: ViewScene = {
  ...SESSION_SCENE,
  bodies: SESSION_SCENE.bodies.map((body) => ({ ...body, rotation: IDENTITY_ROTATION })),
};

/** A fully surveyed bake of `key` at `heightM` everywhere. */
function groundBake(key: PatchKey, heightM: number): BakedPatch {
  const bake = edgeBake(key, heightM);
  const layers = bake.layers;
  if (layers === null) {
    throw new Error("the edge bake has layers");
  }
  return { ...bake, layers: { ...layers, surveyMask: new Uint8Array(65 * 65).fill(255) } };
}

describe("a wireframe view's terrain (R10.T11; Design note 14)", () => {
  it("draws each body whose wireframe pass took the view, giving its draw", () => {
    const { source, drawn } = wireSource(theMoon());
    const frame = wireframeTerrainOf(source, wireInput());
    expect([frame.draws, drawn.map(({ view, body }) => [view, body])]).toEqual([
      [WIRE_DRAW],
      [[VIEW, MOON]],
    ]);
  });

  describe("draws no terrain of a body", () => {
    it.each<[string, ReadonlyMap<BodyIdHex, WireBody>, ViewScene]>([
      [
        "whose wireframe pass did not take the view",
        theMoon({ viewIds: new Set([INSTRUMENT]) }),
        SCENE,
      ],
      ["with no wireframe pass", new Map(), SCENE],
      ["whose rotation is not known", theMoon(), SESSION_SCENE],
      ["with no header held", theMoon({ header: null }), SCENE],
    ])("%s", (_case, bodies, scene) => {
      const { source, drawn } = wireSource(bodies);
      const frame = wireframeTerrainOf(source, wireInput({ scene }));
      expect([frame.draws, drawn]).toEqual([[], []]);
    });
  });

  it("draws from the view's camera, in the body's rotation, at the frame's time and slope term", () => {
    const { source, drawn } = wireSource(theMoon());
    wireframeTerrainOf(source, wireInput());
    const input = drawn[0]?.input;
    expect([
      input?.view.rotation,
      input?.view.orientation,
      input?.view.fovXRad,
      input?.view.viewport,
      input?.nowMs,
      input?.wire?.occluderSlopePx,
    ]).toEqual([IDENTITY_ROTATION, POSE.orientation, Math.PI / 2, VIEWPORT, 1_234, 3]);
    // The camera from the moon's centre: 300 km over the datum at its pole.
    expect(input?.view.cameraM.z).toBeCloseTo(MOON_POLAR_M + 300_000, 6);
  });

  it("states one interval for the view, from the body it stands lowest over, and draws every body at it", () => {
    const { source, drawn } = wireSource(
      new Map([
        [FIXTURE_PLANET, {}],
        [MOON, {}],
      ]),
    );
    const frame = wireframeTerrainOf(source, wireInput({ scene: BOTH_TURNING }));
    // 300 km over the moon's datum, 384,000 km from the planet's: a twentieth of 300 km is 15 km.
    const intervalM = contourInterval(300_000, null);
    expect([intervalM, frame.intervalM, drawn.map(({ input }) => input.wire?.intervalM)]).toEqual([
      20_000,
      20_000,
      [20_000, 20_000],
    ]);
  });

  it("measures the height at the nadir over the wireframe pass's resident ground", () => {
    // 4 km over the datum: 200 m contours; over ground drawn at 2 km, 2 km over it: 100 m.
    const heights = new ResidentHeights();
    heights.take(groundBake({ face: 2, level: 0, i: 0, j: 0 }, 2_000));
    const over = (moon: WireBody): number | null =>
      wireframeTerrainOf(wireSource(theMoon(moon)).source, wireInput({ camera: abovePole(4_000) }))
        .intervalM;
    expect([over({}), over({ heights })]).toEqual([200, 100]);
  });

  it("holds the last interval while the height lies between its thresholds", () => {
    // 7 km: a twentieth is 350 m, past the midpoint 316 m to 500 m but short of its 1.2 × 316 m.
    const at = (previousIntervalM: number | null): number | null =>
      wireframeTerrainOf(
        wireSource(theMoon()).source,
        wireInput({ camera: abovePole(7_000), previousIntervalM }),
      ).intervalM;
    expect([at(null), at(200)]).toEqual([500, 200]);
  });

  it("states no interval while it draws nothing, holding the one its pose sets", () => {
    const { source } = wireSource(theMoon({ frame: { draw: null } }));
    const frame = wireframeTerrainOf(source, wireInput({ previousIntervalM: 50_000 }));
    expect([frame.draws, frame.intervalM, frame.heldIntervalM]).toEqual([[], null, 20_000]);
  });

  it("holds the last interval with no body to draw", () => {
    const { source } = wireSource(theMoon({ viewIds: new Set([INSTRUMENT]) }));
    const frame = wireframeTerrainOf(source, wireInput({ previousIntervalM: 500 }));
    expect([frame.intervalM, frame.heldIntervalM, frame.annunciation]).toEqual([null, 500, null]);
  });

  describe("states one interval for a pose through the stage's holder (R10.T12.b)", () => {
    // 7 km over the pole: 500 m from no interval, 200 m held from 200 m.
    const HIGH = abovePole(7_000);
    /** The moon's wireframe pass, which took both views. */
    const BOTH = theMoon({ viewIds: new Set([VIEW, INSTRUMENT]) });

    it("states the interval a first view stated at the same point in the same frame", () => {
      const intervals = new PoseIntervals();
      const { source } = wireSource(BOTH);
      wireframeTerrainOf(source, wireInput({ camera: HIGH, previousIntervalM: 200, intervals }));
      const second = wireframeTerrainOf(
        source,
        wireInput({ view: INSTRUMENT, camera: HIGH, intervals }),
      );
      expect(second.intervalM).toBe(200);
    });

    it("states a view's own interval where no view stated one at its point", () => {
      const { source } = wireSource(BOTH);
      const alone = wireframeTerrainOf(
        source,
        wireInput({ view: INSTRUMENT, camera: HIGH, intervals: new PoseIntervals() }),
      );
      expect(alone.intervalM).toBe(500);
    });

    it("forgets a view in a frame in which it draws no body, so that a view at its point keeps its own", () => {
      const intervals = new PoseIntervals();
      wireframeTerrainOf(
        wireSource(BOTH).source,
        wireInput({ camera: HIGH, previousIntervalM: 200, intervals }),
      );
      // The next frame's pass no longer takes the primary: it has no nadir.
      wireframeTerrainOf(
        wireSource(theMoon({ viewIds: new Set([INSTRUMENT]) })).source,
        wireInput({ camera: HIGH, previousIntervalM: 200, intervals, nowMs: 1_250 }),
      );
      const second = wireframeTerrainOf(
        wireSource(BOTH).source,
        wireInput({ view: INSTRUMENT, camera: HIGH, intervals, nowMs: 1_250 }),
      );
      expect(second.intervalM).toBe(500);
    });
  });

  describe("draws the body's graticule as R02's draw list does", () => {
    /** The graticule each draw of the moon is given, from `camera` at `strokeScale`. */
    function graticuleOf(
      camera: DrawCamera,
      overrides: Partial<WireframeTerrainInput> = {},
    ): NonNullable<TerrainViewFrameInput["wire"]>["graticule"] | undefined {
      const { source, drawn } = wireSource(theMoon());
      wireframeTerrainOf(source, wireInput({ camera, ...overrides }));
      return drawn[0]?.input.wire?.graticule;
    }

    it("at 15° on a body 128 px or more across at a scale of 2, its widths at that scale", () => {
      expect(graticuleOf(seeingMoonAt(200), { strokeScale: 2 })).toEqual({
        stepRad: (15 * Math.PI) / 180,
        thinPx: 2,
        heavyPx: 3,
        casingPx: 2,
      });
    });

    it.each([
      ["under 64 px across", seeingMoonAt(30), false],
      ["at the low setting", seeingMoonAt(200), true],
    ])("at 30° on a body %s", (_case, camera, lowSetting) => {
      expect(graticuleOf(camera, { lowSetting })?.stepRad).toBeCloseTo((30 * Math.PI) / 180, 15);
    });

    it("not at all on a body under 8 px across, drawn as its limb", () => {
      expect(graticuleOf(seeingMoonAt(5))).toBeNull();
    });
  });

  it("draws the coastline at the header's sea level on a world with a sea, and none without", () => {
    const seaLevel = (header: SurfaceHeaderDto): number | null | undefined => {
      const { source, drawn } = wireSource(theMoon({ header }));
      wireframeTerrainOf(source, wireInput());
      return drawn[0]?.input.wire?.seaLevelM;
    };
    expect([
      seaLevel({ ...moonLikeHeader(MOON), main_liquid: 0 }),
      seaLevel(moonLikeHeader(MOON)),
    ]).toEqual([-2_000, null]);
  });

  it("colours its lines with the tokens linearised", () => {
    const { source, drawn } = wireSource(theMoon());
    wireframeTerrainOf(source, wireInput());
    expect(drawn[0]?.input.wire?.colours).toEqual({
      muted: linearColour(WIRE_TOKENS.textMuted),
      text: linearColour(WIRE_TOKENS.text),
      casing: linearColour(WIRE_TOKENS.surface0),
    });
  });

  it("gives a body's mask, axes and stretch only where its draw may take the mask", () => {
    const surveyed = (moon: WireBody): ReturnType<typeof wireframeTerrainOf>["surveyed"] =>
      wireframeTerrainOf(wireSource(theMoon(moon)).source, wireInput()).surveyed;
    expect([
      surveyed({}),
      surveyed({ frame: { maskReady: false } }),
      surveyed({ mask: null }),
    ]).toEqual([
      new Map([
        [
          MOON,
          { mask: MASK, axes: rotationColumns(IDENTITY_ROTATION), stretch: 1_738_100 / 1_736_000 },
        ],
      ]),
      new Map(),
      new Map(),
    ]);
  });

  it("annunciates the more pressing of its bodies' lines", () => {
    const { source } = wireSource(
      new Map([
        [FIXTURE_PLANET, { frame: { annunciation: "TERRAIN: DETAIL LIMITED" } }],
        [MOON, { frame: { annunciation: "TERRAIN: STREAMING" } }],
      ]),
    );
    expect(wireframeTerrainOf(source, wireInput({ scene: BOTH_TURNING })).annunciation).toBe(
      "TERRAIN: STREAMING",
    );
  });
});
