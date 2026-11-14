/**
 * Ocean maps for the ocean's tests (plan R11, R11.T8.b): a worker's `OceanMapAnswer` built from a
 * few cells of a level-5 field, every other cell unsurveyed.
 */
import { fieldCellIndex } from "../view/terrain/coverage";
import { faceUvToXyz, stToUv, type Xyz } from "../view/terrain/cube";
import type { Face } from "../view/terrain/patchKey";
import {
  OCEAN_MAP_MONTHS,
  OCEAN_MAP_WIND_BYTES,
  type OceanMapAnswer,
} from "../view/terrain/workers/messages";

/** The fixtures' field level: 32 cells along a face's side. */
export const OCEAN_FIXTURE_LEVEL = 5;

/** The speed of each of R09's wind speed codes, m/s: 0.01 × 2^((c − 1) ÷ 16), 0 for code 0. */
export const WIND_SPEEDS_MPS: Float64Array = Float64Array.from({ length: 256 }, (_, c) =>
  c === 0 ? 0 : 0.01 * 2 ** ((c - 1) / 16),
);

/** One surveyed cell of a fixture. */
export interface OceanFixtureCell {
  readonly face: Face;
  readonly i: number;
  readonly j: number;
  /** The water surface less the ground, m; 0 for dry ground. */
  readonly depthM: number;
  /** The water surface less the sea level, m. */
  readonly levelM: number;
  readonly ice: number;
  /** Each month's (azimuth code, speed code); a month absent is calm. */
  readonly winds?: ReadonlyArray<readonly [number, number]>;
}

/** The answer a worker gives for `cells`, rising by cell index, the others unsurveyed. */
export function oceanAnswerOf(
  cells: ReadonlyArray<OceanFixtureCell>,
  months = 12,
  seasonEccentricity = 0.0167,
): OceanMapAnswer {
  const sorted = cells
    .map((cell) => ({
      cell,
      index: fieldCellIndex({ face: cell.face, level: OCEAN_FIXTURE_LEVEL, i: cell.i, j: cell.j }),
    }))
    .toSorted((a, b) => a.index - b.index);
  const winds = new Uint8Array(OCEAN_MAP_WIND_BYTES * sorted.length);
  sorted.forEach(({ cell }, k) => {
    for (let month = 0; month < OCEAN_MAP_MONTHS; month += 1) {
      const [azimuth, speed] = cell.winds?.[month] ?? [0, 0];
      winds[OCEAN_MAP_WIND_BYTES * k + 2 * month] = azimuth;
      winds[OCEAN_MAP_WIND_BYTES * k + 2 * month + 1] = speed;
    }
  });
  return {
    level: OCEAN_FIXTURE_LEVEL,
    seaLevelM: -120,
    months,
    seasonEccentricity,
    cells: Uint32Array.from(sorted, (s) => s.index),
    depthM: Float32Array.from(sorted, (s) => s.cell.depthM),
    levelM: Float32Array.from(sorted, (s) => s.cell.levelM),
    ice: Uint8Array.from(sorted, (s) => s.cell.ice),
    winds,
    windSpeedsMps: WIND_SPEEDS_MPS.slice(),
    complete: true,
  };
}

/** The body-fixed direction of the centre of cell (face, i, j) of the fixtures' level. */
export function oceanCellCentre(face: Face, i: number, j: number): Xyz {
  const n = 2 ** OCEAN_FIXTURE_LEVEL;
  return faceUvToXyz(face, stToUv((i + 0.5) / n), stToUv((j + 0.5) / n));
}

/**
 * A world all open water at the fixtures' level, `depthM` deep, under one wind (towards `azimuth`,
 * R09's code, at speed code `speed`) every month, with `cells` in place of the sea where given.
 */
export function waterWorldAnswer(
  depthM: number,
  azimuth: number,
  speed: number,
  cells: ReadonlyArray<OceanFixtureCell> = [],
): OceanMapAnswer {
  const n = 2 ** OCEAN_FIXTURE_LEVEL;
  const winds: ReadonlyArray<readonly [number, number]> = Array.from(
    { length: OCEAN_MAP_MONTHS },
    () => [azimuth, speed],
  );
  const own = new Set(cells.map((cell) => `${cell.face} ${cell.i} ${cell.j}`));
  const all: OceanFixtureCell[] = [...cells];
  for (let face = 0; face < 6; face += 1) {
    for (let j = 0; j < n; j += 1) {
      for (let i = 0; i < n; i += 1) {
        if (!own.has(`${face} ${i} ${j}`)) {
          all.push({ face: faceOf(face), i, j, depthM, levelM: 0, ice: 0, winds });
        }
      }
    }
  }
  return oceanAnswerOf(all);
}

/** Face `n` of the cube sphere. */
function faceOf(n: number): Face {
  switch (n) {
    case 0:
      return 0;
    case 1:
      return 1;
    case 2:
      return 2;
    case 3:
      return 3;
    case 4:
      return 4;
    default:
      return 5;
  }
}
