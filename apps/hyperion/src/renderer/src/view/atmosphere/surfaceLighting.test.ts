import { beforeAll, describe, expect, it } from "vitest";

import { cross, dot, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { earthBody, saturnBody, saturnClassMedium } from "../../test/atmosphereFixtures";
import { forbiddenIn } from "../../test/atmosphereWgsl";
import { phaseFactorFromTable, phaseFactorTableOf } from "../appearance/law";
import { lawFor } from "../appearance/phase";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { ProjectionCamera } from "../camera/projection";
import { annulusEdges } from "../lighting/annuli";
import { planetshineIrradiance } from "../lighting/planetshine";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import bodyDiscWgsl from "../shaders/bodyDisc.wgsl?raw";
import { type DiscRecord, shadeDiscPoint } from "../bodies/discShading";
import { oblateAlbedoScale } from "../bodies/oblate";
import { BODY_DISC_CELLS_KERNEL, BODY_DISC_MATERIALS, SMOOTH_MESH_MATERIAL } from "../bodies/draw";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES } from "./earth";
import {
  gaussLegendre,
  IRRADIANCE_ARC_NODES,
  IRRADIANCE_RING_NODES,
  irradianceDirections,
} from "./irradianceQuadrature";
import type { AtmosphereMedium } from "./medium";
import { gravityRatio } from "./oblate";
import irradianceWgsl from "./shaders/irradiance.wgsl?raw";
import surfaceLightingWgsl from "./shaders/surfaceLighting.wgsl?raw";
import {
  AIRLESS_LIGHTING,
  createAirlessTables,
  SURFACE_LIGHTING_WGSL,
  surfaceLightingTextures,
  type SurfaceLightingReader,
} from "./surfaceLighting";
import { IRRADIANCE_KERNEL, IRRADIANCE_SAMPLES, IRRADIANCE_SIZE } from "./tables";
import {
  IRRADIANCE_KERNEL_OPTIONS,
  irradianceTwin,
  MULTI_SCATTERING_KERNEL_OPTIONS,
  multiScatteringTwin,
  oblateIrradianceTwin,
  oblatePlanetTwin,
  type OblateTwinSizes,
  surfaceLightingTwin,
  type TwinTable,
  transmittanceTablesTwin,
} from "./tablesCpu";

const RAD = Math.PI / 180;

/** The tables at sizes that keep each build to a second or two; the irradiance at the kernel's. */
const SIZES: OblateTwinSizes = {
  transmittance: { widthTexels: 64, heightTexels: 16 },
  transmittanceSamples: 128,
  multiScattering: {
    ...MULTI_SCATTERING_KERNEL_OPTIONS,
    size: { widthTexels: 16, heightTexels: 16 },
    samples: 32,
  },
};

/** Every texel of the irradiance table's rows `rows`. */
function rowsOf(rows: ReadonlyArray<number>): (readonly [number, number])[] {
  return rows.flatMap((y) =>
    Array.from({ length: IRRADIANCE_SIZE.widthTexels }, (_, x) => [x, y] as const),
  );
}

/** A rule's values as the WGSL writes them, to 16 places. */
function literal(rule: Float64Array): string {
  return Array.from(rule, (v) => v.toFixed(16)).join(", ");
}

