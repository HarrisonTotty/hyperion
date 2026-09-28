/**
 * The words an interacting binary's class is read in (plan 11, P11.T14), in upper case as the
 * `GALAXY` readout's star list shows them.
 *
 * @remarks
 * Every term is spelled out, since the guide allows only abbreviations on its nomenclature list: a
 * cataclysmic variable is named by its kind, an X-ray binary's kind follows it after a middle dot,
 * as the guide joins the parts of one reading. `AM CVN` and `R CORONAE BOREALIS` are the names of
 * the prototype stars; `CVN` is the constellation's IAU abbreviation inside that name, as `47 TUC`
 * is inside a cluster's.
 */
import type { BinaryClassDto } from "@hyperion/protocol";

/** A binary class in words: `ALGOL`, `DWARF NOVA`, `LOW-MASS X-RAY BINARY · TRANSIENT`. */
export function binaryClassLabel(binaryClass: BinaryClassDto): string {
  let label: string;
  switch (binaryClass.type) {
    case "algol":
      label = "ALGOL";
      break;
    case "contact":
      label = "CONTACT BINARY";
      break;
    case "blue_straggler":
      label = "BLUE STRAGGLER";
      break;
    case "hot_subdwarf":
      label = "HOT SUBDWARF";
      break;
    case "r_coronae_borealis":
      label = "R CORONAE BOREALIS STAR";
      break;
    case "symbiotic":
      label = "SYMBIOTIC STAR";
      break;
    case "cataclysmic_variable":
      switch (binaryClass.kind) {
        case "dwarf_nova":
          label = "DWARF NOVA";
          break;
        case "nova_like":
          label = "NOVA-LIKE VARIABLE";
          break;
        case "magnetic":
          label = "MAGNETIC CATACLYSMIC VARIABLE";
          break;
        case "am_cvn":
          label = "AM CVN STAR";
          break;
      }
      break;
    case "low_mass_xray_binary":
      switch (binaryClass.kind) {
        case "persistent":
          label = "LOW-MASS X-RAY BINARY · PERSISTENT";
          break;
        case "transient":
          label = "LOW-MASS X-RAY BINARY · TRANSIENT";
          break;
      }
      break;
    case "high_mass_xray_binary":
      switch (binaryClass.kind) {
        case "be_x":
          label = "BE X-RAY BINARY";
          break;
        case "supergiant":
          label = "SUPERGIANT X-RAY BINARY";
          break;
      }
      break;
    case "millisecond_pulsar":
      label = "MILLISECOND PULSAR";
      break;
    case "double_neutron_star":
      label = "DOUBLE NEUTRON STAR";
      break;
    case "double_white_dwarf":
      label = "DOUBLE WHITE DWARF";
      break;
    case "type_ia_progenitor":
      label = "TYPE IA PROGENITOR";
      break;
  }
  return label;
}
