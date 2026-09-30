//! The feature catalogue's rates and the share φ of each population's systems that sit in features
//! (plan 09, P09.T2.a and P09.T2.b; Design notes 1–4).
//!
//! A population's budget covers everything born in it; its field density is the budget times
//! `1 − φ`, with φ computed once per galaxy from the catalogue's rates and never from individual
//! features (brainstorm, "Large features"). [`FeatureShares`] holds φ and what the catalogue's
//! processes need to place features: how many features a system of each component stands for.
//! [`Galaxy`](crate::galaxy::Galaxy) builds it with itself (Design note 15). Nothing reads φ into
//! the fields yet: P09.T2.c applies it, with a version bump, once plan 08's `stay_share` exists.
//!
//! # The young disc (Design notes 1 and 2)
//!
//! Nurseries are born at the young disc's formation rate: `f_n` of the mass formed, on the
//! [`NurseryMassFunction`], so that the nursery birth rate at age `a` is `f_n N_y p_y(a) m̄_f ÷ m̄_n`
//! per year, with `N_y p_y(a)` the young systems born `a` ago, `m̄_f` the mass formed per system
//! ([`Galaxy::mean_formed_mass`](crate::galaxy::Galaxy::mean_formed_mass)) and `m̄_n` the mean
//! nursery mass. The share of the systems born `a` ago that sit in features is then
//!
//! `φ(a) = f_n [Γ_b m_b(a) + (1 − Γ_b)(e(a) + (1 − e(a)) w (1 − G(a)))]`,
//!
//! with `Γ_b` the bound fraction, `m_b` the bound clusters' surviving mass fraction
//! ([`surviving_mass_fraction`]), `e(a)` the share still embedded, `G(a)` the share of
//! associations dissolved, and `w` the share of the nurseries' mass above the association floor.
//! A bound cluster is disrupted from birth on, an unbound nursery keeps all its stars while it is
//! embedded and, if above the floor, while it is an association. So φ is continuous in age.
//!
//! # The old thin disc (Design note 3)
//!
//! Bound clusters older than 100 Myr are a process per sub-disc, with φ constant in each:
//! `φ_k = f_n Γ_b ∫ p_k(a) m_b(a) da`, the sub-disc's age distribution averaging the surviving
//! fraction. Its clusters number `f_n Γ_b (m̄_f ÷ m̄_n) N_k ∫ p_k(a) S(a) da`, with `S` the share of
//! clusters born `a` ago still alive ([`surviving_number_fraction`]).
//!
//! # Globulars and the halo's discrete share (Design note 4)
//!
//! Globulars count against the bulge, the thick disc and the halo ([`GlobularSystem`], P09.T12):
//! a population's φ is its globulars' expected mass, the untruncated law's number times the mass
//! function's mean, over its systems times their mean mass. The halo's discrete share of streams
//! and dwarf cores is plan 10's, through [`set_halo_discrete`](FeatureShares::set_halo_discrete),
//! and adds to its globulars'.
//!
//! # Bands
//!
//! The clusters' class tables deplete the lowest bands (P09.T9.a), so the globulars' φ is not the
//! same in every band: band b takes `φ × s_b ÷ S_b`, `S_b` the field's band share and `s_b` the
//! members', the field's depleted at the slope of a representative globular (the mass function's
//! mean mass at the half-mass radius of 5 kpc, 12 Gyr old; ours) and renormalised. The open
//! clusters and nurseries are young enough that their tables deplete nothing, and keep one φ in
//! every band. A band below the five stellar ones, which plan 13 adds, takes band A's value
//! through one private mapping, so plan 13 changes nothing here.
//!
//! # The carved binary classes (plan 11, P11.T7)
//!
//! The grid redraws every binary that falls into one of plan 11's carved classes (accreting white
//! dwarfs, X-ray binaries, stellar and neutron-star mergers), so no host is in the field, and the
//! catalogue holds them (P11.T8). Each class's share of a population's band leaves its field
//! through [`set_class_shares`](FeatureShares::set_class_shares): the class's hosts per solar
//! mass formed times the mass formed per system, times the share of its hosts whose primary lies
//! in the band, over the band's share of the systems. [`field_factor`](FeatureShares::field_factor)
//! is `1 − φ` less those shares, so that field, features and catalogue add up to each population's
//! budget.

