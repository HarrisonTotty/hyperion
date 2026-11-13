/**
 * The descent's light (plan R10, R10.T14.c, the owner's request): where the star stands over a
 * generated world's descent, so that it is flown on the world's lit side.
 *
 * @remarks
 * - **The test planet** keeps T13.b's sun, at the equinox over the site's meridian at touchdown
 *   (`spikeScene.ts`' `sunDirectionBody`), so that its scene is the one T13.b built, bit for bit.
 * - **`TERRAIN TEST`'s moon**, whose star the spike places too, took the same noon until this
 *   request. Its track's start lies 147° of the moon's arc back from the site, and that noon left
 *   it 29° to 83° below the horizon over the survey's middle rows, approached from each quarter:
 *   the run opened in the dark. Its star stands instead on the great circle through the site at
 *   touchdown and the track's start at the script's start ({@link keptStarDirection}): the site
 *   sees it 20° to 28° high there, and the whole track 8° or more.
 * - **A served world** is lit by its own star, its direction from the body's centre at the push
 *   (`ServedBody.towardStarM`), which the body's spin turns under it. Its site is drawn among the
 *   surveyed cells the star stands {@link SITE_STAR_ELEVATION_DEG} above at both the descent's
 *   start and its end ({@link litSiteCandidates}), and approached away from the star
 *   ({@link litBodySite}), so that the track runs back towards the sub-stellar point. A tidally
 *   locked body, whose star never moves over its ground, is so flown over its day hemisphere.
 *
 * Elevations are taken from the spheroid's normal over a unit direction d of the datum point M·d,
 * as the profile lays its track out, never the geocentric direction.
 */

import { add, dot, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { acos, asin, atan2, cos, sin } from "../../lib/pinnedMath";
import { rotateToBody, rotateToBodyFixed } from "../coords/rotation";
import { PATCH_QUADS, vertexDir, type Xyz } from "../terrain/cube";
import type { PatchKey } from "../terrain/patchKey";
import { type BodyFigure, spheroidNormal } from "../terrain/planet";
import type { BodySite } from "./bodyDescent";
import type { DescentProfile } from "./descentProfile";
import type { SpikeWorld } from "./spikeWorld";

/**
 * The star's elevation over the landing site, degrees, through the descent: high enough that the
 * ground is plainly lit, low enough that its relief casts shadows (the owner's 20–45°).
 */
export const SITE_STAR_ELEVATION_DEG = { lowest: 20, highest: 45 } as const;

/** The least elevation the star is put at over the track's start where the site's band allows, degrees. */
export const START_STAR_ELEVATION_DEG = 10;

const RAD = Math.PI / 180;

/** `x` within [lo, hi]. */
function clamp(x: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, x));
}

/** A triple as a vector. */
function vecOf([x, y, z]: Xyz): Vec3 {
  return vec3(x, y, z);
}

/**
 * The star's elevation over the datum point of unit direction `dir`, rad, its unit direction `star`
 * in the same axes: from the spheroid's normal there.
 */
export function starElevationRad(figure: BodyFigure, dir: Vec3, star: Vec3): number {
  const normal = vecOf(spheroidNormal(figure, [dir.x, dir.y, dir.z]));
  return asin(clamp(dot(normal, star), -1, 1));
}

/**
 * The star's unit direction in the body's non-rotating axes over a kept world's descent other than
 * the test planet's: on the great circle through the site at the script's end and the track's start
 * at its start, the arc a from the site towards the start.
 *
 * @remarks
 * a = clamp(θ − (90° − 10°), 90° − 45°, 90° − 20°) for the arc θ between the two: the site sees the
 * star at 45° where the start then sees it {@link START_STAR_ELEVATION_DEG} or more, else lower,
 * down to 20°. The track's points between see it higher than its ends, an arc under 180° (TERRAIN
 * TEST's 147°) holding no point below both. `TEST MOON`'s 5° turn over the script is in θ.
 */
export function keptStarDirection(profile: DescentProfile, world: SpikeWorld): Vec3 {
  const site = rotateToBody(world.rotationAt(profile.durationS), profile.siteDir);
  const start = rotateToBody(world.rotationAt(0), profile.poseAt(0).groundDir);
  const along = dot(site, start);
  const arcRad = acos(clamp(along, -1, 1));
  const back = normalise(sub(start, scale(site, along)));
  const a = clamp(
    arcRad - (90 - START_STAR_ELEVATION_DEG) * RAD,
    (90 - SITE_STAR_ELEVATION_DEG.highest) * RAD,
    (90 - SITE_STAR_ELEVATION_DEG.lowest) * RAD,
  );
  return normalise(add(scale(site, cos(a)), scale(back, sin(a))));
}

/** A served world's star as its site's draw reads it. */
export interface DescentStar {
  readonly figure: BodyFigure;
  /** The star's unit direction along the body-fixed axes at the descent's start and at its end. */
  readonly atStart: Vec3;
  readonly atEnd: Vec3;
  /** The track's arc from its start to the site, rad (`DescentProfile.trackArcRad`). */
  readonly trackArcRad: number;
}

