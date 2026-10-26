//! Condensation aloft over the registry (plan 14, P14.T52.c; decision-composition §1.3): from a
//! gas below its clouds and its temperature profile T(p), each condensate's deck, the vapour it
//! leaves above, and its mode of particles, on the Surface path.
//!
//! # The loop
//!
//! [`Condensation::derive`] walks every registry row with a law aloft ([`laws_aloft`]: the closed
//! forms of [`substance::aloft`](crate::substance::aloft) and every row's own saturation curve)
//! whose reaction's vapours the gas holds:
//!
//! - **The base** of each is the deepest level, going up from the column's bottom, at which its
//!   vapour reaches saturation along the profile: x P = `p_sat`(T(p), P) for the vapour's mole
//!   fraction x (for a product law, x₁ x₂ P² = K(T)). The column is scanned upward in steps of
//!   [`SCAN_STEP_LN_P`] in ln p, and the crossing found is refined by [`BASE_STEPS`] Newton steps
//!   in ln p, each kept inside its bracket (a step that leaves it bisects instead), so that the
//!   base lies on its saturation curve to 10⁻⁶ in pressure. A vapour already saturated at the
//!   bottom has its base there ([`DeckBound::Bottom`]); one that never saturates forms no deck.
//! - **The cold traps** take the decks hottest base first, ties by [`SubstanceId`], a deck below
//!   the column by the temperature it would condense at the column's bottom (Spiegel,
//!   Silverio and Burrows 2009, ApJ 699, 1487: "Without macroscopic mixing, fTiO = 0 at the top of
//!   the atmosphere"). Each deck's vapour above its base is what its column leaves (below), and
//!   the gas loses the rest of it, and of the reaction's other vapours in proportion (their
//!   [`Reactant`](crate::substance::aloft::Reactant) counts), each held at no more than the gas
//!   has left of it. Every later base is found in the gas the earlier decks leave: forsterite
//!   takes the magnesium and half the silicon, so that enstatite condenses from the silicon it
//!   leaves, about 29 ppm at solar (Morley et al. 2024; Batalha et al. 2025), and ammonium
//!   hydrosulphide takes the sulphur below Jupiter's ammonia deck. An element is conserved
//!   through the traps to rounding: what the gas holds at a level plus what the decks below have
//!   taken is what the deep gas held ([`Condensation::remaining`]). A law whose reaction needs a
//!   vapour the gas lacks (perovskite without calcium) forms no deck.
//! - **The column** of each deck is Ackerman and Marley's (2001, ApJ 556, 872, eq. 4) balance of
//!   mixing against sedimentation, −K ∂`q_t`/∂z − `f_sed` w* `q_c` = 0, with w* = K ÷ L and the
//!   mixing length L the scale height H (as Morley et al. 2024's Diamondback and Batalha et al.
//!   2025's Virga take it): d`q_t` ÷ d ln p = `f_sed` `q_c`, with `q_c` = max(0, `q_t` − `q_s`)
//!   and the vapour `q_v` = min(`q_t`, `q_s`), all excess vapour condensed (their `S_cloud` = 0).
//!   It is integrated upward from the base, where `q_t` is the vapour below, in [`SUBSTEPS`] steps
//!   to each scan step. Within a cloud whose saturation falls across a step, the step is the
//!   balance's own solution for `q_s` falling exponentially between its ends at the rate s, `q_t`
//!   = `q_t0` e^(−`f_sed` Δ) + `f_sed` Δ `q_s,end` φ((`f_sed` − s) Δ), φ(δ) = (1 − e^(−δ)) ÷ δ,
//!   so that a large `f_sed`'s condensate follows s `q_s` ÷ (`f_sed` − s) to any `f_sed` (P14.T24.c's
//!   precipitating decks, `f_sed` up to 10⁴ and more); elsewhere (a saturation that rises with
//!   height, or a vapour not yet saturated at the step's start) it relaxes toward the saturation
//!   held at its ends' geometric mean. In such a cloud the step's condensate, `q_c` = `q_c0`
//!   e^(−`f_sed` x) + s `q_s0` (e^(−s x) − e^(−`f_sed` x)) ÷ (`f_sed` − s), is integrated against p in
//!   closed form (its divided difference taken at its limit within 10⁻⁴ of `f_sed` = s), so that a
//!   base's first step, whose condensate rises from 0 within 1 ÷ `f_sed` in ln p, is counted
//!   exactly; elsewhere by the trapezoid. (Corrected 2026-10-22 under P14.T24.c: the relaxation
//!   alone floored the column at about s Δ ÷ 2 of its vapour above `f_sed` ≈ 100, and the trapezoid
//!   at a base under-counted it by about Δ (1 + s) ÷ 2; the exact step moves an `f_sed` 3 column by
//!   under 1%, decision-bump-plan-after-22, "The decks' `f_sed`, K and humidity", G4.) The deck's
//!   column mass is ∫ ε `q_c` dp ÷ g, ε the condensate's molar mass per limiting vapour over the
//!   gas's; its top is the shallowest level at which `q_c` still holds [`TOP_FRACTION`] of the
//!   vapour below, or the column's top; above it the deck's mixing ratio goes as about
//!   (p ÷ `p_base`)^`f_sed` (P14.T24.c's vertical profile, Mollière et al. 2020's form), which the
//!   balance reaches a few scale heights above the base. Its vapour above is `q_v` at the top, the
//!   cold trap's.
//! - **The mode** of each deck is Ackerman and Marley's log-normal of geometric width `σ_g`
//!   ([`Sedimentation`]): at each scan level of the deck, `r_w` is the radius whose fall speed is w*,
//!   α the slope of ln `v_f` against ln r over [`r_w`, `σ_g` `r_w`] (`σ_g` at least 1.1 for the fit),
//!   `r_g` = `r_w` `f_sed`^(1/α)
//!   exp(−(α + 6) ln²`σ_g` ÷ 2) (eq. 13) and `r_eff` = `r_g` exp(5 ln²`σ_g` ÷ 2); the mode's `r_eff`
//!   is the levels' harmonic mean weighted by their mass, which keeps the deck's projected area,
//!   and its effective variance exp(ln²`σ_g`) − 1 (Hansen and Travis 1974). Its τ(550) is P14.T49.e's
//!   mass extinction times its column mass, or, for a mode larger than the extinction table's grid,
//!   Ackerman and Marley's geometric scatterers, (3 ÷ 2) M ÷ (ρ `r_eff`) (their eq. 16),
//!   labelled ([`OpticalDepthBasis::GeometricLimit`]). Its coverage is the caller's, the snapshot
//!   path's cloud fraction for a surface's air (P14.T51.b) and 1 for an envelope.
//!
//! The α interval departs from Ackerman and Marley's text, which fits it over [`r_w` ÷ `σ_g`, `r_w`]
//! where `f_sed` > 1: their own Jupiter numbers (α 1.3, `r_g` 14 µm, `r_eff` 46 µm at the base) are
//! the upper interval's. With their fall speed at their Jupiter base, the upper interval gives α
//! 1.31, `r_g` 14.4 µm and `r_eff` 48 µm, the lower α 1.89 and `r_eff` 32 µm; with the fall speed
//! here, α 1.56 and `r_eff` 44 µm (the lane's science check).
//!
//! # The fall speed
//!
//! Ohno and Okuzumi 2017's interpolation (ApJ 835, 261, their eq. 23, eq. A5 of arXiv:1701.00917v1;
//! Ohno and Okuzumi 2018, ApJ 859, 34, eqs. 7–8, which quote it and its slip factor), smooth from
//! Stokes's law to the drag of constant `C_D` = 0.45, with Cunningham's slip factor β = 1 + Kn
//! (1.257 + 0.4 exp(−1.1 ÷ Kn)) (Davies 1945) for a mean free path λ = 1 ÷ (√2 n π d²). Virga's
//! code adds a free-molecular branch above Kn = 10, which is discontinuous with this one; the slip
//! form here tends to within about 5% of Epstein's drag there, and stands in for it:
//!
//! ```text
//! v_f = 2 β g r² ρ_p ÷ (9 η) · [1 + (0.45 g r³ ρ_a ρ_p ÷ (54 η²))^(2/5)]^(−5/4)
//! ```
//!
//! and the gas's viscosity η by Chapman and Enskog's first approximation, η = (5 ÷ 16) √(π m k T) ÷
//! (π d² Ω^(2,2)*), with m the gas's mean molecular mass, d and ε the Lennard-Jones constants of
//! its most abundant species ([`COLLISION_CONSTANTS`]) and Neufeld, Janzen and Aziz 1972's fit to
//! the collision integral (J. Chem. Phys. 57, 1100; Poling, Prausnitz and O'Connell 2001, eq.
//! 9-4.3). Ackerman and Marley's eq. B2 (Rosner's form) approximates Ω^(2,2)* as 1.22 (k T ÷
//! ε)^−0.16, which holds for their H₂ (to 2% above 150 K) but overstates the viscosity of H₂O by
//! 1.8 times at 300 K and of CO₂ by 1.3 times at 200 K (the lane's science check).
//!
//! # What is not modelled
//!
//! - **The bulk gas** ([`NotFormed::BulkGas`]): a condensable that holds at least
//!   [`BULK_BOUND`] of the gas (a Mars's or a Venus's CO₂, a Triton's N₂) sets the profile by its
//!   own condensation, which the trace balance here does not; it is labelled, never trapped.
//! - **A displaced law** ([`NotFormed::Displaced`]): quartz, where enstatite can take its silicon
//!   (where the gas holds magnesium), and titanium dioxide, where perovskite takes its titanium
//!   (where it holds calcium; [`substance::aloft`](crate::substance::aloft)).
//! - **A vapour past its critical point** ([`NotFormed::Supercritical`], P14.T52.f): a row's own
//!   curve ends at its critical point, so a vapour whose partial pressure x P is at or above the
//!   critical pressure `p_c` where the profile cools through its critical temperature is
//!   saturated at the first level colder than that, with no crossing: the base's bracket would
//!   converge on the curve's end. When the loop takes such a `Crossing` base, the row leaves as
//!   not formed and draws nothing, its vapour labelled in the gas above and never trapped, as a
//!   bulk gas's. Its dense fluid is the surface partition's (P14.T51.a, with P14.T53.b part 3's
//!   cap) or a supercritical envelope's. A crossing below the critical temperature, and every
//!   [`DeckBound::Bottom`] base, is unchanged; clamping the base below `T_c` is refused, since the
//!   excess would condense at once, the same deck one step colder (decision-n2-critical-decks).
//! - **One phase a deck**, its base's: a deck that rises through its condensate's melting point
//!   (a water deck through 273 K) keeps the phase it formed in, and the grain density of its
//!   base's temperature.
//! - **A deck below the column** ([`DeckBound::Bottom`]): a vapour already saturated at the
//!   column's bottom is trapped from there, its vapour held at saturation, but the condensate
//!   lofted from its base below the column is not counted; its column is what condenses within
//!   the column. A caller wanting a deep deck's own column takes its bottom deeper.
//! - **The stable layers' shorter mixing length**: Ackerman and Marley's L = H max(0.1, Γ ÷ `Γ_ad`)
//!   thins a deck above a radiative layer, and their Jupiter's ammonia column, 52 g m⁻², is about
//!   half of what L = H gives here (105 g m⁻² on the tests' profile).
//! - **The mixing** is the column's stated `K_zz` ([`MixingColumn::eddy_diffusion`]), constant;
//!   Ackerman and Marley's K from the convective flux (their eq. 5) is the caller's to state.
//! - **The co-reactants' own depletion** in a law's metallicity term: the papers' C \[M\] assumes
//!   every co-reactant enriched uniformly, which a deck below (forsterite's oxygen) breaks a little
//!   ([`substance::aloft`](crate::substance::aloft)).
//! - **Enstatite's magnesium**: forsterite takes the magnesium above its base, so enstatite's draw
//!   of it is held at what is left, about none, while its mass counts its formula's (Diamondback's
//!   and Virga's convention): the silicates' total is about 20% above their stoichiometric mass at
//!   solar Mg/Si.
//! - **The surface's coupled condensables** (a water and ammonia liquid, an ocean's solutes) are
//!   P14.T51.g's; this loop holds only decks aloft, each row by its own law.
//! - **Titan's ethane**: no air carries C₂H₆, which nothing produces yet, so no ethane deck forms;
//!   Titan's ethane cloud and seas are P14.T55.m's.
//!
//! # Determinism and cost
//!
//! Every step count is fixed, every sum is in a fixed order (the decks' in placing order, the gas's
//! in its given order) and every transcendental is [`math`]'s, so the result is the same bits on
//! every target. A deck is placed per round, after its base and every base it can move are
//! re-found; a base moves only when a deck that shares a vapour with it is placed. Its cost is
//! the profile's evaluations: one per scan level, a few dozen per base, and [`SUBSTEPS`] per scan
//! level of each deck's column, unmeasured on a quiet machine (P14.T33.a's `body_detail`).
//!
//! Nothing on the record reads it until P14.T54.b wires P14.T52.d's envelopes and P14.T24.c's
//! aerosols, so no generated output moves with it.

