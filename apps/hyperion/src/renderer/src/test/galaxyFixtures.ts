/**
 * Builders for the galaxy messages the server sends, typed against the generated bindings so that
 * a protocol change breaks them at compile time.
 *
 * @remarks
 * Every builder returns a complete, consistent message from a few overrides. The response
 * builders include the `kind`, so that they can be passed straight to
 * `FakeWebSocket.serverAnswers` or `serverResponds`.
 */
import {
  type Census,
  galacticPositionFromLy,
  type LayerCensus,
  type LayerStatus,
  type MapPopulation,
  type MapView,
  type MassLayer,
  type Parameter,
  type ParameterGroup,
  type Population,
  type RequestOf,
  type ResponseFor,
  type StellarBriefDto,
  type SystemRecord,
  u64ToHex,
  type UniverseIdHex,
  type UniverseInfo,
  universeTimeFromYears,
} from "@hyperion/protocol";

/** The layers from the lightest, as the census lists them. */
export const LAYERS: ReadonlyArray<MassLayer> = ["a", "b", "c", "d", "e"];

/**
 * Each layer's band of primary initial mass in M☉, from the brainstorm's "Sizing the layers".
 *
 * @remarks
 * The client holds no copy of this table; the server sends the bands with every census. It is
 * here only so that fixtures are realistic.
 */
const BANDS_MSUN: Readonly<Record<MassLayer, readonly [number, number]>> = {
  a: [0.08, 0.5],
  b: [0.5, 0.75],
  c: [0.75, 2.5],
  d: [2.5, 8],
  e: [8, 150],
};

/** The universe the fixtures describe unless told otherwise. */
export const UNIVERSE_ID: UniverseIdHex = "00000000000000a1";

/** A universe as the server lists it: `SURVEY 1`, seed `4d2`, compatible, generator version 2. */
export function aUniverse(overrides: Partial<UniverseInfo> = {}): UniverseInfo {
  return {
    id: UNIVERSE_ID,
    name: "SURVEY 1",
    seed: "00000000000004d2",
    generator_version: 2,
    status: "compatible",
    ...overrides,
  };
}

/** The answer to `list_universes`. */
export function aUniverseList(
  universes: ReadonlyArray<UniverseInfo> = [aUniverse()],
  serverGeneratorVersion = 2,
): ResponseFor<"list_universes"> {
  return {
    kind: "list_universes",
    universes: [...universes],
    server_generator_version: serverGeneratorVersion,
  };
}

/** The answer to `create_universe`: the new universe. */
export function aCreatedUniverse(info: UniverseInfo = aUniverse()): ResponseFor<"create_universe"> {
  return { kind: "create_universe", ...info };
}

/** The answer to `open_universe`: the universe opened. */
export function anOpenedUniverse(info: UniverseInfo = aUniverse()): ResponseFor<"open_universe"> {
  return { kind: "open_universe", ...info };
}

function aNumber(
  key: string,
  value: number,
  unit: Extract<Parameter["value"], { type: "number" }>["unit"],
  origin: Parameter["origin"],
): Parameter {
  return { key, origin, value: { type: "number", value, unit } };
}

/**
 * The answer to `galaxy_parameters`: a Milky Way-like galaxy with a stellar mass of 5.2 × 10¹⁰ M☉
 * (drawn), a derived system count and pattern speed, and one parameter of every unit, drawn from
 * {@link everyGalaxyParameter}'s list so that a test reads a short list.
 */
