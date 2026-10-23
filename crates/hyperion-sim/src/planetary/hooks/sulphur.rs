//! Sulphur (plan 14, P14.T24.g; science-r08-sulphur-co2ice.md, accepted with amendments in
//! signoff-3.md): the sulphuric-acid deck of a runaway-greenhouse world, the sulphate layer of a
//! temperate or snowball one, and SO₂ as an absorber, called by P14.T24.c's inventory
//! ([`AerosolInventory`](super::aerosols::AerosolInventory)).
//!
//! # The source
//!
//! Volcanic SO₂ is outgassed at Φ = O × Φ⊕ ([`EARTH_SULPHUR_DIOXIDE_FLUX`]), O the present
//! outgassing relative to Earth's (P14.T24.b). Its photochemistry (SO₂ + O → SO₃, SO₃ + H₂O →
//! H₂SO₄, with O from photolysis below about 240 nm) takes days to a month (Loftus, Wordsworth and
//! Morley 2019, ApJ 887, 231, §3.4), so the SO₂ reaching a starlit upper atmosphere is taken to
//! become acid, and a body with no star as host makes none. Water does not limit it (Dai et al.
//! 2022, JGR Planets 127, e2021JE007060, §2.3: production is limited by SO₃ "because H₂O is
//! sufficient"). SO₂ enters no escape, greenhouse or P14.T24.a fraction: P14.T13.c's grey fit,
//! made on Venus, already holds its warming.
//!
//! # Where it goes ([`SulphurCycle`], the caller's: [`SulphurCycle::case_of`])
//!
//! The case is an input, taken from P14.T13.c's state and whether the body's climate keeps the
//! acid deck ([`RunawayDeck`]; decision-absurd-states-24 R3), so that the caller routes a deckless
//! runaway without this module matching the state.
//!
//! 1. **A runaway greenhouse under its deck: the sulphur cycles.** The falling acid evaporates and
//!    decomposes below the deck, returning its sulphur to SO₂, where the air there is hotter than
//!    [`ACID_CYCLING_TEMPERATURE`]; only the surface removes it. Its column is Φ
//!    [`DRY_SULPHUR_LIFETIME`], fixed on Venus's measured state.
//! 2. **A temperate or snowball world, or a runaway too cold beneath its deck to cycle it: once
//!    through.** SO₂ lives [`WET_SULPHUR_LIFETIME`] on a world with liquid water and
//!    [`DRY_ONCE_THROUGH_LIFETIME`] on one without (a runaway's, its water lost, is dry); only a
//!    thin stratospheric layer forms ([`sulphate_layer_optical_depth`]).
//! 3. **An airless body, a magma ocean or a gas envelope: none.**
//!
//! In every case the SO₂ is held a trace, its column at most [`SULPHUR_DIOXIDE_TRACE_LIMIT`] of
//! the air's (decision-thick-acid-decks).
//!
//! # The deck (case 1)
//!
//! The acid's vapour below the deck is `ε_v` `x_SO₂` (`H2SO4`'s law aloft, its vapour share 0.06),
//! and its base is where P14.T52.c's loop finds it saturate along P14.T24.e's profile. Its top is
//! T24.e's tropopause, where the acid is made; with no base below the top there is no deck, the hot
//! limit. Its τ(550) is `σ_d` `x_SO₂` (`p_base` − `p_top`) ÷ (m̄ `m_u` g)
//! ([`ACID_DECK_CROSS_SECTION`]), shared among four log-normal modes of Pollack et al. 1993's radii
//! ([`ACID_MODES`]), each well mixed between fractions u of the deck's log-pressure span, u =
//! ln(`p_base` ÷ p) ÷ ln(`p_base` ÷ `p_top`); mode 2 carries Venus's unidentified ultraviolet absorber
//! (`acid_uv_absorber`, science-r08-unowned-absorbers). Its vapour above is the acid's
//! saturation at the skin over the top's pressure.
//!
//! # Not on the record yet
//!
//! This module is pure: nothing on the record calls it at generator version 23
//! (decision-bump-plan-after-22 §1.3).

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::chem::condense::{BuildModeError, Deck, ModeProfile};
use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::hooks::aerosols::{
    Absorber, AerosolDeck, AerosolMode, BuildAbsorberError, ShapeClass,
};
use crate::planetary::hooks::structure::RadiativeConvective;
use crate::substance::aloft::law_aloft;
use crate::substance::{
    BuildSizeDistributionError, CondensedPhase, MOLAR_GAS_CONSTANT, SizeDistribution, by_key,
};
use crate::units::consts::BOLTZMANN_CONSTANT;
use crate::units::{Kelvin, Metres, MetresPerSecondSquared, Pascals, Seconds};

