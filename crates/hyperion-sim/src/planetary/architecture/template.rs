//! Class templates: what each architecture class places, as data (plan 14, P14.T5).
//!
//! A [`ClassTemplate`] is the ordered list of the [`PlanetGroup`]s its class places, inside out,
//! each with a count law, a mass range and law, a starting location, a spacing family, an
//! eccentricity law and an origin, and the class's belts. P14.T8's placers interpret it; nothing
//! here draws. The figures are the "What it holds" column of the [class
//! table](super#the-classes-p14t4a), with their sources below.
//!
//! | Class | Groups, inside out |
//! | ----- | ------------------ |
//! | `Barren` | none |
//! | `TerrestrialOnly` | rocky of 0.05–2 M⊕ from 0.2–0.5 au × √L to the snow line, as many as the spacing fits (at most 10); ice-rich 0–3 of 0.02–5 M⊕ from 1–2 snow-line radii |
//! | `CompactMulti` | a chain of 1–20 M⊕, first period by Mulders et al. (1–50 days), count zero-truncated Poisson (mean 3.5 at 1 M☉, 6.1 at and below 0.48 M☉, at most 10); hot variant in 40%: 1–2 planets |
//! | `CompactWithColdGiant` | the chain; giants 1–2 of 0.3–10 M♃ at 1–3 snow-line radii, Kipping's Betas by period |
//! | `SolarLike` | rocky as `TerrestrialOnly`'s, up to the giants' chaotic zones; giants 1–3 at 1–2 snow-line radii, low e; ice giants 0–2 of 10–30 M⊕ beyond them; both belts |
//! | `EccentricGiant` | giants 1–2 at 0.5–5 au × √L, scattered in, Beta(0.867, 3.03); a survivor 0–1 of 0.05–10 M⊕ |
//! | `WarmGiant` | a giant at 10–200 days, log-uniform, Kipping's Betas by period, migrated; in half of systems 1–2 companions of 1–20 M⊕ flanking it |
//! | `HotJupiter` | a giant at 1–10 days, log-normal about 3.5 days, migrated, nothing else inside 100 days; in 60% of systems an outer giant of 1–10 M♃ at 2–8 snow-line radii |
//! | `SubstellarCompact` | a chain of 0.01–2 M⊕, first period and count as `CompactMulti`'s |
//!
//! # Sources
//!
//! - **Chains.** The first period follows Mulders et al.'s (2018, AJ 156, 24, Table 2) innermost
//!   planet, dN ÷ d log P ∝ P^1.6 below a break at 12 (+3 −2) days and P^−0.9 above it,
//!   truncated here to 1–50 days (plan 14's range). Mulders, Pascucci and Apai (2015, ApJ 798,
//!   112, abstract) find that break at a semi-major axis ∝ M★^⅓, which is a period independent
//!   of the host's mass, so the same law serves every host. The count is a zero-truncated Poisson
//!   with rate λ = 3.38 (max(M, 0.48 M☉) ÷ M☉)^−0.82, at most 10: a mean of 3.5 at 1 M☉ (plan
//!   14), and 6.1 at and below 0.48 M☉, Ballard and Johnson's (2016, ApJ 816, 66, abstract) 6.1 ±
//!   1.9 planets in an M dwarf's coplanar system, held below the median mass of the Kepler M
//!   dwarfs (Dressing and Charbonneau 2015, ApJ 807, 45, §2: median 0.47 R☉); 2.9 at 1.3 M☉,
//!   falling as Yang, Xie and Zhou's (2020, AJ 159, 164, abstract) multiplicities do towards F
//!   stars. Ballard and Johnson's planets lie at 1–200 days, and a chain placed outward from
//!   about 12 days at P14.T6.b's spacing leaves about 30% of an M dwarf's beyond 200 days, so the
//!   count is of the chain and not of their window (P14.T10.b measures the window). The cap of
//!   10, where plan 14 had 7, under which a mean of 6.1 cannot be reached, is Mulders et al.'s
//!   (2018) planets per system, which they fix "because it is not well constrained in the
//!   fitting", and above Kepler-90's eight: about a tenth of M-dwarf chains reach it. The hot
//!   variant's 40% is Mulders et al.'s (2018, Table 2) isotropic fraction, 0.38 ± 0.08 (their
//!   isotropic systems keep all their planets; here the variant has 1–2); Ballard and Johnson
//!   find 55 (+23 −12)% of M-dwarf systems single or inclined. Zhu et al. (2018, ApJ 860, 101,
//!   abstract) read the same data as "the fewer planets in a system, the hotter it is
//!   dynamically", of which the variant is the two-state form.
//! - **Giants.** Masses 0.3–10 M♃ with dN ÷ d ln M ∝ M^−0.31 (Cumming et al. 2008, PASP 120,
//!   531, abstract: α = −0.31 ± 0.2). Eccentricities from Kipping (2013, MNRAS 434, L51):
//!   Beta(0.867, 3.03) for all radial-velocity planets (abstract, Table 1), which
//!   `EccentricGiant` takes; and, for every other giant, Beta(0.697, 3.27) inside the sample's
//!   median period of 382.3 days and Beta(1.12, 3.09) beyond it (Table 2), chosen by the drawn
//!   period ([`EccentricityLaw::BetaByPeriod`]), since a location scaled by √L or the snow line
//!   crosses 382.3 days at different radii for different hosts. The warm giant's 10–200 days is
//!   Huang, Wu and Triaud's (2016, ApJ 825, 98, abstract) definition of a warm Jupiter, in
//!   period, so that no host's warm giant is a hot one (plan 14 had 0.1–1 au × √L, which is
//!   1.6–49 days around a 0.5 M☉ host); it is marked migrated from a formation zone the class
//!   does not fix, since Huang et al. "propose that these warm Jupiters are formed in-situ" for
//!   the half with companions, while plan 14 reads the class as disc migration. The hot
//!   Jupiter's period is log-normal about 3.5 days with σ = 0.15 dex, truncated to 1–10 days:
//!   plan 14's centre, on the pile-up "close to 3 days" (Cumming et al. 2008, §3.3.2 and
//!   Fig. 12); the width, which puts two thirds of them at 2.5–5 days, is this module's choice.
//!   Its outer giant, of 1–10 M♃ at 2–8 snow-line radii (4.5–18 au around the zero-age Sun) in
//!   60% of systems, lies in Bryan et al.'s (2016, ApJ 821, 89, abstract) range in 56% of
//!   hot-Jupiter systems, against their
//!   52 ± 5% of giant hosts with a 1–20 M♃ companion at 5–20 au and their finding that hot giants
//!   "may be more likely to have an outer companion than cold gas giants". Huang et al. find no
//!   companion inside 50 days of a hot Jupiter; the 100 days of `quiet_inside` is plan 14's. The
//!   warm giant's companions in half of systems are Huang et al.'s "half of the warm Jupiters
//!   are closely flanked by small companions".
//! - **Small planets.** Rocky 0.05–2 M⊕, chains 1–20 M⊕, substellar chains 0.01–2 M⊕ and the
//!   hot variant's 1–2 planets are plan 14's; their masses are P14.T7's correlated law
//!   ([`MassLaw::Correlated`]), held to the range. A rocky group fills its reach ([`CountLaw::Fill`],
//!   ruling 60): P14.T8.b places "rocky planets from about 0.3 au × √L to the snow line", and
//!   plan 14's count of 2–6, drawn uniformly, left them short of it, the habitable zone included,
//!   at P14.T6.b's terrestrial spacing of about 30 mutual Hill radii (the outermost rocky planet at
//!   0.66 of the zone's inner edge in the median system, η⊕ 0.09 against Bryson et al.'s (2021, AJ
//!   161, 36, Table 3) 0.37–0.60). Filled to the snow line, most groups have 5–10 planets, 38% of
//!   `TerrestrialOnly`'s about Sun-like stars reaching the cap of 10, the chain's (Mulders et al.
//!   2018 fix 10 planets per system), and η⊕ is 0.38 (P14.T10.b). The Solar System, with giants,
//!   is a `SolarLike` system, whose rocky group stops at its giants' chaotic zones: its four
//!   rocky planets end at 1.52 au, inside its snow line. The ice-rich bodies' 0.02–5 M⊕, the survivor's
//!   0.05–10 M⊕ and the ice giants' 10–30 M⊕ (Uranus 14.5, Neptune 17.1; below the spacing's
//!   giant mass of 0.1 M♃) are this module's, where plan 14 gave none.
//! - **Spacing and eccentricity.** The families of P14.T6.b ([`SpacingFamily`]) and the
//!   eccentricities of P14.T8.d: Rayleigh σ = 0.04 for cold chains, half-normal σ = 0.3 for the hot
//!   variant (Xie et al. 2016; Van Eylen et al. 2019), Rayleigh σ = 0.05 for the groups of the
//!   Solar-like and terrestrial classes; those plans re-check their figures.