export function someGalaxyParameters(
  universe: UniverseIdHex = UNIVERSE_ID,
  seed = "00000000000004d2",
): ResponseFor<"galaxy_parameters"> {
  const groups: ParameterGroup[] = [
    {
      key: "identity",
      parameters: [
        aText("seed", seed, "fixed"),
        aNumber("generator_version", 2, "count", "fixed"),
        aText("mass_function", "kroupa", "fixed"),
      ],
    },
    {
      key: "mass",
      parameters: [
        aNumber("stellar_mass", 5.2e10, "msun", "drawn"),
        aNumber("system_count", 1.21e11, "count", "derived"),
        aNumber("mean_system_mass", 0.43, "msun", "derived"),
        aNumber("black_hole.mass", 4.3e6, "msun", "derived"),
      ],
    },
    {
      key: "populations",
      parameters: [aNumber("population.thick_disc.share", 0.112, "none", "drawn")],
    },
    {
      key: "discs",
      parameters: [
        aNumber("disc.thin.scale_length", 8_480.4, "ly", "derived"),
        aNumber("disc.thin.scale_height", 1_004.2, "ly", "drawn"),
      ],
    },
    {
      key: "bulge_and_bar",
      parameters: [
        aNumber("bar.half_length", 16_300, "ly", "derived"),
        aNumber("bar.pattern_speed", 2.227, "deg_per_myr", "derived"),
      ],
    },
    {
      key: "arms",
      parameters: [
        aNumber("arms.count", 2, "count", "drawn"),
        aNumber("arms.pitch", 12.5, "deg", "drawn"),
      ],
    },
    {
      key: "history",
      parameters: [
        aNumber("history.formation_timescale", 6.8, "gyr", "drawn"),
        aNumber("history.last_major_merger", 10.2, "gyr", "drawn"),
      ],
    },
    {
      key: "rotation",
      parameters: [
        aNumber("rotation.radius", 26_000, "ly", "fixed"),
        aNumber("rotation.circular_speed", 229.6, "km_per_s", "derived"),
      ],
    },
  ];
  return { kind: "galaxy_parameters", universe, seed, generator_version: 2, groups };
}

/** A text parameter. */
function aText(key: string, value: string, origin: Parameter["origin"]): Parameter {
  return { key, origin, value: { type: "text", value } };
}

/** One parameter for each population, in the wire's order, from a value per population. */
function perPopulation(
  quantity: "share" | "mass" | "mean_system_mass",
  values: Readonly<Record<Population, number>>,
  origins: Partial<Readonly<Record<Population, Parameter["origin"]>>> = {},
): Parameter[] {
  const populations: ReadonlyArray<Population> = [
    "young_thin_disc",
    "old_thin_disc",
    "thick_disc",
    "bulge",
    "long_bar",
    "nuclear_disc",
    "halo",
  ];
  return populations.map((population) =>
    aNumber(
      `population.${population}.${quantity}`,
      values[population],
      quantity === "share" ? "none" : "msun",
      origins[population] ?? "derived",
    ),
  );
}

/**
 * The answer to `galaxy_parameters` with every parameter P04.T14.b sends, group by group, in
 * its order, with Milky Way-like values.
 *
 * @remarks
 * The list the client's glossary is held to (`parameterLabels.test.ts`), and so the client's copy
 * of the table in plan 04's P04.T14.b: a key added there is added here.
 */
