/**
 * The smoke page's check of the ground's thermal emission on R07's disc (plan R08, R08.T18.a;
 * `view/atmosphere/discReflectance.ts`): the lit bodies' kernels' emissive forms against their CPU
 * twin on SwiftShader, for an airless magma ocean, whose night side its own light alone draws at the
 * solidus and above, for the protocol fixture's locked lava world on its dayside, and for that world's
 * night side under a two-class map whose second class is `Melt`, which alone glows there, at the
 * melt's least temperature.
 *
 * @remarks
 * Each case is planned as the photorealistic frame plans it (`planLitBodies`, its body given its
 * record's ground temperature field), drawn through the `disc cells` pass's emissive kernel and the
 * disc's, the limb's and the mesh regime's emissive materials, and held two ways:
 * - **the kernel's own output**: every pixel's sums the emissive `disc cells` kernel leaves in
 *   `cell_sums`, read back as `f32`, against the twin's (`discCellSums`): a wholly covered pixel's
 *   light and covered share within 10⁻³ · max(|twin|, 10⁻⁶ · largest), the plan's twin-bound arm
 *   for a kernel's own output (`decision-r08-t6b-floor.md`); a pixel the limb covers in part within
 *   the image's bound below, which R07's `f32` limb geometry is held to (the limb's last pixels'
 *   covered share, a sliver of 10⁻⁵ of a pixel, differs by a few per cent of itself between `f32`
 *   and `f64` whatever the light: 7.5% of 1.7 × 10⁻⁴ on the 160 px disc, measured);
 * - **the image**: the scene target's `rgba16float` pixels against the twin's composite
 *   (`rasteriseDisc`), each channel within 10⁻⁴ + 4 × 10⁻³ of it, as R07's disc checks hold it
 *   (`smoke/bodies.ts`), and every interior pixel's meter class equal.
 *
 * Each case also says how much of its light is the emission's: the twin's image without its
 * record's emission (the plain kernels'), whose night side is black.
 */