/// Avogadro's number, mol⁻¹: R ÷ k, both exact in the 2019 SI.
const AVOGADRO: f64 = MOLAR_GAS_CONSTANT / BOLTZMANN_CONSTANT;

/// Seconds in a day.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// Earth's volcanic SO₂ flux, molecules m⁻² s⁻¹: its passive degassing of 23 ± 2 Tg SO₂ yr⁻¹ (Carn
/// et al. 2017, Sci. Rep. 7, 44095) over its surface, 23 × 10¹² g ÷ 64.066 g mol⁻¹ × `N_A` ÷ (one
/// Julian year × 4π (6,371 km)²) = 1.343 × 10¹³. Hu, Seager and Bains 2013 (ApJ 769, 6) take
/// about twice that for all of Earth's volcanic sulphur.
pub const EARTH_SULPHUR_DIOXIDE_FLUX: f64 = 1.34e13;

/// SO₂'s life on a temperate world with liquid water, s: 2 days, dissolved in cloud and rain (Hu,
/// Seager and Bains 2013, after Seinfeld and Pandis 2006).
pub const WET_SULPHUR_LIFETIME: Seconds = Seconds::new(2.0 * SECONDS_PER_DAY);

/// SO₂'s life on a temperate or snowball world without liquid water, s: 30 days, the photochemical
/// conversion on Earth (Loftus, Wordsworth and Morley 2019, §3.4), after which the acid falls to the
/// ground as sulphate.
pub const DRY_ONCE_THROUGH_LIFETIME: Seconds = Seconds::new(30.0 * SECONDS_PER_DAY);

/// Venus's SO₂ below its clouds: 130 ± 50 ppmv at 30–40 km (Marcq et al. 2008, JGR 113, E00B07).
pub const VENUS_SULPHUR_DIOXIDE: f64 = 130e-6;

/// Venus's air column, molecules m⁻²: P14.T57.a's measured inventory, 4.69 × 10²⁰ kg of CO₂ and
/// 1.08 × 10¹⁹ kg of N₂ (44.0095 and 28.0134 g mol⁻¹), over its mean radius of 6,051.8 km (NASA's
/// Venus fact sheet): about 1.445 × 10³¹.
pub const VENUS_AIR_COLUMN: f64 = (4.69e20 / 44.009_5e-3 + 1.08e19 / 28.013_4e-3) * AVOGADRO
    / (4.0 * core::f64::consts::PI * 6.051_8e6 * 6.051_8e6);

/// P14.T24.b's present outgassing O on the Solar System table's Venus, at which
/// [`DRY_SULPHUR_LIFETIME`] is fixed (the module's test `the_dry_lifetime_is_fixed_on_venus`
/// holds the table to it).
pub const VENUS_OUTGASSING: f64 = 0.363_470_162_966_442_9;

/// SO₂'s life on a runaway-greenhouse world, s: fixed on Venus's measured state, Venus's 130 ppm in
/// its measured column at T24.b's O for Venus, t = `x_SO₂` N ÷ (`O_V` Φ⊕): 4.4 Myr ÷ `O_V`, 12.2 Myr
/// at the table's `O_V` 0.363, beside Fegley and Prinn 1989's laboratory 1.9 Myr for its removal by
/// calcite (Nature 337, 55). The model's sink is so 6.4 times slower than the laboratory's, which
/// would need `O_V` near 2.3: recorded.
pub const DRY_SULPHUR_LIFETIME: Seconds = Seconds::new(
    VENUS_SULPHUR_DIOXIDE * VENUS_AIR_COLUMN / (VENUS_OUTGASSING * EARTH_SULPHUR_DIOXIDE_FLUX),
);

