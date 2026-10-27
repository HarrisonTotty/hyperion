/**
 * The smoke page's checks of the `disc cells` pass for every record (plan R07, R07.T8.e;
 * decision-r08-t11-frame §2 and §7): G11 on the GPU, each plan drawn through the pass and
 * in-fragment in the same run, every channel within one `rgba16float` ulp and every alpha (the
 * interior's meter class) equal. The plans hold a 13 px body with air (R05's Earth's tables) as a
 * disc and as the mesh the view draws it as, and a 13 px disc under a half-surveyed two-class map,
 * which the pass now binds. The class map's disc is held to the CPU rasteriser too (G10).
 *
 * @remarks
 * The pass is exact, so an in-fragment draw where the pass was meant equals it too: a third draw of
 * each plan proves its pixels read the pass. Its sums are overwritten after the dispatch with a
 * sentinel far above any light drawn, and every pixel the draws must take from the pass shows it:
 * each pixel of a disc under 32 px that its draws' corner tests (`bodyDisc.wgsl`'s `disc_pixel`)
 * take in `f64` with a hundredth of a pixel to spare, so that `f32`'s rounding at the tests' edges
 * cannot decide it.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { add, cross, dot, norm, normalise, scale, type Vec3, vec3 } from "../geometry/vec3";
import type { ClassMapDiscSurface } from "../view/appearance/bodyAppearance";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { lawFor } from "../view/appearance/phase";
import { R05_EARTH_REFERENCE } from "../view/atmosphere/earth";
import {
  type SurfaceLightingTables,
  surfaceLightingTablesOf,
} from "../view/atmosphere/surfaceLighting";
import { AtmosphereTables } from "../view/atmosphere/tables";
import {
  compositeDiscPixels,
  type DiscRecord,
  LIMB_OVERLAP_PX,
  rasteriseDisc,
  viewRay,
} from "../view/bodies/discShading";
import { type ClassMapTexels, classMapSurface, packClassMap } from "../view/bodies/discSurface";
import {
  BODY_DISC_CELLS_KERNEL,
  type BodyFramePlan,
  type LitBodyEngine,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
} from "../view/bodies/draw";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  toViewAxes,
  type Viewport,
  viewRotation4,
} from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import { rotation3FromRows } from "../view/coords/rotation";
import type { BufferHandle, ComputeHandle, RenderEngine } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { createSceneTarget } from "../view/photoreal/sceneTarget";
import { METER_CLASS } from "../view/post/meter";
import { AU_M } from "../view/scenes/kept";
import type { BodyFigure } from "../view/terrain/planet";
import { WIREFRAME_MATERIALS } from "../view/wireframe/submit";
import { type Checks, halfTexels, halfUlpsApart } from "./harness";

const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const EXPOSURE = 1e-4;
const RAD_PER_DEG = Math.PI / 180;

/** R05's spherical Earth's mean radius, m, on which its medium's tables are built. */
const EARTH_MEAN_RADIUS_M = 6_371_008.8;

const EARTH: BodyFigure = {
  equatorialRadiusM: EARTH_MEAN_RADIUS_M,
  polarRadiusM: EARTH_MEAN_RADIUS_M,
  pole: null,
};

/** The agreement of a texel with the rasteriser: `rgba16float`'s rounding and `f32`'s shading. */
const TEXEL_RELATIVE = 4e-3;
const TEXEL_ABSOLUTE = 1e-4;

/**
 * The sentinel's light in every channel, pre-exposed: about 300 times the brightest the bodies
 * draw at this exposure (a few units), and exact in `rgba16float`.
 */
const SENTINEL = 1024;

/** The sentinel's coverage: the limb draws a pixel only where it is above 0. */
const SENTINEL_COVERAGE = 0.5;

/** How far inside the corner tests' edges a pixel must lie to be held to the sentinel, px. */
const SENTINEL_SPARE_PX = 0.01;

/** The limb draw's bound outside the limb (`bodyDisc.wgsl`'s `OUTSIDE_PX`), px. */
const OUTSIDE_PX = 0.75;