use crate::galaxy::POPULATIONS;
use crate::galaxy::Population;
use crate::galaxy::ages::{SubDisc, YOUNG_AGE_LIMIT};
use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use crate::galaxy::consts::YEARS_PER_MEGAYEAR;
use crate::galaxy::fields::Fields;
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::quad::gl_panels;
use crate::math;
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::lifetime;
use crate::tables::binary::ClassShareTable;
use crate::units::{LightYears, SolarMasses, Years};

use super::cluster::relaxation_time;
use super::interior::counts::{CANONICAL_SLOPE, band_depletion};
use super::kinds::globular::{GlobularSystem, HALF_MASS_RADIUS_LAW};

use super::kinds::cloud::{CLOUD_NEUTRAL_SHARE, MOLECULAR_CLOUD_MULTIPLE, mean_cloud_mass};
use super::kinds::nursery::{
    ASSOCIATION_MASS_FLOOR, BOUND_FRACTION, DISSOLUTION_AGE_MYR, EMBEDDED_DURATION_MYR,
    NURSERY_SHARE, SupernovaClock, dissolved_share, embedded_share,
};
use super::kinds::open_cluster::{
    NurseryMassFunction, dissolution_time, surviving_mass_fraction, surviving_number_fraction,
};

/// The least initial mass of a core-collapse progenitor, M☉: band E's lower edge.
pub const CORE_COLLAPSE_MIN_MASS: SolarMasses = SolarMasses::new(8.0);

/// The heaviest star plan 06's tracks reach, M☉, whose lifetime starts a nursery's supernovae.
pub const CORE_COLLAPSE_MAX_TRACK_MASS: SolarMasses = SolarMasses::new(100.0);

/// The young disc's nursery rates (P09.T2.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NurseryRates {
    /// The young systems per unit of their age distribution's density: `N_y` of the module docs.
    young_systems: f64,
    /// Nurseries born per young system per unit of the age density: `f_n m̄_f ÷ m̄_n`.
    per_system: f64,
    /// The share of young systems born at or after the epoch's age 0 and before the young disc's
    /// age limit.
    born_share: f64,
    /// The share of the nurseries born over those ages that are alive at the epoch.
    alive_share: f64,
}

impl NurseryRates {
    /// The rates of the young disc in `fields`, with `mean_formed_mass` formed per system.
    #[must_use]
    pub fn from_fields(fields: &Fields, mean_formed_mass: SolarMasses) -> Self {
        let (mut young_systems, mut born_share, mut alive_share) = (0.0, 1.0, 1.0);
        for component in fields.components() {
            if component.population() == Population::YoungThinDisc {
                let ages = component.ages();
                young_systems += component.count_with_unborn();
                born_share = ages.born_fraction();
                alive_share = gl_panels(
                    |a| ages.pdf(Years::new(a)) * nursery_alive_probability(Years::new(a)),
                    &young_age_edges(),
                ) / born_share;
            }
        }
        Self {
            young_systems,
            per_system: NURSERY_SHARE * mean_formed_mass.value()
                / NurseryMassFunction::STANDARD.mean().value(),
            born_share,
            alive_share,
        }
    }

    /// The share of the nurseries born over the young disc's ages that are alive at the epoch:
    /// the mean of [`nursery_alive_probability`] over the born ages.
    #[must_use]
    pub const fn alive_share(&self) -> f64 {
        self.alive_share
    }

    /// The nurseries alive at the epoch.
    #[must_use]
    pub fn alive(&self) -> f64 {
        self.births() * self.alive_share
    }

