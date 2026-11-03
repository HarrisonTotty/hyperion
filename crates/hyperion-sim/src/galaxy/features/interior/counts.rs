//! The expected count of every member class of a cluster, in closed form (plan 09, P09.T9).
//!
//! A cluster's primaries are on the galaxy's mass function, and each class's count is the number
//! of systems it formed, `N₀`, times the share of primaries that end in it. `N₀` is counted in the
//! classes' own currency (ruling 145.1). Per system formed, the classes hold `p_dep(age)` today
//! (the living stars at the depleted slope below, with the binary mix and the runaways, and the
//! retained white dwarfs and neutron stars) and would hold `p_can(age)` at the canonical slope.
//! The cluster's history says how much of what stellar evolution left it still holds, its
//! dynamical survival `s`: L05's `disruption_survival(m₀, age)` for an open cluster (the `s` of
//! `M = m₀ μ_ev s`), and `M ÷ (M + Ṁ age)` on a globular's linear BM03 history, whose evolved mass
//! is `M + Ṁ t`. Then
//!
//! `N₀ = (M − M_BH) ÷ min(s p_can, p_dep)` and `K = min(s p_can ÷ p_dep, 1)`,
//!
//! and every class but the black holes is `K N₀` times its share per system, so the classes hold
//! the present mass less the black holes'. Evaporation takes stars of every class, and the
//! depleted slope says which it takes most of. Where the depletion alone removes more than the
//! history lost (`s p_can > p_dep`), `K` is held at 1: no band keeps more stars than it was born
//! with, and the escapers are all from the depleted bands A and B. Neither the galaxy's mean mass
//! formed per system nor any other population's μ enters: the classes keep their own binary mix
//! (below) until plan 11.
//!
//! - **Living stars (T9.a)** of band b have primaries in `[lo_b, min(hi_b, m_TO)]`. For 0.2–0.8 M☉
//!   the present slope is `α = −0.46 − 0.79 (log₁₀ t_rh − 9) + 0.54 n` (the brainstorm, after
//!   Baumgardt et al. 2023), `n` the cluster's normal mark, held at or above the canonical −1.5
//!   so that no cluster gains dwarfs, and the mass function is multiplied by `(m ÷ 0.8)^(α + 1.5)`
//!   there and by its value at 0.2 below, so bands A and B take the ratio of the depleted to the
//!   canonical integral. The fit is to globulars 12 Gyr old, so a cluster of another age reads it
//!   at `t_rh × 12 Gyr ÷ age`, its relaxation time in a globular's units of dynamical age (ours):
//!   a young cluster is not depleted.
//! - **White dwarfs (T9.a)** of band b have primaries in `[max(lo_b, m_TO), min(hi_b, 8 M☉)]`, and
//!   their mass is Kalirai et al. 2008's (ApJ 676, 594) initial–final relation, `0.109 m + 0.394`
//!   M☉, which sets their profile. They stay by their 1 km/s kick's retention.
//! - **Neutron stars (T9.b)** are plan 08's per-primary rate times their retention; they weigh 1.35
//!   M☉ (ours).
//! - **Black holes (T9.c)** are [`ClusterModel::black_hole_count`], unscaled, a compact Plummer
//!   class of scale `(0.1 + 0.2 f ÷ f_max) r_h`, `f_max` = 0.06 (ours: the plan's 0.1–0.3 `r_h`,
//!   "correlated with f").
//! - **Binaries (T9.e)** are the living stars' and neutron stars' share `b` by kind and population,
//!   profiled by their system mass, 1.5 times the primary's (ours): 5% of a globular's first
//!   population and 1% of its second ([`BINARY_FRACTIONS`]), and in an open cluster, from
//!   generator version 27, plan 11's multiple fraction at the band's mean living mass restricted
//!   to pairs inside the cluster's hard–soft boundary at its half-mass radius
//!   ([`open_cluster_binary_fraction`], P11.T8.f), 30% before it.
//! - **Runaways (T9.f)**: a cluster under 100 Myr has lost `f_ej(M)` of its band-E stars to close
//!   encounters, 15% rising log-linearly to 38% at 10^3.5 M☉ and falling back to 15% at 10^4.5,
//!   and 3% of band D. The peak at 10^3.5 M☉ is Oh, Kroupa and Pflamm-Altenburg's (2015, ApJ 805,
//!   92); the amplitudes are ours (provisional): their 15% and "up to 38%" are O-star shares over a
//!   whole population of embedded clusters, varying with the models, not the ends of `f_ej(M)`.
//!   From generator version 27 (P09.T33.b's `RUNAWAY_DEATHS_VERSION`; research-leans-21) a
//!   cluster of an ejecting source has lost instead, at each living mass of bands D and E, plan
//!   08's `RunawayModel` share ejected by its age, as a runaway or a walkaway by either channel
//!   (`ClassTable::ejected_by`), integrated over the band as its other counts are: the grid's
//!   runaway and walkaway classes hold every living ejected star at the whole budget, a feature's
//!   with the field's, so the cluster holds the rest. Its age takes the place of the 100 Myr step,
//!   and the cluster's mass no longer enters.
//! - **Tails (T9.g; ruling 139.4)**: a tail is as long as its oldest escaper has drifted and holds
//!   what the cluster lost in that time ([`tail_window`]). Escapers drift along the tail at Küpper,
//!   Macleod and Heggie's (2008, MNRAS 387, 1248, eq. 5's secular term) mean speed `v_drift = 2Ω
//!   |4Ω² ÷ κ² − 1| r_t`, so the tail runs `ℓ = min(reach, r_t + v_drift × age) − r_t` beyond the
//!   tidal radius on each side and its stars left in the last `τ = ℓ ÷ v_drift`. The tail's share
//!   `w ≤ 1` of every star the cluster has lost is a share of its history's time (ruling 145.2).
//!   An open cluster lost `M_esc(age) − M_esc(age − τ)` of its `M_esc(age)`, `M_esc(t) = m₀ μ_ev(t)
//!   (1 − s(m₀, t))` (Lamers et al. 2005 as built in `kinds::open_cluster`): stellar mass that
//!   left, not the gas stellar evolution returned. A globular loses mass at a steady `Ṁ` on its
//!   BM03 history, so `w = min(τ, age) ÷ age` if `Ṁ > 0` and 0 otherwise. Which stars and how many
//!   are the interior's own (ruling 142.3): band by band, the living systems born (net of the
//!   runaways, with the binary mix) less the living classes kept after the scale `K`, clamped at 0,
//!   so escapers are the lightest and band A is stripped hardest (Baumgardt and Makino 2003). Each
//!   band's tail is `w` times its lost systems, at their mean mass, spread uniformly over both
//!   sides. In a cluster with no depleted band and no remnant kept, the interior's lost living
//!   mass is `M (1 − s) ÷ s`, the history's own loss, which for an open cluster is L05's
//!   `M_esc(age)`: a young cluster's tail with `τ = age` carries L05's mass. The tail is labelled first population, though a globular's lost stars are counted over both. Stars
//!   lost by gas expulsion, isotropically at about 1 km/s, are outside the model (Design note 1's
//!   single bound mark).
//! - **Multiple populations (T9.h)**: a globular born above 10⁵ M☉ splits every class but the tail
//!   into its first population's share and its second's, whose core is 0.7 times the cluster's.
//!
//! A core-collapsed cluster's classes all take its cusp.

