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
//! Globulars count against the bulge, the thick disc and the halo. Phase 3 (P09.T12) supplies
//! their density, so until then their φ is 0 and their process places nothing. The halo's
//! discrete share of streams and dwarf cores is plan 10's, through
//! [`set_halo_discrete`](FeatureShares::set_halo_discrete).
//!
//! # Bands
//!
//! φ is uniform across the mass bands until the class tables of phase 2 deplete some
//! (TODO(P09.T9): per-band φ from the class counts). A band below the five stellar ones, which
//! plan 13 adds, takes band A's value through one private mapping, so plan 13 changes nothing here.

use crate::galaxy::Population;
use crate::galaxy::ages::{SubDisc, YOUNG_AGE_LIMIT};
use crate::galaxy::consts::YEARS_PER_MEGAYEAR;
use crate::galaxy::fields::Fields;
use crate::galaxy::imf::{MassBand, MassFunction};
use crate::galaxy::quad::gl_panels;
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::lifetime;
use crate::units::{SolarMasses, Years};

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
}

impl FeatureShares {
    /// The shares of the galaxy whose fields are `fields`, whose mass function is `mass_function`
    /// and which forms `mean_formed_mass` per system.
    #[must_use]
    pub fn new(
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
        }
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
        let _band = stellar_band_or_a(band);
        match population {
            Population::YoungThinDisc => self.young_mean_phi,
            Population::OldThinDisc => self.old_mean_phi,
            Population::Halo => self.halo_discrete,
            Population::ThickDisc
            | Population::Bulge
            | Population::LongBar
            | Population::NuclearDisc => 0.0,
        }
    }

    /// `1 − φ`: what `population`'s field density keeps of its budget in `band`.
    #[must_use]
    pub fn field_factor(&self, population: Population, band: MassBand) -> f64 {
        1.0 - self.phi(population, band)
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
    fn every_band_gets_the_same_share_until_phase_two() {
        let galaxy = milky_way();
        let shares = galaxy.feature_shares();
        for population in crate::galaxy::POPULATIONS {
            let a = shares.phi(population, MassBand::A);
            // Plan 13's substellar bands answer as band A (its Design note 4).
            for band in MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR) {
                assert!((shares.phi(population, band) - a).abs() < 1e-15);
                let f = shares.field_factor(population, band);
                assert!((f + shares.phi(population, band) - 1.0).abs() < 1e-15);
            }
        }
    }

    #[test]
    fn the_halo_takes_plan_ten_s_discrete_share() {
        let mut shares = milky_way().feature_shares().clone();
        shares.set_halo_discrete(0.05);
        assert!((shares.field_factor(Population::Halo, MassBand::A) - 0.95).abs() < 1e-15);
    }
}