use super::ArchitectureClass;
use crate::units::consts::{EARTH_MASS_KG, JUPITER_MASS_KG};
use crate::units::{Days, EarthMasses, SolarMasses};

/// One Jupiter mass in Earth masses, 317.8, from the two constants' shared G.
pub const EARTH_MASSES_PER_JUPITER_MASS: f64 = JUPITER_MASS_KG / EARTH_MASS_KG;

/// What role a group plays in its system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GroupRole {
    /// Rocky planets inside the snow line.
    Rocky,
    /// Ice-rich bodies beyond the snow line, in a system without giants.
    IceRich,
    /// A compact chain of super-Earths and sub-Neptunes.
    Chain,
    /// Gas giants.
    Giant,
    /// Ice giants beyond the gas giants.
    IceGiant,
    /// A small planet that survived its giants' scattering.
    Survivor,
    /// Small planets flanking a warm giant.
    Companions,
}

/// How many bodies a group places.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CountLaw {
    /// Uniform on `min..=max`.
    Uniform {
        /// The least count.
        min: u8,
        /// The greatest count.
        max: u8,
    },
    /// A zero-truncated Poisson law of rate
    /// λ(M) = `rate` × (max(M, `mass_floor`) ÷ M☉)^`mass_exponent`, with a draw above `max` held
    /// at `max`.
    ZeroTruncatedPoisson {
        /// λ at 1 M☉.
        rate: f64,
        /// How λ scales with the host's mass.
        mass_exponent: f64,
        /// The mass below which λ is held.
        mass_floor: SolarMasses,
        /// The greatest count.
        max: u8,
    },
    /// As many bodies as the group's spacing fits between its first body and the end of its
    /// reach, at most `max`: no draw, the count being where the walk outward stops (P14.T8.b's
    /// rocky planets "from about 0.3 au × √L to the snow line"; ruling 60).
    Fill {
        /// The greatest count.
        max: u8,
    },
}