/// The air temperature under a runaway greenhouse's acid deck at which its sulphur cycles (case
/// 1): about 430 K. On Venus the falling acid evaporates at the cloud base (near 48 km), and its
/// vapour is decomposed thermally, H₂SO₄ → SO₃ + H₂O, the SO₃ then going to SO₂ (Dai et al. 2022,
/// JGR Planets 127, e2021JE007060, §2.3 and §3.1). Magellan's radio occultations find the vapour
/// peaking at 18–24 ppm near 39 km and "dropping precipitously below 38 km" (Jenkins et al. 1994,
/// Icarus 110, 79). VIRA puts 425.1 K at 39 km and 432.5 K at 38 km, 4.34 bar (Seiff et al. 1985,
/// Adv. Space Res. 5(11), 3, the low-latitude model). The caller compares it with the climate's
/// surface temperature, the warmest air below the deck. The figure is fixed at Venus's pressure
/// there: one molecule splitting into two, the decomposition sets in cooler in thinner air (half
/// at about 406 K at 0.42 bar by NIST-JANAF's free energies at 31 ppm of water, the science
/// check's own estimate), so a thinner air under its deck is dropped from it somewhat sooner than
/// its acid would stop cycling (recorded, not modelled).
///
/// It is the climate's test, not this module's: from generator version 24 a runaway greenhouse
/// whose climate under Venus's deck is colder than this is solved without the deck and takes case
/// 2's dry branch (P14.T53.b part 1 (b), decision-absurd-states-24 R3, through
/// [`SulphurCycle::case_of`] and [`RunawayDeck::Deckless`]).
pub const ACID_CYCLING_TEMPERATURE: Kelvin = Kelvin::new(430.0);

/// The largest share of the air's column that SO₂ may hold, in every case, dimensionless: 10⁻²
/// (decision-thick-acid-decks §3, 2026-10-23). The source is the model's own contract: SO₂ enters
/// none of P14.T24.a's fractions, its mean molar mass or its greenhouse, so it may only be a
/// trace, and at 1% T24.a's fractions are off by at most 1% and m̄ by at most 0.5%
/// (decision-absurd-states-24 R3's line). Φ t is fixed whatever the air, so without the cap a thin
/// air would carry more SO₂ than air: 3.7 times its own column in a 10-mbar CO₂ air at O = 1
/// under Earth's gravity in case 1. SO₂ as a major gas is P14.T55.a's (the outgassing's
/// speciation).
pub const SULPHUR_DIOXIDE_TRACE_LIMIT: f64 = 1e-2;

/// The acid deck's extinction per SO₂ molecule within its layers, m²: `σ_d` of τ(550) = `σ_d`
/// `x_SO₂` (`p_base` − `p_top`) ÷ (m̄ `m_u` g), fitted through this plan's own model (signoff-3.md;
/// decision-p14-moons item 4's rule) so that a Venus given its measured state by hand (90.4 bar of
/// CO₂ and 2.09 of N₂ by weight, 735 K) carries [`VENUS_CLOUD_OPTICAL_DEPTH`] along T24.e's
/// profile on T49.b's `c_p`(T): 1.3511 × 10⁻²⁴ m², its deck from 1.313 bar at 365.8 K up to the
/// skin level's 0.0556 bar, against VIRA's 1.38 and 0.037 bar at 48 and 70 km. `Adopted`; the
/// cited 1.27 × 10⁻²⁴ m² from Haus et al.'s column over VIRA's pressures stands beside it, 6%
/// below.
pub const ACID_DECK_CROSS_SECTION: f64 = 1.351_131_591_053_468e-24;

/// Venus's cloud τ(550): 34.5, Haus et al. 2016's standard cloud model (Icarus 272, 178) with
/// Pollack et al. 1993's sizes (Icarus 103, 1) through Mie at n = 1.44, read via Reyes-Guerrero et
/// al. 2026 (arXiv:2607.22187).
pub const VENUS_CLOUD_OPTICAL_DEPTH: f64 = 34.5;

