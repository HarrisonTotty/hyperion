/**
 * The smoke page's lit-body checks (plan R07, T8.a): `shaders/bodyDisc.wgsl`'s two draws into an
 * `rgba16float` target the test makes, against `bodies/discShading.ts`' CPU rasteriser of the same
 * arithmetic: a disc's texels and meter classes; its summed flux against the point's at the 3 px
 * switch, over phases and sub-pixel placements; a Saturn-like f = 0.098 disc's extents at 100 px.
 */

import { add, scale, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { phaseFactorTableOf } from "../view/appearance/law";
import {
  compositeDiscPixels,
  type CompositePixel,
  rasteriseDisc,
  viewRay,
} from "../view/bodies/discShading";
import { LitBodyRenderer, type LitBodyInput, planLitBodies, pointFlux } from "../view/bodies/draw";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  pixelSolidAngle,
  type ProjectionCamera,
  type Viewport,
  viewRotation4,
} from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import type { BodyFigure } from "../view/terrain/planet";
import type { RenderEngine } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import type { PlacedLight } from "../view/lighting/hostLights";
import { METER_CLASS } from "../view/post/meter";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { createSceneTarget } from "../view/photoreal/sceneTarget";
import { AU_M } from "../view/scenes/kept";
import { WIREFRAME_MATERIALS, WIREFRAME_MESHES } from "../view/wireframe/submit";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import { SKY_SPRITE_HDR_MATERIAL } from "../view/sky/spriteHdr";
import { addCanvas } from "./frames";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  NEAR_M,
} from "./harness";

const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

/**
 * The flux check's camera: 32 px across 1.1°, the centre pixel's scale of 1080p across 60°
 * (1,663 px/rad), so that a 3 px body subtends what it does at the switch in a real view.
 */
const SWITCH_CAMERA: ProjectionCamera = {
  orientation: IDENTITY_QUATERNION,
  fovXRad: 2 * Math.atan(16 / 1663),
};
const EXPOSURE = 1e-4;
const RADIUS_M = 6.371e6;
const RAD_PER_DEG = Math.PI / 180;

/** The agreement of a texel with the rasteriser: `rgba16float`'s rounding and `f32`'s shading. */
const TEXEL_RELATIVE = 4e-3;
const TEXEL_ABSOLUTE = 1e-4;

/** One drawn disc: the GPU's texels and the CPU's composite. */
interface Drawn {
  readonly viewport: Viewport;
  readonly camera: ProjectionCamera;
  readonly texels: Float32Array;
  readonly expected: ReadonlyArray<CompositePixel>;
  readonly body: LitBodyInput;
  readonly hosts: ReadonlyArray<PlacedLight>;
}

/**
 * Draws one body `diameterPx` across, centred `offsetPx` from the view's centre, lit by a Sun 1 au
 * away at `phaseDeg`, into a target of `viewport`.
 */
async function drawDisc(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  viewport: Viewport,
  diameterPx: number,
  phaseDeg: number,
  figure: BodyFigure,
  offsetPx: readonly [number, number] = [0, 0],
  camera: ProjectionCamera = CAMERA,
  occluder: BodyFigure | null = null,
): Promise<Drawn> {
  const pxPerRad = viewport.widthPx / (2 * Math.tan(camera.fovXRad / 2));
  const distance = figure.equatorialRadiusM / Math.sin(diameterPx / 2 / pxPerRad);
  const centreM = vec3(
    (offsetPx[0] * distance) / pxPerRad,
    (-offsetPx[1] * distance) / pxPerRad,
    -distance,
  );
  const towards = vec3(Math.sin(phaseDeg * RAD_PER_DEG), 0, Math.cos(phaseDeg * RAD_PER_DEG));
  const hosts = [{ disc: sunLikeHostDisc(), centreM: add(centreM, scale(towards, AU_M)) }];
  const body: LitBodyInput = {
    id: "0200080020000000.0001",
    centreM,
    figure,
    photometry: PROVISIONAL_PHOTOMETRY,
  };
  const options = { camera, viewport, exposureScale: EXPOSURE, annuli: DISC_ANNULI_HIGH };
  // An occluder 3.844 × 10⁸ m towards the star and 3 × 10⁶ m aside, its shadow across the disc.
  const others: LitBodyInput[] =
    occluder === null
      ? []
      : [
          {
            ...body,
            id: "0200080020000000.0002",
            centreM: add(add(centreM, scale(towards, 3.844e8)), vec3(3e6, 0, 0)),
            figure: occluder,
          },
        ];
  const plan = planLitBodies([body, ...others], hosts, options, new Map([[body.id, "disc"]]));
  const target = createSceneTarget(engine, "smoke bodies", viewport);
  try {
    target.render({
      label: PHOTOREAL_PASS_LABELS.discs,
      viewRotation: viewRotation4(camera.orientation),
      projection: perspectiveReversedInfinite(
        camera.fovXRad,
        viewport.widthPx / viewport.heightPx,
        NEAR_PLANE_M,
      ),
      draws: renderer.draws(plan),
      postProcesses: [],
    });
    const texels = halfTexels(await engine.readTexture(target.colour));
    const record = plan.discs.find((each) => each.body === body.id);
    const expected =
      record === undefined
        ? []
        : compositeDiscPixels(
            rasteriseDisc(record, phaseFactorTableOf(record.law), camera, viewport),
          );
    return { viewport, camera, texels, expected, body, hosts };
  } finally {
    target.dispose();
  }
}