describe("the sky-irradiance quadrature (R08.T9.b)", () => {
  it("is Gauss–Legendre's: exact to degree 2n − 1, Abramowitz and Stegun's nodes", () => {
    for (const n of [1, 3, 6, 8]) {
      const { nodes, weights } = gaussLegendre(n);
      expect(weights.reduce((a, b) => a + b, 0)).toBeCloseTo(1, 14);
      for (let degree = 0; degree <= 2 * n - 1; degree += 1) {
        const sum = nodes.reduce((s, t, k) => s + (weights[k] ?? 0) * t ** degree, 0);
        expect(sum).toBeCloseTo(1 / (degree + 1), 13);
      }
    }
    // Table 25.4's 8-point abscissae on [−1, 1], ±0.18343 46424 95650 to ±0.96028 98564 97536.
    const { nodes } = gaussLegendre(8);
    expect(2 * (nodes[4] ?? 0) - 1).toBeCloseTo(0.183_434_642_495_65, 13);
    expect(2 * (nodes[7] ?? 0) - 1).toBeCloseTo(0.960_289_856_497_536, 13);
    expect(() => gaussLegendre(0)).toThrow(RangeError);
  });

  it("integrates μ over the sky above the horizon to π wherever the sun stands", () => {
    for (const muSun of [1, 0.9, 0.5, 0.17, 0.02, 0, -0.02, -0.3, -0.8, -1]) {
      const directions = irradianceDirections(muSun);
      expect(directions).toHaveLength(2 * IRRADIANCE_RING_NODES * IRRADIANCE_ARC_NODES);
      const sum = directions.reduce((s, { weight }) => s + weight, 0);
      expect(Math.abs(sum / Math.PI - 1)).toBeLessThan(2e-3);
      for (const { direction, weight } of directions) {
        expect(Math.hypot(direction.x, direction.y, direction.z)).toBeCloseTo(1, 12);
        // No node below the horizon carries weight.
        expect(weight > 0 ? direction.z : 0).toBeGreaterThanOrEqual(0);
      }
    }
  });

  it("is irradiance.wgsl's: its rules, its counts and its texel's directions", () => {
    const code = irradianceWgsl.replaceAll(/\s+/gu, " ");
    const ring = gaussLegendre(IRRADIANCE_RING_NODES);
    const arc = gaussLegendre(IRRADIANCE_ARC_NODES);
    expect(code).toContain(literal(ring.nodes));
    expect(code).toContain(literal(ring.weights));
    expect(code).toContain(literal(arc.nodes));
    expect(code).toContain(literal(arc.weights));
    expect(irradianceWgsl).toContain(`const RING_NODES : u32 = ${IRRADIANCE_RING_NODES}u;`);
    expect(irradianceWgsl).toContain(`const ARC_NODES : u32 = ${IRRADIANCE_ARC_NODES}u;`);
    expect(irradianceWgsl).toContain(
      `const DIRECTIONS : u32 = ${2 * IRRADIANCE_RING_NODES * IRRADIANCE_ARC_NODES}u;`,
    );
    expect(irradianceWgsl).toContain(
      `@workgroup_size(${2 * IRRADIANCE_RING_NODES * IRRADIANCE_ARC_NODES}, 1, 1)`,
    );
    expect(IRRADIANCE_KERNEL_OPTIONS).toEqual({
      size: IRRADIANCE_SIZE,
      samples: IRRADIANCE_SAMPLES,
      ringNodes: IRRADIANCE_RING_NODES,
      arcNodes: IRRADIANCE_ARC_NODES,
    });
    expect(IRRADIANCE_SIZE).toEqual({ widthTexels: 64, heightTexels: 16 });
  });
});

/**
 * A thin pure-Rayleigh air of vertical optical depth `tau` over a black ground, exponential in
 * `scaleHeightM` (Rayleigh's 8 km by default).
 */
function thinRayleigh(tau: number, scaleHeightM = 8_000): AtmosphereMedium {
  const topHeightM = 100_000;
  const sigma = tau / (scaleHeightM * -Math.expm1(-topHeightM / scaleHeightM));
  return {
    name: "thin rayleigh",
    topHeightM,
    groundAlbedo: [0, 0, 0],
    terms: [
      {
        name: "rayleigh",
        density: { kind: "exponential", scaleHeightM },
        scattering: [sigma, sigma, sigma],
        absorption: [0, 0, 0],
        phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
      },
    ],
  };
}

/** The sky's irradiance twin of a sphere's medium at the texels listed, small tables beneath. */
function sphereIrradiance(
  medium: AtmosphereMedium,
  radiusM: number,
  texels: ReadonlyArray<readonly [number, number]>,
): TwinTable {
  const transmittance = transmittanceTablesTwin(
    medium,
    radiusM,
    [],
    SIZES.transmittance,
    SIZES.transmittanceSamples,
  );
  const multiple = multiScatteringTwin(medium, radiusM, transmittance, SIZES.multiScattering);
  return irradianceTwin(
    medium,
    radiusM,
    transmittance,
    [multiple],
    IRRADIANCE_KERNEL_OPTIONS,
    texels,
  );
}

/** A texel's rgb. */
function texelOf(table: TwinTable, x: number, y: number): readonly [number, number, number] {
  const at = (y * table.widthTexels + x) * 4;
  return [
    table.texels[at] ?? Number.NaN,
    table.texels[at + 1] ?? Number.NaN,
    table.texels[at + 2] ?? Number.NaN,
  ];
}