impl CountLaw {
    /// The least and greatest counts.
    #[must_use]
    pub const fn range(&self) -> (u8, u8) {
        match *self {
            Self::Uniform { min, max } => (min, max),
            Self::ZeroTruncatedPoisson { max, .. } | Self::Fill { max } => (1, max),
        }
    }

    /// The Poisson rate λ for a host of `host_mass`, or `None` for a uniform law.
    #[must_use]
    pub fn poisson_rate(&self, host_mass: SolarMasses) -> Option<f64> {
        match *self {
            Self::Uniform { .. } | Self::Fill { .. } => None,
            Self::ZeroTruncatedPoisson {
                rate,
                mass_exponent,
                mass_floor,
                ..
            } => Some(held_rate(rate, mass_exponent, mass_floor, host_mass)),
        }
    }

    /// The mean count for a host of `host_mass`, with the cap applied; for a [`Fill`](Self::Fill)
    /// law, which draws nothing, its cap.
    #[must_use]
    pub fn mean(&self, host_mass: SolarMasses) -> f64 {
        match *self {
            Self::Uniform { min, max } => f64::midpoint(f64::from(min), f64::from(max)),
            Self::Fill { max } => f64::from(max),
            Self::ZeroTruncatedPoisson {
                rate,
                mass_exponent,
                mass_floor,
                max,
            } => {
                let lambda = held_rate(rate, mass_exponent, mass_floor, host_mass);
                let zero = crate::math::exp(-lambda);
                // P(N = k) for k = 1..max − 1 by recurrence, and P(N ≥ max) held at max.
                let mut term = zero;
                let mut below = zero;
                let mut mean = 0.0;
                for k in 1..max {
                    term *= lambda / f64::from(k);
                    below += term;
                    mean += f64::from(k) * term;
                }
                mean += f64::from(max) * (1.0 - below);
                mean / (1.0 - zero)
            }
        }
    }
}

/// λ = `rate` × (max(M, `floor`) ÷ M☉)^`exponent`.
#[must_use]
fn held_rate(rate: f64, exponent: f64, floor: SolarMasses, host_mass: SolarMasses) -> f64 {
    let m = if host_mass < floor { floor } else { host_mass };
    rate * crate::math::powf(m.value(), exponent)
}

/// How a group's masses are drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MassLaw {
    /// P14.T7's correlated law: a characteristic mass per group from the disc's solids, and
    /// members scattered about it ("peas in a pod"), held to the range.
    Correlated,
    /// dN ÷ d ln M ∝ M^`index` over the range.
    PowerLaw {
        /// The index.
        index: f64,
    },
    /// Uniform in ln M over the range.
    LogUniform,
}

/// A group's mass range and law.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassRange {
    min: EarthMasses,
    max: EarthMasses,
    law: MassLaw,
}

impl MassRange {
    /// The least mass.
    #[must_use]
    pub const fn min(&self) -> EarthMasses {
        self.min
    }

    /// The greatest mass.
    #[must_use]
    pub const fn max(&self) -> EarthMasses {
        self.max
    }

    /// The law within the range.
    #[must_use]
    pub const fn law(&self) -> MassLaw {
        self.law
    }
}

/// How the period of a group's first body is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PeriodLaw {
    /// dN ÷ d log P ∝ (P ÷ `break_period`)^`rising` below the break and ^`falling` above it,
    /// truncated to `min`–`max`: drawn inside the range, never clamped to its ends (Mulders et al.
    /// 2018).
    BrokenPowerLaw {
        /// The break.
        break_period: Days,
        /// The index below the break.
        rising: f64,
        /// The index above the break.
        falling: f64,
        /// The shortest period.
        min: Days,
        /// The longest period.
        max: Days,
    },
    /// log₁₀ P normal about log₁₀ `median` with σ `sigma_dex`, truncated to `min`–`max`.
    LogNormal {
        /// The median.
        median: Days,
        /// The width, in dex.
        sigma_dex: f64,
        /// The shortest period.
        min: Days,
        /// The longest period.
        max: Days,
    },
    /// Uniform in log P between `min` and `max`.
    LogUniform {
        /// The shortest period.
        min: Days,
        /// The longest period.
        max: Days,
    },
}

impl PeriodLaw {
    /// The shortest and longest periods.
    #[must_use]
    pub const fn range(&self) -> (Days, Days) {
        match *self {
            Self::BrokenPowerLaw { min, max, .. }
            | Self::LogNormal { min, max, .. }
            | Self::LogUniform { min, max } => (min, max),
        }
    }
}

/// Where a group's first body goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Location {
    /// At a period drawn from the law.
    Period(PeriodLaw),
    /// Log-uniform between `inner` and `outer` au × √(L ÷ L☉), with the host's zero-age
    /// luminosity (design note 6).
    ScaledAu {
        /// The inner end, in au × √L.
        inner: f64,
        /// The outer end, in au × √L.
        outer: f64,
    },
    /// Log-uniform between `inner` and `outer` snow-line radii.
    SnowLines {
        /// The inner end, in snow-line radii.
        inner: f64,
        /// The outer end, in snow-line radii.
        outer: f64,
    },
    /// The next orbit the spacing allows beyond the previous group.
    Outward,
    /// Beside the previous group's planet, inside or outside it, at the group's spacing.
    Flanking,
}

