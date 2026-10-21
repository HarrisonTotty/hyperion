/**
 * The world the descent spike flies (plan R10, R10.T14.c): R05's test planet, the default, or
 * `TERRAIN TEST`'s fixture moon, whose field comes through T16's terrain session.
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
 */

import type { BodyIdHex, DetailSeedHex, HostDiscDto, SystemIdHex } from "@hyperion/protocol";

import type { SpikeWorldName as LaunchWorldName } from "../../../../preload/api";
import type { WireAppearance } from "../appearance/fromWire";
import type { Rotation3 } from "../coords/rotation";
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
  readonly bodyKind: "planet" | "moon";
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

/** `TERRAIN TEST`'s world once made, which every later call of {@link spikeWorldNamed} returns. */
let terrainWorld: SpikeWorld | null = null;

/**
 * The world `--world` names, one object a name for the life of the page, so that it is stable in
 * identity where an effect depends on it: {@link TEST_PLANET_WORLD}, or {@link terrainTestWorld}'s
 * made at the first call.
 */
export function spikeWorldNamed(name: SpikeWorldName): SpikeWorld {
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