use crate::galaxy::catalogue_classes::core_collapse::ejecting_table;
use crate::galaxy::displaced::BirthSource;
use crate::galaxy::displaced::class_table::ClassTable;
use crate::galaxy::features::members::hard_soft_separation;
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::galaxy::quad::gl_log_panels;
use crate::galaxy::{Galaxy, PointLy};
use crate::math;
use crate::stellar::multiplicity::MultiplicityModel;
use crate::units::consts::SECONDS_PER_DAY;
use crate::units::{GravitationalParameter, LightYears, SolarMasses, Years};

use super::super::cluster::{ClusterKind, ClusterModel};
use super::profile::ProfileShape;
use super::{ClassKind, Generation, MemberClass, Multiplicity};

/// The canonical slope of the mass function over 0.2–0.8 M☉ (the brainstorm).
pub const CANONICAL_SLOPE: f64 = -1.5;

/// The depleted slope's per-cluster normal scatter (the brainstorm).
pub const DEPLETION_SCATTER: f64 = 0.54;

/// The age of the globulars the depleted slope was fitted to, years.
pub const DEPLETION_REFERENCE_AGE: f64 = 12e9;

/// The heaviest progenitor of a white dwarf, M☉.
pub const WHITE_DWARF_PROGENITOR_MAX: f64 = 8.0;

/// A neutron star's mass, M☉ (ours).
pub const NEUTRON_STAR_MASS: f64 = 1.35;

/// A binary's system mass over its primary's (ours).
pub const BINARY_MASS_FACTOR: f64 = 1.5;

/// The binary fractions: a globular's first population, its second, and an open cluster's before
/// generator version 27, from which plan 11's model gives an open cluster's
/// ([`open_cluster_binary_fraction`], P11.T8.f).
///
/// A globular's are kept by plan 11 (P11.T8.f): Lucatello et al.'s (2015, A&A 584, A52) radial
/// velocity binaries among ten globulars' red giants, 4.9% ± 1.3% of the first population and 1.2%
/// ± 0.4% of the second, rounded; and of the order of Milone et al.'s (2012, A&A 540, A16)
/// photometric main-sequence binaries of the ACS survey's 59 globulars, whose fraction is a few
/// per cent in the massive clusters and above ten only in some of the least massive, falling with
/// the cluster's luminosity (both recalled, not read at source: the pod that checked them reaches
/// no literature).
pub const BINARY_FRACTIONS: (f64, f64, f64) = (0.05, 0.01, 0.30);

/// The binary fraction of an open cluster's living stars of mean mass `mass` (plan 11, P11.T8.f):
/// plan 11's multiple fraction there ([`MultiplicityModel::multiple_fraction`]) times the share of
/// its period law below the period of a pair of `mass` times [`BINARY_MASS_FACTOR`] at the
/// cluster's hard–soft boundary at its half-mass radius ([`hard_soft_separation`]): the pairs that
/// encounters harden rather than unbind (Heggie 1975).
#[must_use]
pub fn open_cluster_binary_fraction(model: &ClusterModel, mass: SolarMasses) -> f64 {
    let multiplicity = MultiplicityModel::default_v1();
    let system = SolarMasses::new(mass.value() * BINARY_MASS_FACTOR);
    let separation = hard_soft_separation(model, model.half_mass_radius(), system);
    let mu = GravitationalParameter::from_solar_masses(system).value();
    let a = separation.value();
    let period_days = std::f64::consts::TAU * (a * a * a / mu).sqrt() / SECONDS_PER_DAY;
    multiplicity.multiple_fraction(mass)
        * multiplicity
            .period_distribution(mass)
            .cdf(math::log10(period_days))
}

