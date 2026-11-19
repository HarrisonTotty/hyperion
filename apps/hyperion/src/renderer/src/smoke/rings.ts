/**
 * The smoke page's ring slab checks (plan R11, R11.T9.c), in part 1: a ringed planet's frame drawn
 * by the photorealistic renderer on both settings, every pixel of its ring against the slab's `f64`
 * twin and its edges against R02's wireframe ellipses, and the slab's other forms drawn alone.
 *
 * @remarks
 * The GPU's `f32`, its filtering and its derivatives are not the twin's, and need not be, so each
 * value is held within a stated tolerance:
 * - a pixel the ring covers wholly, away from its bands' edges by the profile's footprint, to
 *   {@link RELATIVE_TOLERANCE} of the twin's light and {@link ABSOLUTE_TOLERANCE} beside it;
 * - every pixel more than {@link EDGE_MARGIN_PX} outside both of the ring's ellipses, as R02's
 *   `ringEllipse` places them, untouched, and every one as far inside covered whole: the edges
 *   within half a pixel of the wireframe's, the plan's test;
 * - the form drawn over an air's composite (`sampled`) hidden wherever the scene's depth is nearer,
 *   and otherwise the same as the scene target's form (`attached`) to {@link SAME_TOLERANCE};
 * - the ring's fragments beyond a planet's air shell and the others, drawn into one target, the
 *   ring whole to {@link SAME_TOLERANCE};
 * - the slab with its particles' share taken (R11.T9.d) at {@link SPLIT_PIXEL_RAD}, against its
 *   twin's as the whole is, the share's light unlike the whole's by the twin's ratio.
 */
import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import type { LitBodyInput } from "../view/bodies/draw";
import {
  NEAR_PLANE_M,
  perspectiveReversedInfinite,
  type ProjectionCamera,
  project,
  viewRotation4,
} from "../view/camera/projection";
import { lookAlong, rotationRows } from "../view/camera/quaternion";
import type { RenderEngine, RenderTarget } from "../view/engine/types";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { lightsAt, type PlacedLight } from "../view/lighting/hostLights";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import { type QualitySetting, SETTINGS } from "../view/quality/qualitySetting";
import { ringProfile } from "../view/rings/ringProfile";
import {
  pixelRay,
  RING_PROFILE_TEXELS,
  ringProfileTexels,
  profileLevel,
  ringSlabFragment,
  type RingSlabRecord,
  ringSlabRecordOf,
} from "../view/rings/ringSlab";
import { RingSlabs } from "../view/rings/ringSlabs";
import type { ViewRing } from "../view/scene/model";
import { ringEllipse } from "../view/wireframe/bodies";
import { withRingShadows } from "../displays/view/photorealFrame";
import { base64Of, type CapturedImage } from "./atmosphere";
import { addCanvas } from "./frames";
import { type Checks, flatSpec, halfTexels, rectangleMesh } from "./harness";

/** A Saturn-like planet's equatorial radius, m. */
const A_M = 6.0e7;
const AU_M = 1.495978707e11;
const PLANET = "0300080020000000.0101";
const VIEWPORT = { widthPx: 192, heightPx: 108 };
const FOV_X_RAD = Math.PI / 3;
/** The planet's radius on the view, px, and the ring's elevation over its plane from the camera. */
const PLANET_PX = 14;
const ELEVATION_RAD = (25 * Math.PI) / 180;

/** A wholly covered pixel's light against the twin's, relative. */
const RELATIVE_TOLERANCE = 0.02;
/** And beside it, pre-exposed (a lit pixel's light is about 0.5 here). */
const ABSOLUTE_TOLERANCE = 2e-3;
/** How far outside or inside the ellipses a pixel's centre must lie to be held, px. */
const EDGE_MARGIN_PX = 0.6;
/**
 * The pixel's angle the slab's particles' share is taken at in the forms' check, rad: a footprint
 * of 0.6–0.85 m at the ring's 6–8.5 × 10⁸ m, 40–45% of its optical depth the particles'.
 */
const SPLIT_PIXEL_RAD = 1e-9;
/** Two forms' or draws' light that must agree but for rounding, pre-exposed. */
const SAME_TOLERANCE = 1e-5;

