//! The expected count of every member class of a cluster, in closed form (plan 09, P09.T9).
//!
//! A cluster formed `N₀ = M₀ ÷ m̄_f` systems, `m̄_f` the galaxy's mass formed per system, their
//! primaries on the galaxy's mass function. Each class's count is `N₀` times the share of
//! primaries that end in it, and then every class but the black holes is scaled by one factor `K`
//! so that the classes' mass is the cluster's present mass: evaporation takes stars of every
//! class, and the depleted slope (below) says which it takes most of.
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
//! - **Binaries (T9.e)** are the living stars' and neutron stars' share `b` by kind and population
//!   (scratch: 5% of a globular's first population, 1% of its second, 30% in open clusters),
//!   profiled by their system mass, 1.5 times the primary's (ours).
//! - **Runaways (T9.f)**: a cluster under 100 Myr has lost `f_ej(M)` of its band-E stars to close
//!   encounters, 15% rising log-linearly to 38% at 10^3.5 M☉ and falling back to 15% at 10^4.5,
//!   and 3% of band D. The peak at 10^3.5 M☉ is Oh, Kroupa and Pflamm-Altenburg's (2015, ApJ 805,
//!   92); the amplitudes are ours (provisional): their 15% and "up to 38%" are O-star shares over a
//!   whole population of embedded clusters, varying with the models, not the ends of `f_ej(M)`.
//! - **Tails (T9.g)**: per band, the members lost in the grid's reach, `Ṁ ÷ (2 ⟨m⟩ v_drift)` per
//!   light-year on each side (half the loss leads, half trails), with the band shares of what the cluster lost (the canonical minus
//!   the depleted living stars) and `v_drift` the central dispersion (ours). First population
//!   only.
//! - **Multiple populations (T9.h)**: a globular born above 10⁵ M☉ splits every class but the tail
//!   into its first population's share and its second's, whose core is 0.7 times the cluster's.
//!
//! A core-collapsed cluster's classes all take its cusp.

use crate::galaxy::Galaxy;
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::galaxy::quad::gl_log_panels;
use crate::math;
use crate::units::{LightYears, SolarMasses};

use super::super::cluster::{ClusterKind, ClusterModel};
use super::abundances::first_population_share;
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

/// The binary fractions (scratch; plan 11 replaces the numbers): a globular's first population,
/// its second, an open cluster.
pub const BINARY_FRACTIONS: (f64, f64, f64) = (0.05, 0.01, 0.30);

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

