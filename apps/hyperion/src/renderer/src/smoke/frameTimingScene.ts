/**
 * The scenes of R08.T11's photorealistic frame's timings (plan R08, R08.T11; `frameTiming.ts`
 * draws them): pure geometry and the source arithmetic, no GPU.
 *
 * @remarks
 * Each far-limb frame is R08.T8.c's timed camera (`atmosphere.ts`' `farLimbTimingCamera`, at
 * `FAR_LIMB_TIMING_PLACES`' five distances) over an Earth on WGS 84 whose air the frame draws, its
 * body-fixed axes the frame's own (identity), the camera at the origin and the Earth's centre at
 * minus the camera's position. Its sources are scene geometry, so that `PhotorealRenderer`'s plan
 * (`bodyLightSources`) and `skySources` make them as VIEW's would:
 *
 * - the Sun, a Sun-like host disc at 1 au in T8.c's timed sun's direction (30° from the zenith at
 *   the limb's tangent point, 90° right of the view, `limbSunToward`), 90° from the view's axis;
 * - a binary's second star: a Sun-like disc at √10 au, a tenth of the Sun's illuminance, its
 *   direction the Sun's turned 40° about the pole (+z), as R08.T8.e's `sourcesOf` turns it;
 * - giants whose planetshine lights the Earth: Jupiter-like spheroids
 *   ({@link GIANT_EQUATORIAL_RADIUS_M}) six of their radii from the Earth, as a moon of a giant
 *   stands, each at a 60° phase (Sun–giant–Earth), behind the camera's view: the first in the
 *   plane of the Sun and the view's axis, the second turned 60° from it about the anti-solar axis.
 *   So each is out of the view, above the camera's horizon dip (the twilight skip keeps it with
 *   20° to spare and more), a hundredth of the Sun's illuminance at the camera (past
 *   `STAR_CUT_RELATIVE` 20 times over, and under the second star's tenth, so that the march's two
 *   stay the stars), 120° from the Sun (no eclipse of the Sun for the Earth) and 60° from the
 *   anti-solar axis (none of them in the Earth's shadow).
 *
 * The sets ({@link sourceSetsOf}): `x1` the Sun; `x2` a binary's two light slots; `x2p` the Sun and
 * one giant's planetshine, both on slot 0; and the setting's whole count, `x4` on high (two stars
 * and two giants, `SKY_SUN_CAP` and `PLANETSHINE_SOURCES_HIGH`), `x3` on low (one giant,
 * `PLANETSHINE_SOURCES_LOW`). The small bodies' frames ({@link smallEarthScene}) are R08.T10.b's
 * (`atmosphereFrame.ts`): the Earth whole at a diameter in pixels, the Sun 40° off the line of
 * sight.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { type BodyPhotometry, PROVISIONAL_PHOTOMETRY } from "../view/appearance/fromWire";
import { lawFor } from "../view/appearance/phase";
import {
  type AtmosphereCamera,
  marchSourcesOf,
  type SpheroidFigure,
  type SunState,
} from "../view/atmosphere/hillaire";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import {
  type SkySkip,
  type SkySource,
  type SkySourceKind,
  skySourcesWithSkips,
} from "../view/atmosphere/sources";
import type { BodyLightSources, LitBodyInput } from "../view/bodies/draw";
import { DEFAULT_FOV_DEG, type ProjectionCamera, type Viewport } from "../view/camera/projection";
import { IDENTITY_QUATERNION, rotate } from "../view/camera/quaternion";
import { sunLikeHostDisc } from "../view/lighting/hostDisc";
import type { PlacedLight } from "../view/lighting/hostLights";
import { photopicIlluminance } from "../view/lighting/illuminance";
import type { Rgb } from "../view/photometry/toneCurve";
import { type QualitySetting, SETTINGS } from "../view/quality/qualitySetting";
import { AU_M } from "../view/scenes/kept";
import {
  FAR_LIMB_TIMING_FRAME,
  FAR_LIMB_TIMING_SUN_AZIMUTH_DEG,
  FAR_LIMB_TIMING_SUN_ZENITH_DEG,
  farLimbTimingCamera,
  limbSunToward,
  SLICES_EARTH,
} from "./atmosphere";

/**
 * A timed set's name: `x1` the Sun, `x2` a binary's two stars, `x2p` the Sun and a giant's
 * planetshine, and the setting's whole count, two stars and its giants (`x3` on low, `x4` on high).
 */
