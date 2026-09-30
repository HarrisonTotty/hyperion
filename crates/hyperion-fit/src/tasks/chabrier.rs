//! The scale of Chabrier's high-mass branch (plan 15, P15.T4), which `galaxy::imf::Chabrier`
//! multiplies its power law above 1 M☉ by.
//!
//! **P15.T4.a, the move.** Plan 02 shipped the scale as a named constant, 0.68, in
//! `galaxy::imf` (its Design note 5), and revision 0 moved it into `tables::chabrier` at the same
//! value, provisionally. **P15.T4.b, the fit** (revision 1, ruling 138): the scale s is the
//! multinomial maximum-likelihood fit to the 20 pc census's primary counts in bands A–D
//! ([`CENSUS_PRIMARY_COUNTS`], our tally of Kirkpatrick et al. 2024, ApJS 271, 55, Table 4), with
//! the model's band shares renormalised over 0.08–8 M☉, since the census holds no neutron-star or
//! black-hole system. It depends on the mass function alone. The quadratures are deterministic, so
//! nothing is sampled, and s is found by a fixed golden-section search ([`fit_scale`]).
//!
//! The run then checks, like for like, over systems whose primary lies below 8 M☉
//! ([`ChabrierChecks`]): Pearson χ² at most 9.21 on 2 degrees of freedom with every band within 2
//! Poisson errors; all stars below 0.5 M☉, plan 11's drawn companions included, in 67.8–71.0%; and
//! the local mid-plane mean present-day mass per system, counting stars and white dwarfs only, in
//! 0.54–0.60 M☉. It reports, without gating, the all-inclusive local mean that the system count
//! uses, the neutron stars' and black holes' mass per system inside it, the primaries' mean
//! present mass against the census's 0.437, and s under Cummings's and El-Badry's initial–final
//! mass relations. Nothing is clamped: a failed check is reported in the acceptance line and the
//! table is then marked provisional, for the owner to rule on.

use std::f64::consts::PI;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_KILOPARSEC;
use hyperion_sim::galaxy::fates::Counted;
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::imf::{Chabrier, MassFunction, Truncated};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::galaxy::{POPULATIONS, PointLy};
use hyperion_sim::math;
use hyperion_sim::stellar::multiplicity::{MultiplicityModel, all_stars_fraction_below_as_drawn};
use hyperion_sim::units::SolarMasses;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's revision: 1, the fit (P15.T4.b, ruling 138).
pub const VERSION: u32 = 1;

/// The census's primaries in bands A (0.08–0.5 M☉), B (0.5–0.75), C (0.75–2.5) and D (2.5–8,
/// white-dwarf progenitors at their initial masses): 1,491 / 288 / 400 / 64 of 2,243 systems with
/// a star of 0.080 M☉ or more or a white dwarf within 20 pc (our tally of Kirkpatrick et al. 2024,
/// Table 4, ruling 138.1).
pub const CENSUS_PRIMARY_COUNTS: [u32; 4] = [1_491, 288, 400, 64];

/// Band D's count under Cummings et al.'s (2018) and El-Badry et al.'s (2018) initial–final mass
/// relations, with C taking or giving the difference (ruling 138.4's report).
pub const IFMR_COUNTS: [(&str, [u32; 4]); 2] = [
    ("Cummings 2018", [1_491, 288, 411, 53]),
    ("El-Badry 2018", [1_491, 288, 393, 71]),
];

/// The mass, M☉, below which the census's primaries lie: it holds no neutron star or black hole.
pub const CENSUS_PRIMARY_MAX: f64 = 8.0;

/// The largest Pearson χ² accepted on 2 degrees of freedom, p = 0.01.
pub const CHI_SQUARE_LIMIT: f64 = 9.21;

/// The largest miss of a band's count, in Poisson errors √(N p).
pub const BAND_MISS_LIMIT: f64 = 2.0;