/** The camera at the origin looking down −z, y up. */
const CAMERA: ProjectionCamera = {
  orientation: lookAlong(vec3(0, 0, -1), vec3(0, 1, 0)),
  fovXRad: FOV_X_RAD,
};
const ROWS = rotationRows(CAMERA.orientation);
const PX_PER_RAD = VIEWPORT.widthPx / (2 * Math.tan(FOV_X_RAD / 2));
const DISTANCE_M = A_M / Math.sin(PLANET_PX / PX_PER_RAD);
const CENTRE_M = vec3(0, 0, -DISTANCE_M);
/** The ring plane's normal: the camera ELEVATION_RAD above the plane, the plane nearly level. */
const NORMAL = vec3(0, Math.cos(ELEVATION_RAD), Math.sin(ELEVATION_RAD));
/**
 * The star 9.5 au away, from the planet: `front` on the camera's side of the plane and off to its
 * right, the lit face seen; `back` behind the planet beyond the plane, the unlit face seen and the
 * planet's shadow and its penumbra thrown on the ring before it.
 */
const LIGHTINGS = {
  front: normalise(vec3(1, 0.3, 0.6)),
  back: normalise(vec3(0.45, -0.25, -0.86)),
} as const;

/** A lighting of {@link LIGHTINGS}. */
type Lighting = keyof typeof LIGHTINGS;

/** The frame's lights under `lighting`. */
function hostsOf(lighting: Lighting): ReadonlyArray<PlacedLight> {
  return [
    { disc: sunLikeHostDisc(), centreM: add(CENTRE_M, scale(LIGHTINGS[lighting], 9.5 * AU_M)) },
  ];
}
const EXPOSURE = 0.01;

/** A Saturn-like massive ring of porous ice from 1.24 a to 2.27 a, τ 1.5, with a gap at 1.95 a. */
const RING: ViewRing = {
  body: PLANET,
  ringKind: "massive",
  material: "porous_ice",
  innerRadiusM: 1.24 * A_M,
  outerRadiusM: 2.27 * A_M,
  opticalDepth: 1.5,
  gapRadiiM: [1.95 * A_M],
  normal: NORMAL,
};

const PLANET_BODY: LitBodyInput = {
  id: PLANET,
  centreM: CENTRE_M,
  figure: { equatorialRadiusM: A_M, polarRadiusM: A_M, pole: NORMAL },
  photometry: PROVISIONAL_PHOTOMETRY,
  lighting: undefined,
};

/** The ring's record as the renderer makes it on `setting` under `lighting`. */
function recordOn(setting: QualitySetting, lighting: Lighting): RingSlabRecord {
  return ringSlabRecordOf(RING, PLANET_BODY, lightsAt(CENTRE_M, hostsOf(lighting), 2), [], {
    annuli: SETTINGS[setting].photoreal.discAnnuli,
    exposureScale: EXPOSURE,
  });
}

/** The ring's radius ÷ a along a ray. */
function radiusAlong(record: RingSlabRecord, ray: Vec3): number {
  return norm(sub(scale(ray, record.planeOffset / dot(record.normal, ray)), record.centre));
}

/** One pixel's ray, the ring's radius along it and the radius's change across the pixel, ÷ a. */
interface PixelRing {
  readonly ray: Vec3;
  readonly r: number;
  readonly footprint: number;
}

function pixelRing(record: RingSlabRecord, x: number, y: number): PixelRing {
  const ray = pixelRay(x + 0.5, y + 0.5, ROWS, FOV_X_RAD, VIEWPORT);
  const r = radiusAlong(record, ray);
  const dx = radiusAlong(record, pixelRay(x + 1.5, y + 0.5, ROWS, FOV_X_RAD, VIEWPORT)) - r;
  const dy = radiusAlong(record, pixelRay(x + 0.5, y + 1.5, ROWS, FOV_X_RAD, VIEWPORT)) - r;
  return { ray, r, footprint: Math.max(Math.abs(dx), Math.abs(dy)) };
}

/** Whether a pixel's ray passes within 1.5 px of the planet's disc. */
function nearPlanet(ray: Vec3): boolean {
  const towards = normalise(CENTRE_M);
  const apart = Math.acos(Math.min(dot(ray, towards), 1));
  return apart * PX_PER_RAD < PLANET_PX + 1.5;
}

