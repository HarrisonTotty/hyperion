/**
 * The smoke harness's datum check (plan R10, R10.T16.g, `R10.T16.g the datum inside the figure`):
 * a body's datum over the photorealistic image is stroked inside its figure, so that it covers none
 * of the image beyond the limb, the air's limb glow above it included (main's ruling,
 * `decision-r08-approximate-plate.md` item 2); and since R02.T19 on a flattened body's spheroid.
 *
 * @remarks
 * A sphere of R05's Earth radius, 6,371,008.8 m (`TEST EARTH`'s sphere), with R05's Earth medium
 * (`R05_EARTH_REFERENCE`, standing in for `TEST EARTH`'s measured Earth: the check is of the
 * overlay, not of the medium), is drawn through `PhotorealRenderer` from 51 Mm, where the
 * finding was seen, on a 64 × 48 view at the pixel scale of a 1920 px, 60° field: its limb runs
 * down the middle of the view, lit from behind the camera, and the air's top, 100 km up, stands
 * 3.3 px beyond it. The frame is drawn at a fixed exposure with no meter, its symbology the body's
 * datum alone, its limb and graticule from R02's draw list at a line scale of 2 (a display ratio
 * of 1) and its occluder sphere, cased as the overlay cases it:
 *
 * - with the datum inside the figure (`DrawOptions.datumInside`, as the view builds it for the
 *   image), the frame's canvas holds the same bytes as with no symbology at every pixel whose
 *   centre's line of sight misses the sphere: the glow and the space beyond the limb untouched;
 * - its limb is drawn: pixels inside the outline differ;
 * - the glow is there to protect: pixels beyond the limb read above black with no symbology, some
 *   brighter than in the frame drawn without the air (bloom alone spreads the disc's light there);
 * - the control bites: the datum centred on the limb, as the overlay was built before, changes
 *   pixels beyond it.
 *
 * Then `TEST SATURN`'s spheroid, 60,268 by 54,364 km, about its pole (plan R02, R02.T19), is drawn
 * airless from 482 Mm, where R10.T16.g measured the sphere's datum 20.5 px beyond its figure at the
 * poles, on the same view, its datum a flattened body's in the draw list (its figure on its
 * appearance) and its occluder its spheroid (decision-r02-t19-leans, item 5): seen equator-on with
 * the view straddling its north pole, the limb across the middle of the view and the body below
 * it, and seen pole-on with the limb down the middle, as the Earth's:
 *
 * - with its datum inside the figure, every pixel whose centre's line of sight misses the spheroid
 *   holds the same bytes as with no symbology;
 * - its limb is drawn: in every column (equator-on) or row (pole-on) the limb crosses, a pixel of
 *   the band its cased width deep inside the outline differs;
 * - the control bites, equator-on: the sphere's datum, as R10.T16.g drew it, changes pixels beyond
 *   the figure at the pole. Pole-on the spheroid's outline lies some 0.3 px inside the sphere's, so
 *   no control is asked there.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { type BodyFigure, PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { R05_EARTH_REFERENCE } from "../view/atmosphere/earth";
import { AtmosphereTables } from "../view/atmosphere/tables";
import type { ProjectionCamera, Viewport } from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import type { RenderEngine, RenderView } from "../view/engine/types";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { overlaySubmission } from "../view/photoreal/overlay";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import type { ViewBody, ViewScene } from "../view/scene/model";
import { ATMOSPHERE_FIXTURES } from "../view/scenes/atmosphereScene";
import { AU_M } from "../view/scenes/kept";
import { type ColourTokens, readTokens } from "../spatial/paint";
import {
  buildWireframeDrawList,
  datumInsideAt,
  type DrawCamera,
  viewStrokesAt,
  type WireframeDrawList,
} from "../view/wireframe/drawList";
import { WireframeRenderer } from "../view/wireframe/submit";
import {
  aBody,
  aViewScene,
  FIXTURE_PLANET_CENTRE_M,
  FIXTURE_SYSTEM,
  NO_TURN,
} from "../test/viewFixtures";
import { addCanvas } from "./frames";
import type { Checks } from "./harness";

/** R05's spherical Earth's mean radius, m, on which its medium's tables are built. */
const EARTH_MEAN_RADIUS_M = 6_371_008.8;

/** The range the finding was seen at, m from the body's centre: 51 Mm. */
const RANGE_M = 51e6;