export function everyGalaxyParameter(
  universe: UniverseIdHex = UNIVERSE_ID,
  seed = "00000000000004d2",
): ResponseFor<"galaxy_parameters"> {
  const groups: ParameterGroup[] = [
    {
      key: "identity",
      parameters: [
        aText("seed", seed, "fixed"),
        aNumber("generator_version", 2, "count", "fixed"),
        aText("mass_function", "kroupa", "fixed"),
      ],
    },
    {
      key: "mass",
      parameters: [
        aNumber("stellar_mass", 5.2e10, "msun", "drawn"),
        aNumber("system_count", 1.21e11, "count", "derived"),
        aNumber("mean_system_mass", 0.43, "msun", "derived"),
        aNumber("mean_formed_mass", 0.62, "msun", "derived"),
        aNumber("gas.mass", 6.1e9, "msun", "derived"),
        aNumber("black_hole.mass", 4.3e6, "msun", "derived"),
        aNumber("nuclear_cluster.mass", 2.5e7, "msun", "derived"),
        aNumber("dark_halo.mass", 1.3e12, "msun", "derived"),
        aNumber("dark_halo.concentration", 9.4, "none", "derived"),
        aNumber("dark_halo.virial_radius", 652_000, "ly", "derived"),
        aNumber("dark_halo.f_star", 0.32, "none", "drawn"),
      ],
    },
    {
      key: "populations",
      parameters: perPopulation(
        "share",
        {
          young_thin_disc: 0.0021,
          old_thin_disc: 0.5474,
          thick_disc: 0.112,
          bulge: 0.217,
          long_bar: 0.093,
          nuclear_disc: 0.0175,
          halo: 0.011,
        },
        { thick_disc: "drawn", nuclear_disc: "drawn", halo: "drawn" },
      ),
    },
    {
      key: "population_masses",
      parameters: perPopulation("mass", {
        young_thin_disc: 1.1e8,
        old_thin_disc: 2.8e10,
        thick_disc: 6.5e9,
        bulge: 1.1e10,
        long_bar: 4.8e9,
        nuclear_disc: 1.05e9,
        halo: 5.2e8,
      }),
    },
    {
      key: "population_mean_masses",
      parameters: perPopulation("mean_system_mass", {
        young_thin_disc: 0.52,
        old_thin_disc: 0.45,
        thick_disc: 0.41,
        bulge: 0.4,
        long_bar: 0.4,
        nuclear_disc: 0.44,
        halo: 0.38,
      }),
    },
    {
      key: "discs",
      parameters: [
        aNumber("disc.thin.scale_length", 8_480.4, "ly", "derived"),
        aNumber("disc.thin.scale_height", 1_004.2, "ly", "drawn"),
        aNumber("disc.young.scale_length", 8_480.4, "ly", "derived"),
        aNumber("disc.young.scale_height", 160, "ly", "drawn"),
        aNumber("disc.thick.scale_length", 6_784.3, "ly", "derived"),
        aNumber("disc.thick.scale_height", 3_012.6, "ly", "derived"),
        aNumber("disc.gas.scale_length", 14_416.7, "ly", "derived"),
      ],
    },
    {
      key: "bulge_and_bar",
      parameters: [
        aNumber("bulge.scale_x", 2_280, "ly", "derived"),
        aNumber("bulge.scale_y", 1_440, "ly", "derived"),
        aNumber("bulge.scale_z", 820, "ly", "derived"),
        aNumber("bulge.boxiness", 3.5, "none", "drawn"),
        aNumber("bar.share_of_bulge", 0.3, "none", "drawn"),
        aNumber("bar.half_length", 16_300, "ly", "derived"),
        aNumber("bar.width", 1_630, "ly", "derived"),
        aNumber("bar.height", 590, "ly", "drawn"),
        aNumber("bar.corotation_ratio", 1.2, "none", "drawn"),
        aNumber("bar.corotation_radius", 19_560, "ly", "derived"),
        aNumber("bar.pattern_speed", 2.227, "deg_per_myr", "derived"),
      ],
    },
    {
      key: "nuclear_disc",
      parameters: [
        aNumber("nuclear_disc.scale_length", 290, "ly", "derived"),
        aNumber("nuclear_disc.scale_height", 93, "ly", "derived"),
      ],
    },
    {
      key: "halo",
      parameters: [
        aNumber("halo.in_situ.share", 0.22, "none", "derived"),
        aNumber("halo.dominant_merger.share", 0.47, "none", "derived"),
        aNumber("halo.lesser.share", 0.19, "none", "derived"),
        aNumber("halo.globular_debris.share", 0.12, "none", "derived"),
        aNumber("halo.in_situ.slope", 3.5, "none", "drawn"),
        aNumber("halo.dominant_merger.slope", 3.4, "none", "drawn"),
        aNumber("halo.globular_debris.slope", 4.2, "none", "drawn"),
        aNumber("halo.in_situ.core", 2_200, "ly", "drawn"),
        aNumber("halo.dominant_merger.core", 3_400, "ly", "drawn"),
        aNumber("halo.globular_debris.core", 4_100, "ly", "drawn"),
        aNumber("halo.in_situ.flattening", 0.5, "none", "drawn"),
        aNumber("halo.dominant_merger.flattening", 0.7, "none", "drawn"),
        aNumber("halo.dominant_merger.break_radius", 62_000, "ly", "drawn"),
        aNumber("halo.dominant_merger.break_steepening", 1.5, "none", "drawn"),
      ],
    },
    {
      key: "arms",
      parameters: [
        aNumber("arms.count", 2, "count", "drawn"),
        aNumber("arms.pitch", 12.5, "deg", "drawn"),
        aNumber("arms.young_width", 380, "ly", "drawn"),
        aNumber("arms.young_fraction", 0.8, "none", "drawn"),
        aNumber("arms.old_amplitude", 0.2, "none", "drawn"),
      ],
    },
    {
      key: "history",
      parameters: [
        aNumber("history.formation_timescale", 6.8, "gyr", "drawn"),
        aNumber("history.last_major_merger", 10.2, "gyr", "drawn"),
        aNumber("history.globular_clusters", 160, "count", "derived"),
      ],
    },
    {
      key: "rotation",
      parameters: [
        aNumber("rotation.radius", 26_000, "ly", "fixed"),
        aNumber("rotation.circular_speed", 229.6, "km_per_s", "derived"),
        aNumber("rotation.escape_speed", 550, "km_per_s", "derived"),
      ],
    },
  ];
  return { kind: "galaxy_parameters", universe, seed, generator_version: 2, groups };
}