    /// Living nurseries per unit of the young disc's density: the nursery process's density over
    /// the young disc's, `f_n (m̄_f ÷ m̄_n)` times the born and alive shares.
    #[must_use]
    pub fn alive_per_system(&self) -> f64 {
        self.per_system * self.born_share * self.alive_share
    }

    /// The nurseries born per year `age` ago, given the young disc's age density `density` there
    /// (per year): `f_n N_y p_y(a) m̄_f ÷ m̄_n`.
    #[must_use]
    pub fn birth_rate(&self, density: f64) -> f64 {
        self.young_systems * density * self.per_system
    }

    /// Nurseries born per young system born, `f_n m̄_f ÷ m̄_n`: the nursery process's density over
    /// the young disc's.
    #[must_use]
    pub const fn per_system(&self) -> f64 {
        self.per_system
    }

    /// The young systems born since the young disc's age limit, `N_y` times its born share.
    #[must_use]
    pub fn born_systems(&self) -> f64 {
        self.young_systems * self.born_share
    }

    /// The nurseries born over the young disc's ages, 0–100 Myr.
    #[must_use]
    pub fn births(&self) -> f64 {
        self.born_systems() * self.per_system
    }

    /// The bound clusters born over the same ages.
    #[must_use]
    pub fn bound_births(&self) -> f64 {
        self.births() * BOUND_FRACTION
    }
}

/// What the catalogue's old-open-cluster process of one sub-disc needs (Design note 3).
#[derive(Debug, Clone, Copy, PartialEq)]
struct SubDiscClusters {
    /// φ of the sub-disc.
    phi: f64,
    /// Living clusters per system: `f_n Γ_b (m̄_f ÷ m̄_n) ∫ p_k S da`, the process's candidates.
    alive_per_system: f64,
    /// `S(a_lo)`: the share of clusters born at the sub-disc's youngest age still alive, which
    /// the age acceptance divides by.
    youngest_survival: f64,
}

/// φ for every population and what the catalogue's processes need (P09.T2.b).
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::{Galaxy, Population};
/// use hyperion_sim::galaxy::imf::MassBand;
/// use hyperion_sim::units::Years;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let shares = galaxy.feature_shares();
/// // A few million years after birth nine in ten young stars are still in their nursery; by
/// // 100 Myr only the bound clusters' survivors are.
/// assert!((0.85..0.95).contains(&shares.phi_young(Years::new(3e6))));
/// assert!((0.05..0.15).contains(&shares.phi_young(Years::new(1e8))));
/// let factor = shares.field_factor(Population::YoungThinDisc, MassBand::C);
/// assert!(factor > 0.0 && factor < 1.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureShares {
    nurseries: NurseryRates,
    young_mean_phi: f64,
    old_mean_phi: f64,
    sub_discs: [SubDiscClusters; 5],
    halo_discrete: f64,
    clock: SupernovaClock,
    cloud_per_mass: f64,
    globulars: GlobularSystem,
    /// The globulars' φ of the bulge, the thick disc and the halo.
    globular_phi: [f64; 3],
    /// The globulars' band weights `s_b ÷ S_b`.
    globular_band_weights: [f64; 5],
    /// The mass formed per system, M☉, which turns the class shares' rates per solar mass formed
    /// into shares of systems.
    formed_mass: f64,
    /// The field's share of the systems in each stellar band, A–E.
    band_shares: [f64; 5],
    /// The carved binary classes' share of each population's band, by population in
    /// [`POPULATIONS`] order (P11.T7): zero until [`FeatureShares::set_class_shares`].
    class_shares: [[f64; 5]; 7],
}