describe("the sky's irradiance (R08.T9.b)", () => {
  it("gives a thin Rayleigh air's single scattering under a sun at the zenith, the sphere's column at the horizon", () => {
    // So thin that the second order, about τ of itself, is 10⁻⁴.
    const tau = 1e-4;
    const radiusM = 6_371_000;
    const medium = thinRayleigh(tau);
    const sky = sphereIrradiance(medium, radiusM, [[63, 0]]);
    // E = τ ∫ P(μ) μ C(μ) dω over the sky, C the slant column over the vertical, by direct
    // quadrature on the sphere: in the plane-parallel limit C = 1 ÷ μ and E = τ ÷ 2, Rayleigh's
    // phase sending half its light down; on the sphere the horizon's column is bounded (Chapman's
    // grazing √(πR ÷ 2H)), a deficit of about 3% here.
    const scaleHeightM = 8_000;
    const top = medium.topHeightM;
    const vertical = scaleHeightM * -Math.expm1(-top / scaleHeightM);
    const column = (mu: number): number => {
      const tMax = -radiusM * mu + Math.sqrt((radiusM * mu) ** 2 + top * (2 * radiusM + top));
      const steps = 4_000;
      let sum = 0;
      for (let k = 0; k < steps; k += 1) {
        // Steps placed quadratically toward the ground, where the air is.
        const a = (k / steps) ** 2 * tMax;
        const b = ((k + 1) / steps) ** 2 * tMax;
        const t = (a + b) / 2;
        const r = Math.sqrt(radiusM * radiusM + 2 * radiusM * mu * t + t * t);
        sum += Math.exp(-(r - radiusM) / scaleHeightM) * (b - a);
      }
      return sum / vertical;
    };
    const rule = gaussLegendre(48);
    let reference = 0;
    for (const [lo, hi] of [
      [0, 0.05],
      [0.05, 0.3],
      [0.3, 1],
    ] as const) {
      rule.nodes.forEach((x, k) => {
        const mu = lo + (hi - lo) * x;
        const phase = (3 / (16 * Math.PI)) * (1 + mu * mu);
        reference += (hi - lo) * (rule.weights[k] ?? 0) * 2 * Math.PI * phase * mu * column(mu);
      });
    }
    expect(reference / 0.5 - 1).toBeGreaterThan(-0.04);
    expect(reference / 0.5 - 1).toBeLessThan(-0.02);
    for (const value of texelOf(sky, 63, 0)) {
      expect(Math.abs(value / (tau * reference) - 1)).toBeLessThan(0.01);
    }
  });

  it("reads a 1.2 km haze's sky above the ground within 10% of its thin falloff, its rows dense near the ground", () => {
    // Thin and exponential, the sky's irradiance falls with the column above the point, as
    // e^(−s h ÷ H); with Bruneton's linear rows, 6.7 km apart, the reads were 1.5 to 3.7 times it.
    const scaleHeightM = 1_200;
    const haze = thinRayleigh(1e-4, scaleHeightM);
    const planet = oblatePlanetTwin(haze, earthBody(), [], SIZES);
    const zenithColumn = Array.from(
      { length: IRRADIANCE_SIZE.heightTexels },
      (_, y) => [IRRADIANCE_SIZE.widthTexels - 1, y] as const,
    );
    const sky = surfaceLightingTwin(
      planet,
      oblateIrradianceTwin(planet, IRRADIANCE_KERNEL_OPTIONS, zenithColumn),
    );
    const latitude = 0.6;
    const s = gravityRatio(earthBody().gravity, latitude);
    const ground = sky.skyIrradiance(0, 1, latitude, 0);
    for (const heightM of [500, 1_000, 2_000]) {
      const above = sky.skyIrradiance(heightM, 1, latitude, 0);
      const falloff = Math.exp(-(s * heightM) / scaleHeightM);
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(above[c] / ground[c] / falloff - 1)).toBeLessThan(0.1);
      }
    }
  });

  it("is 0 at every altitude with no scattering term", () => {
    const absorbing: AtmosphereMedium = {
      ...thinRayleigh(0.1),
      terms: [
        {
          name: "dark",
          density: { kind: "exponential", scaleHeightM: 8_000 },
          scattering: [0, 0, 0],
          absorption: [1e-5, 2e-5, 3e-5],
          phase: { kind: "none" },
        },
      ],
    };
    const sky = sphereIrradiance(absorbing, 6_371_000, rowsOf([0, 5, 10, 15]));
    expect(sky.texels.every((v) => v === 0)).toBe(true);
  });
});