use std::error::Error;
use std::fmt;

use super::quench::MixingColumn;
use crate::math;
use crate::substance::aerosol::{
    BuildSizeDistributionError, ExtinctionBasis, ResolveAerosolError, SizeDistribution,
    aerosol_optics, density_variant,
};
use crate::substance::aloft::{LawAloft, SaturationLaw, Vapour, laws_aloft};
use crate::substance::{CondensedPhase, MOLAR_GAS_CONSTANT, SubstanceId, SubstanceKey};
use crate::units::consts::BOLTZMANN_CONSTANT;
use crate::units::{Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, Pascals};

/// The scan's step in ln p: the column is evaluated at levels this far apart, from its bottom to
/// its top, where each base is bracketed and each deck's vapour is kept.
pub const SCAN_STEP_LN_P: f64 = 0.05;

/// The most scan levels a column takes: a column deeper than [`SCAN_STEP_LN_P`] times this in
/// ln p (about 435 decades of pressure) is scanned in longer steps.
const MAX_LEVELS: usize = 20_000;

/// The integration steps of a deck's column to each scan step.
pub const SUBSTEPS: u32 = 5;

/// The Newton steps that refine each base inside its bracket.
pub const BASE_STEPS: u32 = 40;

/// The share of the gas at or above which a condensable is its bulk, labelled
/// [`NotFormed::BulkGas`] rather than trapped (the [module](self)'s documentation).
pub const BULK_BOUND: f64 = 0.5;

/// The share of the vapour below that a deck's condensate must still hold at a level for the level
/// to lie within the deck: its top is the shallowest such level.
pub const TOP_FRACTION: f64 = 1e-6;

/// The bisections that find each level's `r_w`, in ln r over [`RADIUS_RANGE_M`].
const RADIUS_BISECTIONS: u32 = 48;

/// The radii `r_w` is sought between, m: a nanometre to ten centimetres.
const RADIUS_RANGE_M: (f64, f64) = (1e-9, 0.1);

/// The narrowest interval, as a ratio of radii, over which α is fitted: Ackerman and Marley's
/// floor of 1.1 on `σ_g` for the fit (their §2.2), which Virga's `sig_alpha` keeps.
const ALPHA_FIT_WIDTH: f64 = 1.1;

/// Grams per kilogram.
const GRAMS_PER_KILOGRAM: f64 = 1_000.0;

/// Ackerman and Marley's two free parameters of a deck: its sedimentation efficiency `f_sed`
/// and the geometric width `σ_g` of its log-normal.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Sedimentation {
    efficiency: f64,
    width: f64,
}

impl Sedimentation {
    /// Ackerman and Marley's baseline, `f_sed` = 3 and `σ_g` = 2 (their Table 1; "leaning toward
    /// values > 1"; Jupiter's ammonia fits 1–3 by Carlson et al. 1994's cloud scale height).
    /// Diamondback spans `f_sed` 1–8 at `σ_g` 2 (Morley et al. 2024), Morley et al. 2012 fit
    /// L dwarfs at 2 ± 1 and T dwarfs' sulphides at 4–5, Virga's code suggests 0.1–1 for hot
    /// Jupiters, and Morley et al. 2013 fit the sub-Neptune GJ 1214b only at 0.1.
    pub const BASELINE: Self = Self {
        efficiency: 3.0,
        width: 2.0,
    };

    /// A deck's sedimentation of efficiency `efficiency` (`f_sed`) and log-normal width `width`
    /// (`σ_g`).
    ///
    /// # Errors
    ///
    /// [`BuildSedimentationError`] unless `f_sed` is finite and positive and `σ_g` lies within
    /// [`WIDTH_RANGE`](Self::WIDTH_RANGE).
    pub fn new(efficiency: f64, width: f64) -> Result<Self, BuildSedimentationError> {
        if !(efficiency.is_finite() && efficiency > 0.0) {
            return Err(BuildSedimentationError::Efficiency { efficiency });
        }
        let (narrowest, widest) = Self::WIDTH_RANGE;
        if !(narrowest..=widest).contains(&width) {
            return Err(BuildSedimentationError::Width { width });
        }
        Ok(Self { efficiency, width })
    }

    /// The widths `σ_g` a deck may take: those whose effective variance, exp(ln²`σ_g`) − 1, lies
    /// within P14.T49.e's log-normal grid, 0.04–0.65 (0.051 at 1.25, 0.617 at 2), so that its
    /// τ(550) is the table's.
    pub const WIDTH_RANGE: (f64, f64) = (1.25, 2.0);

    /// `f_sed`, the ratio of the mass-weighted sedimentation velocity to w*.
    #[must_use]
    pub const fn efficiency(&self) -> f64 {
        self.efficiency
    }

    /// `σ_g`, the log-normal's geometric standard deviation.
    #[must_use]
    pub const fn width(&self) -> f64 {
        self.width
    }
}

/// [`Sedimentation::new`] refused its parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildSedimentationError {
    /// `f_sed` is not finite and positive.
    Efficiency {
        /// The value given.
        efficiency: f64,
    },
    /// `σ_g` is outside [`Sedimentation::WIDTH_RANGE`].
    Width {
        /// The value given.
        width: f64,
    },
}

impl fmt::Display for BuildSedimentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Efficiency { efficiency } => {
                write!(
                    f,
                    "sedimentation efficiency {efficiency} is not finite and positive"
                )
            }
            Self::Width { width } => {
                write!(f, "log-normal width {width} is outside 1.25 to 2")
            }
        }
    }
}

impl Error for BuildSedimentationError {}

/// The Lennard-Jones constants of a gas, by which its viscosity and mean free path are found.
///
/// The fields are plain data with no invariant between them, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionConstants {
    /// The gas's key.
    pub key: &'static str,
    /// The collision diameter d, m.
    pub diameter_m: f64,
    /// The well depth ε over Boltzmann's constant, kelvin.
    pub well_depth_k: f64,
}

/// The Lennard-Jones constants of the gases that can be a column's most abundant: R. A. Svehla,
/// "Estimated viscosities and thermal conductivities of gases at high temperatures", NASA TR R-132
/// (1962), Table I(a), each fitted to its gas's viscosity but helium's, which is from
/// quantum-mechanical formulas (Svehla's method). H₂'s are also Ackerman and Marley's (2001,
/// Appendix B: 2.827 × 10⁻⁸ cm and 59.7 K). A closed set, for the composition audit: a gas whose
/// most abundant species is not here has no mode size.
pub const COLLISION_CONSTANTS: [CollisionConstants; 11] = [
    collision("H2", 2.827, 59.7),
    collision("He", 2.551, 10.22),
    collision("H2O", 2.641, 809.1),
    collision("CH4", 3.758, 148.6),
    collision("N2", 3.798, 71.4),
    collision("O2", 3.467, 106.7),
    collision("CO2", 3.941, 195.2),
    collision("Ar", 3.542, 93.3),
    collision("Ne", 2.820, 32.8),
    collision("CO", 3.690, 91.7),
    collision("SO2", 4.112, 335.4),
];

/// The constants of `key`, of diameter `angstroms` Å and well depth `well_k` kelvin.
#[must_use]
const fn collision(key: &'static str, angstroms: f64, well_k: f64) -> CollisionConstants {
    CollisionConstants {
        key,
        diameter_m: angstroms * 1e-10,
        well_depth_k: well_k,
    }
}

/// Where a deck's base lies in the column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeckBound {
    /// Within the column, where the vapour reaches saturation.
    Crossing,
    /// At the column's bottom: the vapour is saturated there already, so the deck's base lies at
    /// or below it.
    Bottom,
}

