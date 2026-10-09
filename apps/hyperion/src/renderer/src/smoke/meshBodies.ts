/**
 * The smoke page's mesh-regime checks (plan R07, T9): `shaders/smoothMesh.wgsl`'s figures, with
 * depth, and the disc's limb draw on the limb's plane, into a scene target, against
 * `bodies/frameTwin.ts`' CPU twin of the same passes. A promoted disc and its mesh (an Earth 20 px
 * across at 80° of phase, a Saturn-like giant 64 px across, its pole tilted, and an Earth from
 * 400 km, its horizon in view) draw the same texels, classes and flux; a small promoted body draws
 * the same texels through the `disc cells` pass as in-fragment (R07.T8.d's G11). Over the scripted
 * occultation (`view/scenes/occultationScene.ts`), every step equals the twin and the painter's
 * frame, and the moon is hidden where the first-hit oracle says. The captures draw the
 * occultation through the photorealistic frame onto a canvas, as meshes and as discs, for the
 * owner's check by eye.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { add, normalise, scale, type Vec3, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { viewRay } from "../view/bodies/discShading";
import {
  BODY_DISC_CELLS_KERNEL,
  type BodyFramePlan,
  type BodyFrameOptions,
  type LitBodyInput,
  LitBodyRenderer,
  planLitBodies,
} from "../view/bodies/draw";
import { compositeBodyFrame, type FramePixel, firstHitShares } from "../view/bodies/frameTwin";
import { type LitRegime, sphereFootprint } from "../view/bodies/regime";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  pixelSolidAngle,
  type ProjectionCamera,
  type Viewport,
  viewRotation4,
} from "../view/camera/projection";
import { IDENTITY_QUATERNION, lookAlong } from "../view/camera/quaternion";
import type { ComputeHandle, RenderEngine } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import { createSceneTarget } from "../view/photoreal/sceneTarget";
import { METER_CLASS } from "../view/post/meter";
import { AU_M } from "../view/scenes/kept";
import {
  OCCULTATION_CAMERA,
  OCCULTATION_MOON,
  OCCULTATION_STEPS,
  OCCULTATION_VIEWPORT,
  occultationFrame,
} from "../view/scenes/occultationScene";
import type { BodyFigure } from "../view/terrain/planet";
import { WIREFRAME_MATERIALS } from "../view/wireframe/submit";
import { base64Of, type CapturedImage } from "./atmosphere";
import { addCanvas } from "./frames";
import { type Checks, halfTexels, halfUlpsApart } from "./harness";

const EXPOSURE = 1e-4;
const RAD = Math.PI / 180;

/** The agreement of a texel with the twin: `rgba16float`'s rounding and `f32`'s shading (T8.a's). */
const TEXEL_RELATIVE = 4e-3;
const TEXEL_ABSOLUTE = 1e-4;

/**
 * A plan drawn into a scene target, read back as its `rgba16float` bytes, and whether the `disc
 * cells` pass ran for it (dispatched with `cells`, R07.T8.d).
 */
