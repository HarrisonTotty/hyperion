/**
 * A body's cloud coverage over its whole sphere, and its motion (plan R11, R11.T7.a; Design notes 8
 * and 19): the share of the sky its clouds cover at each place, from a terrain-free zonal
 * climatology refined where the ground is surveyed, scaled once to plan 14's cloud fraction, and
 * the stateless flow map that moves the clouds' structure along the wind.
 *
 * @remarks
 * - **A labelled heuristic** (Design note 8). A place's standardised log precipitation z, the zonal
 *   climatology's ({@link ZonalClimate}) blended over surveyed ground with its own cell's
 *   ({@link SurveyedClimate}), goes through a logistic to a pseudo-humidity
 *   RH = 1 ÷ (1 + e^(−(z − b))), and then through Sundqvist et al. 1989's closure,
 *   C = 1 − √((1 − RH) ÷ (1 − RH_c)) above RH_c ({@link sundqvistFraction}), with a low marine
 *   term over a surveyed sea cell colder than its latitude band (after Klein and Hartmann 1993,
 *   {@link marineStratus}), combined as 1 − (1 − C_conv)(1 − C_strat).
 * - **One offset a season.** b is solved by bisection ({@link calibrateOffset}) so that the
 *   area-weighted mean over the whole sphere equals plan 14's cloud fraction, on what the body holds
 *   when it is solved, the zonal field before any survey; it is never solved again over the
 *   surveyed subset, so surveying new ground changes no coverage outside the cells it blends. The
 *   season is the subsolar latitude ({@link CloudSeason}), which the stand-in climatology reads in
 *   place of R09's months (`month_blend` has no client twin; plan R11's Consumes, re-validated
 *   2026-11-02): the seasons then turn with no step at a month's edge.
 * - **The zonal climatology is provisional.** R09's terrain-free zonal part, asked of R09, does not
 *   exist; {@link PROVISIONAL_ZONAL_CLIMATE} stands in, a rain band about the subsolar latitude,
 *   dry belts and storm tracks shifted with it, labelled provisional (plan R11's Risks).
 * - **Motion** is a two-phase flow map ({@link flowMapPhases}; Neyret 2003, Vlachos 2010): each
 *   phase samples the structure upwind of a place by its advection time, the two phases half a
 *   period apart, each restarting where its weight is 0, so the field is a pure function of the
 *   scene's time with no accumulated state. Under time warp the structure fades to the calibrated
 *   mean ({@link timeRateFade}).
 * - Pure: no state, no platform API; `f64`, with `pinnedMath`'s transcendentals, so that two
 *   clients compute the same coverage bit for bit.
 */
import type { Vec3 } from "../../geometry/vec3";
import { cos, exp, sin } from "../../lib/pinnedMath";

/** A season of a body's clouds: where its brightest star stands over it. */
export interface CloudSeason {
  /**
   * The subsolar latitude, rad, in [−π ÷ 2, π ÷ 2]: the star's declination in the body-fixed axes,
   * asin of the z component of its unit direction.
   */
  readonly subsolarLatitudeRad: number;
}

/**
 * A zonal climatology: the standardised log precipitation z at a latitude in a season, mean 0 and
 * standard deviation 1 over the sphere's area, dimensionless.
 */
export type ZonalClimate = (season: CloudSeason) => (latitudeRad: number) => number;

/** A surveyed place's own climate (R09's `ClimateCell`, read for the clouds). */
export interface SurveyedCell {
  /**
   * How much the cell's own climate replaces the zonal one, in [0, 1]: 1 inside the survey, falling
   * to 0 across the two or three coarse cells of its edge (Design note 8).
   */
  readonly weight: number;
  /** Its own standardised log precipitation in the season, dimensionless. */
  readonly precipitationZ: number;
  /**
   * For a sea cell, how much colder it is than its latitude band's mean, K (positive colder); `null`
   * over land, where no marine term applies.
   */
  readonly seaColdAnomalyK: number | null;
}

/** The surveyed refinement of a body's coverage: each place's own cell, `null` off the survey. */
export type SurveyedClimate = (latitudeRad: number, longitudeRad: number) => SurveyedCell | null;