/// The window on all stars below 0.5 M☉ over systems whose primary lies below 8 M☉: the census's
/// 68.8% less a point, to its 70.0% with Winters et al.'s (2019) M-dwarf companions plus a point.
pub const ALL_STARS_WINDOW: (f64, f64) = (0.678, 0.710);

/// The window on the local mean present-day mass per system in stars and white dwarfs, M☉: the
/// census's 0.548 less its 1σ, up to 0.59 with the hidden white dwarfs of McKee, Parravano and
/// Hollenbach (2015) and 0.60 with the census's missing M-dwarf companions.
pub const LOCAL_MEAN_WINDOW: (f64, f64) = (0.54, 0.60);

/// The census's primaries' mean present mass, M☉ (report only).
pub const CENSUS_PRIMARY_MEAN: f64 = 0.437;

/// The interval of the search for s.
pub const SCALE_SEARCH: (f64, f64) = (0.3, 1.6);

/// Golden-section iterations: the interval's 1.3 shrunk by 0.618 each time, below 10⁻¹³.
pub const SECTIONS: u32 = 72;

/// Bisections of each side of the 1σ interval.
pub const BISECTIONS: u32 = 60;

/// The Sun's galactocentric radius, ly: 8.178 kpc (GRAVITY Collaboration 2019, A&A 625, L10).
pub const SOLAR_RADIUS_LY: f64 = 8.178 * LIGHT_YEARS_PER_KILOPARSEC;

/// Chabrier's function at `scale`.
///
/// # Panics
///
/// If `scale` is not positive and finite.
#[must_use]
fn chabrier(scale: f64) -> Chabrier {
    Chabrier::new(scale).expect("the search's scales are positive")
}

/// The model's shares of primaries in bands A–D over 0.08–8 M☉ at `scale`.
#[must_use]
pub fn primary_shares(scale: f64) -> [f64; 4] {
    let f = chabrier(scale);
    let edges = [0.08, 0.5, 0.75, 2.5, CENSUS_PRIMARY_MAX];
    let total = f.integral(edges[0], edges[4]);
    std::array::from_fn(|k| f.integral(edges[k], edges[k + 1]) / total)
}

/// The multinomial log-likelihood of `counts` at `scale`, up to a constant.
#[must_use]
pub fn log_likelihood(scale: f64, counts: &[u32; 4]) -> f64 {
    primary_shares(scale)
        .iter()
        .zip(counts)
        .fold(0.0, |sum, (&p, &n)| sum + f64::from(n) * math::ln(p))
}