/** The disc's flux at the camera in the green channel, lx, from the read-back texels. */
function texelFlux(drawn: Drawn): number {
  let flux = 0;
  const { widthPx, heightPx } = drawn.viewport;
  for (let y = 0; y < heightPx; y += 1) {
    for (let x = 0; x < widthPx; x += 1) {
      const green = drawn.texels[(y * widthPx + x) * 4 + 1] ?? 0;
      if (green > 0) {
        const omega = pixelSolidAngle(
          viewRay(x + 0.5, y + 0.5, drawn.camera, drawn.viewport),
          drawn.camera,
          drawn.viewport,
        );
        flux += (green * omega) / EXPOSURE;
      }
    }
  }
  return flux;
}

const SPHERE: BodyFigure = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M, pole: null };

/** How the GPU's texels agree with the rasteriser's: the worst error over its tolerance, the classes. */
function texelAgreement(drawn: Drawn): {
  readonly worst: number;
  readonly classes: boolean;
  readonly seen: ReadonlySet<number>;
  readonly mismatches: ReadonlyArray<string>;
} {
  let worst = 0;
  let classes = true;
  const mismatches: string[] = [];
  const seen = new Set<number>();
  for (const pixel of drawn.expected) {
    const at = (pixel.yPx * drawn.viewport.widthPx + pixel.xPx) * 4;
    for (const c of [0, 1, 2] as const) {
      const gpu = drawn.texels[at + c] ?? Number.NaN;
      const error = Math.abs(gpu - pixel.rgb[c]) / (TEXEL_ABSOLUTE + TEXEL_RELATIVE * pixel.rgb[c]);
      worst = Math.max(worst, Number.isFinite(error) ? error : Number.POSITIVE_INFINITY);
    }
    const alpha = drawn.texels[at + 3] ?? Number.NaN;
    // The limb keeps the class beneath: the target's cleared `other`.
    const expectedClass = pixel.meterClass ?? METER_CLASS.other;
    if (alpha !== expectedClass) {
      classes = false;
      mismatches.push(
        `(${String(pixel.xPx)}, ${String(pixel.yPx)}) ${String(alpha)} for ${String(expectedClass)}`,
      );
    }
    seen.add(alpha);
  }
  return { worst, classes, seen, mismatches };
}

/** T8.a: the disc regime on the GPU. */
export async function checkBodies(engine: RenderEngine, checks: Checks): Promise<void> {
  const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
  try {
    await checkTexels(engine, renderer, checks);
    await checkFlux(engine, renderer, checks);
    await checkExtents(engine, renderer, checks);
  } finally {
    renderer.dispose();
  }
}

