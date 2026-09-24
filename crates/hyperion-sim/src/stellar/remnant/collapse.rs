//! Neutron stars and black holes: the type and mass of what a collapsing star leaves under the
//! generator's default recipe, [`RemnantRecipe::MandelMuller2020`](super::RemnantRecipe), from iron
//! core collapse, electron capture and pair instability (plan 06, P06.T18.a–c).
//!
//! **Core collapse** follows Mandel and Müller (2020, MNRAS 499, 3214, "MM20"), section 3 and
//! Table 1: a probabilistic recipe in the mass of the carbon–oxygen core at collapse, `M_CO`.
//!
//! - Below M₁ = 2 M☉ ([`NEUTRON_STAR_CORE_LIMIT`]) the remnant is always a neutron star. From M₁
//!   it is a black hole with probability (`M_CO` − M₁) ÷ (M₃ − M₁), which reaches 1 at M₃ = 7 M☉
//!   ([`BLACK_HOLE_CORE_LIMIT`]). The line between the two is therefore a probability over about
//!   10–25 M☉ of initial mass, not a threshold.
//! - A black hole forms by complete fallback with probability (`M_CO` − M₁) ÷ (M₄ − M₁), which
//!   reaches 1 at M₄ = 8 M☉ ([`COMPLETE_FALLBACK_CORE_LIMIT`]). It then has the helium core's
//!   mass, with no supernova and no kick. Otherwise its mass is drawn about 0.8 `M_CO` with σ =
//!   0.5 M☉, above the largest neutron star.
//! - Neutron-star masses are drawn about 1.2 M☉ (σ 0.02) below M₁ and along two strands above it,
//!   broken at M₂ = 3 M☉ ([`NEUTRON_STAR_STRAND_BREAK`]), within 1.13–2.0 M☉.
//!
//! MM20 fit the recipe to models of `M_CO` ≈ 1.4–9 M☉ (their Fig. 1); above that, complete fallback
//! is their rule carried on, as they carry it to every star above 36 M☉ (their section 4).
//!
//! The hydrogen envelope never enters: MM20 take any envelope to be unbound in the collapse
//! (their section 3, after Lovegrove and Woosley 2013), so the black hole of complete fallback is
//! the helium core, which is the whole star for a naked helium star. The masses are gravitational.
//!
//! **Electron capture** (plan 06, design note 12) takes windows of initial mass that end at the
//! lowest initial mass that makes an iron core, `m_cc(Z)`, found once per metallicity by
//! [`ElectronCaptureWindows::new`]. It leaves MM20's 1.26 M☉ neutron star
//! ([`electron_capture_remnant`]).
//!
//! **Pair instability** (plan 06, design note 10) follows Belczynski et al. (2016, A&A 594, A97),
//! sections 2 and 3 (their model M10), in the helium core at death ([`pair_instability`]). MM20
//! stop below it.
//!
//! Every function takes plain masses and the star's three remnant draws ([`RemnantDraws`]), so
//! that the track's death (P06.T18.d, in `Track::death`) and a quadrature over explicit variates
//! run the same code. Each decision is a fixed [`Mark`] against a [`Threshold`] that moves with
//! `M_CO`. A lower mark therefore never turns a black hole into a neutron star, and neither does a
//! heavier core for the same mark. One standard normal gives the mass on whichever branch
//! applies.
//!
//! MM20 redraw a mass that falls outside its range. The one normal a star has for its mass
//! ([`tags::STAR_REMNANT_MASS`](crate::rng::tags::STAR_REMNANT_MASS)) is instead mapped through
//! the quantile of the normal truncated to that range. This has the distribution of redrawing
//! until the mass is inside, spends no further words, and is monotone in the draw.

use core::f64::consts::FRAC_1_SQRT_2;

use crate::math;
use crate::rng::{Mark, Threshold};
use crate::stellar::draws::{StandardNormal, StarDraws};
use crate::stellar::sse::{ZCoeffs, m_c_bagb};
use crate::units::SolarMasses;

use super::{CompactRemnant, RemnantKind};

// ---------------------------------------------------------------------------------------------
// Mandel and Müller (2020), Table 1.

/// M₁: the carbon–oxygen core mass below which a collapse always leaves a neutron star, and from
/// which the black hole and complete-fallback probabilities rise: 2.0 M☉ (MM20, Table 1).
pub const NEUTRON_STAR_CORE_LIMIT: SolarMasses = SolarMasses::new(2.0);

/// M₂: the carbon–oxygen core mass at which the neutron-star mass law passes from its upper
/// strand back to its lower: 3.0 M☉ (MM20, Table 1, "break in NS mass distribution fits").
pub const NEUTRON_STAR_STRAND_BREAK: SolarMasses = SolarMasses::new(3.0);

/// M₃: the carbon–oxygen core mass from which a collapse always leaves a black hole: 7.0 M☉
/// (MM20, Table 1).
pub const BLACK_HOLE_CORE_LIMIT: SolarMasses = SolarMasses::new(7.0);

/// M₄: the carbon–oxygen core mass from which every black hole forms by complete fallback:
/// 8.0 M☉ (MM20, Table 1).
pub const COMPLETE_FALLBACK_CORE_LIMIT: SolarMasses = SolarMasses::new(8.0);

/// `M_NS,min`: the lightest neutron star a core collapse leaves, 1.13 M☉ (MM20, Table 1).
pub const MIN_NEUTRON_STAR_MASS: SolarMasses = SolarMasses::new(1.13);

/// `M_NS,max`: the heaviest neutron star, and the floor of a black hole's mass, 2.0 M☉ (MM20,
/// Table 1).
pub const MAX_NEUTRON_STAR_MASS: SolarMasses = SolarMasses::new(2.0);

/// μ₁ and σ₁, M☉: the neutron-star mass law below M₁, 1.2 ± 0.02 (MM20, Table 1).
const LOW_CORE_NEUTRON_STAR: (f64, f64) = (1.2, 0.02);

/// μ₂ₐ, μ₂ᵦ and σ₂: the law from M₁ to M₂, a mean of μ₂ₐ + μ₂ᵦ (`M_CO` − M₁) ÷ (M₂ − M₁) M☉
/// with σ₂ M☉, 1.4 + 0.5 (…) ± 0.05 (MM20, Table 1).
const UPPER_STRAND: (f64, f64, f64) = (1.4, 0.5, 0.05);

/// μ₃ₐ, μ₃ᵦ and σ₃: the law from M₂ to M₃, a mean of μ₃ₐ + μ₃ᵦ (`M_CO` − M₂) ÷ (M₃ − M₂) M☉
/// with σ₃ M☉, 1.4 + 0.4 (…) ± 0.05 (MM20, Table 1).
const LOWER_STRAND: (f64, f64, f64) = (1.4, 0.4, 0.05);

/// `μ_BH` and `σ_BH`: a black hole that does not form by complete fallback has a mean mass of
/// `μ_BH` × `M_CO` with `σ_BH` M☉, 0.8 `M_CO` ± 0.5 (MM20, Table 1).
const FALLBACK_BLACK_HOLE: (f64, f64) = (0.8, 0.5);

// ---------------------------------------------------------------------------------------------
// Electron capture.

/// The core mass at the base of the AGB from which the carbon–oxygen core does not become
/// degenerate and goes on to an iron core that collapses: 2.25 M☉ (Hurley, Pols and Tout 2000,
/// MNRAS 315, 543, section 6; MM20, section 3). Between 1.6 and 2.25 M☉ HPT's core is
/// oxygen–neon.
pub const IRON_CORE_MC_BAGB: SolarMasses = SolarMasses::new(2.25);

/// The width of a single star's electron-capture window in initial mass, 0.1 M☉ (the brainstorm's
/// "Winds and remnant masses" row; plan 06, design note 12).
///
/// P06.T19's `KickLawParams` carries it as `ec_window_single`, whose default must be this value.
pub const SINGLE_STAR_WINDOW: SolarMasses = SolarMasses::new(0.1);

/// The width of a companion-stripped star's electron-capture window in initial mass, 1.0 M☉ (the
/// brainstorm's "about 1 M☉"; plan 06, design note 12).
///
/// P06.T19's `KickLawParams` carries it as `ec_window_stripped`, whose default must be this value.
pub const COMPANION_STRIPPED_WINDOW: SolarMasses = SolarMasses::new(1.0);

