/**
 * Surface lighting through the air (plan R08, R08.T9.b; Design notes 7 and 17): what a lit pass
 * binds to read `shaders/surfaceLighting.wgsl`, and the reader its CPU twins share.
 *
 * @remarks
 * `surfaceLighting.wgsl` exports `atmosphere_sun_transmittance`, `atmosphere_sky_irradiance` and
 * `atmosphere_source_transmittance` to R07's disc shading (`view/shaders/bodyDisc.wgsl`), R10's
 * `terrainLit.wgsl` and R11's lit passes. A lit pass composes {@link SURFACE_LIGHTING_WGSL} into
 * its module and declares the seven tables at bindings of its own ({@link surfaceLightingTextures}
 * for a material's layout), binding a body's {@link SurfaceLightingTables}, or, for a body without
 * an atmosphere, {@link createAirlessTables}' one-texel tables of zeros, through which the lookups
 * read 1 and 0, as R07's stubs did. A compute kernel binds them by their WGSL names
 * ({@link surfaceLightingKernelTextures}), the arrays as one-layer arrays where they have one layer
 * (R08.T0's seam).
 */
import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { RenderEngine, TextureBindingSpec, TextureHandle } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import commonWgsl from "./shaders/common.wgsl?raw";
import oblateWgsl from "./shaders/oblate.wgsl?raw";
import surfaceLightingWgsl from "./shaders/surfaceLighting.wgsl?raw";
import type { AtmosphereTables } from "./tables";

/**
 * The WGSL a lit pass composes before its own to read the surface's lighting:
 * `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `oblate.wgsl` and `surfaceLighting.wgsl`. It
 * declares no binding; the pass declares the tables (`surfaceLighting.wgsl`'s head).
 */
export const SURFACE_LIGHTING_WGSL = `${preciseTrigWgsl}\n${commonWgsl}\n${oblateWgsl}\n${surfaceLightingWgsl}\n`;

/** A body's atmosphere tables as a lit pass binds them (`surfaceLighting.wgsl`'s head). */
export interface SurfaceLightingTables {
  /** The body's table (`tables.ts`'s `packOblateBody`), `rgba32float`. */
  readonly body: TextureHandle;
  /** The transmittance tables, one layer a κ slice. */
  readonly transmittance: TextureHandle;
  readonly absorberColumns: TextureHandle;
  readonly absorberPressureColumns: TextureHandle;
  readonly lastAbsorberPressureColumn: TextureHandle;
  /** The lights' curves of growth, `rgba32float`. */
  readonly absorberCurves: TextureHandle;
  /** The sky's irradiance, one layer a latitude band and light slot. */
  readonly irradiance: TextureHandle;
}

/**
 * A planet's tables as a lit pass binds them, the sky's irradiance built if the tables' last build
 * has not built it (`AtmosphereTables.irradiance`). A device restore makes them anew, so a caller
 * takes them each frame rather than keeping them (`AtmosphereTables`' remarks).
 *
 * @throws `EngineUnavailable` when the irradiance has to be built while the engine has no device.
 */
export function surfaceLightingTablesOf(tables: AtmosphereTables): SurfaceLightingTables {
  return {
    body: tables.oblateBody,
    transmittance: tables.transmittance,
    absorberColumns: tables.absorberColumns,
    absorberPressureColumns: tables.absorberPressureColumns,
    lastAbsorberPressureColumn: tables.lastAbsorberPressureColumn,
    absorberCurves: tables.absorberCurves,
    irradiance: tables.irradiance(),
  };
}

/** Each table's name in a material's `DrawItem.textures` and in `surfaceLighting.wgsl`. */
const NAMES: Readonly<
  Record<
    keyof SurfaceLightingTables,
    { readonly draw: string; readonly wgsl: string; readonly array: boolean }
  >
> = {
  body: { draw: "atmosphereBody", wgsl: "atmosphere_body", array: false },
  transmittance: { draw: "atmosphereTransmittance", wgsl: "atmosphere_transmittance", array: true },
  absorberColumns: {
    draw: "atmosphereAbsorberColumns",
    wgsl: "atmosphere_absorber_columns",
    array: true,
  },
  absorberPressureColumns: {
    draw: "atmosphereAbsorberPressureColumns",
    wgsl: "atmosphere_absorber_pressure_columns",
    array: true,
  },
  lastAbsorberPressureColumn: {
    draw: "atmosphereLastAbsorberPressureColumn",
    wgsl: "atmosphere_last_absorber_pressure_column",
    array: true,
  },
  absorberCurves: {
    draw: "atmosphereAbsorberCurves",
    wgsl: "atmosphere_absorber_curves",
    array: false,
  },
  irradiance: { draw: "atmosphereIrradiance", wgsl: "atmosphere_irradiance", array: true },
};

