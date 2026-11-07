//! The snapshot path's closed form of a gas envelope's column (P14.T24.h.b part 5): what the
//! passes read of an envelope, at about a tenth of a millisecond, where P14.T52.d's own derivation
//! costs tens of milliseconds.
//!
//! - **The body**: its gravity G M ÷ R², its `T_eq` the pass's, and P14.T24.d's internal
//!   temperature ([`envelope_internal_temperature`]) of the internal heat its heated temperature
//!   carries, as the record's envelope takes them.
//! - **The gas**: P14.T50.b's abundances ([`EnvelopeAbundances::derive`]) about a host of solar
//!   metallicity at its trends' medians, formed where Jupiter did against the snow line beyond it
//!   (`T` 113 K) and at half the snow line's distance inside it (240 K): the snapshot carries no
//!   host abundances or formation distance (labelled). Carbon and nitrogen are speciated at 1 bar
//!   in equilibrium, CO + 3 H₂ ⇌ CH₄ + H₂O and N₂ + 3 H₂ ⇌ 2 NH₃ (P14.T49.d's constants), not
//!   quenched as P14.T52.b's are (labelled); oxygen's silicate share, three atoms to each silicon,
//!   is taken from the water; every other element is its atom's gas, sulphur H₂S and chlorine HCl.
//! - **The profile**: Guillot's grey profile of P14.T24.d
//!   ([`GreyProfile::at_effective_temperature`], [`GreyProfile::ADOPTED`] but κ₁) to its join,
//!   where its gradient reaches the dry adiabat's (by [`JOIN_BISECTIONS`] bisections in ln p), and
//!   below it the adiabat stepped in ln p by
//!   [`ADIABAT_STEP_LN_P`], `c_p` of the gas's H₂, He, CH₄, H₂O, NH₃ and CO at each step's
//!   midpoint, its temperature the power law between steps.
//! - **The decks** ([`CANDIDATES`], in their condensation order): each row's law aloft, its base
//!   the deepest level above 100 bar at which its vapour saturates on the profile (bisected in ln
//!   p), each later deck read in the gas the earlier leave (a deck's limiting vapour gone above it,
//!   its other vapours by their counts). A vapour already saturated at 100 bar is trapped there
//!   and draws no deck in the column: the deep decks' tails above the floor are not read
//!   (labelled; on the table's Jupiter, 2.4 of its 10). Each deck's column is Ackerman and
//!   Marley's balance at the saturation slope s of the scan step above its base (P14.T52.c's), M =
//!   ε x `p_b` s ÷ (g (s + 1)(`f_sed` + 1)), the loop's step solved for a constant s; its
//!   `f_sed` is its kind's (a volatile's by its supply, [`sedimentation_for_supply`](crate::planetary::hooks::aerosols::sedimentation_for_supply)'s law); its
//!   profile's exponent the one in force (P14.T52.g); its top where its condensate falls to
//!   10⁻⁶ of its vapour; its sizes Ackerman and Marley's eq. 13 at its base alone
//!   ([`settling_size_at`]); its τ(550) P14.T49.e's ([`Mode::new`], [`AerosolMode::of_settling`]),
//!   cut on a hot dayside as P14.T52.h cuts it.
//! - **The hazes**: P14.T52.b's rules on that gas, as P14.T52.d reads them: the S₈ haze, soot
//!   (by [`soot_weight`]), the ice giants' haze on a CH₄ deck and the chromophore on an NH₃ deck
//!   with no CH₄ deck above it, and that cloud's NH₃-ice haze ([`ammonia_haze`], part 6), as the
//!   record draws them from 27, each by the rules in force (the chromophore's column by its haze's
//!   gas from 31, held: `photochem::CHROMOPHORE_MIXING_VERSION`).
//! - **The absorbers**: each layer's CH₄, H₂O, Na and K, held above its deck's base (or 100 bar's,
//!   for a trapped one) to its saturation there.

use crate::math;
use crate::planetary::chem::condense::{
    Mode, ModeProfile, SCAN_STEP_LN_P, Sedimentation, TOP_FRACTION, deck_phase, settling_size_at,
};
use crate::planetary::chem::envelope_abundances::{EnvelopeAbundances, EnvelopeInputs};
use crate::planetary::chem::photochem::{
    UpperAtmosphere, ammonia_haze, chromophore, ice_giant_haze, soot, sulphur_haze,
};
use crate::planetary::chem::quench::MixingColumn;
use crate::planetary::chem::{AbundanceDraws, HostAbundances};
use crate::planetary::derive::atmosphere::{AtmosphereInputs, Gas as Species};
use crate::planetary::derive::composition::{MassFractions, SnowLineSide};
use crate::planetary::derive::photometry::area_mix::HostShade;
use crate::planetary::derive::radius::CoreComposition;
use crate::planetary::hooks::aerosols::{
    AerosolMode, CLOUD_EDDY_DIFFUSION, DECK_WIDTH, LEAST_SEDIMENTATION_EFFICIENCY, ParticleSize,
    SEDIMENTATION_SPEED_AT_UNIT_SUPPLY, SEDIMENTATION_SUPPLY_EXPONENT,
};
use crate::planetary::hooks::envelope_atmosphere::{
    DAYSIDE_LEVEL, DaysideDecksRules, DeckKind, HOT_DAYSIDE_TEMPERATURE, haze_mode_at,
    methane_layer_mode, soot_weight,
};
use crate::planetary::hooks::envelope_internal_temperature;
use crate::planetary::hooks::structure::envelope::{
    ADIABAT_BOTTOM, GreyProfile, irradiation_temperature,
};
use crate::substance::aloft::{LawAloft, SaturationLaw, Vapour, law_aloft};
use crate::substance::formation::Reaction;
use crate::substance::{
    Element, MOLAR_GAS_CONSTANT, SubstanceId, ids, mixture_heat_capacity_over_r_at,
};
use crate::units::consts::{GRAVITATIONAL_CONSTANT, STEFAN_BOLTZMANN};
use crate::units::{
    Dex, EarthMasses, Kelvin, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, Pascals,
    Watts,
};

