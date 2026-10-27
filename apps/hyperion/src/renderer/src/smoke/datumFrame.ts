/**
 * The smoke harness's datum check (plan R10, R10.T16.g, `R10.T16.g the datum inside the figure`):
 * a body's datum over the photorealistic image is stroked inside its figure, so that it covers none
 * of the image beyond the limb, the air's limb glow above it included (main's ruling,
 * `decision-r08-approximate-plate.md` item 2).
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
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { R05_EARTH_REFERENCE } from "../view/atmosphere/earth";
import { AtmosphereTables } from "../view/atmosphere/tables";
import type { ProjectionCamera, Viewport } from "../view/camera/projection";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import type { RenderEngine, RenderView } from "../view/engine/types";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import { overlaySubmission } from "../view/photoreal/overlay";
import { PhotorealRenderer } from "../view/photoreal/renderer";
import type { ViewScene } from "../view/scene/model";
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

/** The datum's scene: the body alone, as R02's planet fixture, with no craft, orbit or star. */
function datumScene(): ViewScene {
  return aViewScene({
    bodies: [aBody({ radiusM: EARTH_MEAN_RADIUS_M })],
    craft: [],
    stars: [],
    ownShip: null,
  });
}

/** The camera the datum is built from: {@link CENTRE_M} from the planet fixture's centre. */
const DRAW_CAMERA: DrawCamera = {
  pose: {
    frame: { kind: "system", system: FIXTURE_SYSTEM },
    positionM: sub(FIXTURE_PLANET_CENTRE_M, CENTRE_M),
    orientation: NO_TURN,
  },
  fovXRad: CAMERA.fovXRad,
};

/**
 * The body's datum alone, as the overlay draws it: its limb and graticule batches and its occluder
 * sphere, from R02's list at a display ratio of 1 (a line scale of 2), inside its figure or centred.
 */
function datumList(inside: boolean, tokens: ColourTokens): WireframeDrawList {
  const list = buildWireframeDrawList(datumScene(), DRAW_CAMERA, VIEWPORT, tokens, {
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

/** Whether pixel (x, y)'s centre's line of sight meets the sphere. */
function onTheSphere(x: number, y: number): boolean {
  const u = vec3(
    (x + 0.5 - VIEWPORT.widthPx / 2) / FOCAL_PX,
    -(y + 0.5 - VIEWPORT.heightPx / 2) / FOCAL_PX,
    -1,
  );
  const uc = dot(u, CENTRE_M);
  const k = (RANGE_M - EARTH_MEAN_RADIUS_M) * (RANGE_M + EARTH_MEAN_RADIUS_M);
  return uc > 0 && uc * uc - dot(u, u) * k >= 0;
}

/**
 * What one frame is drawn with: its symbology and the tokens it was built with, and whether the
 * body's air is drawn.
 */
interface FrameOf {
  readonly datum: WireframeDrawList | null;
  readonly tokens: ColourTokens;
  readonly air: boolean;
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
    lights: [{ disc: sunLikeHostDisc(), centreM: add(CENTRE_M, scale(TO_SUN, AU_M)) }],
    bodies: [
      {
        id: BODY,
        centreM: CENTRE_M,
        figure: {
          equatorialRadiusM: EARTH_MEAN_RADIUS_M,
          polarRadiusM: EARTH_MEAN_RADIUS_M,
          pole: null,
        },
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
            overlaySubmission(wireframe, datum, tokens, DRAW_CAMERA, VIEWPORT, surveyed),
    meter: null,
    atmospheres: frame.air
      ? [
          {
            id: BODY,
            tables,
            camera: {
              positionM: scale(CENTRE_M, -1),
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

/** R10.T16.g's datum over the image: see the module's remarks. */
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
    const inside = datumList(true, tokens);
    const centred = datumList(false, tokens);
    const withInside = await draw({ datum: inside, tokens, air: true });
    const bare = await draw({ datum: null, tokens, air: true });
    const withCentred = await draw({ datum: centred, tokens, air: true });
    const airless = await draw({ datum: null, tokens, air: false });

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
    const bandPx = Math.ceil(2 * datumInsideAt(viewStrokesAt(1).strokeScale).reachPx.thin);
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
  } finally {
    for (const each of made.toReversed()) {
      each.dispose();
    }
    element.remove();
  }
}