/**
 * The star over `world`'s descent for its site's draw, or `null` over a world whose star the spike
 * places itself (`SpikeWorld.starDirection` absent).
 *
 * @param datum - Any profile over the world's figure: the script's duration and track are the
 *   table's, whatever the site.
 */
export function descentStarOf(world: SpikeWorld, datum: DescentProfile): DescentStar | null {
  const star = world.starDirection;
  if (star === undefined) {
    return null;
  }
  return {
    figure: world.figure,
    atStart: rotateToBodyFixed(world.rotationAt(0), star),
    atEnd: rotateToBodyFixed(world.rotationAt(datum.durationS), star),
    trackArcRad: datum.trackArcRad,
  };
}

/** The profile's north and east at a site of unit direction `dir`, as `DescentProfile` forms them. */
function northEast(dir: Vec3): { readonly north: Vec3; readonly east: Vec3 } {
  const lat = asin(clamp(dir.z, -1, 1));
  const lon = atan2(dir.y, dir.x);
  return {
    north: vec3(-sin(lat) * cos(lon), -sin(lat) * sin(lon), cos(lat)),
    east: vec3(-sin(lon), cos(lon), 0),
  };
}

/**
 * The approach's azimuth at a site of unit direction `dir` that flies away from the star, rad
 * clockwise from north: the track's start then lies back towards the sub-stellar point.
 */
export function downStarAzimuthRad(dir: Vec3, star: Vec3): number {
  const { north, east } = northEast(dir);
  return atan2(-dot(star, east), -dot(star, north));
}

/** The unit direction of the track's start from a site of unit direction `dir`, as the profile lays it. */
function trackStartDir(dir: Vec3, azimuthRad: number, arcRad: number): Vec3 {
  const { north, east } = northEast(dir);
  const heading = add(scale(north, cos(azimuthRad)), scale(east, sin(azimuthRad)));
  return add(scale(dir, cos(arcRad)), scale(heading, -sin(arcRad)));
}

/** The largest arc from a cell's centre to its corners, rad: how far any of its points lies from it. */
function cellReachRad(cell: PatchKey, centre: Vec3): number {
  let least = 1;
  for (const [x, y] of [
    [0, 0],
    [PATCH_QUADS, 0],
    [0, PATCH_QUADS],
    [PATCH_QUADS, PATCH_QUADS],
  ] as const) {
    least = Math.min(least, dot(centre, vecOf(vertexDir(cell, x, y))));
  }
  return acos(clamp(least, -1, 1));
}

/**
 * The candidate cells a served world's site may be drawn in: those whose every point the star
 * stands {@link SITE_STAR_ELEVATION_DEG} above at the descent's start and its end, and of them,
 * where any, those whose track's start, approached away from the star ({@link
 * downStarAzimuthRad}) from the cell's centre, sees it {@link START_STAR_ELEVATION_DEG} or more
 * high at the start; in the candidates' order.
 *
 * @remarks
 * Each band is narrowed by the cell's reach from its centre ({@link cellReachRad}), so that a site
 * anywhere in it keeps the band. The track's start is lit so where its arc is under 150° or from
 * 325° to 510° (a body of more than about 1,700 km in radius, or of about 500 to 790 km). Between,
 * no site in the band lights it, and the first set is kept: the descent starts over the night side
 * and flies into the light, 160 s into the script's 1,230 s on a body of 1,200 km (an arc of 213°),
 * early in the 900 s descent arc.
 */
export function litSiteCandidates(
  candidates: ReadonlyArray<PatchKey>,
  star: DescentStar,
): ReadonlyArray<PatchKey> {
  const lowest = SITE_STAR_ELEVATION_DEG.lowest * RAD;
  const highest = SITE_STAR_ELEVATION_DEG.highest * RAD;
  const lit: { readonly cell: PatchKey; readonly startRad: number; readonly reachRad: number }[] =
    [];
  for (const cell of candidates) {
    const centre = vecOf(vertexDir(cell, PATCH_QUADS / 2, PATCH_QUADS / 2));
    const elevations = [star.atStart, star.atEnd].map((at) =>
      starElevationRad(star.figure, centre, at),
    );
    // The centre's band first, which most cells miss, then the band narrowed by the cell's reach.
    const within = (reachRad: number): boolean =>
      elevations.every((rad) => rad >= lowest + reachRad && rad <= highest - reachRad);
    if (!within(0)) {
      continue;
    }
    const reachRad = cellReachRad(cell, centre);
    if (within(reachRad)) {
      const azimuth = downStarAzimuthRad(centre, star.atStart);
      const start = trackStartDir(centre, azimuth, star.trackArcRad);
      lit.push({ cell, startRad: starElevationRad(star.figure, start, star.atStart), reachRad });
    }
  }
  const startLit = lit.filter(
    (each) => each.startRad >= START_STAR_ELEVATION_DEG * RAD + each.reachRad,
  );
  return (startLit.length > 0 ? startLit : lit).map((each) => each.cell);
}

/** `site` approached away from the star at the descent's start ({@link downStarAzimuthRad}). */
export function litBodySite(site: BodySite, star: DescentStar): BodySite {
  const azimuthRad = downStarAzimuthRad(vecOf(site.dir), star.atStart);
  return { ...site, site: { ...site.site, azimuthRad } };
}