use super::{
    Absorbers, COLUMN_TOP_PA, ColumnMode, EnvelopeColumn, assemble, column_bond, particle_table,
};

/// The rows whose decks the closed form draws, in their condensation order on a solar gas
/// (Lodders and Fegley 2006's sequence, as P14.T52.d's module documentation quotes it): the
/// refractories, the sulphides and salts, then the volatiles. A fixed order stands for the loop's
/// hottest-base-first (labelled).
pub(crate) const CANDIDATES: [&str; 15] = [
    "Al2O3", "CaTiO3", "Fe", "Mg2SiO4", "MgSiO3", "Cr", "MnS", "Na2S", "ZnS", "KCl", "H2O",
    "NH4SH", "NH3", "H2S", "CH4",
];

/// The bisections of the join and of each deck's base in ln p: 40, to 10⁻¹¹ of a step.
const JOIN_BISECTIONS: u32 = 40;

/// The largest share of an envelope's hydrogen its hydrides (CH₄, NH₃, H₂O and H₂S) may take for
/// the closed form to read it: ½, P14.T52.a's bound on a hydrogen-dominated envelope.
const HYDROGEN_DOMINANCE: f64 = 0.5;

/// The decks' upward scan for their base, in ln p: 1.
const DECK_SCAN_STEP_LN_P: f64 = 1.0;

/// The adiabat's step in ln p: 0.5.
const ADIABAT_STEP_LN_P: f64 = 0.5;

/// The thermal optical depth from which the join is sought: 10⁻⁶ (P14.T24.d's scan top).
const JOIN_TOP_OPTICAL_DEPTH: f64 = 1e-6;

/// The formation temperature of an envelope formed beyond the snow line, K: Jupiter's, at 2.26 of
/// the snow line's distance (5.2 au against 2.3; P14.T50.b's table case); and of one formed inside
/// it, at half its distance.
const FORMED_AT: [(SnowLineSide, f64); 2] =
    [(SnowLineSide::Beyond, 2.26), (SnowLineSide::Inside, 0.5)];

/// A vapour's amount and, once a deck traps it, the law and the base above which its saturation
/// holds it.
#[derive(Debug, Clone, Copy)]
struct Held {
    /// Its mole fraction below the trap.
    deep: f64,
    /// The trap: its law, its base (Pa), and for a product law the other vapour's mole fraction.
    trap: Option<(LawAloft, f64, f64)>,
}

impl Held {
    /// A free vapour of mole fraction `x`.
    const fn free(x: f64) -> Self {
        Self {
            deep: x,
            trap: None,
        }
    }

    /// Its mole fraction at `p` (Pa) and `t` (K): below its trap the deep one, above it at most
    /// its saturation there.
    fn at(&self, p: f64, t: f64, metallicity_dex: f64) -> f64 {
        match self.trap {
            Some((law, base, other)) if p < base => {
                let saturated = ln_saturated_fraction(&law, p, t, other, metallicity_dex)
                    .map_or(f64::INFINITY, math::exp);
                self.deep.min(saturated)
            }
            _ => self.deep,
        }
    }
}

/// ln of the limiting vapour's saturated mole fraction under `law` at `p` (Pa) and `t` (K) in a
/// gas of metallicity `metallicity_dex`: `p_sat` ÷ p, or for a product law K ÷ (x₂ p²) with
/// `other` the other vapour's mole fraction; `None` where it does not condense.
fn ln_saturated_fraction(
    law: &LawAloft,
    p: f64,
    t: f64,
    other: f64,
    metallicity_dex: f64,
) -> Option<f64> {
    let ln = law.ln_saturation_pressure(Kelvin::new(t), Pascals::new(p), metallicity_dex)?;
    match law.law() {
        SaturationLaw::Product { .. } => {
            (other > 0.0).then(|| ln - math::ln(other) - 2.0 * math::ln(p))
        }
        SaturationLaw::Vapour { .. } | SaturationLaw::Diagram | SaturationLaw::KulmalaLaaksonen => {
            Some(ln - math::ln(p))
        }
    }
}

/// The envelope's profile in closed form (the [module](self)'s documentation).
#[derive(Debug, Clone)]
struct Profile {
    grey: GreyProfile,
    gravity: MetresPerSecondSquared,
    internal: Kelvin,
    irradiation: Kelvin,
    /// The join's pressure, Pa.
    join: f64,
    /// The adiabat's nodes from the join down, (ln p, ln T).
    nodes: Vec<(f64, f64)>,
}

impl Profile {
    /// The grey profile's temperature at `p` (Pa).
    fn grey_at(&self, p: f64) -> f64 {
        let tau = self.grey.optical_depth(Pascals::new(p), self.gravity);
        self.grey
            .temperature(tau, self.internal, self.irradiation)
            .value()
    }

    /// The temperature at `p` (Pa): the grey profile's above the join, the adiabat's nodes' power
    /// law below it, and the last segment's beyond the last node.
    fn at(&self, p: f64) -> f64 {
        if p < self.join {
            return self.grey_at(p);
        }
        let x = math::ln(p);
        let last = self.nodes.len() - 1;
        let above = self
            .nodes
            .partition_point(|&(lp, _)| lp <= x)
            .clamp(1, last);
        let ((p0, t0), (p1, t1)) = (self.nodes[above - 1], self.nodes[above]);
        math::exp(t0 + (t1 - t0) * (x - p0) / (p1 - p0))
    }

