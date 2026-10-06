/**
 * The smoke page's eclipse-scene checks and captures (plan R07, T10.c), on the kept scene
 * (`view/scenes/eclipseScene.ts`) as the view's photorealistic frame assembles it
 * (`photorealFrame`), with R06's host disc drawn at its place in the painter's order.
 *
 * @remarks
 * The checks: the moon's shadow on the planet, drawn into a scene target, equals the CPU twin of
 * the same passes (`compositeBodyFrame`) in texels and classes; the giant behind the star leaves
 * no texel of its own, the frame equal to the same frame without it, as a disc and, promoted by a
 * depth writer, as a mesh under the star's disc on its limb's plane (R06's follow-up to R07.T9);
 * the moon in front of the star
 * covers exactly the star's texels that its wholly covered pixels take on the CPU. The captures,
 * for the owner's check by eye: the shadow crossing the planet under the camera, the moon crossing
 * the star from inside its penumbra (a partial eclipse, then the total), and the giant passing
 * behind the star.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { photorealFrame } from "../displays/view/photorealFrame";
import { startRun } from "../displays/view/viewRun";
import { cross, dot, norm, normalise } from "../geometry/vec3";
import { rasteriseDisc, viewRay } from "../view/bodies/discShading";
import { type BodyFramePlan, LitBodyRenderer, planLitBodies } from "../view/bodies/draw";
import { compositeBodyFrame, type FramePixel } from "../view/bodies/frameTwin";
import { type LitRegime, type ScreenCircle, sphereFootprint } from "../view/bodies/regime";
import type { CameraPose } from "../view/camera/pose";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type Viewport,
  viewRotation4,
} from "../view/camera/projection";
import { rotate } from "../view/camera/quaternion";
import type { DrawItem, RenderEngine } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import { PHOTOREAL_PASS_LABELS } from "../view/photoreal/passes";
import { type PhotorealFrame, PhotorealRenderer } from "../view/photoreal/renderer";
import { createSceneTarget } from "../view/photoreal/sceneTarget";
import { exposureScale } from "../view/photometry/exposure";
import { METER_CLASS } from "../view/post/meter";
import {
  ECLIPSE_CAMERA_POSE,
  ECLIPSE_CONJUNCTION_S,
  ECLIPSE_GIANT,
  ECLIPSE_MID_S,
  ECLIPSE_MOON,
  ECLIPSE_TIME_RATE,
  eclipsePoseAtStar,
  eclipseScene,
  eclipseSceneAt,
} from "../view/scenes/eclipseScene";
import { HostDiscLayer } from "../view/sky/disc";
import { SKY_SPRITE_HDR_MATERIAL } from "../view/sky/spriteHdr";
import type { WireframeDrawList } from "../view/wireframe/drawList";
import { base64Of, type CapturedImage } from "./atmosphere";
import { addCanvas } from "./frames";
import { type Checks, halfTexels } from "./harness";

/** The checks' pre-exposure: the planet's day side near 1, never clamped. */
const EXPOSURE = 1e-4;

/** The agreement of a texel with the twin: `rgba16float`'s rounding and `f32`'s shading (T8.a's). */
const TEXEL_RELATIVE = 4e-3;
const TEXEL_ABSOLUTE = 1e-4;

/** No wireframe draw list: the stars are not under test. */
const NO_LIST: WireframeDrawList = {
  occluderSpheres: [],
  occluderMeshes: [],
  lines: [],
  sprites: [],
  anchors: [],
};

/**
 * The scene's photorealistic frame at scene time `sceneS` from `pose` across `fovDeg`, pre-exposed
 * by `preExposure` (`exposureScale`).
 */
function eclipseFrame(
  discs: HostDiscLayer,
  sceneS: number,
  pose: CameraPose,
  fovDeg: number,
  viewport: Viewport,
  preExposure: number,
): PhotorealFrame {
  const start = startRun(eclipseScene());
  return photorealFrame({
    run: { ...start, scene: eclipseSceneAt(sceneS), camera: { ...start.camera, pose, fovDeg } },
    pose,
    viewport,
    setting: "high",
    exposureScale: preExposure,
    list: NO_LIST,
    sky: null,
    band: null,
    discs,
    cube: null,
    previousRegimes: new Map(),
    overlay: null,
    meter: null,
  });
}