describe("surface lighting through Earth's air (R08.T9.b)", () => {
  let earth: SurfaceLightingReader | undefined;
  beforeAll(() => {
    const planet = oblatePlanetTwin(EARTH_REFERENCE, earthBody(), EARTH_REFERENCE_CURVES, SIZES);
    earth = surfaceLightingTwin(
      planet,
      oblateIrradianceTwin(planet, IRRADIANCE_KERNEL_OPTIONS, rowsOf([0, 15])),
    );
  }, 120_000);
  const lighting = (): SurfaceLightingReader => {
    if (earth === undefined) {
      throw new Error("Earth's twin is built before the tests");
    }
    return earth;
  };

  it("passes all the sun's light and has no sky at the top of the atmosphere", () => {
    // The top stands at h* = s h = H: a geodetic height of H ÷ s, a little above it.
    const latitude = 0.4;
    const top =
      (EARTH_REFERENCE.topHeightM / gravityRatio(earthBody().gravity, latitude)) * (1 + 1e-12);
    for (const mu of [1, 0.5, 0.1]) {
      expect(lighting().sunTransmittance(top, mu, latitude, 1, 0)).toEqual([1, 1, 1]);
      for (const value of lighting().skyIrradiance(top, mu, latitude, 0)) {
        expect(value).toBeLessThan(1e-15);
      }
    }
  });

  it("puts the ground's direct and diffuse light under a zenith sun between the direct beam's and the top's", () => {
    const direct = lighting().sunTransmittance(0, 1, 0.6, 0, 0);
    const diffuse = lighting().skyIrradiance(0, 1, 0.6, 0);
    for (const c of [0, 1, 2] as const) {
      expect(direct[c] + diffuse[c]).toBeGreaterThan(direct[c]);
      expect(direct[c] + diffuse[c]).toBeLessThan(1);
    }
    // The blue sky's share is the larger.
    expect(diffuse[2] / direct[2]).toBeGreaterThan(diffuse[0] / direct[0]);
  });

  it("takes the three-node rule to the centre's lookup as ρ → 0, to 10⁻⁶", () => {
    for (const [h, mu, latitude, azimuth] of [
      [0, 0.5, 0.3, 1],
      [2_000, 0.05, 1.1, -2.5],
      [30_000, 0.3, 0, 0.3],
    ] as const) {
      const centre = lighting().sunTransmittance(h, mu, latitude, azimuth, 0);
      const rule = lighting().sourceTransmittance(h, mu, latitude, azimuth, 1e-9, 0);
      for (const c of [0, 1, 2] as const) {
        expect(Math.abs(rule[c] - centre[c])).toBeLessThanOrEqual(1e-6 * centre[c]);
      }
    }
    // Wholly below the local horizon every node is dropped: it lights no horizontal surface, though
    // from 30 km the ground does not hide its centre.
    expect(lighting().sunTransmittance(30_000, -0.02, 0, 0.3, 0)[2]).toBeGreaterThan(0);
    expect(lighting().sourceTransmittance(30_000, -0.02, 0, 0.3, 1e-9, 0)).toEqual([0, 0, 0]);
  });

  it("matches a 200 × 200 quadrature over a 9.8° disc at 10° of elevation to 2% with three nodes", () => {
    const latitude = 30 * RAD;
    const rho = 9.8 * RAD;
    const n = vec3(Math.cos(latitude), 0, Math.sin(latitude));
    const north = vec3(-Math.sin(latitude), 0, Math.cos(latitude));
    const east = vec3(0, 1, 0);
    const elevation = 10 * RAD;
    const centre = normalise(
      vec3(
        Math.sin(elevation) * n.x + Math.cos(elevation) * north.x,
        Math.sin(elevation) * n.y + Math.cos(elevation) * north.y,
        Math.sin(elevation) * n.z + Math.cos(elevation) * north.z,
      ),
    );
    const up = normalise(
      vec3(
        n.x - dot(n, centre) * centre.x,
        n.y - dot(n, centre) * centre.y,
        n.z - dot(n, centre) * centre.z,
      ),
    );
    const across = cross(centre, up);
    let sum: [number, number, number] = [0, 0, 0];
    let total = 0;
    const cells = 200;
    for (let i = 0; i < cells; i += 1) {
      for (let j = 0; j < cells; j += 1) {
        const x = -rho + ((i + 0.5) * 2 * rho) / cells;
        const y = -rho + ((j + 0.5) * 2 * rho) / cells;
        const r = Math.hypot(x, y);
        if (r > rho) {
          continue;
        }
        // The exponential map about the centre, its Jacobian sin r ÷ r.
        const d: Vec3 =
          r === 0
            ? centre
            : normalise(
                vec3(
                  Math.cos(r) * centre.x + (Math.sin(r) / r) * (x * up.x + y * across.x),
                  Math.cos(r) * centre.y + (Math.sin(r) / r) * (x * up.y + y * across.y),
                  Math.cos(r) * centre.z + (Math.sin(r) / r) * (x * up.z + y * across.z),
                ),
              );
        const mu = dot(d, n);
        if (!(mu > 0)) {
          continue;
        }
        const w = (r === 0 ? 1 : Math.sin(r) / r) * mu;
        const t = lighting().sunTransmittance(
          0,
          mu,
          latitude,
          Math.atan2(dot(d, east), dot(d, north)),
          0,
        );
        sum = [sum[0] + w * t[0], sum[1] + w * t[1], sum[2] + w * t[2]];
        total += w;
      }
    }
    const rule = lighting().sourceTransmittance(0, Math.sin(elevation), latitude, 0, rho, 0);
    const centreOnly = lighting().sunTransmittance(0, Math.sin(elevation), latitude, 0, 0);
    for (const c of [0, 1, 2] as const) {
      const quadrature = sum[c] / total;
      expect(Math.abs(rule[c] / quadrature - 1)).toBeLessThan(0.02);
      // The centre alone misses it by more than the rule does.
      expect(Math.abs(centreOnly[c] / quadrature - 1)).toBeGreaterThan(
        Math.abs(rule[c] / quadrature - 1),
      );
    }
  });

  it("gives a source 2° below the local horizon no direct term and a sky term", () => {
    const mu = -Math.sin(2 * RAD);
    expect(lighting().sunTransmittance(0, mu, 0.7, 0.4, 0)).toEqual([0, 0, 0]);
    expect(lighting().sourceTransmittance(0, mu, 0.7, 0.4, 0.001, 0)).toEqual([0, 0, 0]);
    for (const value of lighting().skyIrradiance(0, mu, 0.7, 0)) {
      expect(value).toBeGreaterThan(0);
    }
    // On a lit body's disc, the star's light is the sky's alone.
    const point = discPoint(40 * RAD);
    const record = recordWith([starAt(point.towards(mu, 1), 1)], []);
    const shaded = shadeDiscPoint(record, CAMERA, point.q, point.ray, lighting());
    const sky = lighting().skyIrradiance(0, muFrom(point.q, point.towards(mu, 1)), 40 * RAD, 0);
    for (const c of [0, 1, 2] as const) {
      const expected = ILLUMINANCE[c] * EXPOSURE_OVER_PI * SCALED_A[c] * sky[c];
      expect(shaded.radiance[c]).toBeGreaterThan(0);
      expect(shaded.radiance[c] / expected - 1).toBeCloseTo(0, 9);
    }
    expect(shaded.lit).toBe(false);
  });

  it("lights a point with a planetshine source as with a star of equal illuminance and direction as ρ → 0", () => {
    const point = discPoint(35 * RAD);
    for (const mu of [0.8, 0.3, 0.05]) {
      const towards = point.towards(mu, 0.9);
      const star = shadeDiscPoint(
        recordWith([starAt(towards, 1e-3)], []),
        CAMERA,
        point.q,
        point.ray,
        lighting(),
      );
      const shine = shadeDiscPoint(
        recordWith([], [secondaryAt(towards, 1e-3)]),
        CAMERA,
        point.q,
        point.ray,
        lighting(),
      );
      for (const c of [0, 1, 2] as const) {
        expect(shine.radiance[c] / star.radiance[c] - 1).toBeCloseTo(0, 5);
      }
    }
  });
});