    /// The thermal photosphere's pressure, Pa, where τ = ⅔.
    fn photosphere(&self) -> f64 {
        self.grey.pressure_at_depth(2.0 / 3.0, self.gravity).value()
    }

    /// The skin's temperature, K.
    fn skin(&self) -> Kelvin {
        self.grey.temperature(0.0, self.internal, self.irradiation)
    }
}

/// The temperatures at which [`HeatCapacity`] evaluates the mixture's `c_p`, K: ratios of 1.5.
const HEAT_CAPACITY_NODES_K: [f64; 14] = [
    40.0,
    60.0,
    90.0,
    135.0,
    202.5,
    303.75,
    455.625,
    683.4375,
    1_025.156_25,
    1_537.734_375,
    2_306.601_562_5,
    3_459.902_343_75,
    5_189.853_515_625,
    7_784.780_273_437_5,
];

/// A mixture's `c_p` ÷ R by temperature, from its statistical sums at [`HEAT_CAPACITY_NODES_K`]
/// alone, each evaluated when first read, linear in ln T between them and held beyond them (a
/// stated approximation of the closed form's adiabat: each node costs a statistical sum, some
/// 3 µs).
#[derive(Debug)]
struct HeatCapacity<'a> {
    mixture: &'a [(SubstanceId, f64)],
    nodes: [Option<f64>; 14],
}

impl<'a> HeatCapacity<'a> {
    /// The mixture `mixture`'s, none evaluated yet.
    fn of(mixture: &'a [(SubstanceId, f64)]) -> Self {
        Self {
            mixture,
            nodes: [None; 14],
        }
    }

    /// Node `i`'s `c_p` ÷ R; `None` where a row has no heat capacity.
    fn node(&mut self, i: usize) -> Option<f64> {
        if let Some(value) = self.nodes[i] {
            return Some(value);
        }
        let value =
            mixture_heat_capacity_over_r_at(self.mixture, Kelvin::new(HEAT_CAPACITY_NODES_K[i]))?;
        self.nodes[i] = Some(value);
        Some(value)
    }

    /// `c_p` ÷ R at `t` K.
    fn at(&mut self, t: f64) -> Option<f64> {
        let last = HEAT_CAPACITY_NODES_K.len() - 1;
        if t.is_nan() || t <= HEAT_CAPACITY_NODES_K[0] {
            return self.node(0);
        }
        if t >= HEAT_CAPACITY_NODES_K[last] {
            return self.node(last);
        }
        let above = HEAT_CAPACITY_NODES_K
            .partition_point(|&node| node <= t)
            .clamp(1, last);
        let (a, b) = (
            HEAT_CAPACITY_NODES_K[above - 1],
            HEAT_CAPACITY_NODES_K[above],
        );
        let f = math::ln(t / a) / math::ln(b / a);
        Some(self.node(above - 1)? * (1.0 - f) + self.node(above)? * f)
    }
}

/// The profile of an envelope of internal temperature `internal` under the irradiation of `T_eq`
/// `equilibrium` and gravity `gravity`, whose adiabat's `c_p` is `mixture`'s; `None` where no
/// join forms or a `c_p` is not known.
fn profile(
    (grey, internal, equilibrium): (GreyProfile, Kelvin, Kelvin),
    gravity: MetresPerSecondSquared,
    mixture: &[(SubstanceId, f64)],
) -> Option<Profile> {
    let irradiation = irradiation_temperature(equilibrium);
    let mut profile = Profile {
        grey,
        gravity,
        internal,
        irradiation,
        join: 0.0,
        nodes: Vec::new(),
    };
    let mut heat = HeatCapacity::of(mixture);
    let mut gradient = |p: f64| {
        let tau = grey.optical_depth(Pascals::new(p), gravity);
        let t = profile.grey_at(p);
        heat.at(t)
            .map(|c| grey.radiative_gradient(tau, internal, irradiation) - 1.0 / c)
    };
    let top = grey
        .pressure_at_depth(JOIN_TOP_OPTICAL_DEPTH, gravity)
        .value();
    let bracket = grey.join_bracket_optical_depth(internal, irradiation);
    let bottom = grey.pressure_at_depth(bracket, gravity).value().min(1.0e11);
    let (mut low, mut high) = (math::ln(top), math::ln(bottom));
    if !(gradient(top)? < 0.0 && gradient(bottom)? >= 0.0) {
        return None;
    }
    for _ in 0..JOIN_BISECTIONS {
        let middle = f64::midpoint(low, high);
        if gradient(math::exp(middle))? < 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }
    let join = math::exp(high);
    profile.join = join;
    let mut x = math::ln(join);
    let mut ln_t = math::ln(profile.grey_at(join));
    profile.nodes.push((x, ln_t));
    let deepest = math::ln(ADIABAT_BOTTOM.value()).max(x + ADIABAT_STEP_LN_P);
    while x < deepest {
        let step = ADIABAT_STEP_LN_P.min(deepest - x);
        let half = heat.at(math::exp(ln_t))?;
        let middle = ln_t + 0.5 * step / half;
        let whole = heat.at(math::exp(middle))?;
        ln_t += step / whole;
        x += step;
        profile.nodes.push((x, ln_t));
    }
    Some(profile)
}

