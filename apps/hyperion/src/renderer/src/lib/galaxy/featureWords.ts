/**
 * The words the console names large features in (plan 09, P09.T41), from the guide's nomenclature
 * list: each kind's abbreviation, which stands beside its mark, and its name in words, which the
 * list and the readout give.
 */
import type {
  CatalogueClassWire,
  CentreTracerWire,
  ClassKindWire,
  EmissionClassWire,
  FeatureKindWire,
  MapFeatureKindWire,
  ShellPhaseWire,
} from "@hyperion/protocol";

import type { CatalogueClass, ChartLayer, MemberClass } from "./model";
import { layerLetter, UNLISTED_LAYER } from "./wire";

/**
 * A band's letter in a composed reading: {@link UNLISTED_LAYER} for a layer this build does not
 * know, which the server did send, so that the reading keeps its other parts
 * (`LIVING · UNLISTED LAYER · FIRST · BINARY`).
 */
function bandLetter(band: ChartLayer): string {
  return layerLetter(band) ?? UNLISTED_LAYER;
}

/**
 * A feature kind's abbreviation, as its mark's label and the list's `KIND` column write it: `GC`,
 * `OC`, `OB`, `SFR` and `MC`, and the centre, a stream and a dwarf core in words, which have none.
 */
export function featureKindAbbreviation(kind: FeatureKindWire): string {
  let abbreviation: string;
  switch (kind) {
    case "globular":
      abbreviation = "GC";
      break;
    case "open_cluster":
      abbreviation = "OC";
      break;
    case "association":
      abbreviation = "OB";
      break;
    case "star_forming_region":
      abbreviation = "SFR";
      break;
    case "molecular_cloud":
      abbreviation = "MC";
      break;
    case "galactic_centre":
      abbreviation = "CENTRE";
      break;
    case "stream":
      abbreviation = "STREAM";
      break;
    case "dwarf_core":
      abbreviation = "DWARF CORE";
      break;
  }
  return abbreviation;
}

/**
 * The label of a feature's mark: its kind's abbreviation and its designation, as the guide writes a
 * named group (`OC GPF F207`, `GC 47 TUC`), or the designation alone where it is already the
 * kind's name, as the centre's `CENTRE` is.
 */
export function featureMarkLabel(kind: FeatureKindWire, designation: string): string {
  const abbreviation = featureKindAbbreviation(kind);
  return designation === abbreviation ? designation : `${abbreviation} ${designation}`;
}

/**
 * What the galaxy map's overlay names a kind it draws: the globulars' and the centre's
 * abbreviations, and a bright shell's emission class, `SNR`, since a shell is no kind of feature.
 */
export function mapFeatureKindLabel(kind: MapFeatureKindWire): string {
  let label: string;
  switch (kind) {
    case "globular":
    case "galactic_centre":
      label = featureKindAbbreviation(kind);
      break;
    case "bright_shell":
      label = "SNR";
      break;
  }
  return label;
}

/**
 * How many of a kind the galaxy map draws, in words, for the overlay's accessible name: `142
 * globular clusters`, `1 bright supernova remnant shell`, and the centre, of which there is one,
 * `the galactic centre`.
 */
export function mapFeatureKindWords(kind: MapFeatureKindWire, count: number): string {
  let words: string;
  switch (kind) {
    case "globular":
      words = `${String(count)} globular ${count === 1 ? "cluster" : "clusters"}`;
      break;
    case "galactic_centre":
      words = "the galactic centre";
      break;
    case "bright_shell":
      words = `${String(count)} bright supernova remnant ${count === 1 ? "shell" : "shells"}`;
      break;
  }
  return words;
}

/** A feature kind in words, as the list's name for a row and the readout's `KIND` give it. */
export function featureKindWords(kind: FeatureKindWire): string {
  let words: string;
  switch (kind) {
    case "globular":
      words = "GLOBULAR CLUSTER";
      break;
    case "open_cluster":
      words = "OPEN CLUSTER";
      break;
    case "association":
      words = "OB ASSOCIATION";
      break;
    case "star_forming_region":
      words = "STAR-FORMING REGION";
      break;
    case "molecular_cloud":
      words = "MOLECULAR CLOUD";
      break;
    case "galactic_centre":
      words = "GALACTIC CENTRE";
      break;
    case "stream":
      words = "STREAM";
      break;
    case "dwarf_core":
      words = "DWARF CORE";
      break;
  }
  return words;
}

/**
 * A feature's emission class as the guide names it: `H II`, `REFLECTION`, `DARK CLOUD`, `SNR` and
 * `PWN`.
 */
export function emissionWords(emission: EmissionClassWire): string {
  let words: string;
  switch (emission) {
    case "hii_region":
      words = "H II";
      break;
    case "reflection_nebula":
      words = "REFLECTION";
      break;
    case "dark_cloud":
      words = "DARK CLOUD";
      break;
    case "remnant_shell":
      words = "SNR";
      break;
    case "pulsar_wind_nebula":
      words = "PWN";
      break;
  }
  return words;
}