describe("a body without an atmosphere (R08.T9.b)", () => {
  it("reads R07's stubs, 1 and 0", () => {
    expect(AIRLESS_LIGHTING.sunTransmittance(0, -0.5, 0.2, 1, 0)).toEqual([1, 1, 1]);
    expect(AIRLESS_LIGHTING.skyIrradiance(0, 0.5, 0.2, 0)).toEqual([0, 0, 0]);
    expect(AIRLESS_LIGHTING.sourceTransmittance(0, 0.5, 0.2, 1, 0.1, 0)).toEqual([1, 1, 1]);
  });

  it("keeps R07's planetshine, unchanged", () => {
    const point = discPoint(20 * RAD);
    const towards = point.towards(0.4, 2);
    const source = secondaryAt(towards, 30);
    const shaded = shadeDiscPoint(recordWith([], [source]), CAMERA, point.q, point.ray);
    // R07's lunar-Lambert disc term under the neighbour's irradiance factor (Design note 7).
    const toSource = vec3(
      towards.x * source.distance - point.q.x,
      towards.y * source.distance - point.q.y,
      towards.z * source.distance - point.q.z,
    );
    const distance = Math.hypot(toSource.x, toSource.y, toSource.z);
    const h = planetshineIrradiance(toSource, source.radius, source.distance, point.q);
    const mu0 = dot(scale(toSource, 1 / distance), point.q);
    const mu = -dot(point.ray, point.q);
    const alpha = Math.acos(-dot(scale(toSource, 1 / distance), point.ray));
    const rho = Math.asin(source.radius / distance);
    const l = LAW.lommelSeeligerShare;
    const disc = (l * 2 * h) / Math.max(Math.max(mu0, 0) + mu, rho) + (1 - l) * h;
    const f = phaseFactorFromTable(phaseFactorTableOf(LAW), alpha);
    for (const c of [0, 1, 2] as const) {
      const expected = source.illuminance[c] * EXPOSURE_OVER_PI * SCALED_A[c] * f[c] * disc;
      expect(shaded.radiance[c] / expected - 1).toBeCloseTo(0, 12);
    }
  });

  it("binds one-texel tables of zeros in every one of the seven bindings", () => {
    const made: string[] = [];
    const tables = createAirlessTables({
      createTexture: (spec) => {
        made.push(spec.name);
        return { kind: "texture", name: spec.name };
      },
    });
    expect(made).toEqual(["atmosphere: no air"]);
    expect(new Set(Object.values(tables)).size).toBe(1);
  });
});

