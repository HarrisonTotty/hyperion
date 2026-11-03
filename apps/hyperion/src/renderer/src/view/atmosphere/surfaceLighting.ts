/**
 * Surface lighting through the air (plan R08, R08.T9.b; Design notes 7 and 17): what a lit pass
 * binds to read `shaders/surfaceLighting.wgsl`, and the reader its CPU twins share.
 *
 * @remarks
 * `surfaceLighting.wgsl` exports `atmosphere_sun_transmittance`, `atmosphere_sky_irradiance` and
 * `atmosphere_source_transmittance` to R07's disc shading (`view/shaders/bodyDisc.wgsl`), R10's
 * `terrainLit.wgsl` and R11's lit passes. A lit pass composes {@link SURFACE_LIGHTING_WGSL} into
 * its module and declares the eight tables at bindings of its own ({@link surfaceLightingTextures}
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
import belowDeckWgsl from "./shaders/belowDeck.wgsl?raw";
import oblateWgsl from "./shaders/oblate.wgsl?raw";
import surfaceLightingWgsl from "./shaders/surfaceLighting.wgsl?raw";
import type { SourceTransmittance } from "./hillaire";
import type { AtmosphereTables } from "./tables";

/**
 * The WGSL a lit pass composes before its own to read the surface's lighting:
 * `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `oblate.wgsl`, `belowDeck.wgsl` (R08.T15.b.2) and
 * `surfaceLighting.wgsl`. It declares no binding; the pass declares the tables
 * (`surfaceLighting.wgsl`'s head).
 */
export const SURFACE_LIGHTING_WGSL = `${preciseTrigWgsl}\n${commonWgsl}\n${oblateWgsl}\n${belowDeckWgsl}\n${surfaceLightingWgsl}\n`;

/**
 * A views check's probe of the lit passes' cost (R08.T9.b's measurement): `transmittance-one`,
 * every source's transmittance through the air 1, with no lookup; `planetshine-centre`, a
 * planetshine source's at its disc's centre node alone, one lookup in place of three;
 * `no-annuli`, the eclipse term with no annuli, every star unhidden; or `per-fragment`, each
 * source's transmittance per fragment whatever the setting's `AtmosphereSettings.sourceTransmittance`
 * (the sources' table built and bound all the same), the lever's way back, so that one build's
 * profiles tell the table's saving apart. It is check-only: an inspection profile's `--lit-probe`,
 * never drawn on a player's launch.
 */
export type LitProbe = "transmittance-one" | "planetshine-centre" | "no-annuli" | "per-fragment";

/** The line of `surfaceLighting.wgsl` a probe's module replaces. */
const LIT_PROBE_LINE = "const LIT_PROBE : u32 = 0u;";

/**
 * `wgsl` under `probe`: `surfaceLighting.wgsl`'s `LIT_PROBE` set, 1 for `transmittance-one`, 2 for
 * `planetshine-centre`, 3 for `no-annuli` and 4 for `per-fragment`, where `wgsl` composes
 * {@link SURFACE_LIGHTING_WGSL}; any other module as it is.
 */
export function litProbedWgsl(wgsl: string, probe: LitProbe): string {
  let value: number;
  switch (probe) {
    case "transmittance-one":
      value = 1;
      break;
    case "planetshine-centre":
      value = 2;
      break;
    case "no-annuli":
      value = 3;
      break;
    case "per-fragment":
      value = 4;
      break;
  }
  return wgsl.replace(LIT_PROBE_LINE, `const LIT_PROBE : u32 = ${String(value)}u;`);
}

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
  /**
   * The table below a cloud deck over a surface (plan R08, R08.T15.b.2; `AtmosphereTables`'
   * `belowDeckLighting`), or a one-texel stub, which the lookups read as no deck.
   */
  readonly belowDeck: TextureHandle;
  /**
   * The sources' transmittance by height and cosine, one layer a band and light slot
   * (`AtmosphereTables.sourceTransmittance`; R08.T9.b's lever, {@link SourceTransmittance}'s
   * `table`), or a one-texel stub, through which each source's lookup is the sliced lookup per
   * fragment.
   */
  readonly sourceTransmittance: TextureHandle;
}

/**
 * A planet's tables as a lit pass binds them, the sky's irradiance built if the tables' last build
 * has not built it (`AtmosphereTables.irradiance`), and the sources' transmittance likewise where
 * `sourceTransmittance` is `table` (R08.T9.b's lever), else its one-texel stub. A device restore
 * makes them anew, so a caller takes them each frame rather than keeping them (`AtmosphereTables`'
 * remarks).
 *
 * @param sourceTransmittance - The lever's way: `per-fragment` by default, as every lit pass took
 *   it before the lever, so that a reader outside a view (the smoke's) draws as before.
 * @throws `EngineUnavailable` when a table has to be built while the engine has no device.
 */
export function surfaceLightingTablesOf(
  tables: AtmosphereTables,
  sourceTransmittance: SourceTransmittance = "per-fragment",
): SurfaceLightingTables {
  return {
    body: tables.oblateBody,
    transmittance: tables.transmittance,
    absorberColumns: tables.absorberColumns,
    absorberPressureColumns: tables.absorberPressureColumns,
    lastAbsorberPressureColumn: tables.lastAbsorberPressureColumn,
    absorberCurves: tables.absorberCurves,
    irradiance: tables.irradiance(),
    belowDeck: tables.belowDeckLighting,
    sourceTransmittance:
      sourceTransmittance === "table"
        ? tables.sourceTransmittance()
        : tables.perFragmentSourceTransmittance,
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
  belowDeck: { draw: "atmosphereBelowDeck", wgsl: "atmosphere_below_deck", array: true },
  sourceTransmittance: {
    draw: "atmosphereSourceTable",
    wgsl: "atmosphere_source_table",
    array: true,
  },
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
  "belowDeck",
  "sourceTransmittance",
];

/**
 * How many bindings the tables take: nine, the table below a deck's since R08.T15.b.2 and the
 * sources' table's since R08.T9.b's lever.
 */
export const SURFACE_LIGHTING_BINDINGS = ORDER.length;

/**
 * The tables' entries in a material's layout from binding `first` on, in
 * `surfaceLighting.wgsl`'s order (the body's table, the four transmittance tables, the curves, the
 * irradiance), the table below a deck (R08.T15.b.2) at `belowDeckBinding` and the sources' table
 * (R08.T9.b's lever) at `sourceTableBinding`, each read by `textureLoad`, so `unfilterable-float`,
 * the arrays `2d-array`.
 *
 * @param belowDeckBinding - The table below a deck's binding: the next after the seven by default,
 *   or a pass's own where that one is taken.
 * @param sourceTableBinding - The sources' table's binding: the next after the table below a deck's
 *   by default, or a pass's own where that one is taken.
 */
export function surfaceLightingTextures(
  first: number,
  belowDeckBinding: number = first + SURFACE_LIGHTING_BINDINGS - 2,
  sourceTableBinding: number = belowDeckBinding + 1,
): ReadonlyArray<TextureBindingSpec> {
  return ORDER.map((key, i) => {
    const { draw, array } = NAMES[key];
    const binding =
      key === "belowDeck"
        ? belowDeckBinding
        : key === "sourceTransmittance"
          ? sourceTableBinding
          : first + i;
    return array
      ? {
          name: draw,
          binding,
          sampleType: "unfilterable-float",
          viewDimension: "2d-array",
        }
      : { name: draw, binding, sampleType: "unfilterable-float" };
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
    belowDeck: texture,
    sourceTransmittance: texture,
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