/// The maximum-likelihood scale of `counts` and its 1σ interval (a fall of ½ in the
/// log-likelihood), by [`SECTIONS`] golden sections of [`SCALE_SEARCH`] and [`BISECTIONS`]
/// bisections on each side.
#[must_use]
pub fn fit_scale(counts: &[u32; 4]) -> (f64, (f64, f64)) {
    let ratio = (5.0_f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = SCALE_SEARCH;
    let mut c = b - ratio * (b - a);
    let mut d = a + ratio * (b - a);
    let (mut fc, mut fd) = (log_likelihood(c, counts), log_likelihood(d, counts));
    for _ in 0..SECTIONS {
        if fc > fd {
            b = d;
            d = c;
            fd = fc;
            c = b - ratio * (b - a);
            fc = log_likelihood(c, counts);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + ratio * (b - a);
            fd = log_likelihood(d, counts);
        }
    }
    let best = f64::midpoint(a, b);
    let floor = log_likelihood(best, counts) - 0.5;
    let edge = |mut inside: f64, mut outside: f64| {
        for _ in 0..BISECTIONS {
            let mid = f64::midpoint(inside, outside);
            if log_likelihood(mid, counts) > floor {
                inside = mid;
            } else {
                outside = mid;
            }
        }
        f64::midpoint(inside, outside)
    };
    (
        best,
        (edge(best, SCALE_SEARCH.0), edge(best, SCALE_SEARCH.1)),
    )
}

/// Pearson's χ² of `counts` against the shares at `scale`, and each band's miss in Poisson
/// errors, `(n − N p) ÷ √(N p)`.
#[must_use]
pub fn chi_square(scale: f64, counts: &[u32; 4]) -> (f64, [f64; 4]) {
    let total = f64::from(counts.iter().sum::<u32>());
    let shares = primary_shares(scale);
    let misses: [f64; 4] = std::array::from_fn(|k| {
        let expected = total * shares[k];
        (f64::from(counts[k]) - expected) / expected.sqrt()
    });
    (misses.iter().map(|m| m * m).sum(), misses)
}

/// The share of all stars below 0.5 M☉ over systems whose primary lies below 8 M☉, under
/// Chabrier's function at `scale` with plan 11's drawn companions.
#[must_use]
pub fn all_stars_below(scale: f64) -> f64 {
    let f = chabrier(scale);
    all_stars_fraction_below_as_drawn(
        &Truncated::new(&f, CENSUS_PRIMARY_MAX),
        &MultiplicityModel::default_v1(),
        SolarMasses::new(0.5),
    )
}

/// The local mid-plane means per system of the Milky Way fixture under Chabrier's function at
/// `scale`: each population's mean weighted by its density of systems at R₀ in the plane, over
/// 360 azimuths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalMeans {
    /// Over systems whose primary lies below 8 M☉, in stars and white dwarfs, M☉.
    pub census: f64,
    /// Every system and remnant, the mean the system count uses, M☉.
    pub all: f64,
    /// Every system, in stars and white dwarfs: `all` less this is the neutron stars' and black
    /// holes' mass per system, M☉.
    pub without_compact: f64,
    /// The primary alone, as a star or white dwarf, over primaries below 8 M☉, M☉.
    pub primaries: f64,
}

/// The local means at `scale`.
#[must_use]
pub fn local_means(scale: f64) -> LocalMeans {
    let params = GalaxyParams::milky_way_like();
    let fields = Fields::new(&params, &MassModel::new(&params));
    let f = chabrier(scale);
    let all = params.mean_system_masses_under(&f);
    let census =
        params.census_system_masses_under(&f, Counted::StarsAndWhiteDwarfs, CENSUS_PRIMARY_MAX);
    let without = params.census_system_masses_under(&f, Counted::StarsAndWhiteDwarfs, 150.0);
    let primaries = params.census_system_masses_under(&f, Counted::Primary, CENSUS_PRIMARY_MAX);
    let azimuths = 360_u32;
    let mut sums = [0.0; 4];
    let mut number = 0.0;
    for j in 0..azimuths {
        let theta = 2.0 * PI * (f64::from(j) + 0.5) / f64::from(azimuths);
        let point = PointLy::new(
            SOLAR_RADIUS_LY * math::cos(theta),
            SOLAR_RADIUS_LY * math::sin(theta),
            0.0,
        );
        for p in POPULATIONS {
            let n = fields.population_density(p, &point);
            number += n;
            for (sum, masses) in sums.iter_mut().zip([&census, &all, &without, &primaries]) {
                *sum += n * masses[p.index()].value();
            }
        }
    }
    LocalMeans {
        census: sums[0] / number,
        all: sums[1] / number,
        without_compact: sums[2] / number,
        primaries: sums[3] / number,
    }
}

/// The checks of ruling 138.4 at a fitted scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChabrierChecks {
    /// Pearson χ² of the census's counts.
    pub chi_square: f64,
    /// Each band's miss, in Poisson errors.
    pub misses: [f64; 4],
    /// All stars below 0.5 M☉, over systems whose primary lies below 8 M☉.
    pub all_stars_below: f64,
    /// The local means.
    pub local: LocalMeans,
}

impl ChabrierChecks {
    /// The checks at `scale`.
    #[must_use]
    pub fn at(scale: f64) -> Self {
        let (chi_square, misses) = chi_square(scale, &CENSUS_PRIMARY_COUNTS);
        Self {
            chi_square,
            misses,
            all_stars_below: all_stars_below(scale),
            local: local_means(scale),
        }
    }