/** The view: small, so that the group stays short. */
const VIEWPORT: Viewport = { widthPx: 64, heightPx: 48 };

/** The focal length, px: a 1920 px field's at 60°, the scale the finding was seen at. */
const FOCAL_PX = 1920 / (2 * Math.tan(Math.PI / 6));

const CAMERA: ProjectionCamera = {
  orientation: IDENTITY_QUATERNION,
  fovXRad: 2 * Math.atan(VIEWPORT.widthPx / 2 / FOCAL_PX),
};

/** The limb's angle from the body's centre, rad: asin(r ÷ d). */
const LIMB_RAD = Math.asin(EARTH_MEAN_RADIUS_M / RANGE_M);

/**
 * The body's centre from the camera, m: the limb's angle to the left of the view's axis, so that
 * the limb runs through the view's centre, the body on its left half and space on its right.
 */
const CENTRE_M: Vec3 = scale(vec3(-Math.sin(LIMB_RAD), 0, -Math.cos(LIMB_RAD)), RANGE_M);

/** The way to the Sun from the body: from behind the camera and to its right, so the limb is lit. */
const TO_SUN = normalise(vec3(1, 0.3, 1));

/** The fixed exposure scale, 1 ÷ (cd/m²): `checkAtmosphereFrame`'s. */
const EXPOSURE = 1e-4;

const BODY: BodyIdHex = "0200080020000000.0310";

/** The Earth's figure, a sphere with no pole. */
const EARTH_FIGURE: BodyFigure = {
  equatorialRadiusM: EARTH_MEAN_RADIUS_M,
  polarRadiusM: EARTH_MEAN_RADIUS_M,
  pole: null,
};

/** The datum's scene: the body alone, as R02's planet fixture, with no craft, orbit or star. */
function datumScene(overrides: Partial<ViewBody> = {}): ViewScene {
  return aViewScene({
    bodies: [aBody({ radiusM: EARTH_MEAN_RADIUS_M, ...overrides })],
    craft: [],
    stars: [],
    ownShip: null,
  });
}

/** The camera a datum is built from: `centreM` from the planet fixture's centre. */
function drawCameraFor(centreM: Vec3): DrawCamera {
  return {
    pose: {
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: sub(FIXTURE_PLANET_CENTRE_M, centreM),
      orientation: NO_TURN,
    },
    fovXRad: CAMERA.fovXRad,
  };
}

/** The camera the Earth's datum is built from: {@link CENTRE_M} from the planet fixture's centre. */
const DRAW_CAMERA: DrawCamera = drawCameraFor(CENTRE_M);

/**
 * A body's datum alone, as the overlay draws it: its limb and graticule batches and its occluder
 * sphere, from R02's list at a display ratio of 1 (a line scale of 2), inside its figure or centred.
 */
function datumList(
  inside: boolean,
  tokens: ColourTokens,
  scene: ViewScene = datumScene(),
  camera: DrawCamera = DRAW_CAMERA,
): WireframeDrawList {
  const list = buildWireframeDrawList(scene, camera, VIEWPORT, tokens, {
    lowSetting: false,
    ev100: 0,
    selection: null,
    destination: null,
    remPx: 16,
    ...viewStrokesAt(1),
    datumInside: inside,
  });
  return { ...list, lines: list.lines.filter((line) => line.body !== undefined), anchors: [] };
}

/** The line of sight through pixel (x, y)'s centre, along the view axes (the camera's). */
function sightOf(x: number, y: number): Vec3 {
  return vec3(
    (x + 0.5 - VIEWPORT.widthPx / 2) / FOCAL_PX,
    -(y + 0.5 - VIEWPORT.heightPx / 2) / FOCAL_PX,
    -1,
  );
}

/** Whether pixel (x, y)'s centre's line of sight meets the sphere. */
function onTheSphere(x: number, y: number): boolean {
  const u = sightOf(x, y);
  const uc = dot(u, CENTRE_M);
  const k = (RANGE_M - EARTH_MEAN_RADIUS_M) * (RANGE_M + EARTH_MEAN_RADIUS_M);
  return uc > 0 && uc * uc - dot(u, u) * k >= 0;
}