describe("the bands at a Saturn-class figure's datum (R08.T9.b)", () => {
  it("give its pole less diffuse light than its equator under the same sun, by their columns' ratio", () => {
    const planet = oblatePlanetTwin(saturnClassMedium(), saturnBody(), [], SIZES);
    const irradiance = oblateIrradianceTwin(planet, IRRADIANCE_KERNEL_OPTIONS, [[63, 0]]);
    const saturn = surfaceLightingTwin(planet, irradiance);
    const equator = saturn.skyIrradiance(0, 1, 0, 0);
    const pole = saturn.skyIrradiance(0, 1, Math.PI / 2, 0);
    const bands = planet.bands;
    const first = bands[0];
    const last = bands[bands.length - 1];
    if (first === undefined || last === undefined) {
      throw new Error("the Saturn-class figure has bands");
    }
    // The bands' vertical columns at the datum: the reference's over each band's s.
    const columnRatio = first.gravityRatio / last.gravityRatio;
    expect(columnRatio).toBeCloseTo(0.7541, 3);
    // Thin, the sky's light goes as the column above; deeper, it saturates towards 1, so the ratio
    // stands above the columns' and nears it as the channel thins: red, the thinnest (τ 0.16 at the
    // equator), within 5% of it.
    const ratios = [0, 1, 2].map((c) => (pole[c] ?? Number.NaN) / (equator[c] ?? Number.NaN));
    for (const ratio of ratios) {
      expect(ratio).toBeGreaterThan(columnRatio);
      expect(ratio).toBeLessThan(1);
    }
    expect(ratios[0]).toBeLessThan(ratios[1] ?? 0);
    expect(ratios[1]).toBeLessThan(ratios[2] ?? 0);
    expect((ratios[0] ?? Number.NaN) / columnRatio - 1).toBeLessThan(0.05);
  }, 120_000);
});