/// The smaller root of y² − `sum` y + `product` = 0, in the form that keeps its precision when
/// `product` is small against `sum`².
fn smaller_root(sum: f64, product: f64) -> f64 {
    let discriminant = (sum * sum - 4.0 * product).max(0.0);
    2.0 * product / (sum + discriminant.sqrt())
}

/// The gas's vapours as the decks read them: each species and element the closed form tracks.
#[derive(Debug, Clone, Copy)]
struct Gas {
    /// Mole fractions of H₂, He, CH₄, CO, H₂O, NH₃, H₂S, HCl.
    hydrogen: f64,
    helium: f64,
    methane: Held,
    carbon_monoxide: f64,
    water: Held,
    ammonia: f64,
    sulphide: f64,
    chloride: f64,
    /// Each element's atoms' mole fraction, by [`Element::ALL`]'s order.
    atoms: [f64; Element::ALL.len()],
    sodium: Held,
    potassium: Held,
    /// The mean molar mass, g mol⁻¹.
    molar_mass: f64,
    /// The metallicity, dex.
    metallicity_dex: f64,
}

/// The index of `element` in [`Element::ALL`].
fn index(element: Element) -> usize {
    Element::ALL
        .iter()
        .position(|&e| e == element)
        .expect("every element is in ALL")
}

/// log₁₀ K of the balanced reaction `species` at `t`, `None` outside its records.
fn log10_constant(species: &[(SubstanceId, i32)], t: f64) -> Option<f64> {
    Reaction::new(species)
        .ok()?
        .log10_equilibrium_constant(Kelvin::new(t))
}

/// The registry rows the closed form reads, found in one scan of the registry: the candidates'
/// (in [`CANDIDATES`]' order) and CO's.
#[derive(Debug, Clone, Copy)]
struct Rows {
    candidates: [Option<SubstanceId>; CANDIDATES.len()],
    carbon_monoxide: Option<SubstanceId>,
}

impl Rows {
    /// The registry's.
    fn find() -> Self {
        let mut rows = Self {
            candidates: [None; CANDIDATES.len()],
            carbon_monoxide: None,
        };
        for id in ids() {
            let key = id.substance().key();
            let key = key.as_str();
            if key == "CO" {
                rows.carbon_monoxide = Some(id);
            } else if let Some(i) = CANDIDATES.iter().position(|&c| c == key) {
                rows.candidates[i] = Some(id);
            }
        }
        rows
    }
}

/// The gas of `abundances` at 1 bar and `t` K (the [module](self)'s documentation).
fn gas_of(abundances: &EnvelopeAbundances, temperature: f64, rows: &Rows) -> Option<Gas> {
    let per_h = |element: Element| abundances.per_hydrogen(element);
    // An envelope whose hydrides would take more than half its hydrogen is not modelled, as
    // P14.T52.a's speciation refuses it.
    let hydrides = 4.0 * per_h(Element::C)
        + 3.0 * per_h(Element::N)
        + 2.0 * per_h(Element::O)
        + 2.0 * per_h(Element::S);
    if hydrides.is_nan() || hydrides > HYDROGEN_DOMINANCE {
        return None;
    }
    let helium = per_h(Element::He);
    let carbon = per_h(Element::C);
    let nitrogen = per_h(Element::N);
    let oxygen = per_h(Element::O) - 3.0 * per_h(Element::Si);
    let oxygen = if oxygen > 0.0 { oxygen } else { 0.0 };
    // Per H atom: the molecules, H₂ ½ and each heavy atom's carrier, then the shares by number.
    let mut heavy = 0.0;
    for element in Element::ALL {
        if !matches!(
            element,
            Element::H | Element::He | Element::C | Element::N | Element::O
        ) {
            heavy += per_h(element);
        }
    }
    // The equilibria at 1 bar in mole fractions of a gas of ½ H₂ per H (the carriers' own share
    // of the total held at its reduced count, a stated simplification).
    let rough = 0.5 + helium + carbon + nitrogen + oxygen + heavy;
    let x_h2 = 0.5 / rough;
    let (x_c, x_o, x_n) = (carbon / rough, oxygen / rough, nitrogen / rough);
    let (co, nh3) = if temperature >= 300.0 {
        // CO + 3 H₂ ⇌ CH₄ + H₂O: (C − y)(O − y) = A y with A = K x_H2³ P², P 1 bar.
        let carbon_monoxide = rows.carbon_monoxide?;
        let (h2, ch4, h2o) = (
            Species::Hydrogen.substance(),
            Species::Methane.substance(),
            Species::Water.substance(),
        );
        let log_k = log10_constant(
            &[(carbon_monoxide, -1), (h2, -3), (ch4, 1), (h2o, 1)],
            temperature,
        )?;
        let shift = math::exp10(log_k) * x_h2 * x_h2 * x_h2;
        let oxidised = smaller_root(x_c + x_o + shift, x_c * x_o);
        // N₂ + 3 H₂ ⇌ 2 NH₃: z² = B (N − z) ÷ 2 with B = K x_H2³ P².
        let (n2, nh3) = (Species::Nitrogen.substance(), Species::Ammonia.substance());
        let log_k = log10_constant(&[(n2, -1), (h2, -3), (nh3, 2)], temperature)?;
        let half = 0.5 * math::exp10(log_k) * x_h2 * x_h2 * x_h2;
        let reduced = 2.0 * half * x_n / (half + (half * half + 4.0 * half * x_n).sqrt());
        (oxidised, reduced)
    } else {
        (0.0, x_n)
    };
    let methane = x_c - co;
    let water = x_o - co;
    let nitrogen_molecules = 0.5 * (x_n - nh3);
    let total = x_h2
        + helium / rough
        + methane
        + co
        + water.max(0.0)
        + nh3
        + nitrogen_molecules
        + heavy / rough;
    let scale = 1.0 / total;
    let mut atoms = [0.0; Element::ALL.len()];
    for (slot, element) in atoms.iter_mut().zip(Element::ALL) {
        *slot = per_h(element) / rough * scale;
    }
    let weight = |element: Element| element.standard_atomic_weight_g_per_mol();
    let mut mass = weight(Element::H) + helium * weight(Element::He);
    for element in Element::ALL {
        if !matches!(element, Element::H | Element::He) {
            mass += per_h(element) * weight(element);
        }
    }
    Some(Gas {
        hydrogen: x_h2 * scale,
        helium: helium / rough * scale,
        methane: Held::free(methane * scale),
        carbon_monoxide: co * scale,
        water: Held::free(water.max(0.0) * scale),
        ammonia: nh3 * scale,
        sulphide: atoms[index(Element::S)],
        chloride: atoms[index(Element::Cl)],
        atoms,
        sodium: Held::free(atoms[index(Element::Na)]),
        potassium: Held::free(atoms[index(Element::K)]),
        molar_mass: mass / (rough * total),
        metallicity_dex: math::log10(abundances.enrichment()),
    })
}