import { add, scale, vec3 } from "../geometry/vec3";
import { groundFieldOf, lavaWorldSurface, magmaOceanSurface } from "../test/thermalSurfaces";
import type { ClassMapDiscSurface } from "../view/appearance/bodyAppearance";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { lawFor } from "../view/appearance/phase";
import type { SurfaceTemperatureField } from "../view/atmosphere/thermal";
import {
  compositeDiscPixels,
  discCellJobs,
  discCellSums,
  type DiscRecord,
  rasteriseDisc,
} from "../view/bodies/discShading";
import { type ClassMapTexels, classMapSurface, packClassMap } from "../view/bodies/discSurface";
import {
  BODY_DISC_CELLS_EMISSIVE_KERNEL,
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
import type { Rgb } from "../view/photometry/toneCurve";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { createSceneTarget } from "../view/photoreal/sceneTarget";
import { METER_CLASS } from "../view/post/meter";
import { AU_M } from "../view/scenes/kept";
import type { BodyFigure } from "../view/terrain/planet";
import { WIREFRAME_MATERIALS } from "../view/wireframe/submit";
import { type Checks, halfTexels } from "./harness";

const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
const RAD_PER_DEG = Math.PI / 180;

/**
 * The cases' exposure: the magma ocean's solidus glows at a few thousand cd m⁻², and the lava
 * world's dayside at 0.048 au reflects some 10⁶, so that every pixel lies within a few units.
 */
const EXPOSURE = 1e-6;

/** The lava world's radius, m (the protocol fixture's 899 km). */
const RADIUS_M = 8.99e5;

const FIGURE: BodyFigure = {
  equatorialRadiusM: RADIUS_M,
  polarRadiusM: RADIUS_M,
  pole: vec3(0, 1, 0),
};

/** The image's agreement with the rasteriser: `rgba16float`'s rounding and `f32`'s shading. */
const TEXEL_RELATIVE = 4e-3;
const TEXEL_ABSOLUTE = 1e-4;

/** The twin bound's arm for a kernel's own output (`decision-r08-t6b-floor.md`): 10⁻³. */
const KERNEL_TOLERANCE = 1e-3;

/** Its floor, a share of the largest value compared: 10⁻⁶. */
const KERNEL_FLOOR = 1e-6;

/** Bytes of one pixel's sums in `cell_sums`: two `vec4f`. */
const CELL_SUM_BYTES = 32;

/** A case's class map, whose second class is `Melt`, and the map's texels for the twin. */
interface MeltMap {
  readonly surface: ClassMapDiscSurface;
  readonly texels: ClassMapTexels;
}

/** One case: a body drawn in a view. */
interface ThermalCase {
  readonly name: string;
  readonly viewport: Viewport;
  readonly field: SurfaceTemperatureField;
  /** Its class map, under which its `Melt` class emits at the melt's least temperature; or none. */
  readonly mapped: MeltMap | null;
  readonly diameterPx: number;
  /** Its distance from its star, au. */
  readonly distanceAu: number;
  /** The phase angle, deg: 0 its dayside face-on, 180 its night side. */
  readonly phaseDeg: number;
  /** Whether it is drawn in the mesh regime, as a body the view draws with depth is. */
  readonly mesh: boolean;
}

/** A case's plan: its body a little off the view's centre and the pixel grid, lit by a Sun. */
function planOf(item: ThermalCase): BodyFramePlan {
  const { viewport } = item;
  const pxPerRad = viewport.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
  const distanceM = RADIUS_M / Math.sin(item.diameterPx / 2 / pxPerRad);
  const centreM = vec3((0.3 * distanceM) / pxPerRad, (0.2 * distanceM) / pxPerRad, -distanceM);
  const towards = vec3(
    Math.sin(item.phaseDeg * RAD_PER_DEG),
    0,
    Math.cos(item.phaseDeg * RAD_PER_DEG),
  );
  const body: LitBodyInput = {
    id: "0200080020000000.0001",
    centreM,
    figure: FIGURE,
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
    thermal: item.field,
    ...(item.mapped === null
      ? {}
      : {
          surface: item.mapped.surface,
          rotation: rotation3FromRows([vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)]),
        }),
  };
  return planLitBodies(
    [body],
    [{ disc: sunLikeHostDisc(), centreM: add(centreM, scale(towards, item.distanceAu * AU_M)) }],
    {
      camera: CAMERA,
      viewport,
      presentedPx: viewport,
      exposureScale: EXPOSURE,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high",
      ...(item.mesh ? { depthBodyShellRadiiM: new Map([[body.id, RADIUS_M] as const]) } : {}),
    },
    new Map([[body.id, "disc"] as const]),
  );
}

/** The renderer's name of the `disc cells` pass's sums (`draw.ts`' `LitBodyRenderer`). */
const CELL_SUMS_BUFFER = "bodies:disc cell sums";

/**
 * The renderer's engine, keeping the sums buffer each `disc cells` dispatch binds, made readable
 * (`COPY_SRC`), which the renderer's own is not, so that the kernel's own output is read back.
 */