/// One of the acid deck's modes: its material's key, its log-normal's geometric radius (µm) and ln
/// `σ_g`, the span of the deck's log-pressure it fills (u from, u to) and its share of τ(550).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct AcidMode {
    material: &'static str,
    geometric_radius: Metres,
    ln_width: f64,
    span: (f64, f64),
    share: f64,
}

impl AcidMode {
    /// The material's registry key: `H2SO4`, or `acid_uv_absorber` for the upper cloud's mode 2.
    #[must_use]
    pub const fn material(&self) -> &'static str {
        self.material
    }

    /// The log-normal's geometric mean radius `r_g`.
    #[must_use]
    pub const fn geometric_radius(&self) -> Metres {
        self.geometric_radius
    }

    /// ln `σ_g`, its geometric width's logarithm.
    #[must_use]
    pub const fn ln_width(&self) -> f64 {
        self.ln_width
    }

    /// The span of u it fills, deeper end first, each within 0 to 1.
    #[must_use]
    pub const fn span(&self) -> (f64, f64) {
        self.span
    }

    /// Its share of the deck's τ(550), as tabled.
    #[must_use]
    pub const fn share(&self) -> f64 {
        self.share
    }
}

/// The acid deck's four modes (plan 14, P14.T24.g's table): Pollack et al. 1993's radii and widths
/// where Haus et al. 2016's layers sit on Venus, the shares Mie at 550 nm over Haus et al.'s
/// columns, summing to 0.99 as rounded (each is taken over their sum). Mode 2's `r_eff` 1.17 µm
/// and `v_eff` 0.064 agree with Hansen and Hovenier 1974's 1.05 ± 0.10 µm and 0.07 ± 0.02.
pub const ACID_MODES: [AcidMode; 4] = [
    AcidMode {
        material: "H2SO4",
        geometric_radius: Metres::new(0.30e-6),
        ln_width: 0.44,
        span: (0.0, 1.0),
        share: 0.15,
    },
    AcidMode {
        material: "acid_uv_absorber",
        geometric_radius: Metres::new(1.00e-6),
        ln_width: 0.25,
        span: (0.55, 1.0),
        share: 0.18,
    },
    AcidMode {
        material: "H2SO4",
        geometric_radius: Metres::new(1.40e-6),
        ln_width: 0.21,
        span: (0.0, 0.5),
        share: 0.27,
    },
    AcidMode {
        material: "H2SO4",
        geometric_radius: Metres::new(3.65e-6),
        ln_width: 0.25,
        span: (0.0, 0.33),
        share: 0.39,
    },
];

/// The sulphate layer's mode (case 2): Earth's background, log-normal of `r_g` 0.08 µm and `σ_g`
/// 1.65 (`r_eff` 0.15 µm, `v_eff` 0.28; after Deshler et al. 2003, from memory: their abstract
/// gives the layers' surface area, not these), of geometric width [`SULPHATE_LAYER_WIDTH`], its
/// mean diameter [`SULPHATE_MEAN_DIAMETER`].
pub const SULPHATE_LAYER_RADIUS: Metres = Metres::new(0.08e-6);

/// The sulphate layer's log-normal width `σ_g`: 1.65 ([`SULPHATE_LAYER_RADIUS`]).
pub const SULPHATE_LAYER_WIDTH: f64 = 1.65;

/// The sulphate layer's mean particle diameter `d_p`, µm: 0.18, the plan's, which puts the table's
/// Earth at τ ≈ 0.003 (P14.T24.g). It is the mode's number mean; Hu, Seager and Bains 2013 (§2, eq.
/// 2) define the mean diameter as the surface-area mean, D₀ exp(ln² `σ_g`), 0.206 µm here, which
/// gives 0.0029: recorded (main kept the plan's 0.18, 2026-10-22).
pub const SULPHATE_MEAN_DIAMETER: Metres = Metres::new(0.18e-6);