/// One band's tail: its expected count over both sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TailCount {
    /// The tail's class.
    pub class: MemberClass,
    /// Its expected count over both sides, from the tidal radius to the reach.
    pub expected: f64,
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
    /// The runaway share of band E, `f_ej`, for a young cluster; 0 otherwise.
    pub runaway_share: f64,
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
fn moments(f: &dyn MassFunction, lo: f64, hi: f64, w: impl Fn(f64) -> f64) -> (f64, f64) {
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
    let n0 = model.initial_mass().value() / galaxy.mean_formed_mass().value();
    let m_to = model.turn_off_mass().value();
    let alpha = depleted_slope(model);
    let young = model.age().value() < RUNAWAY_AGE_LIMIT;
    let ejected = if young {
        runaway_share(model.mass())
    } else {
        0.0
    };
    let retention = *model.retention();
    let (ns_per_primary, _) = model.remnants_per_primary();
    let bh_count = model.black_hole_count();
    let first_share = match model.kind() {
        ClusterKind::Globular => first_population_share(model.initial_mass()),
        ClusterKind::Open => 1.0,
    };
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
            ProfileShape::cored(c, mass / m_to)
        }
    };
    let binary_fraction = |generation: Generation| match (model.kind(), generation) {
        (ClusterKind::Open, _) => BINARY_FRACTIONS.2,
        (ClusterKind::Globular, Generation::First) => BINARY_FRACTIONS.0,
        (ClusterKind::Globular, Generation::Second) => BINARY_FRACTIONS.1,
    };

    // Raw counts before the mass scaling: (class, count, shape).
    let mut raw: Vec<ClassCount> = Vec::new();
    let mut lost = [0.0; 5];
    let mut lost_mass = [0.0; 5];
    for band in MassBand::ALL {
        let (lo, hi) = (
            MASS_BAND_EDGES[band.index()],
            MASS_BAND_EDGES[band.index() + 1],
        );
        let runaway = match band {
            MassBand::E => 1.0 - ejected,
            MassBand::D if young => 1.0 - RUNAWAY_BAND_D_SHARE,
            MassBand::A
            | MassBand::B
            | MassBand::C
            | MassBand::D
            | MassBand::BrownDwarf
            | MassBand::RoguePlanet => 1.0,
        };
        let live_hi = hi.min(m_to);
        let (n_live, m_live) = moments(f, lo, live_hi, |m| depletion(m, alpha));
        let (n_canon, m_canon) = moments(f, lo, live_hi, |_| 1.0);
        lost[band.index()] = n0 * (n_canon - n_live).max(0.0);
        lost_mass[band.index()] = n0 * (m_canon - m_live).max(0.0);
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
            let b = binary_fraction(generation);
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
                        expected: n0 * n_live * share * part * runaway,
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
                    expected: n0 * n_wd * share * retention.white_dwarfs,
                    shape: shape_for(wd_mass, generation),
                });
            }
            if band == MassBand::E {
                let ns = n0 * ns_per_primary * retention.neutron_stars * share;
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
    // The scale K: every class but the black holes, to the present mass less the black holes'.
    let bh_mass = model.black_hole_fraction() * model.mass().value();
    let raw_mass: f64 = raw
        .iter()
        .map(|c| c.expected * c.class.mean_mass.value())
        .sum();
    let k = if raw_mass > 0.0 {
        ((model.mass().value() - bh_mass) / raw_mass).max(0.0)
    } else {
        0.0
    };
    for c in &mut raw {
        c.expected *= k;
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

    // Tails: members lost within the reach, by what was lost.
    let mut tails = [None; 5];
    let rate = model.mass_loss_rate();
    let length = (reach.value() - model.tidal_radius().value()).max(0.0);
    if rate > 0.0 && length > 0.0 {
        let lost_total: f64 = lost.iter().sum();
        let (shares, masses): ([f64; 5], [f64; 5]) = if lost_total > 0.0 {
            (
                lost.map(|l| l / lost_total),
                std::array::from_fn(|i| {
                    if lost[i] > 0.0 {
                        lost_mass[i] / lost[i]
                    } else {
                        0.0
                    }
                }),
            )
        } else {
            // A cluster losing no dwarfs yet loses the living stars in their own proportions.
            let mut s = [0.0; 5];
            let mut m = [0.0; 5];
            for c in raw.iter().filter(|c| c.class.kind == ClassKind::Living) {
                s[c.class.band.index()] += c.expected;
                m[c.class.band.index()] += c.expected * c.class.mean_mass.value();
            }
            let total: f64 = s.iter().sum();
            let masses = std::array::from_fn(|i| if s[i] > 0.0 { m[i] / s[i] } else { 0.0 });
            (s.map(|x| if total > 0.0 { x / total } else { 0.0 }), masses)
        };
        let mean_lost: f64 = shares.iter().zip(&masses).map(|(s, m)| s * m).sum();
        let v_drift = model.sigma(LightYears::ZERO).value();
        if mean_lost > 0.0 && v_drift > 0.0 {
            // Members per light-year of tail on each side: Ṁ ÷ (2 ⟨m⟩ v), with v in ly per year.
            let v_ly_per_yr = v_drift * crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
            let per_ly = rate / (2.0 * mean_lost * v_ly_per_yr);
            for band in MassBand::ALL {
                let share = shares[band.index()];
                if share > 0.0 {
                    let (lo, hi) = (
                        MASS_BAND_EDGES[band.index()],
                        MASS_BAND_EDGES[band.index() + 1],
                    );
                    tails[band.index()] = Some(TailCount {
                        class: MemberClass {
                            kind: ClassKind::Tail,
                            band,
                            generation: Generation::First,
                            multiplicity: Multiplicity::Single,
                            initial_mass_range: (
                                SolarMasses::new(lo),
                                SolarMasses::new(hi.min(m_to)),
                            ),
                            mean_mass: SolarMasses::new(masses[band.index()]),
                        },
                        expected: per_ly * 2.0 * length * share,
                    });
                }
            }
        }
    }
    ClassCounts {
        classes: raw,
        tails,
        depleted_slope: alpha,
        runaway_share: ejected,
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
