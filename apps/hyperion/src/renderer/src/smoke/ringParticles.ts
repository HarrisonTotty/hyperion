/**
 * The smoke page's ring particle checks (plan R11, R11.T9.d), in part 31 beside the slab's: the
 * field's placement on the run's adapter against its `f64` twin, and a frame 300 m over a ring
 * drawn by the photorealistic renderer on both settings.
 *
 * @remarks
 * The placement is `presentation-only` and its f32 the device's, so it is held to its twin within
 * stated tolerances: the particles placed, matched by their tags (a cell's hash and index), the
 * same but for {@link MISMATCH_SHARE} of them, which a cell's count or a particle's distance test
 * may decide the other way at the f32 boundary; and each matched one's centre to
 * {@link CENTRE_TOLERANCE_M}, radius to {@link RADIUS_TOLERANCE} of itself, and colour word (its
 * albedo and the share of the star's beam the layer lets through to it) to 1 in each byte's last
 * bit, more than half of them partly shaded so that the beam's term is exercised. The frames: on high
 * the particles' pass is drawn before the discs and lights the view, every texel finite; on low it
 * is not drawn at all.
 */
import { add, norm, scale, sub, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import type { LitBodyInput } from "../view/bodies/draw";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  viewRotation4,
} from "../view/camera/projection";
import { lookAlong } from "../view/camera/quaternion";
import { cameraCullPassOf } from "../view/clusters/instanceScene";
import { instanceWorkgroups } from "../view/clusters/instanceCull";
import { PARTICLE_INSTANCE_WORDS } from "../view/clusters/particles";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { RenderEngine } from "../view/engine/types";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import type { QualitySetting } from "../view/quality/qualitySetting";
import { fieldParticles, packRingFieldParams, ringFieldOf } from "../view/rings/ringField";
import { RING_PARTICLE_PLACE_KERNEL } from "../view/rings/ringParticles";
import { ringProfile } from "../view/rings/ringProfile";
import { pixelAngleRad } from "../view/rings/ringSplit";
import type { ViewRing } from "../view/scene/model";
import type { Checks } from "./harness";
import { halfTexels } from "./harness";

const A_M = 6.0e7;
const AU_M = 1.495978707e11;
const PLANET = "0300080020000000.0102";
const VIEWPORT = { widthPx: 192, heightPx: 108 };
const FOV_X_RAD = Math.PI / 3;
/** The camera's height over the ring, m, at 1.8 of its planet's radius from its centre. */
const HEIGHT_M = 300;
/** The particles' tags the GPU and the twin may place apart, as a share of the twin's. */
const MISMATCH_SHARE = 0.01;
/** A matched particle's centre, m, at some 300 m to 2 km from the camera. */
const CENTRE_TOLERANCE_M = 0.02;
/** A matched particle's radius, of itself. */
const RADIUS_TOLERANCE = 1e-4;

/** The camera looking 40° below the horizon, along the orbit, the plane below it. */
const CAMERA: ProjectionCamera = {
  orientation: lookAlong(vec3(0, Math.cos(0.7), -Math.sin(0.7)), vec3(0, 0, 1)),
  fovXRad: FOV_X_RAD,
};
const CENTRE_M = vec3(-1.8 * A_M, 0, -HEIGHT_M);
const RING: ViewRing = {
  body: PLANET,
  ringKind: "massive",
  material: "porous_ice",
  innerRadiusM: 1.24 * A_M,
  outerRadiusM: 2.27 * A_M,
  opticalDepth: 1.5,
  gapRadiiM: [1.95 * A_M],
  normal: vec3(0, 0, 1),
  gravitationalParameterM3S2: 3.79e16,
};
const PLANET_BODY: LitBodyInput = {
  id: PLANET,
  centreM: CENTRE_M,
  figure: { equatorialRadiusM: A_M, polarRadiusM: A_M, pole: vec3(0, 0, 1) },
  photometry: PROVISIONAL_PHOTOMETRY,
  lighting: undefined,
};
const VIEW_ROTATION = viewRotation4(CAMERA.orientation);
const PROJECTION = perspectiveReversedInfinite(
  FOV_X_RAD,
  VIEWPORT.widthPx / VIEWPORT.heightPx,
  NEAR_PLANE_M,
);