describe("the surface lighting's WGSL (R08.T9.b)", () => {
  it("calls no builtin trigonometry, R05 radius form or sebh remap", () => {
    expect(forbiddenIn(surfaceLightingWgsl)).toEqual([]);
    expect(forbiddenIn(irradianceWgsl)).toEqual([]);
    expect(forbiddenIn(IRRADIANCE_KERNEL.reference)).toEqual([]);
    expect(forbiddenIn(SURFACE_LIGHTING_WGSL)).toEqual([]);
  });

  it("exports the three lookups with the light's slot, in place of litBody.wgsl's stubs", () => {
    expect(surfaceLightingWgsl).toMatch(
      /fn atmosphere_sun_transmittance\(\s*altitude_m : f32,\s*mu_sun : f32,\s*latitude_rad : f32,\s*sun_azimuth_rad : f32,\s*curve : u32,\s*\) -> vec3f/u,
    );
    expect(surfaceLightingWgsl).toMatch(
      /fn atmosphere_sky_irradiance\(\s*altitude_m : f32,\s*mu_sun : f32,\s*latitude_rad : f32,\s*curve : u32,\s*\) -> vec3f/u,
    );
    expect(surfaceLightingWgsl).toMatch(
      /fn atmosphere_source_transmittance\(\s*altitude_m : f32,\s*mu_centre : f32,\s*latitude_rad : f32,\s*azimuth_rad : f32,\s*angular_radius_rad : f32,\s*curve : u32,\s*\) -> vec3f/u,
    );
    expect(litBodyWgsl).not.toMatch(/fn atmosphere_/u);
    for (const source of [
      BODY_DISC_MATERIALS.interior.fragmentWgsl,
      BODY_DISC_MATERIALS.limb.fragmentWgsl,
      SMOOTH_MESH_MATERIAL.fragmentWgsl,
      BODY_DISC_CELLS_KERNEL.reference,
    ]) {
      expect(source).toContain(SURFACE_LIGHTING_WGSL);
      expect(source.indexOf(SURFACE_LIGHTING_WGSL)).toBeLessThan(source.indexOf(litBodyWgsl));
    }
  });

  it("has the disc take each star's sky at its own μ₀ and slot, and planetshine through the three-node rule", () => {
    const code = bodyDiscWgsl.replaceAll(/\s+/gu, " ");
    expect(code).toContain("atmosphere_sky_irradiance(0.0, mu0, local.latitude, j)");
    expect(code).toContain(
      "atmosphere_sun_transmittance( 0.0, mu0, local.latitude, local_azimuth(local, towards), j )",
    );
    expect(code).toContain("atmosphere_sky_irradiance(0.0, mu0, local.latitude, 0u)");
    expect(code).toContain(
      "atmosphere_source_transmittance( 0.0, mu0, local.latitude, local_azimuth(local, towards), source_radius, 0u )",
    );
    expect(code).not.toContain("first_mu0");
    // Each sky term before the horizon's `continue`, a star's after its eclipse and under it.
    const starSky = code.indexOf(
      "out.radiance = out.radiance + light.xyz * misc.z * mean_a * sky * visible * ring;",
    );
    expect(starSky).toBeGreaterThan(code.indexOf("visible = max(visible, vec3f(0.0));"));
    expect(starSky).toBeLessThan(code.indexOf("if (horizon <= 0.0) { continue; }"));
    expect(code.indexOf("atmosphere_sky_irradiance(0.0, mu0, local.latitude, 0u)")).toBeLessThan(
      code.indexOf("if (irradiance <= 0.0) { continue; }"),
    );
  });

  it("binds the body's tables to the disc materials and the mesh at group 2 bindings 6 to 12", () => {
    const expected = surfaceLightingTextures(6);
    expect(expected.map(({ binding }) => binding)).toEqual([6, 7, 8, 9, 10, 11, 12]);
    for (const spec of [
      BODY_DISC_MATERIALS.interior,
      BODY_DISC_MATERIALS.limb,
      SMOOTH_MESH_MATERIAL,
    ]) {
      // The mesh's coverage mask (plan R10, R10.T10.c) follows the tables, at binding 13.
      expect(spec.textures?.slice(2, 9)).toEqual(expected);
    }
    expect(SMOOTH_MESH_MATERIAL.textures?.slice(9).map(({ binding }) => binding)).toEqual([13]);
    for (const [i, name] of [
      "atmosphere_body",
      "atmosphere_transmittance",
      "atmosphere_absorber_columns",
      "atmosphere_absorber_pressure_columns",
      "atmosphere_last_absorber_pressure_column",
      "atmosphere_absorber_curves",
      "atmosphere_irradiance",
    ].entries()) {
      expect(bodyDiscWgsl).toMatch(
        new RegExp(`@group\\(2\\) @binding\\(${6 + i}\\) var ${name} :`, "u"),
      );
    }
  });
});