impl FeatureShares {
    /// The shares of the galaxy of `params` whose fields are `fields`, whose mass function is
    /// `mass_function` and which forms `mean_formed_mass` per system.
    #[must_use]
    pub fn new(
        params: &GalaxyParams,
        fields: &Fields,
        mass_function: &dyn MassFunction,
        mean_formed_mass: SolarMasses,
    ) -> Self {
        let nurseries = NurseryRates::from_fields(fields, mean_formed_mass);
        let mut young_mean_phi = 0.0;
        let (mut old_members, mut old_systems) = (0.0, 0.0);
        let mut sub_discs = [SubDiscClusters {
            phi: 0.0,
            alive_per_system: 0.0,
            youngest_survival: 0.0,
        }; 5];
        let per_system = NURSERY_SHARE * BOUND_FRACTION * mean_formed_mass.value()
            / NurseryMassFunction::STANDARD.mean().value();
        for component in fields.components() {
            let ages = component.ages();
            if component.population() == Population::YoungThinDisc {
                let edges = young_age_edges();
                let weighted = gl_panels(
                    |a| ages.pdf(Years::new(a)) * phi_young(Years::new(a)),
                    &edges,
                );
                young_mean_phi = weighted / ages.born_fraction();
            }
            if let Some(sub_disc) = component.sub_disc() {
                let [lo, hi] = [ages.min(), ages.max()];
                let edges = panel_edges(lo.value(), hi.value());
                let phi = gl_panels(
                    |a| ages.pdf(Years::new(a)) * surviving_mass_fraction(Years::new(a)),
                    &edges,
                );
                let alive = gl_panels(
                    |a| ages.pdf(Years::new(a)) * surviving_number_fraction(Years::new(a)),
                    &edges,
                );
                let youngest = surviving_number_fraction(lo);
                old_members += component.count() * NURSERY_SHARE * BOUND_FRACTION * phi;
                old_systems += component.count();
                sub_discs[sub_disc.index()] = SubDiscClusters {
                    phi: NURSERY_SHARE * BOUND_FRACTION * phi,
                    alive_per_system: per_system * alive,
                    youngest_survival: youngest,
                };
            }
        }
        let per_system_cc = mass_function.integral(CORE_COLLAPSE_MIN_MASS.value(), 150.0)
            / mass_function.integral(0.0, 150.0)
            / mean_formed_mass.value();
        let draws = StarDraws::median();
        let clock = SupernovaClock::new(
            lifetime(CORE_COLLAPSE_MAX_TRACK_MASS, &Composition::SOLAR, &draws),
            lifetime(CORE_COLLAPSE_MIN_MASS, &Composition::SOLAR, &draws),
            per_system_cc,
        );
        let globulars = GlobularSystem::new(params);
        let (bulge, thick, halo) = globulars.population_masses();
        let population_mass = |population: Population| {
            let count: f64 = fields
                .components()
                .iter()
                .filter(|c| c.population() == population)
                .map(crate::galaxy::fields::Component::count)
                .sum();
            count * params.mean_system_mass(population).value()
        };
        let share = |mass: f64, population: Population| {
            let budget = population_mass(population);
            if budget > 0.0 { mass / budget } else { 0.0 }
        };
        let globular_phi = [
            share(bulge, Population::Bulge),
            share(thick, Population::ThickDisc),
            share(halo, Population::Halo),
        ];
        let globular_band_weights = globular_band_weights(&globulars, mass_function);
        let band_total = mass_function.integral(MASS_BAND_EDGES[0], MASS_BAND_EDGES[5]);
        let band_shares: [f64; 5] = std::array::from_fn(|b| {
            mass_function.integral(MASS_BAND_EDGES[b], MASS_BAND_EDGES[b + 1]) / band_total
        });
        Self {
            nurseries,
            young_mean_phi,
            old_mean_phi: if old_systems > 0.0 {
                old_members / old_systems
            } else {
                0.0
            },
            sub_discs,
            halo_discrete: 0.0,
            clock,
            cloud_per_mass: 1.0 / mean_cloud_mass().value(),
            globulars,
            globular_phi,
            globular_band_weights,
            formed_mass: mean_formed_mass.value(),
            band_shares,
            class_shares: [[0.0; 5]; 7],
        }
    }