function keepingSums(engine: RenderEngine): {
  readonly engine: LitBodyEngine;
  readonly sums: () => BufferHandle | null;
} {
  let sums: BufferHandle | null = null;
  const kept: LitBodyEngine = {
    createMaterial: (...args) => engine.createMaterial(...args),
    createMesh: (...args) => engine.createMesh(...args),
    createBuffer: (spec) =>
      engine.createBuffer(
        spec.name === CELL_SUMS_BUFFER
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
    onRestored: (...args) => engine.onRestored(...args),
    get capabilities() {
      return engine.capabilities;
    },
    prepareMaterial: (...args) => engine.prepareMaterial(...args),
  };
  return { engine: kept, sums: () => sums };
}

/** What a case's draw left: the image, the pass's sums and its jobs. */
interface Drawn {
  readonly image: Float32Array;
  readonly sums: Float32Array | null;
  readonly dispatched: boolean;
}

/** A plan drawn as the photorealistic frame draws it, its image and the pass's sums read back. */
async function drawPlan(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  sums: () => BufferHandle | null,
  plan: BodyFramePlan,
  viewport: Viewport,
  kernels: { readonly plain: ComputeHandle; readonly emissive: ComputeHandle },
): Promise<Drawn> {
  const target = createSceneTarget(engine, "smoke thermal emission", viewport);
  try {
    const viewRotation = viewRotation4(CAMERA.orientation);
    const projection = perspectiveReversedInfinite(
      CAMERA.fovXRad,
      viewport.widthPx / viewport.heightPx,
      NEAR_PLANE_M,
    );
    const dispatched = renderer.dispatchCells(
      plan,
      kernels.plain,
      { viewRotation, projection, size: viewport },
      kernels.emissive,
    );
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
    const image = halfTexels(await engine.readTexture(target.colour));
    const buffer = sums();
    const read = dispatched && buffer !== null ? await engine.readBuffer(buffer) : null;
    return { image, sums: read === null ? null : new Float32Array(read), dispatched };
  } finally {
    target.dispose();
  }
}

/** The worst image error over its tolerance against the twin's composite, and the classes' match. */
function imageAgreement(
  record: DiscRecord,
  viewport: Viewport,
  image: Float32Array,
  texels: ClassMapTexels | null,
): { readonly worst: number; readonly classes: boolean; readonly pixels: number } {
  const expected = compositeDiscPixels(rasteriseDisc(record, CAMERA, viewport, texels));
  let worst = 0;
  let classes = true;
  for (const pixel of expected) {
    const at = (pixel.yPx * viewport.widthPx + pixel.xPx) * 4;
    for (const c of [0, 1, 2] as const) {
      const want = pixel.rgb[c];
      const got = image[at + c] ?? Number.NaN;
      const error = Math.abs(got - want) / (TEXEL_ABSOLUTE + TEXEL_RELATIVE * want);
      worst = Math.max(worst, Number.isFinite(error) ? error : Number.POSITIVE_INFINITY);
    }
    classes &&= (image[at + 3] ?? Number.NaN) === (pixel.meterClass ?? METER_CLASS.other);
  }
  return { worst, classes, pixels: expected.length };
}

/** One arm of the pass's sums against the twin's: its worst error over its bound, and where. */
interface SumsArm {
  readonly worst: number;
  readonly pixels: number;
  readonly where: string;
}

/**
 * The pass's sums against the twin's (the module's remarks): each wholly covered pixel's light and
 * covered share at the kernel arm, 10⁻³ · max(|twin|, 10⁻⁶ · largest); each pixel the limb covers
 * in part at the image's bound, 10⁻⁴ + 4 × 10⁻³ of the twin's, which R07's f32 limb geometry is
 * held to (its covered share's sliver at the limb's last pixels, some 10⁻⁵ of a pixel, differs
 * by a few per cent of itself between `f32` and `f64` whatever the light); and the largest light
 * among them.
 */
function sumsAgreement(
  plan: BodyFramePlan,
  viewport: Viewport,
  sums: Float32Array,
  engine: RenderEngine,
  texels: ClassMapTexels | null,
): { readonly whole: SumsArm; readonly limb: SumsArm; readonly largest: number } {
  const fits = Math.floor(engine.capabilities.maxStorageBufferBindingSize / CELL_SUM_BYTES);
  const frame = {
    viewRotation: viewRotation4(CAMERA.orientation),
    projection: perspectiveReversedInfinite(
      CAMERA.fovXRad,
      viewport.widthPx / viewport.heightPx,
      NEAR_PLANE_M,
    ),
    size: viewport,
  };
  const cells = discCellJobs(plan.discs, fits, frame, true);
  const twins: Array<{ readonly job: number; readonly rgb: Rgb; readonly coverage: number }> = [];
  for (let g = 0; g < cells.groups.length; g += 4) {
    const first = cells.groups[g] ?? 0;
    const count = cells.groups[g + 1] ?? 0;
    const n = cells.groups[g + 2] ?? 0;
    for (let job = first; job < first + count; job += 1) {
      const record = plan.discs[cells.jobs[4 * job] ?? -1];
      if (record === undefined) {
        throw new Error(`the pass's job ${String(job)} has no record`);
      }
      const x = cells.jobs[4 * job + 1] ?? 0;
      const y = cells.jobs[4 * job + 2] ?? 0;
      const [sum] = discCellSums(record, CAMERA, viewport, [[x, y, n]], texels);
      if (sum !== undefined) {
        twins.push({ job, rgb: sum.radiance, coverage: sum.coverage });
      }
    }
  }
  const largest = Math.max(0, ...twins.flatMap(({ rgb }) => rgb.map(Math.abs)));
  const arms = {
    whole: { worst: 0, pixels: 0, where: "none" },
    limb: { worst: 0, pixels: 0, where: "none" },
  };
  for (const { job, rgb, coverage } of twins) {
    const at = 8 * job;
    const whole = coverage >= 1;
    const arm = whole ? arms.whole : arms.limb;
    arm.pixels += 1;
    const pairs: ReadonlyArray<readonly [number, number, number]> = [
      [sums[at] ?? Number.NaN, rgb[0], largest],
      [sums[at + 1] ?? Number.NaN, rgb[1], largest],
      [sums[at + 2] ?? Number.NaN, rgb[2], largest],
      [sums[at + 3] ?? Number.NaN, coverage, 1],
    ];
    pairs.forEach(([got, want, scaleOf], c) => {
      const bound = whole
        ? KERNEL_TOLERANCE * Math.max(Math.abs(want), KERNEL_FLOOR * scaleOf)
        : TEXEL_ABSOLUTE + TEXEL_RELATIVE * Math.abs(want);
      const error = Math.abs(got - want) / bound;
      if (!(error <= arm.worst)) {
        arm.worst = Number.isFinite(error) ? error : Number.POSITIVE_INFINITY;
        arm.where = `(${String(cells.jobs[4 * job + 1])}, ${String(cells.jobs[4 * job + 2])}) channel ${String(c)}`;
      }
    });
  }
  return { ...arms, largest };
}

/** The share of a record's composite light, summed over its pixels, that is its emission's. */
function emittedShare(
  record: DiscRecord,
  viewport: Viewport,
  texels: ClassMapTexels | null,
): number {
  const total = (each: DiscRecord): number =>
    compositeDiscPixels(rasteriseDisc(each, CAMERA, viewport, texels)).reduce(
      (sum, pixel) => sum + pixel.rgb[0] + pixel.rgb[1] + pixel.rgb[2],
      0,
    );
  const { emission: _emission, ...plain } = record;
  const all = total(record);
  return all > 0 ? 1 - total(plain) / all : 0;
}

/** The check's cases (the module's remarks). */
function casesOf(melt: MeltMap): ReadonlyArray<ThermalCase> {
  const ocean = groundFieldOf(magmaOceanSurface());
  const lava = groundFieldOf(lavaWorldSurface());
  return [
    {
      name: "a 13 px locked lava world's night side under a half-surveyed two-class map, its second class Melt",
      viewport: { widthPx: 48, heightPx: 48 },
      field: lava,
      mapped: melt,
      diameterPx: 13,
      distanceAu: 0.048,
      phaseDeg: 180,
      mesh: false,
    },
    {
      name: "a 13 px magma ocean's night side",
      viewport: { widthPx: 48, heightPx: 48 },
      field: ocean,
      mapped: null,
      diameterPx: 13,
      distanceAu: 0.048,
      phaseDeg: 180,
      mesh: false,
    },
    {
      name: "a 13 px locked lava world's dayside, its pool towards the camera",
      viewport: { widthPx: 48, heightPx: 48 },
      field: lava,
      mapped: null,
      diameterPx: 13,
      distanceAu: 0.048,
      phaseDeg: 30,
      mesh: false,
    },
    {
      name: "a 160 px magma ocean at quadrature",
      viewport: { widthPx: 192, heightPx: 192 },
      field: ocean,
      mapped: null,
      diameterPx: 160,
      distanceAu: 0.048,
      phaseDeg: 90,
      mesh: false,
    },
    {
      name: "a 96 px magma ocean at quadrature in the mesh regime",
      viewport: { widthPx: 128, heightPx: 112 },
      field: ocean,
      mapped: null,
      diameterPx: 96,
      distanceAu: 0.048,
      phaseDeg: 90,
      mesh: true,
    },
  ];
}

/**
 * Checks that the lit bodies' emissive kernels draw what their twin draws, the `disc cells`
 * kernel's own output to the kernel arm's 10⁻³ and the image to the disc's texel bound (the
 * module's remarks).
 */
export async function checkThermalDiscs(engine: RenderEngine, checks: Checks): Promise<void> {
  const kept = keepingSums(engine);
  const renderer = new LitBodyRenderer(kept.engine, WIREFRAME_MATERIALS.starSprite);
  // Half the cells surveyed (i + j even): class 0 where i and j are even, a quarter class 0 and
  // three quarters class 1, `Melt`, where both are odd (`smoke/cellPass.ts`' map).
  const twoClass = packClassMap(4, 2, ({ i, j }) =>
    (i + j) % 2 !== 0 ? null : i % 2 === 0 ? [1, 0] : [0.25, 0.75],
  );
  const melt: MeltMap = {
    surface: {
      ...classMapSurface(
        engine,
        "smoke thermal class map",
        twoClass,
        [
          lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
          lawFor([0.05, 0.05, 0.05], [0.6, 0.6, 0.6], "moon"),
        ],
        PROVISIONAL_PHOTOMETRY.law,
      ),
      meltClasses: [false, true],
    },
    texels: twoClass,
  };
  try {
    const [plain, emissive] = await Promise.all([
      engine.createComputeAsync(BODY_DISC_CELLS_KERNEL),
      engine.createComputeAsync(BODY_DISC_CELLS_EMISSIVE_KERNEL),
      renderer.prepareEmissive(),
    ]);
    for (const item of casesOf(melt)) {
      const plan = planOf(item);
      const record = plan.discs[0];
      const regime = plan.regimes.get("0200080020000000.0001");
      if (record === undefined) {
        checks.check(`R08.T18.a ${item.name}`, false, "the body was not drawn larger than a point");
        continue;
      }
      // The harness reads its targets back in order.
      // oxlint-disable-next-line no-await-in-loop
      const drawn = await drawPlan(engine, renderer, kept.sums, plan, item.viewport, {
        plain,
        emissive,
      });
      const texels = item.mapped?.texels ?? null;
      const image = imageAgreement(record, item.viewport, drawn.image, texels);
      const sums =
        drawn.sums === null ? null : sumsAgreement(plan, item.viewport, drawn.sums, engine, texels);
      const share = emittedShare(record, item.viewport, texels);
      const ok =
        record.emission !== undefined &&
        regime === (item.mesh ? "mesh" : "disc") &&
        drawn.dispatched &&
        sums !== null &&
        sums.whole.pixels > 0 &&
        sums.whole.worst <= 1 &&
        sums.limb.worst <= 1 &&
        image.pixels > 0 &&
        image.worst <= 1 &&
        image.classes &&
        share > 0;
      checks.check(
        `R08.T18.a ${item.name}`,
        ok,
        `${String(regime)}, its emission ${(100 * share).toPrecision(3)}% of its light; the emissive disc cells' sums of its ${String(sums?.whole.pixels ?? 0)} wholly covered pixels within ${(sums?.whole.worst ?? Number.POSITIVE_INFINITY).toFixed(3)} of 10⁻³ of the twin's (worst at ${sums?.whole.where ?? "none"}), of its ${String(sums?.limb.pixels ?? 0)} on the limb within ${(sums?.limb.worst ?? Number.POSITIVE_INFINITY).toFixed(3)} of the image's tolerance (largest light ${(sums?.largest ?? 0).toPrecision(4)}); its ${String(image.pixels)} pixels within ${image.worst.toFixed(3)} of the image's tolerance, its classes ${String(image.classes)}`,
      );
    }
  } finally {
    renderer.dispose();
    engine.releaseTexture(melt.surface.weights);
  }
}