/**
 * A frame's plan as the photorealistic renderer makes it on the high setting, with `depthWriters`
 * standing for geometry that writes depth (R07.T9's synthetic writer).
 */
function planOf(
  frame: PhotorealFrame,
  bodies = frame.bodies,
  depthWriters: ReadonlyArray<ScreenCircle> = [],
): BodyFramePlan {
  return planLitBodies(
    bodies,
    frame.lights,
    {
      camera: frame.camera,
      viewport: frame.viewport,
      exposureScale: frame.exposureScale,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      depthWriters,
      setting: "high",
    },
    new Map(),
  );
}

/**
 * A plan drawn into a scene target, read back: its mesh bodies' figures with depth, where it has
 * any, then its painter's sequence with `hostDraws` at its hosts.
 */
async function drawPlan(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  plan: BodyFramePlan,
  frame: PhotorealFrame,
  hostDraws: ReadonlyMap<number, ReadonlyArray<DrawItem>>,
): Promise<Float32Array> {
  const target = createSceneTarget(engine, "smoke eclipse", frame.viewport);
  try {
    const viewRotation = viewRotation4(frame.camera.orientation);
    const projection = perspectiveReversedInfinite(
      frame.camera.fovXRad,
      frame.viewport.widthPx / frame.viewport.heightPx,
      NEAR_PLANE_M,
    );
    const meshes = renderer.meshDraws(plan);
    if (meshes.length > 0) {
      target.render({
        label: PHOTOREAL_PASS_LABELS.bodies,
        viewRotation,
        projection,
        draws: meshes,
        postProcesses: [],
      });
    }
    target.render({
      label: PHOTOREAL_PASS_LABELS.discs,
      viewRotation,
      projection,
      draws: renderer.draws(plan, hostDraws),
      postProcesses: [],
      ...(meshes.length > 0 ? { colourLoad: "load" as const } : {}),
    });
    return halfTexels(await engine.readTexture(target.colour));
  } finally {
    target.dispose();
  }
}

/** How the GPU's texels agree with the twin's: the worst error over its tolerance, the classes. */
function agreement(
  texels: Float32Array,
  expected: ReadonlyMap<number, FramePixel>,
  viewport: Viewport,
): { readonly worst: number; readonly classMismatches: number } {
  let worst = 0;
  let classMismatches = 0;
  for (let index = 0; index < viewport.widthPx * viewport.heightPx; index += 1) {
    const pixel = expected.get(index);
    for (const c of [0, 1, 2] as const) {
      const want = pixel?.rgb[c] ?? 0;
      const gpu = texels[index * 4 + c] ?? Number.NaN;
      const error = Math.abs(gpu - want) / (TEXEL_ABSOLUTE + TEXEL_RELATIVE * want);
      worst = Math.max(worst, Number.isFinite(error) ? error : Number.POSITIVE_INFINITY);
    }
    if (texels[index * 4 + 3] !== (pixel?.meterClass ?? METER_CLASS.other)) {
      classMismatches += 1;
    }
  }
  return { worst, classMismatches };
}

/** Whether a point of the view falls inside the star's disc as R06's pass draws it: sin θ < sin ρ. */
function inStarDisc(frame: PhotorealFrame, xPx: number, yPx: number): boolean {
  const light = frame.lights[0];
  if (light === undefined) {
    throw new Error("the eclipse frame has no light");
  }
  const axis = normalise(light.centreM);
  const sinRho = light.disc.radius_m / norm(light.centreM);
  const ray = rotate(frame.camera.orientation, viewRay(xPx, yPx, frame.camera, frame.viewport));
  return dot(ray, axis) > 0 && norm(cross(ray, axis)) < sinRho;
}