    /// Plan 11's setter for the carved binary classes' shares (P11.T7; module documentation,
    /// "The carved binary classes"): each population's band gives up `r m̄_f w_b ÷ S_b` of its
    /// systems for each class, `r` the class's hosts per solar mass formed in the population, `m̄_f`
    /// the mass formed per system, `w_b` the share of the hosts whose primary lies in band b and
    /// `S_b` the band's share of the systems. [`Galaxy`](crate::galaxy::Galaxy) sets
    /// [`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES) when it builds these shares.
    ///
    /// # Panics
    ///
    /// If a band's share comes out negative or not finite, or a population's shares of a band sum
    /// to 1 or more.
    pub fn set_class_shares(&mut self, table: &ClassShareTable) {
        for (column, &population) in self.class_shares.iter_mut().zip(POPULATIONS.iter()) {
            for (b, share) in column.iter_mut().enumerate() {
                let band = MassBand::ALL[b];
                let per_system = table.rows().iter().fold(0.0, |sum, row| {
                    sum + row.hosts_per_formed_mass(population) * row.layer_share(band)
                });
                let value = if self.band_shares[b] > 0.0 {
                    per_system * self.formed_mass / self.band_shares[b]
                } else {
                    0.0
                };
                assert!(
                    value.is_finite() && (0.0..1.0).contains(&value),
                    "the carved classes' share of {population:?} in {band:?} is {value}"
                );
                *share = value;
            }
        }
    }

    /// The carved binary classes' share of `population`'s systems of `band` (P11.T7): what the
    /// grid's redraw gives up to plan 11's catalogue classes. A substellar band gives up none.
    ///
    /// # Panics
    ///
    /// Never: every population is in [`POPULATIONS`].
    #[must_use]
    pub fn class_share(&self, population: Population, band: MassBand) -> f64 {
        if !band.is_stellar() {
            return 0.0;
        }
        let column = POPULATIONS
            .iter()
            .position(|&p| p == population)
            .expect("every population is in POPULATIONS");
        self.class_shares[column][band.index()]
    }

    /// The globulars' band weights `s_b ÷ S_b` (module documentation, "Bands").
    #[must_use]
    pub const fn globular_band_weights(&self) -> [f64; 5] {
        self.globular_band_weights
    }

    /// The globular system: its count, density and mass function (P09.T12).
    #[must_use]
    pub const fn globulars(&self) -> &GlobularSystem {
        &self.globulars
    }

    /// The globulars' φ of `population` in `band`: the bulge's, the thick disc's and the halo's,
    /// weighted by band (module documentation); 0 for the rest.
    #[must_use]
    pub fn phi_globular(&self, population: Population, band: MassBand) -> f64 {
        let weight = self.globular_band_weights[stellar_band_or_a(band).index()];
        let phi = match population {
            Population::Bulge => self.globular_phi[0],
            Population::ThickDisc => self.globular_phi[1],
            Population::Halo => self.globular_phi[2],
            Population::YoungThinDisc
            | Population::OldThinDisc
            | Population::LongBar
            | Population::NuclearDisc => 0.0,
        };
        phi * weight
    }

    /// The young disc's nursery rates.
    #[must_use]
    pub const fn nurseries(&self) -> &NurseryRates {
        &self.nurseries
    }

    /// φ of the young disc at `age` (module documentation), 0 before birth.
    #[must_use]
    pub fn phi_young(&self, age: Years) -> f64 {
        phi_young(age)
    }

    /// φ of a sub-disc of the old thin disc.
    #[must_use]
    pub fn phi_sub_disc(&self, sub_disc: SubDisc) -> f64 {
        self.sub_discs[sub_disc.index()].phi
    }