/** Whether radius r ÷ a lies within the profile's footprint at `level` of one of its bands' edges. */
function nearBandEdge(record: RingSlabRecord, r: number, level: number): boolean {
  const profile = ringProfile(RING);
  const texelOverA = (record.outerOverA - record.innerOverA) / RING_PROFILE_TEXELS;
  const reach = (2 ** (level + 1) + 2) * texelOverA;
  return profile.bands.some(
    (band) => Math.abs(r - band.innerM / A_M) < reach || Math.abs(r - band.outerM / A_M) < reach,
  );
}

/** The ellipses' signed distance of a pixel inside the ring's annulus, px: positive inside both. */
function insideBy(record: RingSlabRecord, pixel: PixelRing): number {
  return Math.min(pixel.r - record.innerOverA, record.outerOverA - pixel.r) / pixel.footprint;
}

/** A frame of the ringed planet, drawn on `setting` and read back as the scene's HDR light. */
async function drawnFrame(
  engine: RenderEngine,
  setting: QualitySetting,
  lighting: Lighting,
): Promise<Float32Array> {
  const renderer = new PhotorealRenderer(engine, `smoke rings ${setting}`);
  try {
    await renderer.prepare(VIEWPORT, setting, "eye", CAMERA);
    await renderer.prepareRings();
    renderer.render(null, {
      camera: CAMERA,
      viewport: VIEWPORT,
      presentedPx: VIEWPORT,
      role: "eye",
      setting,
      exposureScale: EXPOSURE,
      sky: [],
      starSprites: [],
      hostDraws: new Map(),
      glareSources: [],
      lights: hostsOf(lighting),
      bodies: [PLANET_BODY],
      depthWriters: [],
      previousRegimes: new Map([[PLANET, "disc"]]),
      overlay: null,
      meter: null,
      rings: [RING],
    });
    const colours = renderer.frameColours;
    if (colours === null) {
      throw new Error("the photorealistic frame left no colour to read");
    }
    return halfTexels(await engine.readTexture(colours.drawn));
  } finally {
    renderer.dispose();
  }
}

/** A frame's ring pixels against the twin's: `checkSetting`'s tallies. */
interface TwinTally {
  /** Wholly covered pixels away from the bands' edges compared, and those in the planet's shadow. */
  readonly compared: number;
  readonly unlit: number;
  /** The worst excess over the tolerances, ≤ 0 within them, and where. */
  readonly worst: number;
  readonly worstAt: string;
  /** Pixels outside the ellipses by the margin, and those drawn. */
  readonly outside: number;
  readonly touched: number;
  /** Pixels inside by the margin, those the twin leaves uncovered and those it lights left dark. */
  readonly inside: number;
  readonly uncovered: number;
  readonly dark: number;
  /** The brightest light the twin gives any pixel inside, and the GPU any pixel but the planet's. */
  readonly twinMost: number;
  readonly gpuMost: number;
  readonly gpuMostAt: string;
  /** The compared pixels' green, the twin's and the GPU's, summed. */
  readonly twinGreen: number;
  readonly gpuGreen: number;
}