/** `TEST SATURN`'s figure at 1 bar (`ATMOSPHERE_FIXTURES`): its equatorial and polar radii, m. */
function testSaturnFigure(): { readonly equatorialRadiusM: number; readonly polarRadiusM: number } {
  const saturn = ATMOSPHERE_FIXTURES.find((fixture) => fixture.designation === "TEST SATURN");
  if (saturn === undefined) {
    throw new Error("ATMOSPHERE_FIXTURES has no TEST SATURN");
  }
  return saturn.figure;
}

const SATURN = testSaturnFigure();

/** The range of R10.T16.g's measurement on `TEST SATURN`, m from its centre: 482 Mm. */
const SATURN_RANGE_M = 482e6;

/** How `TEST SATURN` is seen: its centre from the camera, m, and its figure, about its pole. */
interface SaturnView {
  readonly name: string;
  readonly centreM: Vec3;
  readonly figure: BodyFigure;
  /** Whether its limb runs across the view (equator-on) rather than down it (pole-on). */
  readonly across: boolean;
}

/**
 * Seen equator-on, the view straddling its north pole: the centre below the view's axis by the
 * cone's polar half-angle, atan(c ÷ √(d² − a²)), so that the limb at the pole crosses the axis,
 * and the pole up the view, square to the line of sight.
 */
function equatorOn(): SaturnView {
  const { equatorialRadiusM: a, polarRadiusM: c } = SATURN;
  const d = SATURN_RANGE_M;
  const half = Math.atan(c / Math.sqrt(d * d - a * a));
  return {
    name: "equator-on",
    centreM: scale(vec3(0, -Math.sin(half), -Math.cos(half)), d),
    figure: { ...SATURN, pole: vec3(0, Math.cos(half), -Math.sin(half)) },
    across: true,
  };
}

/**
 * Seen pole-on: the centre to the left of the view's axis by the cone's half-angle,
 * atan(a ÷ √(d² − c²)), so that the limb runs down the view's middle, and the pole towards the
 * camera along the line of sight.
 */
function poleOn(): SaturnView {
  const { equatorialRadiusM: a, polarRadiusM: c } = SATURN;
  const d = SATURN_RANGE_M;
  const half = Math.atan(a / Math.sqrt(d * d - c * c));
  const along = vec3(-Math.sin(half), 0, -Math.cos(half));
  return {
    name: "pole-on",
    centreM: scale(along, d),
    figure: { ...SATURN, pole: scale(along, -1) },
    across: false,
  };
}

/** `TEST SATURN`'s scene as the view draws it, its figure on its appearance, or none (a sphere). */
function saturnScene(view: SaturnView, figured: boolean): ViewScene {
  return datumScene({
    radiusM: SATURN.equatorialRadiusM,
    appearance: figured
      ? {
          photometry: PROVISIONAL_PHOTOMETRY,
          figure: view.figure,
          labels: [],
          bondRatioFinding: null,
        }
      : null,
  });
}

/**
 * Whether pixel (x, y)'s centre's line of sight meets `TEST SATURN`'s spheroid, from its quadric
 * S = (I − ppᵀ) ÷ a² + ppᵀ ÷ c²: (u · Sc)² ≥ (c · Sc − 1)(u · Su), on its near nappe.
 */
function onTheSpheroid(view: SaturnView, x: number, y: number): boolean {
  const { equatorialRadiusM: a, polarRadiusM: c, pole } = view.figure;
  const p = pole ?? vec3(0, 0, 1);
  const quadric = (v: Vec3): Vec3 => {
    const z = dot(v, p);
    return add(scale(sub(v, scale(p, z)), 1 / (a * a)), scale(p, z / (c * c)));
  };
  const u = sightOf(x, y);
  const sc = quadric(view.centreM);
  const usc = dot(u, sc);
  return usc > 0 && usc * usc - (dot(view.centreM, sc) - 1) * dot(u, quadric(u)) >= 0;
}

/**
 * What one frame is drawn with: its symbology and the tokens it was built with, the body's centre
 * and figure, and whether the body's air is drawn (the Earth's alone).
 */
interface FrameOf {
  readonly datum: WireframeDrawList | null;
  readonly tokens: ColourTokens;
  readonly air: boolean;
  readonly centreM: Vec3;
  readonly figure: BodyFigure;
  readonly camera: DrawCamera;
}