/// The sulphate layer's top over its base, the tropopause: a fifth of its pressure, Earth's
/// 15–25 km Junge layer (from memory), into which Vernier et al. 2011 see volcanic injection at
/// 18–20 km.
pub const SULPHATE_LAYER_TOP_RATIO: f64 = 0.2;

/// The sulphate layer's τ(550) under an SO₂ flux of `flux` molecules m⁻² s⁻¹: 0.1 (Φ ÷ 10¹⁵ m⁻²
/// s⁻¹)^0.7 (`d_p` ÷ 0.1 µm)^−0.7, Hu, Seager and Bains 2013's eq. 7 for anoxic N₂ and CO₂
/// atmospheres at Earth's deposition velocity (its `V_DEP`^−0.3 term at 1), at the low end of its
/// constant (0.1–3, "about one order of magnitude"), with `d_p` [`SULPHATE_MEAN_DIAMETER`]. Eq. 7
/// gives τ at 500 nm, taken as τ(550) inside the constant's span; it counts S₈ as well as the acid.
/// An oxic air, any pressure but 1 bar, and a cold dry world's slower deposition (100–1,000 times,
/// which would raise τ 4–8 times) lie outside it: recorded extrapolations.
///
/// # Examples
///
/// Earth's own outgassing keeps its layer thin, and a thousand times it still under τ 1 (Loftus
/// et al. 2019):
///
/// ```
/// use hyperion_sim::planetary::hooks::sulphur::{
///     EARTH_SULPHUR_DIOXIDE_FLUX, sulphate_layer_optical_depth,
/// };
///
/// let earth = sulphate_layer_optical_depth(EARTH_SULPHUR_DIOXIDE_FLUX);
/// assert!((1e-3..1e-2).contains(&earth));
/// assert!(sulphate_layer_optical_depth(1e3 * EARTH_SULPHUR_DIOXIDE_FLUX) < 1.0);
/// ```
#[must_use]
pub fn sulphate_layer_optical_depth(flux: f64) -> f64 {
    if !(flux.is_finite() && flux > 0.0) {
        return 0.0;
    }
    0.1 * math::powf(flux / 1e15, 0.7) * math::powf(SULPHATE_MEAN_DIAMETER.value() / 0.1e-6, -0.7)
}

/// Where a body's sulphur goes (the [module](self)'s documentation): the caller's case, by
/// [`SulphurCycle::case_of`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SulphurCycle {
    /// A runaway greenhouse under its deck: the sulphur cycles, and its acid makes a deck.
    Cycling,
    /// A temperate or snowball world, or a deckless runaway: once through, a thin stratospheric
    /// layer.
    OnceThrough,
    /// An airless body, a magma ocean, a gas envelope, a body with no star as host, or one that
    /// outgasses none: no sulphur.
    None,
}

/// Whether a runaway greenhouse's climate keeps Venus's acid deck (P14.T53.b part 1 (b),
/// decision-absurd-states-24 R3): the derivation decides it and publishes it on its atmosphere
/// (`Atmosphere::runaway_deck`), so it lives beside `SurfaceState`; re-exported here, so that this
/// module's and its callers' paths read it as before.
pub use crate::planetary::derive::atmosphere::RunawayDeck;

impl SulphurCycle {
    /// The case of a body in P14.T13.c's state `state` whose climate keeps or drops the acid deck
    /// (`deck`, read only for a runaway greenhouse): a runaway under its deck cycles; a deckless
    /// runaway, a temperate world and a snowball pass it once through; any other state makes none.
    /// The once-through branch, wet or dry, is [`SulphurInputs::liquid_water`]'s.
    ///
    /// # Examples
    ///
    /// A runaway too cold beneath its deck takes the once-through case (decision-absurd-states-24
    /// R3):
    ///
    /// ```
    /// use hyperion_sim::planetary::derive::atmosphere::SurfaceState;
    /// use hyperion_sim::planetary::hooks::sulphur::{RunawayDeck, SulphurCycle};
    ///
    /// let runaway = SurfaceState::RunawayGreenhouse;
    /// assert_eq!(SulphurCycle::case_of(runaway, RunawayDeck::Kept), SulphurCycle::Cycling);
    /// assert_eq!(SulphurCycle::case_of(runaway, RunawayDeck::Deckless), SulphurCycle::OnceThrough);
    /// ```
    #[must_use]
    pub const fn case_of(state: SurfaceState, deck: RunawayDeck) -> Self {
        match (state, deck) {
            (SurfaceState::RunawayGreenhouse, RunawayDeck::Kept) => Self::Cycling,
            (SurfaceState::RunawayGreenhouse, RunawayDeck::Deckless)
            | (SurfaceState::Temperate | SurfaceState::Snowball, _) => Self::OnceThrough,
            (SurfaceState::Airless | SurfaceState::MagmaOcean | SurfaceState::GasEnvelope, _) => {
                Self::None
            }
        }
    }
}

