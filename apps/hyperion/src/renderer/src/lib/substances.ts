/**
 * The name a console shows for a substance, from its registry key (galaxy plan 14, P14.T49.f;
 * decision-substance-names).
 *
 * @remarks
 * Every substance reaches the client as its key in the substance registry (P14.T49.a), and no
 * reading shows a key that is not already a name:
 *
 * - **A formula key is its own name.** A definite species is written as chemists write its
 *   formula, its digits on the line and its case kept, in an upper-case label too (the guide's
 *   Typography): `H2O`, `CO2`, `Mg2SiO4`. An ion's sign stands on the line, a minus as the minus
 *   sign (`Fe+`, `H−`), and the electron is `e−`. So every formula key has a name, a newer
 *   server's included, and no table holds one.
 * - **A name key takes its name from {@link SUBSTANCE_NAMES}**: a material with no single formula,
 *   named in words as the literature names it (`tholin`, `Mars-type dust`).
 * - **Any other string**, a newer server's name key or one outside the key grammar, reads
 *   {@link UNLISTED_MATERIAL}. The key itself is never shown.
 *
 * A name carries neither the phase, which a reading states beside it (`liquid CH4`, `CO2 ice`),
 * nor a fraction, which is a quantity named in words (`mass fraction 75 %`, a solution's fraction
 * being its solute's; signoff-5). The names are the client's, not the registry's: a name can
 * change without moving the registry's golden or its append-only fixture, and the generator never
 * reads one, so no name moves generated output.
 */

/**
 * A formula key, by `SubstanceKey`'s grammar (`crates/hyperion-surface/src/substance_key.rs`):
 * element symbols, each with no count or a count of 2 or more without a leading zero, then at
 * most one sign.
 */
const FORMULA_KEY = /^(?:[A-Z][a-z]?(?:[2-9]|[1-9][0-9]+)?)+[+-]?$/u;

/** A chemical formula among a name's words, which keeps its case in upper case (`Mg(ClO4)2`). */
const FORMULA_WORD = /^(?:[A-Z][a-z]?[0-9]*|\((?:[A-Z][a-z]?[0-9]*)+\)[0-9]*)+$/u;

/** What a reading shows for a substance the console's list does not name. */
export const UNLISTED_MATERIAL = "unlisted material";

/**
 * The names of the registry's name keys, in words, in sentence case, of three words or fewer.
 *
 * @remarks
 * Every name key in the registry's fixture (`packages/protocol/fixtures/substances.json`) and in
 * the client's own tables has a name here, and a task that appends a name-form row to the
 * registry adds its name in the same commit (P14.T49.f). No two keys share a name, and no name is
 * written as a formula key would be.
 */
export const SUBSTANCE_NAMES: Readonly<Record<string, string>> = {
  // P14.T49.b's name-form rows, in registry order (decision-substance-names §4.3; `basalt_fe` as
  // signoff-5's item 8 amends it).
  graphite: "graphite",
  tholin: "tholin",
  soot: "soot",
  chromophore: "Jovian chromophore",
  ice_giant_haze: "ice-giant haze",
  mars_dust: "Mars-type dust",
  acid_uv_absorber: "ultraviolet-absorbing H2SO4",
  brine_nacl: "NaCl brine",
  brine_mgso4: "MgSO4 brine",
  brine_naclo4: "NaClO4 brine",
  brine_mgclo4: "Mg(ClO4)2 brine",
  nh3_h2o: "NH3–H2O mixture",
  ch4_c2h6_n2: "CH4–C2H6–N2 mixture",
  anorthosite: "anorthosite",
  feldspathic: "feldspathic rock",
  basalt: "basalt",
  basalt_fe: "Fe-rich basalt",
  komatiite: "komatiite",
  basalt_reduced: "reduced basalt",
  granite: "granite",
  ultramafic: "ultramafic rock",
  ferric: "ferric rock",
  phyllosilicate: "phyllosilicate",
  lava_glass: "volcanic glass",
  ca_al_residue: "refractory Ca–Al residue",
  iron_nickel: "Fe–Ni metal",
  carbonaceous: "carbonaceous matter",
  chondrite_ci: "CI chondrite",
  chondrite_cm: "CM chondrite",
  chondrite_cv: "CV chondrite",
  chondrite_h: "H chondrite",
  chondrite_l: "L chondrite",
  chondrite_ll: "LL chondrite",
  chondrite_e: "E chondrite",
  cometary: "cometary dust",
};

/** The case a name is shown in: as written, inside a sentence, or in an upper-case label. */
export type NameCase = "sentence" | "upper";

/**
 * A substance's name, from its registry key.
 *
 * @param key - The registry key, as the wire carries it; any string.
 * @param nameCase - `upper` for a label: a name's words are upper-cased, and a formula keeps its
 *   case.
 */
export function substanceName(key: string, nameCase: NameCase = "sentence"): string {
  if (key === "e-") {
    return "e−";
  }
  if (FORMULA_KEY.test(key)) {
    return key.endsWith("-") ? `${key.slice(0, -1)}−` : key;
  }
  const words =
    (Object.hasOwn(SUBSTANCE_NAMES, key) ? SUBSTANCE_NAMES[key] : undefined) ?? UNLISTED_MATERIAL;
  return nameCase === "upper" ? upperCaseWords(words) : words;
}

/**
 * A name's words in upper case, each formula among them kept: `NaCl BRINE`,
 * `REFRACTORY Ca–Al RESIDUE`.
 */
function upperCaseWords(words: string): string {
  return words
    .split(/([ –-])/u)
    .map((part) => (FORMULA_WORD.test(part) ? part : part.toUpperCase()))
    .join("");
}