/** Draws the frame and reads its canvas back, in the task that drew it. */
async function drawFrame(
  renderer: PhotorealRenderer,
  wireframe: WireframeRenderer,
  canvas: RenderView,
  tables: AtmosphereTables,
  frame: FrameOf,
): Promise<Float32Array | Uint8Array> {
  const { datum, tokens } = frame;
  renderer.render(canvas, {
    camera: CAMERA,
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    role: "eye",
    setting: "high",
    exposureScale: EXPOSURE,
    sky: [],
    starSprites: [],
    hostDraws: new Map(),
    glareSources: [],
    lights: [{ disc: sunLikeHostDisc(), centreM: add(frame.centreM, scale(TO_SUN, AU_M)) }],
    bodies: [
      {
        id: BODY,
        centreM: frame.centreM,
        figure: frame.figure,
        photometry: PROVISIONAL_PHOTOMETRY,
        lighting: undefined,
      },
    ],
    depthWriters: [],
    previousRegimes: new Map(),
    overlay:
      datum === null
        ? null
        : (surveyed) =>
            overlaySubmission(wireframe, datum, tokens, frame.camera, VIEWPORT, surveyed),
    meter: null,
    atmospheres: frame.air
      ? [
          {
            id: BODY,
            tables,
            camera: {
              positionM: scale(frame.centreM, -1),
              orientation: IDENTITY_QUATERNION,
              fovXRad: CAMERA.fovXRad,
              viewport: VIEWPORT,
            },
            toBodyFixed: IDENTITY_QUATERNION,
          },
        ]
      : [],
  });
  // Read in the task that drew it (`checkPhotorealFrame`'s note).
  return canvas.readBack();
}

/** Whether pixel `p` of two read-backs holds the same values in every channel. */
function samePixel(a: Float32Array | Uint8Array, b: Float32Array | Uint8Array, p: number): boolean {
  return [0, 1, 2, 3].every((ch) => a[4 * p + ch] === b[4 * p + ch]);
}

/** Whether pixel `p` of a read-back is above black in a colour channel. */
function aboveBlack(read: Float32Array | Uint8Array, p: number): boolean {
  return [0, 1, 2].some((ch) => (read[4 * p + ch] ?? 0) > 0);
}

/** The largest colour channel of pixel `p` of a read-back. */
function brightest(read: Float32Array | Uint8Array, p: number): number {
  return Math.max(...[0, 1, 2].map((ch) => read[4 * p + ch] ?? 0));
}

/** The depth in pixels from the outline in of the band the inside limb's cased width covers. */
const BAND_PX = Math.ceil(2 * datumInsideAt(viewStrokesAt(1).strokeScale).reachPx.thin);

/**
 * How `TEST SATURN`'s frames with its datum and the control compare with the bare frame: the
 * pixels off the figure, those unchanged by the datum, those the control changes, the pixels on
 * it that differ, and the lines across the limb (columns equator-on, rows pole-on) whose band
 * {@link BAND_PX} deep inside the outline the datum changed, of those it crosses within the view.
 */
function compareSaturn(
  view: SaturnView,
  withDatum: Float32Array | Uint8Array,
  bare: Float32Array | Uint8Array,
  control: Float32Array | Uint8Array | null,
): {
  readonly outside: number;
  readonly outsideSame: number;
  readonly controlChanged: number;
  readonly insideDiffer: number;
  readonly limbLines: number;
  readonly limbLinesDrawn: number;
} {
  const { widthPx, heightPx } = VIEWPORT;
  let outside = 0;
  let outsideSame = 0;
  let controlChanged = 0;
  let insideDiffer = 0;
  for (let y = 0; y < heightPx; y += 1) {
    for (let x = 0; x < widthPx; x += 1) {
      const p = y * widthPx + x;
      if (onTheSpheroid(view, x, y)) {
        insideDiffer += samePixel(withDatum, bare, p) ? 0 : 1;
        continue;
      }
      outside += 1;
      outsideSame += samePixel(withDatum, bare, p) ? 1 : 0;
      controlChanged += control === null || samePixel(control, bare, p) ? 0 : 1;
    }
  }
  // Across the limb: equator-on each column from the top, the body below; pole-on each row from the
  // right, the body to the left. The band runs from the first pixel on the figure inwards.
  const lines = view.across ? widthPx : heightPx;
  const depth = view.across ? heightPx : widthPx;
  const pixelOf = (line: number, step: number): number =>
    view.across ? step * widthPx + line : line * widthPx + (widthPx - 1 - step);
  const onFigure = (line: number, step: number): boolean =>
    view.across ? onTheSpheroid(view, line, step) : onTheSpheroid(view, widthPx - 1 - step, line);
  let limbLines = 0;
  let limbLinesDrawn = 0;
  for (let line = 0; line < lines; line += 1) {
    const edge = Array.from({ length: depth }, (_, step) => step).find((step) =>
      onFigure(line, step),
    );
    if (edge === undefined || edge < 1 || edge + BAND_PX > depth) {
      continue;
    }
    limbLines += 1;
    const band = Array.from({ length: BAND_PX }, (_, i) => pixelOf(line, edge + i));
    limbLinesDrawn += band.some((p) => !samePixel(withDatum, bare, p)) ? 1 : 0;
  }
  return { outside, outsideSame, controlChanged, insideDiffer, limbLines, limbLinesDrawn };
}

