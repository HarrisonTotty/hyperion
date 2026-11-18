/**
 * A body's sea as a photorealistic frame draws it (plan R11, R11.T8.b; Design notes 11 and 12): its
 * ocean map, the liquid and the air over it, its datum and sea level from its field's header, and
 * its bed's albedo, which the terrain session gives every view's frames ({@link OceanSource}), and
 * the far ocean's optics a frame forms from them ({@link oceanFarOpticsOf}).
 *
 * @remarks
 * - **The liquid** is the header's main liquid, its palette entry's record (`oceanLiquidOf`); a body
 *   whose header names none, or whose entry carries no record, has no sea drawn.
 * - **The air** over the sea (a lean, 2026-11-18: the header carries the surface's pressure and
 *   reference temperature, not the air's composition): its density by the ideal gas law at Earth
 *   air's molar mass, ρ_a = p M ÷ (R T), and its kinematic viscosity from Sutherland's law for air,
 *   ν_a = μ(T) ÷ ρ_a, the identity of `seaState.ts`' `EARTH_SEA` air at the ISA's sea level. Both
 *   enter only the equivalent wind and the generation threshold, a heuristic of low confidence
 *   (Design note 12); a CO₂ world's density is then 34% low and its ν about 1.9 times high (a
 *   finding).
 * - **The gravity** is the body's air's reference gravity, √(g_e g_p) (`BodyGravity`), where the
 *   frame draws its air, else its record's bulk surface gravity ({@link OceanSea.surfaceGravityMps2},
 *   which the terrain session keeps from the record it reads the detail seed from), and standard
 *   gravity where neither is held (a lean, 2026-11-18).
 * - **The bed** is the class map's `Seabed` classes' mean albedo (`ClassMapStore.seabedAlbedoOf`);
 *   until a map names one, the bed is taken as deep water, its albedo R∞, so that it adds nothing of
 *   its own (a lean, 2026-11-18), never an invented albedo.
 */
import type { BodyIdHex, SurfaceHeaderDto } from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import type { Rgb } from "../photometry/toneCurve";
import { type OceanLiquid, oceanLiquidOf } from "./liquids";
import type { HeldOceanMap } from "./oceanMap";
import { type OceanFarOptics, oceanLiquidOptics, oceanSeaConstants } from "./oceanFar";
import type { SeaAir } from "./seaState";

/** Earth air's mean molar mass at sea level, kg/mol: 28.9644 g/mol (U.S. Standard Atmosphere, 1976). */
export const AIR_MOLAR_MASS_KG_PER_MOL = 0.0289644;

/** The molar gas constant, J mol⁻¹ K⁻¹: 8.314462618 (CODATA 2018, exact in the 2019 SI). */
export const MOLAR_GAS_CONSTANT = 8.314462618;

/**
 * Sutherland's law for air's dynamic viscosity, μ = β T^(3/2) ÷ (T + S): β = 1.458 × 10⁻⁶ kg m⁻¹
 * s⁻¹ K^(−1/2) and S = 110.4 K (U.S. Standard Atmosphere, 1976, eq. 51), 1.7894 × 10⁻⁵ Pa s at the
 * ISA's 288.15 K; fitted over about 170–1,900 K (White, "Viscous Fluid Flow", Table 1-2) and
 * extrapolated below it, about 5% low at Titan's 94 K.
 */
export const SUTHERLAND_AIR = { betaKgPerMSRootK: 1.458e-6, constantK: 110.4 } as const;

/**
 * Standard gravity, 9.80665 m/s², exact by definition (3rd CGPM, 1901): the sea's where neither its
 * air nor its record gives its body's (the module's remarks).
 */
export const STANDARD_GRAVITY_MPS2 = 9.80665;

/** Air's dynamic viscosity at `temperatureK` by Sutherland's law ({@link SUTHERLAND_AIR}), Pa s. */
export function airViscosityPaS(temperatureK: number): number {
  const { betaKgPerMSRootK: beta, constantK: s } = SUTHERLAND_AIR;
  return (beta * temperatureK ** 1.5) / (temperatureK + s);
}

/**
 * The air over a body's sea from its field's header (the module's remarks): density 0 on an airless
 * body, whose sea never moves, its kinematic viscosity then the air's at the reference temperature
 * over the ISA's density, which nothing reads.
 *
 * @throws RangeError for a reference temperature that is not positive and finite, or a pressure
 *   that is negative or not finite.
 */
