/**
 * The smoke page's checks of the `disc cells` pass for every record (plan R07, R07.T8.e to
 * R07.T8.g; decision-r08-t11-frame §2 and §7): G11 on the GPU, each plan drawn through the pass and
 * in-fragment in the same run, every channel within one `rgba16float` ulp and every alpha (the
 * interior's meter class) equal. The plans hold a 13 px body with air (R05's Earth's tables) as a
 * disc and as the mesh the view draws it as, and a 13 px disc under a half-surveyed two-class map,
 * which the pass now binds (T8.e); a 220 px airless disc's limb, and a 96 px air body's limb in the
 * mesh regime, which it now takes (T8.f), each with its face's near-limb band (T8.g); and one plan
 * of four such bodies, three sets of textures and two limbs, so that a later set's jobs and a later
 * limb's span rows are read. The large airless disc and the class map's disc are held to the CPU
 * rasteriser too (G10).
 *
 * @remarks
 * The pass is exact, so an in-fragment draw where the pass was meant equals it too: a third draw of
 * each plan proves its pixels read the pass. Its sums are overwritten after the dispatch with a
 * sentinel far above any light drawn, and every pixel the draws must take from the pass shows it:
 * each pixel of a disc under 32 px, and each limb pixel of a larger one, that the limb draw's
 * corner tests (`bodyDisc.wgsl`'s `disc_pixel`) take in `f64` with a hundredth of a pixel to spare,
 * so that `f32`'s rounding at the tests' edges cannot decide it; and each face pixel of a larger
 * one whose one cell `cell_samples` shades at more than one point (T8.g), with as much to spare.
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
import { SourceTableEngine } from "../view/atmosphere/sourceTableEngine";
import { AtmosphereTables } from "../view/atmosphere/tables";
import {
  compositeDiscPixels,
  type DiscRecord,
  isLeanRecord,
  LIMB_OVERLAP_PX,
  rasteriseDisc,
  viewRay,
} from "../view/bodies/discShading";
import { type ClassMapTexels, classMapSurface, packClassMap } from "../view/bodies/discSurface";
import {
  BODY_DISC_CELLS_KERNEL,
  BODY_DISC_CELLS_LEAN_KERNEL,
  BODY_DISC_CELLS_PACKED_KERNEL,
  BODY_DISC_CELLS_PACKED_LEAN_KERNEL,
  type BodyFramePlan,
  CELL_SUMS_BUFFER_NAME,
  type LitBodyEngine,
  type LitBodyInput,
  LitBodyRenderer,
  type LitBodyRing,
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
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, ComputeHandle, RenderEngine } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { ringProfile } from "../view/rings/ringProfile";
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

/** `cell_samples`' step of the limb angle's gradient (`bodyDisc.wgsl`'s `GRADIENT_STEP_PX`), px. */
const GRADIENT_STEP_PX = 1 / 64;

/** The depth inside the limb within which a cell is shaded at more than one point, cell widths. */
const NEAR_LIMB_CELLS = 2;

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
  /** The ring whose shadow falls on it (plan R11, R11.T9.a); absent for none. */
  readonly ring?: LitBodyRing;
}

/** One case of the check: a plan of bodies in a view. */
interface CellCase {
  readonly name: string;
  readonly viewport: Viewport;
  readonly bodies: ReadonlyArray<CellBody>;
  /** Whether the pass's texels are held to the CPU rasteriser (G10): one uniform or mapped body. */
  readonly twin: boolean;
}

/**
 * A case's plan: its bodies lit by a Sun 1 au away, `sunDeg` off the line of sight (80° by default,
 * a gibbous phase).
 */