/**
 * RH_c, the pseudo-humidity at which Sundqvist et al. 1989's closure starts to form cloud: 0.8, the
 * threshold relative humidity its scheme is commonly run with (Sundqvist, Berge and Kristjánsson
 * 1989, Mon. Wea. Rev. 117, 1641; from memory: the re-check of 2026-11-13 reached no source, and
 * models running the scheme take 0.75 to 0.9, often by height).
 */
export const CRITICAL_RELATIVE_HUMIDITY = 0.8;

/**
 * The low marine cloud's share gained per kelvin of a sea cell's cold anomaly against its latitude
 * band: 0.057 K⁻¹, the slope of Klein and Hartmann 1993's low stratus amount against the lower
 * troposphere's stability θ₇₀₀ − θ_sfc (J. Climate 6, 1587; as CAM3 takes it, 0.057 (θ₇₀₀ − θ_sfc)
 * − 0.5573; from memory, the re-check of 2026-11-13 reaching no source), the anomaly standing in for
 * the stability and the fit's intercept left out (Design note 8).
 */
export const MARINE_STRATUS_PER_K = 0.057;

/** The pseudo-humidity of a place: the logistic of its z less the body's offset. */
export function pseudoHumidity(precipitationZ: number, offset: number): number {
  return 1 / (1 + exp(offset - precipitationZ));
}

/** Sundqvist et al. 1989's cloud fraction at a pseudo-humidity, in [0, 1]. */
export function sundqvistFraction(relativeHumidity: number): number {
  if (!(relativeHumidity > CRITICAL_RELATIVE_HUMIDITY)) {
    return 0;
  }
  const dry = Math.max(1 - relativeHumidity, 0) / (1 - CRITICAL_RELATIVE_HUMIDITY);
  return 1 - Math.sqrt(dry);
}

/** The low marine cloud's share over a sea cell `coldAnomalyK` colder than its band, in [0, 1]. */
export function marineStratus(coldAnomalyK: number): number {
  return Math.min(Math.max(MARINE_STRATUS_PER_K * coldAnomalyK, 0), 1);
}

/** The ITCZ's latitude over the subsolar latitude's, in the stand-in climatology. */
const RAIN_BAND_SHIFT = 0.4;

/** A Gaussian bump of standard deviation `widthRad` at `centreRad`, 1 at its centre. */
function bump(latitudeRad: number, centreRad: number, widthRad: number): number {
  const x = (latitudeRad - centreRad) / widthRad;
  return exp(-0.5 * x * x);
}

const DEG = Math.PI / 180;

/** The stand-in's unstandardised log precipitation at a latitude, about the rain band at `bandRad`. */
function provisionalLogPrecipitation(latitudeRad: number, bandRad: number): number {
  return (
    bump(latitudeRad, bandRad, 7 * DEG) +
    bump(latitudeRad, bandRad + 50 * DEG, 12 * DEG) +
    bump(latitudeRad, bandRad - 50 * DEG, 12 * DEG) -
    bump(latitudeRad, bandRad + 25 * DEG, 9 * DEG) -
    bump(latitudeRad, bandRad - 25 * DEG, 9 * DEG)
  );
}

/** The latitude rows the stand-in is standardised over, centre to centre. */
const STANDARDISE_ROWS = 720;

/**
 * The provisional zonal climatology (plan R11, T7.a; Design note 8), until R09 provides its
 * terrain-free zonal part: a rain band on the energy-flux equator, taken as 0.4 of the subsolar
 * latitude, of width 7°; storm tracks 50° poleward of it, 12° wide; dry belts 25° poleward, 9° wide;
 * each of unit height in log precipitation, standardised over the sphere's area at the season. A
 * labelled heuristic of low confidence, an Earth-like pattern placed on every world (lean,
 * 2026-11-02), which T7.d's zonal check compares with ISCCP's: high at the rain band, minima near
 * ±25°, high near ±50°.
 */