impl Gas {
    /// The deep mole fraction of the vapour `vapour`.
    fn vapour(&self, vapour: Vapour) -> f64 {
        match vapour {
            Vapour::Element(e) => match e {
                Element::Na => self.sodium.deep,
                Element::K => self.potassium.deep,
                Element::Cl => self.chloride,
                _ => self.atoms[index(e)],
            },
            Vapour::Species(key) => match key.as_str() {
                "CH4" => self.methane.deep,
                "H2O" => self.water.deep,
                "NH3" => self.ammonia,
                "H2S" => self.sulphide,
                "HCl" => self.chloride,
                _ => 0.0,
            },
        }
    }

    /// Takes `amount` of the vapour `vapour` from the gas, held at 0.
    fn take(&mut self, vapour: Vapour, amount: f64) {
        let less = |x: &mut f64| *x = (*x - amount).max(0.0);
        match vapour {
            Vapour::Element(e) => match e {
                Element::Na => less(&mut self.sodium.deep),
                Element::K => less(&mut self.potassium.deep),
                Element::Cl => less(&mut self.chloride),
                Element::S => less(&mut self.sulphide),
                _ => less(&mut self.atoms[index(e)]),
            },
            Vapour::Species(key) => match key.as_str() {
                "CH4" => less(&mut self.methane.deep),
                "H2O" => less(&mut self.water.deep),
                "NH3" => less(&mut self.ammonia),
                "H2S" => less(&mut self.sulphide),
                "HCl" => less(&mut self.chloride),
                _ => {}
            },
        }
    }

    /// A deck's limiting vapour `vapour`, of mole fraction `x` below its base: an absorber's
    /// (CH₄, H₂O, Na, K) kept below it and held to its saturation under the trap `trap` (its law,
    /// its base, Pa, and its other vapour's mole fraction) above it; any other's taken from the gas
    /// the later decks read.
    fn trap_or_take(&mut self, vapour: Vapour, (law, base, other): (LawAloft, f64, f64), x: f64) {
        let held = match vapour {
            Vapour::Element(Element::Na) => Some(&mut self.sodium),
            Vapour::Element(Element::K) => Some(&mut self.potassium),
            Vapour::Species(key) if key.as_str() == "CH4" => Some(&mut self.methane),
            Vapour::Species(key) if key.as_str() == "H2O" => Some(&mut self.water),
            Vapour::Element(_) | Vapour::Species(_) => None,
        };
        match held {
            Some(held) => {
                if held.trap.is_none() {
                    held.trap = Some((law, base, other));
                }
            }
            None => self.take(vapour, x),
        }
    }

    /// The adiabat's mixture: H₂, He, CH₄, H₂O, NH₃ and CO (`carbon_monoxide`, its row),
    /// renormalised.
    fn mixture(&self, carbon_monoxide: Option<SubstanceId>) -> Vec<(SubstanceId, f64)> {
        let parts = [
            (Some(Species::Hydrogen.substance()), self.hydrogen),
            (Some(Species::Helium.substance()), self.helium),
            (Some(Species::Methane.substance()), self.methane.deep),
            (Some(Species::Water.substance()), self.water.deep),
            (Some(Species::Ammonia.substance()), self.ammonia),
            (carbon_monoxide, self.carbon_monoxide),
        ];
        let total: f64 = parts.iter().map(|&(_, x)| x).sum();
        let mut out = Vec::with_capacity(parts.len());
        for (id, x) in parts {
            if let Some(id) = id
                && x > 0.0
            {
                out.push((id, x / total));
            }
        }
        out
    }
}