/** T10.c: the eclipse scene on the GPU. */
export async function checkEclipse(engine: RenderEngine, checks: Checks): Promise<void> {
  const renderer = new LitBodyRenderer(engine, SKY_SPRITE_HDR_MATERIAL);
  const discs = new HostDiscLayer(engine);
  try {
    await checkShadow(engine, renderer, discs, checks);
    await checkBehind(engine, renderer, discs, checks);
    await checkInFront(engine, renderer, discs, checks);
  } finally {
    renderer.dispose();
  }
}

async function checkShadow(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  discs: HostDiscLayer,
  checks: Checks,
): Promise<void> {
  const cases = [
    // 45° across 240 px: the penumbra's edge on the disc, the umbra about 2 px across.
    { name: "entering, 45°", sceneS: ECLIPSE_MID_S - 4_000, fovDeg: 45, widthPx: 240 },
    // 10° across 160 px: 22 km a pixel, the umbra about 7 px across.
    { name: "central, 10°", sceneS: ECLIPSE_MID_S, fovDeg: 10, widthPx: 160 },
  ];
  const results: string[] = [];
  let pass = true;
  for (const { name, sceneS, fovDeg, widthPx } of cases) {
    const viewport: Viewport = { widthPx, heightPx: (widthPx * 9) / 16 };
    const frame = eclipseFrame(discs, sceneS, ECLIPSE_CAMERA_POSE, fovDeg, viewport, EXPOSURE);
    const plan = planOf(frame);
    // The harness reads each target back before the next draws.
    // oxlint-disable-next-line no-await-in-loop
    const texels = await drawPlan(engine, renderer, plan, frame, new Map());
    const twin = compositeBodyFrame(plan, frame.camera, viewport);
    const one = agreement(texels, twin.pixels, viewport);
    let umbra = 0;
    for (const pixel of twin.pixels.values()) {
      umbra += pixel.meterClass === METER_CLASS.unlitBody ? 1 : 0;
    }
    // A pixel at the umbra's edge, where the starlight's share crosses `LIT_IRRADIANCE`, may take
    // the other class in f32; at most a few.
    const ok = one.worst <= 1 && one.classMismatches <= 4 && umbra > 0;
    pass &&= ok;
    results.push(
      `${name}: texels within ${one.worst.toFixed(3)} of the tolerance of the twin, ${String(one.classMismatches)} classes not the twin's, ${String(umbra)} umbral pixels`,
    );
  }
  checks.check(
    "T10.c the moon's shadow on the planet draws the twin's texels and classes",
    pass,
    results.join("; "),
  );
}