export type SourceSetName = "x1" | "x2" | "x2p" | "x3" | "x4";

/** One timed set of sources: its name, its stars and its giants. */
export interface SourceSet {
  readonly name: SourceSetName;
  /** The stars: the Sun alone, or the Sun and a binary's second star. */
  readonly stars: 1 | 2;
  /** The giants whose planetshine lights the Earth: none, up to the setting's count. */
  readonly giants: number;
}

/**
 * The whole count's name for a binary's two stars and `giants` planetshine sources.
 *
 * @throws RangeError for a count the sets have no name for (a setting of more than two sources of
 *   planetshine, or none).
 */
function wholeCountName(giants: number): SourceSetName {
  switch (giants) {
    case 1:
      return "x3";
    case 2:
      return "x4";
    default:
      throw new RangeError(`the timed sets name two stars and 1 or 2 giants, not ${giants}`);
  }
}

/**
 * A setting's timed sets: `x1`, `x2`, `x2p`, and the setting's whole count, two stars and
 * `SETTINGS[s].photoreal.planetshineSources` giants (`x4` on high, `x3` on low).
 *
 * @throws RangeError as {@link wholeCountName}.
 */
export function sourceSetsOf(setting: QualitySetting): readonly SourceSet[] {
  const giants = SETTINGS[setting].photoreal.planetshineSources;
  return [
    { name: "x1", stars: 1, giants: 0 },
    { name: "x2", stars: 2, giants: 0 },
    { name: "x2p", stars: 1, giants: 1 },
    { name: wholeCountName(giants), stars: 2, giants },
  ];
}

/** The binary's second star's turn from the Sun about the pole (+z), rad: 40° (R08.T8.e's). */
export const COMPANION_TURN_RAD = (40 * Math.PI) / 180;

/**
 * The binary's second star's distance from the Earth, m: √10 au, so that a Sun-like disc there
 * gives a tenth of the Sun's illuminance at 1 au.
 */
export const COMPANION_DISTANCE_M = Math.sqrt(10) * AU_M;

/**
 * The giants' equatorial radius, m: Jupiter's at 1 bar, 71,492 km (IAU WGCCRE 2015, Archinal et
 * al. 2018, Celest. Mech. Dyn. Astron. 130, 22, Table 4).
 */
export const GIANT_EQUATORIAL_RADIUS_M = 7.1492e7;

/** The giants' polar radius, m: Jupiter's at 1 bar, 66,854 km (the same table). */
export const GIANT_POLAR_RADIUS_M = 6.6854e7;

/**
 * The giants' distance from the Earth's centre, m: six of their equatorial radii, the Earth as a
 * moon of a giant stands (Io at 5.9 Jupiter radii).
 */
export const GIANT_DISTANCE_M = 6 * GIANT_EQUATORIAL_RADIUS_M;

/** The giants' phase angle at the Earth, Sun–giant–Earth, rad: 60°, a gibbous giant. */
export const GIANT_PHASE_RAD = Math.PI / 3;

/** The second giant's turn from the first about the anti-solar axis, rad: 60°. */
export const GIANT_TURN_RAD = Math.PI / 3;

/**
 * The giants' geometric albedo: 0.5, near Jupiter's visual 0.54 (Mallama, Krobusek and Pavlov
 * 2017, Icarus 282, 19), on a Lambert sphere's law (q 1.5): a timing scene's, not Jupiter's
 * photometry.
 */
export const GIANT_GEOMETRIC_ALBEDO = 0.5;

/** A Lambert sphere's phase integral: 3 ÷ 2, so that its Bond albedo is 1.5 p. */
const LAMBERT_PHASE_INTEGRAL = 1.5;

/** The giants' ids, in their order. */
export const GIANT_IDS: readonly BodyIdHex[] = ["0200080020000000.0321", "0200080020000000.0322"];

/** The Sun's disc, R06's Sun-like host. */
const SUN_DISC = sunLikeHostDisc();

/** The second star's disc: the same Sun-like host as star 1. */
const COMPANION_DISC = sunLikeHostDisc({ star: 1 });

/** The pole of every figure here, the frame's +z (identity body-fixed axes). */
const POLE: Vec3 = vec3(0, 0, 1);

