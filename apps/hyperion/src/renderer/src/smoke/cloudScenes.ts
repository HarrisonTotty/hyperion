/**
 * The clouds' scenes by hand (plan R11, R11.T7.c; the owner's requirement, 2026-11-19): an
 * Earth-like body under R05's air, its liquid deck from 1 km to 3 km under plan 14's cloud fraction
 * of 0.67, drawn through `PhotorealRenderer` as a view draws it, at the owner's views: from 2 m on
 * the ground looking up at 45°, at the horizon and towards the sun; from 500 m beneath the deck's
 * base; from within the deck; from just above its tops; from 10 km up; and from orbit, where the
 * layer draws. Each is drawn on high, its volume's history settled over frames, and the views from
 * the ground, beneath and within the deck on low too, where the layer draws, all through one
 * renderer prepared on high (a second's compiling would pass the part's watchdog on SwiftShader).
 * For a research agent's eye check against photographs; nothing is asserted but that each frame is
 * finite.
 *
 * @remarks
 * The sun stands over the equator at 60° E, an equinox. The place is the first, from 15° N to 28° N
 * on the circle where the sun stands 35° up in the morning, whose clouds by the field's twin are
 * broken about it ({@link scenePlace}): a cumulus field, the zonal stand-in's cover there about 0.4
 * (`cloudShape.ts`' kinds). The views look 110° from the sun's azimuth, side-lit, but the one
 * towards it.
 */

