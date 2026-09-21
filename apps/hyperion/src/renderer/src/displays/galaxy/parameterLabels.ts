/**
 * The client's glossary of galaxy parameters: a label for each key the server sends.
 *
 * @remarks
 * Plan 04 sends dotted `snake_case` keys and leaves labels to the client, under the guide's "one
 * name per thing" (plan 04, design note 11). The keys are the ones P04.T14.b lists, group by group,
 * and a test holds the fixture of that list to this glossary. A key missing here is shown as
 * itself in upper case (plan 05, design note D8a), so a parameter added by a later plan appears at
 * once and looks unfinished until it is named here.
 *
 * Labels are upper case, so they have three words or fewer (the guide's Typography). A row is
 * always read under its group's heading, which names the component where the label does not:
 * `THIN SCALE LENGTH` under `DISCS`, `SCALE LENGTH` under `NUCLEAR DISC`, a population's name
 * under `POPULATION MASSES`. `HALO` is the stellar halo, as the population is named, and the dark
 * halo is always `DARK HALO`. The halo's dominant component is the debris of the `MAJOR MERGER`,
 * the event whose time `LAST MAJOR MERGER` gives, and the lesser progenitors are `LESSER MERGERS`.
 * A label is kept short enough to fit the parameters' column at 1280 × 720; at the column's 16rem
 * minimum the longest few end in an ellipsis. Units and values are never part of a label: the server sends each
 * value's unit, and the radius at which the rotation is read is a parameter of its own,
 * `rotation.radius`.
 */
import type { Population } from "@hyperion/protocol";

import { populationLabel } from "../../lib/galaxy/wire";

/** Labels of the parameter groups, keyed by the group's key. */
export const GROUP_LABELS: Readonly<Record<string, string>> = {
  identity: "IDENTITY",
  mass: "MASS",
  populations: "POPULATION SHARES",
  population_masses: "POPULATION MASSES",
  population_mean_masses: "MEAN SYSTEM MASSES",
  discs: "DISCS",
  bulge_and_bar: "BULGE AND BAR",
  nuclear_disc: "NUCLEAR DISC",
  halo: "HALO",
  arms: "SPIRAL ARMS",
  history: "HISTORY",
  rotation: "ROTATION",
};

/** The populations, in the wire's order, whose share, mass and mean system mass are sent. */
const POPULATIONS = [
  "young_thin_disc",
  "old_thin_disc",
  "thick_disc",
  "bulge",
  "long_bar",
  "nuclear_disc",
  "halo",
] as const satisfies ReadonlyArray<Population>;

/**
 * Each population's three parameters, labelled with the population's name as the chart names it:
 * the group heading says which quantity the row is.
 */
const POPULATION_LABELS: Readonly<Record<string, string>> = Object.fromEntries(
  POPULATIONS.flatMap((population) =>
    ["share", "mass", "mean_system_mass"].map((quantity) => [
      `population.${population}.${quantity}`,
      populationLabel(population),
    ]),
  ),
);

/** Labels of the parameters, keyed by the parameter's dotted key. */
export const PARAMETER_LABELS: Readonly<Record<string, string>> = {
  seed: "SEED",
  generator_version: "GEN VER",
  mass_function: "MASS FUNCTION",

  stellar_mass: "STELLAR MASS",
  system_count: "SYSTEMS",
  mean_system_mass: "MEAN SYSTEM MASS",
  mean_formed_mass: "MEAN FORMED MASS",
  "gas.mass": "GAS MASS",
  "black_hole.mass": "BLACK HOLE MASS",
  "nuclear_cluster.mass": "NUCLEAR CLUSTER MASS",
  "dark_halo.mass": "DARK HALO MASS",
  "dark_halo.concentration": "DARK HALO CONCENTRATION",
  "dark_halo.virial_radius": "DARK HALO RADIUS",
  "dark_halo.f_star": "STAR FORMATION EFFICIENCY",

  ...POPULATION_LABELS,

  "disc.thin.scale_length": "THIN SCALE LENGTH",
  "disc.thin.scale_height": "THIN SCALE HEIGHT",
  "disc.young.scale_length": "YOUNG SCALE LENGTH",
  "disc.young.scale_height": "YOUNG SCALE HEIGHT",
  "disc.thick.scale_length": "THICK SCALE LENGTH",
  "disc.thick.scale_height": "THICK SCALE HEIGHT",
  "disc.gas.scale_length": "GAS SCALE LENGTH",

  "bulge.scale_x": "BULGE SCALE X",
  "bulge.scale_y": "BULGE SCALE Y",
  "bulge.scale_z": "BULGE SCALE Z",
  "bulge.boxiness": "BULGE BOXINESS",
  "bar.share_of_bulge": "BAR SHARE",
  "bar.half_length": "BAR HALF-LENGTH",
  "bar.width": "BAR WIDTH",
  "bar.height": "BAR HEIGHT",
  "bar.corotation_ratio": "COROTATION RATIO",
  "bar.corotation_radius": "COROTATION RADIUS",
  "bar.pattern_speed": "PATTERN SPEED",

  "nuclear_disc.scale_length": "SCALE LENGTH",
  "nuclear_disc.scale_height": "SCALE HEIGHT",

  "halo.in_situ.share": "IN SITU SHARE",
  "halo.dominant_merger.share": "MAJOR MERGER SHARE",
  "halo.lesser.share": "LESSER MERGERS SHARE",
  "halo.globular_debris.share": "GLOBULAR DEBRIS SHARE",
  "halo.in_situ.slope": "IN SITU SLOPE",
  "halo.dominant_merger.slope": "MAJOR MERGER SLOPE",
  "halo.globular_debris.slope": "GLOBULAR DEBRIS SLOPE",
  "halo.in_situ.core": "IN SITU CORE",
  "halo.dominant_merger.core": "MAJOR MERGER CORE",
  "halo.globular_debris.core": "GLOBULAR DEBRIS CORE",
  "halo.in_situ.flattening": "IN SITU FLATTENING",
  "halo.dominant_merger.flattening": "MAJOR MERGER FLATTENING",
  "halo.dominant_merger.break_radius": "MAJOR MERGER BREAK",
  "halo.dominant_merger.break_steepening": "BREAK STEEPENING",

  "arms.count": "ARMS",
  "arms.pitch": "ARM PITCH",
  "arms.young_width": "YOUNG ARM WIDTH",
  "arms.young_fraction": "YOUNG ARM FRACTION",
  "arms.old_amplitude": "OLD ARM AMPLITUDE",

  "history.formation_timescale": "FORMATION TIMESCALE",
  "history.last_major_merger": "LAST MAJOR MERGER",
  "history.globular_clusters": "GLOBULAR CLUSTERS",

  "rotation.radius": "RADIUS",
  "rotation.circular_speed": "CIRCULAR SPEED",
  "rotation.escape_speed": "ESCAPE SPEED",
};

/** The label of a key in `labels`, or the key itself in upper case when the glossary lacks it. */
function labelOf(labels: Readonly<Record<string, string>>, key: string): string {
  return Object.hasOwn(labels, key) ? (labels[key] ?? key.toUpperCase()) : key.toUpperCase();
}

/** The label of a parameter group, or its key in upper case when the glossary lacks it. */
export function groupLabel(key: string): string {
  return labelOf(GROUP_LABELS, key);
}

/** The label of a parameter, or its key in upper case when the glossary lacks it. */
export function parameterLabel(key: string): string {
  return labelOf(PARAMETER_LABELS, key);
}