/// The second population's core over the cluster's (scratch).
pub const SECOND_POPULATION_CORE_FACTOR: f64 = 0.7;

/// Clusters younger than this carry the runaway factor, years.
pub const RUNAWAY_AGE_LIMIT: f64 = 1e8;

/// Band D's share lost to runaways (the brainstorm's "a few per cent").
pub const RUNAWAY_BAND_D_SHARE: f64 = 0.03;

/// One class and its expected count and profile shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassCount {
    /// The class.
    pub class: MemberClass,
    /// Its expected count.
    pub expected: f64,
    /// Its profile's shape.
    pub shape: ProfileShape,
}

/// One band's tail: its expected count over both sides and its length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TailCount {
    /// The tail's class.
    pub class: MemberClass,
    /// Its expected count over both sides.
    pub expected: f64,
    /// How far it runs beyond the tidal radius on each side, `ℓ`.
    pub length: LightYears,
}

/// A cluster's tail in time and space (module documentation; ruling 139.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TailWindow {
    /// The escapers' mean drift along the tail, km/s.
    pub drift_speed: f64,
    /// How far the tail runs beyond the tidal radius on each side, `ℓ`.
    pub length: LightYears,
    /// The time its oldest star has drifted, `τ = ℓ ÷ v_drift`.
    pub duration: Years,
    /// The stellar mass the cluster lost in that time, M☉: `M_esc(age) − M_esc(age − τ)` for an
    /// open cluster, `Ṁ min(τ, age)` for a globular.
    pub mass: SolarMasses,
    /// All the stellar mass the cluster has lost since birth, M☉: `M_esc(age)` for an open
    /// cluster, `Ṁ age` on a globular's linear history (0 if `Ṁ ≤ 0`).
    pub lost: SolarMasses,
}