async function checkBehind(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  discs: HostDiscLayer,
  checks: Checks,
): Promise<void> {
  // 2° across 320 px: the star 85 px across, the giant 8 px, a disc, wholly behind it.
  const viewport: Viewport = { widthPx: 320, heightPx: 180 };
  const sceneS = ECLIPSE_CONJUNCTION_S;
  const frame = eclipseFrame(discs, sceneS, eclipsePoseAtStar(sceneS), 2, viewport, EXPOSURE);
  const withGiant = planOf(frame);
  const without = planOf(
    frame,
    frame.bodies.filter((body) => body.id !== ECLIPSE_GIANT),
  );
  const drawn = await drawPlan(engine, renderer, withGiant, frame, frame.hostDraws);
  const bare = await drawPlan(engine, renderer, without, frame, frame.hostDraws);
  const unhidden = await drawPlan(engine, renderer, withGiant, frame, new Map());
  let differing = 0;
  for (let i = 0; i < drawn.length; i += 1) {
    differing += drawn[i] === bare[i] ? 0 : 1;
  }
  let giantTexels = 0;
  for (let i = 0; i < unhidden.length; i += 4) {
    giantTexels += (unhidden[i + 1] ?? 0) > 0 ? 1 : 0;
  }
  checks.check(
    "T10.c a giant behind the star leaves no texel of its own under the star's disc",
    withGiant.regimes.get(ECLIPSE_GIANT) === "disc" && giantTexels > 20 && differing === 0,
    `regime ${withGiant.regimes.get(ECLIPSE_GIANT) ?? "none"}; ${String(giantTexels)} texels of the giant without the star's disc, ${String(differing)} channels differing from the frame without the giant`,
  );
  // Promoted by a depth writer over it (R07.T9's synthetic writer), the giant is a mesh whose
  // figure writes its depth: the star's disc, on its limb's plane, hides it all the same.
  const giant = frame.bodies.find((body) => body.id === ECLIPSE_GIANT);
  const writer =
    giant === undefined
      ? null
      : sphereFootprint(giant.centreM, giant.figure.equatorialRadiusM, frame.camera, viewport);
  if (writer === null) {
    throw new Error("the eclipse frame has no giant on the view");
  }
  const asMesh = planOf(frame, frame.bodies, [writer]);
  const meshDrawn = await drawPlan(engine, renderer, asMesh, frame, frame.hostDraws);
  const meshUnhidden = await drawPlan(engine, renderer, asMesh, frame, new Map());
  let meshDiffering = 0;
  for (let i = 0; i < meshDrawn.length; i += 1) {
    meshDiffering += meshDrawn[i] === bare[i] ? 0 : 1;
  }
  let meshTexels = 0;
  for (let i = 0; i < meshUnhidden.length; i += 4) {
    meshTexels += (meshUnhidden[i + 1] ?? 0) > 0 ? 1 : 0;
  }
  checks.check(
    "R06.T13.e a mesh giant behind the star leaves no texel of its own under the star's disc on its limb's plane",
    asMesh.regimes.get(ECLIPSE_GIANT) === "mesh" && meshTexels > 20 && meshDiffering === 0,
    `regime ${asMesh.regimes.get(ECLIPSE_GIANT) ?? "none"}, ${String(asMesh.meshes.length)} meshes; ${String(meshTexels)} texels of the giant without the star's disc, ${String(meshDiffering)} channels differing from the frame without the giant`,
  );
}

async function checkInFront(
  engine: RenderEngine,
  renderer: LitBodyRenderer,
  discs: HostDiscLayer,
  checks: Checks,
): Promise<void> {
  // From inside the penumbra, 2,000 s before mid-eclipse: the moon over a third of the star.
  const viewport: Viewport = { widthPx: 320, heightPx: 180 };
  const sceneS = ECLIPSE_MID_S - 2_000;
  const frame = eclipseFrame(discs, sceneS, eclipsePoseAtStar(sceneS), 2, viewport, EXPOSURE);
  const plan = planOf(frame);
  const texels = await drawPlan(engine, renderer, plan, frame, frame.hostDraws);
  const record = plan.discs.find((each) => each.body === ECLIPSE_MOON);
  if (record === undefined) {
    throw new Error("the eclipse frame draws no moon");
  }
  const moonInterior = new Set<number>();
  for (const p of rasteriseDisc(record, frame.camera, viewport)) {
    if (p.draw === "interior") {
      moonInterior.add(p.yPx * viewport.widthPx + p.xPx);
    }
  }
  let starTexels = 0;
  let expectedCovered = 0;
  let covered = 0;
  for (let y = 0; y < viewport.heightPx; y += 1) {
    for (let x = 0; x < viewport.widthPx; x += 1) {
      if (!inStarDisc(frame, x + 0.5, y + 0.5)) {
        continue;
      }
      const index = y * viewport.widthPx + x;
      starTexels += 1;
      expectedCovered += moonInterior.has(index) ? 1 : 0;
      covered += texels[index * 4 + 3] === METER_CLASS.hostDisc ? 0 : 1;
    }
  }
  const share = covered / starTexels;
  checks.check(
    "T10.c the moon in front of the star covers the star's disc where its wholly covered pixels lie",
    starTexels > 1_000 && Math.abs(covered - expectedCovered) <= 4 && share > 0.2 && share < 0.5,
    `${String(covered)} of the star's ${String(starTexels)} texels taken by the moon (${(100 * share).toFixed(2)}%), ${String(expectedCovered)} on the CPU`,
  );
}