/// How far out a group may extend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    /// No farther than the snow line.
    InsideSnowLine,
    /// As far as its count, the disc and the zone allow.
    Open,
}

/// Which of P14.T6.b's spacing laws a group's pairs follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpacingFamily {
    /// Small planets: a mean spacing normal about 17 mutual Hill radii, σ = 2.5, held to 13–24
    /// (the brainstorm's 14–20; Weiss et al. 2018; Pu and Wu 2015).
    SmallPlanets,
    /// The terrestrial groups of `SolarLike` and `TerrestrialOnly`: about 30, σ = 8.
    Terrestrial,
    /// Giant pairs: about 9, σ = 2.
    Giants,
}

/// How a group's eccentricities are drawn (P14.T8.d).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EccentricityLaw {
    /// Rayleigh with scale `sigma`.
    Rayleigh {
        /// The scale.
        sigma: f64,
    },
    /// Half-normal with scale `sigma`.
    HalfNormal {
        /// The scale.
        sigma: f64,
    },
    /// Beta(`a`, `b`) (Kipping 2013).
    Beta {
        /// The first shape.
        a: f64,
        /// The second shape.
        b: f64,
    },
    /// Beta(`short_a`, `short_b`) for a body whose period is under `split`, and
    /// Beta(`long_a`, `long_b`) otherwise: Kipping's (2013) two populations, chosen by the drawn
    /// period rather than by the group, so that the law is right for every host mass.
    BetaByPeriod {
        /// The period that divides the two.
        split: Days,
        /// The first shape inside the split.
        short_a: f64,
        /// The second shape inside the split.
        short_b: f64,
        /// The first shape beyond it.
        long_a: f64,
        /// The second shape beyond it.
        long_b: f64,
    },
}

/// Where a group's bodies formed, which fixes their composition (P14.T4.a: a migrated body keeps
/// the composition of where it formed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// Where they are placed: rock inside the snow line, rock and ice beyond it.
    InSitu,
    /// Beyond the snow line, then moved inward: `formed_beyond_snow_line` for every body.
    BeyondSnowLine,
    /// Moved inward from where the class does not fix; each body's origin is set when it is
    /// placed (P14.T8, T11).
    Migrated,
}

impl Origin {
    /// Whether the group moved from where it formed.
    #[must_use]
    pub const fn is_migrated(self) -> bool {
        match self {
            Self::InSitu => false,
            Self::BeyondSnowLine | Self::Migrated => true,
        }
    }
}

/// The dynamically hot variant of a chain (the Kepler dichotomy), which replaces it in a share of
/// systems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotVariant {
    probability: f64,
    count: CountLaw,
    eccentricity: EccentricityLaw,
}

impl HotVariant {
    /// The share of systems that take the variant.
    #[must_use]
    pub const fn probability(&self) -> f64 {
        self.probability
    }

    /// The variant's count.
    #[must_use]
    pub const fn count(&self) -> CountLaw {
        self.count
    }

    /// The variant's eccentricities.
    #[must_use]
    pub const fn eccentricity(&self) -> EccentricityLaw {
        self.eccentricity
    }
}

/// One group of bodies a class places.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanetGroup {
    role: GroupRole,
    presence: f64,
    count: CountLaw,
    masses: MassRange,
    location: Location,
    reach: Reach,
    spacing: SpacingFamily,
    eccentricity: EccentricityLaw,
    origin: Origin,
    hot_variant: Option<HotVariant>,
}

impl PlanetGroup {
    /// The group's role.
    #[must_use]
    pub const fn role(&self) -> GroupRole {
        self.role
    }

    /// The share of systems of the class that have the group at all.
    #[must_use]
    pub const fn presence(&self) -> f64 {
        self.presence
    }

    /// How many bodies it places when present.
    #[must_use]
    pub const fn count(&self) -> CountLaw {
        self.count
    }

    /// Its masses.
    #[must_use]
    pub const fn masses(&self) -> MassRange {
        self.masses
    }

    /// Where its first body goes.
    #[must_use]
    pub const fn location(&self) -> Location {
        self.location
    }

    /// How far out it may extend.
    #[must_use]
    pub const fn reach(&self) -> Reach {
        self.reach
    }

    /// Its spacing family.
    #[must_use]
    pub const fn spacing(&self) -> SpacingFamily {
        self.spacing
    }

    /// Its eccentricities.
    #[must_use]
    pub const fn eccentricity(&self) -> EccentricityLaw {
        self.eccentricity
    }

    /// Where its bodies formed.
    #[must_use]
    pub const fn origin(&self) -> Origin {
        self.origin
    }

    /// Its dynamically hot variant, if it has one.
    #[must_use]
    pub const fn hot_variant(&self) -> Option<HotVariant> {
        self.hot_variant
    }

    /// Whether the group can place a giant: a body of at least the spacing floor's giant mass, 0.1
    /// M♃ ([`SPACING_GIANT_MASS`](crate::planetary::params::SPACING_GIANT_MASS)).
    #[must_use]
    pub fn places_giants(&self) -> bool {
        let giant =
            crate::planetary::params::SPACING_GIANT_MASS.value() * EARTH_MASSES_PER_JUPITER_MASS;
        self.masses.max.value() >= giant
    }
}

