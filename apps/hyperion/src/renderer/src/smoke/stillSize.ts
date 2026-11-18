/**
 * The smoke page's still larger than its view (plan R11, R11.T10.e.1), in part 28: a view of
 * 100 × 70 px holding a lit body at full phase, three star sprites and a host star's disc, taken at
 * twice its output size through the photorealistic renderer's hook (`PhotorealStills`), its frame
 * made again at the still's size as `VIEW`'s drawer makes it: R06's disc through its own
 * `HostDiscLayer` and the sprites prepared at the still's viewport.
 *
 * @remarks
 * Each is found in the still's assembled HDR image by its light's centroid in a window about where
 * T10.a's projection puts it at the still's size (R02's `project`, which the tiles' M · P compose
 * to), and must stand within a tenth to a sixth of a pixel of it: the sprites by their
 * point-spread weights, and the host disc, two pixels across on the view and so a sprite there, as
 * a disc of more than three on the still; and the body's disc, by its analytic cells, where T10.a's
 * mapping puts its centroid on a still of the view's own size, twice it. Black elsewhere: no sky.
 */

import type { HostDiscDto, SkyBand } from "@hyperion/protocol";

import { normalise, scale, type Vec3, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { type LitBodyInput, planLitBodies } from "../view/bodies/draw";
import { type ProjectionCamera, project, type Viewport } from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import type { DrawItem, RenderEngine, ViewSize } from "../view/engine/types";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { PLANETSHINE_SOURCES_HIGH } from "../view/lighting/planetshine";
import {
  type DrawnPhotorealFrame,
  type PhotorealFrame,
  PhotorealStills,
} from "../view/photoreal/renderer";
import { AU_M } from "../view/scenes/kept";
import { BandLayer } from "../view/sky/band";
import { HostDiscLayer } from "../view/sky/disc";
import { encodeStillPng, runStillJob, type StillRenderer } from "../view/stills/stillJob";
import { spriteRecord } from "../view/wireframe/drawList";
import { TEST_STILL_REQUEST } from "../test/stillFixtures";
import { type Checks, halfTexels } from "./harness";

/** The view's output size, px: the still is taken at twice it. */
const VIEW_SIZE: ViewSize = { widthPx: 100, heightPx: 70 };

/** The still's multiple of the view's output size. */
const STILL_SCALE = 2;

/** R02's 60° field of view, looking down −z. */
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

/** The pre-exposure, 1 ÷ (cd/m²): a sunlit rocky disc's, mid-tone. */
const EXPOSURE_SCALE = 1e-4;

/** The body's radius, m: R05's spherical Earth's mean radius. */
const BODY_RADIUS_M = 6_371_008.8;

/** The body's distance, m: some 12 px in radius on the still, 6 on the view. */
const BODY_DISTANCE_M = 1.05e8;

/**
 * The host star's distance, m: its disc 1.2° across, 3.6 px on the still, a disc, and 1.8 px on
 * the view, a sprite (`DISC_MIN_DIAMETER_PX`, 3).
 */
const HOST_DISTANCE_M = 6.64e10;

/** The sprites' illuminance, lx a channel: some 30 pre-exposed a unit of weight. */
const SPRITE_LX = 10;

/**
 * The most a centroid may stand off its predicted pixel, px of the still, for each kind: the body's
 * off its centroid on the view's own still, scaled, whose perspective it shares (a sphere off the
 * axis images to an ellipse whose centre lies outward of its centre's projection, here by some
 * 0.15 px); the sprites' and the host disc's off their directions' projections.
 */
const TOLERANCE_PX = { body: 0.1, host: 1 / 6, sprite: 0.1 } as const;

/** A view direction, unit, on the galactic axes (the camera's): through NDC (x, y) of `size`. */
function through(xNdc: number, yNdc: number, size: ViewSize): Vec3 {
  const s = 1 / Math.tan(CAMERA.fovXRad / 2);
  return normalise(vec3(xNdc / s, yNdc / (s * (size.widthPx / size.heightPx)), -1));
}

/**
 * The view's scene, the same at any size: the body, the sprites' and the host's directions, off the
 * view's axis, and on a 2 × 2-tiled still's seams at twice its size (x 128 px, y 70 px): the body
 * and a sprite across its column seam, the host disc and another sprite across its row seam.
 */
const BODY_DIRECTION = through(0.28, 0.45, VIEW_SIZE);
const HOST_DIRECTION = through(-0.5, 0, VIEW_SIZE);
const SPRITE_DIRECTIONS: ReadonlyArray<Vec3> = [
  through(0.28, -0.5, VIEW_SIZE),
  through(-0.2, 0, VIEW_SIZE),
  through(0.6, 0.6, VIEW_SIZE),
];

/**
 * The host star drawn: a Sun-like disc ten magnitudes fainter at {@link HOST_DISTANCE_M}, a light
 * of the frame, so that the painter's order draws it, too faint to move the body's phase.
 */
const HOST: HostDiscDto = sunLikeHostDisc({ star: 7, absoluteV: 14.83 });

/** The lit body, at full phase: its light straight behind the camera from it. */
function litBody(): { readonly body: LitBodyInput; readonly lights: PhotorealFrame["lights"] } {
  const centreM = scale(BODY_DIRECTION, BODY_DISTANCE_M);
  const body: LitBodyInput = {
    id: "0200080020000000.0100",
    centreM,
    figure: { equatorialRadiusM: BODY_RADIUS_M, polarRadiusM: BODY_RADIUS_M, pole: null },
    photometry: PROVISIONAL_PHOTOMETRY,
    lighting: undefined,
  };
  return {
    body,
    lights: [
      { disc: sunLikeHostDisc({ star: 1 }), centreM: scale(BODY_DIRECTION, -AU_M) },
      { disc: HOST, centreM: scale(HOST_DIRECTION, HOST_DISTANCE_M) },
    ],
  };
}

/** The band's face side in the tiled check, texels: enough that its rays vary across a pixel. */
const BAND_FACE = 16;

/**
 * A band of a different white luminance at every texel, 100 to 1,000 cd/m² (0.01 to 0.1
 * pre-exposed), so that a tile's sky drawn off its place differs from the untiled still's.
 */
function patternedBand(): SkyBand {
  const count = 6 * BAND_FACE * BAND_FACE;
  return {
    count,
    luminanceCdM2: Float32Array.from({ length: count }, (_, i) => 100 + ((i * 37) % 97) * 9.3),
    chroma: new Float32Array(count * 2).fill(1 / 3),
    eyeLimitMag: new Float32Array(count).fill(Number.NaN),
    spRatio: new Float32Array(count).fill(2.26),
  };
}

/**
 * The view's frame at `viewport`, as `VIEW`'s drawer makes it again (`photorealFrame`): R06's disc
 * from `layer` and the sprites projected at the viewport, the hand-over at the view's own size,
 * and R06's band from `band` where given.
 */
function frameAt(
  layer: HostDiscLayer,
  viewport: Viewport,
  band: BandLayer | null = null,
): PhotorealFrame {
  const { body, lights } = litBody();
  const discs = layer.frame(
    [{ host: HOST, direction: HOST_DIRECTION, distanceM: HOST_DISTANCE_M }],
    CAMERA,
    viewport,
    EXPOSURE_SCALE,
  );
  const hostDraws = new Map<number, DrawItem[]>();
  for (const { star, item } of discs.draws) {
    hostDraws.set(star, [...(hostDraws.get(star) ?? []), item]);
  }
  const sprites = [
    ...SPRITE_DIRECTIONS.map((direction) => ({
      direction,
      illuminanceRgbLx: [SPRITE_LX, SPRITE_LX, SPRITE_LX] as const,
    })),
    ...discs.sprites,
  ].flatMap(({ direction, illuminanceRgbLx }) => {
    const p = project(direction, CAMERA, viewport);
    return p.inFront
      ? [
          spriteRecord(
            { xPx: p.xPx, yPx: p.yPx, depth: 0 },
            illuminanceRgbLx,
            EXPOSURE_SCALE,
            direction,
            CAMERA,
            viewport,
          ),
        ]
      : [];
  });
  return {
    camera: CAMERA,
    viewport,
    presentedPx: VIEW_SIZE,
    role: "eye",
    setting: "high",
    exposureScale: EXPOSURE_SCALE,
    sky: [band?.draw(EXPOSURE_SCALE) ?? null].filter((draw) => draw !== null),
    starSprites: sprites,
    hostDraws,
    glareSources: [],
    lights,
    bodies: [body],
    depthWriters: [],
    previousRegimes: new Map(),
    overlay: null,
    meter: null,
  };
}

/** The view's last frame, with its making again at a still's size. */
function viewDrawn(layer: HostDiscLayer, band: BandLayer | null): DrawnPhotorealFrame {
  const frame = frameAt(layer, VIEW_SIZE, band);
  const plan = planLitBodies(
    frame.bodies,
    frame.lights,
    {
      camera: CAMERA,
      viewport: VIEW_SIZE,
      presentedPx: VIEW_SIZE,
      exposureScale: EXPOSURE_SCALE,
      annuli: DISC_ANNULI_HIGH,
      planetshine: PLANETSHINE_SOURCES_HIGH,
      setting: "high",
    },
    new Map(),
  );
  return { frame, plan, remake: ({ viewport }) => frameAt(layer, viewport, band) };
}

/** What a still of the view gave: its size, its tiles and its assembled HDR texels. */
export interface TakenStill {
  readonly outcome: string;
  readonly size: ViewSize;
  readonly tiles: number;
  /** The assembled HDR image, `rgba16float` as numbers, rows from the top; empty where none. */
  readonly hdr: Float32Array;
}

/**
 * A still of the view, at `stillScale` times its output size, its tiles at most `tileLimitPx` on a
 * side, of `samples` samples, through the hook on the page's engine.
 */
export async function takeViewStill(
  engine: RenderEngine,
  options: {
    readonly stillScale: number;
    readonly tileLimitPx: number;
    readonly samples: number;
    /** Whether the view draws R06's band behind its scene. */
    readonly band?: boolean;
  },
): Promise<TakenStill> {
  const layer = new HostDiscLayer(engine);
  const band = options.band === true ? new BandLayer(engine) : null;
  band?.update(patternedBand(), BAND_FACE, new Float64Array(6 * BAND_FACE * BAND_FACE * 3));
  const drawn = viewDrawn(layer, band);
  const { size: _size, ...request } = TEST_STILL_REQUEST;
  const stills = new PhotorealStills({
    tileLimitPx: options.tileLimitPx,
    samples: options.samples,
  });
  const detach = stills.attach({
    engine,
    timer: () => "absent",
    frameIntervalNs: () => 16_683_350,
    setting: () => "high",
    drawn: () => drawn,
    request: () => request,
  });
  const held: { hdr: Promise<ArrayBuffer> | null } = { hdr: null };
  try {
    const snapshot = stills.snapshot({ scale: options.stillScale });
    if (snapshot.kind === "unavailable") {
      return { outcome: snapshot.cause, size: VIEW_SIZE, tiles: 0, hdr: new Float32Array(0) };
    }
    const inner = stills.renderer(snapshot.scene);
    const renderer: StillRenderer = {
      tiling: inner.tiling,
      units: inner.units,
      prepare: () => inner.prepare(),
      streamed: () => inner.streamed(),
      submit: (first, count) => {
        inner.submit(first, count);
      },
      readTile: (tile) => {
        held.hdr ??= inner.readHdr();
        return inner.readTile(tile);
      },
      dispose: () => {
        inner.dispose();
      },
    };
    const outcome = await runStillJob({
      device: stills.device,
      renderer,
      request: snapshot.scene.request,
      setting: stills.setting,
      frameIntervalNs: stills.frameIntervalNs(),
      encode: encodeStillPng,
      save: (_png, label) =>
        Promise.resolve({ kind: "saved", still: { id: "larger still", label } }),
      onStep: () => undefined,
    });
    return {
      outcome: outcome.kind,
      size: snapshot.scene.request.size,
      tiles: inner.tiling.tiles.length,
      hdr: held.hdr === null ? new Float32Array(0) : halfTexels(await held.hdr),
    };
  } finally {
    await Promise.allSettled(held.hdr === null ? [] : [held.hdr]);
    detach();
    band?.dispose();
  }
}

/** The light-weighted centroid of `hdr`'s pixels within `radiusPx` of (x, y), and its light. */
function centroidNear(
  hdr: Float32Array,
  size: ViewSize,
  at: { readonly xPx: number; readonly yPx: number },
  radiusPx: number,
): { readonly xPx: number; readonly yPx: number; readonly light: number } {
  let light = 0;
  let sx = 0;
  let sy = 0;
  const x0 = Math.max(0, Math.floor(at.xPx - radiusPx));
  const x1 = Math.min(size.widthPx, Math.ceil(at.xPx + radiusPx));
  const y0 = Math.max(0, Math.floor(at.yPx - radiusPx));
  const y1 = Math.min(size.heightPx, Math.ceil(at.yPx + radiusPx));
  for (let y = y0; y < y1; y += 1) {
    for (let x = x0; x < x1; x += 1) {
      const i = (y * size.widthPx + x) * 4;
      const l = (hdr[i] ?? 0) + (hdr[i + 1] ?? 0) + (hdr[i + 2] ?? 0);
      light += l;
      sx += l * (x + 0.5);
      sy += l * (y + 0.5);
    }
  }
  return light > 0
    ? { xPx: sx / light, yPx: sy / light, light }
    : { xPx: Number.NaN, yPx: Number.NaN, light: 0 };
}

/** How far a centroid stands from its predicted pixel, px. */
function offPx(
  found: { readonly xPx: number; readonly yPx: number },
  predicted: { readonly xPx: number; readonly yPx: number },
): number {
  return Math.hypot(found.xPx - predicted.xPx, found.yPx - predicted.yPx);
}

/** Part 28's still at twice its view's size (R11.T10.e.1): see the module's remarks. */
export async function checkLargerStill(checks: Checks, engine: RenderEngine): Promise<void> {
  const still = await takeViewStill(engine, {
    stillScale: STILL_SCALE,
    tileLimitPx: 8192,
    samples: 1,
  });
  const own = await takeViewStill(engine, { stillScale: 1, tileLimitPx: 8192, samples: 1 });
  const { size, hdr } = still;
  const expected = {
    widthPx: VIEW_SIZE.widthPx * STILL_SCALE,
    heightPx: VIEW_SIZE.heightPx * STILL_SCALE,
  };
  checks.check(
    "R11.T10.e a still is taken at twice its view's output size",
    still.outcome === "saved" &&
      size.widthPx === expected.widthPx &&
      size.heightPx === expected.heightPx &&
      hdr.length === size.widthPx * size.heightPx * 4,
    `${still.outcome}, ${String(size.widthPx)} × ${String(size.heightPx)} px, ${String(hdr.length / 4)} texels`,
  );
  if (hdr.length !== size.widthPx * size.heightPx * 4) {
    return;
  }
  const radiusPx =
    (BODY_RADIUS_M / BODY_DISTANCE_M) * (size.widthPx / (2 * Math.tan(CAMERA.fovXRad / 2)));
  const centreAt = project(scale(BODY_DIRECTION, BODY_DISTANCE_M), CAMERA, size);
  const body = centroidNear(hdr, size, centreAt, radiusPx + 3);
  const onView = centroidNear(
    own.hdr,
    own.size,
    { xPx: centreAt.xPx / STILL_SCALE, yPx: centreAt.yPx / STILL_SCALE },
    radiusPx / STILL_SCALE + 3,
  );
  const bodyAt = { xPx: onView.xPx * STILL_SCALE, yPx: onView.yPx * STILL_SCALE };
  const hostAt = project(HOST_DIRECTION, CAMERA, size);
  const host = centroidNear(hdr, size, hostAt, 5);
  const sprites = SPRITE_DIRECTIONS.map((direction) => {
    const at = project(direction, CAMERA, size);
    return { at, found: centroidNear(hdr, size, at, 3.5) };
  });
  const spriteOff = Math.max(...sprites.map(({ at, found }) => offPx(found, at)));
  checks.check(
    "R11.T10.e a still larger than its view has its body's disc, its host disc and its sprites on T10.a's predicted pixels",
    body.light > 0 &&
      onView.light > 0 &&
      offPx(body, bodyAt) <= TOLERANCE_PX.body &&
      host.light > 0 &&
      offPx(host, hostAt) <= TOLERANCE_PX.host &&
      sprites.every(({ found }) => found.light > 0) &&
      spriteOff <= TOLERANCE_PX.sprite,
    `body ${offPx(body, bodyAt).toFixed(4)} px off its own still's, ${offPx(body, centreAt).toFixed(4)} px off its centre's projection (${radiusPx.toFixed(1)} px radius), host disc ${offPx(host, hostAt).toFixed(4)} px off, sprites at most ${spriteOff.toFixed(4)} px off`,
  );
}

/**
 * The tile limit at which the still at twice the view's size, 200 × 140 px, is cut into 2 × 2
 * tiles, its seams at x 128 px and y 70 px.
 */
const TWO_BY_TWO_LIMIT_PX = 128;

/**
 * The most a texel of the tiled still may differ from the untiled one's, relative to the larger:
 * two units in the last place of `rgba16float`'s 11-bit significand, 2⁻⁹. The tiles' M · P differs
 * from P by the single `f32` rounding of its entries (the projection test's tolerance, under
 * 10⁻⁵ px here), which moves a pixel's rays by far less than a texel's own rounding.
 */
const TILED_RELATIVE_TOLERANCE = 2 ** -9;

/** The texels' differences: the largest relative one, and how many pass the tolerance. */
function texelDifferences(
  a: Float32Array,
  b: Float32Array,
): { readonly largest: number; readonly over: number } {
  let largest = 0;
  let over = 0;
  for (let i = 0; i < a.length; i += 1) {
    if (i % 4 === 3) {
      continue;
    }
    const x = a[i] ?? Number.NaN;
    const y = b[i] ?? Number.NaN;
    const larger = Math.max(Math.abs(x), Math.abs(y));
    const relative = larger === 0 ? 0 : Math.abs(x - y) / larger;
    if (!(relative <= TILED_RELATIVE_TOLERANCE)) {
      over += 1;
    }
    largest = Math.max(largest, Number.isNaN(relative) ? Number.POSITIVE_INFINITY : relative);
  }
  return { largest, over };
}

/**
 * Part 28's tiled still (R11.T10.e.2): the still at twice the view's size, over R06's band and in
 * two jittered samples, 2 × 2-tiled through each tile's M · P and its origin, against the same still
 * drawn as one tile, texel for texel.
 */
export async function checkTiledStill(checks: Checks, engine: RenderEngine): Promise<void> {
  // Two samples, so that the jitter's moved origins are drawn across the seams, over R06's band.
  const whole = await takeViewStill(engine, {
    stillScale: STILL_SCALE,
    tileLimitPx: 8192,
    samples: 2,
    band: true,
  });
  const tiled = await takeViewStill(engine, {
    stillScale: STILL_SCALE,
    tileLimitPx: TWO_BY_TWO_LIMIT_PX,
    samples: 2,
    band: true,
  });
  const comparable =
    whole.outcome === "saved" &&
    tiled.outcome === "saved" &&
    whole.tiles === 1 &&
    tiled.tiles === 4 &&
    whole.hdr.length > 0 &&
    whole.hdr.length === tiled.hdr.length;
  const { largest, over } = comparable
    ? texelDifferences(whole.hdr, tiled.hdr)
    : { largest: Number.NaN, over: Number.NaN };
  checks.check(
    "R11.T10.e a 2 × 2-tiled still equals an untiled still of the same size within the projection test's tolerance",
    comparable && over === 0,
    `${whole.outcome} in ${String(whole.tiles)} tile, ${tiled.outcome} in ${String(tiled.tiles)}; ${String(over)} channels over ${TILED_RELATIVE_TOLERANCE.toExponential(2)}, the largest relative difference ${largest.toExponential(3)}`,
  );
}