impl TailWindow {
    /// The tail's share of every star the cluster has lost, `w = mass ÷ lost`, at most 1; 0 if it
    /// has lost nothing. For a globular it is the time share `min(τ, age) ÷ age` (ruling 145.2).
    #[must_use]
    pub fn share(&self) -> f64 {
        if self.lost.value() > 0.0 {
            (self.mass.value() / self.lost.value()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// The tail window of `model` in `galaxy`, its tail cut at `reach` from the centre (module
/// documentation; ruling 139.4).
#[must_use]
pub fn tail_window(galaxy: &Galaxy, model: &ClusterModel, reach: LightYears) -> TailWindow {
    use super::super::kinds::open_cluster::{disruption_survival, evolution_survival};
    use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
    let age = model.age().value().max(0.0);
    let TailReach {
        drift_ly_per_year: drift,
        length,
    } = TailReach::of(
        galaxy,
        &model.position(),
        model.tidal_radius(),
        model.age(),
        reach,
    );
    let length = length.value();
    let tau = if drift > 0.0 { length / drift } else { 0.0 };
    let m0 = model.initial_mass();
    let (mass, lost) = match model.kind() {
        ClusterKind::Open => {
            let escaped = |t: f64| {
                let t = Years::new(t.max(0.0));
                m0.value() * evolution_survival(t) * (1.0 - disruption_survival(m0, t))
            };
            let lost = escaped(age).max(0.0);
            ((lost - escaped(age - tau)).max(0.0), lost)
        }
        // Ruling 145.2: a time share, which a steady `Ṁ` makes a mass share.
        ClusterKind::Globular => {
            let rate = model.mass_loss_rate();
            if rate > 0.0 && age > 0.0 {
                (rate * tau.min(age), rate * age)
            } else {
                (0.0, 0.0)
            }
        }
    };
    TailWindow {
        drift_speed: drift / LIGHT_YEARS_PER_YEAR_PER_KM_S,
        length: LightYears::new(length),
        duration: Years::new(tau),
        mass: SolarMasses::new(mass),
        lost: SolarMasses::new(lost),
    }
}

/// How far a cluster's tail runs: [`tail_window`]'s drift and length, which a bound on the tail's
/// reach reads without the cluster model (P09.T40.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TailReach {
    /// The escapers' mean drift along the tail, ly per year.
    pub(crate) drift_ly_per_year: f64,
    /// How far the tail runs beyond the tidal radius on each side.
    pub(crate) length: LightYears,
}

impl TailReach {
    /// The tail of a cluster at `position` (ly from the galactic centre) of tidal radius `tidal`
    /// and age `age`, cut at `reach` from the cluster's centre.
    #[must_use]
    pub(crate) fn of(
        galaxy: &Galaxy,
        position: &PointLy,
        tidal: LightYears,
        age: Years,
        reach: LightYears,
    ) -> Self {
        let (drift, length) = tail_drift_and_length(galaxy, position, tidal, age, reach);
        Self {
            drift_ly_per_year: drift,
            length: LightYears::new(length),
        }
    }
}

/// [`TailReach::of`]'s drift, ly per year, and length, ly.
#[must_use]
fn tail_drift_and_length(
    galaxy: &Galaxy,
    position: &PointLy,
    tidal: LightYears,
    age: Years,
    reach: LightYears,
) -> (f64, f64) {
    let p = position;
    let r = LightYears::new((p.x * p.x + p.y * p.y + p.z * p.z).sqrt());
    let r_t = tidal.value();
    let age = age.value().max(0.0);
    let (omega, kappa) = if r.value() > 0.0 {
        (
            galaxy.potential().omega(r).value(),
            galaxy.potential().kappa(r).value(),
        )
    } else {
        (0.0, 0.0)
    };
    // ly per year.
    let drift = if kappa > 0.0 {
        2.0 * omega * (4.0 * omega * omega / (kappa * kappa) - 1.0).abs() * r_t
    } else {
        0.0
    };
    let length = if drift > 0.0 {
        ((reach.value().min(r_t + drift * age)) - r_t).max(0.0)
    } else {
        0.0
    };
    (drift, length)
}

/// Every class of a cluster (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassCounts {
    /// The profiled classes, band by band in band order, then kind, generation and multiplicity.
    pub classes: Vec<ClassCount>,
    /// Each band's tail, if the cluster loses mass.
    pub tails: [Option<TailCount>; 5],
    /// The depleted slope over 0.2–0.8 M☉.
    pub depleted_slope: f64,
    /// The runaway share of band E, `f_ej`, for a young cluster, 0 otherwise; from generator
    /// version 27 the share of band E's living systems its ejected stars take (module
    /// documentation).
    pub runaway_share: f64,
    /// Each band's living systems born, net of the runaways (module documentation).
    pub born: [f64; 5],
    /// Each band's living systems lost: born less the living classes kept, at least 0.
    pub lost: [f64; 5],
    /// `p_can`: the classes' present mass per system formed at the canonical slope, M☉ (module
    /// documentation; ruling 145.1).
    pub canonical_per_system: f64,
    /// `p_dep`: the classes' present mass per system formed at the depleted slope, M☉.
    pub depleted_per_system: f64,
    /// `s`: the history's dynamical survival, the share of the mass stellar evolution left that
    /// the cluster still holds.
    pub survival: f64,
    /// `K = min(s p_can ÷ p_dep, 1)`: the scale on every class but the black holes.
    pub scale: f64,
    /// `N₀ = (M − M_BH) ÷ min(s p_can, p_dep)`: the systems the cluster formed.
    pub systems_formed: f64,
}

/// The dynamical survival `s` of `model` (module documentation; ruling 145.1): L05's
/// `disruption_survival(m₀, age)` for an open cluster, `M ÷ (M + Ṁ age)` for a globular.
#[must_use]
pub fn dynamical_survival(model: &ClusterModel) -> f64 {
    let age = model.age().value().max(0.0);
    match model.kind() {
        ClusterKind::Open => super::super::kinds::open_cluster::disruption_survival(
            model.initial_mass(),
            Years::new(age),
        ),
        ClusterKind::Globular => {
            let m = model.mass().value();
            let evolved = m + model.mass_loss_rate().max(0.0) * age;
            if evolved > 0.0 { m / evolved } else { 1.0 }
        }
    }
}

/// The depleted slope of a cluster (module documentation).
#[must_use]
pub fn depleted_slope(model: &ClusterModel) -> f64 {
    let age = model.age().value().max(1.0);
    let t = model.relaxation_time().value() * DEPLETION_REFERENCE_AGE / age;
    let alpha = -0.46 - 0.79 * (math::log10(t) - 9.0)
        + DEPLETION_SCATTER * model.marks().depletion_normal();
    alpha.max(CANONICAL_SLOPE)
}

/// The depletion factor at `m` for slope `alpha` (module documentation).
#[must_use]
pub fn depletion(m: f64, alpha: f64) -> f64 {
    let x = alpha - CANONICAL_SLOPE;
    if m >= 0.8 {
        1.0
    } else if m >= 0.2 {
        math::powf(m / 0.8, x)
    } else {
        math::powf(0.25, x)
    }
}

/// The ejected share of a young cluster's band-E stars (module documentation).
#[must_use]
pub fn runaway_share(mass: SolarMasses) -> f64 {
    let x = math::log10(mass.value());
    if x <= 2.5 || x >= 4.5 {
        0.15
    } else if x <= 3.5 {
        0.15 + 0.23 * (x - 2.5)
    } else {
        0.38 - 0.23 * (x - 3.5)
    }
}

/// How a cluster's living counts net out its ejected stars (module documentation, T9.f).
#[derive(Debug, Clone, Copy)]
enum Netting<'g> {
    /// Before generator version 27: `ejected` of band E and [`RUNAWAY_BAND_D_SHARE`] of band D in a
    /// `young` cluster.
    Encounters { young: bool, ejected: f64 },
    /// From 27: `RunawayModel`'s share of `source`'s stars ejected by the cluster's `age`, mass by
    /// mass, in bands D and E.
    Model {
        table: &'g ClassTable,
        source: BirthSource,
        age: Years,
    },
}

impl<'g> Netting<'g> {
    /// The netting of `model` in `galaxy`: `RunawayModel`'s where the core-collapse class follows
    /// the runaways and walkaways, from 27 (`ejecting_table`), the encounters' before.
    #[must_use]
    fn of(galaxy: &'g Galaxy, model: &ClusterModel) -> Self {
        if let Some(table) = ejecting_table(galaxy) {
            // A cluster without a density component is a test's or a named one: the thin disc's.
            return Self::Model {
                table,
                source: model
                    .parameters()
                    .component
                    .map_or(BirthSource::ThinDisc, |c| table.component_source(c)),
                age: model.age(),
            };
        }
        let young = model.age().value() < RUNAWAY_AGE_LIMIT;
        Self::Encounters {
            young,
            ejected: if young {
                runaway_share(model.mass())
            } else {
                0.0
            },
        }
    }

