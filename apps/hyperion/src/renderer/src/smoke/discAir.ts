/**
 * The smoke page's check of R07's disc drawn by its reflectance table beyond the 10⁹ m boundary
 * (plan R08, R08.T16.b; `view/atmosphere/discReflectance.ts`, `discAir.wgsl`): the lit bodies'
 * kernels' air forms against their CPU twin on SwiftShader, for a small disc through the `disc
 * cells` pass's air form, a large disc of four latitude bands, read at each pixel's latitude
 * between them, and a body drawn in the mesh regime; and their forms for emission and air together
 * (R08.T18.b), a glowing magma ocean under the air whose table holds the air's own light, its blue
 * negative as a cool glow's is.
 *
 * @remarks
 * Each case is planned as the photorealistic frame plans it (`planLitBodies` with its body's disc
 * table, past 1.1 × 10⁹ m, the air forms prepared), drawn through the `disc cells` pass's air kernel
 * and the disc's, the limb's and the mesh regime's air materials, and held as R08.T18.a's emissive
 * forms are (`smoke/thermal.ts`) but for the kernel arm's floor (`KERNEL_FLOOR`): every wholly
 * covered pixel's sums the kernel leaves against the twin's (`discCellSums`) within
 * 10⁻³ · max(|twin|, 10⁻³ · largest), a pixel the limb covers in part
 * and the image's `rgba16float` pixels within 10⁻⁴ + 4 × 10⁻³ of the twin's (`rasteriseDisc`), and
 * every interior pixel's meter class equal. The tables are patterned (their values a test's, as
 * `test/thickTables.ts`' are), halves as the texture holds them, so that the read's every axis
 * moves. The four-band case binds a Saturn-class figure's body table (`tables.ts`'s
 * `packOblateBody`) with the airless defaults' zeros for the rest, through which its ground is lit
 * as airless but where the datum hides the star; its twin reads it so.
 */

