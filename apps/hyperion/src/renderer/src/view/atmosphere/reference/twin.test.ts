import { describe, expect, it } from "vitest";

import { normalise, type Vec3 } from "../../../geometry/vec3";
import {
  sweepGeometries,
  sweepMedium,
  type SweepGeometry,
  type SweepPoint,
} from "../../../test/atmosphereSweep";
import { DEFAULT_FOV_DEG } from "../../camera/projection";
import { lookAlong } from "../../camera/quaternion";
import { type AtmosphereCamera, type SunState, TABLE_SIZES } from "../hillaire";
import { kernelMedium } from "../tables";
import {
  aerialPerspectiveTwin,
  planetTwin,
  rayMarchTargetTexelTwin,
  skyViewReadTwin,
  skyViewTexelsTwin,
  tableSources,
  twinView,
  type TwinRgb,
} from "../tablesCpu";
import { EARTH_REFERENCE_RADIUS_M } from "./cases";
import {
  geodeticPoint,
  localDirectionVector,
  REFERENCE_DETECTOR_HALF_ANGLE_DEG,
  type ReferenceGeometry,
} from "./geometries";
import {
  AERIAL_FROXEL_SCALE,
  groundDistanceM,
  referenceTwin,
  TWIN_VIEWPORT,
  twinGeometryKind,
  type ReferenceTwin,
} from "./twin";

const RADIUS_M = EARTH_REFERENCE_RADIUS_M;
const FIGURE = { kind: "sphere", radiusM: RADIUS_M } as const;
const SPHERE = { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M };
const GEOMETRIES = sweepGeometries();

function geometryNamed(name: string): SweepGeometry {
  const found = GEOMETRIES.find((g) => g.name === name);
  if (found === undefined) {
    throw new Error(`the sweep has no geometry ${name}`);
  }
  return found;
}

/** A middling point: an absorbing aerosol of g 0.5 at τ ½. */
const POINT: SweepPoint = {
  singleScatteringAlbedo: 0.9,
  asymmetry: 0.5,
  tau: 0.5,
  groundAlbedo: 0.3,
};

/**
 * The limit of a test that builds a twin, ms. A twin's tables take about 4 s alone and the first
 * test's, with `planetTwin`'s beside it, 6.7 s (2026-10-26, one file alone on a quiet laptop):
 * CPU-bound work, ten times over and rounded up (`vitest.config.mts`'s `testTimeout`).
 */
const TWIN_TIMEOUT_MS = 120_000;

const twins = new Map<string, ReferenceTwin>();

/** The twin of a sweep point, built once a test file. */
function twinOf(point: SweepPoint): ReferenceTwin {
  const key = JSON.stringify(point);
  let twin = twins.get(key);
  if (twin === undefined) {
    twin = referenceTwin(sweepMedium(point), FIGURE);
    twins.set(key, twin);
  }
  return twin;
}

function observerOf(geometry: ReferenceGeometry): Vec3 {
  return geodeticPoint(SPHERE, geometry.observer.latitudeDeg, geometry.observer.heightM);
}

function axisOf(geometry: ReferenceGeometry): Vec3 {
  return localDirectionVector(geometry.observer.latitudeDeg, geometry.view);
}

function sunOf(geometry: ReferenceGeometry): Vec3 {
  return localDirectionVector(geometry.observer.latitudeDeg, geometry.sun);
}

function sunStateOf(geometry: ReferenceGeometry): SunState {
  return { directionBodyFixed: sunOf(geometry), illuminanceLx: [1, 1, 1], angularRadiusRad: 0 };
}

/** The twin's camera at a geometry's observer looking along `direction`. */
function cameraAlong(geometry: ReferenceGeometry, direction: Vec3): AtmosphereCamera {
  const originM = observerOf(geometry);
  return {
    positionM: originM,
    orientation: lookAlong(direction, normalise(originM)),
    fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180,
    viewport: TWIN_VIEWPORT,
  };
}