/** A Lambert sphere's photometry of geometric albedo `p` per channel, q 1.5 (R07's `lambert`). */
export function lambertPhotometry(p: Rgb): BodyPhotometry {
  const q: Rgb = [LAMBERT_PHASE_INTEGRAL, LAMBERT_PHASE_INTEGRAL, LAMBERT_PHASE_INTEGRAL];
  return {
    ...PROVISIONAL_PHOTOMETRY,
    geometricAlbedo: p,
    phaseIntegral: q,
    law: lawFor(p, q, "lambert"),
  };
}

/**
 * The Earth as the frame's lit body: WGS 84 about +z (`SLICES_EARTH.figure`), a Lambert sphere of
 * `medium`'s ground albedo (p = A ÷ 1.5, q 1.5, as `atmosphereFrame.ts`' disc), a static scene's
 * lighting (the frame's hosts and bodies light it).
 *
 * @param centreM - Its centre from the camera, m.
 */
export function earthInput(
  id: BodyIdHex,
  medium: Pick<AtmosphereMedium, "groundAlbedo">,
  centreM: Vec3,
): LitBodyInput {
  const [r, g, b] = medium.groundAlbedo;
  return {
    id,
    centreM,
    figure: { ...SLICES_EARTH.figure, pole: POLE },
    photometry: lambertPhotometry([
      r / LAMBERT_PHASE_INTEGRAL,
      g / LAMBERT_PHASE_INTEGRAL,
      b / LAMBERT_PHASE_INTEGRAL,
    ]),
    lighting: undefined,
  };
}

/** A giant of the scene at `centreM` from the camera: {@link GIANT_EQUATORIAL_RADIUS_M}, Lambert. */
function giantInput(id: BodyIdHex, centreM: Vec3): LitBodyInput {
  const p = GIANT_GEOMETRIC_ALBEDO;
  return {
    id,
    centreM,
    figure: {
      equatorialRadiusM: GIANT_EQUATORIAL_RADIUS_M,
      polarRadiusM: GIANT_POLAR_RADIUS_M,
      pole: POLE,
    },
    photometry: lambertPhotometry([p, p, p]),
    lighting: undefined,
  };
}