    /// Whether each of the three checks passes: the counts, all stars below 0.5 M☉, the local mean.
    #[must_use]
    pub fn passed(&self) -> [bool; 3] {
        [
            self.chi_square <= CHI_SQUARE_LIMIT
                && self.misses.iter().all(|m| m.abs() <= BAND_MISS_LIMIT),
            (ALL_STARS_WINDOW.0..=ALL_STARS_WINDOW.1).contains(&self.all_stars_below),
            (LOCAL_MEAN_WINDOW.0..=LOCAL_MEAN_WINDOW.1).contains(&self.local.census),
        ]
    }
}

/// The table's contents for the scale `scale`.
#[must_use]
pub fn render(scale: f64) -> RustTable {
    RustTable {
        summary: vec![
            "The scale of Chabrier's high-mass branch (plan 15, P15.T4), which `galaxy::imf::Chabrier`"
                .to_owned(),
            "multiplies its power law above 1 M☉ by.".to_owned(),
        ],
        notes: vec![
            "Fitted (P15.T4.b, ruling 138): the multinomial maximum-likelihood scale of the 20 pc"
                .to_owned(),
            "census's primaries in bands A–D, the model's shares renormalised over 0.08–8 M☉."
                .to_owned(),
        ],
        items: vec![TableItem::Scalar {
            name: "HIGH_MASS_BRANCH_SCALE".to_owned(),
            doc: vec![
                "The factor on Chabrier's (2003) power law above 1 M☉, dimensionless: the fit to"
                    .to_owned(),
                "the census's 1,491 / 288 / 400 / 64 primaries in bands A–D (Kirkpatrick et al."
                    .to_owned(),
                "2024, table 4, tallied; ruling 138).".to_owned(),
            ],
            value: scale,
        }],
    }
}

/// The task: P15.T4.b's fit of the scale, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ChabrierTask;