/** What {@link aDensityMap} builds. */
export interface DensityMapSpec {
  /** One 8-bit code per pixel, row by row from the top. */
  readonly codes: ReadonlyArray<number>;
  readonly widthPx?: number;
  readonly heightPx?: number;
  readonly view?: MapView;
  readonly population?: MapPopulation;
  readonly floorLog10PerLy2?: number;
  readonly ceilingLog10PerLy2?: number;
  readonly universe?: UniverseIdHex;
}

/**
 * The answer to `density_map`: a small 8-bit map, 8 × 4 pixels unless told otherwise, spanning
 * the M1 extent of 131,072 ly across.
 *
 * @throws Error when the codes do not fill the map or one is not a byte.
 */
export function aDensityMap({
  codes,
  widthPx = 8,
  heightPx = 4,
  view = "face_on",
  population = "all",
  floorLog10PerLy2 = -4,
  ceilingLog10PerLy2 = 1,
  universe = UNIVERSE_ID,
}: DensityMapSpec): ResponseFor<"density_map"> {
  if (codes.length !== widthPx * heightPx) {
    throw new Error(`${codes.length} codes do not fill a ${widthPx} × ${heightPx} map`);
  }
  let binary = "";
  for (const code of codes) {
    if (!Number.isInteger(code) || code < 0 || code > 255) {
      throw new Error(`${code} is not an 8-bit code`);
    }
    binary += String.fromCharCode(code);
  }
  return {
    kind: "density_map",
    universe,
    view,
    population,
    width_px: widthPx,
    height_px: heightPx,
    centre_ly: [0, 0],
    ly_per_px: 131_072 / widthPx,
    bits: 8,
    floor_log10_per_ly2: floorLog10PerLy2,
    ceiling_log10_per_ly2: ceilingLog10PerLy2,
    data_base64: btoa(binary),
  };
}

/**
 * A main-sequence primary typical of each layer's band, so that a fixture's symbol and place on
 * the HR diagram are realistic: an M3 dwarf, a K5, the Sun, a B8 and a B0 (the mean dwarf sequence
 * of Pecaut and Mamajek, as the server classifies).
 */
const DWARF_BRIEFS: Readonly<Record<MassLayer, StellarBriefDto>> = {
  a: { kind: "dwarf", class: "M3V", log_luminosity_lsun: -1.84, teff_k: 3_410, star_count: 1 },
  b: { kind: "dwarf", class: "K5V", log_luminosity_lsun: -0.84, teff_k: 4_440, star_count: 1 },
  c: { kind: "dwarf", class: "G2V", log_luminosity_lsun: 0, teff_k: 5_772, star_count: 1 },
  d: { kind: "dwarf", class: "B8V", log_luminosity_lsun: 2.2, teff_k: 12_300, star_count: 2 },
  e: { kind: "dwarf", class: "B0V", log_luminosity_lsun: 4.4, teff_k: 31_400, star_count: 2 },
};

/**
 * The stellar brief of a primary of `kind`: a dwarf typical of the layer, or a representative
 * star of the kind asked for, with no luminosity or temperature for a black hole or no remnant.
 */