export const PROVISIONAL_ZONAL_CLIMATE: ZonalClimate = (season) => {
  const band = RAIN_BAND_SHIFT * season.subsolarLatitudeRad;
  let weight = 0;
  let sum = 0;
  let sumSquares = 0;
  for (let j = 0; j < STANDARDISE_ROWS; j += 1) {
    const lower = -Math.PI / 2 + (j * Math.PI) / STANDARDISE_ROWS;
    const upper = -Math.PI / 2 + ((j + 1) * Math.PI) / STANDARDISE_ROWS;
    const area = sin(upper) - sin(lower);
    const value = provisionalLogPrecipitation(0.5 * (lower + upper), band);
    weight += area;
    sum += area * value;
    sumSquares += area * value * value;
  }
  const mean = sum / weight;
  const deviation = Math.sqrt(Math.max(sumSquares / weight - mean * mean, 0));
  return (latitudeRad) => (provisionalLogPrecipitation(latitudeRad, band) - mean) / deviation;
};

/** A place's coverage from its climate and the body's offset (the module's remarks). */
export function coverageOf(zonalZ: number, surveyed: SurveyedCell | null, offset: number): number {
  const weight = surveyed === null ? 0 : Math.min(Math.max(surveyed.weight, 0), 1);
  const z = surveyed === null ? zonalZ : (1 - weight) * zonalZ + weight * surveyed.precipitationZ;
  const convective = sundqvistFraction(pseudoHumidity(z, offset));
  const cold = surveyed?.seaColdAnomalyK ?? null;
  const marine = cold === null ? 0 : weight * marineStratus(cold);
  return 1 - (1 - convective) * (1 - marine);
}

/** A coverage field on a latitude–longitude grid of cells. */
export interface CoverageGrid {
  /** Cells around a parallel, from longitude −π eastwards. */
  readonly widthCells: number;
  /** Cells along a meridian, from the north pole southwards. */
  readonly heightCells: number;
  /**
   * Each cell's coverage at its centre, in [0, 1], row-major: cell (i, j) at j × width + i, at
   * latitude π ÷ 2 − (j + ½) π ÷ height and longitude −π + (i + ½) 2π ÷ width.
   */
  readonly fractions: Float64Array;
  /** The offset b the field was made with. */
  readonly offset: number;
}

/** What a coverage field is made from. */
export interface CoverageInput {
  readonly zonal: ZonalClimate;
  /** The surveyed refinement; `null` before any survey. */
  readonly surveyed: SurveyedClimate | null;
  readonly season: CloudSeason;
  readonly widthCells: number;
  readonly heightCells: number;
}

/** A grid cell's centre's latitude, rad. */
export function cellLatitudeRad(row: number, heightCells: number): number {
  return Math.PI / 2 - ((row + 0.5) * Math.PI) / heightCells;
}

/** A grid cell's centre's longitude, rad. */
export function cellLongitudeRad(column: number, widthCells: number): number {
  return -Math.PI + ((column + 0.5) * 2 * Math.PI) / widthCells;
}

/** A grid row's share of the sphere's solid angle, its true area: (sin φ₁ − sin φ₂) ÷ 2. */
export function rowSolidAngleShare(row: number, heightCells: number): number {
  const north = Math.PI / 2 - (row * Math.PI) / heightCells;
  const south = Math.PI / 2 - ((row + 1) * Math.PI) / heightCells;
  return 0.5 * (sin(north) - sin(south));
}

function requireGrid(widthCells: number, heightCells: number): void {
  if (!(Number.isInteger(widthCells) && widthCells > 0)) {
    throw new RangeError(`a coverage grid's width ${widthCells} must be a positive integer`);
  }
  if (!(Number.isInteger(heightCells) && heightCells > 0)) {
    throw new RangeError(`a coverage grid's height ${heightCells} must be a positive integer`);
  }
}

/** Each cell's zonal z and surveyed cell, which every offset of a calibration shares. */
interface GridClimate {
  readonly zonalZ: Float64Array;
  readonly surveyed: ReadonlyArray<SurveyedCell | null>;
}

function gridClimate(input: CoverageInput): GridClimate {
  const { widthCells: w, heightCells: h } = input;
  requireGrid(w, h);
  const zonalAt = input.zonal(input.season);
  const zonalZ = new Float64Array(w * h);
  const surveyed: Array<SurveyedCell | null> = [];
  for (let j = 0; j < h; j += 1) {
    const latitude = cellLatitudeRad(j, h);
    const z = zonalAt(latitude);
    for (let i = 0; i < w; i += 1) {
      zonalZ[j * w + i] = z;
      surveyed.push(input.surveyed?.(latitude, cellLongitudeRad(i, w)) ?? null);
    }
  }
  return { zonalZ, surveyed };
}

