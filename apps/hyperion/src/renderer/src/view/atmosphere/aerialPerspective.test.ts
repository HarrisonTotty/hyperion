import { beforeAll, describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import { quaternion } from "../camera/quaternion";
import { QUALITY_SETTINGS, type QualitySetting } from "../quality/qualitySetting";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  referenceSunIlluminanceLx,
} from "./earth";
import {
  type AtmosphereCamera,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  tableRadiusM,
} from "./hillaire";
import {
  aerialPerspectiveTwin,
  planetTwin,
  type PlanetTwin,
  rayMarchRayAlongTwin,
  rayMarchTexelTwin,
  twinView,
  type TwinView,
} from "./tablesCpu";

/** WGS 84 (NIMA TR8350.2): a = 6,378,137 m, f = 1 ÷ 298.257223563. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};

/**
 * A camera `heightM` above the equator looking north along the horizon, at the default 60° field
 * of view (`DEFAULT_FOV_DEG`) and 16:9.
 */
function equatorCamera(heightM: number): AtmosphereCamera {
  return {
    positionM: { x: WGS84.equatorialRadiusM + heightM, y: 0, z: 0 },
    // Camera −z (forward) to body +z (north), camera +y (up) to body +x (the normal).
    orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 1920, heightPx: 1080 },
  };
}

/** The sun at an elevation above an equator camera's horizon, at an azimuth from north (east +). */
function sunAt(elevationDeg: number, azimuthDeg: number): SunState {
  const e = (elevationDeg * Math.PI) / 180;
  const a = (azimuthDeg * Math.PI) / 180;
  return {
    directionBodyFixed: {
      x: Math.sin(e),
      y: Math.cos(e) * Math.sin(a),
      z: Math.cos(e) * Math.cos(a),
    },
    illuminanceLx: referenceSunIlluminanceLx(),
    angularRadiusRad: 0.004_650_5,
  };
}

/** The unit ray through the screen at (u, v), (0, 0) at its top left: `view.wgsl`'s `rayThrough`. */
function rayThrough(view: TwinView, u: number, v: number): Vec3 {
  const nx = u * 2 - 1;
  const ny = 1 - v * 2;
  const { forward: f, right: r, upRay: up } = view;
  const d = {
    x: f.x + nx * r.x + ny * up.x,
    y: f.y + nx * r.y + ny * up.y,
    z: f.z + nx * r.z + ny * up.z,
  };
  const length = Math.hypot(d.x, d.y, d.z);
  return { x: d.x / length, y: d.y / length, z: d.z / length };
}

/** The worst relative differences at the far edge over a view's froxels, with how many held. */
interface FarEdge {
  readonly froxels: number;
  readonly inscattering: number;
  readonly transmittance: number;
}

/**
 * The volume against the ray march at the volume's far edge, froxel by froxel: the volume's last
 * slice, which the composite reads at a surface at the reach (`aerialRead.wgsl`), against the
 * march along the froxel's ray to a surface there, as `rayMarch.wgsl` marches it past the reach.
 * Only froxels whose ray reaches the reach above the ground count: a surface stands there.
 */
function farEdge(planet: PlanetTwin, setting: QualitySetting, view: TwinView): FarEdge {
  const sizes = TABLE_SIZES[setting];
  const size = sizes.aerialPerspective;
  const reachM = sizes.aerialPerspectiveReachM;
  const volume = aerialPerspectiveTwin(
    planet,
    view,
    size,
    sizes.aerialPerspectiveSamplesPerSlice,
    reachM,
  );
  const { widthTexels: w, heightTexels: h, slices } = size;
  let froxels = 0;
  let inscattering = 0;
  let transmittance = 0;
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      // Bruneton's mapping on the screen axes: froxel (x, y) at (x, y) ÷ (size − 1).
      const direction = rayThrough(view, x / (w - 1), y / (h - 1));
      const marched = rayMarchRayAlongTwin(planet.medium, view, view.cameraM, direction, reachM);
      if (marched === null || marched.ray.tEndM < reachM) {
        continue;
      }
      froxels += 1;
      const march = rayMarchTexelTwin(planet, view, sizes, marched);
      const at = (((slices - 1) * h + y) * w + x) * 4;
      for (const c of [0, 1, 2] as const) {
        const fromVolume = volume.texels[at + c] ?? Number.NaN;
        inscattering = Math.max(inscattering, Math.abs(fromVolume / march[c] - 1));
      }
      const meanTransmittance = volume.texels[at + 3] ?? Number.NaN;
      transmittance = Math.max(transmittance, Math.abs(meanTransmittance / march[3] - 1));
    }
  }
  return { froxels, inscattering, transmittance };
}

const R1 = tableRadiusM(WGS84);

/**
 * The media: the measured Earth, which the spike draws, and the slab, whose layer edges the march
 * cuts and the volume does not (R08.T6.i).
 */
const MEDIA = [
  { name: "the measured Earth", medium: EARTH_REFERENCE, curves: EARTH_REFERENCE_CURVES },
  { name: "the slab Earth", medium: EARTH_SLAB_REFERENCE, curves: EARTH_SLAB_REFERENCE_CURVES },
] as const;

const CAMERA_HEIGHTS_M = [2, 2_000, 20_000] as const;

/** The sun 20° up ahead of the camera, and 5° up behind it. */
const SUNS = [
  [20, 0],
  [5, 180],
] as const;

/**
 * The far edge of every camera and sun on each medium and setting, keyed by medium and setting:
 * formed once before the tests, which only read it.
 */
const farEdges = new Map<string, ReadonlyArray<FarEdge>>();

beforeAll(() => {
  for (const { name, medium, curves } of MEDIA) {
    const planet = planetTwin(medium, R1, curves);
    for (const setting of QUALITY_SETTINGS) {
      const edges = CAMERA_HEIGHTS_M.flatMap((heightM) =>
        SUNS.map(([elevationDeg, azimuthDeg]) =>
          farEdge(
            planet,
            setting,
            twinView(equatorCamera(heightM), sunAt(elevationDeg, azimuthDeg), WGS84, 0.1),
          ),
        ),
      );
      farEdges.set(`${name}, ${setting}`, edges);
    }
  }
}, 120_000);

/** The far edges of a medium and setting, formed before the tests. */
function farEdgesOf(name: string, setting: QualitySetting): ReadonlyArray<FarEdge> {
  const edges = farEdges.get(`${name}, ${setting}`);
  if (edges === undefined) {
    throw new Error(`no far edges for ${name} on ${setting}`);
  }
  return edges;
}

const CASES = MEDIA.flatMap(({ name }) =>
  QUALITY_SETTINGS.map((setting) => [name, setting] as const),
);

describe("aerial perspective at the volume's far edge (R08.T9.a)", () => {
  it.each(CASES)(
    "takes the same in-scattering from the volume and from the march beyond it to 2%, on %s, %s",
    (name, setting) => {
      for (const edge of farEdgesOf(name, setting)) {
        // At least the froxels above the horizon reach 32 km clear of the ground.
        expect(edge.froxels).toBeGreaterThanOrEqual(512);
        expect(edge.inscattering).toBeLessThan(0.02);
      }
    },
  );

  it.each(CASES)(
    "takes the same mean transmittance, which dims the surface, from both to 2%, on %s, %s",
    (name, setting) => {
      for (const edge of farEdgesOf(name, setting)) {
        expect(edge.froxels).toBeGreaterThanOrEqual(512);
        expect(edge.transmittance).toBeLessThan(0.02);
      }
    },
  );
});