/// Why a row with a law aloft whose vapour the gas holds forms no deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NotFormed {
    /// The vapour holds at least [`BULK_BOUND`] of the gas: its condensation is the profile's,
    /// not modelled here.
    BulkGas,
    /// The law gives no deck of a gas that holds the vapour of the condensate named, which takes
    /// its limiting vapour first.
    Displaced {
        /// The condensate that displaces it.
        by: SubstanceKey,
    },
    /// Its vapour's partial pressure reaches its critical pressure where the profile cools through
    /// its critical temperature. It passes from supercritical fluid to compressed liquid around
    /// the critical point, crossing no saturation curve, so no deck has a base. The dense fluid is
    /// not modelled here: it is the surface partition's, or a supercritical envelope's (P14.T53.b;
    /// P14.T52.f, decision-n2-critical-decks §3.2).
    Supercritical,
}

/// What a mode's τ(550) rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OpticalDepthBasis {
    /// P14.T49.e's mass extinction, on its basis.
    Table(ExtinctionBasis),
    /// Ackerman and Marley's geometric scatterers (their eq. 16, `Q_ext` = 2), for a mode larger
    /// than the extinction table's grid, where `Q̄_ext` tends to 2.
    GeometricLimit,
}

/// A mode's column optical depth at 550 nm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpticalDepth {
    value: f64,
    basis: OpticalDepthBasis,
}

impl OpticalDepth {
    /// τ(550).
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// What it rests on.
    #[must_use]
    pub const fn basis(&self) -> OpticalDepthBasis {
        self.basis
    }
}

/// A mode's size could not be found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeModeError {
    /// The condensate has no grain density in its phase at a level of its deck.
    NoDensity {
        /// The condensate.
        key: SubstanceKey,
        /// Its phase.
        phase: CondensedPhase,
        /// The level's temperature.
        temperature: Kelvin,
    },
    /// The gas's most abundant species has no [`COLLISION_CONSTANTS`], so its viscosity is not
    /// modelled.
    NoCollisionConstants {
        /// The species.
        key: SubstanceKey,
    },
    /// The deck's column holds no condensate at any scan level (a deck thinner than a step).
    NoCondensate,
    /// The levels' sizes give no log-normal: an effective radius that is not finite and positive.
    Distribution(BuildSizeDistributionError),
}

impl fmt::Display for SizeModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDensity {
                key,
                phase,
                temperature,
            } => write!(
                f,
                "{key} has no grain density as a {} at {} K",
                phase_name(*phase),
                temperature.value()
            ),
            Self::NoCollisionConstants { key } => {
                write!(
                    f,
                    "{key} has no collision constants, so the gas's viscosity is not modelled"
                )
            }
            Self::NoCondensate => write!(f, "the deck holds no condensate at any scan level"),
            Self::Distribution(error) => write!(f, "the mode's log-normal: {error}"),
        }
    }
}

impl Error for SizeModeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Distribution(error) => Some(error),
            Self::NoDensity { .. } | Self::NoCollisionConstants { .. } | Self::NoCondensate => None,
        }
    }
}

/// A mode's τ(550) could not be found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolveOpticalDepthError {
    /// Its size is not modelled.
    Size(SizeModeError),
    /// Its optics are not: P14.T49.e refuses its material, phase, variant or size.
    Aerosol(ResolveAerosolError),
}

impl fmt::Display for ResolveOpticalDepthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size(error) => write!(f, "the mode's size: {error}"),
            Self::Aerosol(error) => write!(f, "the mode's optics: {error}"),
        }
    }
}

impl Error for ResolveOpticalDepthError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Size(error) => Some(error),
            Self::Aerosol(error) => Some(error),
        }
    }
}

/// A mode's vertical profile (P14.T24.c): its base and its top, between which its mixing ratio
/// goes as (p ÷ `p_base`)^f, f its exponent, 0 for a well-mixed layer (P14.T24.g's modes). The
/// loop's modes take their deck's base and top and its `f_sed`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ModeProfile {
    base: Pascals,
    top: Pascals,
    exponent: f64,
}

impl ModeProfile {
    /// The profile from `base` up to `top`, its mixing ratio going as (p ÷ `p_base`)^`exponent`.
    ///
    /// # Errors
    ///
    /// [`BuildModeError::Profile`] unless the top is finite and positive and no deeper than the
    /// base, which is finite, and the exponent is finite and non-negative.
    pub fn new(base: Pascals, top: Pascals, exponent: f64) -> Result<Self, BuildModeError> {
        let (b, t) = (base.value(), top.value());
        if !(t.is_finite() && t > 0.0 && b.is_finite() && t <= b && exponent.is_finite())
            || exponent < 0.0
        {
            return Err(BuildModeError::Profile {
                base,
                top,
                exponent,
            });
        }
        Ok(Self {
            base,
            top,
            exponent,
        })
    }

    /// The base's pressure.
    #[must_use]
    pub const fn base(&self) -> Pascals {
        self.base
    }

    /// The top's pressure.
    #[must_use]
    pub const fn top(&self) -> Pascals {
        self.top
    }

    /// The exponent f of the mixing ratio above the base.
    #[must_use]
    pub const fn exponent(&self) -> f64 {
        self.exponent
    }
}

/// [`Mode::new`] or [`ModeProfile::new`] refused its inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildModeError {
    /// The profile's top is not finite and positive and no deeper than its finite base, or its
    /// exponent is not finite and non-negative.
    Profile {
        /// The base given.
        base: Pascals,
        /// The top given.
        top: Pascals,
        /// The exponent given.
        exponent: f64,
    },
    /// The column mass is not finite and non-negative.
    ColumnMass {
        /// The value given.
        column_mass: KilogramsPerSquareMetre,
    },
    /// The coverage is not within 0 to 1.
    Coverage {
        /// The value given.
        coverage: f64,
    },
    /// The temperature its optics are taken at is not finite and positive.
    Temperature {
        /// The value given.
        temperature: Kelvin,
    },
    /// A rule's τ(550) is not finite and non-negative (P14.T24.c's rule modes).
    OpticalDepth {
        /// The value given, dimensionless.
        optical_depth: f64,
    },
}

impl fmt::Display for BuildModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile {
                base,
                top,
                exponent,
            } => write!(
                f,
                "a profile from {} Pa up to {} Pa of exponent {exponent} is not one",
                base.value(),
                top.value()
            ),
            Self::ColumnMass { column_mass } => write!(
                f,
                "column mass {} kg m⁻² is not finite and non-negative",
                column_mass.value()
            ),
            Self::Coverage { coverage } => write!(f, "coverage {coverage} is not within 0 to 1"),
            Self::Temperature { temperature } => write!(
                f,
                "temperature {} K is not finite and positive",
                temperature.value()
            ),
            Self::OpticalDepth { optical_depth } => write!(
                f,
                "optical depth {optical_depth} is not finite and non-negative"
            ),
        }
    }
}

impl Error for BuildModeError {}

/// A deck's particles: one population of one material in one phase, with its size, its column,
/// its τ(550) and its vertical profile, the shape P14.T24.c's aerosol inventory carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mode {
    material: SubstanceId,
    phase: CondensedPhase,
    deck: SubstanceId,
    size: Result<SizeDistribution, SizeModeError>,
    column_mass: KilogramsPerSquareMetre,
    optical_depth: Result<OpticalDepth, ResolveOpticalDepthError>,
    profile: ModeProfile,
    coverage: f64,
}

impl Mode {
    /// A mode of `material` in `phase`, of deck `deck` (by its condensate), of size `size` and
    /// column mass `column_mass`, between its `profile`'s base and top, covering `coverage` of
    /// the sky, its τ(550) from P14.T49.e's mass extinction at `temperature` (or above the
    /// table's grid the geometric limit), as the loop's are: for a rule's modes (P14.T24.g's acid
    /// modes, well mixed between fractions of the deck's span).
    ///
    /// # Errors
    ///
    /// [`BuildModeError`] for a column mass that is not finite and non-negative, a coverage
    /// outside 0 to 1, or a temperature that is not finite and positive. A τ(550) that is not
    /// modelled is the mode's [`optical_depth`](Self::optical_depth), not an error.
    #[expect(
        clippy::too_many_arguments,
        reason = "a mode's eight columns, each checked, as P14.T24.c lists them"
    )]
    pub fn new(
        material: SubstanceId,
        phase: CondensedPhase,
        deck: SubstanceId,
        size: SizeDistribution,
        column_mass: KilogramsPerSquareMetre,
        profile: ModeProfile,
        coverage: f64,
        temperature: Kelvin,
    ) -> Result<Self, BuildModeError> {
        let mass = column_mass.value();
        if !(mass.is_finite() && mass >= 0.0) {
            return Err(BuildModeError::ColumnMass { column_mass });
        }
        if !(0.0..=1.0).contains(&coverage) {
            return Err(BuildModeError::Coverage { coverage });
        }
        let t = temperature.value();
        if !(t.is_finite() && t > 0.0) {
            return Err(BuildModeError::Temperature { temperature });
        }
        Ok(Self {
            material,
            phase,
            deck,
            size: Ok(size),
            column_mass,
            optical_depth: optical_depth(material, phase, Ok(size), column_mass, t),
            profile,
            coverage,
        })
    }

    /// Its material, a registry row.
    #[must_use]
    pub const fn material(&self) -> SubstanceId {
        self.material
    }

    /// Its phase: never a vapour.
    #[must_use]
    pub const fn phase(&self) -> CondensedPhase {
        self.phase
    }

    /// The deck it belongs to, by the deck's condensate.
    #[must_use]
    pub const fn deck(&self) -> SubstanceId {
        self.deck
    }

    /// Its size distribution, Ackerman and Marley's log-normal by its effective radius and
    /// variance, or why it is not modelled.
    ///
    /// # Errors
    ///
    /// [`SizeModeError`] where the size is not modelled.
    pub const fn size(&self) -> Result<SizeDistribution, SizeModeError> {
        self.size
    }

    /// Its column mass.
    #[must_use]
    pub const fn column_mass(&self) -> KilogramsPerSquareMetre {
        self.column_mass
    }

    /// Its τ(550), or why it is not modelled.
    ///
    /// # Errors
    ///
    /// [`ResolveOpticalDepthError`] where it is not modelled.
    pub const fn optical_depth(&self) -> Result<OpticalDepth, ResolveOpticalDepthError> {
        self.optical_depth
    }

    /// Its vertical profile.
    #[must_use]
    pub const fn profile(&self) -> ModeProfile {
        self.profile
    }

    /// The share of the sky it covers, the caller's: the snapshot path's cloud fraction, or 1.
    #[must_use]
    pub const fn coverage(&self) -> f64 {
        self.coverage
    }
}