/** The ring pixels of `drawn`, the scene's HDR light, against the twin of `record`. */
function tallyAgainstTwin(drawn: Float32Array, record: RingSlabRecord): TwinTally {
  const texels = ringProfileTexels(ringProfile(RING));
  let compared = 0;
  let worst = 0;
  let worstAt = "";
  let outside = 0;
  let touched = 0;
  let inside = 0;
  let uncovered = 0;
  let unlit = 0;
  let dark = 0;
  let twinGreen = 0;
  let gpuGreen = 0;
  // The brightest light the twin gives any pixel inside, and the brightest the GPU gives any pixel
  // of the view but the planet's: an outlier anywhere, an edge's or a penumbra's, exceeds it.
  let twinMost = 0;
  let gpuMost = 0;
  let gpuMostAt = "";
  for (let y = 0; y < VIEWPORT.heightPx; y += 1) {
    for (let x = 0; x < VIEWPORT.widthPx; x += 1) {
      const pixel = pixelRing(record, x, y);
      if (nearPlanet(pixel.ray)) {
        continue;
      }
      const at = (y * VIEWPORT.widthPx + x) * 4;
      const gpu = [
        drawn[at] ?? Number.NaN,
        drawn[at + 1] ?? Number.NaN,
        drawn[at + 2] ?? Number.NaN,
      ];
      const brightest = Math.max(
        ...gpu.map((v) => (Number.isFinite(v) ? v : Number.POSITIVE_INFINITY)),
      );
      if (!(brightest <= gpuMost)) {
        gpuMost = brightest;
        gpuMostAt = `(${String(x)}, ${String(y)})`;
      }
      const by = insideBy(record, pixel);
      if (by <= -EDGE_MARGIN_PX) {
        outside += 1;
        touched += gpu.some((v) => v !== 0) ? 1 : 0;
        continue;
      }
      if (by < EDGE_MARGIN_PX) {
        continue;
      }
      inside += 1;
      const twin = ringSlabFragment(record, texels, pixel.ray, pixel.footprint);
      if (twin === null || twin.coverage < 1) {
        uncovered += 1;
        continue;
      }
      // Its own light: a pixel the planet's shadow leaves dark is held at 0 alone. One the twin
      // lights must be lit on the GPU wherever it lies inside, the edges' bands included.
      unlit += twin.radiance[1] > 0 ? 0 : 1;
      twinMost = Math.max(twinMost, ...twin.radiance);
      dark += twin.radiance[1] > 0 && !((gpu[1] ?? 0) > 0) ? 1 : 0;
      if (nearBandEdge(record, pixel.r, profileLevel(record, pixel.footprint))) {
        continue;
      }
      compared += 1;
      twinGreen += twin.radiance[1];
      gpuGreen += gpu[1] ?? Number.NaN;
      for (const [c, value] of gpu.entries()) {
        const expected = twin.radiance[c] ?? Number.NaN;
        const excess =
          Math.abs(value - expected) - (RELATIVE_TOLERANCE * expected + ABSOLUTE_TOLERANCE);
        if (!(excess <= worst)) {
          worst = Number.isFinite(excess) ? excess : Number.POSITIVE_INFINITY;
          worstAt = `(${String(x)}, ${String(y)}) channel ${String(c)}: ${value.toPrecision(5)} against ${expected.toPrecision(5)}`;
        }
      }
    }
  }
  return {
    compared,
    unlit,
    worst,
    worstAt,
    outside,
    touched,
    inside,
    uncovered,
    dark,
    twinMost,
    gpuMost,
    gpuMostAt,
    twinGreen,
    gpuGreen,
  };
}

/** The ring's frame on one setting under one lighting against its twin and R02's ellipses. */
async function checkSetting(
  engine: RenderEngine,
  checks: Checks,
  setting: QualitySetting,
  lighting: Lighting,
): Promise<void> {
  const drawn = await drawnFrame(engine, setting, lighting);
  const record = recordOn(setting, lighting);
  const named = `${setting}, ${lighting}-lit`;
  const {
    compared,
    unlit,
    worst,
    worstAt,
    outside,
    touched,
    inside,
    uncovered,
    dark,
    twinMost,
    gpuMost,
    gpuMostAt,
  } = tallyAgainstTwin(drawn, record);
  checks.check(
    `R11.T9.c the ring slab's light against its twin on ${named}`,
    compared > 100 &&
      unlit > 20 &&
      compared - unlit > 50 &&
      worst <= 0 &&
      uncovered === 0 &&
      gpuMost <= (1 + RELATIVE_TOLERANCE) * twinMost + ABSOLUTE_TOLERANCE,
    `${String(compared)} wholly covered pixels compared, ${String(unlit)} of them in the planet's shadow; ` +
      (worst <= 0 ? "every one within tolerance" : `worst ${worstAt}`) +
      `; ${String(uncovered)} the twin leaves uncovered; the brightest pixel ${gpuMost.toPrecision(4)} at ` +
      `${gpuMostAt}, the twin's brightest ${twinMost.toPrecision(4)}`,
  );
  // R02's ellipses as the wireframe draws them: every vertex within half a pixel of the twin's edge
  // (`ringSlab.test.ts` holds the twin's edge to them), so the margins above are about the ellipses.
  const { edges } = ringEllipse({ ...RING, centreM: CENTRE_M }, CAMERA, VIEWPORT);
  let vertices = 0;
  let off = 0;
  for (const [k, polyline] of edges.entries()) {
    const edgeOverA = k < edges.length / 2 ? record.innerOverA : record.outerOverA;
    for (const point of polyline) {
      const p = project(point, CAMERA, VIEWPORT);
      const pixel = pixelRing(record, p.xPx - 0.5, p.yPx - 0.5);
      vertices += 1;
      off += Math.abs(pixel.r - edgeOverA) / pixel.footprint < 0.5 ? 0 : 1;
    }
  }
  checks.check(
    `R11.T9.c the ring slab's edges are R02's wireframe ellipses to half a pixel on ${named}`,
    outside > 1000 &&
      touched === 0 &&
      inside > 200 &&
      uncovered === 0 &&
      dark === 0 &&
      vertices > 50 &&
      off === 0,
    `${String(outside)} pixels more than ${String(EDGE_MARGIN_PX)} px outside, ${String(touched)} drawn; ` +
      `${String(inside)} as far inside, ${String(uncovered)} not covered whole, ${String(dark)} the twin lights left dark; ` +
      `${String(off)} of ${String(vertices)} ellipse vertices off the slab's edge by half a pixel`,
  );
}