async function drawPlanBytes(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  plan: BodyFramePlan,
  camera: ProjectionCamera,
  viewport: Viewport,
  cells: ComputeHandle | null,
): Promise<{ readonly bytes: ArrayBuffer; readonly throughCells: boolean }> {
  const target = createSceneTarget(engine, "smoke mesh bodies", viewport);
  try {
    const viewRotation = viewRotation4(camera.orientation);
    const projection = perspectiveReversedInfinite(
      camera.fovXRad,
      viewport.widthPx / viewport.heightPx,
      NEAR_PLANE_M,
    );
    // The small discs' cells, shaded at once, before the figures and limbs that read them (T8.d).
    const throughCells =
      cells !== null &&
      renderer.dispatchCells(plan, cells, { viewRotation, projection, size: viewport });
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

/** A plan drawn into a scene target, each disc summing its own cells, read back. */
async function drawPlan(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  plan: BodyFramePlan,
  camera: ProjectionCamera,
  viewport: Viewport,
): Promise<Float32Array> {
  const { bytes } = await drawPlanBytes(engine, renderer, plan, camera, viewport, null);
  return halfTexels(bytes);
}

/** Plans drawn and read back one after another, each before the next draws. */
async function drawInTurn(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  plans: readonly [BodyFramePlan, BodyFramePlan],
  camera: ProjectionCamera,
  viewport: Viewport,
): Promise<readonly [Float32Array, Float32Array]> {
  const first = await drawPlan(engine, renderer, plans[0], camera, viewport);
  const second = await drawPlan(engine, renderer, plans[1], camera, viewport);
  return [first, second];
}

/** How the GPU's texels agree with the twin's frame: the worst error over its tolerance, the classes. */
function agreement(
  texels: Float32Array,
  expected: ReadonlyMap<number, FramePixel>,
  viewport: Viewport,
): { readonly worst: number; readonly mismatches: ReadonlyArray<string> } {
  let worst = 0;
  const mismatches: string[] = [];
  for (let index = 0; index < viewport.widthPx * viewport.heightPx; index += 1) {
    const pixel = expected.get(index);
    for (const c of [0, 1, 2] as const) {
      const want = pixel?.rgb[c] ?? 0;
      const gpu = texels[index * 4 + c] ?? Number.NaN;
      const error = Math.abs(gpu - want) / (TEXEL_ABSOLUTE + TEXEL_RELATIVE * want);
      worst = Math.max(worst, Number.isFinite(error) ? error : Number.POSITIVE_INFINITY);
    }
    const alpha = texels[index * 4 + 3];
    const wanted = pixel?.meterClass ?? METER_CLASS.other;
    if (alpha !== wanted && mismatches.length < 6) {
      mismatches.push(
        `(${String(index % viewport.widthPx)}, ${String(Math.floor(index / viewport.widthPx))}) class ${String(alpha)} for ${String(wanted)}`,
      );
    }
  }
  return { worst, mismatches };
}

/**
 * How two read-backs differ: the worst difference of a colour over the texel tolerance, and whether
 * every class is equal.
 */
function sameTexels(
  a: Float32Array,
  b: Float32Array,
): { readonly worst: number; readonly classes: boolean } {
  let worst = 0;
  let classes = a.length === b.length;
  for (let i = 0; i < a.length; i += 1) {
    const x = a[i] ?? Number.NaN;
    const y = b[i] ?? Number.NaN;
    if (i % 4 === 3) {
      classes &&= x === y;
      continue;
    }
    const error = Math.abs(x - y) / (TEXEL_ABSOLUTE + TEXEL_RELATIVE * Math.abs(y));
    worst = Math.max(worst, Number.isFinite(error) ? error : Number.POSITIVE_INFINITY);
  }
  return { worst, classes };
}

/** The texels' flux in the green channel, lx. */
function texelFlux(texels: Float32Array, camera: ProjectionCamera, viewport: Viewport): number {
  let flux = 0;
  for (let y = 0; y < viewport.heightPx; y += 1) {
    for (let x = 0; x < viewport.widthPx; x += 1) {
      const green = texels[(y * viewport.widthPx + x) * 4 + 1] ?? 0;
      if (green > 0) {
        flux +=
          (green * pixelSolidAngle(viewRay(x + 0.5, y + 0.5, camera, viewport), camera, viewport)) /
          EXPOSURE;
      }
    }
  }
  return flux;
}

/** A body `diameterPx` across, a little off the view's centre, straight ahead of the camera. */
function ahead(
  figure: BodyFigure,
  diameterPx: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): Vec3 {
  const pxPerRad = viewport.widthPx / (2 * Math.tan(camera.fovXRad / 2));
  const distance = figure.equatorialRadiusM / Math.sin(diameterPx / 2 / pxPerRad);
  return vec3((0.13 * distance) / pxPerRad, (-0.41 * distance) / pxPerRad, -distance);
}

/**
 * One body at `centreM`, lit by a Sun 1 au away along `towards` from it, as a mesh over a depth
 * writer or as a disc.
 */
function planOfOne(
  figure: BodyFigure,
  centreM: Vec3,
  towards: Vec3,
  asMesh: boolean,
  camera: ProjectionCamera,
  viewport: Viewport,
): BodyFramePlan {
  const body: LitBodyInput = {
    id: "0200080020000000.0001",
    centreM,
    figure,
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
  };
  const writer = sphereFootprint(centreM, figure.equatorialRadiusM, camera, viewport);
  const options: BodyFrameOptions = {
    camera,
    viewport,
    exposureScale: EXPOSURE,
    annuli: DISC_ANNULI_HIGH,
    planetshine: PLANETSHINE_SOURCES_HIGH,
    setting: "high" as const,
    depthWriters: asMesh && writer !== null ? [writer] : [],
  };
  return planLitBodies(
    [body],
    [{ disc: sunLikeHostDisc(), centreM: add(centreM, scale(normalise(towards), AU_M)) }],
    options,
    new Map([[body.id, "disc"]]),
  );
}

/** T9: the mesh regime on the GPU. */
export async function checkMeshBodies(engine: RenderEngine, checks: Checks): Promise<void> {
  const renderer = new LitBodyRenderer(engine, WIREFRAME_MATERIALS.starSprite);
  try {
    await checkPromoted(engine, renderer, checks);
    await checkPromotedCells(engine, renderer, checks);
    await checkOccultation(engine, renderer, checks);
  } finally {
    renderer.dispose();
  }
}

/**
 * R07.T8.d's G11 for the mesh regime: an Earth promoted to a mesh at 20 px (4 × 4) and at 3.6 px
 * (8 × 8), lit at 80°, its figure and limb drawn through the `disc cells` pass and in-fragment in
 * the same run, agree within one `rgba16float` ulp in every channel, with the classes identical.
 */
async function checkPromotedCells(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  checks: Checks,
): Promise<void> {
  const kernel = await engine.createComputeAsync(BODY_DISC_CELLS_KERNEL);
  const camera: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
  const viewport: Viewport = { widthPx: 96, heightPx: 80 };
  const earth: BodyFigure = { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6, pole: null };
  const towards = vec3(Math.sin(80 * RAD), 0, Math.cos(80 * RAD));
  let pass = true;
  const results: string[] = [];
  for (const diameterPx of [20, 3.6]) {
    const centreM = ahead(earth, diameterPx, camera, viewport);
    const plan = (): BodyFramePlan => planOfOne(earth, centreM, towards, true, camera, viewport);
    const parallelPlan = plan();
    // The harness reads its targets back in order: through the pass, then in-fragment.
    // oxlint-disable-next-line no-await-in-loop
    const parallel = await drawPlanBytes(engine, renderer, parallelPlan, camera, viewport, kernel);
    // The in-fragment draw reads back after the pass's, before the next case draws.
    // oxlint-disable-next-line no-await-in-loop
    const inFragment = await drawPlanBytes(engine, renderer, plan(), camera, viewport, null);
    const {
      ulps,
      differing,
      alphasEqual: classes,
    } = halfUlpsApart(parallel.bytes, inFragment.bytes);
    const meshes = parallelPlan.meshes.length;
    const patches = parallelPlan.meshes[0]?.mesh.patches.length ?? 0;
    const cells = parallelPlan.discs[0]?.interiorSamples ?? 0;
    pass &&=
      meshes === 1 &&
      patches > 0 &&
      cells === (diameterPx < 4 ? 8 : 4) &&
      parallel.throughCells &&
      !inFragment.throughCells &&
      ulps <= 1 &&
      classes;
    results.push(
      `${String(diameterPx)} px as ${String(meshes)} mesh of ${String(patches)} patches at ${String(cells)} × ${String(cells)}: through the pass ${String(parallel.throughCells)}; within ${String(ulps)} ulp of the in-fragment draw (${String(differing)} channels differ); classes equal ${String(classes)}`,
    );
  }
  checks.check(
    "R07.T8.d G11 a small body promoted to a mesh draws through the disc cells pass its in-fragment texels within one rgba16float ulp",
    pass,
    results.join("; "),
  );
}

async function checkPromoted(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  checks: Checks,
): Promise<void> {
  const straight: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
  // From 400 km up, pitched 15° down: the horizon, 2,290 km off, 4.8° below the view's centre, and
  // the upper corners seeing the limb's plane behind the camera.
  const lowOrbit: ProjectionCamera = {
    orientation: lookAlong(vec3(0, -Math.sin(15 * RAD), -Math.cos(15 * RAD)), vec3(0, 1, 0)),
    fovXRad: Math.PI / 3,
  };
  const viewport: Viewport = { widthPx: 96, heightPx: 80 };
  const earth: BodyFigure = { equatorialRadiusM: 6.371e6, polarRadiusM: 6.371e6, pole: null };
  const saturn: BodyFigure = {
    equatorialRadiusM: 6.0268e7,
    polarRadiusM: 6.0268e7 * (1 - 0.098),
    pole: normalise(vec3(0, 1, 0.4)),
  };
  const atPhase = (deg: number): Vec3 => vec3(Math.sin(deg * RAD), 0, Math.cos(deg * RAD));
  const cases = [
    {
      name: "Earth 20 px at 80°",
      camera: straight,
      figure: earth,
      centreM: ahead(earth, 20, straight, viewport),
      towards: atPhase(80),
    },
    {
      name: "Saturn-like 64 px at 60°",
      camera: straight,
      figure: saturn,
      centreM: ahead(saturn, 64, straight, viewport),
      towards: atPhase(60),
    },
    {
      name: "Earth from 400 km, its horizon in view",
      camera: lowOrbit,
      figure: earth,
      centreM: vec3(0, -(6.371e6 + 4e5), 0),
      towards: vec3(0.2, 0.9, -0.4),
    },
  ];
  const results: string[] = [];
  let pass = true;
  for (const { name, camera, figure, centreM, towards } of cases) {
    const mesh = planOfOne(figure, centreM, towards, true, camera, viewport);
    const disc = planOfOne(figure, centreM, towards, false, camera, viewport);
    // The harness reads its targets back in order: the mesh's, then the disc's.
    // oxlint-disable-next-line no-await-in-loop
    const [meshTexels, discTexels] = await drawInTurn(
      engine,
      renderer,
      [mesh, disc],
      camera,
      viewport,
    );
    const twin = compositeBodyFrame(mesh, camera, viewport);
    // The twin's colours; its classes are f64's, which f32 may flip at the terminator's
    // threshold (`LIT_IRRADIANCE`, T8.a), so the classes are held to the disc's own on the GPU.
    // A pixel both of the disc's draws take (a corner within `LIMB_OVERLAP_PX` of the limb) keeps
    // the mesh's opaque light: the limb, on its plane, lies behind the figure's surface there.
    const { worst, mismatches } = agreement(meshTexels, twin.pixels, viewport);
    const asDisc = sameTexels(meshTexels, discTexels);
    const ratio = texelFlux(meshTexels, camera, viewport) / texelFlux(discTexels, camera, viewport);
    const ok =
      mesh.meshes.length === 1 &&
      twin.holes === 0 &&
      worst <= 1 &&
      asDisc.worst <= 1 &&
      asDisc.classes &&
      Math.abs(ratio - 1) < 0.01;
    pass &&= ok;
    results.push(
      `${name}: ${String(mesh.meshes[0]?.mesh.patches.length ?? 0)} patches; texels within ${worst.toFixed(3)} of the tolerance of the twin and ${asDisc.worst.toFixed(3)} of the disc's, classes the disc's ${String(asDisc.classes)}; flux ${(100 * (ratio - 1)).toFixed(4)}% from the disc's${mismatches.length > 0 ? `; against the twin's f64 classes ${mismatches.join("; ")}` : ""}`,
    );
  }
  checks.check(
    "T9 a promoted disc's mesh draws the disc's texels and classes, its flux to 1%, and the twin's colours",
    pass,
    results.join(". "),
  );
}

async function checkOccultation(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  checks: Checks,
): Promise<void> {
  const camera = OCCULTATION_CAMERA;
  const viewport = OCCULTATION_VIEWPORT;
  const meshes = occultationPlans(true);
  const discs = occultationPlans(false);
  let worst = 0;
  let worstAsDiscs = 0;
  let classes = true;
  let misplaced = 0;
  const switched = meshes.findIndex((plan) => plan.regimes.get(OCCULTATION_MOON) === "mesh");
  const mismatches: string[] = [];
  for (let step = 0; step < OCCULTATION_STEPS; step += 1) {
    const frame = occultationFrame(step);
    const plan = meshes[step];
    const asDiscs = discs[step];
    if (plan === undefined || asDiscs === undefined) {
      throw new Error(`the script has no step ${String(step)}`);
    }
    // Each step is drawn and read back, as meshes and then as discs, before the next.
    // oxlint-disable-next-line no-await-in-loop
    const [texels, discTexels] = await drawInTurn(
      engine,
      renderer,
      [plan, asDiscs],
      camera,
      viewport,
    );
    // The twin's colours; the classes are held to the painter's on the GPU, as the promoted
    // disc's are, since f32 may flip the twin's f64 class at a terminator's threshold.
    const twin = compositeBodyFrame(plan, camera, viewport);
    const one = agreement(texels, twin.pixels, viewport);
    worst = Math.max(worst, one.worst);
    mismatches.push(...one.mismatches.map((m) => `step ${String(step)} ${m}`));
    const same = sameTexels(texels, discTexels);
    worstAsDiscs = Math.max(worstAsDiscs, same.worst);
    classes &&= same.classes;
    // The oracle over the moon's pixels: where its rays all meet the moon, or none do, the twin
    // the GPU equals gives the moon all or nothing.
    const region = [...twin.pixels.entries()]
      .filter(([, pixel]) => (pixel.shares.get(OCCULTATION_MOON) ?? 0) > 0)
      .map(([index]) => index);
    // The script's bodies are spheres, so any pole serves.
    const bodies = frame.bodies.map((body) => ({
      id: body.id,
      centreM: body.centreM,
      figure: body.figure,
      pole: vec3(0, 0, 1),
    }));
    const oracle = firstHitShares(bodies, camera, viewport, 4, region);
    for (const index of region) {
      const truth = oracle.get(index)?.get(OCCULTATION_MOON) ?? 0;
      const drawn = twin.pixels.get(index)?.shares.get(OCCULTATION_MOON) ?? 0;
      if (truth === 0 && drawn > 0.5) {
        misplaced += 1;
      }
    }
  }
  checks.check(
    "T9 a moon passing behind a planet's limb as meshes draws the twin and the painter's frames, hidden where the oracle says",
    worst <= 1 && worstAsDiscs <= 1 && classes && misplaced === 0 && switched > 0,
    `${String(OCCULTATION_STEPS)} steps, the moon promoted at step ${String(switched)}; texels within ${worst.toFixed(3)} of the tolerance of the twin and ${worstAsDiscs.toFixed(3)} of the painter's, classes the painter's ${String(classes)}; ${String(misplaced)} pixels more than half the moon's where no oracle ray meets it${mismatches.length > 0 ? `; against the twin's f64 classes ${mismatches.slice(0, 6).join("; ")}` : ""}`,
  );
}

/** The script's plans, as meshes over the planet's depth writer or as discs. */
function occultationPlans(promoted: boolean): BodyFramePlan[] {
  const plans: BodyFramePlan[] = [];
  let previous: ReadonlyMap<BodyIdHex, LitRegime> = new Map();
  for (let step = 0; step < OCCULTATION_STEPS; step += 1) {
    const frame = occultationFrame(step, promoted);
    const plan = planLitBodies(
      frame.bodies,
      frame.lights,
      {
        camera: OCCULTATION_CAMERA,
        viewport: OCCULTATION_VIEWPORT,
        exposureScale: EXPOSURE,
        annuli: DISC_ANNULI_HIGH,
        planetshine: PLANETSHINE_SOURCES_HIGH,
        depthWriters: frame.depthWriters,
        setting: "high",
      },
      previous,
    );
    plans.push(plan);
    previous = plan.regimes;
  }
  return plans;
}

/** The captures' view: the script's field, four times the pixels. */
const CAPTURE_VIEWPORT: Viewport = { widthPx: 512, heightPx: 384 };

/** The steps captured. */
const CAPTURE_STEPS = [0, 3, 6, 9, 12, 15, 17, 19, 21, 23] as const;

/**
 * T9's captures, for the owner's check by eye: the scripted occultation through the
 * photorealistic frame onto a 512 × 384 canvas, tone-mapped, its planet a mesh over a depth
 * writer and, in a second series, both bodies discs; the two series should not differ.
 */
export async function captureOccultation(engine: RenderEngine): Promise<CapturedImage[]> {
  const view = engine.createView(addCanvas(), "smoke occultation");
  view.resize(CAPTURE_VIEWPORT);
  const renderer = new PhotorealRenderer(engine, "smoke occultation");
  const images: CapturedImage[] = [];
  try {
    await renderer.prepare(CAPTURE_VIEWPORT, "high", "eye", OCCULTATION_CAMERA);
    for (const promoted of [true, false]) {
      let previous: ReadonlyMap<BodyIdHex, LitRegime> = new Map();
      for (const step of CAPTURE_STEPS) {
        const frame = occultationFrame(step, promoted, CAPTURE_VIEWPORT);
        const plan = renderer.render(view, {
          camera: OCCULTATION_CAMERA,
          viewport: CAPTURE_VIEWPORT,
          role: "eye",
          setting: "high",
          exposureScale: 5e-5,
          sky: [],
          starSprites: [],
          hostDraws: new Map(),
          glareSources: [],
          lights: frame.lights,
          bodies: frame.bodies,
          depthWriters: frame.depthWriters,
          previousRegimes: previous,
          overlay: null,
          meter: null,
        });
        previous = plan?.regimes ?? previous;
        // The canvas is read back in the task that drew it, each step before the next.
        // oxlint-disable-next-line no-await-in-loop
        const bytes = await view.readBack();
        images.push({
          name: `r07-t9-occultation-${promoted ? "mesh" : "disc"}-${String(step).padStart(2, "0")}`,
          width: CAPTURE_VIEWPORT.widthPx,
          height: CAPTURE_VIEWPORT.heightPx,
          rgba: base64Of(bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes)),
        });
      }
    }
  } finally {
    renderer.dispose();
    view.dispose();
  }
  return images;
}