/** One body of a case: its size and place, and what it carries. */
interface CellBody {
  readonly id: BodyIdHex;
  readonly diameterPx: number;
  /** Its centre's offset from the view's centre, px, right and down. */
  readonly offsetPx: readonly [number, number];
  /** Its air's tables, which its draws and the pass bind; absent for an airless body. */
  readonly atmosphere?: SurfaceLightingTables;
  /** Its class map and the map's texels, for the twin; absent for a uniform body. */
  readonly mapped?: { readonly surface: ClassMapDiscSurface; readonly texels: ClassMapTexels };
  /** Whether it is drawn in the mesh regime, as a body whose air is drawn is (R08.T10.b). */
  readonly mesh: boolean;
  /** The cells per axis it should take inside and on the limb. */
  readonly samples: readonly [number, number];
}

/** One case of the check: a plan of bodies in a view. */
interface CellCase {
  readonly name: string;
  readonly viewport: Viewport;
  readonly bodies: ReadonlyArray<CellBody>;
  /** Whether the pass's texels are held to the CPU rasteriser (G10): one uniform or mapped body. */
  readonly twin: boolean;
}

/** A case's plan: its bodies lit by a Sun 1 au away, 80° off the line of sight, a gibbous phase. */
function planOf(item: CellCase): BodyFramePlan {
  const { viewport } = item;
  const pxPerRad = viewport.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
  const towards = vec3(Math.sin(80 * RAD_PER_DEG), 0, Math.cos(80 * RAD_PER_DEG));
  const bodies = item.bodies.map((spec): LitBodyInput => {
    const distanceM = EARTH.equatorialRadiusM / Math.sin(spec.diameterPx / 2 / pxPerRad);
    const [rightPx, downPx] = spec.offsetPx;
    return {
      id: spec.id,
      centreM: vec3((rightPx * distanceM) / pxPerRad, (-downPx * distanceM) / pxPerRad, -distanceM),
      figure: EARTH,
      photometry: PROVISIONAL_PHOTOMETRY,
      lighting: undefined,
      ...(spec.atmosphere === undefined ? {} : { atmosphere: spec.atmosphere }),
      ...(spec.mapped === undefined
        ? {}
        : {
            surface: spec.mapped.surface,
            rotation: rotation3FromRows([vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)]),
          }),
    };
  });
  // The Sun as seen from the first body, the bodies being near one another beside 1 au.
  const first = bodies[0]?.centreM ?? vec3(0, 0, 0);
  // A body whose air is drawn is drawn with depth, as a mesh (R08.T10.b).
  const meshes = item.bodies.filter((spec) => spec.mesh);
  return planLitBodies(
    bodies,
    [{ disc: sunLikeHostDisc(), centreM: add(first, scale(towards, AU_M)) }],
    {
      camera: CAMERA,
      viewport,
      presentedPx: viewport,
      exposureScale: EXPOSURE,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high",
      ...(meshes.length === 0
        ? {}
        : {
            depthBodyShellRadiiM: new Map(
              meshes.map((spec) => [spec.id, EARTH.equatorialRadiusM] as const),
            ),
          }),
    },
    new Map(bodies.map((body) => [body.id, "disc"] as const)),
  );
}

/**
 * The renderer's engine, keeping the sums buffer each `disc cells` dispatch binds, so that the
 * sentinel draw can overwrite the sums the draws then read; every call is the engine's own.
 */
function keepingSums(engine: RenderEngine): {
  readonly engine: LitBodyEngine;
  readonly sums: () => BufferHandle | null;
} {
  let sums: BufferHandle | null = null;
  const kept: LitBodyEngine = {
    createMaterial: (...args) => engine.createMaterial(...args),
    createMesh: (...args) => engine.createMesh(...args),
    createBuffer: (...args) => engine.createBuffer(...args),
    createTexture: (...args) => engine.createTexture(...args),
    writeBuffer: (...args) => {
      engine.writeBuffer(...args);
    },
    writeTexture: (...args) => {
      engine.writeTexture(...args);
    },
    releaseBuffer: (...args) => {
      engine.releaseBuffer(...args);
    },
    releaseTexture: (...args) => {
      engine.releaseTexture(...args);
    },
    dispatch: (kernel, bindings, workgroups, pass) => {
      sums = bindings.buffers["cell_sums"] ?? sums;
      engine.dispatch(kernel, bindings, workgroups, pass);
    },
    onRestored: (...args) => engine.onRestored(...args),
    get capabilities() {
      return engine.capabilities;
    },
    prepareMaterial: (...args) => engine.prepareMaterial(...args),
  };
  return { engine: kept, sums: () => sums };
}