/// A condensate's deck: where it forms, its column, the vapour it leaves above, and its modes.
#[derive(Debug, Clone, PartialEq)]
pub struct Deck {
    condensate: SubstanceId,
    phase: CondensedPhase,
    base: Pascals,
    base_temperature: Kelvin,
    bound: DeckBound,
    top: Pascals,
    vapour_below: f64,
    vapour_above: f64,
    sedimentation: f64,
    column_mass: KilogramsPerSquareMetre,
    extrapolated: bool,
    modes: Vec<Mode>,
    /// The deck's own vapour above its base: ln `q_v` at each scan level from the first above the
    /// base, `first`, to the column's top, the levels `step` apart in ln p from `bottom`.
    base_ln_p: f64,
    bottom_ln_p: f64,
    step_ln_p: f64,
    first: usize,
    vapour_ln: Vec<f64>,
}

impl Deck {
    /// The condensate.
    #[must_use]
    pub const fn condensate(&self) -> SubstanceId {
        self.condensate
    }

    /// Its phase at the base: liquid at or above its row's first melt (its triple point for a row
    /// with a phase diagram), solid below.
    #[must_use]
    pub const fn phase(&self) -> CondensedPhase {
        self.phase
    }

    /// The base's pressure.
    #[must_use]
    pub const fn base(&self) -> Pascals {
        self.base
    }

    /// The base's temperature, on the profile.
    #[must_use]
    pub const fn base_temperature(&self) -> Kelvin {
        self.base_temperature
    }

    /// Where the base lies in the column.
    #[must_use]
    pub const fn bound(&self) -> DeckBound {
        self.bound
    }

    /// The top's pressure: the shallowest level at which the condensate holds [`TOP_FRACTION`] of
    /// the vapour below, or the column's top.
    #[must_use]
    pub const fn top(&self) -> Pascals {
        self.top
    }

    /// The limiting vapour's mole fraction below the base: atoms of the element per molecule of
    /// gas for an element's vapour (for a product law, the scarcer reactant's), times the law's
    /// vapour share.
    #[must_use]
    pub const fn vapour_below(&self) -> f64 {
        self.vapour_below
    }

    /// The limiting vapour's mole fraction at the top, which the gas above keeps: the cold trap's.
    #[must_use]
    pub const fn vapour_above(&self) -> f64 {
        self.vapour_above
    }

    /// The deck's `f_sed`, the exponent of its profile above the base.
    #[must_use]
    pub const fn sedimentation(&self) -> f64 {
        self.sedimentation
    }

    /// Its condensate's column mass.
    #[must_use]
    pub const fn column_mass(&self) -> KilogramsPerSquareMetre {
        self.column_mass
    }

    /// Whether its law is applied outside the metallicities or the pressures it was fitted over
    /// ([`Fitted`](crate::substance::aloft::Fitted)).
    #[must_use]
    pub const fn extrapolated(&self) -> bool {
        self.extrapolated
    }

    /// Its modes: one, Ackerman and Marley's log-normal.
    #[must_use]
    pub fn modes(&self) -> &[Mode] {
        &self.modes
    }

    /// The deck's own vapour's mole fraction at `pressure`: the vapour below at and below the base,
    /// and above it what the column leaves; `None` for a pressure that is not finite and positive.
    #[must_use]
    pub fn vapour_at(&self, pressure: Pascals) -> Option<f64> {
        let p = pressure.value();
        (p.is_finite() && p > 0.0).then(|| math::exp(self.ln_vapour_at(ln(p))))
    }

    /// The condensing vapour the deck has taken from the gas at ln p `u`: none at or below its
    /// base. [`Condensation::remaining`] and the loop's own cold traps both read it.
    #[must_use]
    fn removal(&self, u: f64) -> f64 {
        if u >= self.base_ln_p {
            return 0.0;
        }
        (self.vapour_below - math::exp(self.ln_vapour_at(u))).max(0.0)
    }

    /// ln `q_v` at ln p `u`, through [`Grid`]'s levels, linear in ln p.
    #[must_use]
    fn ln_vapour_at(&self, u: f64) -> f64 {
        let below = math::ln(self.vapour_below);
        if u >= self.base_ln_p || self.vapour_ln.is_empty() {
            return below;
        }
        let step = self.step_ln_p;
        let first_u = self.bottom_ln_p - step * index_f64(self.first);
        if u >= first_u {
            let span = self.base_ln_p - first_u;
            let t = if span > 0.0 {
                (self.base_ln_p - u) / span
            } else {
                1.0
            };
            return below + t * (self.vapour_ln[0] - below);
        }
        let offset = (first_u - u) / step;
        let last = self.vapour_ln.len() - 1;
        let k = offset_index(offset, last);
        if k >= last {
            return self.vapour_ln[last];
        }
        let t = offset - index_f64(k);
        self.vapour_ln[k] + t * (self.vapour_ln[k + 1] - self.vapour_ln[k])
    }
}

/// [`Condensation::derive`] refused its inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveCondensationError {
    /// A species' mole fraction is not finite and non-negative.
    Amount {
        /// The species.
        id: SubstanceId,
        /// The value given.
        amount: f64,
    },
    /// A species is given twice.
    Repeated {
        /// The species.
        id: SubstanceId,
    },
    /// A field of the column is out of its range: gravity, molar mass, eddy diffusion and
    /// metallicity finite and positive, and the top finite, positive and above the bottom.
    Column {
        /// The field, by its name.
        field: &'static str,
        /// The value given.
        value: f64,
    },
    /// The coverage is not within 0 to 1.
    Coverage {
        /// The value given.
        coverage: f64,
    },
    /// The profile's temperature at a level is not finite and positive.
    Profile {
        /// The level.
        pressure: Pascals,
        /// The temperature the profile gave.
        temperature: Kelvin,
    },
}

impl fmt::Display for DeriveCondensationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Amount { id, amount } => write!(
                f,
                "{}'s mole fraction {amount} is not finite and non-negative",
                id.substance().key()
            ),
            Self::Repeated { id } => write!(f, "{} is given twice", id.substance().key()),
            Self::Column { field, value } => {
                write!(f, "the column's {field} {value} is out of its range")
            }
            Self::Coverage { coverage } => write!(f, "coverage {coverage} is not within 0 to 1"),
            Self::Profile {
                pressure,
                temperature,
            } => write!(
                f,
                "the profile gives {} K at {} Pa, not finite and positive",
                temperature.value(),
                pressure.value()
            ),
        }
    }
}

impl Error for DeriveCondensationError {}

/// A column's condensation: its decks, hottest base first, and the rows that form none for a
/// stated reason (the [module](self)'s documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct Condensation {
    gas: Vec<(SubstanceId, f64)>,
    decks: Vec<Deck>,
    draws: Vec<Vec<(Vapour, f64)>>,
    not_formed: Vec<(SubstanceId, NotFormed)>,
}

/// The scan levels: ln p from the column's bottom up to its top, evenly, with the profile's
/// temperature at each.
struct Grid {
    bottom: f64,
    step: f64,
    temperatures: Vec<f64>,
}

impl Grid {
    /// ln p of level `i`.
    #[must_use]
    fn u(&self, i: usize) -> f64 {
        self.bottom - self.step * index_f64(i)
    }

    /// The levels' count.
    #[must_use]
    fn len(&self) -> usize {
        self.temperatures.len()
    }

    /// The temperature at ln p `ln_p` between the levels, ln T linear in ln p (a power law
    /// between them, as P14.T24.e's nodes are read).
    #[must_use]
    fn temperature(&self, ln_p: f64) -> f64 {
        let last = self.len() - 1;
        let offset = ((self.bottom - ln_p) / self.step).max(0.0);
        let below = offset_index(offset, last);
        if below >= last {
            return self.temperatures[last];
        }
        let fraction = offset - index_f64(below);
        let deep = math::ln(self.temperatures[below]);
        let shallow = math::ln(self.temperatures[below + 1]);
        math::exp(deep + fraction * (shallow - deep))
    }
}

/// A row's law in the loop: its vapours' places in the pool and their ratios per limiting vapour.
struct Candidate {
    id: SubstanceId,
    law: LawAloft,
    /// The condensate's molar mass, g mol⁻¹.
    molar_mass: f64,
    /// The limiting vapour's place, and for a product law the second reactant's.
    limiting: usize,
    second: Option<usize>,
    /// Every vapour the deck draws on, the limiting one first, with its count over the limiting
    /// one's.
    draws: Vec<(usize, f64)>,
    /// The base in the gas the placed decks leave, `None` if it forms none, current while
    /// `fresh`.
    base: Option<Base>,
    fresh: bool,
}

/// A base found.
#[derive(Clone, Copy)]
struct Base {
    u: f64,
    temperature: f64,
    bound: DeckBound,
    /// The temperature the decks are taken in order of, hottest first: the base's, or for a
    /// deck below the column its condensation temperature at the column's bottom, at which its
    /// base below would lie.
    order: f64,
}

/// A placed deck's draw on the pool: its removal of its condensing vapour by level, through its
/// [`Deck`]'s `removal`, and its vapours with their ratios.
struct Placed {
    deck: Deck,
    draws: Vec<(usize, f64)>,
}

/// The vapours the laws draw on, with the deep gas's amount of each.
struct Pool {
    vapours: Vec<Vapour>,
    deep: Vec<f64>,
}

impl Pool {
    /// The place of `vapour`, added with its amount in `gas` if it is new.
    fn place(&mut self, vapour: Vapour, gas: &[(SubstanceId, f64)]) -> usize {
        if let Some(i) = self.vapours.iter().position(|v| *v == vapour) {
            return i;
        }
        self.vapours.push(vapour);
        self.deep.push(amount(vapour, gas));
        self.vapours.len() - 1
    }

    /// The gas's amount of each vapour at ln p `u` once the decks `placed` have drawn on it
    /// ([`draw`]).
    #[must_use]
    fn left(&self, u: f64, placed: &[Placed]) -> Vec<f64> {
        let mut left = self.deep.clone();
        for p in placed {
            draw(&mut left, p.deck.removal(u), &p.draws);
        }
        left
    }
}