/**
 * R02.T19's checks of `TEST SATURN` seen one way: its frames with its datum, with none and, seen
 * equator-on, with the sphere's datum as R10.T16.g drew it, each read back before the next.
 */
async function checkSaturnView(
  view: SaturnView,
  draw: (frame: FrameOf) => Promise<Float32Array | Uint8Array>,
  tokens: ColourTokens,
  checks: Checks,
): Promise<void> {
  const camera = drawCameraFor(view.centreM);
  const saturn = { tokens, centreM: view.centreM, figure: view.figure, camera, air: false };
  const datum = datumList(true, tokens, saturnScene(view, true), camera);
  const withDatum = await draw({ ...saturn, datum });
  const none = await draw({ ...saturn, datum: null });
  let control: Float32Array | Uint8Array | null = null;
  if (view.across) {
    const sphere = datumList(true, tokens, saturnScene(view, false), camera);
    control = await draw({ ...saturn, datum: sphere });
  }
  const seen = compareSaturn(view, withDatum, none, control);
  const spheroids = datum.occluderSpheres.filter((sphere) => sphere.spheroid !== undefined).length;
  const lineName = view.across ? "columns" : "rows";
  checks.check(
    `R02.T19 over the image TEST SATURN's datum, seen ${view.name}, leaves every pixel beyond its spheroid as the image drew it`,
    datum.lines.length > 0 &&
      spheroids === 1 &&
      seen.outside > 0 &&
      seen.outsideSame === seen.outside,
    `${String(datum.lines.length)} datum batches, ${String(spheroids)} spheroid occluder; ${String(seen.outsideSame)} of the ${String(seen.outside)} pixels whose centres' lines of sight miss the spheroid hold the same bytes with and without it`,
  );
  checks.check(
    `R02.T19 TEST SATURN's datum inside its spheroid is drawn, seen ${view.name}: in every one of the ${lineName} its limb crosses`,
    seen.insideDiffer > 0 && seen.limbLines > 0 && seen.limbLinesDrawn === seen.limbLines,
    `${String(seen.insideDiffer)} pixels on the figure differ; ${String(seen.limbLinesDrawn)} of the ${String(seen.limbLines)} ${lineName} the limb crosses changed within ${String(BAND_PX)} px inside it`,
  );
  if (control !== null) {
    checks.check(
      "R02.T19 the control bites: the sphere's datum, as R10.T16.g drew it, changes pixels beyond TEST SATURN's figure at its pole, seen equator-on",
      seen.controlChanged > 0,
      `${String(seen.controlChanged)} of the ${String(seen.outside)} pixels beyond the figure changed by the sphere's datum`,
    );
  }
}