/** The sentinel's sums for a buffer of `bytes`: every pixel's light, coverage and points. */
function sentinelSums(bytes: number): Float32Array {
  const sums = new Float32Array(bytes / 4);
  for (let i = 0; i + 8 <= sums.length; i += 8) {
    sums.set([SENTINEL, SENTINEL, SENTINEL, SENTINEL_COVERAGE, 0, 1, 0, 0], i);
  }
  return sums;
}

/** How a plan is drawn: in-fragment, through the pass, or through the pass under the sentinel. */
type DrawnBy = "in-fragment" | "pass" | "sentinel";

/**
 * A plan drawn into a scene target as the photorealistic frame draws it (its mesh figures, then its
 * discs and limbs), read back as its `rgba16float` bytes, and whether the pass dispatched.
 */
async function drawPlan(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  sums: () => BufferHandle | null,
  plan: BodyFramePlan,
  viewport: Viewport,
  cells: ComputeHandle,
  by: DrawnBy,
): Promise<{ readonly bytes: ArrayBuffer; readonly throughCells: boolean }> {
  const target = createSceneTarget(engine, "smoke cell pass", viewport);
  try {
    const viewRotation = viewRotation4(CAMERA.orientation);
    const projection = perspectiveReversedInfinite(
      CAMERA.fovXRad,
      viewport.widthPx / viewport.heightPx,
      NEAR_PLANE_M,
    );
    const throughCells =
      by !== "in-fragment" &&
      renderer.dispatchCells(plan, cells, { viewRotation, projection, size: viewport });
    const buffer = sums();
    if (by === "sentinel" && throughCells && buffer !== null) {
      // After the dispatches on the queue, before the draws that read the sums.
      engine.writeBuffer(buffer, 0, sentinelSums(buffer.bytes));
    }
    target.render({
      label: PHOTOREAL_PASS_LABELS.bodies,
      viewRotation,
      projection,
      draws: renderer.meshDraws(plan),
      postProcesses: [],
    });
    target.render({
      label: PHOTOREAL_PASS_LABELS.discs,
      viewRotation,
      projection,
      draws: renderer.draws(plan),
      postProcesses: [],
      colourLoad: "load",
    });
    return { bytes: await engine.readTexture(target.colour), throughCells };
  } finally {
    target.dispose();
  }
}

/**
 * The pixels a record's draws must take from the pass (the module's remarks): each that its
 * interior draw takes, where every pixel reads the pass, and each that its limb draw takes, by the
 * corner tests in `f64` with {@link SENTINEL_SPARE_PX} to spare.
 */
function passPixels(record: DiscRecord, viewport: Viewport): Array<readonly [number, number]> {
  const centre = toViewAxes(record.direction, CAMERA.orientation);
  const pole = normalise(toViewAxes(record.pole, CAMERA.orientation));
  const stretch = 1 / record.polarOverEquatorial;
  const stretched = (x: Vec3): Vec3 => add(x, scale(pole, (stretch - 1) * dot(x, pole)));
  const scaledCentre = stretched(centre);
  const limbRad = Math.asin(record.radiusOverDistance / norm(scaledCentre));
  const angleAt = (x: number, y: number): number => {
    const ray = stretched(viewRay(x, y, CAMERA, viewport));
    return Math.atan2(norm(cross(ray, scaledCentre)), dot(ray, scaledCentre)) - limbRad;
  };
  const whole = record.interiorSamples === record.limbSamples;
  const pixels: Array<readonly [number, number]> = [];
  const { rect } = record;
  for (let y = Math.floor(rect.topPx); y < Math.ceil(rect.bottomPx); y += 1) {
    for (let x = Math.floor(rect.leftPx); x < Math.ceil(rect.rightPx); x += 1) {
      const corners = [angleAt(x, y), angleAt(x + 1, y), angleAt(x, y + 1), angleAt(x + 1, y + 1)];
      const [c00 = 0, c10 = 0, c01 = 0, c11 = 0] = corners;
      const gradient = Math.hypot((c10 + c11 - c00 - c01) / 2, (c01 + c11 - c00 - c10) / 2);
      if (!(gradient > 0)) {
        continue;
      }
      const depths = corners.map((angle) => angle / gradient);
      const mean = (c00 + c10 + c01 + c11) / 4 / gradient;
      const limb =
        mean <= OUTSIDE_PX - SENTINEL_SPARE_PX &&
        Math.max(...depths) >= -LIMB_OVERLAP_PX + SENTINEL_SPARE_PX;
      const interior = whole && depths.every((depth) => depth < -SENTINEL_SPARE_PX);
      if (limb || interior) {
        pixels.push([x, y]);
      }
    }
  }
  return pixels;
}