    /// The share of `band`'s living stars of initial mass `m` the cluster keeps: 1 before 27,
    /// whose share is the band's scalar ([`band_share`](Self::band_share)).
    #[must_use]
    fn kept(&self, band: MassBand, m: f64) -> f64 {
        match *self {
            Self::Model { table, source, age } if matches!(band, MassBand::D | MassBand::E) => {
                1.0 - table.ejected_by(source, SolarMasses::new(m), age)
            }
            Self::Encounters { .. } | Self::Model { .. } => 1.0,
        }
    }

    /// The share of `band`'s living stars the cluster keeps, whatever their mass: before 27,
    /// `1 − f_ej` of band E and `1 − 3%` of band D in a young cluster; 1 otherwise and from 27,
    /// whose share is each mass's ([`kept`](Self::kept)).
    #[must_use]
    fn band_share(&self, band: MassBand) -> f64 {
        match (*self, band) {
            (Self::Encounters { ejected, .. }, MassBand::E) => 1.0 - ejected,
            (Self::Encounters { young: true, .. }, MassBand::D) => 1.0 - RUNAWAY_BAND_D_SHARE,
            (
                Self::Encounters { .. } | Self::Model { .. },
                MassBand::A
                | MassBand::B
                | MassBand::C
                | MassBand::D
                | MassBand::E
                | MassBand::BrownDwarf
                | MassBand::RoguePlanet,
            ) => 1.0,
        }
    }
}

/// Kalirai et al. 2008's white-dwarf mass for a progenitor of `m`, M☉.
#[must_use]
pub fn white_dwarf_mass(m: f64) -> f64 {
    0.109 * m + 0.394
}

/// The ratio of the depleted to the canonical number of every band at slope `alpha`.
#[must_use]
pub fn band_depletion(f: &dyn MassFunction, alpha: f64) -> [f64; 5] {
    std::array::from_fn(|b| {
        let (lo, hi) = (MASS_BAND_EDGES[b], MASS_BAND_EDGES[b + 1]);
        let (depleted, _) = moments(f, lo, hi, |m| depletion(m, alpha));
        let (canonical, _) = moments(f, lo, hi, |_| 1.0);
        if canonical > 0.0 {
            depleted / canonical
        } else {
            1.0
        }
    })
}

/// `∫ φ w(m) dm` and `∫ m φ w(m) dm` over `[lo, hi]` for the normalised mass function φ and weight
/// `w`, by 32-node Gauss–Legendre panels in `ln m` split at the depletion's kinks.
pub(crate) fn moments(
    f: &dyn MassFunction,
    lo: f64,
    hi: f64,
    w: impl Fn(f64) -> f64,
) -> (f64, f64) {
    if hi <= lo {
        return (0.0, 0.0);
    }
    let mut edges = vec![lo];
    for k in [0.2, 0.8] {
        if k > lo && k < hi {
            edges.push(k);
        }
    }
    edges.extend(f.breaks().iter().copied().filter(|&b| b > lo && b < hi));
    edges.push(hi);
    edges.sort_by(f64::total_cmp);
    edges.dedup();
    let norm = f.integral(0.0, 150.0);
    let n = gl_log_panels(|m| f.pdf(m) * w(m), &edges) / norm;
    let mass = gl_log_panels(|m| m * f.pdf(m) * w(m), &edges) / norm;
    (n, mass)
}

/// Every class's expected count for `model` in `galaxy`, with the tails out to `reach` from the
/// centre (module documentation).
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::cluster::{ClusterKind, ClusterMarks, ClusterModel, ClusterParameters};
/// use hyperion_sim::galaxy::features::interior::counts::class_counts;
/// use hyperion_sim::galaxy::{PointLy, Population};
/// use hyperion_sim::units::{Dex, LightYears, SolarMasses, Years};
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let model = ClusterModel::new(&galaxy, &ClusterParameters {
///     kind: ClusterKind::Open,
///     population: Population::OldThinDisc,
///     component: None,
///     position: PointLy::new(0.0, 26_000.0, 0.0),
///     age: Years::new(3e8),
///     fe_h: Dex::ZERO,
///     mass: SolarMasses::new(1e3),
///     initial_mass: SolarMasses::new(2e3),
///     half_mass_radius: LightYears::new(8.0),
///     birth_half_mass_radius: LightYears::new(8.0),
///     core_radius: None,
///     concentration: 1.0,
///     mass_loss_rate: 1e-6,
///     age_spread: Years::ZERO,
///     marks: ClusterMarks::MEDIAN,
/// });
/// let counts = class_counts(&galaxy, &model, LightYears::new(400.0));
/// // A thousand solar masses of stars: a few thousand systems, and a tail.
/// let total: f64 = counts.classes.iter().map(|c| c.expected).sum();
/// assert!((500.0..5_000.0).contains(&total), "{total}");
/// assert!(counts.tails.iter().flatten().count() > 0);
/// ```
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "one closed form per class, in the module documentation's order"
)]
pub fn class_counts(galaxy: &Galaxy, model: &ClusterModel, reach: LightYears) -> ClassCounts {
    let f = galaxy.mass_function();
    let m_to = model.turn_off_mass().value();
    let alpha = depleted_slope(model);
    let netting = Netting::of(galaxy, model);
    let mut ejected = match netting {
        Netting::Encounters { ejected, .. } => ejected,
        Netting::Model { .. } => 0.0,
    };
    let retention = *model.retention();
    let (ns_per_primary, _) = model.remnants_per_primary();
    let bh_count = model.black_hole_count();
    let first_share = model.first_population_share();
    let core = model.profile_core_radius().value();
    let cusp = model.cusp_slope();
    let shape_for = |mass: f64, generation: Generation| {
        if let Some(slope) = cusp {
            ProfileShape::Cusp { slope }
        } else {
            let c = match generation {
                Generation::First => core,
                Generation::Second => core * SECOND_POPULATION_CORE_FACTOR,
            };
            ProfileShape::cored(c, mass / m_to, model.half_mass_radius().value())
        }
    };
    let binary_fraction = |generation: Generation, mean: f64| match (model.kind(), generation) {
        (ClusterKind::Open, _) if mean > 0.0 => {
            open_cluster_binary_fraction(model, SolarMasses::new(mean))
        }
        (ClusterKind::Open, _) => BINARY_FRACTIONS.2,
        (ClusterKind::Globular, Generation::First) => BINARY_FRACTIONS.0,
        (ClusterKind::Globular, Generation::Second) => BINARY_FRACTIONS.1,
    };

    // Counts per system formed, before `K N₀`: (class, count, shape).
    let mut raw: Vec<ClassCount> = Vec::new();
    let mut born = [0.0; 5];
    let mut born_mass = [0.0; 5];
    for band in MassBand::ALL {
        let (lo, hi) = (
            MASS_BAND_EDGES[band.index()],
            MASS_BAND_EDGES[band.index() + 1],
        );
        let runaway = netting.band_share(band);
        let live_hi = hi.min(m_to);
        let (n_live, m_live) = moments(f, lo, live_hi, |m| {
            depletion(m, alpha) * netting.kept(band, m)
        });
        if let (Netting::Model { .. }, MassBand::E) = (netting, band) {
            let (all, _) = moments(f, lo, live_hi, |m| depletion(m, alpha));
            ejected = if all > 0.0 { 1.0 - n_live / all } else { 0.0 };
        }
        // The mass a band's binary fraction is read at: its living stars' mean, or for a band with
        // none, its primaries' at birth.
        let fraction_mass = if n_live > 0.0 {
            m_live / n_live
        } else {
            let (n, m) = moments(f, lo, hi, |_| 1.0);
            if n > 0.0 { m / n } else { 0.0 }
        };
        let (n_canon, m_canon) = moments(f, lo, live_hi, |m| netting.kept(band, m));
        let wd_lo = lo.max(m_to);
        let wd_hi = hi.min(WHITE_DWARF_PROGENITOR_MAX);
        let (n_wd, m_wd_initial) = moments(f, wd_lo, wd_hi, |_| 1.0);
        let wd_mass = if n_wd > 0.0 {
            white_dwarf_mass(m_wd_initial / n_wd)
        } else {
            0.0
        };
        for generation in [Generation::First, Generation::Second] {
            let share = match generation {
                Generation::First => first_share,
                Generation::Second => 1.0 - first_share,
            };
            if share <= 0.0 {
                continue;
            }
            let b = binary_fraction(generation, fraction_mass);
            born[band.index()] += n_canon * share * runaway;
            born_mass[band.index()] +=
                m_canon * share * runaway * (1.0 - b + b * BINARY_MASS_FACTOR);
            if n_live > 0.0 {
                let mean = m_live / n_live;
                for (multiplicity, part, mass) in [
                    (Multiplicity::Single, 1.0 - b, mean),
                    (Multiplicity::Binary, b, mean * BINARY_MASS_FACTOR),
                ] {
                    raw.push(ClassCount {
                        class: MemberClass {
                            kind: ClassKind::Living,
                            band,
                            generation,
                            multiplicity,
                            initial_mass_range: (SolarMasses::new(lo), SolarMasses::new(live_hi)),
                            mean_mass: SolarMasses::new(mass),
                        },
                        expected: n_live * share * part * runaway,
                        shape: shape_for(mass, generation),
                    });
                }
            }
            if n_wd > 0.0 {
                raw.push(ClassCount {
                    class: MemberClass {
                        kind: ClassKind::WhiteDwarf,
                        band,
                        generation,
                        multiplicity: Multiplicity::Single,
                        initial_mass_range: (SolarMasses::new(wd_lo), SolarMasses::new(wd_hi)),
                        mean_mass: SolarMasses::new(wd_mass),
                    },
                    expected: n_wd * share * retention.white_dwarfs,
                    shape: shape_for(wd_mass, generation),
                });
            }
            if band == MassBand::E {
                let ns = ns_per_primary * retention.neutron_stars * share;
                for (multiplicity, part, mass) in [
                    (Multiplicity::Single, 1.0 - b, NEUTRON_STAR_MASS),
                    (
                        Multiplicity::Binary,
                        b,
                        NEUTRON_STAR_MASS * BINARY_MASS_FACTOR,
                    ),
                ] {
                    raw.push(ClassCount {
                        class: MemberClass {
                            kind: ClassKind::NeutronStar,
                            band,
                            generation,
                            multiplicity,
                            initial_mass_range: (SolarMasses::new(8.0), SolarMasses::new(hi)),
                            mean_mass: SolarMasses::new(mass),
                        },
                        expected: ns * part,
                        shape: shape_for(mass, generation),
                    });
                }
            }
        }
    }
    // Ruling 145.1: N₀ and K in the classes' own currency, every class but the black holes to the
    // present mass less the black holes'.
    let bh_mass = model.black_hole_fraction() * model.mass().value();
    let stellar = (model.mass().value() - bh_mass).max(0.0);
    let mass_of = |c: &ClassCount| c.expected * c.class.mean_mass.value();
    let p_dep: f64 = raw.iter().map(mass_of).sum();
    let living: f64 = raw
        .iter()
        .filter(|c| c.class.kind == ClassKind::Living)
        .map(mass_of)
        .sum();
    let p_can = born_mass.iter().sum::<f64>() + (p_dep - living);
    let survival = dynamical_survival(model);
    let floor = (survival * p_can).min(p_dep);
    let (n0, k) = if floor > 0.0 {
        (stellar / floor, (survival * p_can / p_dep).min(1.0))
    } else {
        (0.0, 0.0)
    };
    for c in &mut raw {
        c.expected *= k * n0;
    }
    for (n, m) in born.iter_mut().zip(&mut born_mass) {
        *n *= n0;
        *m *= n0;
    }
    if let (true, Some(scale)) = (bh_count > 0.0, model.black_hole_scale()) {
        let scale = scale.value();
        raw.push(ClassCount {
            class: MemberClass {
                kind: ClassKind::BlackHole,
                band: MassBand::E,
                generation: Generation::First,
                multiplicity: Multiplicity::Single,
                initial_mass_range: (SolarMasses::new(20.0), SolarMasses::new(150.0)),
                mean_mass: SolarMasses::new(super::super::cluster::MEAN_BLACK_HOLE_MASS),
            },
            expected: bh_count,
            shape: ProfileShape::Plummer { scale },
        });
    }
    raw.sort_by(|a, b| {
        (
            a.class.band,
            a.class.kind,
            a.class.generation,
            a.class.multiplicity,
        )
            .cmp(&(
                b.class.band,
                b.class.kind,
                b.class.generation,
                b.class.multiplicity,
            ))
    });

    // Tails: the interior's lost stars, band by band, times the window's share of them.
    let mut kept = [0.0; 5];
    let mut kept_mass = [0.0; 5];
    for c in raw.iter().filter(|c| c.class.kind == ClassKind::Living) {
        kept[c.class.band.index()] += c.expected;
        kept_mass[c.class.band.index()] += c.expected * c.class.mean_mass.value();
    }
    let mut lost = [0.0; 5];
    let mut lost_mass = [0.0; 5];
    for i in 0..5 {
        let (n, m) = (born[i] - kept[i], born_mass[i] - kept_mass[i]);
        if n > 0.0 && m > 0.0 {
            lost[i] = n;
            lost_mass[i] = m;
        }
    }
    let mut tails = [None; 5];
    let window = tail_window(galaxy, model, reach);
    let w = window.share();
    if w > 0.0 && window.length.value() > 0.0 {
        for band in MassBand::ALL {
            let i = band.index();
            if lost[i] > 0.0 {
                let (lo, hi) = (MASS_BAND_EDGES[i], MASS_BAND_EDGES[i + 1]);
                tails[i] = Some(TailCount {
                    class: MemberClass {
                        kind: ClassKind::Tail,
                        band,
                        generation: Generation::First,
                        multiplicity: Multiplicity::Single,
                        initial_mass_range: (SolarMasses::new(lo), SolarMasses::new(hi.min(m_to))),
                        mean_mass: SolarMasses::new(lost_mass[i] / lost[i]),
                    },
                    expected: lost[i] * w,
                    length: window.length,
                });
            }
        }
    }
    ClassCounts {
        classes: raw,
        tails,
        depleted_slope: alpha,
        runaway_share: ejected,
        born,
        lost,
        canonical_per_system: p_can,
        depleted_per_system: p_dep,
        survival,
        scale: k,
        systems_formed: n0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::imf::MassFunctionKind;

    /// P09.T9.a (ruling 126.8): at 47 Tucanae's measured slope (Baumgardt et al. 2023, MNRAS 521,
    /// 3991, Table B1: `α_Tot` = −0.65) band A, 0.08–0.5 M☉, holds 0.386 ± 0.01 of the canonical
    /// count ("a third" was the factor below 0.2 M☉).
    #[test]
    fn band_a_holds_its_depleted_share_at_47_tucanae_s_slope() {
        let f = MassFunctionKind::default().to_mass_function();
        let (lo, hi) = (MASS_BAND_EDGES[0], MASS_BAND_EDGES[1]);
        let (depleted, _) = moments(f.as_ref(), lo, hi, |m| depletion(m, -0.65));
        let (canonical, _) = moments(f.as_ref(), lo, hi, |_| 1.0);
        let ratio = depleted / canonical;
        assert!((ratio - 0.386).abs() <= 0.01, "{ratio}");
    }

    /// P09.T9.f from 27 (research-leans-21): a young nursery of the young disc nets, at each of the
    /// class table's mass nodes of bands D and E below its turn-off, `RunawayModel`'s share of its
    /// stars ejected by its age, both kinds and both channels, to 10⁻¹²; so does an old open
    /// cluster, whose every ejection has passed, past the 100 Myr step; and the counts take it:
    /// band E's living systems fall by about its share.
    #[test]
    fn from_27_a_cluster_nets_the_runaway_model_s_ejected_stars() {
        use crate::Seed;
        use crate::galaxy::ages::SubDisc;
        use crate::galaxy::displaced::runaway::Ejected;
        use crate::galaxy::features::catalogue::FeatureCatalogue;
        use crate::galaxy::features::ids::FeatureProcess;
        use crate::galaxy::params::GalaxyParams;
        use crate::galaxy::rules::GalaxyRules;

        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(0x0909_f000_0000_0027),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .unwrap();
        let table = ejecting_table(&galaxy).expect("every 27 hold follows the runaways");
        let models = |process: FeatureProcess| {
            FeatureCatalogue::walk_process(&galaxy, process)
                .filter_map(|feature| ClusterModel::from_record(&galaxy, &feature))
                .filter(|model| model.parameters().component.is_some())
        };
        let nursery = models(FeatureProcess::Nursery)
            .find(|model| model.age().value() < 1.5e7)
            .expect("a young bound nursery");
        let old = models(FeatureProcess::OldOpenCluster(SubDisc::First))
            .find(|model| model.age().value() > 2e8)
            .expect("an old open cluster");
        for model in [&nursery, &old] {
            let netting = Netting::of(&galaxy, model);
            let Netting::Model { source, age, .. } = netting else {
                panic!("from 27 a cluster reads the runaway model");
            };
            assert_eq!(age, model.age());
            let runaways = table.runaway_model();
            let m_to = model.turn_off_mass().value();
            let mut checked = 0;
            for band in [MassBand::D, MassBand::E] {
                for &m in table.nodes(band).masses().iter().filter(|&&m| m < m_to) {
                    let mass = SolarMasses::new(m);
                    let ejected = [
                        (Ejected::Runaway, runaways.runaway_share(mass)),
                        (Ejected::Walkaway, runaways.walkaway_share(mass)),
                    ]
                    .into_iter()
                    .fold(0.0, |sum, (kind, share)| {
                        let before = table
                            .ejection_ages(source, kind, mass)
                            .into_iter()
                            .filter(|&(t, _)| t < age)
                            .fold(0.0, |s, (_, w)| s + w);
                        sum + share * before
                    });
                    let netted = 1.0 - netting.kept(band, m);
                    assert!(
                        (netted - ejected).abs() <= 1e-12,
                        "{band:?} at {m} M☉, {age:?}: {netted} netted against {ejected}"
                    );
                    checked += 1;
                }
            }
            assert!(checked > 5, "{checked} nodes below the turn-off");
            // An old cluster's ejections have all passed: each mass's whole lifetime share.
            if model.age().value() > 2e8 {
                let m = SolarMasses::new(5.0);
                let lifetime = runaways.runaway_share(m) + runaways.walkaway_share(m);
                assert!((1.0 - netting.kept(MassBand::D, 5.0) - lifetime).abs() <= 1e-12);
            }
        }
        let counts = class_counts(&galaxy, &nursery, LightYears::new(400.0));
        assert!(
            (0.05..0.6).contains(&counts.runaway_share),
            "{}",
            counts.runaway_share
        );
    }

    #[test]
    fn no_cluster_gains_dwarfs() {
        for alpha in [-3.0_f64, -1.5, -1.0, 0.0, 1.0] {
            let a = alpha.max(CANONICAL_SLOPE);
            for m in [0.1, 0.3, 0.7, 1.0] {
                assert!(depletion(m, a) <= 1.0 + 1e-15);
            }
        }
    }

    /// P09.T9.f, table-driven.
    #[test]
    fn the_runaway_share_peaks_at_three_thousand_solar_masses() {
        for (m, want) in [
            (1e2, 0.15),
            (math::exp10(3.0), 0.265),
            (math::exp10(3.5), 0.38),
            (1e4, 0.265),
            (1e5, 0.15),
        ] {
            let got = runaway_share(SolarMasses::new(m));
            assert!((got - want).abs() < 1e-12, "{m}: {got}");
        }
    }

    #[test]
    fn white_dwarfs_follow_kalirai() {
        assert!((white_dwarf_mass(1.0) - 0.503).abs() < 1e-12);
        assert!((white_dwarf_mass(6.0) - 1.048).abs() < 1e-12);
    }
}