/// The column of the gas envelope `inputs` describes (the [module](self)'s closed form); `None`
/// where a piece refuses its inputs (no internal heat, no join, abundances refused), for the class
/// table to stand.
#[must_use]
pub(crate) fn snapshot_column(inputs: &AtmosphereInputs) -> Option<EnvelopeColumn> {
    let (mass, radius) = (inputs.mass.value(), inputs.radius.value());
    let gravity = GRAVITATIONAL_CONSTANT * mass / (radius * radius);
    let equilibrium = inputs.equilibrium.value();
    let heated = inputs.heated.value();
    let area = 4.0 * core::f64::consts::PI * radius * radius;
    let excess = math::powi(heated, 4) - math::powi(equilibrium, 4);
    let luminosity = if excess > 0.0 {
        area * STEFAN_BOLTZMANN * excess
    } else {
        0.0
    };
    let earths = EarthMasses::from(inputs.mass);
    let internal = envelope_internal_temperature(
        Watts::new(luminosity),
        Metres::new(radius),
        earths,
        inputs.age,
    )
    .ok()?;
    let effective = math::powf(
        math::powi(equilibrium, 4) + math::powi(internal.value(), 4),
        0.25,
    );
    // The grey parameters at that effective temperature (part 8), read once for both profiles.
    let grey = GreyProfile::at_effective_temperature(Kelvin::new(effective));
    let (_, distance) = FORMED_AT
        .iter()
        .find(|(side, _)| *side == inputs.formed)
        .copied()
        .unwrap_or(FORMED_AT[0]);
    let snow_line = Metres::new(1.0);
    let helium = inputs.envelope_helium.map_or_else(
        || Element::He.protosolar_abundance_per_hydrogen(),
        |h| h.helium_to_hydrogen(),
    );
    let abundances = EnvelopeAbundances::derive(&EnvelopeInputs {
        mass: earths,
        fractions: MassFractions::of(CoreComposition::EARTH_LIKE, inputs.envelope_fraction),
        formation_distance: Metres::new(distance),
        snow_line,
        fe_h: Dex::ZERO,
        alpha_fe: Dex::ZERO,
        host: HostAbundances::of(Dex::ZERO, &AbundanceDraws::MEDIAN),
        effective_temperature: Kelvin::new(effective),
        age: inputs.age,
        surface_gravity: MetresPerSecondSquared::new(gravity),
        primordial_helium_per_hydrogen: helium,
    })
    .ok()?;
    let g = MetresPerSecondSquared::new(gravity);
    // The profile on the gas speciated at the grey profile's 1-bar temperature, then the gas at
    // the profile's own.
    let rows = Rows::find();
    let first = gas_of(
        &abundances,
        profile_free_one_bar((grey, internal, inputs.equilibrium), g),
        &rows,
    )?;
    let shape = profile(
        (grey, internal, inputs.equilibrium),
        g,
        &first.mixture(rows.carbon_monoxide),
    )?;
    let mut gas = gas_of(&abundances, shape.at(1.0e5), &rows)?;
    let (modes, decks) = decks(&shape, &mut gas, (g, inputs.equilibrium), &rows);
    let mut column_modes = Vec::with_capacity(modes.len() + 4);
    column_modes.extend(modes.iter().filter_map(column_mode));
    for mode in hazes(&shape, &gas, &decks, (g, inputs.equilibrium)) {
        if let Some(mode) = column_mode(&mode) {
            column_modes.push(mode);
        }
    }
    let scattering = Absorbers {
        hydrogen: gas.hydrogen,
        helium: gas.helium,
        methane: gas.methane.deep,
        water: gas.water.deep,
        ..Absorbers::default()
    };
    let metallicity = gas.metallicity_dex;
    Some(assemble(
        (gravity, gas.molar_mass),
        &scattering,
        &column_modes,
        |p| shape.at(p),
        |p, t| Absorbers {
            hydrogen: gas.hydrogen,
            helium: gas.helium,
            methane: gas.methane.at(p, t, metallicity),
            water: gas.water.at(p, t, metallicity),
            carbon_monoxide: gas.carbon_monoxide,
            sodium: gas.sodium.at(p, t, metallicity),
            potassium: gas.potassium.at(p, t, metallicity),
        },
    ))
}

/// The grey profile's temperature at 1 bar, K, before any join: the first speciation's.
fn profile_free_one_bar(
    (grey, internal, equilibrium): (GreyProfile, Kelvin, Kelvin),
    gravity: MetresPerSecondSquared,
) -> f64 {
    let tau = grey.optical_depth(Pascals::new(1.0e5), gravity);
    grey.temperature(tau, internal, irradiation_temperature(equilibrium))
        .value()
}

/// The Bond albedo of the gas envelope `inputs` describes under the light `shade`, its own
/// column's ([`snapshot_column`], [`column_bond`]); `None` where the closed form gives no column,
/// or where its column returns no light at all (labelled): an opaque top layer the Eddington
/// closure holds at R = 0, ω′(4 − 3g′) < 1, as a soot-shrouded sub-Neptune's (τ 17, ω about 0.25),
/// where the photometry needs a p above 0 and a semi-infinite layer reflects a few per cent.
#[must_use]
pub(crate) fn snapshot_bond(inputs: &AtmosphereInputs, shade: &HostShade) -> Option<f64> {
    let bond = column_bond(&snapshot_column(inputs)?, shade);
    (bond.is_finite() && bond > 0.0 && bond <= 1.0).then_some(bond)
}

/// A mode as the column's assembly reads it: `None` for one with no τ(550).
pub(crate) fn column_mode(mode: &AerosolMode) -> Option<ColumnMode> {
    let tau = mode.optical_depth().ok()?.value();
    let profile = mode.profile();
    let key = mode.material().substance().key();
    let optics = particle_table(key.as_str(), mode.phase()).and_then(|table| match mode.size() {
        Ok(ParticleSize::Distribution(d)) => Some((table, d.effective_radius().value())),
        Ok(ParticleSize::Aggregate(_)) | Err(_) => None,
    });
    Some(ColumnMode {
        tau,
        base: profile.base().value(),
        top: profile.top().value(),
        exponent: profile.exponent(),
        coverage: mode.coverage(),
        optics,
    })
}