/** The most two `rgba8unorm` words' bytes differ by. */
function worstByteApart(a: number, b: number): number {
  let worst = 0;
  for (let shift = 0; shift < 32; shift += 8) {
    worst = Math.max(worst, Math.abs(((a >>> shift) & 0xff) - ((b >>> shift) & 0xff)));
  }
  return worst;
}

/** The field's placement on the GPU against its twin. */
async function checkPlacement(engine: RenderEngine, checks: Checks): Promise<void> {
  const pass = cameraCullPassOf(VIEW_ROTATION, PROJECTION, VIEWPORT.widthPx, 1);
  const field = ringFieldOf({
    centreM: CENTRE_M,
    normal: RING.normal,
    profile: ringProfile(RING),
    gravitationalParameterM3S2: RING.gravitationalParameterM3S2 ?? null,
    referenceRadiusM: 1.8 * A_M - 500,
    sinceS: 40,
    pixelRad: pixelAngleRad(FOV_X_RAD, VIEWPORT.widthPx),
    planes: pass.planes,
    seed: 11,
    towardsStar: vec3(0.3, 0.2, 0.93),
  });
  if (field === null) {
    throw new Error("the smoke's field reaches no octave");
  }
  const twin = fieldParticles(field);
  const capacity = Math.max(2 * twin.length, 1024);
  const kernel = await engine.createComputeAsync(RING_PARTICLE_PLACE_KERNEL);
  const instances = engine.createBuffer({
    name: "smoke ring particles",
    bytes: capacity * PARTICLE_INSTANCE_WORDS * 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC | BUFFER_USAGE.COPY_DST,
    category: "geometry-clusters",
  });
  const count = engine.createBuffer({
    name: "smoke ring particle count",
    bytes: 16,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC | BUFFER_USAGE.COPY_DST,
    category: "geometry-clusters",
  });
  try {
    engine.writeBuffer(count, 0, new Uint32Array(4));
    engine.dispatch(
      kernel,
      {
        uniforms: { params: new Uint32Array(packRingFieldParams(field, capacity)) },
        buffers: { instances, instance_count: count },
        sampled: {},
        storage: {},
      },
      instanceWorkgroups(field.cells),
      RING_PARTICLE_PLACE_KERNEL.name,
    );
    const [words, placed] = await Promise.all([
      engine.readBuffer(instances, "tolerance"),
      engine.readBuffer(count, "tolerance"),
    ]);
    const n = new Uint32Array(placed)[0] ?? 0;
    const floats = new Float32Array(words);
    const ints = new Uint32Array(words);
    const gpu = new Map<
      number,
      { readonly centre: readonly number[]; readonly radius: number; readonly colour: number }
    >();
    for (let k = 0; k < Math.min(n, capacity); k += 1) {
      const at = k * PARTICLE_INSTANCE_WORDS;
      gpu.set(ints[at + 7] ?? 0, {
        centre: [floats[at] ?? 0, floats[at + 1] ?? 0, floats[at + 2] ?? 0],
        radius: floats[at + 3] ?? 0,
        colour: ints[at + 6] ?? 0,
      });
    }
    let unmatched = 0;
    let worstCentre = 0;
    let worstRadius = 0;
    let worstByte = 0;
    let shaded = 0;
    for (const p of twin) {
      const on = gpu.get(p.tag);
      if (on === undefined) {
        unmatched += 1;
        continue;
      }
      const [x = 0, y = 0, z = 0] = on.centre;
      worstCentre = Math.max(worstCentre, norm(sub(vec3(x, y, z), p.centreM)));
      worstRadius = Math.max(worstRadius, Math.abs(on.radius / p.radiusM - 1));
      worstByte = Math.max(worstByte, worstByteApart(on.colour, p.colour));
      // The beam's share, in the colour's top byte, strictly between none and all of it.
      const beam = p.colour >>> 24;
      shaded += beam > 0 && beam < 255 ? 1 : 0;
    }
    const extra = Math.max(n - (twin.length - unmatched), 0);
    checks.check(
      "R11.T9.d the ring particles' field on the GPU against its twin, their albedo and beam to a unit of their last bit",
      twin.length > 1000 &&
        unmatched + extra <= MISMATCH_SHARE * twin.length &&
        worstCentre <= CENTRE_TOLERANCE_M &&
        worstRadius <= RADIUS_TOLERANCE &&
        worstByte <= 1 &&
        shaded > twin.length / 2,
      `${String(n)} placed against the twin's ${String(twin.length)} over ${String(field.cells)} cells ` +
        `of ${String(field.levels.length)} octaves; ${String(unmatched)} unmatched, ${String(extra)} more; ` +
        `worst centre ${worstCentre.toExponential(2)} m, radius ${worstRadius.toExponential(2)} of itself, ` +
        `colour byte ${String(worstByte)} apart; ${String(shaded)} partly shaded by the layer`,
    );
  } finally {
    engine.releaseBuffer(instances);
    engine.releaseBuffer(count);
    engine.releaseCompute(kernel);
  }
}