function planOf(item: CellCase, sunDeg = 80): BodyFramePlan {
  const { viewport } = item;
  const pxPerRad = viewport.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
  const towards = vec3(Math.sin(sunDeg * RAD_PER_DEG), 0, Math.cos(sunDeg * RAD_PER_DEG));
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
      ...(spec.ring === undefined ? {} : { ring: spec.ring }),
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
 * sentinel draw can overwrite the sums the draws then read, and making it readable; every call is
 * the engine's own.
 */
function keepingSums(engine: RenderEngine): {
  readonly engine: LitBodyEngine;
  readonly sums: () => BufferHandle | null;
} {
  let sums: BufferHandle | null = null;
  const kept: LitBodyEngine = {
    createMaterial: (...args) => engine.createMaterial(...args),
    createMesh: (...args) => engine.createMesh(...args),
    // The sums read back too (plan R07's packed kernel against the per-cell one), which the
    // renderer's own buffer, never read back, is not made for.
    createBuffer: (spec) =>
      engine.createBuffer(
        spec.name === CELL_SUMS_BUFFER_NAME
          ? { ...spec, usage: spec.usage | BUFFER_USAGE.COPY_SRC }
          : spec,
      ),
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
    createComputeAsync: (...args) => engine.createComputeAsync(...args),
    releaseCompute: (...args) => {
      engine.releaseCompute(...args);
    },
    onRestored: (...args) => engine.onRestored(...args),
    get capabilities() {
      return engine.capabilities;
    },
    prepareMaterial: (...args) => engine.prepareMaterial(...args),
    releaseMaterial: (...args) => {
      engine.releaseMaterial(...args);
    },
    releaseMesh: (...args) => {
      engine.releaseMesh(...args);
    },
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
  leanCells: ComputeHandle | null = null,
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
      renderer.dispatchCells(
        plan,
        cells,
        { viewRotation, projection, size: viewport },
        null,
        null,
        null,
        { plain: leanCells, emissive: null },
      );
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
 * corner tests in `f64` with {@link SENTINEL_SPARE_PX} to spare; and where the interior takes one
 * cell beside a larger limb's, each its interior draw takes whose centre's depth, less half its
 * extent across the limb's normal, is within {@link NEAR_LIMB_CELLS} of the limb (R07.T8.g's band),
 * with as much to spare.
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
  const banded = !whole && record.interiorSamples === 1;
  // `cell_samples`' test at the pixel's centre: more than one point within the band.
  const inBand = (x: number, y: number): boolean => {
    const at = angleAt(x + 0.5, y + 0.5);
    const sx = (angleAt(x + 0.5 + GRADIENT_STEP_PX, y + 0.5) - at) / GRADIENT_STEP_PX;
    const sy = (angleAt(x + 0.5, y + 0.5 + GRADIENT_STEP_PX) - at) / GRADIENT_STEP_PX;
    const slope = Math.hypot(sx, sy);
    const halfExtent = (Math.abs(sx) + Math.abs(sy)) / slope / 2;
    return slope > 0 && -at / slope - halfExtent <= NEAR_LIMB_CELLS - SENTINEL_SPARE_PX;
  };
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
      const inside = depths.every((depth) => depth < -SENTINEL_SPARE_PX);
      const interior = inside && (whole || (banded && inBand(x, y)));
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
    {
      name: "a 220 px airless disc's limb",
      viewport: { widthPx: 256, heightPx: 256 },
      bodies: one({ diameterPx: 220, mesh: false, samples: [1, 4] }),
      twin: true,
    },
    {
      name: "a 96 px air body's limb in the mesh regime",
      viewport: { widthPx: 128, heightPx: 112 },
      bodies: one({ diameterPx: 96, atmosphere, mesh: true, samples: [1, 4] }),
      twin: false,
    },
    {
      name: "four bodies of three sets of textures in one plan",
      viewport: { widthPx: 192, heightPx: 112 },
      bodies: [
        {
          id: "0200080020000000.0011",
          diameterPx: 13,
          offsetPx: [-70.3, -25.2],
          atmosphere,
          mesh: true,
          samples: [4, 4],
        },
        {
          id: "0200080020000000.0012",
          diameterPx: 40,
          offsetPx: [-20.4, 15.3],
          mapped,
          mesh: false,
          samples: [1, 4],
        },
        {
          id: "0200080020000000.0013",
          diameterPx: 48,
          offsetPx: [45.2, 0.4],
          mesh: false,
          samples: [1, 4],
        },
        {
          id: "0200080020000000.0014",
          diameterPx: 13,
          offsetPx: [80.1, -35.3],
          mesh: false,
          samples: [4, 4],
        },
      ],
      twin: false,
    },
  ];
}

/**
 * Checks that bodies with air, under a class map and with large limbs, disc and mesh, draw through
 * the `disc cells` pass what they draw in-fragment, and that their pixels read the pass (the
 * module's remarks).
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
    const [kernel, packedKernel] = await Promise.all([
      engine.createComputeAsync(BODY_DISC_CELLS_KERNEL),
      engine.createComputeAsync(BODY_DISC_CELLS_PACKED_KERNEL),
    ]);
    const cases = casesOf(surfaceLightingTablesOf(tables), {
      surface: weights,
      texels: twoClass,
    });
    let pass = true;
    let packedEqual = true;
    const results: string[] = [];
    const packedResults: string[] = [];
    for (const item of cases) {
      const plan = planOf(item);
      const draw = (by: DrawnBy, drawnPlan: BodyFramePlan, cells = kernel) =>
        drawPlan(engine, renderer, kept.sums, drawnPlan, item.viewport, cells, by);
      // The pass's sums as the draws found them, its whole buffer, where it dispatched.
      const sumsNow = async (): Promise<Uint8Array | null> => {
        const buffer = kept.sums();
        return buffer === null ? null : new Uint8Array(await engine.readBuffer(buffer));
      };
      // The harness reads its targets back in order; each draw takes a plan of its own, since a
      // plan's records are written once.
      // oxlint-disable-next-line no-await-in-loop
      const throughPass = await draw("pass", plan);
      // Read before the next dispatch writes the renderer's one sums buffer again.
      // oxlint-disable-next-line no-await-in-loop
      const perCellSums = await sumsNow();
      // The packed kernel's (`GeometrySettings.discCells`): its sums and its draws byte for byte
      // the per-cell kernel's, the bytes past its jobs' sums left as the per-cell run left them.
      // Drawn and read back in turn, as every draw of the case is.
      // oxlint-disable-next-line no-await-in-loop
      const packed = await draw("pass", planOf(item), packedKernel);
      // Read before the in-fragment draw's sentinel overwrites the buffer.
      // oxlint-disable-next-line no-await-in-loop
      const packedSums = await sumsNow();
      const sumsEqual =
        perCellSums !== null &&
        packedSums !== null &&
        perCellSums.length === packedSums.length &&
        perCellSums.every((byte, i) => byte === packedSums[i]);
      const drawsEqual =
        packed.throughCells === throughPass.throughCells &&
        bytesEqual(packed.bytes, throughPass.bytes);
      packedEqual &&= sumsEqual && drawsEqual;
      packedResults.push(
        `${item.name}: sums ${String(sumsEqual)} (${String(perCellSums?.length ?? 0)} bytes), draws ${String(drawsEqual)}`,
      );
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
      "R07.T8.e–g G11 air bodies, a class map's body and large discs' limbs and their faces' near-limb bands, disc and mesh, alone and together, drawn through the disc cells pass equal their in-fragment draws within one rgba16float ulp, and read the pass",
      pass,
      results.join("; "),
    );
    checks.check(
      "plan R07's packed disc cells (the owner's Mac): every plan's sums through the packed kernel equal the per-cell kernel's byte for byte, and so do its draws",
      packedEqual,
      packedResults.join("; "),
    );
  } finally {
    renderer.dispose();
    tables?.dispose();
    if (weights !== null) {
      engine.releaseTexture(weights.weights);
    }
  }
}

/** Whether two read-back buffers hold the same bytes. */
function bytesEqual(a: ArrayBuffer, b: ArrayBuffer): boolean {
  const x = new Uint8Array(a);
  const y = new Uint8Array(b);
  return x.length === y.length && x.every((byte, i) => byte === y[i]);
}