/// Where a deck's vapour first saturates going up from the column's bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Base {
    /// Already at 100 bar: trapped there, no deck in the column.
    Trapped,
    /// Nowhere: no deck.
    Nowhere,
    /// At this pressure, Pa.
    At(f64),
}

/// A deck's law and its vapours as the closed form reads them: the law, the limiting vapour's
/// mole fraction and count, and for a product law the other vapour's mole fraction.
#[derive(Debug, Clone, Copy)]
struct DeckVapour {
    law: LawAloft,
    fraction: f64,
    count: f64,
    other: f64,
}

impl DeckVapour {
    /// ln of its supersaturation at `p` (Pa) on `shape` in a gas of metallicity `metallicity_dex`:
    /// positive where it condenses, −∞ where it cannot.
    fn excess(&self, shape: &Profile, p: f64, metallicity_dex: f64) -> f64 {
        ln_saturated_fraction(&self.law, p, shape.at(p), self.other, metallicity_dex)
            .map_or(f64::NEG_INFINITY, |s| math::ln(self.fraction) - s)
    }

    /// Its base on `shape` (the [module](self)'s documentation): the first level up from 100 bar,
    /// in steps of [`DECK_SCAN_STEP_LN_P`], at which it saturates, bisected in that step (a vapour
    /// saturated only about the temperature's minimum is undersaturated again at the column's
    /// top).
    fn base(&self, shape: &Profile, metallicity_dex: f64) -> Base {
        let bottom = math::ln(ADIABAT_BOTTOM.value());
        if self.excess(shape, math::exp(bottom), metallicity_dex) >= 0.0 {
            return Base::Trapped;
        }
        let top = math::ln(COLUMN_TOP_PA);
        let (mut deep, mut shallow) = (bottom, f64::NAN);
        let mut level = bottom;
        while level > top {
            let next = (level - DECK_SCAN_STEP_LN_P).max(top);
            if self.excess(shape, math::exp(next), metallicity_dex) >= 0.0 {
                (deep, shallow) = (level, next);
                break;
            }
            level = next;
        }
        if shallow.is_nan() {
            return Base::Nowhere;
        }
        for _ in 0..JOIN_BISECTIONS {
            let middle = f64::midpoint(deep, shallow);
            if self.excess(shape, math::exp(middle), metallicity_dex) >= 0.0 {
                shallow = middle;
            } else {
                deep = middle;
            }
        }
        Base::At(math::exp(shallow))
    }
}

/// The deck of `id` whose vapour `vapour` is based at `base` (Pa) on `shape` in `gas`, in the
/// sedimentation's `column`, cut at `dayside` where given: its mode, Ackerman and Marley's column
/// at the saturation slope of the scan step above its base (the [module](self)'s documentation);
/// `None` where a piece refuses it.
fn deck_mode(
    (id, vapour): (SubstanceId, &DeckVapour),
    base: f64,
    (shape, gas): (&Profile, &Gas),
    column: &MixingColumn,
    dayside: Option<f64>,
) -> Option<AerosolMode> {
    let gravity = column.gravity.value();
    let metallicity = gas.metallicity_dex;
    let base_t = shape.at(base);
    let above = base * math::exp(-SCAN_STEP_LN_P);
    let slope = (ln_saturated_fraction(&vapour.law, base, base_t, vapour.other, metallicity)?
        - ln_saturated_fraction(
            &vapour.law,
            above,
            shape.at(above),
            vapour.other,
            metallicity,
        )?)
        / SCAN_STEP_LN_P;
    let slope = if slope > 0.0 { slope } else { 0.0 };
    let epsilon = id.substance().molar_mass_g_per_mol()? / vapour.count / gas.molar_mass;
    let fraction = vapour.fraction;
    let efficiency = DeckKind::of(id)
        .and_then(DeckKind::fixed_sedimentation)
        .unwrap_or_else(|| {
            let supply = epsilon * fraction * base / gravity;
            let speed = SEDIMENTATION_SPEED_AT_UNIT_SUPPLY.value()
                * math::powf(supply, SEDIMENTATION_SUPPLY_EXPONENT);
            let height = MOLAR_GAS_CONSTANT * base_t / (gas.molar_mass / 1_000.0 * gravity);
            (speed * height / CLOUD_EDDY_DIFFUSION.square_metres_per_second())
                .max(LEAST_SEDIMENTATION_EFFICIENCY)
        });
    let mass = epsilon * fraction * base * slope / (gravity * (slope + 1.0) * (efficiency + 1.0));
    let exponent = efficiency.min(slope);
    let top = if exponent > 0.0 {
        (base * math::powf(TOP_FRACTION, 1.0 / exponent)).max(COLUMN_TOP_PA)
    } else {
        COLUMN_TOP_PA
    };
    let phase = deck_phase(id, base_t);
    let sedimentation = Sedimentation::new(efficiency, DECK_WIDTH).ok()?;
    let hydrogen = Species::Hydrogen.substance();
    let size =
        settling_size_at((id, phase), (base, base_t), column, sedimentation, hydrogen).ok()?;
    let profile = ModeProfile::new(Pascals::new(base), Pascals::new(top), exponent).ok()?;
    let mode = Mode::new(
        id,
        phase,
        id,
        size,
        KilogramsPerSquareMetre::new(mass),
        profile,
        1.0,
        Kelvin::new(base_t),
    )
    .ok()?;
    let cut = match DeckKind::of(id) {
        Some(DeckKind::Refractory | DeckKind::SulphideOrSalt) => dayside,
        Some(DeckKind::Volatile) | None => None,
    };
    let mode = match cut {
        Some(level) => mode.below(Pascals::new(level), Kelvin::new(base_t)).ok()?,
        None => mode,
    };
    AerosolMode::of_settling(&mode, Kelvin::new(base_t)).ok()
}