/** A frame 300 m over the ring on `setting`: the particles' draws it made and its HDR light. */
async function frameOver(
  engine: RenderEngine,
  setting: QualitySetting,
): Promise<{ readonly draws: number; readonly texels: Float32Array }> {
  const renderer = new PhotorealRenderer(engine, `smoke ring particles ${setting}`);
  try {
    await renderer.prepare(VIEWPORT, setting, "eye", CAMERA);
    await Promise.all([renderer.prepareRings(), renderer.prepareRingParticles(setting)]);
    renderer.render(null, {
      camera: CAMERA,
      viewport: VIEWPORT,
      presentedPx: VIEWPORT,
      role: "eye",
      setting,
      exposureScale: 0.01,
      sky: [],
      starSprites: [],
      hostDraws: new Map(),
      glareSources: [],
      lights: [
        {
          disc: sunLikeHostDisc(),
          centreM: add(CENTRE_M, scale(vec3(0.3, 0.2, 0.93), 9.5 * AU_M)),
        },
      ],
      bodies: [PLANET_BODY],
      depthWriters: [],
      previousRegimes: new Map([[PLANET, "disc"]]),
      overlay: null,
      meter: null,
      rings: [RING],
      sceneTime: { seconds: 1_000_000, nanos: 0 },
    });
    const colours = renderer.frameColours;
    if (colours === null) {
      throw new Error("the photorealistic frame left no colour to read");
    }
    return {
      draws: renderer.ringParticleDraws,
      texels: halfTexels(await engine.readTexture(colours.drawn)),
    };
  } finally {
    renderer.dispose();
  }
}

/** The particles drawn in a frame on high and never on low, every texel finite on both. */
async function checkFrames(engine: RenderEngine, checks: Checks): Promise<void> {
  const frames: Array<{
    readonly setting: QualitySetting;
    readonly draws: number;
    readonly lit: number;
    readonly finite: boolean;
  }> = [];
  for (const setting of ["high", "low"] as const) {
    // One frame at a time, each on its own renderer.
    // oxlint-disable-next-line no-await-in-loop
    const { draws, texels } = await frameOver(engine, setting);
    frames.push({
      setting,
      draws,
      lit: texels.filter((v, i) => i % 4 === 1 && v > 0).length,
      finite: texels.every((v) => Number.isFinite(v)),
    });
  }
  const [high, low] = frames;
  checks.check(
    "R11.T9.d a frame over a ring draws its particles on high and none on low, every texel finite",
    high !== undefined &&
      low !== undefined &&
      high.draws > 0 &&
      high.lit > 100 &&
      low.draws === 0 &&
      high.finite &&
      low.finite,
    frames
      .map(
        (f) =>
          `${f.setting}: ${String(f.draws)} particle draws, ${String(f.lit)} lit texels, finite ${String(f.finite)}`,
      )
      .join("; "),
  );
}

/** The ring particles' checks (the module's remarks). */
export async function checkRingParticles(engine: RenderEngine, checks: Checks): Promise<void> {
  await checkPlacement(engine, checks);
  await checkFrames(engine, checks);
}