/// Whether a class's belt is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BeltRule {
    /// Always, where the giants and the disc leave room (P14.T21).
    Required,
    /// As P14.T21 decides.
    Allowed,
}

/// What one architecture class places (P14.T5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassTemplate {
    class: ArchitectureClass,
    groups: &'static [PlanetGroup],
    quiet_inside: Option<Days>,
    asteroid_belt: BeltRule,
    outer_belt: BeltRule,
}

impl ClassTemplate {
    /// The class.
    #[must_use]
    pub const fn class(&self) -> ArchitectureClass {
        self.class
    }

    /// The groups, in the order they are placed, inside out.
    #[must_use]
    pub const fn groups(&self) -> &'static [PlanetGroup] {
        self.groups
    }

    /// The period inside which the class's first group has no companion, if any (the hot
    /// Jupiter's 100 days).
    #[must_use]
    pub const fn quiet_inside(&self) -> Option<Days> {
        self.quiet_inside
    }

    /// Whether an asteroid belt, inside the innermost giant, is placed.
    #[must_use]
    pub const fn asteroid_belt(&self) -> BeltRule {
        self.asteroid_belt
    }

    /// Whether a Kuiper-like belt, beyond the outermost planet, is placed.
    #[must_use]
    pub const fn outer_belt(&self) -> BeltRule {
        self.outer_belt
    }
}

/// M⊕ as an [`EarthMasses`].
#[must_use]
const fn earth(m: f64) -> EarthMasses {
    EarthMasses::new(m)
}

/// M♃ as an [`EarthMasses`].
#[must_use]
const fn jupiter(m: f64) -> EarthMasses {
    EarthMasses::new(m * EARTH_MASSES_PER_JUPITER_MASS)
}

/// dN ÷ d ln M ∝ M^−0.31 for giants (Cumming et al. 2008).
const GIANT_MASSES: MassRange = MassRange {
    min: jupiter(0.3),
    max: jupiter(10.0),
    law: MassLaw::PowerLaw { index: -0.31 },
};

/// Kipping's (2013) Beta for all radial-velocity planets.
const KIPPING_ALL: EccentricityLaw = EccentricityLaw::Beta { a: 0.867, b: 3.03 };

/// Kipping's (2013, Table 2) two populations, split at the sample's median period of 382.3 days:
/// Beta(0.697, 3.27) inside it and Beta(1.12, 3.09) beyond.
const KIPPING_BY_PERIOD: EccentricityLaw = EccentricityLaw::BetaByPeriod {
    split: Days::new(382.3),
    short_a: 0.697,
    short_b: 3.27,
    long_a: 1.12,
    long_b: 3.09,
};

/// P14.T8.d's cold chains.
const COLD_CHAIN: EccentricityLaw = EccentricityLaw::Rayleigh { sigma: 0.04 };

/// P14.T8.d's Solar-like and terrestrial groups.
const LOW_ECCENTRICITY: EccentricityLaw = EccentricityLaw::Rayleigh { sigma: 0.05 };

/// P14.T8.d's hot variant.
const HOT: EccentricityLaw = EccentricityLaw::HalfNormal { sigma: 0.3 };

/// Mulders et al.'s (2018) innermost planet, truncated to plan 14's 1–50 days.
const FIRST_PERIOD: PeriodLaw = PeriodLaw::BrokenPowerLaw {
    break_period: Days::new(12.0),
    rising: 1.6,
    falling: -0.9,
    min: Days::new(1.0),
    max: Days::new(50.0),
};

/// A chain's count: mean 3.5 at 1 M☉ and 6.1 at and below 0.48 M☉, at most 10.
pub const CHAIN_COUNT: CountLaw = CountLaw::ZeroTruncatedPoisson {
    rate: 3.38,
    mass_exponent: -0.82,
    mass_floor: SolarMasses::new(0.48),
    max: 10,
};

/// The dynamically hot variant: 40% of systems, 1–2 planets, half-normal eccentricities.
const HOT_VARIANT: HotVariant = HotVariant {
    probability: 0.4,
    count: CountLaw::Uniform { min: 1, max: 2 },
    eccentricity: HOT,
};

/// The most rocky planets one group places: 10, as many as a chain's cap (Mulders et al. 2018).
pub const ROCKY_MAX_COUNT: u8 = 10;

/// The least mass of a rocky planet: 0.05 M⊕ (plan 14), which a drift-fed group's floor, scaled
/// with its host's mass, never goes under (ruling 66).
pub const ROCKY_MASS_FLOOR: EarthMasses = earth(0.05);

/// The rocky planets of `TerrestrialOnly` and `SolarLike`: as many as P14.T6.b's terrestrial
/// spacing fits from their first body to the snow line, at most [`ROCKY_MAX_COUNT`].
const ROCKY: PlanetGroup = PlanetGroup {
    role: GroupRole::Rocky,
    presence: 1.0,
    count: CountLaw::Fill {
        max: ROCKY_MAX_COUNT,
    },
    masses: MassRange {
        min: ROCKY_MASS_FLOOR,
        max: earth(2.0),
        law: MassLaw::Correlated,
    },
    location: Location::ScaledAu {
        inner: 0.2,
        outer: 0.5,
    },
    reach: Reach::InsideSnowLine,
    spacing: SpacingFamily::Terrestrial,
    eccentricity: LOW_ECCENTRICITY,
    origin: Origin::InSitu,
    hot_variant: None,
};