    /// The expected share of `population`'s systems of `band` inside features: the young disc's φ
    /// averaged over its born ages, the old thin disc's sub-discs' by their shares of it, the
    /// globulars' for the bulge, the thick disc and the halo (0 until P09.T12), and the halo's
    /// discrete share too.
    #[must_use]
    pub fn phi(&self, population: Population, band: MassBand) -> f64 {
        let own = match population {
            Population::YoungThinDisc => self.young_mean_phi,
            Population::OldThinDisc => self.old_mean_phi,
            Population::Halo => self.halo_discrete,
            Population::ThickDisc
            | Population::Bulge
            | Population::LongBar
            | Population::NuclearDisc => 0.0,
        };
        own + self.phi_globular(population, band)
    }

    /// `1 − φ` less the carved binary classes' share ([`class_share`](Self::class_share)): what
    /// `population`'s field density keeps of its budget in `band`.
    #[must_use]
    pub fn field_factor(&self, population: Population, band: MassBand) -> f64 {
        1.0 - self.phi(population, band) - self.class_share(population, band)
    }

    /// Plan 10's setter for the halo's discrete share of streams and dwarf cores.
    ///
    /// # Panics
    ///
    /// If `phi` is not in [0, 1).
    pub fn set_halo_discrete(&mut self, phi: f64) {
        assert!((0.0..1.0).contains(&phi), "a halo discrete share of {phi}");
        self.halo_discrete = phi;
    }

    /// The living old open clusters per system of `sub_disc`: the process's density over the
    /// sub-disc's.
    #[must_use]
    pub fn clusters_alive_per_system(&self, sub_disc: SubDisc) -> f64 {
        self.sub_discs[sub_disc.index()].alive_per_system
    }

    /// The share of clusters born at `sub_disc`'s youngest age still alive: the ceiling of the
    /// age draw's rejection, which keeps an age `a` with odds `S(a) ÷ S(a_lo)`.
    #[must_use]
    pub fn youngest_survival(&self, sub_disc: SubDisc) -> f64 {
        self.sub_discs[sub_disc.index()].youngest_survival
    }

    /// When nurseries' core collapses happen and how many they expect.
    #[must_use]
    pub const fn supernova_clock(&self) -> &SupernovaClock {
        &self.clock
    }

    /// Clouds per solar mass of cloud gas: `1 ÷` the mean cloud mass.
    #[must_use]
    pub const fn clouds_per_solar_mass(&self) -> f64 {
        self.cloud_per_mass
    }

    /// The clouds' share ε of the smooth neutral gas, and the multiple of the molecular disc they
    /// hold (Design note 19): the weight `w = 9 ÷ ε` of the design note.
    #[must_use]
    pub const fn cloud_weights(&self) -> (f64, f64) {
        (CLOUD_NEUTRAL_SHARE, MOLECULAR_CLOUD_MULTIPLE)
    }
}

/// φ of the young disc at `age` (module documentation), 0 before birth.
#[must_use]
pub fn phi_young(age: Years) -> f64 {
    if age.value() < 0.0 {
        return 0.0;
    }
    let e = embedded_share(age);
    let floor = NurseryMassFunction::STANDARD.mass_above(ASSOCIATION_MASS_FLOOR);
    let unbound = e + (1.0 - e) * floor * (1.0 - dissolved_share(age));
    NURSERY_SHARE
        * (BOUND_FRACTION * surviving_mass_fraction(age) + (1.0 - BOUND_FRACTION) * unbound)
}

/// The probability that a nursery born `age` ago is alive at the epoch,
/// `Γ_b S(a) + (1 − Γ_b)(e(a) + (1 − e(a)) w_N (1 − G(a)))`, with `w_N` the number share above
/// the association floor. A bound cluster outlives its embedded stage, since the lightest lasts
/// 75 Myr.
#[must_use]
pub fn nursery_alive_probability(age: Years) -> f64 {
    if age.value() < 0.0 {
        return 0.0;
    }
    let e = embedded_share(age);
    let floor = NurseryMassFunction::STANDARD.number_above(ASSOCIATION_MASS_FLOOR);
    let unbound = e + (1.0 - e) * floor * (1.0 - dissolved_share(age));
    BOUND_FRACTION * surviving_number_fraction(age) + (1.0 - BOUND_FRACTION) * unbound
}

