/**
 * The substance registry's two optics flags, as the client reads them at run time (plan R08,
 * R08.T10.a; `decision-r08-t10a-leans.md` item 7): each row's `absorber` and `rayleigh` of
 * `packages/protocol/fixtures/substances.json` (galaxy plan 14, P14.T49.a and P14.T49.b), which the
 * client imports only in tests.
 *
 * @remarks
 * `parity.test.ts` (R08.T19) holds the table to the fixture, every row both ways. A task that
 * appends a row or changes a flag updates the table in the same commit, as `SUBSTANCE_NAMES`'
 * rule has it (P14.T49.f). A key the table lacks is a newer server's, and takes the unknown-key
 * path (`atmosphereApproximate`). The module has no imports, so that any thread can read it.
 */

/** A registry row's optics flags. */
export interface RegistryOpticsFlags {
  /** The registry asks the client for the species' visible absorption (its `absorber`). */
  readonly absorber: boolean;
  /** The registry asks the client for the species' Rayleigh scattering (its `rayleigh`). */
  readonly rayleigh: boolean;
}

/** The fixture's rows at registry version 23, in index order (115 rows; 54 `absorber`, 43 `rayleigh`). */
const ROWS = {
  H2: { absorber: true, rayleigh: true },
  He: { absorber: true, rayleigh: true },
  H2O: { absorber: true, rayleigh: true },
  CH4: { absorber: true, rayleigh: true },
  NH3: { absorber: true, rayleigh: true },
  N2: { absorber: true, rayleigh: true },
  O2: { absorber: true, rayleigh: true },
  CO2: { absorber: true, rayleigh: true },
  Ar: { absorber: false, rayleigh: true },
  Ne: { absorber: false, rayleigh: true },
  Kr: { absorber: false, rayleigh: true },
  Xe: { absorber: false, rayleigh: true },
  N2O: { absorber: true, rayleigh: true },
  CO: { absorber: true, rayleigh: true },
  SO2: { absorber: true, rayleigh: true },
  H2S: { absorber: true, rayleigh: true },
  O3: { absorber: true, rayleigh: true },
  HCN: { absorber: true, rayleigh: true },
  C2H2: { absorber: true, rayleigh: true },
  C2H4: { absorber: true, rayleigh: true },
  C2H6: { absorber: true, rayleigh: true },
  PH3: { absorber: true, rayleigh: true },
  CH3OH: { absorber: true, rayleigh: true },
  H: { absorber: true, rayleigh: true },
  O: { absorber: true, rayleigh: true },
  "e-": { absorber: true, rayleigh: false },
  "H-": { absorber: true, rayleigh: false },
  Na: { absorber: true, rayleigh: true },
  K: { absorber: true, rayleigh: true },
  Fe: { absorber: true, rayleigh: true },
  Mg: { absorber: true, rayleigh: true },
  Si: { absorber: true, rayleigh: true },
  Ca: { absorber: true, rayleigh: true },
  Ti: { absorber: true, rayleigh: true },
  SiO: { absorber: true, rayleigh: true },
  TiO: { absorber: true, rayleigh: true },
  VO: { absorber: true, rayleigh: true },
  FeH: { absorber: true, rayleigh: true },
  Al: { absorber: true, rayleigh: true },
  V: { absorber: true, rayleigh: true },
  Mn: { absorber: true, rayleigh: true },
  Ni: { absorber: true, rayleigh: true },
  Zn: { absorber: true, rayleigh: true },
  "Na+": { absorber: true, rayleigh: false },
  "Mg+": { absorber: true, rayleigh: false },
  "Al+": { absorber: true, rayleigh: false },
  "Si+": { absorber: true, rayleigh: false },
  "K+": { absorber: true, rayleigh: false },
  "Ca+": { absorber: true, rayleigh: false },
  "Ti+": { absorber: true, rayleigh: false },
  "V+": { absorber: true, rayleigh: false },
  "Cr+": { absorber: true, rayleigh: false },
  "Mn+": { absorber: true, rayleigh: false },
  "Fe+": { absorber: true, rayleigh: false },
  "Ni+": { absorber: true, rayleigh: false },
  "Zn+": { absorber: true, rayleigh: false },
  HCl: { absorber: true, rayleigh: true },
  NH4SH: { absorber: false, rayleigh: false },
  H2SO4: { absorber: false, rayleigh: false },
  S8: { absorber: false, rayleigh: false },
  KCl: { absorber: false, rayleigh: false },
  NaCl: { absorber: false, rayleigh: false },
  ZnS: { absorber: false, rayleigh: false },
  Na2S: { absorber: false, rayleigh: false },
  MnS: { absorber: false, rayleigh: false },
  Cr: { absorber: true, rayleigh: true },
  FeS: { absorber: false, rayleigh: false },
  MgSiO3: { absorber: false, rayleigh: false },
  Mg2SiO4: { absorber: false, rayleigh: false },
  SiO2: { absorber: false, rayleigh: false },
  Al2O3: { absorber: false, rayleigh: false },
  CaTiO3: { absorber: false, rayleigh: false },
  TiO2: { absorber: false, rayleigh: false },
  graphite: { absorber: false, rayleigh: false },
  tholin: { absorber: false, rayleigh: false },
  soot: { absorber: false, rayleigh: false },
  chromophore: { absorber: false, rayleigh: false },
  ice_giant_haze: { absorber: false, rayleigh: false },
  mars_dust: { absorber: false, rayleigh: false },
  acid_uv_absorber: { absorber: false, rayleigh: false },
  brine_nacl: { absorber: false, rayleigh: false },
  brine_mgso4: { absorber: false, rayleigh: false },
  brine_naclo4: { absorber: false, rayleigh: false },
  brine_mgclo4: { absorber: false, rayleigh: false },
  nh3_h2o: { absorber: false, rayleigh: false },
  ch4_c2h6_n2: { absorber: false, rayleigh: false },
  anorthosite: { absorber: false, rayleigh: false },
  feldspathic: { absorber: false, rayleigh: false },
  basalt: { absorber: false, rayleigh: false },
  basalt_fe: { absorber: false, rayleigh: false },
  komatiite: { absorber: false, rayleigh: false },
  basalt_reduced: { absorber: false, rayleigh: false },
  granite: { absorber: false, rayleigh: false },
  ultramafic: { absorber: false, rayleigh: false },
  ferric: { absorber: false, rayleigh: false },
  phyllosilicate: { absorber: false, rayleigh: false },
  lava_glass: { absorber: false, rayleigh: false },
  ca_al_residue: { absorber: false, rayleigh: false },
  iron_nickel: { absorber: false, rayleigh: false },
  SiC: { absorber: false, rayleigh: false },
  MgSO4: { absorber: false, rayleigh: false },
  Na2CO3: { absorber: false, rayleigh: false },
  CaCO3: { absorber: false, rayleigh: false },
  carbonaceous: { absorber: false, rayleigh: false },
  chondrite_ci: { absorber: false, rayleigh: false },
  chondrite_cm: { absorber: false, rayleigh: false },
  chondrite_cv: { absorber: false, rayleigh: false },
  chondrite_h: { absorber: false, rayleigh: false },
  chondrite_l: { absorber: false, rayleigh: false },
  chondrite_ll: { absorber: false, rayleigh: false },
  chondrite_e: { absorber: false, rayleigh: false },
  cometary: { absorber: false, rayleigh: false },
  S: { absorber: false, rayleigh: false },
  N: { absorber: false, rayleigh: false },
  C: { absorber: false, rayleigh: false },
} satisfies Readonly<Record<string, RegistryOpticsFlags>>;

/**
 * Each registry key's optics flags, in the fixture's order; a key absent here is a newer
 * server's.
 */
export const REGISTRY_OPTICS_FLAGS: ReadonlyMap<string, RegistryOpticsFlags> = new Map(
  Object.entries(ROWS),
);