import { add, scale, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import type { DiscAir } from "../view/atmosphere/discReflectance";
import {
  DISC_AZIMUTHS,
  DISC_LAYER_VALUES,
  DISC_SUN_COSINES,
  DISC_SUN_ROWS_BELOW,
  DISC_TABLE_HEIGHT,
  DISC_TABLE_WIDTH,
  DISC_VIEW_COSINES,
  type DiscReflectanceTable,
  discTableHalves,
  discTableLayers,
  discTableValuesOf,
  discTexelIndex,
} from "../view/atmosphere/discTable";
import { tableRadiusM } from "../view/atmosphere/hillaire";
import type { SurfaceTemperatureField } from "../view/atmosphere/thermal";
import { groundFieldOf, magmaOceanSurface } from "../test/thermalSurfaces";
import { gravityRatio } from "../view/atmosphere/oblate";
import {
  SATURN_OBLATE_GRAVITY,
  SATURN_OBLATE_REFERENCE,
  SATURN_OBLATE_SPHEROID,
} from "../view/atmosphere/reference/cases";
import {
  createAirlessTables,
  type SurfaceLightingReader,
  type SurfaceLightingTables,
} from "../view/atmosphere/surfaceLighting";
import { OBLATE_BODY_TEXELS, packOblateBody, tablesPlanOf } from "../view/atmosphere/tables";
import {
  compositeDiscPixels,
  discCellJobs,
  discCellSums,
  type DiscRecord,
  rasteriseDisc,
} from "../view/bodies/discShading";
import {
  BODY_DISC_CELLS_AIR_KERNEL,
  BODY_DISC_CELLS_EMISSIVE_AIR_KERNEL,
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
import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type {
  BufferHandle,
  ComputeHandle,
  RenderEngine,
  TextureHandle,
} from "../view/engine/types";
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
import { log } from "../lib/pinnedMath";
import { type Checks, halfTexels } from "./harness";

const RAD_PER_DEG = Math.PI / 180;

/** The cases' exposure: a disc 1 au from a Sun-like star, I/F of order 0.1, at a few units. */
const EXPOSURE = 1e-5;

/** The image's agreement with the rasteriser: `rgba16float`'s rounding and `f32`'s shading. */
const TEXEL_RELATIVE = 4e-3;
const TEXEL_ABSOLUTE = 1e-4;

/** The twin bound's arm for a kernel's own output (`decision-r08-t6b-floor.md`): 10⁻³. */
const KERNEL_TOLERANCE = 1e-3;

/**
 * Its floor, a share of the largest value compared: 10⁻³, so that a pixel under 10⁻³ of the
 * brightest is held to 10⁻⁶ of the brightest, 300 times under Design note 10's F · L_max.
 *
 * @remarks
 * R07's limb cells weight their points by the limb's local normal, a difference of `f32` limb
 * angles 1/64 px apart (`bodyDisc.wgsl`'s `cell_samples`), each weight within about 10⁻³ of the
 * twin's (10⁻² on the 4° view). The weights' sum holds, so a light smooth across a cell agrees to
 * 10⁻⁵, but where the air's light rises from 0 at the table's floor partway across one, the dark
 * pixel is held to a weight's own error; the plain kernels' 10⁻⁶ would test R07's normal there.
 */
const KERNEL_FLOOR = 1e-3;

/** Bytes of one pixel's sums in `cell_sums`: two `vec4f`. */
const CELL_SUM_BYTES = 32;

/** The id the cases draw. */
const BODY = "0200080020000000.0001";

/**
 * A table of `bands` bands and one slot whose texels follow a pattern in every axis, as halves, its
 * air's light falling to 0 at the floor's row as a bake's does (the sun there grazes the air's top).
 */
function patternedTable(bands: number, glowing = false): DiscReflectanceTable {
  const values = new Float32Array(bands * (glowing ? 2 : 1) * DISC_LAYER_VALUES);
  for (let b = 0; b < bands; b += 1) {
    for (let s = 0; s < DISC_SUN_COSINES; s += 1) {
      // A step at the floor would hold a limb cell across it to one point's weight, which R07's
      // kernel keeps to about 10⁻³ alone (`KERNEL_FLOOR`).
      const rising = Math.min(s / DISC_SUN_ROWS_BELOW, 1);
      for (let v = 0; v < DISC_VIEW_COSINES; v += 1) {
        for (let k = 0; k < DISC_AZIMUTHS; k += 1) {
          const at = discTexelIndex(b, s, v, k);
          for (let c = 0; c < 3; c += 1) {
            values[at + c] =
              rising *
              (0.03 + 0.01 * (1 + b) * (1 + Math.sin(0.31 * s + 0.73 * v + 0.47 * k + 0.9 * c)));
          }
          values[at + 3] = 0.45 + (0.5 * v) / (DISC_VIEW_COSINES - 1) - 0.02 * b;
        }
      }
    }
  }
  if (!glowing) {
    return { bands, slots: 1, sunFloor: -0.18, values: discTableValuesOf(values) };
  }
  // The air's own light (R08.T18.b): a thermal layer a band past the light slots', in a pattern by
  // view cosine, the same at every sun row and azimuth, as a bake holds it. Its blue is negative, as
  // a cool glow's is out of Rec. 709's gamut, so the case checks that the table's halves and the
  // target keep its sign: a format or an upload that clamped it would put the blue 0.4 of the glow
  // off the twin's.
  for (let b = 0; b < bands; b += 1) {
    for (let s = 0; s < DISC_SUN_COSINES; s += 1) {
      for (let v = 0; v < DISC_VIEW_COSINES; v += 1) {
        for (let k = 0; k < DISC_AZIMUTHS; k += 1) {
          const at = discTexelIndex(bands + b, s, v, k);
          for (let c = 0; c < 3; c += 1) {
            const glow = 0.5 + 0.3 * Math.sin(0.61 * v + 1.1 * c + b);
            values[at + c] = c === 2 ? -0.4 * glow : glow;
          }
        }
      }
    }
  }
  return {
    bands,
    slots: 1,
    sunFloor: -0.18,
    values: discTableValuesOf(values),
    thermalScale: GLOW_SCALE,
  };
}

/**
 * The glowing case's thermal layers' scale, π cd m⁻²: at the cases' exposure the air's own light
 * draws about as bright as the reflected (R08.T18.b).
 */
const GLOW_SCALE = 3e4;

/** One case: a body with air drawn in a view, its record's lighting as the twin reads it. */
interface AirCase {
  readonly name: string;
  readonly viewport: Viewport;
  readonly camera: ProjectionCamera;
  readonly figure: BodyFigure;
  readonly distanceM: number;
  /** The phase angle, deg. */
  readonly phaseDeg: number;
  readonly air: DiscAir;
  /** Its surface lighting's tables, or none (the airless defaults). */
  readonly lighting: SurfaceLightingTables | null;
  readonly reader: SurfaceLightingReader;
  /** Whether a craft's footprint over it promotes it to the mesh regime. */
  readonly mesh: boolean;
  /** Its ground's temperature field, where the ground glows under the air (R08.T18.b). */
  readonly thermal?: SurfaceTemperatureField;
}

/** Airless lighting as the twin reads it, the star hidden below the datum's horizon. */
const DATUM_SHADOW: SurfaceLightingReader = {
  sunTransmittance: (_h, mu) => (mu < 0 ? [0, 0, 0] : [1, 1, 1]),
  skyIrradiance: () => [0, 0, 0],
  sourceTransmittance: (_h, mu) => (mu < 0 ? [0, 0, 0] : [1, 1, 1]),
};

/** The airless lookups, R07's stubs, as the one-texel defaults give them. */
const AIRLESS: SurfaceLightingReader = {
  sunTransmittance: () => [1, 1, 1],
  skyIrradiance: () => [0, 0, 0],
  sourceTransmittance: () => [1, 1, 1],
};

/** A case's plan: its body a little off the view's centre, lit by a Sun 1 au away. */
function planOf(item: AirCase): BodyFramePlan {
  const { viewport, camera } = item;
  const pxPerRad = viewport.widthPx / (2 * Math.tan(camera.fovXRad / 2));
  const d = item.distanceM;
  const centreM = vec3((0.3 * d) / pxPerRad, (0.2 * d) / pxPerRad, -d);
  const towards = vec3(
    Math.sin(item.phaseDeg * RAD_PER_DEG),
    0,
    Math.cos(item.phaseDeg * RAD_PER_DEG),
  );
  const body: LitBodyInput = {
    id: BODY,
    centreM,
    figure: item.figure,
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
    air: item.air,
    ...(item.thermal === undefined ? {} : { thermal: item.thermal }),
    ...(item.lighting === null ? {} : { atmosphere: item.lighting }),
  };
  const centrePx = {
    xPx: viewport.widthPx / 2 + 0.3,
    yPx: viewport.heightPx / 2 - 0.2,
    radiusPx: 4,
  };
  return planLitBodies(
    [body],
    [{ disc: sunLikeHostDisc(), centreM: add(centreM, scale(towards, AU_M)) }],
    {
      camera,
      viewport,
      presentedPx: viewport,
      exposureScale: EXPOSURE,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high",
      drawsAir: true,
      ...(item.mesh ? { depthWriters: [centrePx] } : {}),
    },
    new Map([[BODY, "disc"] as const]),
  );
}

/** The renderer's name of the `disc cells` pass's sums (`draw.ts`' `LitBodyRenderer`). */
const CELL_SUMS_BUFFER = "bodies:disc cell sums";

/** The renderer's engine, keeping the sums buffer made readable (`smoke/thermal.ts`' alike). */
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

/** A case's frame for the `disc cells` pass and the passes. */
function frameOf(item: AirCase): {
  readonly viewRotation: Float32Array;
  readonly projection: Float32Array;
  readonly size: Viewport;
} {
  return {
    viewRotation: viewRotation4(item.camera.orientation),
    projection: perspectiveReversedInfinite(
      item.camera.fovXRad,
      item.viewport.widthPx / item.viewport.heightPx,
      NEAR_PLANE_M,
    ),
    size: item.viewport,
  };
}

/** A plan drawn as the photorealistic frame draws it, its image and the pass's sums read back. */
async function drawPlan(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  sums: () => BufferHandle | null,
  plan: BodyFramePlan,
  item: AirCase,
  kernels: CellKernels,
): Promise<{
  readonly image: Float32Array;
  readonly sums: Float32Array | null;
  readonly dispatched: boolean;
}> {
  const target = createSceneTarget(engine, "smoke disc air", item.viewport);
  try {
    const { viewRotation, projection, size } = frameOf(item);
    const dispatched = renderer.dispatchCells(
      plan,
      kernels.plain,
      { viewRotation, projection, size },
      kernels.emissive,
      kernels.air,
      kernels.emissiveAir,
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
  item: AirCase,
  image: Float32Array,
): { readonly worst: number; readonly classes: boolean; readonly pixels: number } {
  const expected = compositeDiscPixels(
    rasteriseDisc(record, item.camera, item.viewport, null, item.reader),
  );
  let worst = 0;
  let classes = true;
  for (const pixel of expected) {
    const at = (pixel.yPx * item.viewport.widthPx + pixel.xPx) * 4;
    for (const c of [0, 1, 2] as const) {
      const want = pixel.rgb[c];
      const got = image[at + c] ?? Number.NaN;
      const error = Math.abs(got - want) / (TEXEL_ABSOLUTE + TEXEL_RELATIVE * Math.abs(want));
      worst = Math.max(worst, Number.isFinite(error) ? error : Number.POSITIVE_INFINITY);
    }
    classes &&= (image[at + 3] ?? Number.NaN) === (pixel.meterClass ?? METER_CLASS.other);
  }
  return { worst, classes, pixels: expected.length };
}

/** One arm of the pass's sums against the twin's: its worst error over its bound, and where. */
interface SumsArm {
  worst: number;
  pixels: number;
  where: string;
}

/** The pass's sums against the twin's (the module's remarks), wholly covered pixels and the limb's. */
function sumsAgreement(
  plan: BodyFramePlan,
  item: AirCase,
  sums: Float32Array,
  engine: RenderEngine,
): { readonly whole: SumsArm; readonly limb: SumsArm } {
  const fits = Math.floor(engine.capabilities.maxStorageBufferBindingSize / CELL_SUM_BYTES);
  const cells = discCellJobs(plan.discs, fits, frameOf(item), true, true, true);
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
      const [sum] = discCellSums(
        record,
        item.camera,
        item.viewport,
        [[x, y, n]],
        null,
        item.reader,
      );
      if (sum !== undefined) {
        twins.push({ job, rgb: sum.radiance, coverage: sum.coverage });
      }
    }
  }
  const largest = Math.max(0, ...twins.flatMap(({ rgb }) => rgb.map(Math.abs)));
  const whole: SumsArm = { worst: 0, pixels: 0, where: "none" };
  const limb: SumsArm = { worst: 0, pixels: 0, where: "none" };
  for (const { job, rgb, coverage } of twins) {
    const at = 8 * job;
    const isWhole = coverage >= 1;
    const arm = isWhole ? whole : limb;
    arm.pixels += 1;
    const pairs: ReadonlyArray<readonly [number, number, number]> = [
      [sums[at] ?? Number.NaN, rgb[0], largest],
      [sums[at + 1] ?? Number.NaN, rgb[1], largest],
      [sums[at + 2] ?? Number.NaN, rgb[2], largest],
      [sums[at + 3] ?? Number.NaN, coverage, 1],
    ];
    pairs.forEach(([got, want, scaleOf], c) => {
      const bound = isWhole
        ? KERNEL_TOLERANCE * Math.max(Math.abs(want), KERNEL_FLOOR * scaleOf)
        : TEXEL_ABSOLUTE + TEXEL_RELATIVE * Math.abs(want);
      const error = Math.abs(got - want) / bound;
      if (!(error <= arm.worst)) {
        arm.worst = Number.isFinite(error) ? error : Number.POSITIVE_INFINITY;
        arm.where = `(${String(cells.jobs[4 * job + 1])}, ${String(cells.jobs[4 * job + 2])}) channel ${String(c)}, ${got.toPrecision(6)} against ${want.toPrecision(6)}`;
      }
    });
  }
  return { whole, limb };
}

/** The Saturn-class figure's body table as the kernels read it, uploaded (`packOblateBody`). */
function saturnBodyTable(engine: RenderEngine): TextureHandle {
  const plan = tablesPlanOf(
    { figure: SATURN_OBLATE_SPHEROID, gravity: SATURN_OBLATE_GRAVITY },
    tableRadiusM(SATURN_OBLATE_SPHEROID),
  );
  const texels = packOblateBody(
    SATURN_OBLATE_SPHEROID,
    SATURN_OBLATE_GRAVITY,
    plan,
    tableRadiusM(SATURN_OBLATE_SPHEROID),
    SATURN_OBLATE_REFERENCE,
    1,
  );
  const texture = engine.createTexture({
    name: "smoke disc air: Saturn-class body table",
    size: { width: OBLATE_BODY_TEXELS, height: 1 },
    dimension: "2d",
    format: "rgba32float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(texture, [0, 0, 0], [OBLATE_BODY_TEXELS, 1, 1], texels);
  return texture;
}

/** A sphere of radius `radiusM`, its pole up the image. */
function sphere(radiusM: number): BodyFigure {
  return { equatorialRadiusM: radiusM, polarRadiusM: radiusM, pole: vec3(0, 1, 0) };
}

/** The check's cases (the module's remarks). */
function casesOf(saturnLighting: SurfaceLightingTables): ReadonlyArray<AirCase> {
  const oneBand: DiscAir = {
    table: patternedTable(1),
    texture: { kind: "texture", name: "pending" },
    lnBands: [0],
    gravity: null,
  };
  const wide: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
  const narrow: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: 4 * RAD_PER_DEG };
  const saturnPlan = tablesPlanOf(
    { figure: SATURN_OBLATE_SPHEROID, gravity: SATURN_OBLATE_GRAVITY },
    tableRadiusM(SATURN_OBLATE_SPHEROID),
  );
  return [
    {
      name: "a 13 px body's air at quadrature, its cells through the air form",
      viewport: { widthPx: 48, heightPx: 48 },
      camera: wide,
      figure: sphere(2e9 * Math.sin(6.5 / (48 / (2 * Math.tan(Math.PI / 6))))),
      distanceM: 2e9,
      phaseDeg: 90,
      air: oneBand,
      lighting: null,
      reader: AIRLESS,
      mesh: false,
    },
    {
      name: "a 160 px Saturn-class figure's air of four bands at 40°, read at each pixel's latitude",
      viewport: { widthPx: 192, heightPx: 192 },
      camera: narrow,
      figure: {
        equatorialRadiusM: SATURN_OBLATE_SPHEROID.equatorialRadiusM,
        polarRadiusM: SATURN_OBLATE_SPHEROID.polarRadiusM,
        pole: vec3(0, 1, 0),
      },
      distanceM:
        SATURN_OBLATE_SPHEROID.equatorialRadiusM /
        Math.sin(80 / (192 / (2 * Math.tan(2 * RAD_PER_DEG)))),
      phaseDeg: 40,
      air: {
        table: patternedTable(saturnPlan.bands.length),
        texture: { kind: "texture", name: "pending" },
        lnBands: saturnPlan.bands.map((band) => log(band.gravityRatio)),
        gravity: SATURN_OBLATE_GRAVITY,
      },
      lighting: saturnLighting,
      reader: DATUM_SHADOW,
      mesh: false,
    },
    {
      name: "a 96 px body's air at 60° in the mesh regime",
      viewport: { widthPx: 128, heightPx: 112 },
      camera: wide,
      figure: sphere(2e9 * Math.sin(48 / (128 / (2 * Math.tan(Math.PI / 6))))),
      distanceM: 2e9,
      phaseDeg: 60,
      air: oneBand,
      lighting: null,
      reader: AIRLESS,
      mesh: true,
    },
    {
      name: "a 96 px glowing body's air at 120°, its ground's emission and the air's own light, its blue negative",
      viewport: { widthPx: 128, heightPx: 112 },
      camera: wide,
      figure: sphere(2e9 * Math.sin(48 / (128 / (2 * Math.tan(Math.PI / 6))))),
      distanceM: 2e9,
      phaseDeg: 120,
      air: { ...oneBand, table: patternedTable(1, true) },
      lighting: null,
      reader: AIRLESS,
      mesh: false,
      thermal: groundFieldOf(magmaOceanSurface()),
    },
  ];
}

/** The `disc cells` kernel's forms the cases are drawn by. */
interface CellKernels {
  readonly plain: ComputeHandle;
  readonly emissive: ComputeHandle;
  readonly air: ComputeHandle;
  readonly emissiveAir: ComputeHandle;
}

/** A case's table uploaded as `AtmosphereTables.setDiscReflectance` uploads it. */
function uploaded(engine: RenderEngine, air: DiscAir): DiscAir {
  const layers = discTableLayers(air.table);
  const texture = engine.createTexture({
    name: "smoke disc air: table",
    size: [DISC_TABLE_WIDTH, DISC_TABLE_HEIGHT, layers],
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(
    texture,
    [0, 0, 0],
    [DISC_TABLE_WIDTH, DISC_TABLE_HEIGHT, layers],
    discTableHalves(air.table),
  );
  return { ...air, texture };
}

/**
 * Checks that the lit bodies' air forms draw what their twin draws, the `disc cells` kernel's own
 * output to the kernel arm's 10⁻³ and the image to the disc's texel bound (the module's remarks).
 */
export async function checkDiscAir(engine: RenderEngine, checks: Checks): Promise<void> {
  const kept = keepingSums(engine);
  const renderer = new LitBodyRenderer(kept.engine, WIREFRAME_MATERIALS.starSprite);
  const airless = createAirlessTables(engine, "smoke disc air: no air");
  const bodyTable = saturnBodyTable(engine);
  const made: TextureHandle[] = [airless.body, bodyTable];
  try {
    const [plain, emissive, air, emissiveAir] = await Promise.all([
      engine.createComputeAsync(BODY_DISC_CELLS_KERNEL),
      engine.createComputeAsync(BODY_DISC_CELLS_EMISSIVE_KERNEL),
      engine.createComputeAsync(BODY_DISC_CELLS_AIR_KERNEL),
      engine.createComputeAsync(BODY_DISC_CELLS_EMISSIVE_AIR_KERNEL),
      renderer.prepareAir(),
      renderer.prepareEmissive(),
      renderer.prepareEmissiveAir(),
    ]);
    for (const given of casesOf({ ...airless, body: bodyTable })) {
      const item = { ...given, air: uploaded(engine, given.air) };
      made.push(item.air.texture);
      const plan = planOf(item);
      const record = plan.discs[0];
      const regime = plan.regimes.get(BODY);
      if (
        record === undefined ||
        record.air === undefined ||
        (item.thermal !== undefined) !== (record.emission !== undefined)
      ) {
        checks.check(
          `${item.thermal === undefined ? "R08.T16.b" : "R08.T18.b"} ${item.name}`,
          false,
          "the body was not drawn by its table, or not with its ground's emission",
        );
        continue;
      }
      // The harness reads its targets back in order.
      // oxlint-disable-next-line no-await-in-loop
      const drawn = await drawPlan(engine, renderer, kept.sums, plan, item, {
        plain,
        emissive,
        air,
        emissiveAir,
      });
      const image = imageAgreement(record, item, drawn.image);
      const sums = drawn.sums === null ? null : sumsAgreement(plan, item, drawn.sums, engine);
      const s = (latitudeRad: number): number =>
        item.air.gravity === null ? 1 : gravityRatio(item.air.gravity, latitudeRad);
      const ok =
        plan.tabled.has(BODY) &&
        regime === (item.mesh ? "mesh" : "disc") &&
        drawn.dispatched &&
        sums !== null &&
        sums.whole.pixels + sums.limb.pixels > 0 &&
        sums.whole.worst <= 1 &&
        sums.limb.worst <= 1 &&
        image.pixels > 0 &&
        image.worst <= 1 &&
        image.classes;
      checks.check(
        `${item.thermal === undefined ? "R08.T16.b" : "R08.T18.b"} ${item.name}`,
        ok,
        `${String(regime)}, s ${s(0).toFixed(4)} to ${s(Math.PI / 2).toFixed(4)}; the air disc cells' sums of its ${String(sums?.whole.pixels ?? 0)} wholly covered pixels within ${(sums?.whole.worst ?? Number.POSITIVE_INFINITY).toFixed(3)} of 10⁻³ of the twin's (worst at ${sums?.whole.where ?? "none"}), of its ${String(sums?.limb.pixels ?? 0)} on the limb within ${(sums?.limb.worst ?? Number.POSITIVE_INFINITY).toFixed(3)} of the image's tolerance; its ${String(image.pixels)} pixels within ${image.worst.toFixed(3)} of the image's tolerance, its classes ${String(image.classes)}`,
      );
    }
  } finally {
    renderer.dispose();
    for (const texture of made) {
      engine.releaseTexture(texture);
    }
  }
}