async function checkTexels(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  checks: Checks,
): Promise<void> {
  // 20 px across, every pixel sampled 8 × 8, at 80° so that the terminator crosses pixels.
  const drawn = await drawDisc(engine, renderer, { widthPx: 48, heightPx: 48 }, 20, 80, SPHERE);
  const { worst, classes, seen, mismatches } = texelAgreement(drawn);
  checks.check(
    "T8.a a disc's texels equal the CPU rasteriser's",
    worst <= 1 && drawn.expected.length > 0,
    `${String(drawn.expected.length)} pixels; worst error ${worst.toFixed(3)} of the tolerance (${String(TEXEL_RELATIVE)} relative + ${String(TEXEL_ABSOLUTE)})`,
  );
  const moon: BodyFigure = { equatorialRadiusM: 1.737e6, polarRadiusM: 1.737e6, pole: null };
  const shadowed = await drawDisc(
    engine,
    renderer,
    { widthPx: 48, heightPx: 48 },
    20,
    0,
    SPHERE,
    [0, 0],
    CAMERA,
    moon,
  );
  const eclipse = texelAgreement(shadowed);
  const free = await drawDisc(engine, renderer, { widthPx: 48, heightPx: 48 }, 20, 0, SPHERE);
  const darker = texelFlux(shadowed) / texelFlux(free);
  checks.check(
    "T8.a a disc in a moon's shadow equals the CPU rasteriser's",
    eclipse.worst <= 1 && darker < 0.99,
    `worst error ${eclipse.worst.toFixed(3)} of the tolerance; the shadow keeps ${(100 * darker).toFixed(2)}% of the light`,
  );
  checks.check(
    "T8.a lit, unlit and terminator pixels carry their meter classes",
    classes &&
      [METER_CLASS.litBody, METER_CLASS.unlitBody, METER_CLASS.other].every((c) => seen.has(c)),
    `classes seen ${[...seen].toSorted((x, y) => x - y).join(", ")}; each as the rasteriser's ${String(classes)}${mismatches.length > 0 ? `: ${mismatches.slice(0, 6).join("; ")}` : ""}`,
  );
}

async function checkFlux(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  checks: Checks,
): Promise<void> {
  let worst = 0;
  const cases: string[] = [];
  for (const phaseDeg of [0, 90, 150]) {
    for (const offset of [
      [0, 0],
      [0.25, 0.5],
      [0.5, 0.25],
      [0.75, 0.75],
    ] as const) {
      // The harness's checks run in order: each reads the GPU back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const drawn = await drawDisc(
        engine,
        renderer,
        { widthPx: 32, heightPx: 32 },
        3,
        phaseDeg,
        SPHERE,
        offset,
        SWITCH_CAMERA,
      );
      const point = pointFlux(drawn.body, drawn.hosts, [], DISC_ANNULI_HIGH)[1];
      const error = Math.abs(texelFlux(drawn) / point - 1);
      worst = Math.max(worst, error);
      const cpu =
        drawn.expected.reduce(
          (sum, p) =>
            sum +
            p.rgb[1] *
              pixelSolidAngle(
                viewRay(p.xPx + 0.5, p.yPx + 0.5, drawn.camera, drawn.viewport),
                drawn.camera,
                drawn.viewport,
              ),
          0,
        ) / EXPOSURE;
      cases.push(
        `${String(phaseDeg)}° ${(100 * error).toFixed(2)}% (CPU ${(100 * (cpu / point - 1)).toFixed(2)}%, texels ${texelAgreement(drawn).worst.toFixed(2)})`,
      );
    }
  }
  checks.check(
    "T8.a at the 3 px switch the disc's summed flux equals the point's to 1%",
    worst < 0.01,
    cases.join("; "),
  );
}

async function checkExtents(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  checks: Checks,
): Promise<void> {
  const viewport = { widthPx: 128, heightPx: 128 };
  const a = 6.0268e7;
  const c = a * (1 - 0.098);
  const figure: BodyFigure = { equatorialRadiusM: a, polarRadiusM: c, pole: vec3(0, 1, 0) };
  const drawn = await drawDisc(engine, renderer, viewport, 100, 0, figure);
  // The coverage along the centre row and column of the rasteriser the GPU's texels are checked
  // against.
  const sumAlong = (pick: (p: CompositePixel) => boolean): number =>
    drawn.expected.filter(pick).reduce((sum, p) => sum + p.coverage, 0);
  const across = sumAlong((p) => p.yPx === viewport.heightPx / 2);
  const down = sumAlong((p) => p.xPx === viewport.widthPx / 2);
  const s = 1 / Math.tan(CAMERA.fovXRad / 2);
  const pxPerRad = viewport.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
  const tanHalf = Math.tan(Math.asin(Math.sin(50 / pxPerRad)));
  const width = s * viewport.widthPx * tanHalf;
  const height = s * viewport.widthPx * (c / a) * tanHalf;
  const agreement = texelAgreement(drawn);
  checks.check(
    "T8.a a Saturn-like f = 0.098 disc's extents are drawn to half a pixel at 100 px",
    Math.abs(across - width) < 0.5 && Math.abs(down - height) < 0.5 && agreement.worst <= 1,
    `across ${across.toFixed(3)} against ${width.toFixed(3)} px; down ${down.toFixed(3)} against ${height.toFixed(3)} px; the GPU's texels within ${agreement.worst.toFixed(3)} of the tolerance`,
  );
}

/** The sprite materials whose row's z is the sprite's depth (decision-r07-t8a, item 2). */
const SPRITE_MATERIALS = [WIREFRAME_MATERIALS.starSprite, SKY_SPRITE_HDR_MATERIAL] as const;