/// The core mass at which electron captures on ²⁴Mg and ²⁰Ne collapse a degenerate oxygen–neon
/// core, and so the heaviest oxygen–neon white dwarf the default recipe leaves: 1.37 M☉ (ruling
/// 57 of 2026-09-22).
///
/// Miyaji et al. (1980, PASJ 32, 303) and Nomoto (1984, ApJ 277, 791) find the collapse at about
/// 1.375 M☉, the figure Doherty et al. (2015, MNRAS 446, 2599, sections 1 and 3.2) use for a
/// super-AGB core's electron-capture limit; model assumptions move it slightly (1.367 M☉ in
/// Takahashi et al. 2013, as Doherty et al. note). HPT use the Chandrasekhar mass, 1.44 M☉, which
/// `RemnantRecipe::Hurley2000` keeps.
pub const OXYGEN_NEON_CAPTURE_MASS: SolarMasses = SolarMasses::new(1.37);

/// The gravitational mass of the neutron star an electron-capture supernova leaves: 1.26 M☉
/// (MM20, section 3).
pub const ELECTRON_CAPTURE_NEUTRON_STAR_MASS: SolarMasses = SolarMasses::new(1.26);

/// The bracket of initial masses, M☉, in which [`ElectronCaptureWindows::new`] seeks `m_cc`: the
/// range of Hurley, Pols and Tout's fits. The root lies well inside it at every metallicity of
/// the fits (6.72–8.32 M☉).
const ROOT_BRACKET: (f64, f64) = (0.1, 100.0);

// ---------------------------------------------------------------------------------------------
// Pair instability: Belczynski et al. (2016, A&A 594, A97), sections 2 and 3, model M10.

/// The helium core mass from which pulsations shed everything above it before the core
/// collapses: 45 M☉ (Belczynski et al. 2016, section 3).
pub const PULSATIONAL_PAIR_INSTABILITY_CORE: SolarMasses = SolarMasses::new(45.0);

/// The helium core mass from which the first pulse disrupts the whole star, leaving nothing:
/// 65 M☉ (Belczynski et al. 2016, section 3).
pub const PAIR_INSTABILITY_CORE: SolarMasses = SolarMasses::new(65.0);

/// The helium core mass from which photodisintegration softens the core so much that it collapses
/// to a black hole instead of exploding: 135 M☉ (Belczynski et al. 2016, section 2, after Heger
/// and Woosley 2002, ApJ 567, 532). No star of up to 150 M☉ reaches it.
pub const PAIR_INSTABILITY_COLLAPSE_CORE: SolarMasses = SolarMasses::new(135.0);

/// The black hole a pulsational pair instability leaves: 40.5 M☉, the 45 M☉ core that the
/// pulses leave less a tenth lost in neutrinos, 45 (1 − `f_n`) with `f_n` = 0.1 (Belczynski et al.
/// 2016, equation 1), as plan 06's design note 10 takes it.
///
/// MM20 neglect a black hole's loss to neutrinos, at most 0.1 M☉ (their section 3), so a 44.9 M☉
/// helium core leaves a 44.9 M☉ black hole by complete fallback and a 45 M☉ core this lighter one.
/// Belczynski et al. themselves expect `f_n` below 0.1 (44.6 M☉ at 0.01).
pub const PULSATIONAL_PAIR_INSTABILITY_BLACK_HOLE: SolarMasses = SolarMasses::new(40.5);

// ---------------------------------------------------------------------------------------------
// The draws.

/// A star's three remnant draws, which decide its type and mass at a core collapse.
///
/// Built from a star's [`StarDraws`] by [`RemnantDraws::of`], or from explicit variates by
/// [`RemnantDraws::from_parts`] for a quadrature that has no star.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemnantDraws {
    remnant_type: Mark,
    fallback: Mark,
    mass: StandardNormal,
}

impl RemnantDraws {
    /// The remnant draws of a star: its [`remnant_type`](StarDraws::remnant_type),
    /// [`remnant_fallback`](StarDraws::remnant_fallback) and
    /// [`remnant_mass`](StarDraws::remnant_mass).
    #[must_use]
    pub const fn of(draws: &StarDraws) -> Self {
        Self::from_parts(
            draws.remnant_type(),
            draws.remnant_fallback(),
            draws.remnant_mass(),
        )
    }

    /// The draws `remnant_type` (neutron star or black hole), `fallback` (complete fallback or not)
    /// and `mass` (the mass's standard normal).
    #[must_use]
    pub const fn from_parts(remnant_type: Mark, fallback: Mark, mass: StandardNormal) -> Self {
        Self {
            remnant_type,
            fallback,
            mass,
        }
    }

    /// The mark that decides a neutron star or a black hole: a black hole below the threshold.
    #[must_use]
    pub const fn remnant_type(&self) -> Mark {
        self.remnant_type
    }

    /// The mark that decides a black hole's complete fallback: complete below the threshold.
    #[must_use]
    pub const fn fallback(&self) -> Mark {
        self.fallback
    }

    /// The standard normal that sets the remnant's mass.
    #[must_use]
    pub const fn mass(&self) -> StandardNormal {
        self.mass
    }
}

// ---------------------------------------------------------------------------------------------
// Core collapse (P06.T18.a) and pair instability (P06.T18.c).

/// What an iron-core collapse leaves under
/// [`RemnantRecipe::MandelMuller2020`](super::RemnantRecipe), pair instability included
/// ([`core_collapse`]).
///
/// The variants follow the ways such a star can die:
///
/// | Variant                        | Death (P06.T18.d)               | Remnant                   |
/// | ------------------------------ | ------------------------------- | ------------------------- |
/// | [`NeutronStar`]                | a core-collapse supernova       | a neutron star            |
/// | [`BlackHole`]                  | a supernova with some fallback  | a black hole              |
/// | [`DirectCollapse`]             | complete fallback, no supernova | the helium core, whole    |
/// | [`PulsationalPairInstability`] | pulses, then complete fallback  | a 40.5 M☉ black hole      |
/// | [`PairInstabilitySupernova`]   | a pair-instability supernova    | none                      |
///
/// [`NeutronStar`]: Self::NeutronStar
/// [`BlackHole`]: Self::BlackHole
/// [`DirectCollapse`]: Self::DirectCollapse
/// [`PulsationalPairInstability`]: Self::PulsationalPairInstability
/// [`PairInstabilitySupernova`]: Self::PairInstabilitySupernova
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoreCollapse {
    /// A supernova that leaves a neutron star of `mass`.
    NeutronStar {
        /// The gravitational mass, M☉, in [`MIN_NEUTRON_STAR_MASS`]–[`MAX_NEUTRON_STAR_MASS`].
        mass: SolarMasses,
    },
    /// A supernova whose fallback leaves a black hole of `mass`, which can be kicked (MM20,
    /// equation 3).
    BlackHole {
        /// The gravitational mass, M☉, from [`MAX_NEUTRON_STAR_MASS`] to the helium core's.
        mass: SolarMasses,
    },
    /// Complete fallback: the helium core collapses whole into a black hole of `mass`, with no
    /// supernova and no kick.
    DirectCollapse {
        /// The gravitational mass, M☉: the helium core's.
        mass: SolarMasses,
    },
    /// Pulsations shed everything above a 45 M☉ core, which then collapses whole into a 40.5 M☉
    /// black hole ([`PULSATIONAL_PAIR_INSTABILITY_BLACK_HOLE`]), with no supernova and no kick.
    PulsationalPairInstability,
    /// A pair-instability supernova disrupts the star and leaves nothing.
    PairInstabilitySupernova,
}

impl CoreCollapse {
    /// The remnant left: a neutron star, a black hole, or none after a pair-instability
    /// supernova.
    ///
    /// # Panics
    ///
    /// In debug builds, if a variant built by hand carries a mass that is not finite and
    /// non-negative.
    #[must_use]
    pub fn remnant(&self) -> CompactRemnant {
        match *self {
            Self::NeutronStar { mass } => CompactRemnant::new(RemnantKind::NeutronStar, mass),
            Self::BlackHole { mass } | Self::DirectCollapse { mass } => {
                CompactRemnant::new(RemnantKind::BlackHole, mass)
            }
            Self::PulsationalPairInstability => CompactRemnant::new(
                RemnantKind::BlackHole,
                PULSATIONAL_PAIR_INSTABILITY_BLACK_HOLE,
            ),
            Self::PairInstabilitySupernova => {
                CompactRemnant::new(RemnantKind::None, SolarMasses::ZERO)
            }
        }
    }
}