/** The ring drawn alone into `target` by `slabs`' frame, with the identity camera. */
function drawAlone(
  target: RenderTarget,
  draws: Parameters<RenderTarget["render"]>[0]["draws"],
): void {
  target.render({
    label: "smoke rings",
    viewRotation: viewRotation4(CAMERA.orientation),
    projection: perspectiveReversedInfinite(
      FOV_X_RAD,
      VIEWPORT.widthPx / VIEWPORT.heightPx,
      NEAR_PLANE_M,
    ),
    draws,
    postProcesses: [],
  });
}

/** The slab's forms drawn alone: over the scene's depth read, and by parts about an air shell. */
async function checkForms(engine: RenderEngine, checks: Checks): Promise<void> {
  const slabs = new RingSlabs(engine, "smoke rings alone");
  const made: RenderTarget[] = [];
  const target = (name: string, depth: boolean): RenderTarget => {
    const each = engine.createRenderTarget({
      name: `smoke rings ${name}`,
      size: VIEWPORT,
      format: "rgba16float",
      mips: 1,
      depth,
      category: "render-targets",
    });
    made.push(each);
    return each;
  };
  const occluderMesh = rectangleMesh(
    engine,
    "smoke rings occluder",
    [-1e12, -1e12, 0, 1e12],
    -DISTANCE_M / 2,
  );
  const flat = engine.createMaterial(flatSpec("smoke rings occluder"));
  try {
    await slabs.prepare();
    const frame = slabs.frame({
      rings: [RING],
      bodies: [PLANET_BODY],
      plan: {
        regimes: new Map([[PLANET, "disc"]]),
        sources: new Map([
          [PLANET, { stars: lightsAt(CENTRE_M, hostsOf("front"), 2), planetshine: [] }],
        ]),
      },
      camera: CAMERA,
      viewport: VIEWPORT,
      exposureScale: EXPOSURE,
      annuli: SETTINGS.high.photoreal.discAnnuli,
    });
    if (frame === null) {
      throw new Error("the slabs drew no ring");
    }
    // The attached form into a target with depth; the scene's depth with an occluder nearer than
    // the planet over the view's left half; the sampled form over it into a target without depth.
    const attached = target("attached", true);
    drawAlone(attached, [frame.draw(0, "all", 0, null)]);
    const scene = target("scene", true);
    drawAlone(scene, [
      {
        mesh: occluderMesh,
        material: flat,
        offsetFromCameraM: new Float32Array(3),
        uniforms: { tint: new Float32Array([0, 0, 0, 0]) },
        textures: {},
      },
    ]);
    const sceneDepth = scene.depth;
    if (sceneDepth === null) {
      throw new Error("the scene target holds no depth");
    }
    const sampled = target("sampled", false);
    drawAlone(sampled, [frame.draw(0, "all", 0, sceneDepth)]);
    // The two parts about a shell of 1.6 a, which the ring crosses, into one target.
    const parts = target("parts", false);
    drawAlone(parts, [
      frame.draw(0, "beyond-air", 1.6, null),
      frame.draw(0, "before-air", 1.6, null),
    ]);
    const beyondOnly = target("beyond", false);
    drawAlone(beyondOnly, [frame.draw(0, "beyond-air", 1.6, null)]);
    const read = async (each: RenderTarget): Promise<Float32Array> =>
      halfTexels(await engine.readTexture(each.colour));
    const [a, s, p, b] = await Promise.all([
      read(attached),
      read(sampled),
      read(parts),
      read(beyondOnly),
    ]);
    const width = VIEWPORT.widthPx;
    let hidden = 0;
    let leaked = 0;
    let shown = 0;
    let differs = 0;
    let partsDiffer = 0;
    let beyond = 0;
    let lit = 0;
    for (let i = 0; i < width * VIEWPORT.heightPx; i += 1) {
      const x = i % width;
      const green = a[4 * i + 1] ?? Number.NaN;
      lit += green > 0 ? 1 : 0;
      beyond += (b[4 * i + 1] ?? 0) > 0 ? 1 : 0;
      for (let c = 0; c < 4; c += 1) {
        const all = a[4 * i + c] ?? Number.NaN;
        if (Math.abs((p[4 * i + c] ?? Number.NaN) - all) > SAME_TOLERANCE) {
          partsDiffer += 1;
        }
      }
      if (green === 0) {
        continue;
      }
      const over = s[4 * i + 1] ?? Number.NaN;
      // The occluder's edge at the view's middle column; a column either side of it is left out.
      if (x < width / 2 - 1) {
        hidden += 1;
        leaked += over === 0 ? 0 : 1;
      } else if (x > width / 2) {
        shown += 1;
        differs += Math.abs(over - green) <= SAME_TOLERANCE ? 0 : 1;
      }
    }
    checks.check(
      "R11.T9.c the ring slab over an air reads the scene's depth: hidden behind it, the same before it",
      hidden > 100 && leaked === 0 && shown > 100 && differs === 0,
      `${String(hidden)} ring pixels behind the nearer depth, ${String(leaked)} drawn; ` +
        `${String(shown)} before it, ${String(differs)} unlike the scene target's form`,
    );
    checks.check(
      "R11.T9.c the ring's fragments beyond an air shell and the others make the ring whole",
      lit > 200 && beyond > 0 && beyond < lit && partsDiffer === 0,
      `${String(lit)} ring pixels, ${String(beyond)} beyond a shell of 1.6 a; ` +
        `${String(partsDiffer)} channels of the two parts unlike the ring whole`,
    );
    // The slab's share of the particles taken (R11.T9.d) at a pixel's angle that puts the
    // footprint 7 × 10⁸ m away between the particles' sizes: the attached form against its twin's
    // with the share and without it, the share's light unlike the whole slab's.
    const lights = lightsAt(CENTRE_M, hostsOf("front"), 2);
    const split = slabs.frame({
      rings: [RING],
      bodies: [PLANET_BODY],
      plan: {
        regimes: new Map([[PLANET, "disc"]]),
        sources: new Map([[PLANET, { stars: lights, planetshine: [] }]]),
      },
      camera: CAMERA,
      viewport: VIEWPORT,
      exposureScale: EXPOSURE,
      annuli: SETTINGS.high.photoreal.discAnnuli,
      instancedPixelRad: new Map([[RING, SPLIT_PIXEL_RAD]]),
    });
    if (split === null) {
      throw new Error("the slabs drew no split ring");
    }
    const splitTarget = target("split", true);
    drawAlone(splitTarget, [split.draw(0, "all", 0, null)]);
    const whole = tallyAgainstTwin(a, recordOn("high", "front"));
    const shared = tallyAgainstTwin(
      await read(splitTarget),
      ringSlabRecordOf(RING, PLANET_BODY, lights, [], {
        annuli: SETTINGS.high.photoreal.discAnnuli,
        exposureScale: EXPOSURE,
        instancedPixelRad: SPLIT_PIXEL_RAD,
      }),
    );
    const twinRatio = shared.twinGreen / whole.twinGreen;
    const gpuRatio = shared.gpuGreen / whole.gpuGreen;
    checks.check(
      "R11.T9.d the ring slab without its particles' share against its twin, the share's light unlike the whole's",
      [whole, shared].every(
        (tally) => tally.compared > 100 && tally.worst <= 0 && tally.uncovered === 0,
      ) &&
        twinRatio < 0.97 &&
        Math.abs(gpuRatio / twinRatio - 1) < RELATIVE_TOLERANCE,
      `whole: ${String(whole.compared)} compared, ` +
        (whole.worst <= 0 ? "within tolerance" : `worst ${whole.worstAt}`) +
        `; split at ${SPLIT_PIXEL_RAD.toExponential(1)} rad: ${String(shared.compared)} compared, ` +
        (shared.worst <= 0 ? "within tolerance" : `worst ${shared.worstAt}`) +
        `; its light over the whole's ${twinRatio.toFixed(4)} by the twin, ${gpuRatio.toFixed(4)} on the GPU`,
    );
  } finally {
    slabs.dispose();
    for (const each of made) {
      each.dispose();
    }
    engine.releaseMaterial(flat);
    engine.releaseMesh(occluderMesh);
  }
}