/** The share of a frame's lit pixels the sources' table holds within 1/3 stop (R08.T9.b's lever). */
const SOURCE_TABLE_HELD_SHARE = 0.995;

/**
 * R08.T9.b's lever on drawn bodies: R05's Earth's air on a 13 px disc, a 96 px mesh body and a
 * 220 px disc, each with the Sun 80°, 90° and 120° off the line of sight, drawn in-fragment with
 * the sources' table bound (`AtmosphereSettings.sourceTransmittance`'s `table`) and with its stub
 * (`per-fragment`); at least {@link SOURCE_TABLE_HELD_SHARE} of the pixels either draw's meter class
 * says the Sun lights within 1/3 stop in every channel (a channel both leave dark agreeing), the
 * classes equal.
 */
export async function checkSourceTableFrames(engine: RenderEngine, checks: Checks): Promise<void> {
  const kept = keepingSums(engine);
  const renderer = new LitBodyRenderer(kept.engine, WIREFRAME_MATERIALS.starSprite);
  let tables: AtmosphereTables | null = null;
  try {
    tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);
    const kernel = await engine.createComputeAsync(BODY_DISC_CELLS_KERNEL);
    const ways = {
      perFragment: surfaceLightingTablesOf(tables),
      table: surfaceLightingTablesOf(tables, "table"),
    };
    const casesWith = (atmosphere: SurfaceLightingTables): ReadonlyArray<CellCase> => [
      {
        name: "a 13 px air body as a disc",
        viewport: { widthPx: 48, heightPx: 48 },
        bodies: one({ diameterPx: 13, atmosphere, mesh: false, samples: [4, 4] }),
        twin: false,
      },
      {
        name: "a 96 px air body in the mesh regime",
        viewport: { widthPx: 128, heightPx: 112 },
        bodies: one({ diameterPx: 96, atmosphere, mesh: true, samples: [1, 4] }),
        twin: false,
      },
      {
        name: "a 220 px air body as a disc",
        viewport: { widthPx: 256, heightPx: 256 },
        bodies: one({ diameterPx: 220, atmosphere, mesh: false, samples: [1, 4] }),
        twin: false,
      },
    ];
    const perFragmentCases = casesWith(ways.perFragment);
    const tableCases = casesWith(ways.table);
    let lit = 0;
    let held = 0;
    let classes = true;
    let worstStops = 0;
    const results: string[] = [];
    for (const [i, item] of perFragmentCases.entries()) {
      const tabled = tableCases[i];
      if (tabled === undefined) {
        throw new Error("each case is drawn both ways");
      }
      for (const sunDeg of [80, 90, 120]) {
        const draw = (drawn: CellCase) =>
          drawPlan(
            engine,
            renderer,
            kept.sums,
            planOf(drawn, sunDeg),
            drawn.viewport,
            kernel,
            "in-fragment",
          );
        // The harness reads its targets back in order.
        // oxlint-disable-next-line no-await-in-loop
        const want = halfTexels((await draw(item)).bytes);
        // oxlint-disable-next-line no-await-in-loop
        const got = halfTexels((await draw(tabled)).bytes);
        let caseLit = 0;
        let caseHeld = 0;
        for (let at = 0; at < want.length; at += 4) {
          const wantClass = want[at + 3];
          const gotClass = got[at + 3];
          classes &&= wantClass === gotClass;
          if (wantClass !== METER_CLASS.litBody && gotClass !== METER_CLASS.litBody) {
            continue;
          }
          caseLit += 1;
          let stops = 0;
          for (const c of [0, 1, 2]) {
            const w = want[at + c] ?? Number.NaN;
            const g = got[at + c] ?? Number.NaN;
            if (w === 0 && g === 0) {
              continue;
            }
            const ratio = g / w;
            stops = Math.max(
              stops,
              ratio > 0 && Number.isFinite(ratio)
                ? Math.abs(Math.log2(ratio))
                : Number.POSITIVE_INFINITY,
            );
          }
          worstStops = Math.max(worstStops, stops);
          if (stops <= 1 / 3) {
            caseHeld += 1;
          }
        }
        lit += caseLit;
        held += caseHeld;
        results.push(
          `${item.name} at ${String(sunDeg)}°: ${String(caseHeld)} of ${String(caseLit)} lit pixels`,
        );
      }
    }
    const share = lit > 0 ? held / lit : 0;
    checks.check(
      `R08.T9.b's lever: bodies drawn with the sources' table within 1/3 stop of their draws per fragment on ${String(SOURCE_TABLE_HELD_SHARE * 100)}% or more of their lit pixels, their classes equal`,
      lit > 0 && share >= SOURCE_TABLE_HELD_SHARE && classes,
      `${String(held)} of ${String(lit)} lit pixels (${(share * 100).toFixed(2)}%) within 1/3 stop, worst ${worstStops.toFixed(3)} stops; classes equal ${String(classes)}; ${results.join("; ")}`,
    );
  } finally {
    renderer.dispose();
    tables?.dispose();
  }
}