describe("the reference twin's tables", { timeout: TWIN_TIMEOUT_MS }, () => {
  it("builds the march's tables as planetTwin builds them, once a medium", () => {
    const medium = sweepMedium(POINT);
    const own = planetTwin(kernelMedium(medium), RADIUS_M);
    const { planet } = twinOf(POINT);
    expect(planet.bottomRadiusM).toBe(own.bottomRadiusM);
    expect(planet.transmittance.texels).toEqual(own.transmittance.texels);
    expect(planet.multiScattering.texels).toEqual(own.multiScattering.texels);
  });

  it("lights the ground by the sliced sun's lookup, the table's at the ground, and the sky", () => {
    const twin = twinOf(POINT);
    const point = { x: RADIUS_M, y: 0, z: 0 };
    const sun = { x: Math.cos(Math.PI / 6), y: 0, z: Math.sin(Math.PI / 6) };
    const latitude = 0;
    const azimuth = 0;
    const transmitted = twin.surfaceLighting.sunTransmittance(0, sun.x, latitude, azimuth, 0);
    const sky = twin.surfaceLighting.skyIrradiance(0, sun.x, latitude, 0);
    const table: TwinRgb = [0, 0, 0];
    tableSources(twin.planet).sunTransmittance(0, sun.x, table, 0);
    expect(transmitted[1]).toBeCloseTo(table[1], 12);
    expect(sky[1]).toBeGreaterThan(0);
    const lit = twin.litGroundRadiance(point, sun);
    expect(lit[1]).toBeCloseTo((0.3 * (sun.x * transmitted[1] + sky[1])) / Math.PI, 14);
  });
});

describe("the reference twin's options", { timeout: TWIN_TIMEOUT_MS }, () => {
  const names = ["ground-s30-v60-a90", "disc-p30", "limb-1H", "aerial-10km"];

  it("scales every radiance and its surface's part by the sun's irradiance", () => {
    const unit = twinOf(POINT);
    const bright = referenceTwin(sweepMedium(POINT), FIGURE, { sunIrradiance: [2, 3, 5] });
    for (const name of names) {
      const geometry = geometryNamed(name);
      const [r, g, b] = unit.radiance(geometry);
      const [sr, sg, sb] = unit.surfaceRadiance(geometry);
      expect(bright.radiance(geometry)).toEqual([2 * r, 3 * g, 5 * b]);
      expect(bright.surfaceRadiance(geometry)).toEqual([2 * sr, 3 * sg, 5 * sb]);
    }
  });

  // One twin of a cone of zero width, built once for the two tests that read it.
  let narrowBuilt: ReferenceTwin | undefined;
  const narrowTwin = (): ReferenceTwin => {
    narrowBuilt ??= referenceTwin(sweepMedium(POINT), FIGURE, { detectorHalfAngleDeg: 0 });
    return narrowBuilt;
  };

  it("takes its cone where a geometry gives none", () => {
    const narrow = narrowTwin();
    for (const name of names) {
      const geometry = geometryNamed(name);
      expect(narrow.radiance(geometry)).toEqual(
        narrow.radianceAlong(geometry, axisOf(geometry)).total,
      );
    }
  });

  it("takes a geometry's own cone over its own", () => {
    const narrow = narrowTwin();
    const own = REFERENCE_DETECTOR_HALF_ANGLE_DEG;
    for (const name of names) {
      const geometry = geometryNamed(name);
      expect(narrow.radiance({ ...geometry, detectorHalfAngleDeg: own })).toEqual(
        twinOf(POINT).radiance(geometry),
      );
    }
  });
});

describe("the reference twin's cone", { timeout: TWIN_TIMEOUT_MS }, () => {
  it("is its axis's radiance at a cone of zero width", () => {
    const twin = twinOf(POINT);
    const geometry = { ...geometryNamed("ground-s30-v60-a90"), detectorHalfAngleDeg: 0 };
    expect(twin.radiance(geometry)).toEqual(twin.radianceAlong(geometry, axisOf(geometry)).total);
  });

  it("averages the reads over the cone, close to its axis's for a smooth sky", () => {
    const twin = twinOf(POINT);
    const geometry = geometryNamed("ground-s30-v60-a90");
    const cone = twin.radiance(geometry);
    const axis = twin.radianceAlong(geometry, axisOf(geometry)).total;
    expect(cone[1]).not.toBe(axis[1]);
    expect(Math.abs(cone[1] / axis[1] - 1)).toBeLessThan(0.01);
  });

  it("gives each geometry's share of the cone that meets the ground", () => {
    const twin = twinOf(POINT);
    const share = (name: string): number => twin.groundConeShare(geometryNamed(name));
    expect([share("ground-s30-v85-a90"), share("limb-10H"), share("disc-p60")]).toEqual([0, 0, 1]);
    expect(share("aerial-1km")).toBeGreaterThan(0.2);
    expect(share("aerial-1km")).toBeLessThan(share("aerial-32km"));
    expect(share("aerial-32km")).toBeLessThan(0.5);
    expect(share("limb-0.1H")).toBeGreaterThan(share("limb-3H"));
  });

  it("keeps the drawn surface's part apart, none where no ray meets the ground", () => {
    const twin = twinOf(POINT);
    for (const name of ["ground-s60-v30-a0", "limb-10H"]) {
      expect(twin.surfaceRadiance(geometryNamed(name))).toEqual([0, 0, 0]);
    }
    for (const name of ["disc-p30", "aerial-10km", "limb-0.3H"]) {
      const surface = twin.surfaceRadiance(geometryNamed(name))[1];
      expect(surface).toBeGreaterThan(0);
      expect(surface).toBeLessThan(twin.radiance(geometryNamed(name))[1]);
    }
  });
});