impl FitTask for ChabrierTask {
    fn name(&self) -> &'static str {
        "chabrier"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "chabrier.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["HIGH_MASS_BRANCH_SCALE"]
    }

    /// The model's four band shares at a scale of 0.5, away from 1 so that the scale's own
    /// arithmetic in Chabrier's function shows; the fit reads nothing else of the generator.
    fn fingerprint(&self) -> SimFingerprint {
        let shares = primary_shares(0.5);
        SimFingerprint::new(
            ["A", "B", "C", "D"]
                .iter()
                .zip(shares)
                .map(|(band, share)| (format!("primary_shares(0.5)[{band}]"), share))
                .collect(),
        )
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let counts: Vec<u32> = ["count_a", "count_b", "count_c", "count_d"]
            .iter()
            .map(|key| {
                manifest
                    .u64(key)
                    .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
            })
            .collect::<Result<_, _>>()?;
        if counts != CENSUS_PRIMARY_COUNTS {
            return Err(ManifestParamError::new(
                "count_a–count_d",
                "the census's 1,491 / 288 / 400 / 64, the task's constants",
            )
            .into());
        }
        let (scale, (lo, hi)) = fit_scale(&CENSUS_PRIMARY_COUNTS);
        let checks = ChabrierChecks::at(scale);
        let passed = checks.passed();
        let ifmr: Vec<String> = IFMR_COUNTS
            .iter()
            .map(|(name, counts)| format!("{name} {:.4}", fit_scale(counts).0))
            .collect();
        let verdict = |ok: bool| if ok { "passes" } else { "FAILS" };
        let shares = primary_shares(scale);
        let local = checks.local;
        let acceptance = format!(
            "s = {scale:.4} (1σ {lo:.4}–{hi:.4}; under the IFMRs of {}); over primaries below 8 M☉: \
             band shares A–D {:.1}, {:.1}, {:.1}, {:.2}% against 66.5, 12.8, 17.8, 2.85%, misses of \
             {:+.2}, {:+.2}, {:+.2}, {:+.2} Poisson errors, χ² {:.2} on 2 dof ({}); all stars below \
             0.5 M☉ {:.2}% in 67.8–71.0% ({}); local mid-plane mean per system in stars and white \
             dwarfs {:.4} M☉ in 0.54–0.60 ({}). Reported: the all-inclusive local mean {:.4} M☉, of \
             which neutron stars and black holes {:.4}; the primaries' mean present mass {:.4} M☉ \
             against the census's 0.437",
            ifmr.join(", "),
            100.0 * shares[0],
            100.0 * shares[1],
            100.0 * shares[2],
            100.0 * shares[3],
            checks.misses[0],
            checks.misses[1],
            checks.misses[2],
            checks.misses[3],
            checks.chi_square,
            verdict(passed[0]),
            100.0 * checks.all_stars_below,
            verdict(passed[1]),
            local.census,
            verdict(passed[2]),
            local.all,
            local.all - local.without_compact,
            local.primaries,
        );
        Ok(TaskOutput {
            table: render(scale),
            source:
                "the volume-complete census within 20 pc (Kirkpatrick et al. 2024, ApJS 271, 55, \
                     table 4, tallied; ruling 138); Chabrier (2003, PASP 115, 763); plan 11's \
                     multiplicity model"
                    .to_owned(),
            acceptance,
            // A failed check is the owner's to rule on (ruling 138.5); the fit is not clamped.
            provisional: (!passed.iter().all(|&ok| ok)).then_some("P15.T4.b"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_scale() {
        let table = render(0.9);
        assert_eq!(table.item_names(), ["HIGH_MASS_BRANCH_SCALE"]);
        assert!(
            table
                .body()
                .unwrap()
                .ends_with("pub const HIGH_MASS_BRANCH_SCALE: f64 = 0.9;\n")
        );
    }

    /// The fit maximises the likelihood: nearby scales do worse, the 1σ ends fall by ½, and the
    /// shares sum to 1.
    #[test]
    fn the_fit_maximises_the_likelihood() {
        let counts = CENSUS_PRIMARY_COUNTS;
        let (s, (lo, hi)) = fit_scale(&counts);
        let best = log_likelihood(s, &counts);
        assert!(best >= log_likelihood(s * 1.01, &counts));
        assert!(best >= log_likelihood(s * 0.99, &counts));
        assert!(lo < s && s < hi);
        for edge in [lo, hi] {
            assert!((best - log_likelihood(edge, &counts) - 0.5).abs() < 1e-6);
        }
        assert!((primary_shares(s).iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    /// The committed table is this fit, bit for bit.
    #[test]
    fn the_committed_scale_is_the_fit() {
        let (s, _) = fit_scale(&CENSUS_PRIMARY_COUNTS);
        let committed = hyperion_sim::tables::chabrier::HIGH_MASS_BRANCH_SCALE;
        assert!(s.total_cmp(&committed).is_eq(), "{s} against {committed}");
    }

    /// The checks' verdicts follow their limits.
    #[test]
    fn the_checks_follow_their_limits() {
        let local = LocalMeans {
            census: 0.55,
            all: 0.6,
            without_compact: 0.56,
            primaries: 0.44,
        };
        let checks = |chi_square: f64, misses: [f64; 4], below: f64, census: f64| ChabrierChecks {
            chi_square,
            misses,
            all_stars_below: below,
            local: LocalMeans { census, ..local },
        };
        assert_eq!(
            checks(3.0, [1.0, -1.5, 0.2, 1.9], 0.69, 0.55).passed(),
            [true; 3]
        );
        assert_eq!(
            checks(3.0, [1.0, -2.5, 0.2, 1.0], 0.72, 0.61).passed(),
            [false; 3]
        );
        assert_eq!(
            checks(9.5, [0.0; 4], 0.69, 0.55).passed(),
            [false, true, true]
        );
    }
}
