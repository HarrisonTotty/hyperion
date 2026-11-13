/**
 * The world the descent spike flies (plan R10, R10.T14.c): R05's test planet, the default,
 * `TERRAIN TEST`'s fixture moon, whose field comes through T16's terrain session, or a generated
 * body the server serves (R10.T15.b), whose field comes through the same session over the server's
 * link.
 *
 * @remarks
 * A world is what the scene, the camera, the sun and the measurement read of the bodies: their
 * IDs, designations and kind, the body's figure, Hill radius and rotation, and for a generated
 * world its appearance, detail seed and host disc.
 *
 * - **The test planet** ({@link TEST_PLANET_WORLD}) holds R05's values exactly, so that its scene is
 *   the one T13.b built, bit for bit.
 * - **The terrain test** ({@link terrainTestWorld}) is `TERRAIN TEST`'s star and moon: its body ID,
 *   detail seed and figure the terrain golden's, its rotation and photometry `terrainScene.ts`'s,
 *   read there and never restated. The spike's clock runs at one scene second a second, where
 *   `TERRAIN TEST`'s runs at {@link TERRAIN_TIME_RATE}, so the spike's script seconds are the moon's
 *   scene seconds: its rotation is `terrainMoonRotationAt` of the script's time.
 * - **A served world** ({@link servedSpikeWorld}) is a body as the server's scene draws it
 *   (`servedBody.ts`, through `fromServer.ts`): its and its star's IDs and designations, its kind,
 *   figure with its pole, Hill radius at pericentre and appearance, its star's radius and distance,
 *   and the host discs the server's sky gives its system, and its star's direction at the push,
 *   which lights it. Its rotation is its rotation section's law at the scene's time when it was read
 *   plus the script's seconds, the spike's clock running at one second a second. It carries no
 *   detail seed: the session asks `body_detail` for it, as `VIEW`'s does over the server's scene.
 */

import type {
  BodyIdHex,
  DetailSeedHex,
  HostDiscDto,
  SystemIdHex,
  UniverseTime,
} from "@hyperion/protocol";

import type { SpikeWorldName as LaunchWorldName } from "../../../../preload/api";
import type { TerrainSessionSource } from "../../displays/view/useTerrainSession";
import type { ConnectionStatus } from "../../lib/connection";
import { norm, normalise, type Vec3 } from "../../geometry/vec3";
import { timeAfter } from "../../lib/scene/lightTime";
import type { WireAppearance } from "../appearance/fromWire";
import type { Rotation3 } from "../coords/rotation";
import type { ServedBody } from "../scene/servedBody";
import { KEPT_SYSTEM, keptBody } from "../scenes/kept";
import {
  TERRAIN_HOST_DISCS,
  TERRAIN_MOON_HILL_M,
  TERRAIN_MOON_POLE,
  terrainMoonPhotometry,
  terrainMoonRotationAt,
  terrainStar,
  terrainSystem,
  terrainTestGolden,
} from "../scenes/terrainScene";
import type { BodyFigure } from "../terrain/planet";
import { testPlanetRotationAt } from "./rotation";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

/** The worlds the spike flies, as `--world` names them: the launch's own type (`SpikeLaunch.world`). */
export type SpikeWorldName = LaunchWorldName;

/** What the spike's scene, camera, sun and measurement read of the world it flies. */
export interface SpikeWorld {
  readonly name: SpikeWorldName;
  /** The bodies' system. */
  readonly system: SystemIdHex;
  /** The star, at the system's barycentre. */
  readonly star: BodyIdHex;
  /** The body the craft descends to, 1 au from the star. */
  readonly body: BodyIdHex;
  /** The star's and the body's designations, as the view labels them. */
  readonly starDesignation: string;
  readonly bodyDesignation: string;
  readonly bodyKind: "planet" | "dwarf_planet" | "moon";
  /** The body's reference spheroid, its heights' datum. */
  readonly figure: BodyFigure;
  /** The body's Hill radius, m. */
  readonly hillRadiusM: number;
  /** The body's rotation from its body-fixed axes to its body axes at script time `tS`, s. */
  readonly rotationAt: (tS: number) => Rotation3;
  /** How the photorealistic style shades the body, or `null` for the provisional sphere. */
  readonly appearance: WireAppearance | null;
  /**
   * The body's own detail seed (`ViewBody.detailSeed`), which the terrain session takes in place of
   * asking `body_detail`; absent for the test planet, which has no field.
   */
  readonly detailSeed?: DetailSeedHex;
  /** The scene's host discs (`ViewScene.hostDiscs`); absent for the test planet's scene. */
  readonly hostDiscs?: ReadonlyArray<HostDiscDto>;
  /**
   * The body's distance from its star, m, and the star's radius, m: a served world's; 1 au and the
   * Sun's nominal radius where absent, as the test planet and `TERRAIN TEST` stand.
   */
  readonly starDistanceM?: number;
  readonly starRadiusM?: number;
  /**
   * The star's unit direction from the body's centre in the body's non-rotating axes, a served
   * world's (its centre's from the body's at the push), by which its site is drawn lit
   * (`siteLight.ts`); absent for the kept worlds, whose star the spike places (`sunDirectionBody`).
   */
  readonly starDirection?: Vec3;
  /**
   * The scene's time at the script's start, a served world's (the push it was read at), from which
   * its scene's clock counts the script's seconds; absent for the kept worlds, whose clock starts at
   * the kept epoch (`keptTime`).
   */
  readonly epoch?: UniverseTime;
}