/// One deck's draw on the gas `left` (each vapour's amount, by place), of its condensing vapour's
/// loss `removal` and its vapours `draws` (the limiting one first, each with its count over the
/// limiting one's): the limiting vapour gives what it can of the loss, and each other vapour its
/// count's share of what the limiting one gave, each held at what is left of it, so that every
/// vapour's amount and draws sum to the deep gas's.
fn draw(left: &mut [f64], removal: f64, draws: &[(usize, f64)]) {
    let Some((&(limiting, _), others)) = draws.split_first() else {
        return;
    };
    let given = removal.min(left[limiting]);
    left[limiting] -= given;
    for &(place, ratio) in others {
        let taken = (ratio * given).min(left[place]);
        left[place] -= taken;
    }
}

/// The amount of `vapour` in `gas`: its mole fraction for a species, its atoms per molecule of
/// gas for an element, summed in the gas's order.
#[must_use]
fn amount(vapour: Vapour, gas: &[(SubstanceId, f64)]) -> f64 {
    match vapour {
        Vapour::Species(key) => gas
            .iter()
            .filter(|(id, _)| id.substance().key().as_str() == key.as_str())
            .map(|&(_, x)| x)
            .sum(),
        Vapour::Element(element) => {
            let mut total = 0.0;
            for &(id, x) in gas {
                for &(e, n) in id.substance().elements() {
                    if e == element {
                        total += f64::from(n) * x;
                    }
                }
            }
            total
        }
    }
}

/// The column's inputs, checked.
fn check(column: &MixingColumn, coverage: f64) -> Result<(), DeriveCondensationError> {
    let positive = |field: &'static str, value: f64| {
        if value.is_finite() && value > 0.0 {
            Ok(())
        } else {
            Err(DeriveCondensationError::Column { field, value })
        }
    };
    positive("gravity", column.gravity.value())?;
    positive("molar mass", column.molar_mass_g_per_mol)?;
    positive(
        "eddy diffusion",
        column.eddy_diffusion.square_metres_per_second(),
    )?;
    positive("metallicity", column.metallicity)?;
    positive("bottom", column.bottom.value())?;
    positive("top", column.top.value())?;
    if column.top.value() >= column.bottom.value() {
        return Err(DeriveCondensationError::Column {
            field: "top",
            value: column.top.value(),
        });
    }
    if !(0.0..=1.0).contains(&coverage) {
        return Err(DeriveCondensationError::Coverage { coverage });
    }
    Ok(())
}

impl Condensation {
    /// The condensation of the gas `gas` (each species' mole fraction below the clouds, as P14.T52.a
    /// and P14.T52.b give it at the level the caller picks, or P14.T24.a's fractions at a
    /// surface's ground) along the temperature profile `profile`, T(p), in `column` (its gravity,
    /// mean molar mass, `K_zz`, metallicity as a multiple of solar, and the levels from its
    /// `bottom` up to its `top`), each condensate's deck with the [`Sedimentation`] that
    /// `sedimentation` gives it (P14.T52.d and P14.T24.c state it by class; Ackerman and
    /// Marley's [`Sedimentation::BASELINE`] for all is `|_| Sedimentation::BASELINE`), covering
    /// `coverage` of the sky.
    ///
    /// # Errors
    ///
    /// [`DeriveCondensationError`] for a mole fraction that is not finite and non-negative or is
    /// given twice, a column field out of its range, a coverage outside 0 to 1, or a profile
    /// whose temperature at a level is not finite and positive.
    ///
    /// # Examples
    ///
    /// Jupiter's ammonia condenses near 0.7 bar, above its ammonium hydrosulphide at about 2 bar:
    ///
    /// ```
    /// use hyperion_sim::math;
    /// use hyperion_sim::planetary::chem::condense::{Condensation, Sedimentation};
    /// use hyperion_sim::planetary::chem::quench::{EddyDiffusion, MixingColumn};
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let power = math::ln(427.7 / 166.1) / math::ln(22.0);
    /// let jupiter = |p: Pascals| Kelvin::new(166.1 * math::powf(p.value() / 1e5, power));
    /// let [h2, he, nh3, h2s] = ["H2", "He", "NH3", "H2S"].map(|k| by_key(k).ok_or("a row"));
    /// let gas = [(h2?, 0.862), (he?, 0.136), (nh3?, 3.4e-4), (h2s?, 1.03e-4)];
    /// let column = MixingColumn {
    ///     gravity: MetresPerSecondSquared::new(24.79),
    ///     molar_mass_g_per_mol: 2.3,
    ///     eddy_diffusion: EddyDiffusion::from_square_metres_per_second(2e4),
    ///     metallicity: 3.0,
    ///     top: Pascals::new(1e3),
    ///     bottom: Pascals::new(1e7),
    /// };
    /// let baseline = |_| Sedimentation::BASELINE;
    /// let clouds = Condensation::derive(gas, jupiter, &column, baseline, 1.0)?;
    /// let deck = |key: &str| {
    ///     let id = by_key(key)?;
    ///     clouds.decks().iter().find(|d| d.condensate() == id)
    /// };
    /// let ammonia = deck("NH3").ok_or("an ammonia deck")?.base().value() / 1e5;
    /// let salt = deck("NH4SH").ok_or("a salt deck")?.base().value() / 1e5;
    /// assert!((0.5..1.0).contains(&ammonia) && (1.5..3.0).contains(&salt));
    /// # Ok(())
    /// # }
    /// ```
    pub fn derive(
        gas: impl IntoIterator<Item = (SubstanceId, f64)>,
        profile: impl Fn(Pascals) -> Kelvin,
        column: &MixingColumn,
        sedimentation: impl Fn(SubstanceId) -> Sedimentation,
        coverage: f64,
    ) -> Result<Self, DeriveCondensationError> {
        Self::derive_in(gas, profile, column, sedimentation, coverage, SUBSTEPS)
    }

    /// [`derive`](Self::derive) with each deck's column integrated in `substeps` steps to each scan
    /// step: [`SUBSTEPS`] there, and others in the tests of its convergence.
    fn derive_in(
        gas: impl IntoIterator<Item = (SubstanceId, f64)>,
        profile: impl Fn(Pascals) -> Kelvin,
        column: &MixingColumn,
        sedimentation: impl Fn(SubstanceId) -> Sedimentation,
        coverage: f64,
        substeps: u32,
    ) -> Result<Self, DeriveCondensationError> {
        check(column, coverage)?;
        let mut mixture: Vec<(SubstanceId, f64)> = Vec::new();
        for (id, x) in gas {
            if !(x.is_finite() && x >= 0.0) {
                return Err(DeriveCondensationError::Amount { id, amount: x });
            }
            if mixture.iter().any(|(seen, _)| *seen == id) {
                return Err(DeriveCondensationError::Repeated { id });
            }
            mixture.push((id, x));
        }
        // The gas in registry order, so that its sums, and the background's ties, do not depend on
        // the order it was given in.
        mixture.sort_by_key(|&(id, _)| id);
        let grid = grid(&profile, column)?;
        let metallicity = math::log10(column.metallicity);
        let mut pool = Pool {
            vapours: Vec::new(),
            deep: Vec::new(),
        };
        let mut not_formed = Vec::new();
        let all: Vec<Candidate> = laws_aloft()
            .filter_map(|(id, law)| candidate(id, law, &mut pool, &mixture))
            .collect();
        let held: Vec<SubstanceKey> = all.iter().map(|c| c.id.substance().key()).collect();
        let mut candidates = Vec::with_capacity(all.len());
        for candidate in all {
            // A law is displaced only where its displacing law can form: quartz in a gas with
            // magnesium, titanium dioxide in one with calcium.
            if let Some(by) = candidate.law.displaced_by()
                && held.contains(&by)
            {
                not_formed.push((candidate.id, NotFormed::Displaced { by }));
                continue;
            }
            if let Vapour::Species(_) = candidate.law.limiting().vapour()
                && pool.deep[candidate.limiting] >= BULK_BOUND
            {
                not_formed.push((candidate.id, NotFormed::BulkGas));
                continue;
            }
            candidates.push(candidate);
        }
        let context = Context {
            column,
            grid: &grid,
            metallicity,
            sedimentation: &sedimentation,
            coverage,
            background: background(&mixture),
            substeps,
        };
        let mut placed: Vec<Placed> = Vec::new();
        loop {
            let mut best: Option<(usize, Base)> = None;
            for (i, candidate) in candidates.iter_mut().enumerate() {
                if !candidate.fresh {
                    candidate.base = find_base(candidate, &pool, &placed, &context);
                    candidate.fresh = true;
                }
                if let Some(base) = candidate.base
                    && best.is_none_or(|(_, hottest)| base.order > hottest.order)
                {
                    best = Some((i, base));
                }
            }
            let Some((i, base)) = best else {
                break;
            };
            let c = candidates.remove(i);
            if past_its_critical_point(&c, base, &pool, &placed) {
                // P14.T52.f: the vapour crosses no saturation curve, so the row forms no deck and
                // draws nothing, and no other candidate's base moves.
                not_formed.push((c.id, NotFormed::Supercritical));
                continue;
            }
            let deck = place(&c, base, &pool, &placed, &context);
            let touched: Vec<usize> = c.draws.iter().map(|&(j, _)| j).collect();
            placed.push(Placed {
                deck,
                draws: c.draws,
            });
            for other in &mut candidates {
                if other.draws.iter().any(|(j, _)| touched.contains(j)) {
                    other.fresh = false;
                }
            }
        }
        let draws = placed
            .iter()
            .map(|p| p.draws.iter().map(|&(j, r)| (pool.vapours[j], r)).collect())
            .collect();
        // The candidates' labels are in registry order already; a row past its critical point
        // joins them as the loop takes its base, hottest first.
        not_formed.sort_by_key(|&(id, _)| id);
        Ok(Self {
            gas: mixture,
            decks: placed.into_iter().map(|p| p.deck).collect(),
            draws,
            not_formed,
        })
    }

    /// The decks, hottest base first: the order the cold traps took them in.
    #[must_use]
    pub fn decks(&self) -> &[Deck] {
        &self.decks
    }

    /// The rows with a law aloft whose vapour the gas holds and that form no deck for a stated
    /// reason, in registry order. A row whose vapour never saturates in the column forms no deck
    /// and is not listed.
    #[must_use]
    pub fn not_formed(&self) -> &[(SubstanceId, NotFormed)] {
        &self.not_formed
    }