/// The globulars' band weights `s_b ÷ S_b` (module documentation).
fn globular_band_weights(globulars: &GlobularSystem, mass_function: &dyn MassFunction) -> [f64; 5] {
    let mass = SolarMasses::new(globulars.mean_mass());
    let r_h = LightYears::new(
        HALF_MASS_RADIUS_LAW.0 * math::powf(5.0, HALF_MASS_RADIUS_LAW.1) * LIGHT_YEARS_PER_PARSEC,
    );
    let t = relaxation_time(mass, r_h).value();
    let alpha = (-0.46 - 0.79 * (math::log10(t) - 9.0)).max(CANONICAL_SLOPE);
    let ratios = band_depletion(mass_function, alpha);
    let field: [f64; 5] =
        std::array::from_fn(|b| mass_function.integral(MASS_BAND_EDGES[b], MASS_BAND_EDGES[b + 1]));
    let total: f64 = field.iter().sum();
    let members: f64 = field.iter().zip(&ratios).map(|(f, r)| f * r).sum();
    std::array::from_fn(|b| {
        if members > 0.0 {
            ratios[b] * total / members
        } else {
            1.0
        }
    })
}

/// Every stellar band maps to itself, and plan 13's substellar bands to band A, the stars these
/// objects most resemble in number and origin (plan 13, Design note 4).
const fn stellar_band_or_a(band: MassBand) -> MassBand {
    match band {
        MassBand::A | MassBand::B | MassBand::C | MassBand::D | MassBand::E => band,
        MassBand::BrownDwarf | MassBand::RoguePlanet => MassBand::A,
    }
}

/// Panel edges over the young disc's born ages, at every kink of φ: the embedded range, the
/// dissolution range and the age at which the lightest bound cluster dissolves.
fn young_age_edges() -> Vec<f64> {
    let myr = YEARS_PER_MEGAYEAR;
    let mut edges = vec![
        0.0,
        EMBEDDED_DURATION_MYR.0 * myr,
        EMBEDDED_DURATION_MYR.1 * myr,
        10.0 * myr,
        DISSOLUTION_AGE_MYR.0 * myr,
        dissolution_time(NurseryMassFunction::STANDARD.lo()).value(),
        DISSOLUTION_AGE_MYR.1 * myr,
    ];
    edges.sort_by(f64::total_cmp);
    edges.retain(|&e| e <= YOUNG_AGE_LIMIT.value());
    if edges.last().is_some_and(|&e| e < YOUNG_AGE_LIMIT.value()) {
        edges.push(YOUNG_AGE_LIMIT.value());
    }
    edges.dedup();
    edges
}

