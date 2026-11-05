//! The hold on how a white dwarf's core ends where it has kept its kind, and where an oxygen–neon
//! core collapses (plan 11, P11.T4's fix, at [`CORE_ENDINGS_VERSION`]; research-leans-38).
//!
//! As built under the union's other holds, a carbon–oxygen core that a merger product or a swollen
//! giant keeps (`Engine::dwarf_cores`) disrupts and leaves nothing where its track would collapse
//! it, at the Chandrasekhar mass on the way (`Engine::core_limit_age`): 378 such products in
//! 17,814 pairs of the dwarf-limit census's layer D (2026-11-05; 365 in 15,222 at version 25),
//! nearly all early-AGB products whose track, its `Mc,BAGB` 2.25 M☉ or more, collapses an iron
//! core, several times layer D's Type Ia. And an
//! oxygen–neon core collapses at two masses: at the capture mass, 1.37 M☉, where the default
//! recipe caps a single star's (`Builder::oxygen_neon_cap`), and at the Chandrasekhar mass, 1.44
//! M☉, inside the single star's electron-capture window, as a dwarf fed by a companion, as a white
//! dwarfs' merger's product and in the dwarf limit's arms.
//!
//! From 29 ([`CoreEndingRules::Unified`]):
//!
//! - the track's own `Mc,BAGB` decides how a kept core ends, whatever kind it keeps, a modelling
//!   choice after BSE, whose merger product takes the type of the giant it makes (BSE table 2;
//!   research-leans-38): where the track collapses an iron core, so does the kept core; the kept
//!   kind sets only what is left where the track leaves a white dwarf (a dwarf of that kind) or
//!   collapses an oxygen–neon core in its window (a carbon–oxygen core there disrupts, with no
//!   Type Ia candidate); and a kept carbon–oxygen core has no limit of its own
//!   (`swollen_dwarf::SwollenDeath::of`, `Engine::core_limit_age`);
//! - under [`RemnantRecipe::MandelMuller2020`] every oxygen–neon core collapses at the capture
//!   mass, where electron captures on ²⁴Mg and ²⁰Ne collapse it (Miyaji et al. 1980, PASJ 32, 303;
//!   Nomoto 1984, ApJ 277, 791; ruling 57's [`OXYGEN_NEON_CAPTURE_MASS`]): a window's star whose
//!   thermal pulses start below it, which the track caps there and then collapses
//!   (`Builder::pulses_cap`), a fed oxygen–neon dwarf, a white dwarfs' merger's oxygen–neon
//!   product, a bare oxygen–neon core a strip leaves, a helium star's at its end, and the dwarf
//!   limit's oxygen–neon arms ([`CoreEndingRules::collapse_mass`]). A single star in the window,
//!   whose core is above the capture mass from the second dredge-up, still collapses at `Mc,SN`
//!   (lean, 2026-11-03). [`RemnantRecipe::Hurley2000`] keeps the Chandrasekhar mass, as
//!   carbon–oxygen and helium dwarfs do;
//! - a helium star whose core reaches the Chandrasekhar mass collapses by its mass, which stands
//!   for `Mc,BAGB` (HPT section 6.1; research-leans-40): an iron core from 2.25 M☉, and from
//!   1.6 M☉, where HPT section 6 types the core oxygen–neon, an electron capture under the track's
//!   recipe (plan 06's `helium_star_end`, which under the held arm collapses an iron core from
//!   1.6 M☉; MM20 section 3's 1.26 M☉ neutron star by default);
//! - a merger's helium-giant product that no star holds ends by its mass too, as that helium star
//!   ends: an iron core from 2.25 M☉, an electron capture from 1.6 M☉, a thermonuclear disruption
//!   that leaves nothing, with no Type Ia candidate, from 1.44 M☉, and a dwarf below
//!   (`dwarf_limit::DwarfLimitRules::unplaced`); a thermally pulsing AGB product keeps the 1.438
//!   M☉ carbon–oxygen core of its iron core;
//! - a core the integration brings to the Chandrasekhar mass has reached it within
//!   [`CORE_ROUNDING`] ([`CoreEndingRules::below_the_chandrasekhar_mass`]), so that no kind of
//!   death turns on the last bits of a helium star's core or the pulses' at `Mc,SN`.