/// The decks of the candidates `rows` on `shape` in `gas` under gravity `gravity` at `T_eq`
/// `equilibrium`, the gas left as the decks leave it: each deck's mode and each deck's (row,
/// base) (the [module](self)'s documentation).
fn decks(
    shape: &Profile,
    gas: &mut Gas,
    (gravity, equilibrium): (MetresPerSecondSquared, Kelvin),
    rows: &Rows,
) -> (Vec<AerosolMode>, Vec<(SubstanceId, f64)>) {
    let column = MixingColumn {
        gravity,
        molar_mass_g_per_mol: gas.molar_mass,
        eddy_diffusion: CLOUD_EDDY_DIFFUSION,
        metallicity: math::exp10(gas.metallicity_dex),
        top: Pascals::new(COLUMN_TOP_PA),
        bottom: ADIABAT_BOTTOM,
    };
    let dayside = (DaysideDecksRules::in_force() == DaysideDecksRules::Cleared
        && equilibrium.value() >= HOT_DAYSIDE_TEMPERATURE.value())
    .then(|| shape.photosphere().max(DAYSIDE_LEVEL.value()));
    let mut modes = Vec::new();
    let mut formed = Vec::new();
    for id in rows.candidates.iter().flatten().copied() {
        let Some(law) = law_aloft(id) else { continue };
        let product = matches!(law.law(), SaturationLaw::Product { .. });
        let (mut limiting, mut others) = (law.limiting(), law.others().first().copied());
        let (mut fraction, mut other) = (
            gas.vapour(limiting.vapour()),
            others.map_or(0.0, |o| gas.vapour(o.vapour())),
        );
        // For a product law the scarcer of its two vapours limits it.
        if product
            && let Some(second) = others
            && other < fraction
        {
            (limiting, others) = (second, Some(limiting));
            core::mem::swap(&mut fraction, &mut other);
        }
        if fraction <= 0.0 || (product && other <= 0.0) {
            continue;
        }
        let vapour = DeckVapour {
            law,
            fraction,
            count: f64::from(limiting.count()),
            other,
        };
        let base = match vapour.base(shape, gas.metallicity_dex) {
            Base::Nowhere => continue,
            Base::Trapped => {
                gas.trap_or_take(
                    limiting.vapour(),
                    (law, ADIABAT_BOTTOM.value(), other),
                    fraction,
                );
                continue;
            }
            Base::At(base) => base,
        };
        let mode = deck_mode((id, &vapour), base, (shape, gas), &column, dayside);
        // A CH₄ deck's particles are its methane layer's (part 7), as on the record.
        let mode = if id == Species::Methane.substance() {
            mode.and_then(|mode| methane_layer_mode(&mode).ok())
        } else {
            mode
        };
        // Taken from the gas: the limiting vapour above the base, the others by their counts.
        if let Some(second) = others {
            gas.take(
                second.vapour(),
                fraction * f64::from(second.count()) / vapour.count,
            );
        }
        gas.trap_or_take(limiting.vapour(), (law, base, other), fraction);
        formed.push((id, base));
        modes.extend(mode);
    }
    (modes, formed)
}

/// The hazes P14.T52.b's rules give on `shape` in the gas `gas` the decks `decks` leave, under
/// gravity `gravity` at `T_eq` `equilibrium`, as P14.T52.d reads them on a starlit envelope.
fn hazes(
    shape: &Profile,
    gas: &Gas,
    decks: &[(SubstanceId, f64)],
    (gravity, equilibrium): (MetresPerSecondSquared, Kelvin),
) -> Vec<AerosolMode> {
    let air = UpperAtmosphere {
        equilibrium_temperature: equilibrium,
        stratosphere_temperature: shape.skin(),
        metallicity: math::exp10(gas.metallicity_dex),
        gravity,
        molar_mass_g_per_mol: gas.molar_mass,
    };
    let base = |gas: Species| {
        let id = gas.substance();
        decks.iter().find(|(d, _)| *d == id).map(|&(_, b)| b)
    };
    let upper = 1.0e3;
    let t_upper = shape.at(upper);
    let mut hazes = Vec::new();
    if let Ok(Some(haze)) = sulphur_haze(&air, gas.sulphide) {
        hazes.push((haze, 1.0));
    }
    let weight = soot_weight(equilibrium);
    if weight > 0.0
        && let Ok(Some(haze)) = soot(&air, gas.methane.at(upper, t_upper, gas.metallicity_dex))
    {
        hazes.push((haze, weight));
    }
    let methane = base(Species::Methane);
    if let Some(methane) = methane
        && let Ok(Some(haze)) = ice_giant_haze(Pascals::new(methane))
    {
        hazes.push((haze, 1.0));
    }
    let cloud = base(Species::Ammonia).filter(|&ammonia| methane.is_none_or(|m| m >= ammonia));
    if let Some(ammonia) = cloud
        && let Ok(Some(haze)) = chromophore(Pascals::new(ammonia), gravity)
    {
        hazes.push((haze, 1.0));
    }
    // The cloud's haze (part 6), the NH₃ deck's second mode on the record.
    if let Some(ammonia) = cloud
        && let Ok(Some(haze)) = ammonia_haze(Pascals::new(ammonia))
    {
        hazes.push((haze, 1.0));
    }
    hazes
        .iter()
        .filter_map(|(haze, weight)| {
            haze_mode_at(haze, *weight, |p| Kelvin::new(shape.at(p.value()))).ok()
        })
        .collect()
}