/// Whether and how pair instability acts on a helium core ([`pair_instability`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PairInstability {
    /// Below 45 M☉: no significant pulses, and the core collapses as MM20 describe.
    None,
    /// 45–65 M☉: pulsations shed everything above 45 M☉, and the rest collapses to a 40.5 M☉
    /// black hole.
    Pulsational,
    /// 65–135 M☉: the first pulse disrupts the star, leaving no remnant.
    Disruptive,
    /// From 135 M☉: photodisintegration makes the core collapse whole into a black hole.
    Collapse,
}

/// How pair instability acts on a star whose helium core at death, everything inside the hydrogen
/// envelope, is `helium_core`: none below 45 M☉, pulsations to 65 M☉, disruption to 135 M☉, and
/// collapse beyond (Belczynski et al. 2016, sections 2 and 3, model M10; plan 06, design note 10).
///
/// Each range includes its lower end. Only helium cores of the lowest metallicities' 100–150 M☉
/// stars reach 45 M☉, and none reaches 135.
///
/// # Panics
///
/// In debug builds, if `helium_core` is not finite and non-negative.
#[must_use]
pub fn pair_instability(helium_core: SolarMasses) -> PairInstability {
    debug_assert!(
        helium_core.value().is_finite() && helium_core.value() >= 0.0,
        "a helium core is finite and non-negative: {helium_core:?}"
    );
    if helium_core < PULSATIONAL_PAIR_INSTABILITY_CORE {
        PairInstability::None
    } else if helium_core < PAIR_INSTABILITY_CORE {
        PairInstability::Pulsational
    } else if helium_core < PAIR_INSTABILITY_COLLAPSE_CORE {
        PairInstability::Disruptive
    } else {
        PairInstability::Collapse
    }
}

/// What the iron-core collapse of a star with a carbon–oxygen core of `co_core` inside a helium
/// core of `helium_core` (both M☉, at death) leaves, decided by its `draws`: the default recipe's
/// core collapse (MM20, section 3), with pair instability first ([`pair_instability`]).
///
/// The helium core is everything inside the hydrogen envelope, the whole star for a naked helium
/// star, as in `ProgenitorAtDeath::helium_core_mass`. The envelope does not enter (see the
/// [module](self) documentation).
///
/// - The type: a black hole where the [`remnant_type`](RemnantDraws::remnant_type) mark lies below
///   the probability (`M_CO` − M₁) ÷ (M₃ − M₁), clamped to 0–1.
/// - A black hole's fallback: complete where the [`fallback`](RemnantDraws::fallback) mark lies
///   below (`M_CO` − M₁) ÷ (M₄ − M₁), clamped; the mass is then the helium core's.
/// - Otherwise the mass: normal about 0.8 `M_CO` with σ = 0.5 M☉ held to 2.0 M☉ up to the helium
///   core, for a black hole; and for a neutron star normal about 1.2 M☉ (σ 0.02) below M₁, about
///   1.4 + 0.5 (`M_CO` − M₁) ÷ (M₂ − M₁) M☉ (σ 0.05) below M₂ and about
///   1.4 + 0.4 (`M_CO` − M₂) ÷ (M₃ − M₂) M☉ (σ 0.05) above it, held to 1.13–2.0 M☉. Each mass is
///   the [`mass`](RemnantDraws::mass) normal's quantile in the normal truncated to its range.
///
/// MM20 print no upper bound for a black hole. The helium core is the mass of complete fallback,
/// and a black hole with partial fallback cannot be heavier, so it bounds the draw. That follows
/// COMPAS, the code in which MM20 implemented the recipe, which redraws such a mass until it lies
/// between the largest neutron star and the helium core
/// (`GiantBranch::CalculateFallbackBHMassMullerMandel`, <https://github.com/TeamCOMPAS/COMPAS>,
/// `dev` branch, read 2026-09-23). It binds rarely: a single star's helium core exceeds its
/// carbon–oxygen core by 1 M☉ or more.
///
/// A helium core of 45 M☉ or more decides the outcome by pair instability alone, before the
/// draws are read: [`CoreCollapse::PulsationalPairInstability`] to 65 M☉,
/// [`CoreCollapse::PairInstabilitySupernova`] to 135 M☉, and above that the core's
/// [`CoreCollapse::DirectCollapse`], which is also MM20's outcome for any core above M₄.
///
/// # Panics
///
/// In debug builds, if a mass is not finite and non-negative, or the carbon–oxygen core is
/// heavier than the helium core.
///
/// # Examples
///
/// Along a line of carbon–oxygen cores, one star's draws give neutron stars up to where its type
/// mark first lies below the black-hole probability, and black holes from there on:
///
/// ```
/// use hyperion_sim::rng::Mark;
/// use hyperion_sim::stellar::draws::StandardNormal;
/// use hyperion_sim::stellar::remnant::RemnantKind;
/// use hyperion_sim::stellar::remnant::collapse::{RemnantDraws, core_collapse};
/// use hyperion_sim::units::SolarMasses;
///
/// // A type mark at 30% of its range: a black hole once (M_CO − 2) ÷ 5 exceeds 0.3, from 3.5 M☉.
/// let draws = RemnantDraws::from_parts(
///     Mark::from_word(u64::MAX / 10 * 3),
///     Mark::from_word(u64::MAX),
///     StandardNormal::ZERO,
/// );
/// let kind = |co: f64| {
///     core_collapse(SolarMasses::new(co), SolarMasses::new(co + 2.0), draws).remnant().kind()
/// };
/// assert_eq!(kind(1.5), RemnantKind::NeutronStar);
/// assert_eq!(kind(3.4), RemnantKind::NeutronStar);
/// assert_eq!(kind(3.6), RemnantKind::BlackHole);
/// assert_eq!(kind(6.0), RemnantKind::BlackHole);
/// ```
#[must_use]
pub fn core_collapse(
    co_core: SolarMasses,
    helium_core: SolarMasses,
    draws: RemnantDraws,
) -> CoreCollapse {
    let (co, helium) = (co_core.value(), helium_core.value());
    debug_assert!(
        co.is_finite() && co >= 0.0 && helium.is_finite() && helium >= 0.0,
        "core masses are finite and non-negative: {co_core:?}, {helium_core:?}"
    );
    debug_assert!(
        co <= helium * (1.0 + 1e-12),
        "the carbon–oxygen core lies inside the helium core: {co_core:?} of {helium_core:?}"
    );
    match pair_instability(helium_core) {
        PairInstability::None => {}
        PairInstability::Pulsational => return CoreCollapse::PulsationalPairInstability,
        PairInstability::Disruptive => return CoreCollapse::PairInstabilitySupernova,
        PairInstability::Collapse => return CoreCollapse::DirectCollapse { mass: helium_core },
    }
    let m1 = NEUTRON_STAR_CORE_LIMIT.value();
    if draws
        .remnant_type
        .is_below(ramp(co, m1, BLACK_HOLE_CORE_LIMIT.value()))
    {
        if draws
            .fallback
            .is_below(ramp(co, m1, COMPLETE_FALLBACK_CORE_LIMIT.value()))
        {
            CoreCollapse::DirectCollapse { mass: helium_core }
        } else {
            CoreCollapse::BlackHole {
                mass: fallback_black_hole_mass(co, helium, draws.mass),
            }
        }
    } else {
        CoreCollapse::NeutronStar {
            mass: neutron_star_mass(co, draws.mass),
        }
    }
}

/// The threshold of the probability (`co` − `from`) ÷ (`to` − `from`), clamped to 0–1: MM20's
/// linear rise of the black-hole and complete-fallback probabilities.
#[must_use]
fn ramp(co: f64, from: f64, to: f64) -> Threshold {
    Threshold::from_probability(((co - from) / (to - from)).clamp(0.0, 1.0))
}

/// The mean and standard deviation, M☉, of the neutron star of a carbon–oxygen core of `co` M☉
/// (MM20, section 3 and Table 1). Above M₃, which only a black hole reaches, the last strand runs
/// on.
#[must_use]
fn neutron_star_law(co: f64) -> (f64, f64) {
    let (m1, m2, m3) = (
        NEUTRON_STAR_CORE_LIMIT.value(),
        NEUTRON_STAR_STRAND_BREAK.value(),
        BLACK_HOLE_CORE_LIMIT.value(),
    );
    if co < m1 {
        LOW_CORE_NEUTRON_STAR
    } else if co < m2 {
        let (offset, scaling, sigma) = UPPER_STRAND;
        (offset + scaling * (co - m1) / (m2 - m1), sigma)
    } else {
        let (offset, scaling, sigma) = LOWER_STRAND;
        (offset + scaling * (co - m2) / (m3 - m2), sigma)
    }
}

