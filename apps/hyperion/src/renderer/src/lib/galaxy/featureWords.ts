/**
 * The words the console names large features in (plan 09, P09.T41), from the guide's nomenclature
 * list: each kind's abbreviation, which stands beside its mark, and its name in words, which the
 * list and the readout give.
 */
import type { FeatureKindWire, MapFeatureKindWire } from "@hyperion/protocol";

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