    /// The gas's amount of `vapour` at `pressure` once every deck below it has drawn on it, in the
    /// decks' order: each deck's limiting vapour gives what it can of the deck's loss, and each
    /// other vapour its share of what that gave, each held at what is left. A species' mole
    /// fraction, or an element's atoms per molecule of gas. With what the decks have drawn, it sums to the gas's amount
    /// below them, to rounding. `None` for a pressure that is not finite and positive.
    #[must_use]
    pub fn remaining(&self, vapour: Vapour, pressure: Pascals) -> Option<f64> {
        let p = pressure.value();
        if !(p.is_finite() && p > 0.0) {
            return None;
        }
        let u = ln(p);
        let mut vapours: Vec<Vapour> = vec![vapour];
        let mut places: Vec<Vec<(usize, f64)>> = Vec::with_capacity(self.decks.len());
        for draws in &self.draws {
            let mut deck_places = Vec::with_capacity(draws.len());
            for &(drawn, ratio) in draws {
                let place = vapours.iter().position(|v| *v == drawn).unwrap_or_else(|| {
                    vapours.push(drawn);
                    vapours.len() - 1
                });
                deck_places.push((place, ratio));
            }
            places.push(deck_places);
        }
        let mut left: Vec<f64> = vapours.iter().map(|&v| amount(v, &self.gas)).collect();
        for (deck, draws) in self.decks.iter().zip(&places) {
            draw(&mut left, deck.removal(u), draws);
        }
        Some(left[0])
    }
}

/// What every base and deck of one derivation reads.
struct Context<'a> {
    column: &'a MixingColumn,
    grid: &'a Grid,
    metallicity: f64,
    sedimentation: &'a dyn Fn(SubstanceId) -> Sedimentation,
    coverage: f64,
    background: Option<SubstanceId>,
    /// The integration steps of a deck's column to each scan step ([`SUBSTEPS`]).
    substeps: u32,
}

/// The scan levels of `column` with `profile`'s temperatures.
fn grid(
    profile: &impl Fn(Pascals) -> Kelvin,
    column: &MixingColumn,
) -> Result<Grid, DeriveCondensationError> {
    let (bottom, top) = (ln(column.bottom.value()), ln(column.top.value()));
    let steps = ((bottom - top) / SCAN_STEP_LN_P).ceil();
    let count = offset_index(steps, MAX_LEVELS).max(1);
    let step = (bottom - top) / index_f64(count);
    let mut temperatures = Vec::with_capacity(count + 1);
    for i in 0..=count {
        let u = bottom - step * index_f64(i);
        let p = Pascals::new(math::exp(u));
        let t = profile(p);
        if !(t.value().is_finite() && t.value() > 0.0) {
            return Err(DeriveCondensationError::Profile {
                pressure: p,
                temperature: t,
            });
        }
        temperatures.push(t.value());
    }
    Ok(Grid {
        bottom,
        step,
        temperatures,
    })
}

/// The loop's candidate for the row `id`'s law `law`, with its vapours placed in `pool`; `None`
/// where the gas lacks any vapour of its reaction, or where the condensate has no molar mass.
#[must_use]
fn candidate(
    id: SubstanceId,
    law: LawAloft,
    pool: &mut Pool,
    gas: &[(SubstanceId, f64)],
) -> Option<Candidate> {
    let molar_mass = id.substance().molar_mass_g_per_mol()?;
    let limiting = law.limiting();
    // Every vapour of the reaction must be in the gas: perovskite needs calcium, enstatite
    // magnesium.
    if amount(limiting.vapour(), gas) <= 0.0
        || law
            .others()
            .iter()
            .any(|other| amount(other.vapour(), gas) <= 0.0)
    {
        return None;
    }
    let first = pool.place(limiting.vapour(), gas);
    let mut draws = vec![(first, 1.0)];
    let mut second = None;
    for other in law.others() {
        let place = pool.place(other.vapour(), gas);
        if matches!(law.law(), SaturationLaw::Product { .. }) {
            second = Some(place);
        }
        draws.push((
            place,
            f64::from(other.count()) / f64::from(limiting.count()),
        ));
    }
    Some(Candidate {
        id,
        law,
        molar_mass,
        limiting: first,
        second,
        draws,
        base: None,
        fresh: false,
    })
}

/// The gas's most abundant species, the first in registry order on a tie: the background whose
/// viscosity sizes the particles.
#[must_use]
fn background(gas: &[(SubstanceId, f64)]) -> Option<SubstanceId> {
    let mut best: Option<(SubstanceId, f64)> = None;
    for &(id, x) in gas {
        if best.is_none_or(|(_, b)| x > b) {
            best = Some((id, x));
        }
    }
    best.map(|(id, _)| id)
}

/// The condensing vapour's mole fraction available at ln p `u` to the candidate `c`, after the
/// decks `placed`: the limiting vapour times its share; for a product law, (the scarcer
/// reactant's, the other's).
#[must_use]
fn available(c: &Candidate, u: f64, pool: &Pool, placed: &[Placed]) -> (f64, f64) {
    let left = pool.left(u, placed);
    let first = left[c.limiting];
    match c.second {
        Some(j) => (first, left[j]),
        None => (c.law.vapour_share() * first, f64::INFINITY),
    }
}

/// How far the candidate's vapour at ln p `ln_p` and `temperature` kelvin is past saturation, in
/// ln: ln(x P) − ln `p_sat` (for a product law, ln(x₁ x₂ P²) − ln K); −∞ where it cannot condense.
#[must_use]
fn supersaturation(
    candidate: &Candidate,
    ln_p: f64,
    temperature: f64,
    pool: &Pool,
    placed: &[Placed],
    context: &Context<'_>,
) -> f64 {
    let (first, second) = available(candidate, ln_p, pool, placed);
    if first <= 0.0 || second <= 0.0 {
        return f64::NEG_INFINITY;
    }
    let Some(ln_sat) = candidate.law.ln_saturation_pressure(
        Kelvin::new(temperature),
        Pascals::new(math::exp(ln_p)),
        context.metallicity,
    ) else {
        return f64::NEG_INFINITY;
    };
    match candidate.second {
        Some(_) => math::ln(first) + math::ln(second) + 2.0 * ln_p - ln_sat,
        None => math::ln(first) + ln_p - ln_sat,
    }
}

/// The candidate `c`'s base in the gas the decks `placed` leave: the scan's first saturated level
/// from the bottom up, refined between it and the level below (the [module](self)'s documentation).
#[must_use]
fn find_base(
    candidate: &Candidate,
    pool: &Pool,
    placed: &[Placed],
    context: &Context<'_>,
) -> Option<Base> {
    let grid = context.grid;
    let at_level = |level: usize| {
        let temperature = grid.temperatures[level];
        supersaturation(candidate, grid.u(level), temperature, pool, placed, context)
    };
    if at_level(0) >= 0.0 {
        // Saturated at the bottom already: of two such decks the one that condenses hotter there
        // lies the deeper below the column, and is taken first.
        let (first, second) = available(candidate, grid.u(0), pool, placed);
        let vapour = match candidate.second {
            Some(_) => first * second,
            None => first,
        };
        let bottom = Pascals::new(math::exp(grid.u(0)));
        let order = candidate
            .law
            .condensation_temperature(bottom, vapour, context.metallicity)
            .map_or(grid.temperatures[0], |t| {
                t.value().max(grid.temperatures[0])
            });
        return Some(Base {
            u: grid.u(0),
            temperature: grid.temperatures[0],
            bound: DeckBound::Bottom,
            order,
        });
    }
    let level = (1..grid.len()).find(|&level| at_level(level) >= 0.0)?;
    // Between the levels the profile is the grid's power law, as the column reads it.
    let exact = |ln_p: f64| {
        let temperature = grid.temperature(ln_p);
        supersaturation(candidate, ln_p, temperature, pool, placed, context)
    };
    // The bracket: `deep` unsaturated, `shallow` saturated (ln p falls upward).
    let (mut deep, mut shallow) = (grid.u(level - 1), grid.u(level));
    let mut ln_p = f64::midpoint(deep, shallow);
    for _ in 0..BASE_STEPS {
        let past = exact(ln_p);
        if past >= 0.0 {
            shallow = ln_p;
        } else {
            deep = ln_p;
        }
        let slope = (exact(ln_p + DIFFERENCE_STEP) - exact(ln_p - DIFFERENCE_STEP))
            / (2.0 * DIFFERENCE_STEP);
        let newton = ln_p - past / slope;
        ln_p = if newton.is_finite() && newton > shallow && newton < deep {
            newton
        } else {
            f64::midpoint(deep, shallow)
        };
    }
    let temperature = grid.temperature(ln_p);
    Some(Base {
        u: ln_p,
        temperature,
        bound: DeckBound::Crossing,
        order: temperature,
    })
}

/// Whether the candidate `c`'s base `base`, in the gas the decks `placed` leave, is the end of its
/// row's own curve rather than a crossing of it (P14.T52.f; the [module](self)'s "What is not
/// modelled"): a [`DeckBound::Crossing`] base of a [`SaturationLaw::Diagram`] law whose vapour's
/// partial pressure x P there is at or above the diagram's critical pressure, which the curve
/// reaches only at its critical point, so that no genuine crossing meets it. A closed-form law
/// has no such end, and a [`DeckBound::Bottom`] base is left as it is.
#[must_use]
fn past_its_critical_point(c: &Candidate, base: Base, pool: &Pool, placed: &[Placed]) -> bool {
    let Some(critical) = c.law.critical_pressure() else {
        return false;
    };
    match base.bound {
        DeckBound::Bottom => false,
        DeckBound::Crossing => {
            let (vapour, _) = available(c, base.u, pool, placed);
            vapour * math::exp(base.u) >= critical.value()
        }
    }
}

/// The step in ln p of the base's Newton slopes, by central difference.
const DIFFERENCE_STEP: f64 = 1e-7;

/// The saturated mole fraction of the deck's condensing vapour at ln p `ln_p` and `temperature`:
/// `p_sat` ÷ P, or for a product law the scarcer reactant's that holds the product at K with the
/// other at its excess `excess` over it; +∞ where it cannot condense.
#[must_use]
fn saturated(
    law: &LawAloft,
    ln_p: f64,
    temperature: f64,
    metallicity: f64,
    excess: Option<f64>,
) -> f64 {
    let Some(ln_sat) = law.ln_saturation_pressure(
        Kelvin::new(temperature),
        Pascals::new(math::exp(ln_p)),
        metallicity,
    ) else {
        return f64::INFINITY;
    };
    match excess {
        Some(excess) => {
            // The product of the mole fractions at saturation, and the scarcer's root of
            // x (x + excess) = it, in the form that keeps its digits.
            let product = math::exp(ln_sat - 2.0 * ln_p);
            2.0 * product / (excess + (excess * excess + 4.0 * product).sqrt())
        }
        None => math::exp(ln_sat - ln_p),
    }
}

