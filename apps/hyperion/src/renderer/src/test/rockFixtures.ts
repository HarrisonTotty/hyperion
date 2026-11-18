/**
 * Rocks for the rock layer's, mesh's and pages' tests (plan R11, T4): drawn from a seeded source
 * over the surface crate's ranges, each standing on a random spheroid point's normal.
 */

import { cross, normalise, scale, type Vec3, vec3 } from "../geometry/vec3";
import { type MeshPage, readMeshIndex, readMeshPage } from "../view/clusters/pageFormat";
import {
  ROCK_WORDS,
  type Rock,
  type RockLayer,
  rockLayerOf,
} from "../view/surface/scatter/rockLayer";
import { between, seededRandom } from "./seededRandom";

/**
 * Γ(1 + x) for x in (0, 1] by Lanczos' approximation (g = 7, nine terms), to about 15 digits:
 * the superquadric's outline area factor needs it.
 */
function gammaOnePlus(x: number): number {
  const g = 7;
  const c = [
    0.999_999_999_999_809_9, 676.520_368_121_885_1, -1_259.139_216_722_402_8, 771.323_428_777_653_1,
    -176.615_029_162_140_6, 12.507_343_278_686_905, -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6, 1.505_632_735_149_311_6e-7,
  ];
  let sum = c[0] ?? 0;
  for (let k = 1; k < c.length; k += 1) {
    sum += (c[k] ?? 0) / (x + k);
  }
  const t = x + g + 0.5;
  return Math.sqrt(2 * Math.PI) * t ** (x + 0.5) * Math.exp(-t) * sum;
}

/** The share of its box's area a superquadric outline of exponent `n` covers (`shape.rs`). */
function outlineAreaFactor(n: number): number {
  const g = gammaOnePlus(1 / n);
  return (g * g) / gammaOnePlus(2 / n);
}

/** A rock of `diameterM` at the unit direction `up`, its shape drawn from `random`. */
export function rockOf(random: () => number, diameterM: number, up: Vec3, baseM: Vec3): Rock {
  const heightRatio = between(random, 0.25, 0.5);
  const aspect = between(random, 0.55, 0.9);
  const yaw = between(random, 0, 2 * Math.PI);
  const exponent = between(random, 2, 3);
  const ab = (Math.PI * diameterM * diameterM) / (16 * outlineAreaFactor(exponent));
  const reference = Math.abs(up.z) < 0.9 ? vec3(0, 0, 1) : vec3(1, 0, 0);
  const east = normalise(cross(reference, up));
  const north = cross(up, east);
  const [sin, cos] = [Math.sin(yaw), Math.cos(yaw)];
  const along = vec3(
    cos * east.x + sin * north.x,
    cos * east.y + sin * north.y,
    cos * east.z + sin * north.z,
  );
  const across = cross(up, along);
  return {
    baseCentreM: baseM,
    axes: [along, across, up],
    semiAxesM: [Math.sqrt(ab / aspect), Math.sqrt(ab * aspect), heightRatio * diameterM],
    exponent,
    groundHeightM: 0,
    dir: up,
  };
}

/**
 * `count` rocks from `seed`, their diameters log-uniform over the authoritative octaves' 0.4 to
 * 6.4 m, each on a sphere of `radiusM` at a random direction.
 */
export function randomRocks(seed: number, count: number, radiusM = 1_737_400): Rock[] {
  const random = seededRandom(seed);
  return Array.from({ length: count }, () => {
    const diameterM = 0.4 * 16 ** random();
    const up = normalise(
      vec3(between(random, -1, 1), between(random, -1, 1), between(random, -1, 1)),
    );
    return rockOf(random, diameterM, up, scale(up, radiusM));
  });
}

/** A vector's three components. */
function componentsOf(p: Vec3): number[] {
  return [p.x, p.y, p.z];
}

/** `rocks` as a bake's rock layer lays them out (`rockLayer.ts`' word order). */
export function rockLayerFrom(rocks: ReadonlyArray<Rock>): RockLayer {
  const words = new Float64Array(ROCK_WORDS * rocks.length);
  rocks.forEach((rock, n) => {
    const v = componentsOf;
    words.set(
      [
        ...v(rock.baseCentreM),
        ...rock.axes.flatMap(v),
        ...rock.semiAxesM,
        rock.exponent,
        rock.groundHeightM,
        ...v(rock.dir),
      ],
      ROCK_WORDS * n,
    );
  });
  return rockLayerOf(words);
}

/**
 * `count` rocks from `seed` scattered over a square of `sideM` about the direction `centre` on a
 * sphere of `radiusM`, their diameters log-uniform over `diametersM`, by default 0.4 to 6.4 m.
 */
export function rocksAbout(
  seed: number,
  count: number,
  centre: Vec3,
  sideM: number,
  radiusM = 1_737_400,
  diametersM: readonly [number, number] = [0.4, 6.4],
): Rock[] {
  const random = seededRandom(seed);
  const up = normalise(centre);
  const east = normalise(cross(Math.abs(up.z) < 0.9 ? vec3(0, 0, 1) : vec3(1, 0, 0), up));
  const north = cross(up, east);
  return Array.from({ length: count }, () => {
    const [x, y] = [between(random, -0.5, 0.5) * sideM, between(random, -0.5, 0.5) * sideM];
    const dir = normalise(
      vec3(
        up.x * radiusM + x * east.x + y * north.x,
        up.y * radiusM + x * east.y + y * north.y,
        up.z * radiusM + x * east.z + y * north.z,
      ),
    );
    const [least, most] = diametersM;
    return rockOf(random, least * (most / least) ** random(), dir, scale(dir, radiusM));
  });
}

/**
 * The pages of the mesh file `file`, read as the producer reads them.
 *
 * @throws Error naming why the file or a page was refused.
 */
export function meshPagesOf(file: ArrayBuffer): MeshPage[] {
  const index = readMeshIndex(file);
  if (index.kind !== "read") {
    throw new Error(index.reason);
  }
  return index.value.pages.map((entry, p) => {
    const page = readMeshPage(
      file.slice(entry.offsetBytes, entry.offsetBytes + entry.bytes),
      entry,
      p,
    );
    if (page.kind !== "read") {
      throw new Error(page.reason);
    }
    return page.value;
  });
}
