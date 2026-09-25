/**
 * The words a body's kind, state, class and sections are read in, in upper case as the list and
 * the readout show them, so that a symbol's shape is never the only signal of what a body is (plan
 * 14, P14.T43).
 *
 * @remarks
 * Every term is spelled out, since the guide allows only abbreviations on its nomenclature list.
 * The section states are the owner's draft of the guide's data states (the orchestrator's rulings
 * 33 and 34).
 */
import type {
  ArchitectureClassDto,
  BeltCompositionDto,
  BeltSiteDto,
  DestructionCauseDto,
  DetailLevelDto,
  MoonOriginDto,
  PlanetClassDto,
  RingKindDto,
  RingMaterialDto,
} from "@hyperion/protocol";

import type { BodyKind, BodyState, Section } from "./model";

/** A section that stands in place of its values: withheld, or not computed. */
export type SectionStandIn = Extract<Section<unknown>["state"], "not_resolved" | "not_modelled">;

/** What a section reads in place of its values: `NOT RESOLVED` or `NOT YET MODELLED`. */
export function sectionStateLabel(state: SectionStandIn): string {
  return state === "not_resolved" ? "NOT RESOLVED" : "NOT YET MODELLED";
}

/** What a body is, in words: `PLANET`, `DWARF PLANET`, `KUIPER BELT`, `UNRESOLVED CONTACT`. */
export function bodyKindLabel(kind: BodyKind): string {
  let label: string;
  switch (kind.kind) {
    case "planet":
      label = "PLANET";
      break;
    case "dwarf_planet":
      label = "DWARF PLANET";
      break;
    case "moon":
      label = "MOON";
      break;
    case "ring":
      label = "RING";
      break;
    case "belt":
      label = kind.beltKind === "asteroid" ? "ASTEROID BELT" : "KUIPER BELT";
      break;
    case "cometary_halo":
      label = "COMETARY HALO";
      break;
    case "protoplanetary_disc":
      label = "PROTOPLANETARY DISC";
      break;
    case "debris_disc":
      label = "DEBRIS DISC";
      break;
    case "unresolved":
      label = "UNRESOLVED CONTACT";
      break;
  }
  return label;
}

/** How a moon came to orbit its planet, in words. */
export function moonOriginLabel(origin: MoonOriginDto): string {
  let label: string;
  switch (origin) {
    case "regular":
      label = "REGULAR";
      break;
    case "giant_impact":
      label = "GIANT IMPACT";
      break;
    case "captured":
      label = "CAPTURED";
      break;
  }
  return label;
}

/** What has become of a body, in words: `PRESENT`, `NOT YET FORMED`, `DESTROYED`, `UNBOUND`. */
export function bodyStateLabel(state: BodyState): string {
  let label: string;
  switch (state.kind) {
    case "not_yet_formed":
      label = "NOT YET FORMED";
      break;
    case "present":
      label = "PRESENT";
      break;
    case "destroyed":
      label = "DESTROYED";
      break;
    case "unbound":
      label = "UNBOUND";
      break;
  }
  return label;
}

/** What destroyed a body, in words. */
export function destructionCauseLabel(cause: DestructionCauseDto): string {
  let label: string;
  switch (cause) {
    case "dispersed":
      label = "DISPERSED";
      break;
    case "engulfed":
      label = "ENGULFED";
      break;
    case "tidally_disrupted":
      label = "TIDALLY DISRUPTED";
      break;
    case "collided":
      label = "COLLIDED";
      break;
  }
  return label;
}

/** A planet's class by composition, in words: `ROCKY`, `SUB-NEPTUNE`, `GAS GIANT`. */
export function planetClassLabel(planetClass: PlanetClassDto): string {
  let label: string;
  switch (planetClass) {
    case "rocky":
      label = "ROCKY";
      break;
    case "icy":
      label = "ICY";
      break;
    case "sub_neptune":
      label = "SUB-NEPTUNE";
      break;
    case "ice_giant":
      label = "ICE GIANT";
      break;
    case "gas_giant":
      label = "GAS GIANT";
      break;
  }
  return label;
}

/** A host's planetary architecture (plan 14, P14.T4), in words: `SOLAR-LIKE`, `HOT JUPITER`. */
export function architectureLabel(architecture: ArchitectureClassDto): string {
  let label: string;
  switch (architecture) {
    case "barren":
      label = "BARREN";
      break;
    case "terrestrial_only":
      label = "TERRESTRIAL ONLY";
      break;
    case "compact_multi":
      label = "COMPACT MULTI";
      break;
    case "compact_with_cold_giant":
      label = "COMPACT WITH COLD GIANT";
      break;
    case "solar_like":
      label = "SOLAR-LIKE";
      break;
    case "eccentric_giant":
      label = "ECCENTRIC GIANT";
      break;
    case "warm_giant":
      label = "WARM GIANT";
      break;
    case "hot_jupiter":
      label = "HOT JUPITER";
      break;
    case "substellar_compact":
      label = "SUBSTELLAR COMPACT";
      break;
  }
  return label;
}

/**
 * The detail level a record holds, in words: `MASS AND ORBIT ONLY` (plan 14, P14.T41.b), and each
 * level above it by the section it reaches, up to `FULL`.
 */
export function detailLevelLabel(level: DetailLevelDto): string {
  let label: string;
  switch (level) {
    case "contact":
      label = "CONTACT ONLY";
      break;
    case "mass_and_orbit":
      label = "MASS AND ORBIT ONLY";
      break;
    case "bulk":
      label = "TO BULK";
      break;
    case "surface":
      label = "TO SURFACE";
      break;
    case "full":
      label = "FULL";
      break;
  }
  return label;
}

/** Which of a giant's rings a ring is, in words: `MASSIVE`, `DUSTY`. */
export function ringKindLabel(kind: RingKindDto): string {
  let label: string;
  switch (kind) {
    case "massive":
      label = "MASSIVE";
      break;
    case "dusty":
      label = "DUSTY";
      break;
  }
  return label;
}

/** What a ring is made of, in words: `POROUS ICE`, `ROCK`. */
export function ringMaterialLabel(material: RingMaterialDto): string {
  let label: string;
  switch (material) {
    case "porous_ice":
      label = "POROUS ICE";
      break;
    case "rock":
      label = "ROCK";
      break;
  }
  return label;
}

/**
 * Where a belt lies, by the rule that placed it, in words: `INSIDE GIANT` (inside a giant's
 * orbit), `IN GAP` (in a gap between planets), `BEYOND PLANETS`, `OUTER DISC`.
 */
export function beltSiteLabel(site: BeltSiteDto): string {
  let label: string;
  switch (site) {
    case "inside_giant":
      label = "INSIDE GIANT";
      break;
    case "gap":
      label = "IN GAP";
      break;
    case "beyond_planets":
      label = "BEYOND PLANETS";
      break;
    case "outer_disc":
      label = "OUTER DISC";
      break;
  }
  return label;
}

/** Which side of the snow line most of a belt's solids lie on, in words: `ROCKY`, `ICY`. */
export function beltCompositionLabel(composition: BeltCompositionDto): string {
  let label: string;
  switch (composition) {
    case "rocky":
      label = "ROCKY";
      break;
    case "icy":
      label = "ICY";
      break;
  }
  return label;
}