/** `v` turned about the pole (+z) by `angleRad`, from +x towards +y. */
function aboutPole(v: Vec3, angleRad: number): Vec3 {
  const c = Math.cos(angleRad);
  const s = Math.sin(angleRad);
  return vec3(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
}

/**
 * The giants' directions from the Earth's centre: each {@link GIANT_PHASE_RAD} from the anti-solar
 * axis, so that the giant's phase at the Earth is that angle; the first towards the back of the view
 * in the plane of the Sun and the view's axis, the second turned {@link GIANT_TURN_RAD} from it about
 * the anti-solar axis, towards `sun × forward`.
 *
 * @param sun - The Sun's unit direction from the Earth.
 * @param forward - The view's unit axis.
 */
export function giantDirections(sun: Vec3, forward: Vec3): readonly [Vec3, Vec3] {
  const anti = scale(sun, -1);
  // The view's axis, less its part along the Sun, reversed: away from the view.
  const back = normalise(sub(scale(sun, dot(forward, sun)), forward));
  const side = normalise(cross(sun, scale(back, -1)));
  const c = Math.cos(GIANT_PHASE_RAD);
  const s = Math.sin(GIANT_PHASE_RAD);
  const turned = add(scale(back, Math.cos(GIANT_TURN_RAD)), scale(side, Math.sin(GIANT_TURN_RAD)));
  return [
    normalise(add(scale(anti, c), scale(back, s))),
    normalise(add(scale(anti, c), scale(turned, s))),
  ];
}

/** A frame of the scene: its cameras, the Earth's place, its lights and its giants. */
export interface FrameScene {
  /**
   * The Earth's atmosphere camera: from its centre, in its body-fixed axes, which are the frame's
   * (`FrameAtmosphere.camera`).
   */
  readonly air: AtmosphereCamera;
  /** The frame's camera, at the origin: the air camera's orientation and field. */
  readonly camera: ProjectionCamera;
  /** The Earth's centre from the camera, m. */
  readonly earthCentreM: Vec3;
  /** The hosts at their centres from the camera: the Sun, and the second star in a binary's sets. */
  readonly lights: ReadonlyArray<PlacedLight>;
  /** The set's giants, lit bodies of the frame. */
  readonly giants: ReadonlyArray<LitBodyInput>;
}

/**
 * The far limb's frame at `distanceM` from the Earth's centre on `viewport`, for `set`: R08.T8.c's
 * camera (`farLimbTimingCamera`) and sun (`limbSunToward`, 30° from the zenith at the tangent
 * point, 90° right of the view), the set's second star and giants placed as the module's remarks
 * say.
 */
export function farLimbFrameScene(
  distanceM: number,
  viewport: Viewport,
  set: SourceSet,
): FrameScene {
  const air = farLimbTimingCamera(distanceM, viewport);
  const earthCentreM = scale(air.positionM, -1);
  const sun = limbSunToward(
    distanceM,
    SLICES_EARTH.figure,
    FAR_LIMB_TIMING_SUN_ZENITH_DEG,
    FAR_LIMB_TIMING_SUN_AZIMUTH_DEG,
    FAR_LIMB_TIMING_FRAME,
  ).directionBodyFixed;
  const forward = rotate(air.orientation, vec3(0, 0, -1));
  const lights: PlacedLight[] = [{ disc: SUN_DISC, centreM: add(earthCentreM, scale(sun, AU_M)) }];
  if (set.stars === 2) {
    const toward = aboutPole(sun, COMPANION_TURN_RAD);
    lights.push({
      disc: COMPANION_DISC,
      centreM: add(earthCentreM, scale(toward, COMPANION_DISTANCE_M)),
    });
  }
  const giants = giantDirections(sun, forward)
    .slice(0, set.giants)
    .flatMap((direction, i) => {
      const id = GIANT_IDS[i];
      return id === undefined
        ? []
        : [giantInput(id, add(earthCentreM, scale(direction, GIANT_DISTANCE_M)))];
    });
  return {
    air,
    camera: { orientation: air.orientation, fovXRad: air.fovXRad },
    earthCentreM,
    lights,
    giants,
  };
}

/** The Sun's direction from the small Earth: 40° off the line of sight, towards the camera's +x. */
const SMALL_SUN_OFF_RAD = (40 * Math.PI) / 180;

/**
 * The small air bodies' frame (plan R08, R08.T11's (C)): the Earth on `figure` seen whole, centred
 * in `viewport` at the view's 60° across, `diameterPx` across its equator (`atmosphereFrame.ts`'
 * reckoning, the pixel's angle at the centre), lit by the Sun 40° off the line of sight; the
 * camera's axes the Earth's body-fixed ones.
 */
export function smallEarthScene(
  diameterPx: number,
  viewport: Viewport,
  figure: SpheroidFigure,
): Omit<FrameScene, "giants"> {
  const fovXRad = (DEFAULT_FOV_DEG * Math.PI) / 180;
  const pxPerRad = viewport.widthPx / (2 * Math.tan(fovXRad / 2));
  const distanceM = figure.equatorialRadiusM / Math.sin(diameterPx / 2 / pxPerRad);
  const earthCentreM = vec3(0, 0, -distanceM);
  const towards = vec3(Math.sin(SMALL_SUN_OFF_RAD), 0, Math.cos(SMALL_SUN_OFF_RAD));
  return {
    air: {
      positionM: vec3(0, 0, distanceM),
      orientation: IDENTITY_QUATERNION,
      fovXRad,
      viewport,
    },
    camera: { orientation: IDENTITY_QUATERNION, fovXRad },
    earthCentreM,
    lights: [{ disc: SUN_DISC, centreM: add(earthCentreM, scale(towards, AU_M)) }],
  };
}

/** A source as a set means it: its array and its light slot. */
export interface ExpectedSource {
  readonly kind: SkySourceKind;
  readonly curveSlot: number;
}

/** The sources `set` means the sky to draw, stars first: each star on its own slot, planetshine on 0. */
export function expectedSources(set: SourceSet): ExpectedSource[] {
  const stars = Array.from({ length: set.stars }, (_, i) => ({
    kind: "star" as const,
    curveSlot: i,
  }));
  const giants = Array.from({ length: set.giants }, () => ({
    kind: "planetshine" as const,
    curveSlot: 0,
  }));
  return [...stars, ...giants];
}

/**
 * The sources `set` means the march to take under a cap of `cap`: the brightest, which the scene
 * makes the stars before any giant (x1 the Sun, x2 the two stars, x2p the Sun and the giant, the
 * whole count the two stars).
 */
export function expectedMarch(set: SourceSet, cap: number): ExpectedSource[] {
  return expectedSources(set).slice(0, cap);
}

/** Why a source is left out of the sky: Design note 7's skips, or a light slot the tables lack. */
export type SourceLeftOut = NonNullable<SkySkip> | "no light slot";

/** A body's sky sources as the frame draws them: kept, left out and why, and the march's. */
export interface ListedSources {
  /** Past Design note 7's skips and the tables' light slots, in `skySources`' order. */
  readonly kept: ReadonlyArray<SkySource>;
  /** Each source left out, with its skip, or `no light slot` for a star past the tables' slots. */
  readonly left: ReadonlyArray<{ readonly source: SkySource; readonly why: SourceLeftOut }>;
  /** The kept sources the ray march takes (`marchSourcesOf` at the cap), in their order. */
  readonly march: ReadonlyArray<SkySource>;
}

/**
 * A body's sources (`BodyFramePlan.sources`) as `PhotorealRenderer` draws its sky: through
 * `skySourcesWithSkips` with the atmosphere camera, the figure and the medium (identity body-fixed
 * axes), less any star on a light slot past `lightSlots` (`Infinity` for a medium with no
 * absorber), and the march's `cap` of them.
 */
export function listSources(
  sources: BodyLightSources,
  air: Pick<AtmosphereCamera, "positionM">,
  figure: SpheroidFigure,
  medium: Pick<AtmosphereMedium, "topHeightM">,
  lightSlots: number,
  cap: number,
): ListedSources {
  const listed = skySourcesWithSkips(
    sources.stars,
    sources.planetshine,
    air,
    figure,
    medium,
    IDENTITY_QUATERNION,
  );
  const kept: SkySource[] = [];
  const left: Array<{ readonly source: SkySource; readonly why: SourceLeftOut }> = [];
  for (const { source, skip } of listed) {
    if (skip !== null) {
      left.push({ source, why: skip });
    } else if (source.curveSlot >= lightSlots) {
      left.push({ source, why: "no light slot" });
    } else {
      kept.push(source);
    }
  }
  const marched = new Set<SunState>(marchSourcesOf(kept, cap));
  return { kept, left, march: kept.filter((source) => marched.has(source)) };
}

/** Whether `sources` are `expected`, kind and slot, in order. */
function matches(
  sources: ReadonlyArray<SkySource>,
  expected: ReadonlyArray<ExpectedSource>,
): boolean {
  return (
    sources.length === expected.length &&
    sources.every((source, i) => {
      const meant = expected[i];
      return (
        meant !== undefined && source.kind === meant.kind && source.curveSlot === meant.curveSlot
      );
    })
  );
}

/** Whether the frame draws exactly `set`'s sources, and marches the set's under `cap`. */
export function sourcesAsSet(listed: ListedSources, set: SourceSet, cap: number): boolean {
  return (
    matches(listed.kept, expectedSources(set)) && matches(listed.march, expectedMarch(set, cap))
  );
}

/** One source as the checks name it: `star 1 (slot 1)`. */
function nameOf(source: SkySource): string {
  return `${source.kind} ${String(source.index)} (slot ${String(source.curveSlot)})`;
}

/** The listing as a check's detail: the kept, the left out and why, the march's. */
export function describeSources(listed: ListedSources): string {
  const kept = listed.kept.map(nameOf).join(", ");
  const left =
    listed.left.length === 0
      ? "none"
      : listed.left.map(({ source, why }) => `${nameOf(source)}: ${why}`).join(", ");
  const march = listed.march.map(nameOf).join(", ");
  return `kept ${kept === "" ? "none" : kept}; left out ${left}; march ${march === "" ? "none" : march}`;
}

/**
 * The faintest kept planetshine source's photopic illuminance at the camera over the brightest
 * kept star's, or `null` with no planetshine kept.
 */
export function planetshineShare(listed: ListedSources): number | null {
  const star = Math.max(
    0,
    ...listed.kept
      .filter((source) => source.kind === "star")
      .map((source) => photopicIlluminance(source.illuminanceLx)),
  );
  const shine = listed.kept
    .filter((source) => source.kind === "planetshine")
    .map((source) => photopicIlluminance(source.illuminanceLx));
  return shine.length === 0 || !(star > 0) ? null : Math.min(...shine) / star;
}

/** The angle between two directions, rad. */
export function angleBetweenRad(a: Vec3, b: Vec3): number {
  const c = dot(a, b) / (norm(a) * norm(b));
  return Math.acos(Math.min(1, Math.max(-1, c)));
}