/**
 * R08.T9.b's lever on drawn bodies of one band, by their modules' table-only twins
 * (`SourceTableEngine`): R05's Earth's air on a 13 px disc, a 96 px mesh body and a 220 px disc,
 * the Sun 80° and 120° off the line of sight, its sources' table bound, drawn in-fragment and
 * through the `disc cells` pass, once on the engine the page's wraps (its modules as written) and
 * once on the page's, its twins made and prepared by a first draw; every byte the same, and the
 * twins taken.
 */
export async function checkSourceTableTwins(engine: RenderEngine, checks: Checks): Promise<void> {
  if (!(engine instanceof SourceTableEngine)) {
    checks.check(
      "R08.T9.b's lever: bodies of one band drawn by their table-only twins draw their modules' bytes",
      false,
      "the page's engine is not a SourceTableEngine",
    );
    return;
  }
  const written = keepingSums(engine.inner);
  const twinned = keepingSums(engine);
  const writtenRenderer = new LitBodyRenderer(written.engine, WIREFRAME_MATERIALS.starSprite);
  const twinnedRenderer = new LitBodyRenderer(twinned.engine, WIREFRAME_MATERIALS.starSprite);
  let tables: AtmosphereTables | null = null;
  try {
    tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);
    const atmosphere = surfaceLightingTablesOf(tables, "table");
    const writtenKernel = await engine.inner.createComputeAsync(BODY_DISC_CELLS_KERNEL);
    const twinnedKernel = await engine.createComputeAsync(BODY_DISC_CELLS_KERNEL);
    const cases: ReadonlyArray<CellCase> = [
      {
        name: "a 13 px air body as a disc",
        viewport: { widthPx: 48, heightPx: 48 },
        bodies: one({ diameterPx: 13, atmosphere, mesh: false, samples: [4, 4] }),
        twin: false,
      },
      {
        name: "a 96 px air body in the mesh regime",
        viewport: { widthPx: 128, heightPx: 112 },
        bodies: one({ diameterPx: 96, atmosphere, mesh: true, samples: [1, 4] }),
        twin: false,
      },
      {
        name: "a 220 px air body as a disc",
        viewport: { widthPx: 256, heightPx: 256 },
        bodies: one({ diameterPx: 220, atmosphere, mesh: false, samples: [1, 4] }),
        twin: false,
      },
    ];
    const ways: ReadonlyArray<DrawnBy> = ["in-fragment", "pass"];
    const draw = (
      twin: boolean,
      item: CellCase,
      sunDeg: number,
      by: DrawnBy,
    ): Promise<{ readonly bytes: ArrayBuffer; readonly throughCells: boolean }> =>
      drawPlan(
        twin ? engine : engine.inner,
        twin ? twinnedRenderer : writtenRenderer,
        twin ? twinned.sums : written.sums,
        planOf(item, sunDeg),
        item.viewport,
        twin ? twinnedKernel : writtenKernel,
        by,
      );
    // A first draw of each through the page's engine makes and prepares its twins.
    for (const item of cases) {
      for (const by of ways) {
        // The harness reads its targets back in order.
        // oxlint-disable-next-line no-await-in-loop
        await draw(true, item, 80, by);
      }
    }
    await engine.twinsSettled();
    const swapsBefore = engine.swaps;
    let pixels = 0;
    let differing = 0;
    const results: string[] = [];
    for (const item of cases) {
      for (const sunDeg of [80, 120]) {
        for (const by of ways) {
          // oxlint-disable-next-line no-await-in-loop
          const asWritten = await draw(false, item, sunDeg, by);
          // oxlint-disable-next-line no-await-in-loop
          const byTwins = await draw(true, item, sunDeg, by);
          const apart = halfUlpsApart(asWritten.bytes, byTwins.bytes);
          pixels += asWritten.bytes.byteLength / 8;
          differing += apart.differing;
          results.push(
            `${item.name} at ${String(sunDeg)}° ${by}: ${String(apart.differing)} channels differ`,
          );
        }
      }
    }
    const swaps = engine.swaps - swapsBefore;
    checks.check(
      "R08.T9.b's lever: bodies of one band drawn by their table-only twins draw their modules' bytes",
      differing === 0 && swaps > 0,
      `${String(pixels)} pixels, ${String(differing)} channels differing; ${String(swaps)} draws and dispatches by a twin; ${results.join("; ")}`,
    );
  } finally {
    writtenRenderer.dispose();
    twinnedRenderer.dispose();
    tables?.dispose();
  }
}

/**
 * Plan R07's airless twins (perf-bodies-pass; `SourceTableEngine`): airless bodies, a 13 px disc
 * and a 13 px one under a half-surveyed two-class map, a 96 px mesh body, plain and mapped, and a
 * 220 px disc, plain and under a two-class map whose classes' edges are keyed (lane
 * fix-jagged-biome-edges' `classEdge.wgsl`), as is a 96 px mesh body's, and a 96 px mesh body under
 * a Saturn-like ring's shadow (plan R11, R11.T9.a's `ringShadow.wgsl`), the Sun 80° and 120° off
 * the line of sight, drawn in-fragment and through the `disc cells` pass, per cell and packed, once
 * on the engine the page's wraps (their modules as written) and once on the page's, their twins made
 * and prepared by a first draw; every byte the same, the twins taken, the keyed edges moving the
 * 220 px disc's bytes from the same map's unkeyed, and the ring's shadow the mesh body's from its
 * ringless.
 */