import { cross, norm, scale, type Vec3 } from "../geometry/vec3";
import { type BodyPhotometry, PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { lawFor } from "../view/appearance/phase";
import { R05_EARTH_REFERENCE } from "../view/atmosphere/earth";
import { AtmosphereTables } from "../view/atmosphere/tables";
import { lookAlong } from "../view/camera/quaternion";
import { cloudFieldAt, cloudFieldParams, cloudSeedOf } from "../view/clouds/cloudDensity";
import {
  cloudLayerHeightM,
  largestWindMps,
  layerCondensateOf,
  seasonCoverage,
} from "../view/clouds/cloudLayer";
import { cloudNoiseTexels } from "../view/clouds/cloudNoise";
import type { BodyClouds } from "../view/clouds/condensates";
import type { RenderEngine } from "../view/engine/types";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import type { Rgb } from "../view/photometry/toneCurve";
import { type PhotorealFrame, PhotorealRenderer } from "../view/photoreal/renderer";
import type { QualitySetting } from "../view/quality/qualitySetting";
import { AU_M } from "../view/scenes/kept";
import type { CapturedImage } from "./atmosphere";
import { addCanvas } from "./frames";
import { halfTexels } from "./harness";
import { hdrCapture } from "./hdrCapture";

/** R05's spherical Earth's mean radius, m, on which its medium's tables are built. */
const EARTH_MEAN_RADIUS_M = 6_371_008.8;

/** The scenes' size, px: small PNGs, the volume marched at half of each side. */
const VIEWPORT = { widthPx: 320, heightPx: 180 };

/**
 * The frames each scene is drawn, for the volume's history to settle (TEMPORAL_WEIGHT 1/8: 66% of
 * the way at 8), within part 26's watchdog on SwiftShader.
 */
const SCENE_FRAMES = 8;

/** The pre-exposure, 1 ÷ (cd/m²): a daylit scene's, about EV 14.5. */
const EXPOSURE_SCALE = 1 / 12_000;

/** The body's id, whose seed its clouds take. */
const BODY = "0200080020000000.0103";

/** The scenes' time on the universe clock, s. */
const SCENE_TIME_S = 123_456;

const RAD = Math.PI / 180;

/** The sun's direction, body-fixed: over the equator at 60° E. */
const SUN_DIRECTION: Vec3 = { x: Math.cos(60 * RAD), y: Math.sin(60 * RAD), z: 0 };

/** The sun's elevation at the place, degrees. */
const SUN_ELEVATION_DEG = 35;

/** The body's clouds: a liquid water deck from 1 km to 3 km, in-cloud τ 8, under 67% cover. */
const EARTH_CLOUDS: BodyClouds = {
  cloudFraction: 0.67,
  surfacePressurePa: 101_325,
  decks: [
    {
      condensate: "H2O",
      phase: "liquid",
      inCloudOpticalDepth550: 8,
      coverage: 0.67,
      baseAltitudeM: 1_000,
      topAltitudeM: 3_000,
      optics: null,
    },
  ],
};

/** A view: its height, its pitch and its azimuth from the sun's, degrees, and its field. */
interface SceneView {
  readonly name: string;
  readonly heightM: number;
  readonly pitchDeg: number;
  readonly azimuthFromSunDeg: number;
  readonly fovXDeg: number;
}

/** The owner's views (the module's remarks). */
const VIEWS: ReadonlyArray<SceneView> = [
  { name: "ground-up-45", heightM: 2, pitchDeg: 45, azimuthFromSunDeg: -110, fovXDeg: 90 },
  { name: "ground-horizon", heightM: 2, pitchDeg: 4, azimuthFromSunDeg: -110, fovXDeg: 90 },
  { name: "ground-towards-sun", heightM: 2, pitchDeg: 20, azimuthFromSunDeg: -10, fovXDeg: 90 },
  { name: "below-base-500m", heightM: 500, pitchDeg: 30, azimuthFromSunDeg: -110, fovXDeg: 90 },
  { name: "within-deck", heightM: 2_000, pitchDeg: 0, azimuthFromSunDeg: -110, fovXDeg: 90 },
  { name: "above-tops", heightM: 3_300, pitchDeg: -15, azimuthFromSunDeg: -110, fovXDeg: 90 },
  { name: "up-10km", heightM: 10_000, pitchDeg: -20, azimuthFromSunDeg: -110, fovXDeg: 80 },
  { name: "orbit-2000km", heightM: 2_000_000, pitchDeg: -90, azimuthFromSunDeg: 0, fovXDeg: 60 },
];

/** The views drawn on low as well, where the layer draws (the brief's item 3). */
const LOW_VIEWS = new Set(["ground-up-45", "ground-horizon", "below-base-500m", "within-deck"]);

/** A place's latitude and longitude, degrees. */
interface Place {
  readonly latitudeDeg: number;
  readonly longitudeDeg: number;
}

/** A place's local frame: its up, east and north, body-fixed, unit. */
function localFrameAt(place: Place): {
  readonly up: Vec3;
  readonly east: Vec3;
  readonly north: Vec3;
} {
  const phi = place.latitudeDeg * RAD;
  const lambda = place.longitudeDeg * RAD;
  const up = {
    x: Math.cos(phi) * Math.cos(lambda),
    y: Math.cos(phi) * Math.sin(lambda),
    z: Math.sin(phi),
  };
  const raw = cross({ x: 0, y: 0, z: 1 }, up);
  const east = scale(raw, 1 / norm(raw));
  return { up, east, north: cross(up, east) };
}

function dot3(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

/** The sun's azimuth from north at a place, degrees. */
function sunAzimuthDeg(place: Place): number {
  const { east, north } = localFrameAt(place);
  return Math.atan2(dot3(SUN_DIRECTION, east), dot3(SUN_DIRECTION, north)) / RAD;
}

/** A direction at an elevation and an azimuth from north, degrees, in a place's frame. */
function towards(place: Place, elevationDeg: number, azimuthDeg: number): Vec3 {
  const { up, east, north } = localFrameAt(place);
  const c = Math.cos(elevationDeg * RAD);
  const s = Math.sin(elevationDeg * RAD);
  const ca = Math.cos(azimuthDeg * RAD);
  const sa = Math.sin(azimuthDeg * RAD);
  return {
    x: c * (ca * north.x + sa * east.x) + s * up.x,
    y: c * (ca * north.y + sa * east.y) + s * up.y,
    z: c * (ca * north.z + sa * east.z) + s * up.z,
  };
}

/** The place the scenes fall back on where no latitude's clouds are broken: 22° N, 8.2° E. */
const FALLBACK_PLACE: Place = { latitudeDeg: 22, longitudeDeg: 8.2 };

/**
 * The scenes' place (the module's remarks): on the circle 55° from the subsolar point, west of it,
 * the first latitude from 15° N whose clouds about it are broken, by the field's twin as the
 * renderer makes it at the scenes' time: its share at a 300 m footprint over 5 × 5 points 3 km apart
 * of mean 0.3–0.7, under 0.1 and over 0.9 somewhere.
 */
function scenePlace(): Place {
  const coverage = seasonCoverage({ subsolarLatitudeRad: 0 }, EARTH_CLOUDS.cloudFraction);
  const condensate = layerCondensateOf(EARTH_CLOUDS);
  if (condensate === null) {
    return FALLBACK_PLACE;
  }
  const params = cloudFieldParams({
    seed: cloudSeedOf(BODY),
    condensate,
    radiusM: EARTH_MEAN_RADIUS_M,
    time: { seconds: SCENE_TIME_S, nanos: 0 },
    rate: 1,
    windMps: largestWindMps(coverage),
    octaves: 12,
  });
  const noise = cloudNoiseTexels();
  const shellM = EARTH_MEAN_RADIUS_M + cloudLayerHeightM(condensate);
  const cosine = Math.cos((90 - SUN_ELEVATION_DEG) * RAD);
  for (let latitudeDeg = 15; latitudeDeg <= 28; latitudeDeg += 0.5) {
    const c = Math.cos(latitudeDeg * RAD);
    const place = {
      latitudeDeg,
      longitudeDeg: 60 - Math.acos(Math.min(Math.max(cosine / c, -1), 1)) / RAD,
    };
    const { up, east, north } = localFrameAt(place);
    let sum = 0;
    let least = 1;
    let most = 0;
    for (let i = -2; i <= 2; i += 1) {
      for (let j = -2; j <= 2; j += 1) {
        const p = {
          x: shellM * up.x + 3_000 * (i * east.x + j * north.x),
          y: shellM * up.y + 3_000 * (i * east.y + j * north.y),
          z: shellM * up.z + 3_000 * (i * east.z + j * north.z),
        };
        const f = cloudFieldAt(coverage, noise, params, p, 300).fraction;
        sum += f;
        least = Math.min(least, f);
        most = Math.max(most, f);
      }
    }
    if (sum / 25 >= 0.3 && sum / 25 <= 0.7 && least < 0.1 && most > 0.9) {
      return place;
    }
  }
  return FALLBACK_PLACE;
}

/** A Lambert disc's photometry at `albedo`, R07's provisional law's. */
function photometryAt(albedo: number): BodyPhotometry {
  const p: Rgb = [albedo, albedo, albedo];
  const q: Rgb = [1.5, 1.5, 1.5];
  return {
    ...PROVISIONAL_PHOTOMETRY,
    geometricAlbedo: p,
    phaseIntegral: q,
    law: lawFor(p, q, "lambert"),
  };
}

/** The frame of one view: the camera at the world's origin, the body's centre beneath it. */
function frameOf(
  view: SceneView,
  place: Place,
  tables: AtmosphereTables,
  setting: QualitySetting,
): PhotorealFrame {
  const { up, north } = localFrameAt(place);
  const placeM = scale(up, EARTH_MEAN_RADIUS_M + view.heightM);
  const centreM = scale(placeM, -1);
  const forward = towards(place, view.pitchDeg, sunAzimuthDeg(place) + view.azimuthFromSunDeg);
  // Straight down, the frame's up is the place's north.
  const orientation = lookAlong(forward, Math.abs(view.pitchDeg) > 89 ? north : up);
  const fovXRad = view.fovXDeg * RAD;
  return {
    camera: { orientation, fovXRad },
    viewport: VIEWPORT,
    presentedPx: VIEWPORT,
    role: "eye",
    setting,
    exposureScale: EXPOSURE_SCALE,
    time: { seconds: SCENE_TIME_S, nanos: 0 },
    timeRate: 1,
    sky: [],
    starSprites: [],
    hostDraws: new Map(),
    glareSources: [],
    lights: [
      {
        disc: sunLikeHostDisc(),
        centreM: {
          x: centreM.x + SUN_DIRECTION.x * AU_M,
          y: centreM.y + SUN_DIRECTION.y * AU_M,
          z: centreM.z + SUN_DIRECTION.z * AU_M,
        },
      },
    ],
    bodies: [
      {
        id: BODY,
        centreM,
        figure: {
          equatorialRadiusM: EARTH_MEAN_RADIUS_M,
          polarRadiusM: EARTH_MEAN_RADIUS_M,
          pole: null,
        },
        photometry: photometryAt(0.18),
        lighting: undefined,
      },
    ],
    depthWriters: [],
    previousRegimes: new Map(),
    overlay: null,
    meter: null,
    atmospheres: [
      {
        id: BODY,
        tables,
        camera: { positionM: placeM, orientation, fovXRad, viewport: VIEWPORT },
        toBodyFixed: { x: 0, y: 0, z: 0, w: 1 },
        clouds: EARTH_CLOUDS,
      },
    ],
  };
}

/**
 * Each view drawn at each of its settings through one renderer prepared on high, its frames
 * settled after a cut of the last view's history, read back as a capture.
 */
async function scenesOn(
  engine: RenderEngine,
  views: ReadonlyArray<{ readonly view: SceneView; readonly setting: QualitySetting }>,
  place: Place,
): Promise<CapturedImage[]> {
  const images: CapturedImage[] = [];
  const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);
  const renderer = new PhotorealRenderer(engine, "cloud scenes");
  const canvas = engine.createView(addCanvas(), "cloud scenes");
  canvas.resize(VIEWPORT);
  try {
    const first = views[0];
    if (first === undefined) {
      return images;
    }
    await renderer.prepare(
      VIEWPORT,
      "high",
      "eye",
      frameOf(first.view, place, tables, first.setting).camera,
    );
    for (const { view, setting } of views) {
      const frame = frameOf(view, place, tables, setting);
      renderer.prepareClouds(frame.atmospheres);
      renderer.dropHistograms();
      for (let at = 0; at < SCENE_FRAMES; at += 1) {
        renderer.render(canvas, {
          ...frame,
          time: { seconds: SCENE_TIME_S, nanos: Math.round((at * 1e9) / 60) },
        });
      }
      const drawn = renderer.frameColours?.drawn ?? null;
      if (drawn === null) {
        throw new Error(`the cloud scene ${view.name} left no frame to read`);
      }
      // The harness's scenes run in order: each reads the GPU back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = halfTexels(await engine.readTexture(drawn));
      if (!texels.every(Number.isFinite)) {
        throw new Error(`the cloud scene ${view.name} on ${setting} drew a texel not finite`);
      }
      images.push(hdrCapture(`cloud-scene-${view.name}-${setting}`, texels, VIEWPORT, 1, 1));
    }
  } finally {
    renderer.dispose();
    canvas.dispose();
    tables.dispose();
  }
  return images;
}

/** The clouds' scenes (the module's remarks): every view on high, some on low, at the place found. */
export function captureCloudScenes(engine: RenderEngine): Promise<CapturedImage[]> {
  const views = [
    ...VIEWS.map((view) => ({ view, setting: "high" as const })),
    ...VIEWS.filter((view) => LOW_VIEWS.has(view.name)).map((view) => ({
      view,
      setting: "low" as const,
    })),
  ];
  return scenesOn(engine, views, scenePlace());
}