/// A compact chain of stellar hosts.
const CHAIN: PlanetGroup = PlanetGroup {
    role: GroupRole::Chain,
    presence: 1.0,
    count: CHAIN_COUNT,
    masses: MassRange {
        min: earth(1.0),
        max: earth(20.0),
        law: MassLaw::Correlated,
    },
    location: Location::Period(FIRST_PERIOD),
    reach: Reach::Open,
    spacing: SpacingFamily::SmallPlanets,
    eccentricity: COLD_CHAIN,
    origin: Origin::Migrated,
    hot_variant: Some(HOT_VARIANT),
};

const TERRESTRIAL_ONLY: &[PlanetGroup] = &[
    ROCKY,
    PlanetGroup {
        role: GroupRole::IceRich,
        presence: 1.0,
        count: CountLaw::Uniform { min: 0, max: 3 },
        masses: MassRange {
            min: earth(0.02),
            max: earth(5.0),
            law: MassLaw::LogUniform,
        },
        location: Location::SnowLines {
            inner: 1.0,
            outer: 2.0,
        },
        reach: Reach::Open,
        spacing: SpacingFamily::Terrestrial,
        eccentricity: LOW_ECCENTRICITY,
        origin: Origin::InSitu,
        hot_variant: None,
    },
];

const COMPACT_MULTI: &[PlanetGroup] = &[CHAIN];

const COMPACT_WITH_COLD_GIANT: &[PlanetGroup] = &[
    CHAIN,
    PlanetGroup {
        role: GroupRole::Giant,
        presence: 1.0,
        count: CountLaw::Uniform { min: 1, max: 2 },
        masses: GIANT_MASSES,
        location: Location::SnowLines {
            inner: 1.0,
            outer: 3.0,
        },
        reach: Reach::Open,
        spacing: SpacingFamily::Giants,
        eccentricity: KIPPING_BY_PERIOD,
        origin: Origin::InSitu,
        hot_variant: None,
    },
];

const SOLAR_LIKE: &[PlanetGroup] = &[
    ROCKY,
    PlanetGroup {
        role: GroupRole::Giant,
        presence: 1.0,
        count: CountLaw::Uniform { min: 1, max: 3 },
        masses: GIANT_MASSES,
        location: Location::SnowLines {
            inner: 1.0,
            outer: 2.0,
        },
        reach: Reach::Open,
        spacing: SpacingFamily::Giants,
        eccentricity: LOW_ECCENTRICITY,
        origin: Origin::InSitu,
        hot_variant: None,
    },
    PlanetGroup {
        role: GroupRole::IceGiant,
        presence: 1.0,
        count: CountLaw::Uniform { min: 0, max: 2 },
        masses: MassRange {
            min: earth(10.0),
            max: earth(30.0),
            law: MassLaw::LogUniform,
        },
        location: Location::Outward,
        reach: Reach::Open,
        spacing: SpacingFamily::Giants,
        eccentricity: LOW_ECCENTRICITY,
        origin: Origin::InSitu,
        hot_variant: None,
    },
];

const ECCENTRIC_GIANT: &[PlanetGroup] = &[
    PlanetGroup {
        role: GroupRole::Survivor,
        presence: 1.0,
        count: CountLaw::Uniform { min: 0, max: 1 },
        masses: MassRange {
            min: earth(0.05),
            max: earth(10.0),
            law: MassLaw::LogUniform,
        },
        location: Location::ScaledAu {
            inner: 0.2,
            outer: 0.5,
        },
        reach: Reach::InsideSnowLine,
        spacing: SpacingFamily::Terrestrial,
        eccentricity: HOT,
        origin: Origin::InSitu,
        hot_variant: None,
    },
    PlanetGroup {
        role: GroupRole::Giant,
        presence: 1.0,
        count: CountLaw::Uniform { min: 1, max: 2 },
        masses: GIANT_MASSES,
        location: Location::ScaledAu {
            inner: 0.5,
            outer: 5.0,
        },
        reach: Reach::Open,
        spacing: SpacingFamily::Giants,
        eccentricity: KIPPING_ALL,
        origin: Origin::BeyondSnowLine,
        hot_variant: None,
    },
];

const WARM_GIANT: &[PlanetGroup] = &[
    PlanetGroup {
        role: GroupRole::Giant,
        presence: 1.0,
        count: CountLaw::Uniform { min: 1, max: 1 },
        masses: GIANT_MASSES,
        location: Location::Period(PeriodLaw::LogUniform {
            min: Days::new(10.0),
            max: Days::new(200.0),
        }),
        reach: Reach::Open,
        spacing: SpacingFamily::Giants,
        eccentricity: KIPPING_BY_PERIOD,
        origin: Origin::Migrated,
        hot_variant: None,
    },
    PlanetGroup {
        role: GroupRole::Companions,
        presence: 0.5,
        count: CountLaw::Uniform { min: 1, max: 2 },
        masses: MassRange {
            min: earth(1.0),
            max: earth(20.0),
            law: MassLaw::Correlated,
        },
        location: Location::Flanking,
        reach: Reach::Open,
        spacing: SpacingFamily::SmallPlanets,
        eccentricity: COLD_CHAIN,
        origin: Origin::Migrated,
        hot_variant: None,
    },
];