use crate::GeneratorVersion;
use crate::stellar::Phase;
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::collapse::OXYGEN_NEON_CAPTURE_MASS;
use crate::stellar::remnant::structure::CHANDRASEKHAR_MASS;
use crate::stellar::rules::StellarRules;
use crate::units::SolarMasses;

/// The relative rounding to which a track's core that reaches the Chandrasekhar mass is that mass
/// under [`CoreEndingRules::Unified`]: the thermal pulses' and a helium star's cores reach it to
/// within 10⁻¹⁵ of it (1.439 999 999 999 998 6 M☉ measured, 2026-11-03), not to the bit.
pub(crate) const CORE_ROUNDING: f64 = 1e-9;

/// The generator version whose stellar rules end a kept white dwarf's core as its track does and
/// collapse every oxygen–neon core at one mass and by electron capture (P11.T4's fix):
/// `Engine::die`'s swollen death, `Engine::core_limit_age`, `Builder::pulses_cap`, plan 06's
/// `helium_star_end`, the dwarfs' collapse masses and the unplaced helium giant take
/// [`CoreEndingRules::Unified`].
///
/// Below it every pair and star takes the held arm, [`CoreEndingRules::Held`], the code of version
/// 28 verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_29_hold()`. The bump to 29 deletes the hold, not the rule: this module
/// with [`CoreEndingRules`] (but the rules of [`CoreEndingRules::collapse_mass`] and
/// [`CoreEndingRules::below_the_chandrasekhar_mass`], which move to their callers), the stellar
/// rules' `core_endings` field and its reads, the brief's read of
/// [`CoreEndingRules::fitted_tables_hold`], and the tripwire
/// (`the_core_endings_are_held_only_until_the_29_bump`); it refits the pair-light and reach tables,
/// which the engine's runs fit, and the kick tables, whose fingerprint moves (the movers of
/// `stellar/rules/core_endings_hold.rs`).
pub(crate) const CORE_ENDINGS_VERSION: GeneratorVersion = GeneratorVersion::new(29);

/// How a kept white dwarf's core ends and where an oxygen–neon core collapses (the stellar rules'
/// `core_endings` field, the hold of [`CORE_ENDINGS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CoreEndingRules {
    /// Version 28's and before: a kept carbon–oxygen core disrupts where its track would collapse
    /// it, at the Chandrasekhar mass on the way, and an oxygen–neon core collapses at the
    /// Chandrasekhar mass but where a single star's is capped below its window.
    Held,
    /// Version 29's: a kept core ends as its track does but for what is left, and every
    /// oxygen–neon core collapses at the capture mass under the default recipe.
    Unified,
}

impl CoreEndingRules {
    /// The mass, M☉, at which a white dwarf of `phase` collapses under `recipe`: the capture mass,
    /// 1.37 M☉, for an oxygen–neon one under [`CoreEndingRules::Unified`] and
    /// [`RemnantRecipe::MandelMuller2020`], and the Chandrasekhar mass otherwise.
    #[must_use]
    pub(crate) fn collapse_mass(self, phase: Phase, recipe: RemnantRecipe) -> SolarMasses {
        match (self, recipe) {
            (Self::Unified, RemnantRecipe::MandelMuller2020)
                if phase == Phase::OxygenNeonWhiteDwarf =>
            {
                OXYGEN_NEON_CAPTURE_MASS
            }
            (
                Self::Held | Self::Unified,
                RemnantRecipe::MandelMuller2020 | RemnantRecipe::Hurley2000,
            ) => CHANDRASEKHAR_MASS,
        }
    }

    /// Whether a core of `core` is below the Chandrasekhar mass, 1.44 M☉, where it has not reached
    /// it: under [`CoreEndingRules::Held`] to the bit, and under [`CoreEndingRules::Unified`] by
    /// more than [`CORE_ROUNDING`] of it.
    #[must_use]
    pub(crate) fn below_the_chandrasekhar_mass(self, core: SolarMasses) -> bool {
        match self {
            Self::Held => core < CHANDRASEKHAR_MASS,
            Self::Unified => core.value() < CHANDRASEKHAR_MASS.value() * (1.0 - CORE_ROUNDING),
        }
    }

    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold for a pair under this arm: under the version's arm only, until the bump refits
    /// them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).core_endings
    }
}