// --- A lit body's disc point, for the shading's twin ---------------------------------------------

/** A camera along the galactic axes. */
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

/** A lunar-Lambert law. */
const LAW = lawFor([0.3, 0.25, 0.2], [1.1, 1.1, 1.1], "mars");

/** Its A as a sphere's record scales it (`scaledLaws`: `oblateAlbedoScale` at c ÷ a = 1). */
const ALBEDO_SCALE = oblateAlbedoScale(LAW.lommelSeeligerShare, 1);
const SCALED_A: readonly [number, number, number] = [
  LAW.a[0] * ALBEDO_SCALE,
  LAW.a[1] * ALBEDO_SCALE,
  LAW.a[2] * ALBEDO_SCALE,
];

/** μ₀ at the point `q` ÷ a towards a source in direction `towards` {@link SOURCE_DISTANCE} off. */
function muFrom(q: Vec3, towards: Vec3): number {
  const to = vec3(
    towards.x * SOURCE_DISTANCE - q.x,
    towards.y * SOURCE_DISTANCE - q.y,
    towards.z * SOURCE_DISTANCE - q.z,
  );
  return dot(q, to) / Math.hypot(to.x, to.y, to.z);
}

/** A star's illuminance face-on, lx. */
const ILLUMINANCE: readonly [number, number, number] = [1.1e5, 1e5, 0.9e5];

/** The exposure over π the record carries. */
const EXPOSURE_OVER_PI = 1e-4 / Math.PI;

/** How far the sources stand, in the body's radii. */
const SOURCE_DISTANCE = 1e6;

/**
 * A point of a sphere about the pole z at latitude `latitudeRad`, seen from straight above it: its
 * place ÷ a (its normal), the ray to it, and the direction at zenith cosine `mu` and azimuth
 * `azimuthRad` from its north.
 */
function discPoint(latitudeRad: number): {
  readonly q: Vec3;
  readonly ray: Vec3;
  readonly towards: (mu: number, azimuthRad: number) => Vec3;
} {
  const q = vec3(Math.cos(latitudeRad), 0, Math.sin(latitudeRad));
  const north = vec3(-Math.sin(latitudeRad), 0, Math.cos(latitudeRad));
  const east = vec3(0, 1, 0);
  return {
    q,
    ray: scale(q, -1),
    towards: (mu, azimuthRad) => {
      const horizontal = Math.sqrt(1 - mu * mu);
      return normalise(
        vec3(
          mu * q.x + horizontal * (Math.cos(azimuthRad) * north.x + Math.sin(azimuthRad) * east.x),
          mu * q.y + horizontal * (Math.cos(azimuthRad) * north.y + Math.sin(azimuthRad) * east.y),
          mu * q.z + horizontal * (Math.cos(azimuthRad) * north.z + Math.sin(azimuthRad) * east.z),
        ),
      );
    },
  };
}

/** A star in direction `towards` of radius `radius` ÷ a, far off. */
function starAt(towards: Vec3, radius: number): DiscRecord["lights"][number] {
  const set = annulusEdges(0.6, 0.6, 4);
  return {
    direction: towards,
    distance: SOURCE_DISTANCE,
    radius,
    illuminance: ILLUMINANCE,
    annuli: [set, set, set],
  };
}

/** A lit neighbour in direction `towards` of radius `radius` ÷ a, its illuminance the star's. */
function secondaryAt(towards: Vec3, radius: number): DiscRecord["secondaries"][number] {
  return { direction: towards, distance: SOURCE_DISTANCE, radius, illuminance: ILLUMINANCE };
}

/** A sphere's record about the pole z, straight ahead of the camera, with these lights. */
function recordWith(
  lights: DiscRecord["lights"],
  secondaries: DiscRecord["secondaries"],
): DiscRecord {
  return {
    body: "body",
    rect: { leftPx: 0, topPx: 0, rightPx: 1, bottomPx: 1 },
    direction: vec3(0, 0, -1),
    radiusOverDistance: 1e-3,
    pole: vec3(0, 0, 1),
    polarOverEquatorial: 1,
    interiorSamples: 1,
    limbSamples: 4,
    surface: { kind: "uniform", law: LAW },
    tableRows: [0],
    exposureOverPi: EXPOSURE_OVER_PI,
    lights,
    occluders: [],
    secondaries,
  };
}