export function aStellarBrief(
  layer: MassLayer,
  kind: StellarBriefDto["kind"] = "dwarf",
): StellarBriefDto {
  const dwarf = DWARF_BRIEFS[layer];
  let brief: StellarBriefDto;
  switch (kind) {
    case "dwarf":
      brief = dwarf;
      break;
    case "giant":
      brief = { ...dwarf, kind, class: "K0III", log_luminosity_lsun: 1.8, teff_k: 4_800 };
      break;
    case "supergiant":
      brief = { ...dwarf, kind, class: "M2Iab", log_luminosity_lsun: 5.1, teff_k: 3_600 };
      break;
    case "white_dwarf":
      brief = { ...dwarf, kind, class: "DA4.2", log_luminosity_lsun: -2.5, teff_k: 12_000 };
      break;
    case "neutron_star":
      brief = { ...dwarf, kind, class: "NS", log_luminosity_lsun: -4.5, teff_k: 600_000 };
      break;
    case "black_hole":
      brief = { ...dwarf, kind, class: "BH", log_luminosity_lsun: null, teff_k: null };
      break;
    case "no_remnant":
      brief = { ...dwarf, kind, class: "NONE", log_luminosity_lsun: null, teff_k: null };
      break;
    case "substellar":
      brief = { ...dwarf, kind, class: "T5", log_luminosity_lsun: -5.2, teff_k: 1_100 };
      break;
    case "protostar":
    case "pre_main_sequence":
    case "subgiant":
    case "wolf_rayet":
    case "hot_subdwarf":
      brief = { ...dwarf, kind };
      break;
  }
  return brief;
}

/** What {@link aSystemRecord} builds. */
export interface SystemRecordSpec {
  /** Position in the `GALACTIC` frame. */
  readonly positionLy: readonly [number, number, number];
  readonly layer?: MassLayer;
  /** Distinguishes systems: the ID is this number in hex, and the designation ends with it. */
  readonly index?: number;
  readonly initialMassMsun?: number;
  readonly ageMyr?: number;
  readonly population?: Population;
  /**
   * Its primary's brief: a dwarf typical of the layer when absent, and none, as for a system not
   * yet formed, when `null`.
   */
  readonly stellar?: StellarBriefDto | null;
  /** Velocity along the `GALACTIC` axes, km/s; a disc star's near the Sun by default. */
  readonly velocityKmS?: readonly [number, number, number];
}

/** One system of a range query's answer, as a query with `include_stellar` returns it. */
export function aSystemRecord({
  positionLy,
  layer = "a",
  index = 1,
  initialMassMsun,
  ageMyr = 4_600,
  population = "old_thin_disc",
  stellar,
  velocityKmS = [-231.25, 12.5, -7],
}: SystemRecordSpec): SystemRecord {
  const [low, high] = BANDS_MSUN[layer];
  const record: SystemRecord = {
    id: u64ToHex(BigInt(index)),
    designation: `H7K 4C0RFZ ${layer.toUpperCase()}-${index}`,
    position: galacticPositionFromLy(positionLy),
    layer,
    initial_mass_msun: initialMassMsun ?? (low + high) / 2,
    age_myr: ageMyr,
    population,
    velocity_km_s: [...velocityKmS],
  };
  // A row without a brief leaves the key out, as the server writes it.
  return stellar === null ? record : { ...record, stellar: stellar ?? aStellarBrief(layer) };
}

/** What {@link aCensus} builds. */
export interface CensusSpec {
  /** The lightest layer asked for; lighter ones are below the mass floor. */
  readonly minLayer?: MassLayer;
  /** Layers left out for the census limit; the lightest layers are dropped first. */
  readonly overLimit?: ReadonlyArray<MassLayer>;
  /** Layers left out for the server's cell budget. */
  readonly overCellBudget?: ReadonlyArray<MassLayer>;
  /** Systems returned per included layer. */
  readonly returned?: Partial<Readonly<Record<MassLayer, number>>>;
  readonly limit?: number;
}