export async function checkAirlessTwins(engine: RenderEngine, checks: Checks): Promise<void> {
  const name = "plan R07: airless bodies drawn by their airless twins draw their modules' bytes";
  if (!(engine instanceof SourceTableEngine)) {
    checks.check(name, false, "the page's engine is not a SourceTableEngine");
    return;
  }
  const written = keepingSums(engine.inner);
  const twinned = keepingSums(engine);
  const writtenRenderer = new LitBodyRenderer(written.engine, WIREFRAME_MATERIALS.starSprite);
  const twinnedRenderer = new LitBodyRenderer(twinned.engine, WIREFRAME_MATERIALS.starSprite);
  let weights: ClassMapDiscSurface | null = null;
  let halvesWeights: ClassMapDiscSurface | null = null;
  // The kernels made, each with the engine that made it, released at the end.
  const made: Array<{ readonly by: RenderEngine; readonly kernel: ComputeHandle }> = [];
  try {
    const twoClass = packClassMap(4, 2, ({ i, j }) =>
      (i + j) % 2 !== 0 ? null : i % 2 === 0 ? [1, 0] : [0.25, 0.75],
    );
    const laws = [
      lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
      lawFor([0.5, 0.45, 0.4], [1.3, 1.3, 1.3], "earth"),
    ];
    weights = classMapSurface(
      engine,
      "smoke airless twins class map",
      twoClass,
      laws,
      PROVISIONAL_PHOTOMETRY.law,
    );
    const mapped = { surface: weights, texels: twoClass };
    // The classes' edges (lane fix-jagged-biome-edges): a two-class map of four texels a face split
    // along i = 2, its classes' edges keyed, so that their noise shapes them, as the disc's own check
    // of them draws it (`bodies.ts`' `checkEdgeKeys`); and the same map with no keys.
    const halves = packClassMap(4, 2, ({ i }) => (i < 2 ? [1, 0] : [0, 1]));
    halvesWeights = classMapSurface(
      engine,
      "smoke airless twins keyed edges",
      halves,
      laws,
      PROVISIONAL_PHOTOMETRY.law,
    );
    const keyed = {
      surface: { ...halvesWeights, edgeKeys: [0x51f00d, 0x2c0ffe] },
      texels: halves,
    };
    const unkeyed = { surface: halvesWeights, texels: halves };
    // A Saturn-like massive ring of porous ice, 1.24 a to 2.27 a, τ 1.5, a gap at 1.95 a (the
    // rings' own check's, `rings.ts`), its plane 37° from the Sun at 80°, so that its shadow falls
    // across the lit face below it.
    const a = EARTH.equatorialRadiusM;
    const ring: LitBodyRing = {
      profile: ringProfile({
        ringKind: "massive",
        material: "porous_ice",
        innerRadiusM: 1.24 * a,
        outerRadiusM: 2.27 * a,
        opticalDepth: 1.5,
        gapRadiiM: [1.95 * a],
      }),
      normal: vec3(0.6, 0.8, 0),
    };
    const [writtenKernel, writtenPacked, twinnedKernel, twinnedPacked] = await Promise.all([
      engine.inner.createComputeAsync(BODY_DISC_CELLS_KERNEL),
      engine.inner.createComputeAsync(BODY_DISC_CELLS_PACKED_KERNEL),
      engine.createComputeAsync(BODY_DISC_CELLS_KERNEL),
      engine.createComputeAsync(BODY_DISC_CELLS_PACKED_KERNEL),
    ]);
    made.push(
      { by: engine.inner, kernel: writtenKernel },
      { by: engine.inner, kernel: writtenPacked },
      { by: engine, kernel: twinnedKernel },
      { by: engine, kernel: twinnedPacked },
    );
    const small = { widthPx: 48, heightPx: 48 };
    const cases: ReadonlyArray<CellCase> = [
      {
        name: "a 13 px airless disc",
        viewport: small,
        bodies: one({ diameterPx: 13, mesh: false, samples: [4, 4] }),
        twin: false,
      },
      {
        name: "a 13 px disc under a two-class map",
        viewport: small,
        bodies: one({ diameterPx: 13, mapped, mesh: false, samples: [4, 4] }),
        twin: false,
      },
      {
        name: "a 96 px airless body in the mesh regime",
        viewport: { widthPx: 128, heightPx: 112 },
        bodies: one({ diameterPx: 96, mesh: true, samples: [1, 4] }),
        twin: false,
      },
      {
        name: "a 96 px mesh body under a two-class map",
        viewport: { widthPx: 128, heightPx: 112 },
        bodies: one({ diameterPx: 96, mapped, mesh: true, samples: [1, 4] }),
        twin: false,
      },
      {
        name: "a 220 px airless disc",
        viewport: { widthPx: 256, heightPx: 256 },
        bodies: one({ diameterPx: 220, mesh: false, samples: [1, 4] }),
        twin: false,
      },
      {
        name: "a 220 px disc under a two-class map, its edges keyed",
        viewport: { widthPx: 256, heightPx: 256 },
        bodies: one({ diameterPx: 220, mapped: keyed, mesh: false, samples: [1, 4] }),
        twin: false,
      },
      {
        name: "a 96 px mesh body under a two-class map, its edges keyed",
        viewport: { widthPx: 128, heightPx: 112 },
        bodies: one({ diameterPx: 96, mapped: keyed, mesh: true, samples: [1, 4] }),
        twin: false,
      },
      {
        name: "a 96 px airless mesh body under its ring's shadow",
        viewport: { widthPx: 128, heightPx: 112 },
        bodies: one({ diameterPx: 96, mesh: true, samples: [1, 4], ring }),
        twin: false,
      },
    ];
    const keyedDisc = cases.find((item) => item.name.startsWith("a 220 px disc under"));
    const ringless = cases.find((item) => item.name === "a 96 px airless body in the mesh regime");
    const ringed = cases.find((item) => item.name.endsWith("under its ring's shadow"));
    const unkeyedDisc: CellCase = {
      name: "a 220 px disc under the same map, unkeyed",
      viewport: { widthPx: 256, heightPx: 256 },
      bodies: one({ diameterPx: 220, mapped: unkeyed, mesh: false, samples: [1, 4] }),
      twin: false,
    };
    const ways: ReadonlyArray<{ readonly by: DrawnBy; readonly packed: boolean }> = [
      { by: "in-fragment", packed: false },
      { by: "pass", packed: false },
      { by: "pass", packed: true },
    ];
    const draw = (
      twin: boolean,
      item: CellCase,
      sunDeg: number,
      way: { readonly by: DrawnBy; readonly packed: boolean },
    ): Promise<{ readonly bytes: ArrayBuffer; readonly throughCells: boolean }> =>
      drawPlan(
        twin ? engine : engine.inner,
        twin ? twinnedRenderer : writtenRenderer,
        twin ? twinned.sums : written.sums,
        planOf(item, sunDeg),
        item.viewport,
        twin
          ? way.packed
            ? twinnedPacked
            : twinnedKernel
          : way.packed
            ? writtenPacked
            : writtenKernel,
        way.by,
      );
    // A first draw of each through the page's engine makes and prepares its twins.
    for (const item of cases) {
      for (const way of ways) {
        // The harness reads its targets back in order.
        // oxlint-disable-next-line no-await-in-loop
        await draw(true, item, 80, way);
      }
    }
    await engine.twinsSettled();
    // A second round, for the kernels' twins, which stand in from the dispatch after they are made.
    for (const item of cases) {
      for (const way of ways) {
        // oxlint-disable-next-line no-await-in-loop
        await draw(true, item, 80, way);
      }
    }
    await engine.twinsSettled();
    const swapsBefore = engine.swaps;
    let pixels = 0;
    let differing = 0;
    const results: string[] = [];
    // The keyed disc's bytes as written in-fragment at 80°, which its edges move from the unkeyed's,
    // and the ringed body's and its ringless one's, which its ring's shadow moves.
    let keyedBytes: ArrayBuffer | null = null;
    let ringedBytes: ArrayBuffer | null = null;
    let ringlessBytes: ArrayBuffer | null = null;
    for (const item of cases) {
      for (const sunDeg of [80, 120]) {
        for (const way of ways) {
          // oxlint-disable-next-line no-await-in-loop
          const asWritten = await draw(false, item, sunDeg, way);
          // oxlint-disable-next-line no-await-in-loop
          const byTwins = await draw(true, item, sunDeg, way);
          const apart = halfUlpsApart(asWritten.bytes, byTwins.bytes);
          pixels += asWritten.bytes.byteLength / 8;
          differing += apart.differing;
          results.push(
            `${item.name} at ${String(sunDeg)}° ${way.by}${way.packed ? " packed" : ""}: ${String(apart.differing)} channels differ`,
          );
          if (sunDeg === 80 && way.by === "in-fragment") {
            keyedBytes = item === keyedDisc ? asWritten.bytes : keyedBytes;
            ringedBytes = item === ringed ? asWritten.bytes : ringedBytes;
            ringlessBytes = item === ringless ? asWritten.bytes : ringlessBytes;
          }
        }
      }
    }
    const swaps = engine.swaps - swapsBefore;
    const unkeyedBytes = (await draw(false, unkeyedDisc, 80, { by: "in-fragment", packed: false }))
      .bytes;
    const moved = keyedBytes === null ? 0 : halfUlpsApart(keyedBytes, unkeyedBytes).differing;
    const shaded =
      ringedBytes === null || ringlessBytes === null
        ? 0
        : halfUlpsApart(ringedBytes, ringlessBytes).differing;
    checks.check(
      name,
      differing === 0 && swaps > 0 && moved > 0 && shaded > 0,
      `${String(pixels)} pixels, ${String(differing)} channels differing; ${String(swaps)} draws and dispatches by a twin; the keyed edges move ${String(moved)} channels of the 220 px disc from its unkeyed map's, the ring's shadow ${String(shaded)} of the 96 px mesh body from its ringless; ${results.join("; ")}`,
    );
  } finally {
    writtenRenderer.dispose();
    twinnedRenderer.dispose();
    for (const { by, kernel } of made) {
      by.releaseCompute(kernel);
    }
    for (const surface of [weights, halvesWeights]) {
      if (surface !== null) {
        engine.releaseTexture(surface.weights);
      }
    }
  }
}