/** The captures' view. */
const CAPTURE_VIEWPORT: Viewport = { widthPx: 768, heightPx: 432 };

/** One series of captures: its name, the camera, field and exposure, and its frames' times. */
interface Series {
  readonly name: string;
  readonly fovDeg: number;
  /** The exposure, as `MAN`'s field would take it. */
  readonly ev100: number;
  readonly pose: (sceneS: number) => CameraPose;
  readonly sceneTimesS: ReadonlyArray<number>;
}

/** The series, for the owner's check by eye. */
const SERIES: ReadonlyArray<Series> = [
  {
    // The shadow crossing the planet beneath the camera, at the view's 60°, exposed for its
    // sunlit day side.
    name: "shadow",
    fovDeg: 60,
    ev100: 15,
    pose: () => ECLIPSE_CAMERA_POSE,
    sceneTimesS: [-8_000, -5_000, -2_500, -1_000, 0, 1_000, 2_500, 5_000].map(
      (s) => ECLIPSE_MID_S + s,
    ),
  },
  {
    // The star seen from inside the moon's penumbra at the view's narrowest 10°: the partial
    // eclipse to the total. Exposed as through a solar filter, the star's disc inside AgX's range:
    // at a day side's exposure its glare fills the frame, as the Sun's fills an eye.
    name: "penumbra",
    fovDeg: 10,
    ev100: 30,
    pose: eclipsePoseAtStar,
    sceneTimesS: [-3_300, -2_800, -2_200, -1_600, -1_000, -500, -300, 0].map(
      (s) => ECLIPSE_MID_S + s,
    ),
  },
  {
    // The giant passing behind the star at 10°: beside it, touching, half behind, wholly behind and
    // central. At EV100 27 the star's centre stands at the top of AgX's range and the giant's full
    // disc, about 10⁻³ of its luminance, some 3 stops below middle grey.
    name: "behind",
    fovDeg: 10,
    ev100: 27,
    pose: eclipsePoseAtStar,
    sceneTimesS: [-7_000, -6_100, -5_600, -5_000, 0].map((s) => ECLIPSE_CONJUNCTION_S + s),
  },
];

/**
 * T10.c's captures, for the owner's check by eye: each series through the photorealistic frame
 * onto a 768 × 432 canvas, tone-mapped, R06's host disc at its place in the painter's order.
 */
export async function captureEclipse(engine: RenderEngine): Promise<CapturedImage[]> {
  const view = engine.createView(addCanvas(), "smoke eclipse");
  view.resize(CAPTURE_VIEWPORT);
  const renderer = new PhotorealRenderer(engine, "smoke eclipse");
  const discs = new HostDiscLayer(engine);
  const images: CapturedImage[] = [];
  try {
    // The pipelines compile once, before the first frame; a camera of another field only moves
    // the bloom kernel, which the renderer follows.
    await renderer.prepare(CAPTURE_VIEWPORT, "high", "eye", {
      orientation: ECLIPSE_CAMERA_POSE.orientation,
      fovXRad: (60 * Math.PI) / 180,
    });
    for (const series of SERIES) {
      let previous: ReadonlyMap<BodyIdHex, LitRegime> = new Map();
      for (const sceneS of series.sceneTimesS) {
        const frame = eclipseFrame(
          discs,
          sceneS,
          series.pose(sceneS),
          series.fovDeg,
          CAPTURE_VIEWPORT,
          exposureScale(series.ev100),
        );
        const plan = renderer.render(view, { ...frame, previousRegimes: previous });
        previous = plan?.regimes ?? previous;
        // The canvas is read back in the task that drew it, each frame before the next.
        // oxlint-disable-next-line no-await-in-loop
        const bytes = await view.readBack();
        // Named by the script's second, as the view's run reaches it.
        const scriptS = Math.round(sceneS / ECLIPSE_TIME_RATE);
        images.push({
          name: `r07-t10c-eclipse-${series.name}-t${String(scriptS).padStart(3, "0")}`,
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