/** The tables in the order their bindings take. */
const ORDER: ReadonlyArray<keyof SurfaceLightingTables> = [
  "body",
  "transmittance",
  "absorberColumns",
  "absorberPressureColumns",
  "lastAbsorberPressureColumn",
  "absorberCurves",
  "irradiance",
];

/** How many bindings the tables take: seven. */
export const SURFACE_LIGHTING_BINDINGS = ORDER.length;

/**
 * The tables' entries in a material's layout from binding `first` on, in
 * `surfaceLighting.wgsl`'s order (the body's table, the four transmittance tables, the curves, the
 * irradiance), each read by `textureLoad`, so `unfilterable-float`, the arrays `2d-array`.
 */
export function surfaceLightingTextures(first: number): ReadonlyArray<TextureBindingSpec> {
  return ORDER.map((key, i) => {
    const { draw, array } = NAMES[key];
    return array
      ? {
          name: draw,
          binding: first + i,
          sampleType: "unfilterable-float",
          viewDimension: "2d-array",
        }
      : { name: draw, binding: first + i, sampleType: "unfilterable-float" };
  });
}

/** The tables as a material's `DrawItem.textures` names them ({@link surfaceLightingTextures}). */
export function surfaceLightingDrawTextures(
  tables: SurfaceLightingTables,
): Readonly<Record<string, TextureHandle>> {
  return Object.fromEntries(ORDER.map((key) => [NAMES[key].draw, tables[key]]));
}

/** The tables as a compute kernel's `sampled` names them: `surfaceLighting.wgsl`'s own names. */
export function surfaceLightingKernelTextures(
  tables: SurfaceLightingTables,
): Readonly<Record<string, TextureHandle>> {
  return Object.fromEntries(ORDER.map((key) => [NAMES[key].wgsl, tables[key]]));
}

/**
 * The tables of a body without an atmosphere: one one-texel `rgba32float` texture of zeros in every
 * binding, whose body table's equatorial radius of 0 makes the lookups read 1 and 0. The caller
 * releases {@link SurfaceLightingTables.body}, the one texture.
 *
 * @throws `EngineUnavailable` while the engine has no device (a loss).
 */
export function createAirlessTables(
  engine: Pick<RenderEngine, "createTexture">,
  name = "atmosphere: no air",
): SurfaceLightingTables {
  const texture = engine.createTexture({
    name,
    size: { width: 1, height: 1 },
    dimension: "2d",
    format: "rgba32float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING,
    category: "atmosphere-tables",
  });
  return {
    body: texture,
    transmittance: texture,
    absorberColumns: texture,
    absorberPressureColumns: texture,
    lastAbsorberPressureColumn: texture,
    absorberCurves: texture,
    irradiance: texture,
  };
}

/**
 * The CPU's reading of `surfaceLighting.wgsl`'s three lookups for one body, which the lit passes'
 * twins take (R07's disc twin, `view/bodies/discShading.ts`): the twin of a body's tables
 * (`tablesCpu.ts`'s `surfaceLightingTwin`), or {@link AIRLESS_LIGHTING}.
 */
export interface SurfaceLightingReader {
  /** `atmosphere_sun_transmittance`. */
  readonly sunTransmittance: (
    altitudeM: number,
    muSun: number,
    latitudeRad: number,
    sunAzimuthRad: number,
    curve: number,
  ) => Rgb;
  /** `atmosphere_sky_irradiance`. */
  readonly skyIrradiance: (
    altitudeM: number,
    muSun: number,
    latitudeRad: number,
    curve: number,
  ) => Rgb;
  /** `atmosphere_source_transmittance`. */
  readonly sourceTransmittance: (
    altitudeM: number,
    muCentre: number,
    latitudeRad: number,
    azimuthRad: number,
    angularRadiusRad: number,
    curve: number,
  ) => Rgb;
}

/** A body without an atmosphere: transmittance 1 and the sky's irradiance 0, R07's stubs. */
export const AIRLESS_LIGHTING: SurfaceLightingReader = {
  sunTransmittance: () => [1, 1, 1],
  skyIrradiance: () => [0, 0, 0],
  sourceTransmittance: () => [1, 1, 1],
};