/** A catalogue class this build knows, in words. */
function knownCatalogueClassWords(value: CatalogueClassWire): string {
  let words: string;
  switch (value) {
    case "core_collapse":
      words = "CORE-COLLAPSE SUPERNOVA";
      break;
    case "type_ia":
      words = "TYPE Ia SUPERNOVA";
      break;
    case "stellar_merger":
      words = "STELLAR MERGER";
      break;
    case "neutron_star_merger":
      words = "NEUTRON STAR MERGER";
      break;
    case "luminous_blue_variable":
      words = "LUMINOUS BLUE VARIABLE";
      break;
    case "xray_binary":
      words = "X-RAY BINARY";
      break;
    case "accreting_white_dwarf":
      words = "ACCRETING WD";
      break;
    case "tidal_disruption_victim":
      words = "TIDAL DISRUPTION VICTIM";
      break;
    case "accreting_white_dwarf_slow":
      words = "SLOW ACCRETING WD";
      break;
  }
  return words;
}

/**
 * A catalogue class in words (plan 09's registry): `X-RAY BINARY`, `CORE-COLLAPSE SUPERNOVA`; `null`
 * for one this build does not know, which reads Missing.
 */
export function catalogueClassWords(value: CatalogueClass): string | null {
  return value === "unknown" ? null : knownCatalogueClassWords(value);
}

/** What a member class's systems are, in words. */
function classKindWords(kind: ClassKindWire): string {
  let words: string;
  switch (kind) {
    case "living":
      words = "LIVING";
      break;
    case "white_dwarf":
      words = "WHITE DWARF";
      break;
    case "neutron_star":
      words = "NEUTRON STAR";
      break;
    case "black_hole":
      words = "BLACK HOLE";
      break;
    case "tail":
      words = "TIDAL TAIL";
      break;
  }
  return words;
}

/** The distribution function a centre class's orbits are drawn from, in words. */
function tracerWords(tracer: CentreTracerWire): string {
  let words: string;
  switch (tracer) {
    case "stars":
      words = "STARS";
      break;
    case "black_holes":
      words = "BLACK HOLES";
      break;
    case "young_disc":
      words = "YOUNG DISC";
      break;
    case "young_isotropic":
      words = "YOUNG ISOTROPIC";
      break;
  }
  return words;
}

/**
 * A member's class in words, its parts joined by a middle dot as a reading's are: what its systems
 * are, the layer of its primaries' band, and a cluster's generation and multiplicity
 * (`LIVING · A · FIRST · BINARY`) or the centre's episode and tracer (`LIVING · A · EPISODE 0 ·
 * STARS`), or a listed entry's catalogue class; `null` for a class or a catalogue class this build
 * does not know, which reads Missing. A band's layer it does not know is named in its place
 * (`LIVING · UNLISTED LAYER · FIRST · BINARY`).
 */
export function memberClassWords(memberClass: MemberClass): string | null {
  let words: string | null;
  switch (memberClass.kind) {
    case "cluster":
      words = [
        classKindWords(memberClass.classKind),
        bandLetter(memberClass.band),
        memberClass.generation.toUpperCase(),
        memberClass.multiplicity.toUpperCase(),
      ].join(" · ");
      break;
    case "centre":
      words = [
        classKindWords(memberClass.classKind),
        bandLetter(memberClass.band),
        `EPISODE ${String(memberClass.episode)}`,
        tracerWords(memberClass.tracer),
      ].join(" · ");
      break;
    case "listed":
      words = catalogueClassWords(memberClass.catalogueClass);
      break;
    case "unknown":
      words = null;
      break;
  }
  return words;
}

/** A remnant shell's phase in words. */
export function shellPhaseWords(phase: ShellPhaseWire): string {
  let words: string;
  switch (phase) {
    case "free_expansion":
      words = "FREE EXPANSION";
      break;
    case "sedov_taylor":
      words = "SEDOV-TAYLOR";
      break;
    case "pressure_driven_snowplough":
      words = "PRESSURE-DRIVEN SNOWPLOUGH";
      break;
    case "momentum_conserving_snowplough":
      words = "MOMENTUM-CONSERVING SNOWPLOUGH";
      break;
  }
  return words;
}

/**
 * What a display does not model, composed as the guide composes it: the names, the last joined by
 * `AND`, a colon, and the state, `STREAM AND DWARF CORE: NOT YET MODELLED`, `SNR: NOT YET MODELLED`.
 */
export function notYetModelledWords(names: ReadonlyArray<string>): string {
  const last = names.at(-1) ?? "";
  const named = names.length > 1 ? `${names.slice(0, -1).join(", ")} AND ${last}` : last;
  return `${named}: NOT YET MODELLED`;
}