/** The ring slab's checks (the module's remarks). */
export async function checkRingSlab(engine: RenderEngine, checks: Checks): Promise<void> {
  await checkSetting(engine, checks, "high", "front");
  await checkSetting(engine, checks, "low", "front");
  await checkSetting(engine, checks, "high", "back");
  await checkForms(engine, checks);
}

/** The captures' view, px. */
const CAPTURE_VIEWPORT = { widthPx: 640, heightPx: 360 };

/**
 * A Saturn-like ring's captures (plan R11, R11.T9's acceptance by hand, recorded): front-lit, the
 * star 38° from the camera on its side of the plane, and back-lit, the star behind the planet
 * beyond the plane, at 149°, each on both settings: its shadow on the planet (high only, by
 * `photorealFrame.ts`' `withRingShadows`) and the planet's on it.
 */
export async function captureRings(engine: RenderEngine): Promise<CapturedImage[]> {
  const images: CapturedImage[] = [];
  const pxPerRad = CAPTURE_VIEWPORT.widthPx / (2 * Math.tan(FOV_X_RAD / 2));
  const distanceM = A_M / Math.sin(90 / pxPerRad);
  const centreM = vec3(0, 0, -distanceM);
  const normal = vec3(0, Math.cos((20 * Math.PI) / 180), Math.sin((20 * Math.PI) / 180));
  const planet: LitBodyInput = {
    ...PLANET_BODY,
    centreM,
    figure: { equatorialRadiusM: A_M, polarRadiusM: 0.9 * A_M, pole: normal },
  };
  const ring: ViewRing = { ...RING, normal };
  // From the planet: towards the camera (+z) and 38° round, or behind it, 149° round, below the plane.
  const lights = {
    "front-lit": { towards: normalise(vec3(0.5, 0.35, 0.79)), exposure: 2e-3 },
    "back-lit": { towards: normalise(vec3(0.45, -0.25, -0.86)), exposure: 2e-2 },
  } as const;
  for (const setting of ["high", "low"] as const) {
    for (const [name, light] of Object.entries(lights)) {
      const view = engine.createView(addCanvas(), `smoke rings ${name}`);
      view.resize(CAPTURE_VIEWPORT);
      const renderer = new PhotorealRenderer(engine, `smoke rings ${name} ${setting}`);
      try {
        // Each capture's pipelines are made before its frame, one capture at a time.
        // oxlint-disable-next-line no-await-in-loop
        await renderer.prepare(CAPTURE_VIEWPORT, setting, "eye", CAMERA);
        // Its rings' slabs too, which a view's frame would make only at its first ring.
        // oxlint-disable-next-line no-await-in-loop
        await renderer.prepareRings();
        renderer.render(view, {
          camera: CAMERA,
          viewport: CAPTURE_VIEWPORT,
          presentedPx: CAPTURE_VIEWPORT,
          role: "eye",
          setting,
          exposureScale: light.exposure,
          sky: [],
          starSprites: [],
          hostDraws: new Map(),
          glareSources: [],
          lights: [
            { disc: sunLikeHostDisc(), centreM: add(centreM, scale(light.towards, 9.5 * AU_M)) },
          ],
          // The ring's shadow on the planet where the setting draws it (T9.a).
          bodies: withRingShadows([planet], [ring], SETTINGS[setting].ringShadowOnBody),
          depthWriters: [],
          previousRegimes: new Map([[PLANET, "disc"]]),
          overlay: null,
          meter: null,
          rings: [ring],
        });
        // The canvas is read back in the task that drew it.
        // oxlint-disable-next-line no-await-in-loop
        const bytes = await view.readBack();
        images.push({
          name: `r11-t9c-rings-${name}-${setting}`,
          width: CAPTURE_VIEWPORT.widthPx,
          height: CAPTURE_VIEWPORT.heightPx,
          rgba: base64Of(bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes)),
        });
      } finally {
        renderer.dispose();
        view.dispose();
      }
    }
  }
  images.push(...(await captureClose(engine)));
  return images;
}