const HOT_JUPITER: &[PlanetGroup] = &[
    PlanetGroup {
        role: GroupRole::Giant,
        presence: 1.0,
        count: CountLaw::Uniform { min: 1, max: 1 },
        masses: GIANT_MASSES,
        location: Location::Period(PeriodLaw::LogNormal {
            median: Days::new(3.5),
            sigma_dex: 0.15,
            min: Days::new(1.0),
            max: Days::new(10.0),
        }),
        reach: Reach::Open,
        spacing: SpacingFamily::Giants,
        eccentricity: KIPPING_BY_PERIOD,
        origin: Origin::BeyondSnowLine,
        hot_variant: None,
    },
    PlanetGroup {
        role: GroupRole::Giant,
        presence: 0.6,
        count: CountLaw::Uniform { min: 1, max: 1 },
        masses: MassRange {
            min: jupiter(1.0),
            ..GIANT_MASSES
        },
        location: Location::SnowLines {
            inner: 2.0,
            outer: 8.0,
        },
        reach: Reach::Open,
        spacing: SpacingFamily::Giants,
        eccentricity: KIPPING_BY_PERIOD,
        origin: Origin::InSitu,
        hot_variant: None,
    },
];

const SUBSTELLAR_COMPACT: &[PlanetGroup] = &[PlanetGroup {
    masses: MassRange {
        min: earth(0.01),
        max: earth(2.0),
        law: MassLaw::Correlated,
    },
    ..CHAIN
}];

/// Every class's template, in [`ArchitectureClass::ALL`]'s order.
pub const TEMPLATES: [ClassTemplate; super::CLASS_COUNT] = [
    ClassTemplate {
        class: ArchitectureClass::Barren,
        groups: &[],
        quiet_inside: None,
        asteroid_belt: BeltRule::Allowed,
        outer_belt: BeltRule::Allowed,
    },
    ClassTemplate {
        class: ArchitectureClass::TerrestrialOnly,
        groups: TERRESTRIAL_ONLY,
        quiet_inside: None,
        asteroid_belt: BeltRule::Allowed,
        outer_belt: BeltRule::Allowed,
    },
    ClassTemplate {
        class: ArchitectureClass::CompactMulti,
        groups: COMPACT_MULTI,
        quiet_inside: None,
        asteroid_belt: BeltRule::Allowed,
        outer_belt: BeltRule::Allowed,
    },
    ClassTemplate {
        class: ArchitectureClass::CompactWithColdGiant,
        groups: COMPACT_WITH_COLD_GIANT,
        quiet_inside: None,
        asteroid_belt: BeltRule::Allowed,
        outer_belt: BeltRule::Allowed,
    },
    ClassTemplate {
        class: ArchitectureClass::SolarLike,
        groups: SOLAR_LIKE,
        quiet_inside: None,
        asteroid_belt: BeltRule::Required,
        outer_belt: BeltRule::Required,
    },
    ClassTemplate {
        class: ArchitectureClass::EccentricGiant,
        groups: ECCENTRIC_GIANT,
        quiet_inside: None,
        asteroid_belt: BeltRule::Allowed,
        outer_belt: BeltRule::Allowed,
    },
    ClassTemplate {
        class: ArchitectureClass::WarmGiant,
        groups: WARM_GIANT,
        quiet_inside: None,
        asteroid_belt: BeltRule::Allowed,
        outer_belt: BeltRule::Allowed,
    },
    ClassTemplate {
        class: ArchitectureClass::HotJupiter,
        groups: HOT_JUPITER,
        quiet_inside: Some(Days::new(100.0)),
        asteroid_belt: BeltRule::Allowed,
        outer_belt: BeltRule::Allowed,
    },
    ClassTemplate {
        class: ArchitectureClass::SubstellarCompact,
        groups: SUBSTELLAR_COMPACT,
        quiet_inside: None,
        asteroid_belt: BeltRule::Allowed,
        outer_belt: BeltRule::Allowed,
    },
];