/** The worst texel error over its tolerance against the CPU rasteriser, and the classes' match. */
function twinAgreement(
  record: DiscRecord,
  viewport: Viewport,
  bytes: ArrayBuffer,
  texels: ClassMapTexels | null,
): { readonly worst: number; readonly classes: boolean; readonly pixels: number } {
  const expected = compositeDiscPixels(rasteriseDisc(record, CAMERA, viewport, texels));
  const drawn = halfTexels(bytes);
  let worst = 0;
  let classes = true;
  for (const pixel of expected) {
    const at = (pixel.yPx * viewport.widthPx + pixel.xPx) * 4;
    for (const c of [0, 1, 2] as const) {
      const want = pixel.rgb[c];
      const got = drawn[at + c] ?? Number.NaN;
      const error = Math.abs(got - want) / (TEXEL_ABSOLUTE + TEXEL_RELATIVE * want);
      worst = Math.max(worst, Number.isFinite(error) ? error : Number.POSITIVE_INFINITY);
    }
    // The limb keeps the class beneath: the target's cleared `other`.
    classes &&= (drawn[at + 3] ?? Number.NaN) === (pixel.meterClass ?? METER_CLASS.other);
  }
  return { worst, classes, pixels: expected.length };
}

/** One case's result, and whether it holds. */
function checkCase(
  item: CellCase,
  plan: BodyFramePlan,
  drawn: Readonly<Record<DrawnBy, { readonly bytes: ArrayBuffer; readonly throughCells: boolean }>>,
): { readonly ok: boolean; readonly text: string } {
  const { ulps, differing, alphasEqual } = halfUlpsApart(
    drawn.pass.bytes,
    drawn["in-fragment"].bytes,
  );
  const sentinel = halfTexels(drawn.sentinel.bytes);
  let ok =
    drawn.pass.throughCells &&
    drawn.sentinel.throughCells &&
    !drawn["in-fragment"].throughCells &&
    ulps <= 1 &&
    alphasEqual;
  const bodies: string[] = [];
  for (const spec of item.bodies) {
    const record = plan.discs.find((each) => each.body === spec.id);
    const regime = plan.regimes.get(spec.id);
    const samples = record === undefined ? [0, 0] : [record.interiorSamples, record.limbSamples];
    const read =
      record === undefined
        ? []
        : passPixels(record, item.viewport).filter(
            ([x, y]) => (sentinel[(y * item.viewport.widthPx + x) * 4] ?? 0) >= SENTINEL,
          );
    const expected = record === undefined ? 0 : passPixels(record, item.viewport).length;
    ok &&=
      record !== undefined &&
      (record.atmosphere !== undefined) === (spec.atmosphere !== undefined) &&
      regime === (spec.mesh ? "mesh" : "disc") &&
      samples[0] === spec.samples[0] &&
      samples[1] === spec.samples[1] &&
      expected > 0 &&
      read.length === expected;
    bodies.push(
      `${String(spec.diameterPx)} px${spec.atmosphere === undefined ? "" : " with air"}${spec.mapped === undefined ? "" : " under a class map"} (${String(regime)}, ${String(samples[0])} inside, ${String(samples[1])} × ${String(samples[1])} on the limb), ${String(read.length)} of its ${String(expected)} pixels read the pass`,
    );
  }
  const [only] = item.bodies;
  const record = only === undefined ? undefined : plan.discs.find((each) => each.body === only.id);
  const twin =
    item.twin && record !== undefined
      ? twinAgreement(record, item.viewport, drawn.pass.bytes, only?.mapped?.texels ?? null)
      : null;
  ok &&= twin === null || (twin.worst <= 1 && twin.classes && twin.pixels > 0);
  const twinText =
    twin === null
      ? ""
      : `; ${String(twin.pixels)} pixels within ${twin.worst.toFixed(3)} of the twin's tolerance, its classes ${String(twin.classes)}`;
  return {
    ok,
    text: `${item.name}: ${bodies.join(", ")}; through the pass ${String(drawn.pass.throughCells)}; within ${String(ulps)} ulp of the in-fragment draw (${String(differing)} channels differ); classes equal ${String(alphasEqual)}${twinText}`,
  };
}