/**
 * The ring close to (plan R11, R11.T9.d): 300 m over a Saturn-like ring at 1.8 of its planet's
 * radius, looking along the orbit 25° below the horizon, the star high behind the camera, on high
 * (its particles drawn, the slab's share taken) and on low (the slab alone).
 */
async function captureClose(engine: RenderEngine): Promise<CapturedImage[]> {
  const images: CapturedImage[] = [];
  const normal = vec3(0, 0, 1);
  const camera: ProjectionCamera = {
    orientation: lookAlong(vec3(0, Math.cos(0.44), -Math.sin(0.44)), normal),
    fovXRad: FOV_X_RAD,
  };
  const centreM = vec3(-1.8 * A_M, 0, -300);
  const ring: ViewRing = { ...RING, normal, gravitationalParameterM3S2: 3.79e16 };
  const planet: LitBodyInput = {
    ...PLANET_BODY,
    centreM,
    figure: { equatorialRadiusM: A_M, polarRadiusM: 0.9 * A_M, pole: normal },
  };
  for (const setting of ["high", "low"] as const) {
    const view = engine.createView(addCanvas(), "smoke rings close");
    view.resize(CAPTURE_VIEWPORT);
    const renderer = new PhotorealRenderer(engine, `smoke rings close ${setting}`);
    try {
      // Each capture's pipelines are made before its frame, one capture at a time.
      // oxlint-disable-next-line no-await-in-loop
      await renderer.prepare(CAPTURE_VIEWPORT, setting, "eye", camera);
      // Its rings' slabs and particles too, which a view's frame would make at its first ring.
      // oxlint-disable-next-line no-await-in-loop
      await Promise.all([renderer.prepareRings(), renderer.prepareRingParticles(setting)]);
      renderer.render(view, {
        camera,
        viewport: CAPTURE_VIEWPORT,
        presentedPx: CAPTURE_VIEWPORT,
        role: "eye",
        setting,
        exposureScale: 2e-3,
        sky: [],
        starSprites: [],
        hostDraws: new Map(),
        glareSources: [],
        lights: [
          {
            disc: sunLikeHostDisc(),
            centreM: add(centreM, scale(normalise(vec3(0.2, -0.5, 0.84)), 9.5 * AU_M)),
          },
        ],
        bodies: withRingShadows([planet], [ring], SETTINGS[setting].ringShadowOnBody),
        depthWriters: [],
        previousRegimes: new Map([[PLANET, "disc"]]),
        overlay: null,
        meter: null,
        rings: [ring],
        sceneTime: { seconds: 1_000_000, nanos: 0 },
      });
      // The canvas is read back in the task that drew it.
      // oxlint-disable-next-line no-await-in-loop
      const bytes = await view.readBack();
      images.push({
        name: `r11-t9d-rings-close-${setting}`,
        width: CAPTURE_VIEWPORT.widthPx,
        height: CAPTURE_VIEWPORT.heightPx,
        rgba: base64Of(bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes)),
      });
    } finally {
      renderer.dispose();
      view.dispose();
    }
  }
  return images;
}