/// The template of `class`.
#[must_use]
pub const fn template(class: ArchitectureClass) -> &'static ClassTemplate {
    &TEMPLATES[class.index()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::disc::SNOW_LINE_AT_SOLAR_LUMINOSITY_AU;

    fn all_groups() -> impl Iterator<Item = (ArchitectureClass, &'static PlanetGroup)> {
        TEMPLATES
            .iter()
            .flat_map(|t| t.groups().iter().map(move |g| (t.class(), g)))
    }

    #[test]
    fn templates_are_in_class_order() {
        for (template, class) in TEMPLATES.iter().zip(ArchitectureClass::ALL) {
            assert_eq!(template.class(), class);
            assert_eq!(super::template(class).class(), class);
        }
    }

    /// P14.T5: every template's ranges are ordered and non-empty.
    #[test]
    fn every_template_s_ranges_are_ordered_and_non_empty() {
        for (class, group) in all_groups() {
            let at = format!("{class:?} {:?}", group.role());
            assert!(
                group.presence() > 0.0 && group.presence() <= 1.0,
                "{at}: presence"
            );
            let (least, most) = group.count().range();
            assert!(least <= most && most >= 1, "{at}: count {least}–{most}");
            let masses = group.masses();
            assert!(
                masses.min().value() > 0.0 && masses.min() < masses.max(),
                "{at}: masses"
            );
            match group.location() {
                Location::Period(law) => {
                    let (min, max) = law.range();
                    assert!(min.value() > 0.0 && min < max, "{at}: periods");
                    let centre = match law {
                        PeriodLaw::BrokenPowerLaw { break_period, .. } => Some(break_period),
                        PeriodLaw::LogNormal { median, .. } => Some(median),
                        PeriodLaw::LogUniform { .. } => None,
                    };
                    if let Some(centre) = centre {
                        assert!(min < centre && centre < max, "{at}: centre");
                    }
                }
                Location::ScaledAu { inner, outer } | Location::SnowLines { inner, outer } => {
                    assert!(inner > 0.0 && inner < outer, "{at}: location");
                }
                Location::Outward | Location::Flanking => {}
            }
            if let Some(hot) = group.hot_variant() {
                assert!(hot.probability() > 0.0 && hot.probability() < 1.0, "{at}");
                let (least, most) = hot.count().range();
                assert!(least >= 1 && least <= most, "{at}: hot count");
            }
        }
        // A group that follows another has one to follow.
        for template in &TEMPLATES {
            if let Some(first) = template.groups().first() {
                assert!(
                    !matches!(first.location(), Location::Outward | Location::Flanking),
                    "{:?}",
                    template.class()
                );
            }
        }
    }

    /// Where a group starts, in snow-line radii, if the template fixes it: a period or a
    /// following group counts as inside the snow line, the conservative reading.
    fn inner_edge_in_snow_lines(group: &PlanetGroup) -> f64 {
        match group.location() {
            Location::ScaledAu { inner, .. } => inner / SNOW_LINE_AT_SOLAR_LUMINOSITY_AU,
            Location::SnowLines { inner, .. } => inner,
            Location::Period(_) | Location::Outward | Location::Flanking => 0.0,
        }
    }

    /// P14.T5: a template never asks for a giant inside the snow line unless its group is marked
    /// migrated.
    #[test]
    fn no_template_asks_for_a_giant_inside_the_snow_line_unless_it_migrated() {
        let mut giants = 0;
        for (class, group) in all_groups() {
            if group.places_giants() {
                giants += 1;
                assert!(
                    inner_edge_in_snow_lines(group) >= 1.0 || group.origin().is_migrated(),
                    "{class:?} asks for a giant inside the snow line"
                );
                // Where a giant is placed inside the snow line, its origin is never in situ.
                if inner_edge_in_snow_lines(group) < 1.0 {
                    assert_ne!(group.origin(), Origin::InSitu, "{class:?}");
                }
            }
            // A group that stays inside the snow line never holds giants.
            if group.reach() == Reach::InsideSnowLine {
                assert!(!group.places_giants(), "{class:?}");
            }
        }
        assert_eq!(giants, 6, "the giant groups of five classes, one with two");
    }

    #[test]
    fn a_class_has_giants_exactly_when_its_template_places_them() {
        for template in &TEMPLATES {
            let places = template.groups().iter().any(PlanetGroup::places_giants);
            assert_eq!(
                places,
                template.class().has_giants(),
                "{:?}",
                template.class()
            );
        }
        assert!(
            super::template(ArchitectureClass::Barren)
                .groups()
                .is_empty()
        );
    }

    /// The chain's count law has the means of its sources: 3.5 at 1 M☉ (plan 14), 6.1 at and below
    /// 0.48 M☉ (Ballard and Johnson 2016), and fewer towards F stars (Yang et al. 2020).
    #[test]
    fn chain_counts_have_the_means_of_their_sources() {
        let mean = |m: f64| CHAIN_COUNT.mean(SolarMasses::new(m));
        assert!((mean(1.0) - 3.5).abs() < 0.01, "{}", mean(1.0));
        assert!((mean(0.48) - 6.1).abs() < 0.02, "{}", mean(0.48));
        assert!((mean(0.1) - mean(0.48)).abs() < 1e-12, "held below 0.48 M☉");
        assert!((2.8..3.0).contains(&mean(1.3)), "{}", mean(1.3));
        // The mean of a uniform law and of a zero-truncated Poisson law far from its cap.
        assert!(
            (CountLaw::Uniform { min: 2, max: 6 }.mean(SolarMasses::new(1.0)) - 4.0).abs() < 1e-15
        );
        let far = CountLaw::ZeroTruncatedPoisson {
            rate: 2.0,
            mass_exponent: 0.0,
            mass_floor: SolarMasses::new(0.1),
            max: 60,
        };
        let expected = 2.0 / (1.0 - crate::math::exp(-2.0));
        assert!((far.mean(SolarMasses::new(1.0)) - expected).abs() < 1e-12);
    }

    #[test]
    fn the_giant_mass_range_is_cumming_s() {
        let giants = GIANT_MASSES;
        assert!((giants.min().value() - 95.35).abs() < 0.05);
        assert!((giants.max().value() - 3178.3).abs() < 0.5);
        assert_eq!(giants.law(), MassLaw::PowerLaw { index: -0.31 });
        assert!((EARTH_MASSES_PER_JUPITER_MASS - 317.83).abs() < 0.01);
    }
}
