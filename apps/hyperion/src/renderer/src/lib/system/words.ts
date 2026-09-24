/**
 * The words a star's kind, phase and remnant are read in, in upper case as the readouts show them,
 * so that a symbol's shape is never the only signal of what a body is.
 *
 * @remarks
 * Every term is spelled out: the guide allows only abbreviations on its nomenclature list, which has
 * none for the asymptotic giant branch or a Wolf-Rayet star.
 */
import type { ObjectKindDto, PhaseDto, VariableKindDto } from "@hyperion/protocol";

import type { HostRemnant } from "./model";

/** What a star is now, in words: `DWARF`, `WHITE DWARF`, `BROWN DWARF`. */
export function objectKindLabel(kind: ObjectKindDto): string {
  let label: string;
  switch (kind) {
    case "protostar":
      label = "PROTOSTAR";
      break;
    case "pre_main_sequence":
      label = "PRE-MAIN-SEQUENCE STAR";
      break;
    case "dwarf":
      label = "DWARF";
      break;
    case "subgiant":
      label = "SUBGIANT";
      break;
    case "giant":
      label = "GIANT";
      break;
    case "supergiant":
      label = "SUPERGIANT";
      break;
    case "wolf_rayet":
      label = "WOLF-RAYET STAR";
      break;
    case "hot_subdwarf":
      label = "HOT SUBDWARF";
      break;
    case "white_dwarf":
      label = "WHITE DWARF";
      break;
    case "neutron_star":
      label = "NEUTRON STAR";
      break;
    case "black_hole":
      label = "BLACK HOLE";
      break;
    case "no_remnant":
      label = "NO REMNANT";
      break;
    case "substellar":
      label = "BROWN DWARF";
      break;
  }
  return label;
}

/** A star's evolutionary phase, in words: `MAIN SEQUENCE`, `CORE HELIUM BURNING`. */
export function phaseLabel(phase: PhaseDto): string {
  let label: string;
  switch (phase) {
    case "protostar":
      label = "PROTOSTAR";
      break;
    case "pre_main_sequence":
      label = "PRE-MAIN SEQUENCE";
      break;
    case "main_sequence":
      label = "MAIN SEQUENCE";
      break;
    case "hertzsprung_gap":
      label = "HERTZSPRUNG GAP";
      break;
    case "first_giant_branch":
      label = "FIRST GIANT BRANCH";
      break;
    case "core_helium_burning":
      label = "CORE HELIUM BURNING";
      break;
    case "early_agb":
      label = "EARLY ASYMPTOTIC GIANT BRANCH";
      break;
    case "thermally_pulsing_agb":
      label = "THERMALLY PULSING ASYMPTOTIC GIANT BRANCH";
      break;
    case "helium_main_sequence":
      label = "HELIUM MAIN SEQUENCE";
      break;
    case "helium_hertzsprung_gap":
      label = "HELIUM HERTZSPRUNG GAP";
      break;
    case "helium_giant_branch":
      label = "HELIUM GIANT BRANCH";
      break;
    case "post_agb":
      label = "POST-ASYMPTOTIC GIANT BRANCH";
      break;
    case "helium_white_dwarf":
      label = "HELIUM WHITE DWARF";
      break;
    case "carbon_oxygen_white_dwarf":
      label = "CARBON-OXYGEN WHITE DWARF";
      break;
    case "oxygen_neon_white_dwarf":
      label = "OXYGEN-NEON WHITE DWARF";
      break;
    case "neutron_star":
      label = "NEUTRON STAR";
      break;
    case "black_hole":
      label = "BLACK HOLE";
      break;
    case "no_remnant":
      label = "NO REMNANT";
      break;
    case "substellar":
      label = "SUBSTELLAR";
      break;
  }
  return label;
}

/** What a dead star left, in words. */
export function remnantLabel(remnant: HostRemnant): string {
  let label: string;
  switch (remnant.kind) {
    case "white_dwarf":
      label = "WHITE DWARF";
      break;
    case "neutron_star":
      label = "NEUTRON STAR";
      break;
    case "black_hole":
      label = "BLACK HOLE";
      break;
    case "no_remnant":
      label = "NONE";
      break;
  }
  return label;
}

/**
 * A kind of variable star in words: its wire name in upper case with spaces, `DELTA SCUTI`,
 * `CLASSICAL CEPHEID`.
 *
 * @remarks
 * The kinds are plan 06's T26, which no generator version computes yet; each reads as its name
 * until the guide gives the set its own words.
 */
export function variableKindLabel(kind: VariableKindDto): string {
  return kind.replaceAll("_", " ").toUpperCase();
}