/// Eight equal panels over `[lo, hi]` with an extra edge where the heaviest cluster dissolves, the
/// one kink of `S` and `m_b` beyond the young disc.
fn panel_edges(lo: f64, hi: f64) -> Vec<f64> {
    let mut edges: Vec<f64> = (0..=8)
        .map(|k| lo + (hi - lo) * f64::from(k) / 8.0)
        .collect();
    let kink = dissolution_time(NurseryMassFunction::STANDARD.hi()).value();
    if kink > lo && kink < hi {
        edges.push(kink);
        edges.sort_by(f64::total_cmp);
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;

    fn myr(value: f64) -> Years {
        Years::new(value * YEARS_PER_MEGAYEAR)
    }

    fn milky_way() -> Galaxy {
        Galaxy::from_params(Seed::new(0x0900_0002), GalaxyParams::milky_way_like()).unwrap()
    }

    #[test]
    fn phi_young_is_near_nine_tenths_early_and_the_bound_share_late_and_falls() {
        assert!(
            (0.85..=0.95).contains(&phi_young(myr(3.0))),
            "{}",
            phi_young(myr(3.0))
        );
        let late = phi_young(myr(100.0));
        assert!((0.05..=0.15).contains(&late), "{late}");
        let mut last = phi_young(Years::ZERO);
        for k in 1..=2_000 {
            let phi = phi_young(myr(f64::from(k) * 0.05));
            assert!(phi <= last + 1e-15, "{k}: {phi} after {last}");
            last = phi;
        }
        assert!(phi_young(myr(-1.0)).abs() < 1e-15);
    }

    #[test]
    fn only_the_globulars_share_depends_on_the_band() {
        let galaxy = milky_way();
        let shares = galaxy.feature_shares();
        for population in crate::galaxy::POPULATIONS {
            let own =
                shares.phi(population, MassBand::A) - shares.phi_globular(population, MassBand::A);
            // Plan 13's substellar bands answer as band A (its Design note 4).
            for band in MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR) {
                let phi = shares.phi(population, band);
                assert!((phi - shares.phi_globular(population, band) - own).abs() < 1e-15);
                let f = shares.field_factor(population, band);
                let carved = shares.class_share(population, band);
                assert!((f + phi + carved - 1.0).abs() < 1e-15);
            }
        }
        // The globulars' depleted lowest bands hold less than their field's share, the rest
        // more; the weights conserve members over the field's band shares.
        let w = shares.globular_band_weights();
        assert!(w[0] < 1.0 && w[4] > 1.0, "{w:?}");
        let halo = shares.phi_globular(Population::Halo, MassBand::C);
        assert!((1e-3..0.2).contains(&halo), "{halo}");
    }

    /// Plan 11's P11.T7: the field factor, the feature share and the carved binary classes' share
    /// return each population's budget in every band, and the classes' shares, summed over the
    /// fixture's systems, hold the hosts the scratch table scales to.
    #[test]
    fn the_field_features_and_carved_classes_add_up_to_the_budget() {
        let galaxy = milky_way();
        let shares = galaxy.feature_shares();
        for population in crate::galaxy::POPULATIONS {
            for band in MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR) {
                let total = shares.field_factor(population, band)
                    + shares.phi(population, band)
                    + shares.class_share(population, band);
                assert!(
                    (total - 1.0).abs() < 1e-9,
                    "{population:?} {band:?}: {total}"
                );
            }
            for band in MassBand::SUBSTELLAR {
                assert!(shares.class_share(population, band).abs() < 1e-300);
            }
        }
        let mut hosts = 0.0;
        for component in galaxy.fields().components() {
            for band in MassBand::ALL {
                hosts += component.count()
                    * galaxy.shares().share(band, component.population())
                    * shares.class_share(component.population(), band);
            }
        }
        let expected: f64 = crate::tables::binary::CLASS_SHARES
            .rows()
            .iter()
            .map(|row| row.hosts_per_formed_mass(Population::Halo))
            .sum::<f64>()
            * crate::tables::binary::MILKY_WAY_FORMED_MASS;
        assert!(
            (hosts / expected - 1.0).abs() < 1e-6,
            "{hosts:e} against {expected:e}"
        );
        let largest = MassBand::ALL
            .iter()
            .map(|&band| shares.class_share(Population::OldThinDisc, band))
            .fold(0.0, f64::max);
        assert!(largest < 1e-3, "{largest}");
    }

    #[test]
    fn the_halo_takes_plan_ten_s_discrete_share() {
        let mut shares = milky_way().feature_shares().clone();
        shares.set_halo_discrete(0.05);
        let globulars = shares.phi_globular(Population::Halo, MassBand::A);
        let carved = shares.class_share(Population::Halo, MassBand::A);
        assert!(
            (shares.field_factor(Population::Halo, MassBand::A) - (0.95 - globulars - carved))
                .abs()
                < 1e-15
        );
    }
}