/** R10.T16.g's datum over the image, and R02.T19's on `TEST SATURN`: see the module's remarks. */
export async function checkDatumFrame(engine: RenderEngine, checks: Checks): Promise<void> {
  const tokens: ColourTokens = readTokens(document.documentElement);
  const element = addCanvas();
  // What the group made, released in reverse after it, whatever it threw.
  const made: Array<{ dispose(): void }> = [];
  const hold = <T extends { dispose(): void }>(each: T): T => {
    made.push(each);
    return each;
  };
  try {
    const renderer = hold(new PhotorealRenderer(engine, "datum frame smoke"));
    const wireframe = hold(new WireframeRenderer(engine));
    const tables = hold(new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M));
    const canvas = hold(engine.createView(element, "datum frame smoke"));
    canvas.resize(VIEWPORT);
    await renderer.prepare(VIEWPORT, "high", "eye", CAMERA);
    const draw = (frame: FrameOf): Promise<Float32Array | Uint8Array> =>
      drawFrame(renderer, wireframe, canvas, tables, frame);
    const earth = { tokens, centreM: CENTRE_M, figure: EARTH_FIGURE, camera: DRAW_CAMERA };
    const inside = datumList(true, tokens);
    const centred = datumList(false, tokens);
    const withInside = await draw({ ...earth, datum: inside, air: true });
    const bare = await draw({ ...earth, datum: null, air: true });
    const withCentred = await draw({ ...earth, datum: centred, air: true });
    const airless = await draw({ ...earth, datum: null, air: false });

    let outside = 0;
    let outsideSame = 0;
    let insideDiffer = 0;
    let insidePixels = 0;
    let glow = 0;
    let airGlow = 0;
    let controlChanged = 0;
    // The rows the limb crosses, and those whose band inside it, its cased width from the outline
    // in, the inside datum changed: the limb drawn all down the view, not hidden by its occluder.
    let limbRows = 0;
    let limbRowsDrawn = 0;
    const bandPx = BAND_PX;
    for (let y = 0; y < VIEWPORT.heightPx; y += 1) {
      let edgeX = -1;
      for (let x = 0; x < VIEWPORT.widthPx; x += 1) {
        const p = y * VIEWPORT.widthPx + x;
        if (onTheSphere(x, y)) {
          edgeX = x;
          insidePixels += 1;
          insideDiffer += samePixel(withInside, bare, p) ? 0 : 1;
          continue;
        }
        outside += 1;
        outsideSame += samePixel(withInside, bare, p) ? 1 : 0;
        glow += aboveBlack(bare, p) ? 1 : 0;
        airGlow += brightest(bare, p) > brightest(airless, p) ? 1 : 0;
        controlChanged += samePixel(withCentred, bare, p) ? 0 : 1;
      }
      if (edgeX >= bandPx && edgeX < VIEWPORT.widthPx - 1) {
        limbRows += 1;
        const row = y * VIEWPORT.widthPx;
        const band = Array.from({ length: bandPx }, (_, i) => row + edgeX - i);
        limbRowsDrawn += band.some((p) => !samePixel(withInside, bare, p)) ? 1 : 0;
      }
    }
    const batches = `${String(inside.lines.length)} datum batches, ${String(inside.occluderSpheres.length)} occluder sphere`;
    checks.check(
      "R10.T16.g over the image the datum inside the figure leaves every pixel beyond the limb as the image drew it",
      inside.lines.length > 0 && outside > 0 && outsideSame === outside,
      `${batches}; ${String(outsideSame)} of the ${String(outside)} pixels whose centres' lines of sight miss the sphere hold the same bytes with and without it`,
    );
    checks.check(
      "R10.T16.g the datum inside the figure is drawn: pixels inside the outline differ, in every row its limb crosses",
      insideDiffer > 0 && limbRows > 0 && limbRowsDrawn === limbRows,
      `${String(insideDiffer)} of the ${String(insidePixels)} pixels inside the outline differ; ` +
        `${String(limbRowsDrawn)} of the ${String(limbRows)} rows the limb crosses changed within ${String(bandPx)} px inside it`,
    );
    checks.check(
      "R10.T16.g the air's glow stands beyond the limb: pixels there above black with no symbology, some brighter than without the air",
      glow > 0 && airGlow > 0,
      `${String(glow)} of the ${String(outside)} pixels beyond the limb above black, ${String(airGlow)} brighter than in the frame without the air`,
    );
    checks.check(
      "R10.T16.g the control bites: the datum centred on the limb, as the overlay was built before, changes pixels beyond it",
      controlChanged > 0,
      `${String(controlChanged)} of the ${String(outside)} pixels beyond the limb changed by the centred datum`,
    );

    // R02.T19: `TEST SATURN`'s datum on its spheroid, airless, equator-on and then pole-on.
    await checkSaturnView(equatorOn(), draw, tokens, checks);
    await checkSaturnView(poleOn(), draw, tokens, checks);
  } finally {
    for (const each of made.toReversed()) {
      each.dispose();
    }
    element.remove();
  }
}