/// What [`Sulphur::derive`] reads, as plain values. The derivation checks each, so they are public.
#[derive(Debug, Clone, Copy)]
pub struct SulphurInputs<'a> {
    /// The case, the caller's ([`SulphurCycle::case_of`]).
    pub cycle: SulphurCycle,
    /// P14.T24.b's present outgassing O, relative to Earth's.
    pub outgassing: f64,
    /// Whether a star lights the body (its photochemistry's ultraviolet); a rogue planet or a brown
    /// dwarf's makes no acid.
    pub starlit: bool,
    /// Whether the surface holds liquid water (P14.T24.a's flag), which sets SO₂'s life once
    /// through.
    pub liquid_water: bool,
    /// The surface pressure.
    pub surface_pressure: Pascals,
    /// The air's mean molar mass, g mol⁻¹.
    pub mean_molar_mass_g_per_mol: f64,
    /// The surface gravity.
    pub gravity: MetresPerSecondSquared,
    /// The air's vertical structure, P14.T24.e's.
    pub structure: &'a RadiativeConvective,
}

/// A body's sulphur: its cycle, its SO₂ flux, column and mole fraction (the [module](self)'s
/// documentation).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Sulphur {
    cycle: SulphurCycle,
    flux: f64,
    column_per_m2: f64,
    mole_fraction: f64,
}

/// A sulphur rule's mode or absorber could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveSulphurError {
    /// A mode's size is not one.
    Size(BuildSizeDistributionError),
    /// A mode is not one.
    Mode(BuildModeError),
    /// The SO₂ absorber is not one.
    Absorber(BuildAbsorberError),
}

impl fmt::Display for DeriveSulphurError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size(error) => write!(f, "an acid mode's size: {error}"),
            Self::Mode(error) => write!(f, "an acid mode: {error}"),
            Self::Absorber(error) => write!(f, "the SO₂ absorber: {error}"),
        }
    }
}

impl Error for DeriveSulphurError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Size(error) => Some(error),
            Self::Mode(error) => Some(error),
            Self::Absorber(error) => Some(error),
        }
    }
}

impl From<BuildSizeDistributionError> for DeriveSulphurError {
    fn from(error: BuildSizeDistributionError) -> Self {
        Self::Size(error)
    }
}

impl From<BuildModeError> for DeriveSulphurError {
    fn from(error: BuildModeError) -> Self {
        Self::Mode(error)
    }
}

impl From<BuildAbsorberError> for DeriveSulphurError {
    fn from(error: BuildAbsorberError) -> Self {
        Self::Absorber(error)
    }
}

impl Sulphur {
    /// None: no cycle, no flux, no SO₂.
    pub const NONE: Self = Self {
        cycle: SulphurCycle::None,
        flux: 0.0,
        column_per_m2: 0.0,
        mole_fraction: 0.0,
    };