describe("the reference twin's reads", { timeout: TWIN_TIMEOUT_MS }, () => {
  it("reads a ground view from the sky view built at its observer for its sun", () => {
    const twin = twinOf(POINT);
    const geometry = geometryNamed("ground-s60-v75-a180");
    const view = twinView(cameraAlong(geometry, axisOf(geometry)), sunStateOf(geometry), SPHERE, 1);
    const sizes = TABLE_SIZES.high;
    const texelAt = skyViewTexelsTwin(twin.planet, view, sizes.skyView, sizes.skyViewSamples);
    const direction = axisOf(geometry);
    expect(twin.radianceAlong(geometry, direction).total).toEqual(
      skyViewReadTwin(texelAt, sizes.skyView, twin.planet.medium, view, direction),
    );
  });

  it("reads a limb from orbit as the ray march's texel at the centre of a camera along it", () => {
    const twin = twinOf(POINT);
    const geometry = geometryNamed("limb-1H");
    const direction = axisOf(geometry);
    const view = twinView(cameraAlong(geometry, direction), sunStateOf(geometry), SPHERE, 1);
    const texel = rayMarchTargetTexelTwin(twin.planet, view, TABLE_SIZES.high, 960, 540, 0);
    expect(twin.radianceAlong(geometry, direction).total).toEqual([texel[0], texel[1], texel[2]]);
  });

  it("draws the disc's lit ground behind the march's transmittance to it", () => {
    const twin = twinOf(POINT);
    const geometry = geometryNamed("disc-p30");
    const direction = axisOf(geometry);
    const groundM = groundDistanceM(observerOf(geometry), direction, RADIUS_M);
    if (groundM === null) {
      throw new Error("the disc looks down at the ground");
    }
    const view = twinView(cameraAlong(geometry, direction), sunStateOf(geometry), SPHERE, 1);
    const [r, g, b, t] = rayMarchTargetTexelTwin(
      twin.planet,
      view,
      TABLE_SIZES.high,
      960,
      540,
      1 / groundM,
    );
    const lit = twin.litGroundRadiance({ x: RADIUS_M, y: 0, z: 0 }, sunOf(geometry));
    const drawn = twin.radianceAlong(geometry, direction);
    for (const [c, march] of [r, g, b].entries()) {
      expect(drawn.total[c]).toBeCloseTo(march + (lit[c] ?? Number.NaN) * t, 12);
      expect(drawn.surface[c]).toBeCloseTo((lit[c] ?? Number.NaN) * t, 12);
    }
  });

  it("reads aerial perspective at the target's distance where the ray clears the ground", () => {
    const twin = twinOf(POINT);
    const up = { ...geometryNamed("aerial-1km"), view: { zenithDeg: 85, azimuthDeg: 90 } };
    const far = { ...up, maxDistanceM: 10_000 };
    const direction = axisOf(up);
    const at1km = twin.radianceAlong(up, direction);
    const at10km = twin.radianceAlong(far, direction);
    expect(at10km.total[1]).toBeGreaterThan(at1km.total[1]);
    expect(at1km.surface).toEqual([0, 0, 0]);
    expect(groundDistanceM(observerOf(up), direction, RADIUS_M)).toBeNull();
  });

  it("reads aerial perspective as the client's volume's four central froxels at its camera", () => {
    const twin = twinOf(POINT);
    const geometry = {
      ...geometryNamed("aerial-1km"),
      view: { zenithDeg: 85, azimuthDeg: 90 },
      maxDistanceM: 10_500,
    };
    const direction = axisOf(geometry);
    const view = twinView(cameraAlong(geometry, direction), sunStateOf(geometry), SPHERE, 1);
    const sizes = TABLE_SIZES.high;
    const size = sizes.aerialPerspective;
    const reachM = sizes.aerialPerspectiveReachM;
    const volume = aerialPerspectiveTwin(
      twin.planet,
      view,
      size,
      sizes.aerialPerspectiveSamplesPerSlice,
      reachM,
    );
    const { widthTexels: w, heightTexels: h, slices } = size;
    // The 2 × 2 volume's corners stand at the client's central froxels only for its 32 wide.
    expect(AERIAL_FROXEL_SCALE).toBe(1 / (w - 1));
    const centre = (slice: number, channel: number): number => {
      let sum = 0;
      for (const [x, y] of [
        [w / 2 - 1, h / 2 - 1],
        [w / 2, h / 2 - 1],
        [w / 2 - 1, h / 2],
        [w / 2, h / 2],
      ] as const) {
        sum += volume.texels[((slice * h + y) * w + x) * 4 + channel] ?? Number.NaN;
      }
      return sum / 4;
    };
    // 10.5 km of 32 km over 32 slices: the slice axis at 10 − ½, halfway between slices 9 and 10.
    expect((10_500 / reachM - 0.5 / slices) * slices - 0.5).toBeCloseTo(9.5, 12);
    const read = [0, 1, 2].map((c) => (centre(9, c) + centre(10, c)) / 2);
    const drawn = twin.radianceAlong(geometry, direction).total;
    for (const [c, expected] of read.entries()) {
      expect(drawn[c] ?? Number.NaN).toBeCloseTo(expected, 12);
    }
  });

  it("draws the lit ground behind the air where the ray meets it before its target", () => {
    const twin = twinOf(POINT);
    const geometry = geometryNamed("aerial-10km");
    const originM = observerOf(geometry);
    const below = localDirectionVector(0, { zenithDeg: 90.3, azimuthDeg: 90 });
    const groundM = groundDistanceM(originM, below, RADIUS_M) ?? Number.NaN;
    expect(groundM).toBeLessThan(1_000);
    const lit = twin.litGroundRadiance(
      {
        x: originM.x + groundM * below.x,
        y: originM.y + groundM * below.y,
        z: originM.z + groundM * below.z,
      },
      sunOf(geometry),
    );
    const drawn = twin.radianceAlong(geometry, below);
    expect(drawn.surface[1]).toBeGreaterThan(0.5 * lit[1]);
    expect(drawn.surface[1]).toBeLessThan(lit[1]);
    expect(drawn.total[1]).toBeGreaterThan(drawn.surface[1]);
  });
});