/** A consistent census of all five layers. */
export function aCensus({
  minLayer = "a",
  overLimit = [],
  overCellBudget = [],
  returned = {},
  limit = 4_000,
}: CensusSpec = {}): Census {
  const minIndex = LAYERS.indexOf(minLayer);
  const layers = LAYERS.map((layer, index): LayerCensus => {
    let status: LayerStatus = "included";
    if (index < minIndex) {
      status = "below_mass_floor";
    } else if (overLimit.includes(layer)) {
      status = "over_limit";
    } else if (overCellBudget.includes(layer)) {
      status = "over_cell_budget";
    }
    const count = status === "included" ? (returned[layer] ?? 0) : 0;
    const [low, high] = BANDS_MSUN[layer];
    return {
      layer,
      mass_min_msun: low,
      mass_max_msun: high,
      // An included layer's expectation is near what it returned; a dropped one's is large.
      expected: status === "included" ? count + 0.4 : limit * 2.5,
      returned: count,
      status,
    };
  });
  const lightestIncluded = layers.find((layer) => layer.status === "included");
  return {
    limit,
    complete_above_msun: lightestIncluded === undefined ? null : lightestIncluded.mass_min_msun,
    layers,
  };
}

/** What {@link aRangeRequest} builds. */
export interface RangeRequestSpec {
  readonly centreLy?: readonly [number, number, number];
  readonly radiusLy?: number;
  readonly timeYr?: number;
  readonly minLayer?: MassLayer;
  readonly limit?: number;
  readonly universe?: UniverseIdHex;
}

/** A `systems_in_range` request, for comparing with what the client sends: with the briefs. */
export function aRangeRequest({
  centreLy = [26_000, 0, 0],
  radiusLy = 50,
  timeYr = 0,
  minLayer = "a",
  limit = 4_000,
  universe = UNIVERSE_ID,
}: RangeRequestSpec = {}): RequestOf<"systems_in_range"> {
  return {
    kind: "systems_in_range",
    universe,
    centre: galacticPositionFromLy(centreLy),
    radius_ly: radiusLy,
    time: universeTimeFromYears(timeYr),
    min_layer: minLayer,
    limit,
    include_stellar: true,
  };
}

/** One system of {@link aSystemsInRange}, placed relative to the centre. */
export interface RelativeSystemSpec {
  /** Offset from the centre along the `GALACTIC` axes. */
  readonly relLy: readonly [number, number, number];
  readonly layer: MassLayer;
  /** Its primary's brief, as {@link SystemRecordSpec.stellar} has it. */
  readonly stellar?: StellarBriefDto | null;
  /** Velocity along the `GALACTIC` axes, km/s; {@link aSystemRecord}'s default if absent. */
  readonly velocityKmS?: readonly [number, number, number];
}

/** What {@link aSystemsInRange} builds. */
export interface SystemsInRangeSpec {
  readonly systems?: ReadonlyArray<RelativeSystemSpec>;
  readonly centreLy?: readonly [number, number, number];
  readonly radiusLy?: number;
  readonly timeYr?: number;
  readonly minLayer?: MassLayer;
  readonly overLimit?: ReadonlyArray<MassLayer>;
  readonly overCellBudget?: ReadonlyArray<MassLayer>;
  readonly limit?: number;
  readonly universe?: UniverseIdHex;
}

/**
 * The answer to `systems_in_range`, with a census consistent with the systems given.
 *
 * @remarks
 * Systems are numbered from 1 in the order given, which is also the wire order, and each layer's
 * returned count is the number of systems given in it.
 */
export function aSystemsInRange({
  systems = [],
  centreLy = [26_000, 0, 0],
  radiusLy = 50,
  timeYr = 0,
  minLayer = "a",
  overLimit = [],
  overCellBudget = [],
  limit = 4_000,
  universe = UNIVERSE_ID,
}: SystemsInRangeSpec = {}): ResponseFor<"systems_in_range"> {
  const returned: Partial<Record<MassLayer, number>> = {};
  const records = systems.map(({ relLy, layer, stellar, velocityKmS }, position) => {
    returned[layer] = (returned[layer] ?? 0) + 1;
    return aSystemRecord({
      positionLy: [centreLy[0] + relLy[0], centreLy[1] + relLy[1], centreLy[2] + relLy[2]],
      layer,
      index: position + 1,
      ...(stellar === undefined ? {} : { stellar }),
      ...(velocityKmS === undefined ? {} : { velocityKmS }),
    });
  });
  return {
    kind: "systems_in_range",
    universe,
    centre: galacticPositionFromLy(centreLy),
    radius_ly: radiusLy,
    time: universeTimeFromYears(timeYr),
    census: aCensus({ minLayer, overLimit, overCellBudget, returned, limit }),
    systems: records,
  };
}