/**
 * A served world as the spike flies it (R10.T15.b): its world, its terrain session's source (the
 * server's link with its scene's surface events), and the link's state, which the spike's header
 * shows.
 */
export interface ServedSpikeWorld {
  readonly kind: "served";
  readonly world: SpikeWorld;
  readonly source: TerrainSessionSource;
  readonly link: ConnectionStatus;
}

/** The scene's star, at the kept system's barycentre. */
export const SPIKE_STAR = keptBody(0);

/** The test planet. */
export const SPIKE_PLANET = keptBody(1);

/**
 * R05's test planet (T13.b): WGS 84's figure, turning at the Earth's rate about +z, in the kept
 * system with the Sun-like `TEST STAR`, with no appearance, seed or host disc.
 */
export const TEST_PLANET_WORLD: SpikeWorld = {
  name: "test-planet",
  system: KEPT_SYSTEM,
  star: SPIKE_STAR,
  body: SPIKE_PLANET,
  starDesignation: "TEST STAR",
  bodyDesignation: "TEST PLANET",
  bodyKind: "planet",
  figure: TEST_PLANET_FIGURE,
  // Earth's Hill radius, 1.5 × 10⁹ m (R02.T4's figure, as `frameChange.ts` takes it).
  hillRadiusM: 1.5e9,
  rotationAt: testPlanetRotationAt,
  appearance: null,
};

/**
 * `TERRAIN TEST`'s world (R10.T16.d): `TEST STAR` and `TEST MOON`, the terrain golden's body, seed
 * and figure about `terrainScene.ts`'s tilted pole, its rotation of the script's seconds, the
 * photometry of its field's palette, and the scene's Sun-like host disc.
 */
export function terrainTestWorld(): SpikeWorld {
  const golden = terrainTestGolden();
  const { header } = golden;
  const figure: BodyFigure = {
    equatorialRadiusM: header.equatorial_radius_m,
    polarRadiusM: header.polar_radius_m,
    pole: TERRAIN_MOON_POLE,
  };
  return {
    name: "terrain-test",
    system: terrainSystem(),
    star: terrainStar(),
    body: golden.body,
    starDesignation: "TEST STAR",
    bodyDesignation: "TEST MOON",
    bodyKind: "moon",
    figure,
    hillRadiusM: TERRAIN_MOON_HILL_M,
    rotationAt: terrainMoonRotationAt,
    // As `terrainSceneAt` builds TEST MOON's.
    appearance: {
      photometry: terrainMoonPhotometry(header),
      figure,
      labels: [],
      bondRatioFinding: null,
    },
    detailSeed: golden.seed,
    hostDiscs: TERRAIN_HOST_DISCS,
  };
}

/**
 * A served world from `served`, a body as the server's scene drew it, lit by `hostDiscs`, its
 * system's discs as the server's sky gives them (`servedHostDiscs`).
 */
export function servedSpikeWorld(
  served: ServedBody,
  hostDiscs: ReadonlyArray<HostDiscDto>,
): SpikeWorld {
  const { epoch } = served;
  return {
    name: "served",
    system: served.system,
    star: served.star.id,
    body: served.body.id,
    starDesignation: served.star.designation,
    bodyDesignation: served.body.designation,
    bodyKind: served.kind,
    figure: served.figure,
    hillRadiusM: served.hillRadiusM,
    // One scene second a script second from the scene's time when the body was read.
    rotationAt: (tS) => served.rotationAt(timeAfter(epoch, tS)),
    appearance: served.appearance,
    hostDiscs,
    starDistanceM: norm(served.towardStarM),
    starRadiusM: served.star.radiusM,
    // The scene's axes are the body's non-rotating ones, translated (`ServedBody.towardStarM`).
    starDirection: normalise(served.towardStarM),
    epoch,
  };
}

/** `TERRAIN TEST`'s world once made, which every later call of {@link spikeWorldNamed} returns. */
let terrainWorld: SpikeWorld | null = null;

/**
 * The world `--world` names, one object a name for the life of the page, so that it is stable in
 * identity where an effect depends on it: {@link TEST_PLANET_WORLD}, or {@link terrainTestWorld}'s
 * made at the first call. The served world is read from the server ({@link servedSpikeWorld}), not
 * named.
 */
export function spikeWorldNamed(name: Exclude<SpikeWorldName, "served">): SpikeWorld {
  let world: SpikeWorld;
  switch (name) {
    case "test-planet":
      world = TEST_PLANET_WORLD;
      break;
    case "terrain-test":
      terrainWorld ??= terrainTestWorld();
      world = terrainWorld;
      break;
  }
  return world;
}