describe("the reference twin's media", { timeout: TWIN_TIMEOUT_MS }, () => {
  it("keeps a grey medium's three channels equal", () => {
    const twin = twinOf(POINT);
    for (const name of ["ground-s0-v30-a0", "disc-p60", "limb-0.3H", "aerial-32km"]) {
      const [r, g, b] = twin.radiance(geometryNamed(name));
      expect([r, b]).toEqual([expect.closeTo(g, 12), expect.closeTo(g, 12)]);
    }
  });

  it("scatters nothing into the sky of a medium of ω 0", () => {
    const twin = twinOf({ ...POINT, singleScatteringAlbedo: 0 });
    // The cones of the limbs to 3 H, ±32 km in tangent height from 3,700 km, take in the lit ground.
    for (const name of ["ground-s30-v0-a0", "ground-s80-v85-a180", "limb-10H"]) {
      expect(twin.radiance(geometryNamed(name))).toEqual([0, 0, 0]);
    }
  });

  it("shows the disc of a medium of ω 0 as the ground's direct light through the air both ways", () => {
    const twin = twinOf({ ...POINT, singleScatteringAlbedo: 0 });
    const geometry = { ...geometryNamed("disc-p60"), detectorHalfAngleDeg: 0 };
    const tau = POINT.tau;
    const expected = (0.3 / Math.PI) * 0.5 * Math.exp(-tau / 0.5) * Math.exp(-tau);
    expect(twin.radiance(geometry)[1] / expected).toBeCloseTo(1, 2);
  });
});

describe("the reference twin's geometry", () => {
  it("meets the ground straight down at the observer's height and not looking up", () => {
    const origin = { x: RADIUS_M + 1_000, y: 0, z: 0 };
    expect(groundDistanceM(origin, { x: -1, y: 0, z: 0 }, RADIUS_M)).toBeCloseTo(1_000, 6);
    expect(groundDistanceM(origin, { x: 1, y: 0, z: 0 }, RADIUS_M)).toBeNull();
  });

  it("draws the ground's observers on the sky view, orbit's on the march, paths on the volume", () => {
    const medium = sweepMedium(POINT);
    const pairs = new Set(GEOMETRIES.map((g) => `${g.kind}:${twinGeometryKind(g, medium)}`));
    expect([...pairs].toSorted()).toEqual([
      "aerial:aerial",
      "disc:orbit",
      "ground:ground",
      "limb:orbit",
    ]);
  });

  it("refuses an observer inside the air in the march band", () => {
    const geometry = {
      ...geometryNamed("ground-s30-v0-a0"),
      observer: { heightM: 40_000, latitudeDeg: 0 },
    };
    expect(() => twinGeometryKind(geometry, sweepMedium(POINT))).toThrow(RangeError);
  });
});
