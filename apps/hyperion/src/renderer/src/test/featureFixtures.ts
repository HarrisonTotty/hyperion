/**
 * Builders for plan 09's feature messages, typed against the generated bindings so that a protocol
 * change breaks them at compile time, with values cut from the server's golden answers
 * (`crates/hyperion-server/tests/golden/features.golden`, generator version 23).
 *
 * @remarks
 * The golden's universe is the server tests' seed, whose features in range of its Sun-like point,
 * (0, 26,000, 0) ly within 300 ly at the epoch, are three open clusters, 27 associations and three
 * star-forming regions; a few are copied here whole. The response builders include the `kind`, so
 * that they can be passed straight to `FakeWebSocket.serverAnswers`.
 */
import {
  type BrightShell,
  type FeatureKindCensus,
  type FeatureKindWire,
  type FeatureSummary,
  galacticPositionFromLy,
  type ResponseFor,
  universeTimeFromYears,
} from "@hyperion/protocol";

import { UNIVERSE_ID } from "./galaxyFixtures";

/** The golden's range query's centre, its Sun-like point. */
export const GOLDEN_CENTRE_LY = [0, 26_000, 0] as const;

/** The golden's range query's radius. */
export const GOLDEN_RADIUS_LY = 300;

/** The golden's nearest open cluster, with a superbubble: `GPF F207` at the epoch. */
export const OPEN_CLUSTER: FeatureSummary = {
  id: "e859e06780000000",
  designation: "GPF F207",
  kind: "open_cluster",
  position: {
    cell_ly: [106, 26_115, -22],
    offset_m: [6_814_297_182_197_701.0, 1_813_335_634_579_622.8, 2_887_324_945_432_568.5],
  },
  extent_ly: 26.909_037_649_658_29,
  age_myr: 8.408_509_092_144_168,
  mass_msun: 267.619_558_941_131_63,
  expected_members: 459.101_118_889_670_6,
  emission: null,
  bubble_radius_ly: 378.839_766_237_665_6,
};

/** The golden's star-forming region, a reflection nebula: `GPG F336` at the epoch. */
export const STAR_FORMING_REGION: FeatureSummary = {
  id: "e85a00a800000000",
  designation: "GPG F336",
  kind: "star_forming_region",
  position: {
    cell_ly: [33, 26_169, 66],
    offset_m: [6_126_530_108_715_611.0, 5_041_680_142_170_773.0, 9_060_956_692_131_634.0],
  },
  extent_ly: 10.786_549_084_657_327,
  age_myr: 3.173_389_158_323_052_7,
  mass_msun: 112.279_197_379_825_15,
  expected_members: 0,
  emission: "reflection_nebula",
  bubble_radius_ly: null,
};

/** The golden's association with an H II region: `GPG F530` at the epoch. */
export const ASSOCIATION: FeatureSummary = {
  id: "e85a010900000000",
  designation: "GPG F530",
  kind: "association",
  position: {
    cell_ly: [192, 25_735, 8],
    offset_m: [869_490_455_204_198.9, 6_559_070_329_217_107.0, 3_266_103_891_462_896.0],
  },
  extent_ly: 52.820_862_046_626_67,
  age_myr: 9.011_037_437_136_846,
  mass_msun: 12_319.572_113_971_775,
  expected_members: 0,
  emission: "hii_region",
  bubble_radius_ly: 732.420_253_302_509,
};

/** The golden's dark cloud: `FNF F260` at the epoch. */
export const MOLECULAR_CLOUD: FeatureSummary = {
  id: "e7d5e08200000000",
  designation: "FNF F260",
  kind: "molecular_cloud",
  position: {
    cell_ly: [-1690, 24_463, -307],
    offset_m: [8_378_351_580_031_907.0, 4_857_206_139_830_433.0, 7_449_808_021_708_336.0],
  },
  extent_ly: 20.589_191_396_493_003,
  age_myr: null,
  mass_msun: 17_637.560_149_237_426,
  expected_members: 0,
  emission: "dark_cloud",
  bubble_radius_ly: null,
};

/** The golden's galactic centre, which has no single age and does not move. */
export const GALACTIC_CENTRE: FeatureSummary = {
  id: "f000000000000000",
  designation: "CENTRE",
  kind: "galactic_centre",
  position: { cell_ly: [0, 0, 0], offset_m: [0, 0, 0] },
  extent_ly: 128,
  age_myr: null,
  mass_msun: 38_040_514.463_780_81,
  expected_members: 91_393_385.363_685_76,
  emission: null,
  bubble_radius_ly: null,
};