function fractionsOf(
  climate: GridClimate,
  offset: number,
  widthCells: number,
  heightCells: number,
): Float64Array {
  const fractions = new Float64Array(widthCells * heightCells);
  for (let k = 0; k < fractions.length; k += 1) {
    fractions[k] = coverageOf(climate.zonalZ[k] ?? Number.NaN, climate.surveyed[k] ?? null, offset);
  }
  return fractions;
}

/** A grid's coverage, its cells weighted by their true solid angle. */
export function areaMeanCoverage(grid: CoverageGrid): number {
  const { widthCells: w, heightCells: h, fractions } = grid;
  let mean = 0;
  for (let j = 0; j < h; j += 1) {
    let row = 0;
    for (let i = 0; i < w; i += 1) {
      row += fractions[j * w + i] ?? Number.NaN;
    }
    mean += (rowSolidAngleShare(j, h) * row) / w;
  }
  return mean;
}

/** The bisection's bracket on b, wide enough that the logistic saturates at either end. */
const OFFSET_BRACKET: readonly [number, number] = [-40, 40];

/** The bisection's halvings: the bracket's 80 to under 10⁻¹⁶ of a unit of z. */
const OFFSET_HALVINGS = 64;

/**
 * The offset b at which `input`'s field covers `globalFraction` of the sphere by area, by
 * bisection, which the mean's fall with b admits; the bracket's end where the fraction lies beyond
 * what any b gives (a marine term alone above it, or none of the field able to reach it).
 *
 * @param globalFraction - Plan 14's cloud fraction, in (0, 1).
 * @throws RangeError for a fraction outside (0, 1), whose clouds are not this plan's (Design note
 *   9), or a grid of no cells.
 */
