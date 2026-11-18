/**
 * Ocean maps for the ocean's tests (plan R11, R11.T8.b): a worker's `OceanMapAnswer` built from a
 * few cells of a level-5 field, every other cell unsurveyed.
 */
import type { BodyIdHex, LiquidDto, SurfaceHeaderDto } from "@hyperion/protocol";

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

/** `testing/liquids.rs`' liquid water at 25 °C, as the surface payload carries it. */
export const WATER_RECORD: LiquidDto = {
  temperature_k: 298.15,
  density_kg_m3: 997.047,
  basis: "measured",
  refractive_index: [
    1.343789, 1.341476, 1.339564, 1.337962, 1.336599, 1.335422, 1.334396, 1.333487, 1.332676,
    1.331946, 1.331283, 1.330673, 1.330109, 1.329581, 1.329086,
  ],
  absorption_per_m: [
    0.001875, 0.00305405, 0.00624414, 0.0103012, 0.0177158, 0.039037, 0.0517617, 0.073031, 0.180294,
    0.27759, 0.334405, 0.442138, 0.601402, 1.44247, 2.80088,
  ],
  scattering_per_m: [
    0.005469387272826296, 0.004181643650890754, 0.0032546413241809694, 0.0025727588408902564,
    0.002061546784888825, 0.001671864057626776, 0.0013704710557889293, 0.001134243878283069,
    0.0009469343271450381, 0.0007968215862562356, 0.0006753565140614371, 0.0005761861743582379,
    0.0004945704130290098, 0.00042689541857878524, 0.00037039794350413535,
  ],
  surface_tension_n_per_m: 0.07197,
  viscosity_pa_s: 8.9e-4,
};

/**
 * An Earth-like body's field header at the fixtures' level: WGS 84's semi-axes (6,378,137 m and
 * 6,356,752.314245 m, NIMA TR8350.2 Table 3.1), the ISA's sea-level pressure and temperature
 * (101,325 Pa, 288.15 K), a basalt crust and a water sea, its main liquid; its other figures
 * plausible stand-ins, which no ocean test reads.
 */
export function waterWorldHeader(body: BodyIdHex): SurfaceHeaderDto {
  return {
    body,
    level: OCEAN_FIXTURE_LEVEL,
    radius_m: 6_371_000,
    equatorial_radius_m: 6_378_137,
    polar_radius_m: 6_356_752.314245,
    boundary_diameter_m: 600_000,
    sea_level_m: -120,
    lapse_rate_k_per_m: 0.0065,
    realised_sigma_h_m: 2_400,
    realised_relief_m: 19_000,
    months: 12,
    season_eccentricity: 0.0167,
    reference_temperature_k: 288.15,
    surface_age_gyr: 4.5,
    surface_pressure_pa: 101_325,
    albedo_scale: null,
    climate_model: "radiative_equilibrium",
    precipitation: "heuristic",
    palette: [
      {
        substance: "basalt",
        role: "primary_crust",
        normal_albedo_bvr: [0.1, 0.11, 0.12],
        phase_row: 0,
        liquid: null,
      },
      {
        substance: "H2O",
        role: "liquid",
        normal_albedo_bvr: [0.05, 0.05, 0.05],
        phase_row: 0,
        liquid: WATER_RECORD,
      },
    ],
    main_liquid: 1,
  };
}