    /// The sulphur of the body `inputs` describe (the [module](self)'s documentation). It makes
    /// no draw.
    #[must_use]
    pub fn derive(inputs: &SulphurInputs<'_>) -> Self {
        let o = inputs.outgassing;
        let p = inputs.surface_pressure.value();
        let mu = inputs.mean_molar_mass_g_per_mol;
        let g = inputs.gravity.value();
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !(inputs.starlit && positive(o) && positive(p) && positive(mu) && positive(g)) {
            return Self::NONE;
        }
        let cycle = inputs.cycle;
        let lifetime = match cycle {
            SulphurCycle::Cycling => DRY_SULPHUR_LIFETIME,
            SulphurCycle::OnceThrough if inputs.liquid_water => WET_SULPHUR_LIFETIME,
            SulphurCycle::OnceThrough => DRY_ONCE_THROUGH_LIFETIME,
            SulphurCycle::None => return Self::NONE,
        };
        let flux = o * EARTH_SULPHUR_DIOXIDE_FLUX;
        let air = air_column(p, mu, g);
        let column = flux * lifetime.value();
        // Capped as a trace, the share exact at the cap and the column below it as it was.
        let (column, mole_fraction) = if column <= SULPHUR_DIOXIDE_TRACE_LIMIT * air {
            (column, column / air)
        } else {
            (
                SULPHUR_DIOXIDE_TRACE_LIMIT * air,
                SULPHUR_DIOXIDE_TRACE_LIMIT,
            )
        };
        Self {
            cycle,
            flux,
            column_per_m2: column,
            mole_fraction,
        }
    }

    /// Where the sulphur goes.
    #[must_use]
    pub const fn cycle(&self) -> SulphurCycle {
        self.cycle
    }

    /// The SO₂ flux Φ, molecules m⁻² s⁻¹.
    #[must_use]
    pub const fn flux(&self) -> f64 {
        self.flux
    }

    /// The SO₂ column, molecules m⁻²: the flux times its life, at most
    /// [`SULPHUR_DIOXIDE_TRACE_LIMIT`] of the air's.
    #[must_use]
    pub const fn column_per_m2(&self) -> f64 {
        self.column_per_m2
    }

    /// SO₂'s mole fraction in the air, its column over the air's.
    #[must_use]
    pub const fn mole_fraction(&self) -> f64 {
        self.mole_fraction
    }

    /// The acid deck of a cycling world (case 1) from the loop's `H2SO4` deck `found`, its base:
    /// the deck with its top at the tropopause, its τ(550) by [`ACID_DECK_CROSS_SECTION`] and its
    /// four modes ([`ACID_MODES`]), each covering `coverage` of the sky; `None` where the sulphur
    /// does not cycle or the base lies at or above the top (the hot limit).
    ///
    /// # Errors
    ///
    /// [`DeriveSulphurError`] where a mode cannot be built.
    ///
    /// # Panics
    ///
    /// Never: [`ACID_MODES`]' materials are registry rows.
    pub fn acid_deck(
        &self,
        inputs: &SulphurInputs<'_>,
        found: &Deck,
        coverage: f64,
    ) -> Result<Option<(AerosolDeck, Vec<AerosolMode>)>, DeriveSulphurError> {
        if self.cycle != SulphurCycle::Cycling {
            return Ok(None);
        }
        let structure = inputs.structure;
        let base = found.base();
        let top = structure.tropopause_pressure();
        if base.value() <= top.value() {
            return Ok(None);
        }
        let skin = structure.skin_temperature();
        let molecule = inputs.mean_molar_mass_g_per_mol / 1_000.0 / AVOGADRO;
        let tau = ACID_DECK_CROSS_SECTION * self.mole_fraction * (base.value() - top.value())
            / (molecule * inputs.gravity.value());
        let above = law_aloft(found.condensate())
            .and_then(|law| law.ln_saturation_pressure(skin, top, 0.0))
            .map_or(0.0, |ln_p| math::exp(ln_p) / top.value());
        let deck = AerosolDeck::of_rule(found, CondensedPhase::Liquid, top, above, 0.0);
        let span = math::ln(base.value() / top.value());
        // The tabled shares, rounded to two places, sum to 0.99: each is taken over their sum, so
        // that the modes' τ sum to the deck's.
        let share_sum: f64 = ACID_MODES.iter().map(|m| m.share).sum();
        let mut modes = Vec::with_capacity(ACID_MODES.len());
        for mode in &ACID_MODES {
            let material =
                by_key(mode.material).expect("the acid modes' materials are registry rows");
            let at = |u: f64| Pascals::new(base.value() * math::exp(-u * span));
            let profile = ModeProfile::new(at(mode.span.0), at(mode.span.1), 0.0)?;
            let size = log_normal(mode.geometric_radius, mode.ln_width)?;
            modes.push(AerosolMode::of_rule(
                material,
                CondensedPhase::Liquid,
                ShapeClass::Sphere,
                found.condensate(),
                size,
                tau * mode.share / share_sum,
                profile,
                coverage,
                found.base_temperature(),
            )?);
        }
        Ok(Some((deck, modes)))
    }