/** The first globular of the golden's galaxy map: `5FG F0`. */
export const GLOBULAR: FeatureSummary = {
  id: "e2be000000000000",
  designation: "5FG F0",
  kind: "globular",
  position: {
    cell_ly: [-43_146, -4068, 3191],
    offset_m: [2_389_853_666_176_678.5, 7_073_204_358_720_961.0, 1_265_109_897_788_180.0],
  },
  extent_ly: 205.469_860_258_587_12,
  age_myr: 12_667.937_550_744_378,
  mass_msun: 37_665.879_536_481_25,
  expected_members: 88_639.817_579_495_56,
  emission: null,
  bubble_radius_ly: null,
};

/** Every kind a chart asks for, in its order, each `included` but plan 10's, as the golden has it. */
const CENSUS_KINDS: ReadonlyArray<FeatureKindWire> = [
  "globular",
  "open_cluster",
  "association",
  "star_forming_region",
  "molecular_cloud",
  "galactic_centre",
  "stream",
  "dwarf_core",
];

/** What a features-in-range answer is built from. */
export interface FeaturesInRangeSpec {
  readonly centreLy?: readonly [number, number, number];
  readonly radiusLy?: number;
  readonly timeYr?: number;
  readonly features?: ReadonlyArray<FeatureSummary>;
  /** Each line's status by kind; the golden's when absent. */
  readonly status?: Partial<Record<FeatureKindWire, FeatureKindCensus["status"]>>;
}

/**
 * A features-in-range answer: the golden's centre, radius and time unless told otherwise, the
 * features given, and a census line for every kind, its count the features of that kind.
 */
export function aFeaturesInRange({
  centreLy = GOLDEN_CENTRE_LY,
  radiusLy = GOLDEN_RADIUS_LY,
  timeYr = 0,
  features = [OPEN_CLUSTER, STAR_FORMING_REGION, ASSOCIATION],
  status = {},
}: FeaturesInRangeSpec = {}): ResponseFor<"features_in_range"> {
  return {
    kind: "features_in_range",
    universe: UNIVERSE_ID,
    centre: galacticPositionFromLy(centreLy),
    radius_ly: radiusLy,
    time: universeTimeFromYears(timeYr),
    census: CENSUS_KINDS.map((kind) => {
      const lineStatus =
        status[kind] ?? (kind === "stream" || kind === "dwarf_core" ? "not_generated" : "included");
      return {
        kind,
        returned:
          lineStatus === "included" ? features.filter((each) => each.kind === kind).length : 0,
        status: lineStatus,
      };
    }),
    features: [...features],
  };
}

/** A bright shell for the galaxy map, as P09.T37's would be: none is generated at version 23. */
export const BRIGHT_SHELL: BrightShell = {
  system: "e400000500000000",
  designation: "4RG C5",
  site: galacticPositionFromLy([8_000, 2_000, 30]),
  age_yr: 2_400,
  shell: {
    radius_ly: 30,
    shock_speed_km_s: 2_600,
    phase: "sedov_taylor",
    emission: { x_ray_and_radio: true, optical: true, neutral_hydrogen: false },
  },
};

/** What the galaxy map's features answer is built from. */
export interface GalaxyFeaturesSpec {
  readonly globulars?: ReadonlyArray<FeatureSummary>;
  readonly centre?: ReadonlyArray<FeatureSummary>;
  /** The bright shells, or `null` for the golden's `not_generated`. */
  readonly shells?: ReadonlyArray<BrightShell> | null;
}

/**
 * The galaxy map's features answer at the epoch: complete globulars and centre, and the bright
 * shells not generated, as the golden has them at version 23, unless told otherwise.
 */
export function aGalaxyFeatures({
  globulars = [GLOBULAR],
  centre = [GALACTIC_CENTRE],
  shells = null,
}: GalaxyFeaturesSpec = {}): ResponseFor<"galaxy_features"> {
  return {
    kind: "galaxy_features",
    universe: UNIVERSE_ID,
    time: universeTimeFromYears(0),
    lists: [
      { kind: "globular", status: "complete", features: [...globulars] },
      { kind: "galactic_centre", status: "complete", features: [...centre] },
      shells === null
        ? { kind: "bright_shell", status: "not_generated" }
        : { kind: "bright_shell", status: "complete", shells: [...shells] },
    ],
  };
}