/** A way's cells kernel and its lean form from the four the lean forms' check makes, in its order. */
function kernelsOf(
  handles: ReadonlyArray<ComputeHandle>,
  packed: boolean,
): { readonly cells: ComputeHandle; readonly lean: ComputeHandle } {
  const [cells, cellsPacked, leanCells, leanPacked] = handles;
  if (
    cells === undefined ||
    cellsPacked === undefined ||
    leanCells === undefined ||
    leanPacked === undefined
  ) {
    throw new Error("the lean forms' check made fewer kernels than it asked for");
  }
  return packed ? { cells: cellsPacked, lean: leanPacked } : { cells, lean: leanCells };
}

/** A plan's draws' material names as a renderer makes them. */
function namesOf(renderer: LitBodyRenderer, plan: BodyFramePlan): ReadonlyArray<string> {
  return [...renderer.meshDraws(plan), ...renderer.draws(plan)].map((draw) => draw.material.name);
}

/**
 * Perf-bodies-pass-2's lean forms (`draw.ts`' `leanSource`): bodies of no planetshine source, no
 * occluder and no ring's shadow, airless and with R05's Earth's air, uniform and under the twins
 * check's maps, its edges keyed or not, a 13 px disc, a 96 px mesh body and a 220 px disc, the Sun
 * 80° and 120° off the line of sight, drawn in-fragment and through the `disc cells` pass, per cell
 * and packed, by a renderer made without its lean forms and by one with them prepared, and by the
 * latter through the page's engine, which draws the airless lean forms by their airless twins
 * (`SourceTableEngine`); every byte the same, the lean forms and their twins taken, and a body
 * under a ring's shadow and two lighting each other drawn by the full forms alike.
 */