/// The neutron star of a carbon–oxygen core of `co` M☉ at the mass normal `z`: its law held to
/// 1.13–2.0 M☉.
#[must_use]
fn neutron_star_mass(co: f64, z: StandardNormal) -> SolarMasses {
    let (mean, sigma) = neutron_star_law(co);
    SolarMasses::new(truncated_normal(
        mean,
        sigma,
        MIN_NEUTRON_STAR_MASS.value(),
        MAX_NEUTRON_STAR_MASS.value(),
        z,
    ))
}

/// The black hole of partial fallback from a carbon–oxygen core of `co` M☉ in a helium core of
/// `helium` M☉ at the mass normal `z`: normal about 0.8 `co` with σ = 0.5 M☉, held from 2.0 M☉ to
/// the helium core.
#[must_use]
fn fallback_black_hole_mass(co: f64, helium: f64, z: StandardNormal) -> SolarMasses {
    let (scaling, sigma) = FALLBACK_BLACK_HOLE;
    let floor = MAX_NEUTRON_STAR_MASS.value();
    SolarMasses::new(truncated_normal(
        scaling * co,
        sigma,
        floor,
        helium.max(floor),
        z,
    ))
}

/// The upper tail of the standard normal, Q(x) = 1 − Φ(x) = ½ erfc(x ÷ √2), accurate in both
/// tails.
#[must_use]
fn upper_tail(x: f64) -> f64 {
    0.5 * math::erfc(x * FRAC_1_SQRT_2)
}

/// The value at the rank Φ(`z`) of a normal of `mean` and `sigma` truncated to
/// [`lower`, `upper`]: the distribution of redrawing a normal until it falls inside, from one
/// standard normal, increasing in `z`.
///
/// With a = (`lower` − `mean`) ÷ `sigma`, b likewise and w = Φ(b) − Φ(a), the rank's distance
/// below it is p = Φ(a) + Φ(z) w and above it q = Q(b) + Q(z) w, and the standardised value is
/// Φ⁻¹(p) or −Φ⁻¹(q), whichever of p and q is smaller, so that neither tail loses its precision.
/// The result is clamped to the interval against rounding. An interval too narrow or too far in a
/// tail for w to be positive gives the point of it nearest the mean.
#[must_use]
fn truncated_normal(mean: f64, sigma: f64, lower: f64, upper: f64, z: StandardNormal) -> f64 {
    debug_assert!(
        sigma > 0.0 && lower <= upper,
        "a truncated normal needs σ > 0 and a non-empty range: σ = {sigma}, [{lower}, {upper}]"
    );
    let (a, b) = ((lower - mean) / sigma, (upper - mean) / sigma);
    let inside = if a >= 0.0 {
        upper_tail(a) - upper_tail(b)
    } else if b <= 0.0 {
        upper_tail(-b) - upper_tail(-a)
    } else {
        1.0 - upper_tail(-a) - upper_tail(b)
    };
    if inside <= 0.0 || inside.is_nan() {
        return mean.clamp(lower, upper);
    }
    let z = z.value();
    let below = upper_tail(-a) + upper_tail(-z) * inside;
    let above = upper_tail(b) + upper_tail(z) * inside;
    let x = if below <= above {
        if below > 0.0 {
            math::normal_quantile(below)
        } else {
            a
        }
    } else if above > 0.0 {
        -math::normal_quantile(above)
    } else {
        b
    };
    (mean + sigma * x).clamp(lower, upper)
}

// ---------------------------------------------------------------------------------------------
// Electron capture (P06.T18.b).

/// A window of initial mass, [`lower`, `upper`) M☉.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InitialMassWindow {
    lower: SolarMasses,
    upper: SolarMasses,
}

impl InitialMassWindow {
    /// The lowest initial mass inside the window, M☉.
    #[must_use]
    pub const fn lower(&self) -> SolarMasses {
        self.lower
    }

    /// The initial mass just above the window, M☉: the window is open at this end.
    #[must_use]
    pub const fn upper(&self) -> SolarMasses {
        self.upper
    }

    /// Whether the initial mass `m0` lies in the window: `lower` ≤ `m0` < `upper`.
    #[must_use]
    pub fn contains(&self, m0: SolarMasses) -> bool {
        self.lower <= m0 && m0 < self.upper
    }
}

/// Where electron capture happens at one metallicity: the lowest initial mass that makes an iron
/// core, `m_cc(Z)`, and the windows of initial mass below it whose stars collapse by electron
/// capture instead of leaving a white dwarf (plan 06, design note 12).
///
/// `m_cc` is the initial mass whose core at the base of the AGB, `m_c_bagb` (Hurley, Pols and
/// Tout 2000, equation 66), is [`IRON_CORE_MC_BAGB`] = 2.25 M☉, HPT's `M_ec`: from it the early
/// AGB ends in an iron-core collapse. It is 8.203 M☉ at Z = 0.02, 8.32 M☉ at 0.03 and 6.83 M☉ at
/// 10⁻⁴, and lowest, 6.72 M☉, near Z = 3 × 10⁻⁴, since equation 66's coefficients are not monotone
/// in Z. A window is [`m_cc` − width, `m_cc`): 0.1 M☉ wide for a single star and 1.0 M☉ for a
/// companion-stripped one. Every star of either window has 1.6–2.25 M☉ of core at the base of the
/// AGB (at least 1.88 M☉), where HPT's core is oxygen–neon, at every metallicity of the fits.
///
/// `m_cc` is found at constant mass, as equation 66 is written. A track's `m_c_bagb` reads the
/// initial mass that main-sequence winds have left (HPT section 7.1), so on a track the iron cores
/// begin a little higher in initial mass. The track therefore tests the window in the mass its
/// early AGB's `m_c_bagb` reads, which is what decides its iron core, so that the window ends
/// exactly where the iron cores begin (P06.T18.d, ruling 45 of 2026-09-22): the window of a solar
/// star, [8.103, 8.203) M☉ in that mass, is about [8.20, 8.30) M☉ of initial mass.
///
/// MM20's own criterion is HPT's whole oxygen–neon band, 1.6–2.25 M☉ of core at the base of the
/// AGB, which at Z = 0.02 is 1.9 M☉ of initial mass. The brainstorm's widths narrow it and keep
/// its upper end and MM20's 1.26 M☉ neutron star.
///
/// # Examples
///
/// Built once per metallicity, as the coefficients are:
///
/// ```
/// use hyperion_sim::stellar::remnant::collapse::ElectronCaptureWindows;
/// use hyperion_sim::stellar::sse::ZCoeffs;
/// use hyperion_sim::units::{MetalFraction, SolarMasses};
///
/// let solar = ElectronCaptureWindows::new(&ZCoeffs::new(MetalFraction::new(0.02)));
/// assert!((solar.iron_core_mass().value() - 8.203).abs() < 1e-3);
/// assert!(solar.single().contains(SolarMasses::new(8.15)));
/// assert!(!solar.single().contains(SolarMasses::new(8.05)));
/// assert!(solar.companion_stripped().contains(SolarMasses::new(7.5)));
/// assert!(!solar.companion_stripped().contains(SolarMasses::new(8.25)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElectronCaptureWindows {
    iron_core_mass: SolarMasses,
}

impl ElectronCaptureWindows {
    /// The windows at the metallicity of `coeffs`, finding `m_cc` by bisection on `m_c_bagb`.
    ///
    /// The bisection runs over 0.1–100 M☉ until its bracket is two adjacent doubles, some 56
    /// evaluations of equation 66, and keeps the upper one. `m_cc` is therefore the lowest
    /// initial mass whose `m_c_bagb` is at least 2.25 M☉, the test the early AGB's end applies,
    /// to the last bit. Build it once per [`ZCoeffs`], as the track builds its coefficients.
    #[must_use]
    pub fn new(coeffs: &ZCoeffs) -> Self {
        let makes_iron = |m: f64| m_c_bagb(SolarMasses::new(m), coeffs) >= IRON_CORE_MC_BAGB;
        let (mut below, mut above) = ROOT_BRACKET;
        debug_assert!(
            !makes_iron(below) && makes_iron(above),
            "m_cc lies inside {ROOT_BRACKET:?} at Z = {:?}",
            coeffs.z()
        );
        loop {
            let mid = f64::midpoint(below, above);
            if mid <= below || mid >= above {
                break;
            }
            if makes_iron(mid) {
                above = mid;
            } else {
                below = mid;
            }
        }
        Self {
            iron_core_mass: SolarMasses::new(above),
        }
    }