    /// The sulphate layer of a once-through world (case 2): one acid mode
    /// ([`SULPHATE_LAYER_RADIUS`]) of τ(550) [`sulphate_layer_optical_depth`], well mixed from the
    /// tropopause to [`SULPHATE_LAYER_TOP_RATIO`] of its pressure over the whole sky; `None` where
    /// the sulphur is not once through or makes no layer.
    ///
    /// # Errors
    ///
    /// [`DeriveSulphurError`] where the mode cannot be built.
    ///
    /// # Panics
    ///
    /// Never: `H2SO4` is a registry row.
    pub fn sulphate_layer(
        &self,
        inputs: &SulphurInputs<'_>,
    ) -> Result<Option<AerosolMode>, DeriveSulphurError> {
        let tau = sulphate_layer_optical_depth(self.flux);
        if self.cycle != SulphurCycle::OnceThrough || tau <= 0.0 {
            return Ok(None);
        }
        let acid = by_key("H2SO4").expect("H2SO4 is a registry row");
        let structure = inputs.structure;
        let base = structure.tropopause_pressure();
        let top = Pascals::new(base.value() * SULPHATE_LAYER_TOP_RATIO);
        let profile = ModeProfile::new(base, top, 0.0)?;
        let size = log_normal(SULPHATE_LAYER_RADIUS, math::ln(SULPHATE_LAYER_WIDTH))?;
        Ok(Some(AerosolMode::of_rule(
            acid,
            CondensedPhase::Liquid,
            ShapeClass::Sphere,
            acid,
            size,
            tau,
            profile,
            1.0,
            structure.skin_temperature(),
        )?))
    }

    /// SO₂ as an absorber: its column, from the ground `ground` to `top` (the deck's top, or the
    /// tropopause); `None` where there is none.
    ///
    /// # Errors
    ///
    /// [`DeriveSulphurError::Absorber`] for a layer that is not one.
    ///
    /// # Panics
    ///
    /// Never: `SO2` is a registry row.
    pub fn absorber(
        &self,
        ground: Pascals,
        top: Pascals,
    ) -> Result<Option<Absorber>, DeriveSulphurError> {
        if self.cycle == SulphurCycle::None || self.column_per_m2 <= 0.0 {
            return Ok(None);
        }
        let so2 = by_key("SO2").expect("SO2 is a registry row");
        Ok(Some(Absorber::slab(so2, self.column_per_m2, ground, top)?))
    }
}

/// The air column of a surface pressure `p` pascals of mean molar mass `mu` g mol⁻¹ under gravity
/// `g` m s⁻², molecules m⁻²: p ÷ (m̄ g).
#[must_use]
fn air_column(p: f64, mu: f64, g: f64) -> f64 {
    p * AVOGADRO / (mu / 1_000.0 * g)
}

/// The log-normal of geometric radius `r_g` and ln `σ_g` `ln_width`, by its effective radius
/// `r_g` exp(5 ln²σ ÷ 2) and variance exp(ln²σ) − 1 (Hansen and Travis 1974).
fn log_normal(r_g: Metres, ln_width: f64) -> Result<SizeDistribution, BuildSizeDistributionError> {
    let squared = ln_width * ln_width;
    SizeDistribution::log_normal(
        Metres::new(r_g.value() * math::exp(2.5 * squared)),
        math::exp(squared) - 1.0,
    )
}

#[cfg(test)]
mod tests;