export async function checkLeanForms(engine: RenderEngine, checks: Checks): Promise<void> {
  const name =
    "perf-bodies-pass-2: lean bodies drawn by the lean forms and their twins draw their full forms' bytes";
  if (!(engine instanceof SourceTableEngine)) {
    checks.check(name, false, "the page's engine is not a SourceTableEngine");
    return;
  }
  const full = keepingSums(engine.inner);
  const lean = keepingSums(engine.inner);
  const twinned = keepingSums(engine);
  const fullRenderer = new LitBodyRenderer(full.engine, WIREFRAME_MATERIALS.starSprite, {
    leanForms: false,
  });
  const leanRenderer = new LitBodyRenderer(lean.engine, WIREFRAME_MATERIALS.starSprite);
  const twinnedRenderer = new LitBodyRenderer(twinned.engine, WIREFRAME_MATERIALS.starSprite);
  const made: Array<{ readonly by: RenderEngine; readonly kernel: ComputeHandle }> = [];
  const surfaces: ClassMapDiscSurface[] = [];
  let tables: AtmosphereTables | null = null;
  try {
    const laws = [
      lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
      lawFor([0.5, 0.45, 0.4], [1.3, 1.3, 1.3], "earth"),
    ];
    const mapOf = (
      label: string,
      texels: ClassMapTexels,
      edgeKeys: ReadonlyArray<number> | null,
    ): { readonly surface: ClassMapDiscSurface; readonly texels: ClassMapTexels } => {
      const surface = classMapSurface(engine, label, texels, laws, PROVISIONAL_PHOTOMETRY.law);
      surfaces.push(surface);
      return { surface: edgeKeys === null ? surface : { ...surface, edgeKeys }, texels };
    };
    const mapped = mapOf(
      "smoke lean forms class map",
      packClassMap(4, 2, ({ i, j }) =>
        (i + j) % 2 !== 0 ? null : i % 2 === 0 ? [1, 0] : [0.25, 0.75],
      ),
      null,
    );
    const keyed = mapOf(
      "smoke lean forms keyed edges",
      packClassMap(4, 2, ({ i }) => (i < 2 ? [1, 0] : [0, 1])),
      [0x51f00d, 0x2c0ffe],
    );
    tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);
    const atmosphere = surfaceLightingTablesOf(tables, "table");
    const a = EARTH.equatorialRadiusM;
    const ring: LitBodyRing = {
      profile: ringProfile({
        ringKind: "massive",
        material: "porous_ice",
        innerRadiusM: 1.24 * a,
        outerRadiusM: 2.27 * a,
        opticalDepth: 1.5,
        gapRadiiM: [1.95 * a],
      }),
      normal: vec3(0.6, 0.8, 0),
    };
    const pairs = [
      BODY_DISC_CELLS_KERNEL,
      BODY_DISC_CELLS_PACKED_KERNEL,
      BODY_DISC_CELLS_LEAN_KERNEL,
      BODY_DISC_CELLS_PACKED_LEAN_KERNEL,
    ];
    const [written, twins] = await Promise.all([
      Promise.all(pairs.map((pair) => engine.inner.createComputeAsync(pair))),
      Promise.all(pairs.map((pair) => engine.createComputeAsync(pair))),
    ]);
    written.forEach((kernel) => made.push({ by: engine.inner, kernel }));
    twins.forEach((kernel) => made.push({ by: engine, kernel }));
    await Promise.all([leanRenderer.prepareLean(), twinnedRenderer.prepareLean()]);
    const small = { widthPx: 48, heightPx: 48 };
    const mid = { widthPx: 128, heightPx: 112 };
    const large = { widthPx: 256, heightPx: 256 };
    const lit = (body: Omit<CellBody, "id" | "offsetPx">): CellCase => ({
      name: "",
      viewport: body.diameterPx < 32 ? small : body.diameterPx < 128 ? mid : large,
      bodies: one(body),
      twin: false,
    });
    const leanCases: ReadonlyArray<CellCase> = [
      { ...lit({ diameterPx: 13, mesh: false, samples: [4, 4] }), name: "a 13 px airless disc" },
      {
        ...lit({ diameterPx: 13, mapped, mesh: false, samples: [4, 4] }),
        name: "a 13 px disc under a two-class map",
      },
      {
        ...lit({ diameterPx: 96, mesh: true, samples: [1, 4] }),
        name: "a 96 px airless mesh body",
      },
      {
        ...lit({ diameterPx: 96, mapped: keyed, mesh: true, samples: [1, 4] }),
        name: "a 96 px mesh body under a keyed map",
      },
      {
        ...lit({ diameterPx: 220, mapped: keyed, mesh: false, samples: [1, 4] }),
        name: "a 220 px disc under a keyed map",
      },
      {
        ...lit({ diameterPx: 13, atmosphere, mesh: false, samples: [4, 4] }),
        name: "a 13 px air disc",
      },
      {
        ...lit({ diameterPx: 96, atmosphere, mapped, mesh: true, samples: [1, 4] }),
        name: "a 96 px air mesh body under a two-class map",
      },
    ];
    // Bodies not lean: a ring's shadow, and two lighting each other by planetshine.
    const fullCases: ReadonlyArray<CellCase> = [
      {
        ...lit({ diameterPx: 96, mesh: true, samples: [1, 4], ring }),
        name: "a 96 px mesh body under its ring's shadow",
      },
      {
        name: "a 96 px mesh body and a 24 px disc lighting each other",
        viewport: large,
        bodies: [
          {
            id: "0200080020000000.0001",
            offsetPx: [-20.3, 3.2],
            diameterPx: 96,
            mesh: true,
            samples: [1, 4],
            mapped,
          },
          {
            id: "0200080020000000.0002",
            offsetPx: [62.7, -1.4],
            diameterPx: 24,
            mesh: false,
            samples: [2, 4],
          },
        ],
        twin: false,
      },
    ];
    const ways: ReadonlyArray<{ readonly by: DrawnBy; readonly packed: boolean }> = [
      { by: "in-fragment", packed: false },
      { by: "pass", packed: false },
      { by: "pass", packed: true },
    ];
    // A first draw of each through the page's engine makes and prepares its twins, a second its
    // kernels' twins, which stand in from the dispatch after they are made.
    for (let round = 0; round < 2; round += 1) {
      for (const item of [...leanCases, ...fullCases]) {
        for (const way of ways) {
          const { cells, lean: leanCells } = kernelsOf(twins, way.packed);
          // The harness reads its targets back in order.
          // oxlint-disable-next-line no-await-in-loop
          await drawPlan(
            engine,
            twinnedRenderer,
            twinned.sums,
            planOf(item, 80),
            item.viewport,
            cells,
            way.by,
            leanCells,
          );
        }
      }
      // The second round's draws wait for the first's twins.
      // oxlint-disable-next-line no-await-in-loop
      await engine.twinsSettled();
    }
    const swapsBefore = engine.swaps;
    let pixels = 0;
    let differing = 0;
    let leanDraws = 0;
    let fullDrawn = true;
    let recordsLean = true;
    const results: string[] = [];
    for (const item of [...leanCases, ...fullCases]) {
      const isLean = leanCases.includes(item);
      for (const sunDeg of [80, 120]) {
        // A plan of its own for each draw: a renderer writes a plan's records once.
        const probe = planOf(item, sunDeg);
        recordsLean &&= probe.discs.every((record) => isLeanRecord(record) === isLean);
        const names = namesOf(leanRenderer, probe);
        leanDraws += names.filter((each) => each.includes("Lean")).length;
        // Every draw of a lean case by a lean form, and none of the rest.
        fullDrawn &&= isLean
          ? names.length > 0 && names.every((each) => each.includes("Lean"))
          : !names.some((each) => each.includes("Lean"));
        for (const way of ways) {
          const own = kernelsOf(written, way.packed);
          const twin = kernelsOf(twins, way.packed);
          // Each draw reads its target back before the next is made.
          // oxlint-disable-next-line no-await-in-loop
          const asFull = await drawPlan(
            engine.inner,
            fullRenderer,
            full.sums,
            planOf(item, sunDeg),
            item.viewport,
            own.cells,
            way.by,
          );
          // Each draw reads its target back before the next is made.
          // oxlint-disable-next-line no-await-in-loop
          const asLean = await drawPlan(
            engine.inner,
            leanRenderer,
            lean.sums,
            planOf(item, sunDeg),
            item.viewport,
            own.cells,
            way.by,
            own.lean,
          );
          // Each draw reads its target back before the next is made.
          // oxlint-disable-next-line no-await-in-loop
          const byTwins = await drawPlan(
            engine,
            twinnedRenderer,
            twinned.sums,
            planOf(item, sunDeg),
            item.viewport,
            twin.cells,
            way.by,
            twin.lean,
          );
          const leanApart = halfUlpsApart(asFull.bytes, asLean.bytes).differing;
          const twinApart = halfUlpsApart(asLean.bytes, byTwins.bytes).differing;
          pixels += asFull.bytes.byteLength / 8;
          differing += leanApart + twinApart;
          if (leanApart + twinApart > 0) {
            results.push(
              `${item.name} at ${String(sunDeg)}° ${way.by}${way.packed ? " packed" : ""}: ${String(leanApart)} channels from the full forms', ${String(twinApart)} from the twins'`,
            );
          }
        }
      }
    }
    const swaps = engine.swaps - swapsBefore;
    checks.check(
      name,
      differing === 0 && leanDraws > 0 && fullDrawn && recordsLean && swaps > 0,
      `${String(pixels)} pixels a way, ${String(differing)} channels differing${results.length > 0 ? `: ${results.join("; ")}` : ""}; ${String(leanDraws)} draws by a lean form; every lean case's draws the lean forms' and the ringed and planetshine-lit bodies' the full forms' ${String(fullDrawn)}; the records lean as their cases ${String(recordsLean)}; ${String(swaps)} draws and dispatches by a twin`,
    );
  } finally {
    fullRenderer.dispose();
    leanRenderer.dispose();
    twinnedRenderer.dispose();
    for (const { by, kernel } of made) {
      by.releaseCompute(kernel);
    }
    for (const surface of surfaces) {
      engine.releaseTexture(surface.weights);
    }
    tables?.dispose();
  }
}