    /// `m_cc(Z)`, M☉: the lowest initial mass that makes an iron core.
    #[must_use]
    pub const fn iron_core_mass(&self) -> SolarMasses {
        self.iron_core_mass
    }

    /// The window of `width` M☉ below `m_cc`, [`m_cc` − `width`, `m_cc`): what
    /// [`single`](Self::single) and [`companion_stripped`](Self::companion_stripped) give at the
    /// default widths, and P06.T19's `KickLawParams` at its own.
    ///
    /// # Panics
    ///
    /// In debug builds, if `width` is not finite and positive.
    #[must_use]
    pub fn window(&self, width: SolarMasses) -> InitialMassWindow {
        debug_assert!(
            width.value().is_finite() && width.value() > 0.0,
            "a window has a positive width: {width:?}"
        );
        InitialMassWindow {
            lower: self.iron_core_mass - width,
            upper: self.iron_core_mass,
        }
    }

    /// A single star's window, [`SINGLE_STAR_WINDOW`] = 0.1 M☉ wide. A star whose own wind
    /// stripped it counts as single.
    #[must_use]
    pub fn single(&self) -> InitialMassWindow {
        self.window(SINGLE_STAR_WINDOW)
    }

    /// A companion-stripped star's window, [`COMPANION_STRIPPED_WINDOW`] = 1.0 M☉ wide.
    #[must_use]
    pub fn companion_stripped(&self) -> InitialMassWindow {
        self.window(COMPANION_STRIPPED_WINDOW)
    }
}