/** A case's one body, a little off the view's centre and the pixel grid. */
function one(body: Omit<CellBody, "id" | "offsetPx">): ReadonlyArray<CellBody> {
  return [{ id: "0200080020000000.0001", offsetPx: [0.3, -0.2], ...body }];
}

/** The check's cases, on R05's Earth's air and a half-surveyed two-class map. */
function casesOf(
  atmosphere: SurfaceLightingTables,
  mapped: { readonly surface: ClassMapDiscSurface; readonly texels: ClassMapTexels },
): ReadonlyArray<CellCase> {
  const small = { widthPx: 48, heightPx: 48 };
  return [
    {
      name: "a 13 px air body as a disc",
      viewport: small,
      bodies: one({ diameterPx: 13, atmosphere, mesh: false, samples: [4, 4] }),
      twin: false,
    },
    {
      name: "a 13 px air body as the view draws it, a mesh",
      viewport: small,
      bodies: one({ diameterPx: 13, atmosphere, mesh: true, samples: [4, 4] }),
      twin: false,
    },
    {
      name: "a 13 px body under a half-surveyed two-class map",
      viewport: small,
      bodies: one({ diameterPx: 13, mapped, mesh: false, samples: [4, 4] }),
      twin: true,
    },
  ];
}

/**
 * Checks that bodies with air and under a class map, disc and mesh, draw through the `disc cells`
 * pass what they draw in-fragment, and that their pixels read the pass (the module's remarks).
 */
export async function checkCellPassEveryRecord(
  engine: RenderEngine,
  checks: Checks,
): Promise<void> {
  const kept = keepingSums(engine);
  const renderer = new LitBodyRenderer(kept.engine, WIREFRAME_MATERIALS.starSprite);
  let tables: AtmosphereTables | null = null;
  let weights: ClassMapDiscSurface | null = null;
  try {
    tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);
    // Half the cells surveyed (i + j even): class 0 where i and j are even, a quarter class 0 and
    // three quarters class 1 where both are odd (T8.b's map).
    const twoClass = packClassMap(4, 2, ({ i, j }) =>
      (i + j) % 2 !== 0 ? null : i % 2 === 0 ? [1, 0] : [0.25, 0.75],
    );
    weights = classMapSurface(
      engine,
      "smoke cell pass class map",
      twoClass,
      [
        lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
        lawFor([0.5, 0.45, 0.4], [1.3, 1.3, 1.3], "earth"),
      ],
      PROVISIONAL_PHOTOMETRY.law,
    );
    const kernel = await engine.createComputeAsync(BODY_DISC_CELLS_KERNEL);
    const cases = casesOf(surfaceLightingTablesOf(tables), {
      surface: weights,
      texels: twoClass,
    });
    let pass = true;
    const results: string[] = [];
    for (const item of cases) {
      const plan = planOf(item);
      const draw = (by: DrawnBy, drawnPlan: BodyFramePlan) =>
        drawPlan(engine, renderer, kept.sums, drawnPlan, item.viewport, kernel, by);
      // The harness reads its targets back in order; each draw takes a plan of its own, since a
      // plan's records are written once.
      // oxlint-disable-next-line no-await-in-loop
      const throughPass = await draw("pass", plan);
      // oxlint-disable-next-line no-await-in-loop
      const inFragment = await draw("in-fragment", planOf(item));
      // oxlint-disable-next-line no-await-in-loop
      const sentinel = await draw("sentinel", planOf(item));
      const result = checkCase(item, plan, {
        pass: throughPass,
        "in-fragment": inFragment,
        sentinel,
      });
      pass &&= result.ok;
      results.push(result.text);
    }
    checks.check(
      "R07.T8.e G11 air bodies and a class map's body, disc and mesh, drawn through the disc cells pass equal their in-fragment draws within one rgba16float ulp, and read the pass",
      pass,
      results.join("; "),
    );
  } finally {
    renderer.dispose();
    tables?.dispose();
    if (weights !== null) {
      engine.releaseTexture(weights.weights);
    }
  }
}