/**
 * T8.a: a sprite's row carries its depth through both sprite materials: a star (depth 0) behind a
 * depth-writing plane 1 m ahead is hidden, a body point at 0.5 m in front of it is drawn.
 */
export async function checkSpriteDepth(engine: RenderEngine, checks: Checks): Promise<void> {
  const size = { widthPx: 32, heightPx: 32 };
  const plane = engine.createMaterial(
    flatSpec("smoke depth plane", { colourWrites: false, depthWrite: true }),
  );
  const planeMesh = fullScreenMesh(engine, "smoke depth plane mesh", -1);
  const quad = engine.createMesh({ ...WIREFRAME_MESHES.quad, name: "smoke sprite quad" });
  const sprites = engine.createBuffer({
    name: "smoke sprite depth",
    bytes: 64,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  const results: string[] = [];
  let pass = true;
  for (const spec of SPRITE_MATERIALS) {
    const material = engine.createMaterial(spec);
    for (const [depth, seen] of [
      [0, false],
      [NEAR_M / 0.5, true],
    ] as const) {
      engine.writeBuffer(sprites, 0, new Float32Array([16.5, 16.5, depth, 0, 1, 1, 1, 0]));
      const target = createSceneTarget(engine, "smoke sprite depth", size);
      try {
        target.render(
          frameOf(
            PHOTOREAL_PASS_LABELS.discs,
            [
              drawOf(planeMesh, plane, [0, 0, 0, 0]),
              {
                mesh: quad,
                material,
                offsetFromCameraM: new Float32Array(3),
                uniforms: {},
                textures: {},
                instanceCount: 1,
                storageBuffers: { sprites },
              },
            ],
            1,
          ),
        );
        // The harness reads its targets back in order: each before the next draws.
        // oxlint-disable-next-line no-await-in-loop
        const texels = halfTexels(await engine.readTexture(target.colour));
        const lit = (texels[(16 * size.widthPx + 16) * 4 + 1] ?? 0) > 0;
        pass &&= lit === seen;
        results.push(
          `${spec.displayName} at depth ${depth.toFixed(2)}: ${lit ? "drawn" : "hidden"}`,
        );
      } finally {
        target.dispose();
      }
    }
  }
  checks.check(
    "T8.a a sprite's row carries its depth through both sprite materials",
    pass,
    results.join("; "),
  );
}

/**
 * T8.a: a photorealistic frame end to end on a canvas: a planet 20 px across lit at 60° of phase
 * by a Sun 1 au away, through the scene target, bloom and the tone-mapping pass; its lit side is
 * bright, the sky black.
 */
export async function checkPhotorealFrame(engine: RenderEngine, checks: Checks): Promise<void> {
  const viewport = { widthPx: 64, heightPx: 36 };
  const view = engine.createView(addCanvas(), "smoke photoreal");
  view.resize(viewport);
  const renderer = new PhotorealRenderer(engine, "smoke photoreal");
  try {
    await renderer.prepare(viewport, "high", "eye", CAMERA);
    const pxPerRad = viewport.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2));
    const distance = RADIUS_M / Math.sin(10 / pxPerRad);
    const centreM = vec3(0, 0, -distance);
    const towards = vec3(Math.sin(60 * RAD_PER_DEG), 0, Math.cos(60 * RAD_PER_DEG));
    const plan = renderer.render(view, {
      camera: CAMERA,
      viewport,
      role: "eye",
      setting: "high",
      exposureScale: 1e-4,
      sky: [],
      starSprites: [],
      hostDraws: new Map(),
      glareSources: [],
      lights: [{ disc: sunLikeHostDisc(), centreM: add(centreM, scale(towards, AU_M)) }],
      bodies: [
        {
          id: "0200080020000000.0001",
          centreM,
          figure: SPHERE,
          photometry: PROVISIONAL_PHOTOMETRY,
        },
      ],
      previousRegimes: new Map(),
      overlay: null,
    });
    const bytes = await view.readBack();
    const at = (x: number, y: number): number => bytes[(y * viewport.widthPx + x) * 4 + 1] ?? 0;
    const litSide = at(36, 18);
    const sky = at(2, 2);
    checks.check(
      "T8.a a photorealistic frame tones a lit planet onto the canvas over a black sky",
      plan !== null && litSide > 64 && sky <= 2,
      `planned ${String(plan !== null)}; lit side ${String(litSide)}, sky ${String(sky)} (green codes)`,
    );
  } finally {
    renderer.dispose();
    view.dispose();
  }
}