export function seaAirOf(
  header: Pick<SurfaceHeaderDto, "surface_pressure_pa" | "reference_temperature_k">,
): SeaAir {
  const pressurePa = header.surface_pressure_pa;
  const temperatureK = header.reference_temperature_k;
  if (!(temperatureK > 0 && Number.isFinite(temperatureK))) {
    throw new RangeError(`a reference temperature of ${String(temperatureK)} K`);
  }
  if (!(pressurePa >= 0 && Number.isFinite(pressurePa))) {
    throw new RangeError(`a surface pressure of ${String(pressurePa)} Pa`);
  }
  const densityKgM3 =
    (pressurePa * AIR_MOLAR_MASS_KG_PER_MOL) / (MOLAR_GAS_CONSTANT * temperatureK);
  const viscosity = airViscosityPaS(temperatureK);
  return {
    densityKgM3,
    kinematicViscosityM2s: viscosity / (densityKgM3 > 0 ? densityKgM3 : 1.225),
  };
}

/** A body's sea, everything a frame draws it from but the frame's own light and camera. */
export interface OceanSea {
  readonly body: BodyIdHex;
  /** Its ocean map as the store uploaded it. */
  readonly map: HeldOceanMap;
  /** The palette's key of its liquid (`PaletteEntryDto.substance`). */
  readonly substance: string;
  readonly liquid: OceanLiquid;
  readonly air: SeaAir;
  /** The datum's equatorial and polar radii, m. */
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
  /** The sea surface above the datum, m, of either sign (`SurfaceHeaderDto.sea_level_m`). */
  readonly seaLevelM: number;
  /** The record's bulk surface gravity, m/s², or `null` where no record of it is held. */
  readonly surfaceGravityMps2: number | null;
  /**
   * The bed's albedo per channel, the class map's `Seabed` classes', or `null` until a map names
   * one, the bed then deep water (the module's remarks).
   */
  readonly bedAlbedo: Rgb | null;
}

/** Where a frame finds its bodies' seas: the terrain session's (`TerrainSession.oceans`). */
export interface OceanSource {
  /** `body`'s sea, or `null` where it has none drawn: no main liquid, no map, or no open water. */
  oceanOf(body: BodyIdHex): OceanSea | null;
}

/**
 * A body's sea from its field's header, its ocean map, its bed's albedo and its record's surface
 * gravity, or `null` where the header names no main liquid or its entry carries no liquid record.
 *
 * @throws RangeError as `oceanLiquidOf` and {@link seaAirOf}.
 */
export function oceanSeaOf(
  header: SurfaceHeaderDto,
  map: HeldOceanMap,
  bedAlbedo: Rgb | null,
  surfaceGravityMps2: number | null = null,
): OceanSea | null {
  const index = header.main_liquid;
  const entry = index === null ? undefined : header.palette[index];
  const record = entry?.liquid ?? null;
  if (entry === undefined || record === null) {
    return null;
  }
  return {
    body: header.body,
    map,
    substance: entry.substance,
    liquid: oceanLiquidOf(record),
    air: seaAirOf(header),
    equatorialRadiusM: header.equatorial_radius_m,
    polarRadiusM: header.polar_radius_m,
    seaLevelM: header.sea_level_m,
    surfaceGravityMps2,
    bedAlbedo,
  };
}

/** What a frame gives the far ocean's optics of its own. */
export interface OceanFrameOptics {
  /** The body's gravity, m/s² (the module's remarks). */
  readonly gravityMps2: number;
  /** A pixel's angle at the view's centre, rad. */
  readonly pixelAngleRad: number;
  /** The setting's `whitecaps`. */
  readonly whitecaps: boolean;
  /** The unit direction to the brightest source, body-fixed. */
  readonly sunDirection: Vec3;
  /** The glint's cap, cd/m²: the bloom threshold over the pre-exposure. */
  readonly glintCapCdM2: number;
  /** The brightest source's illuminance face-on, lx, and the sky's on the horizontal. */
  readonly sunLux: Rgb;
  readonly skyLux: Rgb;
}

/**
 * The far ocean's optics of `sea` in a frame: its sea-level spheroid, the datum's semi-axes raised
 * by the sea level, its liquid's optics, its sea state's factors at the frame's gravity, and the
 * frame's own members.
 */
export function oceanFarOpticsOf(sea: OceanSea, frame: OceanFrameOptics): OceanFarOptics {
  const liquid = oceanLiquidOptics(sea.liquid);
  return {
    equatorialRadiusM: sea.equatorialRadiusM + sea.seaLevelM,
    polarRadiusM: sea.polarRadiusM + sea.seaLevelM,
    pixelAngleRad: frame.pixelAngleRad,
    whitecaps: frame.whitecaps,
    sunDirection: frame.sunDirection,
    glintCapCdM2: frame.glintCapCdM2,
    sunLux: frame.sunLux,
    skyLux: frame.skyLux,
    sea: oceanSeaConstants(frame.gravityMps2, sea.liquid, sea.air),
    ...liquid,
    bedAlbedo: sea.bedAlbedo ?? liquid.deepReflectance,
  };
}