/// A deck's column as Ackerman and Marley's balance gives it from its base up: ln `q_v` and `q_c`
/// at each scan level from the first above the base, each level's column mass, and the total.
struct Column {
    vapour_ln: Vec<f64>,
    condensate: Vec<f64>,
    level_masses: Vec<f64>,
    mass: f64,
}

/// The column above `base` of a vapour of mole fraction `below` whose saturated mole fraction at
/// ln p is `saturation`, from the scan level `first`, with `f_sed` `efficiency` and `epsilon`
/// kilograms of condensate per kilogram of gas per unit of the vapour's mole fraction, under
/// `gravity`, in `substeps` steps to each scan step (the [module](self)'s documentation).
#[must_use]
fn integrate(
    base: Base,
    below: f64,
    saturation: impl Fn(f64) -> f64,
    first: usize,
    grid: &Grid,
    (efficiency, epsilon, gravity, substeps): (f64, f64, f64, u32),
) -> Column {
    let levels = grid.len().saturating_sub(first);
    let mut column = Column {
        vapour_ln: Vec::with_capacity(levels),
        condensate: Vec::with_capacity(levels),
        level_masses: Vec::with_capacity(levels),
        mass: 0.0,
    };
    // A deck whose base lies at or below the column's bottom starts from its vapour saturated
    // there: its condensate from below the column is not counted.
    let mut saturated_from = match base.bound {
        DeckBound::Crossing => below,
        DeckBound::Bottom => saturation(base.u).min(below),
    };
    let mut total = saturated_from;
    let mut from = base.u;
    for level in first..grid.len() {
        let to = grid.u(level);
        let step = (from - to) / f64::from(substeps);
        let mut level_mass = 0.0;
        for substep in 1..=substeps {
            let end = from - step * f64::from(substep);
            let saturated_end = saturation(end);
            let pressure_start = math::exp(end + step);
            let loaded_start = (total - saturated_from).max(0.0) * pressure_start;
            let finite = saturated_from.is_finite() && saturated_end.is_finite();
            // The step's ∫ q_c p d ln p, where it is known in closed form.
            let mut exact_load = None;
            if finite
                && saturated_end > 0.0
                && saturated_end < saturated_from
                && total >= saturated_from
            {
                // Within the cloud, under a saturation that falls across the step: the balance's
                // exact solution for q_s falling exponentially at the rate `fall` (the module's
                // documentation).
                let fall = math::ln(saturated_from / saturated_end) / step;
                // q_c = q_c0 e^(−f x) + s q_s0 (e^(−s x) − e^(−f x)) ÷ (f − s), integrated over
                // the step exactly against p = p_start e^(−x): a base's first step's transient and
                // the steady state alike.
                exact_load = Some(
                    pressure_start
                        * ((total - saturated_from) * decay_integral(efficiency, step)
                            + fall * saturated_from * decay_difference(fall, efficiency, step)),
                );
                total = total * math::exp(-efficiency * step)
                    + efficiency * step * saturated_end * mean_decay((efficiency - fall) * step);
            } else {
                let mean = if finite {
                    (saturated_from * saturated_end).sqrt()
                } else {
                    f64::INFINITY
                };
                if total > mean {
                    total = mean + (total - mean) * math::exp(-efficiency * step);
                }
            }
            let loaded_end = (total - saturated_end).max(0.0) * math::exp(end);
            let load = exact_load
                .unwrap_or_else(|| f64::midpoint(loaded_start, loaded_end) * step)
                .max(0.0);
            level_mass += load * epsilon / gravity;
            saturated_from = saturated_end;
        }
        column.mass += level_mass;
        // A vapour whose saturation underflows (alumina at 110 K) is held at the smallest normal
        // fraction, so that its logarithm, which the levels interpolate, stays finite.
        column
            .vapour_ln
            .push(math::ln(total.min(saturated_from).max(f64::MIN_POSITIVE)));
        column.condensate.push((total - saturated_from).max(0.0));
        column.level_masses.push(level_mass);
        from = to;
    }
    column
}

/// ∫₀^Δ e^(−(1 + a) x) dx over a step `step` long, for a decay rate `rate` a: Δ (1 − e^(−(1+a)Δ))
/// ÷ ((1 + a) Δ), the weight of a term e^(−a x) of a step's condensate against p ∝ e^(−x).
#[must_use]
fn decay_integral(rate: f64, step: f64) -> f64 {
    step * mean_decay((1.0 + rate) * step)
}

/// The divided difference (I(s) − I(f)) ÷ (f − s) of [`decay_integral`]'s I in its rate, for the
/// saturation's rate `fall` s and `efficiency` f over a step `step` long: the weight of a step's
/// steady condensate. Within 10⁻⁴ of each other in (f − s) Δ it is −dI/da at their midpoint,
/// ((1 − e^(−kΔ)) − kΔ e^(−kΔ)) ÷ k² with k = 1 + a, its limit, the two forms agreeing to about
/// 10⁻⁸ there.
#[must_use]
fn decay_difference(fall: f64, efficiency: f64, step: f64) -> f64 {
    let gap = efficiency - fall;
    if (gap * step).abs() > 1e-4 {
        (decay_integral(fall, step) - decay_integral(efficiency, step)) / gap
    } else {
        let k = 1.0 + f64::midpoint(fall, efficiency);
        let decayed = math::exp(-k * step);
        ((1.0 - decayed) - k * step * decayed) / (k * k)
    }
}

/// The mean of e^(−u) over u from 0 to `delta`, (1 − e^(−δ)) ÷ δ, and 1 at δ = 0: the weight of
/// the saturation's share in a cloud's exact step (`integrate`), finite for every finite δ.
#[must_use]
fn mean_decay(delta: f64) -> f64 {
    if delta.abs() < f64::EPSILON {
        1.0
    } else {
        -math::exp_m1(-delta) / delta
    }
}

/// The deck of the candidate at its base `base`, in the gas the decks `placed` leave: its column
/// by Ackerman and Marley's balance, its vapour above, and its mode.
#[must_use]
fn place(
    candidate: &Candidate,
    base: Base,
    pool: &Pool,
    placed: &[Placed],
    context: &Context<'_>,
) -> Deck {
    let grid = context.grid;
    let air = context.column;
    let sedimentation = (context.sedimentation)(candidate.id);
    let efficiency = sedimentation.efficiency();
    let (first_amount, second_amount) = available(candidate, base.u, pool, placed);
    let (below, excess) = match candidate.second {
        Some(_) => (
            first_amount.min(second_amount),
            Some((first_amount - second_amount).abs()),
        ),
        None => (first_amount, None),
    };
    let law = candidate.law;
    // Mass of condensate per mole of condensing vapour, over the gas's molar mass.
    let epsilon =
        candidate.molar_mass / f64::from(law.limiting().count()) / air.molar_mass_g_per_mol;
    let saturation = |ln_p: f64| {
        saturated(
            &law,
            ln_p,
            grid.temperature(ln_p),
            context.metallicity,
            excess,
        )
    };
    let first = (0..grid.len())
        .find(|&level| grid.u(level) < base.u)
        .unwrap_or(grid.len());
    let column = integrate(
        base,
        below,
        saturation,
        first,
        grid,
        (efficiency, epsilon, air.gravity.value(), context.substeps),
    );
    let top_index = column
        .condensate
        .iter()
        .rposition(|&q| q >= TOP_FRACTION * below)
        .unwrap_or(0);
    let (top, vapour_above) = if column.condensate.is_empty() {
        (base.u, below)
    } else {
        (
            grid.u(first + top_index),
            math::exp(column.vapour_ln[top_index]),
        )
    };
    let phase = phase_of(candidate.id, base.temperature);
    let column_mass = KilogramsPerSquareMetre::new(column.mass);
    let within = column.condensate.len().min(top_index + 1);
    let levels = Levels {
        first,
        condensate: &column.condensate[..within],
        masses: &column.level_masses,
    };
    let size = size(candidate.id, phase, base.temperature, &levels, context);
    let optical_depth = optical_depth(candidate.id, phase, size, column_mass, base.temperature);
    Deck {
        condensate: candidate.id,
        phase,
        base: Pascals::new(math::exp(base.u)),
        base_temperature: Kelvin::new(base.temperature),
        bound: base.bound,
        top: Pascals::new(math::exp(top)),
        vapour_below: below,
        vapour_above,
        sedimentation: efficiency,
        column_mass,
        extrapolated: extrapolated(&law, base, context.metallicity),
        modes: vec![Mode {
            material: candidate.id,
            phase,
            deck: candidate.id,
            size,
            column_mass,
            optical_depth,
            profile: ModeProfile {
                base: Pascals::new(math::exp(base.u)),
                top: Pascals::new(math::exp(top)),
                exponent: efficiency,
            },
            coverage: context.coverage,
        }],
        base_ln_p: base.u,
        bottom_ln_p: grid.bottom,
        step_ln_p: grid.step,
        first,
        vapour_ln: column.vapour_ln,
    }
}

/// Whether `law` is applied at `base` in a gas of `metallicity` dex outside what it was fitted
/// over.
#[must_use]
fn extrapolated(law: &LawAloft, base: Base, metallicity: f64) -> bool {
    law.fitted().is_some_and(|fitted| {
        let (low, high) = fitted.metallicity_dex;
        let (shallow, deep) = fitted.total_pressure;
        let (cold, hot) = fitted.temperature;
        let pressure = math::exp(base.u);
        metallicity < low
            || metallicity > high
            || pressure < shallow.value()
            || pressure > deep.value()
            || base.temperature < cold.value()
            || base.temperature > hot.value()
    })
}

/// A deck's scan levels, as its mode's size reads them: the first above the base, and from it
/// each level's condensate mole fraction to the deck's top and column mass.
struct Levels<'a> {
    first: usize,
    condensate: &'a [f64],
    masses: &'a [f64],
}

/// The phase the condensate `id` forms in at `temperature`: liquid at or above its first melt (a
/// phase diagram's triple point), solid below; solid for a row with neither.
#[must_use]
fn phase_of(id: SubstanceId, temperature: f64) -> CondensedPhase {
    let row = id.substance();
    let melt = row
        .phase_diagram()
        .map(|d| d.triple_temperature().value())
        .or_else(|| {
            row.condensed()
                .map(|c| c.transition().temperature().value())
        });
    match melt {
        Some(m) if temperature >= m => CondensedPhase::Liquid,
        _ => CondensedPhase::Solid,
    }
}