/// The remnant of an electron-capture supernova: a neutron star of
/// [`ELECTRON_CAPTURE_NEUTRON_STAR_MASS`] = 1.26 M☉ (MM20, section 3), whatever the draws.
#[must_use]
pub fn electron_capture_remnant() -> CompactRemnant {
    CompactRemnant::new(RemnantKind::NeutronStar, ELECTRON_CAPTURE_NEUTRON_STAR_MASS)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::stats;

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId};
    use crate::rng::{ObjectKey, Seed, Stream, tags};
    use crate::stellar::draws::StarDrawsParts;
    use crate::units::{MetalFraction, Years};

    fn m(v: f64) -> SolarMasses {
        SolarMasses::new(v)
    }

    fn normal(z: f64) -> StandardNormal {
        StandardNormal::new(z).unwrap()
    }

    /// The draws with a type mark at `word`, a fallback mark that never accepts short of
    /// probability 1, and the median mass.
    fn type_draw(word: u64) -> RemnantDraws {
        RemnantDraws::from_parts(
            Mark::from_word(word),
            Mark::from_word(u64::MAX),
            StandardNormal::ZERO,
        )
    }

    fn stream(seed: u64) -> Stream {
        Stream::open(Seed::new(seed), tags::SELFTEST_STREAM, ObjectKey::galaxy())
    }

    /// A star's remnant draws, as a quadrature over explicit variates would build them.
    fn random_draws(s: &mut Stream) -> RemnantDraws {
        let (remnant_type, fallback) = (s.mark(), s.mark());
        RemnantDraws::from_parts(remnant_type, fallback, normal(s.standard_normal()))
    }

    /// A carbon–oxygen core inside a helium core 1.5 M☉ heavier, a single star's proportions.
    fn collapse(co: f64, draws: RemnantDraws) -> CoreCollapse {
        core_collapse(m(co), m(co + 1.5), draws)
    }

    fn is_black_hole(outcome: CoreCollapse) -> bool {
        match outcome {
            CoreCollapse::NeutronStar { .. } => false,
            CoreCollapse::BlackHole { .. }
            | CoreCollapse::DirectCollapse { .. }
            | CoreCollapse::PulsationalPairInstability => true,
            CoreCollapse::PairInstabilitySupernova => {
                panic!("no pair instability below 45 M☉ of core")
            }
        }
    }

    fn coeffs(z: f64) -> ZCoeffs {
        ZCoeffs::new(MetalFraction::new(z))
    }

    /// Metallicities across the fits' range, log-spaced from 10⁻⁴ to 0.03.
    fn metallicities() -> impl Iterator<Item = f64> {
        (0..=40).map(|i| 1e-4 * math::powf(300.0, f64::from(i) / 40.0))
    }

    /// The paper's figures, transcribed: MM20's Table 1 and section 3, and Belczynski et al.
    /// (2016), section 3 and equation 1.
    #[test]
    fn the_figures_are_the_papers() {
        for (value, paper) in [
            (NEUTRON_STAR_CORE_LIMIT, 2.0),
            (NEUTRON_STAR_STRAND_BREAK, 3.0),
            (BLACK_HOLE_CORE_LIMIT, 7.0),
            (COMPLETE_FALLBACK_CORE_LIMIT, 8.0),
            (MIN_NEUTRON_STAR_MASS, 1.13),
            (MAX_NEUTRON_STAR_MASS, 2.0),
            (ELECTRON_CAPTURE_NEUTRON_STAR_MASS, 1.26),
            (IRON_CORE_MC_BAGB, 2.25),
            (PULSATIONAL_PAIR_INSTABILITY_CORE, 45.0),
            (PAIR_INSTABILITY_CORE, 65.0),
            (PAIR_INSTABILITY_COLLAPSE_CORE, 135.0),
            (PULSATIONAL_PAIR_INSTABILITY_BLACK_HOLE, 45.0 * (1.0 - 0.1)),
            (SINGLE_STAR_WINDOW, 0.1),
            (COMPANION_STRIPPED_WINDOW, 1.0),
        ] {
            assert_same_bits(value.value(), paper);
        }
        assert_eq!(LOW_CORE_NEUTRON_STAR, (1.2, 0.02));
        assert_eq!(UPPER_STRAND, (1.4, 0.5, 0.05));
        assert_eq!(LOWER_STRAND, (1.4, 0.4, 0.05));
        assert_eq!(FALLBACK_BLACK_HOLE, (0.8, 0.5));
    }

    /// A lower draw never turns a black hole into a neutron star: at every core the black holes
    /// are the marks below one cut. Nor does a heavier core, for one mark.
    #[test]
    fn the_type_is_monotone_in_the_uniform_draw_and_in_the_core() {
        let words: Vec<u64> = (0..=512).map(|i| (u64::MAX / 512) * i).collect();
        for i in 0..=220 {
            let co = 0.5 * f64::from(i) / 20.0;
            let kinds: Vec<bool> = words
                .iter()
                .map(|&w| is_black_hole(collapse(co, type_draw(w))))
                .collect();
            let cut = kinds.iter().position(|&bh| !bh).unwrap_or(kinds.len());
            assert!(
                kinds[cut..].iter().all(|&bh| !bh),
                "M_CO = {co}: a higher mark gives a black hole after a neutron star"
            );
            let expected = ((co - 2.0) / 5.0).clamp(0.0, 1.0);
            let share = f64::from(u32::try_from(cut).unwrap()) / 513.0;
            assert!(
                (share - expected).abs() < 2.0 / 513.0,
                "M_CO = {co}: {share} of marks give black holes, against {expected}"
            );
        }
        for &w in &words {
            let mut was_black_hole = false;
            for i in 0..=1_000 {
                let bh = is_black_hole(collapse(0.01 * f64::from(i), type_draw(w)));
                assert!(
                    bh || !was_black_hole,
                    "mark {w:#x}: a heavier core gives a neutron star"
                );
                was_black_hole = bh;
            }
        }
    }

    /// Below M₁ always a neutron star, from M₃ always a black hole, from M₄ always complete
    /// fallback, whatever the draws.
    #[test]
    fn the_break_points_decide_whatever_the_draws() {
        let extremes = [0, 1 << 11, u64::MAX / 2, u64::MAX];
        for &t in &extremes {
            for &f in &extremes {
                let draws =
                    RemnantDraws::from_parts(Mark::from_word(t), Mark::from_word(f), normal(0.3));
                for co in [0.0, 1.44, 1.99, 2.0 - 1e-12] {
                    assert!(!is_black_hole(collapse(co, draws)), "{co}: {draws:?}");
                }
                for co in [7.0, 7.5, 7.999] {
                    assert!(is_black_hole(collapse(co, draws)), "{co}: {draws:?}");
                }
                for co in [8.0, 12.0, 40.0] {
                    assert_eq!(
                        collapse(co, draws),
                        CoreCollapse::DirectCollapse { mass: m(co + 1.5) },
                        "{co}: {draws:?}"
                    );
                }
            }
        }
        // At M₁ itself the probability is 0, and no mark, not even 0, lies below its threshold.
        assert!(!is_black_hole(collapse(2.0, type_draw(0))));
    }

    /// Fresh draws at fixed cores give black holes with probability (`M_CO` − 2) ÷ 5, and complete
    /// fallback among them with probability (`M_CO` − 2) ÷ 6 (MM20, section 3).
    #[test]
    fn black_hole_and_fallback_shares_follow_the_linear_probabilities() {
        let mut s = stream(0x7218);
        for co in [2.5, 3.7, 5.0, 6.2, 6.9, 7.3] {
            let n = 20_000_u32;
            let (mut black_holes, mut complete) = (0_u64, 0_u64);
            for _ in 0..n {
                match collapse(co, random_draws(&mut s)) {
                    CoreCollapse::NeutronStar { .. } => {}
                    CoreCollapse::BlackHole { .. } => black_holes += 1,
                    CoreCollapse::DirectCollapse { .. } => {
                        black_holes += 1;
                        complete += 1;
                    }
                    other => panic!("{other:?} at {co}"),
                }
            }
            let p_bh = (co - 2.0) / 5.0;
            let p_cf = (co - 2.0) / 6.0;
            if p_bh < 1.0 {
                let total = f64::from(n);
                let bh = stats::chi_square_gof(
                    &[black_holes, u64::from(n) - black_holes],
                    &[total * p_bh, total * (1.0 - p_bh)],
                );
                stats::assert_p_value(&format!("black holes at {co}"), bh.p_value, stats::ALPHA);
            } else {
                assert_eq!(black_holes, u64::from(n), "every collapse at {co}");
            }
            #[expect(
                clippy::cast_precision_loss,
                reason = "a count below 2^53 is exact as an f64"
            )]
            let holes = black_holes as f64;
            let cf = stats::chi_square_gof(
                &[complete, black_holes - complete],
                &[holes * p_cf, holes * (1.0 - p_cf)],
            );
            stats::assert_p_value(&format!("fallback at {co}"), cf.p_value, stats::ALPHA);
        }
    }

    /// The plan's test: every neutron star lies in 1.13–2.0 M☉, at every core and at the extremes
    /// of the normal, far beyond any a Box–Muller draw reaches.
    #[test]
    fn neutron_star_masses_lie_in_1_13_to_2_0() {
        let zs = [-40.0, -8.6, -3.0, -1.0, 0.0, 1.0, 3.0, 8.6, 40.0];
        for i in 0..=700 {
            let co = 0.01 * f64::from(i);
            for &z in &zs {
                let mass = neutron_star_mass(co, normal(z)).value();
                assert!((1.13..=2.0).contains(&mass), "M_CO = {co}, z = {z}: {mass}");
            }
        }
        let mut s = stream(0x1132);
        for _ in 0..50_000 {
            let co = 7.0 * s.uniform();
            if let CoreCollapse::NeutronStar { mass } = collapse(co, random_draws(&mut s)) {
                assert!((1.13..=2.0).contains(&mass.value()), "{co}: {mass:?}");
            }
        }
    }

    /// The mean and spread of a neutron star's mass on each of MM20's three branches, by a
    /// quadrature over the normal: 1.2 ± 0.02 M☉ below M₁, 1.4 + 0.5 (`M_CO` − 2) ± 0.05 up to M₂
    /// and 1.4 + 0.4 (`M_CO` − 3) ÷ 4 ± 0.05 above, where the hold at 1.13–2.0 M☉ is far enough
    /// away to change neither by more than a part in 10⁴.
    #[test]
    fn neutron_star_masses_follow_the_three_branches() {
        let n = 20_000;
        for (co, mean, sigma) in [
            (1.5, 1.2, 0.02),
            (2.4, 1.6, 0.05),
            (2.8, 1.8, 0.05),
            (3.0, 1.4, 0.05),
            (5.0, 1.6, 0.05),
            (6.9, 1.79, 0.05),
        ] {
            let masses: Vec<f64> = (0..n)
                .map(|i| {
                    let z = math::normal_quantile((f64::from(i) + 0.5) / f64::from(n));
                    neutron_star_mass(co, normal(z)).value()
                })
                .collect();
            let count = f64::from(n);
            let average = masses.iter().sum::<f64>() / count;
            let spread = (masses
                .iter()
                .map(|x| (x - average) * (x - average))
                .sum::<f64>()
                / count)
                .sqrt();
            assert!((average - mean).abs() < 1e-4, "M_CO = {co}: mean {average}");
            assert!(
                (spread / sigma - 1.0).abs() < 1e-2,
                "M_CO = {co}: σ {spread}"
            );
        }
    }

    /// The truncated normal is the distribution of redrawing until inside: its CDF, from the
    /// testkit's own Φ, returns the rank Φ(z) of the draw it was given, in the body and in both
    /// tails, for ranges on either side of the mean and across it.
    #[test]
    fn a_held_mass_is_the_quantile_of_the_truncated_normal() {
        for (mean, sigma, lower, upper) in [
            (1.2, 0.02, 1.13, 2.0),
            (1.9, 0.05, 1.13, 2.0),
            (1.6, 0.5, 2.0, 3.1),
            (4.0, 0.5, 2.0, 9.0),
            (0.0, 1.0, 3.0, 4.0),
            (0.0, 1.0, -4.0, -3.0),
            (0.0, 1.0, -0.5, 0.25),
        ] {
            let phi = |x: f64| stats::normal_cdf((x - mean) / sigma);
            let inside = phi(upper) - phi(lower);
            for i in 0..=400 {
                let z = -6.0 + 12.0 * f64::from(i) / 400.0;
                let x = truncated_normal(mean, sigma, lower, upper, normal(z));
                assert!(
                    (lower..=upper).contains(&x),
                    "{x} outside [{lower}, {upper}]"
                );
                let rank = (phi(x) - phi(lower)) / inside;
                assert!(
                    (rank - stats::normal_cdf(z)).abs() < 1e-9,
                    "N({mean}, {sigma}) on [{lower}, {upper}] at z = {z}: rank {rank}"
                );
            }
        }
        // By a rejection sample, as MM20 draw it.
        let mut s = stream(0xbeef);
        let (mean, sigma, lower, upper) = (1.6, 0.5, 2.0, 3.1);
        let mut redrawn = Vec::new();
        while redrawn.len() < 20_000 {
            let x = mean + sigma * s.standard_normal();
            if (lower..=upper).contains(&x) {
                redrawn.push(x);
            }
        }
        let mut mapped: Vec<f64> = (0..20_000)
            .map(|_| truncated_normal(mean, sigma, lower, upper, normal(s.standard_normal())))
            .collect();
        let ks = stats::ks_two_sample(&mut redrawn, &mut mapped);
        stats::assert_p_value("redrawn against mapped", ks.p_value, stats::ALPHA);
        // A range of no width gives its one point.
        assert_same_bits(truncated_normal(1.0, 0.5, 2.0, 2.0, normal(1.0)), 2.0);
    }

    /// The plan's test: black holes of 2–5 M☉ exist, and every black hole of partial fallback
    /// lies between the largest neutron star and the helium core.
    #[test]
    fn black_holes_of_2_to_5_solar_masses_exist() {
        let mut s = stream(0x25);
        let (mut light, mut total) = (0_u32, 0_u32);
        for _ in 0..40_000 {
            let co = 2.0 + 6.0 * s.uniform();
            let helium = co + 0.2 + 3.0 * s.uniform();
            match core_collapse(m(co), m(helium), random_draws(&mut s)) {
                CoreCollapse::BlackHole { mass } => {
                    assert!(
                        mass.value() >= 2.0 && mass.value() <= helium,
                        "{co} in {helium}: {mass:?}"
                    );
                    total += 1;
                    light += u32::from(mass.value() <= 5.0);
                }
                CoreCollapse::DirectCollapse { mass } => {
                    assert_same_bits(mass.value(), helium);
                    total += 1;
                    light += u32::from(mass.value() <= 5.0);
                }
                CoreCollapse::NeutronStar { .. } => {}
                other => panic!("{other:?}"),
            }
        }
        assert!(
            light > total / 10,
            "{light} of {total} black holes are of 2–5 M☉"
        );
        // A black hole whose draw would pass its helium core is held to it.
        let held = core_collapse(m(5.0), m(5.2), type_draw(0));
        assert!(matches!(held, CoreCollapse::BlackHole { .. }), "{held:?}");
        let high =
            RemnantDraws::from_parts(Mark::from_word(0), Mark::from_word(u64::MAX), normal(8.0));
        let CoreCollapse::BlackHole { mass } = core_collapse(m(5.0), m(5.2), high) else {
            panic!("a black hole of partial fallback");
        };
        assert!(
            mass.value() <= 5.2 && mass.value() > 5.2 - 1e-9,
            "{mass:?} against its 5.2 M☉ helium core"
        );
    }

    /// Pair instability by the helium core, each range closed below (Belczynski et al. 2016).
    #[test]
    fn pair_instability_follows_the_helium_core() {
        for (helium, expected) in [
            (0.0, PairInstability::None),
            (44.999, PairInstability::None),
            (45.0, PairInstability::Pulsational),
            (64.999, PairInstability::Pulsational),
            (65.0, PairInstability::Disruptive),
            (134.999, PairInstability::Disruptive),
            (135.0, PairInstability::Collapse),
            (150.0, PairInstability::Collapse),
        ] {
            assert_eq!(pair_instability(m(helium)), expected, "{helium}");
        }
        let mut s = stream(0x45);
        for _ in 0..100 {
            let draws = random_draws(&mut s);
            assert_eq!(
                core_collapse(m(35.0), m(44.0), draws).remnant(),
                CompactRemnant::new(RemnantKind::BlackHole, m(44.0)),
                "below 45 M☉ MM20's complete fallback"
            );
            let pulsed = core_collapse(m(38.0), m(50.0), draws);
            assert_eq!(pulsed, CoreCollapse::PulsationalPairInstability);
            assert_eq!(
                pulsed.remnant(),
                CompactRemnant::new(RemnantKind::BlackHole, m(40.5))
            );
            let disrupted = core_collapse(m(60.0), m(80.0), draws);
            assert_eq!(disrupted, CoreCollapse::PairInstabilitySupernova);
            assert_eq!(disrupted.remnant().kind(), RemnantKind::None);
            assert_same_bits(disrupted.remnant().mass().value(), 0.0);
            assert_eq!(
                core_collapse(m(100.0), m(140.0), draws),
                CoreCollapse::DirectCollapse { mass: m(140.0) }
            );
        }
    }

    /// `m_cc(Z)` is the lowest double whose core at the base of the AGB reaches 2.25 M☉, and
    /// agrees with equation 66 inverted by hand, (((2.25⁴ − b38) ÷ b36)^(1 ÷ b37)), to a few ulps.
    /// At Z = 0.02 it is HPT's `M_ec`, 8.203 M☉, which the AGB's own test finds.
    #[test]
    fn the_iron_core_mass_is_the_root_of_the_core_at_the_base_of_the_agb() {
        for z in metallicities() {
            let c = coeffs(z);
            let m_cc = ElectronCaptureWindows::new(&c).iron_core_mass();
            assert!(m_c_bagb(m_cc, &c) >= IRON_CORE_MC_BAGB, "Z = {z}");
            let below = m(m_cc.value().next_down());
            assert!(m_c_bagb(below, &c) < IRON_CORE_MC_BAGB, "Z = {z}");
            let inverted = math::powf(
                (math::powi(IRON_CORE_MC_BAGB.value(), 4) - c.b(38)) / c.b(36),
                1.0 / c.b(37),
            );
            assert!(
                (m_cc.value() / inverted - 1.0).abs() < 1e-14,
                "Z = {z}: {m_cc:?} against {inverted}"
            );
            assert!(
                (6.7..8.35).contains(&m_cc.value()),
                "Z = {z}: {m_cc:?} lies outside the documented range"
            );
        }
        let solar = ElectronCaptureWindows::new(&coeffs(0.02)).iron_core_mass();
        assert!((solar.value() - 8.203).abs() < 1e-3, "{solar:?}");
    }

    /// The plan's test of the windows by metallicity: each ends at `m_cc(Z)`, is 0.1 or 1.0 M☉
    /// wide, closed below and open above, and holds only stars whose core at the base of the AGB is
    /// oxygen–neon (1.6–2.25 M☉), so that electron capture is the physical end the window
    /// replaces.
    #[test]
    fn the_electron_capture_windows_end_at_the_iron_core_mass_at_every_metallicity() {
        for z in metallicities() {
            let c = coeffs(z);
            let windows = ElectronCaptureWindows::new(&c);
            let m_cc = windows.iron_core_mass();
            for (window, width) in [(windows.single(), 0.1), (windows.companion_stripped(), 1.0)] {
                assert_same_bits(window.upper().value(), m_cc.value());
                assert_same_bits(window.lower().value(), m_cc.value() - width);
                assert!(window.contains(window.lower()), "Z = {z}");
                assert!(!window.contains(window.upper()), "Z = {z}");
                assert!(!window.contains(m(window.lower().value().next_down())));
                assert!(window.contains(m(window.upper().value().next_down())));
                let bottom = m_c_bagb(window.lower(), &c).value();
                assert!(
                    (1.6..2.25).contains(&bottom),
                    "Z = {z}: the window starts at a core of {bottom} M☉"
                );
            }
            assert_eq!(windows.window(m(0.1)), windows.single());
        }
    }

    #[test]
    fn electron_capture_leaves_a_1_26_neutron_star() {
        let remnant = electron_capture_remnant();
        assert_eq!(remnant.kind(), RemnantKind::NeutronStar);
        assert_same_bits(remnant.mass().value(), 1.26);
    }

    /// A star's remnant draws are its three `StarDraws` fields, and the same draws give the same
    /// outcome twice, to the bit.
    #[test]
    fn a_stars_draws_give_the_same_remnant_twice() {
        let parts = StarDrawsParts {
            remnant_type: Mark::from_word(0x1234_5678_9abc_def0),
            remnant_fallback: Mark::from_word(0x0fed_cba9_8765_4321),
            remnant_mass: normal(-0.7),
            ..StarDrawsParts::MEDIAN
        };
        let draws = RemnantDraws::of(&StarDraws::from_parts(parts));
        assert_eq!(draws.remnant_type(), Mark::from_word(0x1234_5678_9abc_def0));
        assert_eq!(draws.fallback(), Mark::from_word(0x0fed_cba9_8765_4321));
        assert_same_bits(draws.mass().value(), -0.7);
        for co in [1.5, 2.5, 4.0, 6.5, 9.0] {
            let (a, b) = (collapse(co, draws), collapse(co, draws));
            assert_eq!(a, b);
            assert_same_bits(a.remnant().mass().value(), b.remnant().mass().value());
        }
    }

    /// Writes an outcome as its variant and the remnant's mass.
    fn write_outcome(w: &mut GoldenWriter, label: &str, outcome: CoreCollapse) {
        let name = match outcome {
            CoreCollapse::NeutronStar { .. } => "neutron star",
            CoreCollapse::BlackHole { .. } => "black hole",
            CoreCollapse::DirectCollapse { .. } => "direct collapse",
            CoreCollapse::PulsationalPairInstability => "pulsational pair instability",
            CoreCollapse::PairInstabilitySupernova => "pair instability",
        };
        w.f64(&format!("{label} {name}"), outcome.remnant().mass().value());
    }

    /// Pins, bit for bit, `m_cc` and its windows at the five metallicities of the SSE comparison,
    /// and the outcome of core collapse over cores from the lightest to pair instability for the
    /// draws of two pinned stars (those of the `stellar/star_draws` golden), the median star and
    /// three sets of explicit variates, so that any change to the recipe's arithmetic or its
    /// reading of the draws is seen. Nothing generated reads these functions before P06.T18.d.
    #[test]
    fn the_recipe_is_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for z in [1e-4, 1e-3, 4e-3, 0.02, 0.03] {
            let windows = ElectronCaptureWindows::new(&coeffs(z));
            w.line(&format!("Z = {z}"));
            w.f64("m_cc", windows.iron_core_mass().value());
            w.f64("single window lower", windows.single().lower().value());
            w.f64(
                "companion-stripped window lower",
                windows.companion_stripped().lower().value(),
            );
        }
        let a = SystemId::from_parts(
            Layer::A,
            GenCell::new(CellSize::Ly8, [652, -4_584, 2_047]).unwrap(),
            7,
        )
        .unwrap();
        let e = SystemId::from_parts(
            Layer::E,
            GenCell::new(CellSize::Ly128, [-40, 17, 0]).unwrap(),
            2,
        )
        .unwrap();
        let stars = [
            (
                "seed 0x0123456789abcdef body A 0",
                RemnantDraws::of(&StarDraws::for_star(
                    Seed::new(0x0123_4567_89ab_cdef),
                    BodyId::new(a, 0),
                )),
            ),
            (
                "seed 42 body E 0",
                RemnantDraws::of(&StarDraws::for_star(Seed::new(42), BodyId::new(e, 0))),
            ),
            ("median", RemnantDraws::of(&StarDraws::median())),
            (
                "low marks, low normal",
                RemnantDraws::from_parts(
                    Mark::from_word(u64::MAX / 7),
                    Mark::from_word(u64::MAX / 5),
                    normal(-1.7),
                ),
            ),
            (
                "high marks, high normal",
                RemnantDraws::from_parts(
                    Mark::from_word(u64::MAX / 8 * 7),
                    Mark::from_word(u64::MAX / 10 * 9),
                    normal(2.3),
                ),
            ),
            (
                "black hole, partial fallback",
                RemnantDraws::from_parts(
                    Mark::from_word(0),
                    Mark::from_word(u64::MAX),
                    normal(0.4),
                ),
            ),
        ];
        let cores = [
            (1.44, 2.3),
            (1.9, 2.9),
            (2.2, 3.3),
            (2.5, 3.7),
            (2.95, 4.3),
            (3.5, 5.0),
            (5.0, 7.0),
            (6.5, 8.5),
            (7.5, 9.8),
            (10.0, 13.0),
            (38.0, 50.0),
            (60.0, 80.0),
            (110.0, 140.0),
        ];
        for (name, draws) in stars {
            w.line("");
            w.line(name);
            for (co, helium) in cores {
                write_outcome(
                    &mut w,
                    &format!("M_CO {co} in {helium}:"),
                    core_collapse(m(co), m(helium), draws),
                );
            }
        }
        golden!("stellar/collapse", w.as_str());
    }

    // -----------------------------------------------------------------------------------------
    // P06.T18.d: the recipe on real tracks.

    /// What the massive stars of a population leave, counted over a sample.
    #[derive(Debug, Default)]
    struct Population {
        stars: u32,
        neutron_stars: u32,
        electron_captures: u32,
        black_holes: u32,
        complete_fallback: u32,
        light_black_holes: u32,
        white_dwarfs: u32,
        no_remnant: u32,
        neutron_star_masses: (f64, f64),
    }

    impl Population {
        fn compact(&self) -> u32 {
            self.neutron_stars + self.black_holes
        }

        fn share(part: u32, whole: u32) -> f64 {
            f64::from(part) / f64::from(whole)
        }
    }

    /// The Kroupa sample of P06.T18's tests: `n` single stars of 8–150 M☉ at Z = 0.02, drawn as
    /// m^−2.3 (Kroupa 2001 above 0.5 M☉) from `seed`, with their own Reimers η and remnant draws
    /// and every other draw at its median, each built to its death under the generator's options.
    ///
    /// The track covers 0.1–100 M☉ until P06.T14, so a star above 100 M☉ is built at 100, as
    /// `Track` itself clamps; at Z = 0.02 every star above about 60 M☉ ends on the same Wolf–Rayet
    /// plateau of carbon–oxygen core, so the clamp moves none of these shares' categories.
    fn kroupa_population(seed: u64, n: u32) -> Population {
        use crate::rng::PowerLaw;
        use crate::stellar::sse::{MAX_INITIAL_MASS, Track};
        use crate::stellar::{Composition, Phase};

        use super::super::DeathKind;

        let imf = PowerLaw::new(2.3, 8.0, 150.0).expect("a valid power law");
        let mut s = stream(seed);
        let mut population = Population {
            neutron_star_masses: (f64::INFINITY, f64::NEG_INFINITY),
            ..Population::default()
        };
        for _ in 0..n {
            let m0 = s.power_law(&imf);
            let eta = normal(s.standard_normal());
            let draws = random_draws(&mut s);
            let star = StarDraws::from_parts(StarDrawsParts {
                eta,
                remnant_type: draws.remnant_type(),
                remnant_fallback: draws.fallback(),
                remnant_mass: draws.mass(),
                ..StarDrawsParts::MEDIAN
            });
            let track = Track::full(
                m(m0.min(MAX_INITIAL_MASS.value())),
                &Composition::SOLAR,
                &star,
            );
            let death = track.death().expect("a full track dies");
            let remnant = track.remnant().expect("a full track leaves its remnant");
            population.stars += 1;
            match remnant.kind() {
                RemnantKind::NeutronStar => {
                    population.neutron_stars += 1;
                    let (lo, hi) = population.neutron_star_masses;
                    let mass = remnant.mass().value();
                    population.neutron_star_masses = (lo.min(mass), hi.max(mass));
                    if death.kind() == DeathKind::ElectronCapture {
                        population.electron_captures += 1;
                    }
                }
                RemnantKind::BlackHole => {
                    population.black_holes += 1;
                    if death.kind() == DeathKind::DirectCollapse {
                        population.complete_fallback += 1;
                    }
                    if remnant.mass().value() <= 5.0 {
                        population.light_black_holes += 1;
                    }
                }
                RemnantKind::WhiteDwarf => population.white_dwarfs += 1,
                RemnantKind::None => population.no_remnant += 1,
            }
            let after = track.state_at(Years::new(death.age().value() * (1.0 + 1e-9)));
            assert!(after.phase().is_remnant(), "{m0} M☉: {:?}", after.phase());
            if remnant.kind() == RemnantKind::BlackHole {
                assert_eq!(after.phase(), Phase::BlackHole);
            }
        }
        population
    }

    /// The seed of the population tests' sample, and its size: the three shares' sampling errors
    /// are then about 0.4%, 0.6% and 0.2%, well inside their bands.
    const POPULATION_SEED: u64 = 0x0618_d000_0000_0001;
    const POPULATION_STARS: u32 = 20_000;

    /// Three shares of one Kroupa sample of 8–150 M☉ at Z = 0.02 (P06.T18), on the generator's
    /// tracks, checked in one test so that the 20,000 tracks are built once:
    ///
    /// - black holes are 38 ± 5% of the compact remnants (the research figure behind "about four
    ///   fifths" of layer-E remnants staying), black holes of 2–5 M☉ exist, and every neutron star
    ///   lies in 1.13–2.0 M☉;
    /// - complete fallback is 70–80% of black holes (the brainstorm's "three quarters of them");
    /// - electron captures are 2–6% of the neutron stars, all single stars (the 0.1 M☉ window of
    ///   design note 12).
    #[test]
    #[ignore = "slow: 20,000 full tracks of 8–150 M☉"]
    fn remnant_shares_of_a_kroupa_sample_on_real_tracks() {
        let p = kroupa_population(POPULATION_SEED, POPULATION_STARS);

        let share = Population::share(p.black_holes, p.compact());
        eprintln!("{p:?}: black holes {share:.4} of compact remnants");
        assert!((0.33..=0.43).contains(&share), "black holes: {share}");
        assert!(p.light_black_holes > 0, "no black hole of 2–5 M☉: {p:?}");
        let (lo, hi) = p.neutron_star_masses;
        assert!(
            lo >= MIN_NEUTRON_STAR_MASS.value() && hi <= MAX_NEUTRON_STAR_MASS.value(),
            "{lo}–{hi}"
        );

        let share = Population::share(p.complete_fallback, p.black_holes);
        eprintln!("complete fallback {share:.4} of black holes");
        assert!((0.70..=0.80).contains(&share), "complete fallback: {share}");

        let share = Population::share(p.electron_captures, p.neutron_stars);
        eprintln!("electron captures {share:.4} of neutron stars");
        assert!((0.02..=0.06).contains(&share), "electron captures: {share}");
    }
}