export function calibrateOffset(input: CoverageInput, globalFraction: number): number {
  if (!(globalFraction > 0 && globalFraction < 1)) {
    throw new RangeError(`a cloud fraction ${globalFraction} must lie in (0, 1)`);
  }
  const climate = gridClimate(input);
  const meanAt = (offset: number): number =>
    areaMeanCoverage({
      widthCells: input.widthCells,
      heightCells: input.heightCells,
      fractions: fractionsOf(climate, offset, input.widthCells, input.heightCells),
      offset,
    });
  let [low, high] = OFFSET_BRACKET;
  for (let step = 0; step < OFFSET_HALVINGS; step += 1) {
    const middle = 0.5 * (low + high);
    if (meanAt(middle) > globalFraction) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return 0.5 * (low + high);
}

/**
 * A body's coverage field (Design note 8): each cell's coverage at the body's offset, which
 * {@link calibrateOffset} solved once.
 *
 * @throws RangeError for a grid of no cells, or an offset that is not finite.
 */
export function coverageField(input: CoverageInput, offset: number): CoverageGrid {
  if (!Number.isFinite(offset)) {
    throw new RangeError(`a coverage offset ${offset} must be finite`);
  }
  const climate = gridClimate(input);
  return {
    widthCells: input.widthCells,
    heightCells: input.heightCells,
    fractions: fractionsOf(climate, offset, input.widthCells, input.heightCells),
    offset,
  };
}

/**
 * The provisional zonal wind at the clouds' height, eastward, m s⁻¹, until the climate field's wind
 * is read for them (R09's `ClimateCell.wind`): easterly trades of 6 m s⁻¹ about the rain band, 20°
 * wide; westerly jets of 15 m s⁻¹ 45° poleward of it, 15° wide; polar easterlies of 4 m s⁻¹ 75°
 * poleward, 10° wide; times cos φ, so that it vanishes at the poles. Earth's lower-tropospheric
 * zonal means in shape (from memory), a labelled heuristic placed on every world (lean, 2026-11-02).
 */
export function provisionalZonalWindMps(latitudeRad: number, season: CloudSeason): number {
  const band = RAIN_BAND_SHIFT * season.subsolarLatitudeRad;
  const zonal =
    -6 * bump(latitudeRad, band, 20 * DEG) +
    15 *
      (bump(latitudeRad, band + 45 * DEG, 15 * DEG) +
        bump(latitudeRad, band - 45 * DEG, 15 * DEG)) -
    4 *
      (bump(latitudeRad, band + 75 * DEG, 10 * DEG) + bump(latitudeRad, band - 75 * DEG, 10 * DEG));
  return cos(latitudeRad) * zonal;
}

/**
 * Where the clouds' structure at `point` came from `advectionS` earlier under an eastward zonal
 * wind of `zonalWindMps` there: the point turned westward about the pole (body-fixed +z) by
 * u t ÷ ρ, ρ its distance from the axis, so that the structure moves eastward at u. Under a wind
 * of uniform angular rate this is a rotation of the sphere; a point on the axis stays.
 *
 * @param point - Body-fixed, m.
 */
export function zonalAdvection(point: Vec3, zonalWindMps: number, advectionS: number): Vec3 {
  const rho = Math.hypot(point.x, point.y);
  if (!(rho > 0)) {
    return point;
  }
  const angle = (-zonalWindMps * advectionS) / rho;
  const c = cos(angle);
  const s = sin(angle);
  return { x: c * point.x - s * point.y, y: s * point.x + c * point.y, z: point.z };
}

/** One of the flow map's two phases at a time (Neyret 2003). */
export interface FlowPhase {
  /**
   * How long the structure it samples has been carried, s: from −T ÷ 2 at its start to T ÷ 2 at its
   * end, 0 at its middle, where its weight is 1.
   */
  readonly advectionS: number;
  /** Its weight, in [0, 1]: 1 − |2p − 1| of its phase p; the two phases' weights sum to 1. */
  readonly weight: number;
  /**
   * Which of its periods it is in, an integer, which keys a fresh structure each time it restarts
   * at weight 0.
   */
  readonly cycle: number;
}

/**
 * The flow map's two phases at the scene's time `tS`, half a period apart (Neyret 2003): phase k's
 * p = frac(t ÷ T + k ÷ 2).
 *
 * @param periodS - T, s, above 0.
 * @throws RangeError for a period that is not finite and above 0, or a time that is not finite.
 */
export function flowMapPhases(tS: number, periodS: number): readonly [FlowPhase, FlowPhase] {
  if (!(periodS > 0 && Number.isFinite(periodS))) {
    throw new RangeError(`a flow map's period ${periodS} s must be finite and above 0`);
  }
  if (!Number.isFinite(tS)) {
    throw new RangeError(`a flow map's time ${tS} s must be finite`);
  }
  const phase = (shift: number): FlowPhase => {
    const turns = tS / periodS + shift;
    const cycle = Math.floor(turns);
    const p = turns - cycle;
    return { advectionS: (p - 0.5) * periodS, weight: 1 - Math.abs(2 * p - 1), cycle };
  };
  return [phase(0), phase(0.5)];
}

/**
 * The flow map's period, s: 6 h, over which the structure of the clouds is carried before each
 * phase restarts, so that a feature lives for hours, as a cumulus field's does (lean, 2026-11-02).
 */
export const FLOW_MAP_PERIOD_S = 21_600;

/**
 * The real time below which a feature crossing its own size flashes, s: a third of a second (Design
 * note 8; guide item 2's flash limit of three alternations a second).
 */
export const FLASH_CROSSING_S = 1 / 3;

/**
 * The share of a feature's structure drawn at a time rate (Design note 8): 1 while it takes at
 * least twice {@link FLASH_CROSSING_S} of real time to cross its own size, falling linearly to 0
 * where it would cross it in a third of a second, the rate 3 L ÷ u, and 0 above; 1 with no wind.
 * The structure fades to the calibrated mean, never to clear sky.
 *
 * @param rate - Simulation seconds per real second; its sign is ignored.
 * @param featureM - The feature's size, m: a coverage cell's for the field's coarsest structure.
 * @param windMps - The wind that carries it, m s⁻¹; its sign is ignored.
 */
export function timeRateFade(rate: number, featureM: number, windMps: number): number {
  const speed = Math.abs(rate * windMps);
  if (!(speed > 0)) {
    return 1;
  }
  const namedRate = featureM / (FLASH_CROSSING_S * Math.abs(windMps));
  return Math.min(Math.max(2 * (1 - Math.abs(rate) / namedRate), 0), 1);
}