/// The mode's log-normal over the deck's scan levels from its first above the base: each level's
/// `r_eff` by Ackerman and Marley's eq. 13 at the radius `r_w` whose fall speed is w*, for the grain
/// density at the base's temperature `base_temperature`, and their harmonic mean weighted by
/// `weights`, the levels' column masses (the [module](self)'s documentation).
fn size(
    id: SubstanceId,
    phase: CondensedPhase,
    base_temperature: f64,
    levels: &Levels<'_>,
    context: &Context<'_>,
) -> Result<SizeDistribution, SizeModeError> {
    let key = id.substance().key();
    let background = context
        .background
        .ok_or(SizeModeError::NoCollisionConstants { key })?;
    let gas_key = background.substance().key();
    let constants = COLLISION_CONSTANTS
        .iter()
        .find(|c| c.key == gas_key.as_str())
        .ok_or(SizeModeError::NoCollisionConstants { key: gas_key })?;
    // One phase and one grain density a deck, its base's (the module's documentation).
    let density = id
        .substance()
        .grain_density(phase, Kelvin::new(base_temperature))
        .ok_or(SizeModeError::NoDensity {
            key,
            phase,
            temperature: Kelvin::new(base_temperature),
        })?;
    let sedimentation = (context.sedimentation)(id);
    let sigma = sedimentation.width();
    let ln_sigma_squared = math::ln(sigma) * math::ln(sigma);
    let efficiency = sedimentation.efficiency();
    let (mut mass, mut area) = (0.0, 0.0);
    for (offset, (&condensate, &level_mass)) in
        levels.condensate.iter().zip(levels.masses).enumerate()
    {
        if condensate <= 0.0 || level_mass <= 0.0 {
            continue;
        }
        let level = levels.first + offset;
        let temperature = context.grid.temperatures[level];
        let pressure = math::exp(context.grid.u(level));
        let air = Air::at(temperature, pressure, context.column, constants);
        let w_star = context.column.eddy_diffusion.square_metres_per_second() / air.scale_height_m;
        let r_w = air.radius_falling_at(w_star, density);
        let fit = sigma.max(ALPHA_FIT_WIDTH);
        let alpha = math::ln(air.fall_speed(fit * r_w, density) / air.fall_speed(r_w, density))
            / math::ln(fit);
        let r_g = r_w
            * math::powf(efficiency, 1.0 / alpha)
            * math::exp(-(alpha + 6.0) / 2.0 * ln_sigma_squared);
        let r_eff = r_g * math::exp(2.5 * ln_sigma_squared);
        mass += level_mass;
        area += level_mass / r_eff;
    }
    if area <= 0.0 {
        return Err(SizeModeError::NoCondensate);
    }
    SizeDistribution::log_normal(Metres::new(mass / area), math::exp(ln_sigma_squared) - 1.0)
        .map_err(SizeModeError::Distribution)
}

/// A level's gas, as the fall speed reads it.
struct Air {
    /// The gravity, m s⁻².
    gravity_m_s2: f64,
    /// The gas's density, kg m⁻³.
    density_kg_m3: f64,
    /// Its dynamic viscosity, Pa s.
    viscosity_pa_s: f64,
    /// Its mean free path, m.
    free_path_m: f64,
    /// Its pressure scale height, m.
    scale_height_m: f64,
}

impl Air {
    /// The gas at `temperature` kelvin and `pressure` pascals in `column`, of the background
    /// `constants`.
    #[must_use]
    fn at(
        temperature: f64,
        pressure: f64,
        column: &MixingColumn,
        constants: &CollisionConstants,
    ) -> Self {
        let molar_kg = column.molar_mass_g_per_mol / GRAMS_PER_KILOGRAM;
        let thermal = BOLTZMANN_CONSTANT * temperature;
        // The mean molecular mass, kg: the molar mass over Avogadro's number, R ÷ k.
        let molecule = molar_kg * BOLTZMANN_CONSTANT / MOLAR_GAS_CONSTANT;
        let diameter = constants.diameter_m;
        let collision_area = core::f64::consts::PI * diameter * diameter;
        let viscosity = 5.0 / 16.0 * (core::f64::consts::PI * molecule * thermal).sqrt()
            / (collision_area * collision_integral(temperature / constants.well_depth_k));
        let gravity = column.gravity.value();
        Self {
            gravity_m_s2: gravity,
            density_kg_m3: pressure * molar_kg / (MOLAR_GAS_CONSTANT * temperature),
            viscosity_pa_s: viscosity,
            free_path_m: thermal / (core::f64::consts::SQRT_2 * collision_area * pressure),
            scale_height_m: MOLAR_GAS_CONSTANT * temperature / (molar_kg * gravity),
        }
    }

    /// The fall speed of a particle of radius `radius` m and density `density`, m s⁻¹ (the
    /// [module](self)'s documentation).
    #[must_use]
    fn fall_speed(&self, radius: f64, density: KilogramsPerCubicMetre) -> f64 {
        let particle = density.value();
        let knudsen = self.free_path_m / radius;
        let slip = 1.0 + knudsen * (1.257 + 0.4 * math::exp(-1.1 / knudsen));
        let viscosity = self.viscosity_pa_s;
        let gravity = self.gravity_m_s2;
        let stokes = 2.0 * slip * gravity * radius * radius * particle / (9.0 * viscosity);
        let inertia = 0.45 * gravity * radius * radius * radius * self.density_kg_m3 * particle
            / (54.0 * viscosity * viscosity);
        stokes * math::powf(1.0 + math::powf(inertia, 0.4), -1.25)
    }

    /// The radius whose fall speed is `speed`, by bisection in ln r over [`RADIUS_RANGE_M`], held
    /// at its ends.
    #[must_use]
    fn radius_falling_at(&self, speed: f64, density: KilogramsPerCubicMetre) -> f64 {
        let (mut small, mut large) = (math::ln(RADIUS_RANGE_M.0), math::ln(RADIUS_RANGE_M.1));
        for _ in 0..RADIUS_BISECTIONS {
            let middle = f64::midpoint(small, large);
            if self.fall_speed(math::exp(middle), density) < speed {
                small = middle;
            } else {
                large = middle;
            }
        }
        math::exp(f64::midpoint(small, large))
    }
}

/// Neufeld, Janzen and Aziz 1972's fit to the Lennard-Jones collision integral Ω^(2,2)* at the
/// reduced temperature `reduced` = k T ÷ ε (J. Chem. Phys. 57, 1100; Poling, Prausnitz and
/// O'Connell 2001, eq. 9-4.3): 1.16145 T*^−0.14874 + 0.52487 e^(−0.77320 T*) + 2.16178
/// e^(−2.43787 T*), within 0.1% for 0.3 ≤ T* ≤ 100 ([`REDUCED_TEMPERATURE_RANGE`]), and held at
/// its ends beyond them (an H₂O background below 243 K, SO₂ below 101 K, He above 1,022 K).
#[must_use]
fn collision_integral(reduced: f64) -> f64 {
    let (low, high) = REDUCED_TEMPERATURE_RANGE;
    let reduced = reduced.clamp(low, high);
    1.161_45 * math::powf(reduced, -0.148_74)
        + 0.524_87 * math::exp(-0.773_20 * reduced)
        + 2.161_78 * math::exp(-2.437_87 * reduced)
}

/// The reduced temperatures over which Neufeld, Janzen and Aziz 1972 fit Ω^(2,2)*, "0.3 ≤ T* ≤
/// 100" (their abstract).
const REDUCED_TEMPERATURE_RANGE: (f64, f64) = (0.3, 100.0);

/// The τ(550) of a mode of `id` in `phase` of size `size` and column mass `mass` at `temperature`:
/// P14.T49.e's, or above its grid the geometric scatterers'.
fn optical_depth(
    id: SubstanceId,
    phase: CondensedPhase,
    size: Result<SizeDistribution, SizeModeError>,
    mass: KilogramsPerSquareMetre,
    temperature: f64,
) -> Result<OpticalDepth, ResolveOpticalDepthError> {
    let size = size.map_err(ResolveOpticalDepthError::Size)?;
    let t = Kelvin::new(temperature);
    let key = id.substance().key();
    match aerosol_optics(id, phase, density_variant(key.as_str()), &size, t) {
        Ok(optics) => Ok(OpticalDepth {
            value: optics.mass_extinction_m2_per_kg() * mass.value(),
            basis: OpticalDepthBasis::Table(optics.extinction_basis()),
        }),
        Err(ResolveAerosolError::OffGrid {
            effective_radius, ..
        }) if effective_radius.value() > GEOMETRIC_FROM_M => {
            let density =
                id.substance()
                    .grain_density(phase, t)
                    .ok_or(ResolveOpticalDepthError::Size(SizeModeError::NoDensity {
                        key,
                        phase,
                        temperature: t,
                    }))?;
            Ok(OpticalDepth {
                value: 1.5 * mass.value() / (density.value() * effective_radius.value()),
                basis: OpticalDepthBasis::GeometricLimit,
            })
        }
        Err(error) => Err(ResolveOpticalDepthError::Aerosol(error)),
    }
}

/// The effective radius above which a mode off the extinction table's grid is a geometric
/// scatterer, m: the grid's largest log-normal is about 33 µm, its smallest 1 nm.
const GEOMETRIC_FROM_M: f64 = 1e-6;

/// ln of a positive value.
#[must_use]
fn ln(x: f64) -> f64 {
    math::ln(x)
}

/// `i` as a float: exact for every count of levels a column holds.
#[expect(
    clippy::cast_precision_loss,
    reason = "a column holds far fewer than 2⁵³ levels, so the count is exact"
)]
fn index_f64(i: usize) -> f64 {
    i as f64
}

/// The whole levels in `offset` (non-negative), held at `last`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the offset is non-negative and held at `last` before the cast"
)]
#[must_use]
fn offset_index(offset: f64, last: usize) -> usize {
    let floor = offset.floor();
    if floor >= index_f64(last) {
        last
    } else {
        floor.max(0.0) as usize
    }
}

/// A phase's name, as messages write it.
#[must_use]
const fn phase_name(phase: CondensedPhase) -> &'static str {
    match phase {
        CondensedPhase::Solid => "solid",
        CondensedPhase::Liquid => "liquid",
    }
}

#[cfg(test)]
mod tests;
