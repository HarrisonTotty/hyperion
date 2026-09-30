/**
 * The words an interacting binary's class is read in (plan 11, P11.T14), in upper case as the
 * `GALAXY` readout's star list shows them.
 *
 * @remarks
 * Every term is spelled out, since the guide allows only abbreviations on its nomenclature list: a
 * cataclysmic variable and an X-ray binary are each named by their kind, as astronomers name them
 * (`DWARF NOVA`, `X-RAY NOVA`; the orchestrator's ruling 130.3). A classification symbol inside the
 * label keeps the astronomers' case, as a unit symbol does (`Be X-RAY BINARY`, `TYPE Ia
 * PROGENITOR`, ruling 130.4); names stay in upper case, a constellation's abbreviation among them:
 * `AM CVN STAR`, as `GC 47 TUC`.
 */
import type { BinaryClassDto } from "@hyperion/protocol";

import type { Modelled, SystemModel } from "../system/model";
import { starListRows } from "../system/starList";

/** A binary class in words: `ALGOL`, `DWARF NOVA`, `X-RAY NOVA`. */
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
          label = "LOW-MASS X-RAY BINARY";
          break;
        case "transient":
          label = "X-RAY NOVA";
          break;
        case "symbiotic":
          label = "SYMBIOTIC X-RAY BINARY";
          break;
      }
      break;
    case "high_mass_xray_binary":
      switch (binaryClass.kind) {
        case "be_x":
          label = "Be X-RAY BINARY";
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
      label = "TYPE Ia PROGENITOR";
      break;
  }
  return label;
}

/** One row of a system's binary-class table: the stars a class belongs to, and the class. */
export interface BinaryClassRow {
  /** The row's key, unique within the system: the first star's body ID. */
  readonly id: string;
  /** `A–B` for an innermost pair whose stars share the class, as `ORBITS` names it; else `A`. */
  readonly stars: string;
  /** The class, or not computed for this star. */
  readonly binaryClass: Exclude<Modelled<BinaryClassDto>, { readonly kind: "none" }>;
}

/** A system's binary classes: not computed for any star, or the rows (none when no star has one). */
export type BinaryClassRows =
  | { readonly kind: "not_modelled" }
  | { readonly kind: "rows"; readonly rows: ReadonlyArray<BinaryClassRow> };

/** Whether two computed classes are the same class of the same kind. */
function sameClass(a: BinaryClassDto, b: BinaryClassDto): boolean {
  return a.type === b.type && ("kind" in a ? a.kind : null) === ("kind" in b ? b.kind : null);
}

/**
 * The rows of a system's binary-class table (the orchestrator's ruling 130.1): one row for each
 * innermost pair whose two stars carry the same class, named `A–B` as `ORBITS` names it, and one
 * row for each star that carries a class alone (a merger's product, a star whose partner's class
 * differs or is not computed), named by its letter.
 *
 * @remarks
 * The sim classes a pair, and the server gives each star its pair's class, so a class that names
 * one star (`HOT SUBDWARF`) is read against the pair, and `STATE` says which star it is. Rows keep
 * the star list's order, by their first letter. A star in no class has no row.
 */
export function binaryClassRows(model: SystemModel): BinaryClassRows {
  const { stars } = starListRows(model);
  if (stars.every(({ host }) => host.binaryClass.kind === "not_modelled")) {
    return { kind: "not_modelled" };
  }
  const partners = new Map<number, number>();
  for (const node of model.hierarchy) {
    if (node.kind !== "pair") {
      continue;
    }
    const inner = model.hierarchy[node.inner];
    const outer = model.hierarchy[node.outer];
    if (inner?.kind === "star" && outer?.kind === "star") {
      partners.set(inner.bodyIndex, outer.bodyIndex);
      partners.set(outer.bodyIndex, inner.bodyIndex);
    }
  }
  const taken = new Set<number>();
  const rows: BinaryClassRow[] = [];
  for (const { letter, host } of stars) {
    const own = host.binaryClass;
    if (taken.has(host.bodyIndex) || own.kind === "none") {
      continue;
    }
    const partnerIndex = partners.get(host.bodyIndex);
    const partner = stars.find((star) => star.host.bodyIndex === partnerIndex);
    const other = partner?.host.binaryClass;
    if (
      partner !== undefined &&
      own.kind === "value" &&
      other?.kind === "value" &&
      sameClass(own.value, other.value)
    ) {
      taken.add(partner.host.bodyIndex);
      rows.push({ id: host.id, stars: `${letter}–${partner.letter}`, binaryClass: own });
    } else {
      rows.push({ id: host.id, stars: letter, binaryClass: own });
    }
  }
  return { kind: "rows", rows };
}
