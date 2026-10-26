//! The kick law's defaults review (plan 15, P15.T5.b): the observables of plan 06's kick-law tests
//! (P06.T19.d) on a grid of each of the law's four defaults about its value, one at a time, the
//! rest held, and the interval of each default over which each observable keeps passing.
//!
//! The four defaults are `hyperion_sim::tables::kick_rank`'s ([`KickDefault`]): the ramp over
//! carbon–oxygen cores of 2–3 M☉ across which a companion-stripped progenitor's chance of the low
//! kick mode falls from 1 to 0, the black holes' factor of 0.75 on the ordinary kick, and the
//! electron-capture windows' widths, 0.1 M☉ for a single star and 1.0 M☉ for a companion-stripped
//! one. The table `tables::kick_defaults` is what [`KickDefaultsTask`] writes; no generator code
//! reads it, and no default moves here (decision-p15-revalidation Q2 and Q3).
//!
//! # The harness
//!
//! At each grid point the task calls the sim's `kick_observables` on the slow tests' own sample,
//! 20,000 stars of seed `0x0619d00000000001`, with `KickLawParams::default()` with the one default
//! moved ([`KickDefault::params_at`]), and a rank table rebuilt at that point's membership: the
//! scores of [`ReferencePopulation`] of the same parameters, by [`scores_parallel`] on the committed
//! table's seed `0x0619b00000000000`, through [`quantiles_of`] (decision-p15-revalidation Q1). The
//! production run takes 10⁵ scores, whose knots' rank errors, about 0.0016, are no larger than
//! the observables' own binomial errors at 20,000 stars. The ramp's two ends move together, its
//! width held. The black-hole factor's points use the centre's rebuild, since the membership never
//! reads the factor; the windows' points are rebuilt anyway, and the table's notes say whether each
//! rebuild equals the centre's. The centre is also evaluated on the committed table,
//! `KickRankTable::generator()`, which gives the slow tests' own figures: the difference between
//! the two is the harness's resolution, which the header states.
//!
//! # The observables
//!
//! Each criterion of the seven slow tests of `crates/hyperion-sim/tests/stellar_kick.rs` is one
//! observable, judged by the test's own criterion ([`Observable`]). Two are held at measured
//! values by ruling 147.2 ([`Held`]): the isolated pulsars' share under 50 km/s on the sky and
//! the neutron stars' escape share. They have no interval; the table reports their values along
//! each grid and whether each lies inside its literature window. The test's K–S statistic is
//! computed here ([`ks_one_sample`]), as `hyperion_testkit::stats::ks_one_sample` computes it,
//! since the testkit is not a dependency of this crate's library.
//!
//! # Intervals and verdicts
//!
//! An observable's interval is the run of grid points about the default over which it passes,
//! contiguous, its ends the outermost passing points ([`interval_of`]); its resolution is one grid
//! step. An end at the grid's first or last point is open: no edge was found there. A default at a
//! closed end, or within a tenth of the interval's width of one, is a finding ([`verdict_of`]),
//! recorded with the table and not acted on.
//!
//! # Cost
//!
//! Each score and each sample star costs a full track of a massive star, about half a millisecond
//! in release. The production run, 31 rebuilds and 40 samples, is about 50 CPU-minutes, so the task
//! is slow; its tests run the smoke manifest. Each rebuild runs on every thread through
//! [`scores_parallel`], and the samples run in parallel through [`map_reduce_chunks`], joined in
//! grid order, so that the table is the same bytes on any thread count.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::Seed;
use hyperion_sim::math;
use hyperion_sim::stellar::remnant::reference::{
    KickObservables, QUANTILES, ReferencePopulation, kick_observables, quantiles_of,
};
use hyperion_sim::stellar::remnant::{KickLawParams, KickRankTable};
use hyperion_sim::tables::kick_rank;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};
use crate::tasks::kick_rank::{KickRankTask, scores_parallel};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The K–S test's level: plan 01's α, `hyperion_testkit::stats::ALPHA`, which the slow test
/// asserts the p-value against.
pub const ALPHA: f64 = 1e-3;

/// The share of an interval's width within which a default lies near its edge: a tenth (P15.T5.b's
/// acceptance).
pub const NEAR_EDGE: f64 = 0.1;

/// One of the kick law's four defaults that the review varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KickDefault {
    /// The low mode's ramp over carbon–oxygen cores, M☉, `kick_rank::LOW_RAMP`, (2.0, 3.0).
    ///
    /// Its grid is of the lower end, the upper end moving with it at the committed width.
    LowRamp,
    /// The factor on a black hole's ordinary kick, `kick_rank::BH_FACTOR`, 0.75.
    BhFactor,
    /// The single star's electron-capture window, M☉ of the mass `m_c_bagb` reads,
    /// `kick_rank::EC_WINDOW_SINGLE`, 0.1.
    EcWindowSingle,
    /// A companion-stripped star's window, M☉ of the same mass, `kick_rank::EC_WINDOW_STRIPPED`,
    /// 1.0.
    EcWindowStripped,
}

impl KickDefault {
    /// The four, in the table's order.
    pub const ALL: [Self; 4] = [
        Self::LowRamp,
        Self::BhFactor,
        Self::EcWindowSingle,
        Self::EcWindowStripped,
    ];

    /// The manifest's key of its grid.
    #[must_use]
    pub const fn grid_key(self) -> &'static str {
        match self {
            Self::LowRamp => "low_ramp_lower_grid",
            Self::BhFactor => "bh_factor_grid",
            Self::EcWindowSingle => "ec_window_single_grid",
            Self::EcWindowStripped => "ec_window_stripped_grid",
        }
    }

    /// The prefix of its items' names in the table.
    #[must_use]
    pub const fn item_prefix(self) -> &'static str {
        match self {
            Self::LowRamp => "LOW_RAMP",
            Self::BhFactor => "BH_FACTOR",
            Self::EcWindowSingle => "EC_WINDOW_SINGLE",
            Self::EcWindowStripped => "EC_WINDOW_STRIPPED",
        }
    }

    /// What it is, for the table's text.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LowRamp => "the low-mode ramp",
            Self::BhFactor => "the black-hole factor",
            Self::EcWindowSingle => "the single-star window",
            Self::EcWindowStripped => "the companion-stripped window",
        }
    }

    /// Its grid's unit, for the table's text.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::LowRamp => "M☉ of carbon–oxygen core",
            Self::BhFactor => "a factor on the ordinary kick",
            Self::EcWindowSingle | Self::EcWindowStripped => "M☉ of the mass `m_c_bagb` reads",
        }
    }

    /// Its committed value on its grid: the ramp's lower end, the factor, or the window's width.
    #[must_use]
    pub const fn committed(self) -> f64 {
        match self {
            Self::LowRamp => kick_rank::LOW_RAMP.0,
            Self::BhFactor => kick_rank::BH_FACTOR,
            Self::EcWindowSingle => kick_rank::EC_WINDOW_SINGLE,
            Self::EcWindowStripped => kick_rank::EC_WINDOW_STRIPPED,
        }
    }

    /// The law's parameters with this default at `value` on its grid, the rest held.
    ///
    /// For the ramp, the lower end is at `value` and the upper at `value` plus the committed
    /// width. At [`committed`](Self::committed) they are `KickLawParams::default()`.
    #[must_use]
    pub fn params_at(self, value: f64) -> KickLawParams {
        let p = KickLawParams::default();
        match self {
            Self::LowRamp => KickLawParams {
                low_ramp: (value, value + (p.low_ramp.1 - p.low_ramp.0)),
                ..p
            },
            Self::BhFactor => KickLawParams {
                bh_factor: value,
                ..p
            },
            Self::EcWindowSingle => KickLawParams {
                ec_window_single: value,
                ..p
            },
            Self::EcWindowStripped => KickLawParams {
                ec_window_stripped: value,
                ..p
            },
        }
    }

    /// Whether a point of its grid rebuilds the rank table: its membership reads the ramp, and
    /// its tracks the windows, but nothing of the table reads the black-hole factor.
    #[must_use]
    pub const fn rebuilds(self) -> bool {
        match self {
            Self::LowRamp | Self::EcWindowSingle | Self::EcWindowStripped => true,
            Self::BhFactor => false,
        }
    }

    /// Whether every point of its grid must be positive: a window's width must be.
    #[must_use]
    pub const fn positive(self) -> bool {
        match self {
            Self::EcWindowSingle | Self::EcWindowStripped => true,
            Self::LowRamp | Self::BhFactor => false,
        }
    }
}

/// How an observable passes: the slow test's own comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Criterion {
    /// |x − centre| ≤ half, as the test writes it.
    Within {
        /// The centre.
        centre: f64,
        /// The half-width.
        half: f64,
    },
    /// lower ≤ x ≤ upper.
    Between {
        /// The least passing value.
        lower: f64,
        /// The greatest.
        upper: f64,
    },
    /// x ≥ lower; for a share or a probability, so at most 1.
    AtLeast {
        /// The least passing value.
        lower: f64,
    },
}

impl Criterion {
    /// Whether `x` passes.
    #[must_use]
    pub fn holds(self, x: f64) -> bool {
        match self {
            Self::Within { centre, half } => (x - centre).abs() <= half,
            Self::Between { lower, upper } => (lower..=upper).contains(&x),
            Self::AtLeast { lower } => x >= lower,
        }
    }
}

/// An observable of the kick-law tests that passes at the committed defaults: one criterion of one
/// of the seven slow tests of `crates/hyperion-sim/tests/stellar_kick.rs` (P06.T19.d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Observable {
    /// Test 1: the mean of ln(v ÷ km/s) of the ordinary-mode neutron stars, 5.60 ± 0.12 (Disberg
    /// and Mandel 2025).
    OrdinaryLnMean,
    /// Test 1: its standard deviation, 0.68 ± 0.10.
    OrdinaryLnSd,
    /// Test 1: the K–S p-value against the log-normal truncated at 1,000 km/s, at least α.
    OrdinaryKs,
    /// Test 2: the isolated pulsars' share under 100 km/s, 7–13%, between Disberg and Mandel's
    /// (2025) 7% and Igoshev's (2020) 13% (ruling 96.3).
    IsolatedUnder100,
    /// Test 3: the low mode's share of neutron stars, 10–30% (Igoshev et al. 2021).
    LowModeShare,
    /// Test 4: the share retained under 20 km/s, 18–30%, and within 0.002 of the low mode's
    /// (ruling 147.3, after ruling 137.3; ruling 96.3).
    RetainedUnder20,
    /// Test 4: under 50 km/s, 18–31% (ruling 147.3).
    RetainedUnder50,
    /// Test 4: under 100 km/s, 24–35% (ruling 147.3).
    RetainedUnder100,
    /// Test 5: the toy's surviving double neutron stars with e < 0.3, 50–90%, about Grichener et
    /// al.'s (2026) 13 of 21 observed (ruling 96.3); the toy is Hills's (1983) and Brandt and
    /// Podsiadlowski's (1995).
    CircularDoubleNeutronStars,
    /// Test 6: the black holes with no kick, at least half.
    BlackHolesUnkicked,
    /// Test 6: the black holes under 12 M☉ with no kick, 30–70%, the 90% binomial range on
    /// Nagarajan and El-Badry's (2025) 5 of 11 (ruling 96.3).
    LightBlackHolesUnkicked,
    /// Test 6: those above 100 km/s, 12–50%, the same range on their 4 of 11.
    LightBlackHolesFast,
}

impl Observable {
    /// The twelve, in the table's order.
    pub const ALL: [Self; 12] = [
        Self::OrdinaryLnMean,
        Self::OrdinaryLnSd,
        Self::OrdinaryKs,
        Self::IsolatedUnder100,
        Self::LowModeShare,
        Self::RetainedUnder20,
        Self::RetainedUnder50,
        Self::RetainedUnder100,
        Self::CircularDoubleNeutronStars,
        Self::BlackHolesUnkicked,
        Self::LightBlackHolesUnkicked,
        Self::LightBlackHolesFast,
    ];

    /// Its index in [`ALL`](Self::ALL) and in a [`Figures`]' values.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The slow test it comes from.
    #[must_use]
    pub const fn test(self) -> &'static str {
        match self {
            Self::OrdinaryLnMean | Self::OrdinaryLnSd | Self::OrdinaryKs => {
                "ordinary_neutron_star_speeds_follow_the_log_normal"
            }
            Self::IsolatedUnder100 => "isolated_pulsars_are_rarely_slow",
            Self::LowModeShare => "the_low_mode_is_a_fifth_of_neutron_stars",
            Self::RetainedUnder20 | Self::RetainedUnder50 | Self::RetainedUnder100 => {
                "a_fifth_to_a_third_of_neutron_stars_is_retained_by_a_cluster"
            }
            Self::CircularDoubleNeutronStars => "most_double_neutron_stars_are_nearly_circular",
            Self::BlackHolesUnkicked
            | Self::LightBlackHolesUnkicked
            | Self::LightBlackHolesFast => "most_black_holes_are_unkicked",
        }
    }

    /// What it measures, for the table's text.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OrdinaryLnMean => "ordinary ln v mean",
            Self::OrdinaryLnSd => "ordinary ln v sd",
            Self::OrdinaryKs => "ordinary K–S p",
            Self::IsolatedUnder100 => "isolated pulsars under 100 km/s",
            Self::LowModeShare => "low-mode share",
            Self::RetainedUnder20 => "retained under 20 km/s",
            Self::RetainedUnder50 => "retained under 50 km/s",
            Self::RetainedUnder100 => "retained under 100 km/s",
            Self::CircularDoubleNeutronStars => "double neutron stars with e < 0.3",
            Self::BlackHolesUnkicked => "black holes unkicked",
            Self::LightBlackHolesUnkicked => "light black holes unkicked",
            Self::LightBlackHolesFast => "light black holes above 100 km/s",
        }
    }

    /// The test's criterion.
    ///
    /// [`Observable::RetainedUnder20`] also asks to lie within [`LOW_MODE_MATCH`] of the low
    /// mode's share ([`Figures::passes`]).
    #[must_use]
    pub const fn criterion(self) -> Criterion {
        match self {
            Self::OrdinaryLnMean => Criterion::Within {
                centre: 5.60,
                half: 0.12,
            },
            Self::OrdinaryLnSd => Criterion::Within {
                centre: 0.68,
                half: 0.10,
            },
            Self::OrdinaryKs => Criterion::AtLeast { lower: ALPHA },
            Self::IsolatedUnder100 => Criterion::Between {
                lower: 0.07,
                upper: 0.13,
            },
            Self::LowModeShare => Criterion::Between {
                lower: 0.10,
                upper: 0.30,
            },
            Self::RetainedUnder20 => Criterion::Between {
                lower: 0.18,
                upper: 0.30,
            },
            Self::RetainedUnder50 => Criterion::Between {
                lower: 0.18,
                upper: 0.31,
            },
            Self::RetainedUnder100 => Criterion::Between {
                lower: 0.24,
                upper: 0.35,
            },
            Self::CircularDoubleNeutronStars => Criterion::Between {
                lower: 0.5,
                upper: 0.9,
            },
            Self::BlackHolesUnkicked => Criterion::AtLeast { lower: 0.5 },
            Self::LightBlackHolesUnkicked => Criterion::Between {
                lower: 0.30,
                upper: 0.70,
            },
            Self::LightBlackHolesFast => Criterion::Between {
                lower: 0.12,
                upper: 0.50,
            },
        }
    }

    /// The bounds the table states for the criterion, as decimals: centre ∓ half for
    /// [`Criterion::Within`], and 1 above a lower bound alone, every such observable being a share
    /// or a probability.
    #[must_use]
    pub const fn bounds(self) -> (f64, f64) {
        match self {
            Self::OrdinaryLnMean => (5.48, 5.72),
            Self::OrdinaryLnSd => (0.58, 0.78),
            Self::OrdinaryKs
            | Self::IsolatedUnder100
            | Self::LowModeShare
            | Self::RetainedUnder20
            | Self::RetainedUnder50
            | Self::RetainedUnder100
            | Self::CircularDoubleNeutronStars
            | Self::BlackHolesUnkicked
            | Self::LightBlackHolesUnkicked
            | Self::LightBlackHolesFast => match self.criterion() {
                Criterion::Between { lower, upper } => (lower, upper),
                Criterion::AtLeast { lower } => (lower, 1.0),
                Criterion::Within { centre, half } => (centre - half, centre + half),
            },
        }
    }
}

/// How close the share retained under 20 km/s must lie to the low mode's: 0.002 (ruling 96.3).
pub const LOW_MODE_MATCH: f64 = 0.002;

/// An observable that ruling 147.2 holds at its measured value, outside its literature window,
/// as an accepted finding (decision-p15-revalidation Q2): it has no interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Held {
    /// Test 2: the isolated pulsars' share under 50 km/s on the sky, 0.0708, against 3–7%
    /// (Willcox et al. 2021).
    SkyUnder50,
    /// The neutron stars' share above the escape speed with the rotation, 0.1159, against 12–20%
    /// (ruling 96.3).
    EscapeShare,
}

impl Held {
    /// The two, in the table's order.
    pub const ALL: [Self; 2] = [Self::SkyUnder50, Self::EscapeShare];

    /// Its index in [`ALL`](Self::ALL) and in a [`Figures`]' held values.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The slow test that pins it.
    #[must_use]
    pub const fn test(self) -> &'static str {
        match self {
            Self::SkyUnder50 => "isolated_pulsars_are_rarely_slow",
            Self::EscapeShare => "about_a_sixth_of_neutron_stars_leave_the_disc",
        }
    }

    /// What it measures, for the table's text.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SkyUnder50 => "sky share under 50 km/s",
            Self::EscapeShare => "escape share",
        }
    }

    /// Its literature window, which the test reports.
    #[must_use]
    pub const fn window(self) -> (f64, f64) {
        match self {
            Self::SkyUnder50 => (0.03, 0.07),
            Self::EscapeShare => (0.12, 0.20),
        }
    }

    /// The value ruling 147.2 measured and the test pins to ±0.002.
    #[must_use]
    pub const fn measured(self) -> f64 {
        match self {
            Self::SkyUnder50 => 0.0708,
            Self::EscapeShare => 0.1159,
        }
    }

    /// Whether `x` lies inside its literature window, ends included.
    #[must_use]
    pub fn inside(self, x: f64) -> bool {
        let (lower, upper) = self.window();
        (lower..=upper).contains(&x)
    }
}

/// The observables measured on one sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Figures {
    /// The values of [`Observable::ALL`], in order.
    pub values: [f64; Observable::ALL.len()],
    /// The values of [`Held::ALL`], in order.
    pub held: [f64; Held::ALL.len()],
}

impl Figures {
    /// The figures of `o`, measured under `params`, whose ordinary mode's log-normal the K–S test
    /// compares with.
    ///
    /// # Panics
    ///
    /// If `o` holds no ordinary-mode speed, which `kick_observables` rules out.
    #[must_use]
    pub fn of(mut o: KickObservables, params: &KickLawParams) -> Self {
        let (mean, sd) = o.ordinary_ln_moments;
        let [under_20, under_50, under_100] = o.retention;
        Self {
            values: [
                mean,
                sd,
                ordinary_ks(&mut o.ordinary_ln_speeds, params).p_value,
                o.isolated_under_100_share,
                o.low_mode_share,
                under_20,
                under_50,
                under_100,
                o.double_neutron_stars.0,
                o.black_holes_unkicked,
                o.light_black_holes.0,
                o.light_black_holes.1,
            ],
            held: [o.isolated_slow_share, o.escape_share],
        }
    }

    /// The value of `observable`.
    #[must_use]
    pub const fn value(&self, observable: Observable) -> f64 {
        self.values[observable.index()]
    }

    /// Whether `observable` passes its test's criterion.
    #[must_use]
    pub fn passes(&self, observable: Observable) -> bool {
        let x = self.value(observable);
        let matched = match observable {
            Observable::RetainedUnder20 => {
                (x - self.value(Observable::LowModeShare)).abs() <= LOW_MODE_MATCH
            }
            Observable::OrdinaryLnMean
            | Observable::OrdinaryLnSd
            | Observable::OrdinaryKs
            | Observable::IsolatedUnder100
            | Observable::LowModeShare
            | Observable::RetainedUnder50
            | Observable::RetainedUnder100
            | Observable::CircularDoubleNeutronStars
            | Observable::BlackHolesUnkicked
            | Observable::LightBlackHolesUnkicked
            | Observable::LightBlackHolesFast => true,
        };
        observable.criterion().holds(x) && matched
    }

    /// The observables that fail.
    #[must_use]
    pub fn failing(&self) -> Vec<Observable> {
        Observable::ALL
            .into_iter()
            .filter(|&o| !self.passes(o))
            .collect()
    }
}

/// A one-sample Kolmogorov–Smirnov test's distance and p-value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KsTest {
    /// The largest distance between the sample's distribution and the model's.
    pub statistic: f64,
    /// The probability of a distance at least as large under the model.
    pub p_value: f64,
}

/// The one-sample Kolmogorov–Smirnov test of `sample` against the distribution function `cdf`, as
/// `hyperion_testkit::stats::ks_one_sample` computes it, sorting `sample` in place.
///
/// D is the largest distance between the sample's distribution, sorted with `total_cmp`, and
/// `cdf`, and the p-value is Kolmogorov's survival function at Stephens's λ = (√n + 0.12 + 0.11 ÷
/// √n) D (Stephens 1970, J. R. Stat. Soc. B 32, 115).
///
/// # Panics
///
/// If `sample` is empty or holds a NaN, or `cdf` returns a value outside [0, 1].
#[must_use]
pub fn ks_one_sample(sample: &mut [f64], cdf: impl Fn(f64) -> f64) -> KsTest {
    assert!(!sample.is_empty(), "a K–S test needs a sample");
    assert!(
        sample.iter().all(|x| !x.is_nan()),
        "a K–S test needs ordered values"
    );
    sample.sort_by(f64::total_cmp);
    let n = count_f64(sample.len());
    let mut d: f64 = 0.0;
    for (i, &x) in sample.iter().enumerate() {
        let rank = count_f64(i);
        let f = cdf(x);
        assert!(
            (0.0..=1.0).contains(&f),
            "cdf({x}) = {f} is not a probability"
        );
        d = d
            .max(((rank + 1.0) / n - f).abs())
            .max((f - rank / n).abs());
    }
    let root = n.sqrt();
    KsTest {
        statistic: d,
        p_value: kolmogorov_survival((root + 0.12 + 0.11 / root) * d),
    }
}

/// Kolmogorov's survival function, Q(λ) = 2 Σ_{j ≥ 1} (−1)^(j−1) exp(−2 j² λ²), summed until a
/// term falls below 10⁻¹², held to [0, 1]; 1 at λ = 0.
///
/// # Panics
///
/// If `lambda` is negative or NaN.
#[must_use]
fn kolmogorov_survival(lambda: f64) -> f64 {
    assert!(lambda >= 0.0, "λ = {lambda} must be non-negative");
    if lambda <= 0.0 {
        return 1.0;
    }
    let (mut sum, mut sign, mut j) = (0.0, 1.0, 1.0);
    loop {
        let term = math::exp(-2.0 * j * j * lambda * lambda);
        sum += sign * term;
        if term < 1e-12 {
            break;
        }
        sign = -sign;
        j += 1.0;
    }
    (2.0 * sum).clamp(0.0, 1.0)
}

/// Test 1's K–S test of the ordinary-mode neutron stars' ln(v ÷ km/s), `ln_speeds`, against the
/// log-normal of `params` truncated at its maximum speed, Φ(z) ÷ Φ(b), as the slow test writes it.
///
/// # Panics
///
/// As [`ks_one_sample`]: if `ln_speeds` is empty or holds a NaN.
#[must_use]
pub fn ordinary_ks(ln_speeds: &mut [f64], params: &KickLawParams) -> KsTest {
    let normal_cdf = |z: f64| 0.5 * math::erfc(-z * core::f64::consts::FRAC_1_SQRT_2);
    let below = normal_cdf((math::ln(params.max_speed_km_s) - params.ln_mu) / params.ln_sigma);
    ks_one_sample(ln_speeds, |x| {
        (normal_cdf((x - params.ln_mu) / params.ln_sigma) / below).clamp(0.0, 1.0)
    })
}

/// A count as a float.
#[must_use]
fn count_f64(n: usize) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a sample's count is far below 2^53"
    )]
    let n = n as f64;
    n
}

/// Where a default lies in an observable's interval (P15.T5.b's acceptance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Verdict {
    /// Inside, and not within a tenth of the interval's width of a closed end.
    Inside,
    /// Within a tenth of the interval's width of a closed end, but not at it: a finding.
    NearEdge,
    /// At a closed end, the next grid point failing: a finding.
    AtEdge,
    /// The observable fails at the default's own point: a finding, with no interval.
    FailsAtDefault,
}

impl Verdict {
    /// Whether the verdict is a finding.
    #[must_use]
    pub const fn is_finding(self) -> bool {
        match self {
            Self::Inside => false,
            Self::NearEdge | Self::AtEdge | Self::FailsAtDefault => true,
        }
    }

    /// The verdict in words, for the table's text.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Inside => "inside",
            Self::NearEdge => "within a tenth of an edge",
            Self::AtEdge => "at an edge",
            Self::FailsAtDefault => "fails at the default",
        }
    }
}

/// The interval about `centre` over which `passes` holds, as the indices of its first and last
/// points, or `None` if the centre fails or lies outside `passes`: the contiguous run of passing
/// points that holds it.
#[must_use]
pub fn interval_of(passes: &[bool], centre: usize) -> Option<(usize, usize)> {
    if !passes.get(centre).copied().unwrap_or(false) {
        return None;
    }
    let lower = passes[..centre]
        .iter()
        .rposition(|&p| !p)
        .map_or(0, |failing| failing + 1);
    let upper = passes[centre..]
        .iter()
        .position(|&p| !p)
        .map_or(passes.len() - 1, |failing| centre + failing - 1);
    Some((lower, upper))
}

/// Where the default at `points[centre]` lies in `interval` ([`interval_of`]) of the increasing
/// grid `points`.
///
/// An end at the grid's first or last point is open, and is never an edge.
///
/// # Panics
///
/// If `points` is empty, or `centre` or an end of `interval` is not an index of `points`.
#[must_use]
pub fn verdict_of(points: &[f64], centre: usize, interval: Option<(usize, usize)>) -> Verdict {
    let Some((lower, upper)) = interval else {
        return Verdict::FailsAtDefault;
    };
    let last = points.len() - 1;
    let width = points[upper] - points[lower];
    let mut verdict = Verdict::Inside;
    for (end, closed) in [(lower, lower > 0), (upper, upper < last)] {
        if !closed {
            continue;
        }
        if end == centre {
            return Verdict::AtEdge;
        }
        if (points[centre] - points[end]).abs() <= NEAR_EDGE * width {
            verdict = Verdict::NearEdge;
        }
    }
    verdict
}

/// One default's sweep: its grid, the default's place on it, and the figures at each point.
#[derive(Debug, Clone, PartialEq)]
pub struct Sweep {
    default: KickDefault,
    points: Vec<f64>,
    centre: usize,
    figures: Vec<Figures>,
}

impl Sweep {
    /// The sweep of `default` over the grid `points` with `figures` at each, the centre's on the
    /// centre's rebuild, or `None` unless the grid is increasing and holds the committed value
    /// exactly and the two have one length.
    #[must_use]
    pub fn new(default: KickDefault, points: Vec<f64>, figures: Vec<Figures>) -> Option<Self> {
        let centre = points
            .iter()
            .position(|p| p.total_cmp(&default.committed()).is_eq())?;
        let increasing = points.windows(2).all(|w| w[0] < w[1]);
        (increasing && points.len() == figures.len()).then_some(Self {
            default,
            points,
            centre,
            figures,
        })
    }

    /// The default swept.
    #[must_use]
    pub const fn default(&self) -> KickDefault {
        self.default
    }

    /// Its grid, increasing, holding the committed value.
    #[must_use]
    pub fn points(&self) -> &[f64] {
        &self.points
    }

    /// The committed value's index in [`points`](Self::points).
    #[must_use]
    pub const fn centre(&self) -> usize {
        self.centre
    }

    /// The figures at each point.
    #[must_use]
    pub fn figures(&self) -> &[Figures] {
        &self.figures
    }

    /// The interval of `observable` ([`interval_of`]).
    #[must_use]
    pub fn interval(&self, observable: Observable) -> Option<(usize, usize)> {
        let passes: Vec<bool> = self.figures.iter().map(|f| f.passes(observable)).collect();
        interval_of(&passes, self.centre)
    }

    /// The verdict of `observable` ([`verdict_of`]).
    #[must_use]
    pub fn verdict(&self, observable: Observable) -> Verdict {
        verdict_of(&self.points, self.centre, self.interval(observable))
    }

    /// Whether the interval of `observable` is open at both ends: it passes over the whole grid,
    /// and no edge was found.
    #[must_use]
    pub fn open_at_both_ends(&self, observable: Observable) -> bool {
        self.interval(observable)
            .is_some_and(|(lower, upper)| lower == 0 && upper == self.points.len() - 1)
    }
}

/// What the table's body holds: the committed defaults' figures and the four sweeps.
#[derive(Debug, Clone, PartialEq)]
pub struct Sensitivity {
    /// The figures at the committed defaults on the committed rank table: the slow tests' own.
    pub committed: Figures,
    /// The figures at the committed defaults on the centre's rebuild.
    pub rebuild: Figures,
    /// The sweeps, in [`KickDefault::ALL`]'s order.
    pub sweeps: [Sweep; KickDefault::ALL.len()],
}

impl Sensitivity {
    /// The harness's resolution: the largest differences between the centre on its rebuild and
    /// on the committed table, of the two ln-speed figures and of the shares, held ones included,
    /// the K–S p-value aside.
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        let largest =
            |figures: &mut dyn Iterator<Item = (&'static str, f64, f64)>| {
                figures.map(|(label, a, b)| ((a - b).abs(), label)).fold(
                    (0.0, "none"),
                    |best, next| {
                        if next.0 > best.0 { next } else { best }
                    },
                )
            };
        let of = |o: Observable| (o.label(), self.committed.value(o), self.rebuild.value(o));
        let ln_speed = largest(
            &mut [Observable::OrdinaryLnMean, Observable::OrdinaryLnSd]
                .into_iter()
                .map(of),
        );
        let held = Held::ALL.into_iter().map(|h| {
            (
                h.label(),
                self.committed.held[h.index()],
                self.rebuild.held[h.index()],
            )
        });
        // In `ALL` the shares follow the two moments and the K–S p-value.
        let share = largest(
            &mut Observable::ALL[Observable::IsolatedUnder100.index()..]
                .iter()
                .map(|&o| of(o))
                .chain(held),
        );
        Resolution { ln_speed, share }
    }
}

/// The harness's resolution ([`Sensitivity::resolution`]): each largest difference with the label
/// of the figure it is of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolution {
    /// Of the ordinary ln v mean and sd, in ln(v ÷ km/s).
    pub ln_speed: (f64, &'static str),
    /// Of the shares, the two held ones included.
    pub share: (f64, &'static str),
}

/// The run's parameters, from its manifest ([`Settings::from_manifest`], which checks them).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    rebuild_scores: u64,
    rebuild_seed: Seed,
    stars: u64,
    stars_seed: Seed,
    grids: [Vec<f64>; KickDefault::ALL.len()],
}

impl Settings {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] for a count or seed that is missing or not a non-negative integer,
    /// a score count below 2 or a star count of 0, or a grid that is not an increasing array of
    /// finite numbers holding its default's committed value, with only positive widths for a
    /// window.
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let rebuild_scores = manifest.u64("rebuild_scores")?;
        if rebuild_scores < 2 {
            return Err(ManifestParamError::new("rebuild_scores", "at least 2"));
        }
        let stars = manifest.u64("stars")?;
        if stars == 0 {
            return Err(ManifestParamError::new("stars", "at least 1"));
        }
        let [ramp, factor, single, stripped] = KickDefault::ALL;
        Ok(Self {
            rebuild_scores,
            rebuild_seed: Seed::new(manifest.u64("rebuild_seed")?),
            stars,
            stars_seed: Seed::new(manifest.u64("stars_seed")?),
            grids: [
                grid(manifest, ramp)?,
                grid(manifest, factor)?,
                grid(manifest, single)?,
                grid(manifest, stripped)?,
            ],
        })
    }
}

/// The grid of `default` in `manifest`.
fn grid(manifest: &Manifest, default: KickDefault) -> Result<Vec<f64>, ManifestParamError> {
    let key = default.grid_key();
    let committed = default.committed();
    let positive = default.positive();
    let expected = format!(
        "an increasing array of finite numbers{} holding the committed {committed}",
        if positive { " above 0" } else { "" }
    );
    let bad = || ManifestParamError::new(key, &expected);
    let points: Vec<f64> = manifest
        .params()
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(bad)?
        .iter()
        .map(|v| match v {
            toml::Value::Float(x) => Some(*x),
            toml::Value::Integer(i) => i32::try_from(*i).ok().map(f64::from),
            _ => None,
        })
        .collect::<Option<_>>()
        .ok_or_else(bad)?;
    let valid = points
        .iter()
        .all(|x| x.is_finite() && (!positive || *x > 0.0))
        && points.windows(2).all(|w| w[0] < w[1])
        && points.iter().any(|x| x.total_cmp(&committed).is_eq());
    if valid { Ok(points) } else { Err(bad()) }
}

/// The review: its settings, what each window's rebuild found, and the sensitivity table.
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    /// The run's parameters.
    pub settings: Settings,
    /// Each grid point that rebuilt the rank table, in the order rebuilt.
    pub rebuilds: Vec<RebuildCheck>,
    /// The figures.
    pub sensitivity: Sensitivity,
}

/// One grid point's rebuild of the rank table against the centre's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RebuildCheck {
    /// The default moved.
    pub default: KickDefault,
    /// Its value at the point.
    pub value: f64,
    /// Whether the rebuild's quantiles equal the centre's bit for bit.
    pub equals_centre: bool,
}

/// One evaluation: the law's parameters and the index of its rank table.
type Job = (KickLawParams, usize);

/// The rank table's quantiles rebuilt at the membership of `params`: [`scores_parallel`] of
/// `scores` scores of seed `seed` over [`ReferencePopulation`] of `params`, then
/// [`quantiles_of`], which is `score_quantiles` bit for bit.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If the windows of `params` are not finite and positive, which [`Settings::from_manifest`]
/// rules out for every grid point, or `scores` is below 2.
fn rebuild(
    params: &KickLawParams,
    scores: u64,
    seed: Seed,
    threads: NonZeroUsize,
) -> Result<[f64; QUANTILES], BuildThreadPoolError> {
    let population = ReferencePopulation::new(*params)
        .expect("Settings::from_manifest admits only finite, positive windows");
    let mut drawn = scores_parallel(&population, scores, seed, threads)?;
    Ok(quantiles_of(&mut drawn))
}

/// The review of `settings` on `threads` threads: the rebuilds one after another, each on every
/// thread, then every evaluation in parallel, joined in grid order.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If a grid point's sample holds no ordinary-mode neutron star, which `kick_observables`
/// refuses: a star count far below the production run's.
pub fn review(settings: Settings, threads: NonZeroUsize) -> Result<Review, BuildThreadPoolError> {
    let centre = KickLawParams::default();
    let centre_quantiles = rebuild(
        &centre,
        settings.rebuild_scores,
        settings.rebuild_seed,
        threads,
    )?;
    let table = |q: &[f64; QUANTILES]| {
        KickRankTable::new(q.to_vec()).expect("quantiles_of returns strictly increasing quantiles")
    };
    let mut tables = vec![KickRankTable::generator(), table(&centre_quantiles)];
    let mut jobs: Vec<Job> = vec![(centre, 0), (centre, 1)];
    let mut rebuilds = Vec::new();
    // The job of each grid point, the centre's being job 1.
    let mut slots: Vec<Vec<usize>> = Vec::new();
    for (default, points) in KickDefault::ALL.into_iter().zip(&settings.grids) {
        let mut slot = Vec::with_capacity(points.len());
        for &value in points {
            if value.total_cmp(&default.committed()).is_eq() {
                slot.push(1);
                continue;
            }
            let params = default.params_at(value);
            let table_index = if default.rebuilds() {
                let q = rebuild(
                    &params,
                    settings.rebuild_scores,
                    settings.rebuild_seed,
                    threads,
                )?;
                rebuilds.push(RebuildCheck {
                    default,
                    value,
                    equals_centre: q
                        .iter()
                        .zip(&centre_quantiles)
                        .all(|(a, b)| a.total_cmp(b).is_eq()),
                });
                tables.push(table(&q));
                tables.len() - 1
            } else {
                1
            };
            jobs.push((params, table_index));
            slot.push(jobs.len() - 1);
        }
        slots.push(slot);
    }
    let figures = evaluate(&jobs, &tables, &settings, threads)?;
    let sweeps: Vec<Sweep> = KickDefault::ALL
        .into_iter()
        .zip(&settings.grids)
        .zip(&slots)
        .map(|((default, points), slot)| {
            Sweep::new(
                default,
                points.clone(),
                slot.iter().map(|&j| figures[j]).collect(),
            )
            .expect("a validated grid holds its committed value")
        })
        .collect();
    let sweeps = sweeps
        .try_into()
        .expect("one sweep for each of the four defaults");
    Ok(Review {
        settings,
        rebuilds,
        sensitivity: Sensitivity {
            committed: figures[0],
            rebuild: figures[1],
            sweeps,
        },
    })
}

/// The figures of each of `jobs` on `tables`, in parallel, in the jobs' order.
fn evaluate(
    jobs: &[Job],
    tables: &[KickRankTable],
    settings: &Settings,
    threads: NonZeroUsize,
) -> Result<Vec<Figures>, BuildThreadPoolError> {
    let mut figures = Vec::with_capacity(jobs.len());
    let n = u64::try_from(jobs.len()).expect("a job count fits in 64 bits");
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .map(|j| {
                    let (params, t) = &jobs[usize::try_from(j).expect("a job index fits")];
                    let o =
                        kick_observables(params, &tables[*t], settings.stars, settings.stars_seed);
                    Figures::of(o, params)
                })
                .collect::<Vec<_>>()
        },
        |chunk| figures.extend(chunk),
    )?;
    Ok(figures)
}

/// The table's item names, in order.
const ITEMS: [&str; 24] = [
    "DEFAULT_LOW_RAMP",
    "DEFAULT_BH_FACTOR",
    "DEFAULT_EC_WINDOW_SINGLE",
    "DEFAULT_EC_WINDOW_STRIPPED",
    "CRITERIA",
    "HELD_WINDOWS",
    "CENTRE_ON_COMMITTED_TABLE",
    "CENTRE_ON_REBUILD",
    "LOW_RAMP_GRID",
    "LOW_RAMP_OBSERVED",
    "LOW_RAMP_HELD",
    "LOW_RAMP_INTERVALS",
    "BH_FACTOR_GRID",
    "BH_FACTOR_OBSERVED",
    "BH_FACTOR_HELD",
    "BH_FACTOR_INTERVALS",
    "EC_WINDOW_SINGLE_GRID",
    "EC_WINDOW_SINGLE_OBSERVED",
    "EC_WINDOW_SINGLE_HELD",
    "EC_WINDOW_SINGLE_INTERVALS",
    "EC_WINDOW_STRIPPED_GRID",
    "EC_WINDOW_STRIPPED_OBSERVED",
    "EC_WINDOW_STRIPPED_HELD",
    "EC_WINDOW_STRIPPED_INTERVALS",
];

/// The widest a line of the table's docs or notes runs, in characters, its comment prefix aside.
const TEXT_WIDTH: usize = 96;

/// `text` wrapped to [`TEXT_WIDTH`]: each of its lines in turn, broken at spaces, an empty line
/// kept as a paragraph break.
#[must_use]
fn wrap(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > TEXT_WIDTH {
                lines.push(core::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(line);
    }
    lines
}

/// The sentence that gives the units of the observables' rows, in [`Observable::ALL`]'s order.
const UNITS: &str = "Rows 0 and 1 are ln(v ÷ km/s), row 2 is a p-value, and the rest are shares.";

/// The items of the table's body, which hold the figures of `s` alone.
#[must_use]
pub fn items(s: &Sensitivity) -> Vec<TableItem> {
    let scalar = |name: &str, text: &str, value: f64| TableItem::Scalar {
        name: name.to_owned(),
        doc: wrap(text),
        value,
    };
    let mut items = vec![
        TableItem::Array {
            name: "DEFAULT_LOW_RAMP".to_owned(),
            doc: wrap(
                "The low-mode ramp reviewed, M☉ of carbon–oxygen core: `kick_rank::LOW_RAMP`'s two \
                 ends.",
            ),
            values: vec![kick_rank::LOW_RAMP.0, kick_rank::LOW_RAMP.1],
        },
        scalar(
            "DEFAULT_BH_FACTOR",
            "The black-hole factor reviewed, on a black hole's ordinary kick: `kick_rank::BH_FACTOR`.",
            kick_rank::BH_FACTOR,
        ),
        scalar(
            "DEFAULT_EC_WINDOW_SINGLE",
            "The single-star window reviewed, M☉ of the mass `m_c_bagb` reads: \
             `kick_rank::EC_WINDOW_SINGLE`.",
            kick_rank::EC_WINDOW_SINGLE,
        ),
        scalar(
            "DEFAULT_EC_WINDOW_STRIPPED",
            "The companion-stripped window reviewed, M☉ of the same mass: \
             `kick_rank::EC_WINDOW_STRIPPED`.",
            kick_rank::EC_WINDOW_STRIPPED,
        ),
        TableItem::Tuples {
            name: "CRITERIA".to_owned(),
            doc: wrap(&criteria_doc()),
            rows: Observable::ALL
                .iter()
                .map(|o| {
                    let (lower, upper) = o.bounds();
                    vec![lower, upper]
                })
                .collect(),
        },
        TableItem::Tuples {
            name: "HELD_WINDOWS".to_owned(),
            doc: wrap(
                "The literature windows of the two figures ruling 147.2 holds at measured values, \
                 which the tests report and do not assert (decision-p15-revalidation Q2), as \
                 shares: the isolated pulsars' share under 50 km/s on the sky (Willcox et al. \
                 2021), then the neutron stars' share above the escape speed (ruling 96.3).",
            ),
            rows: Held::ALL
                .iter()
                .map(|h| vec![h.window().0, h.window().1])
                .collect(),
        },
        centre_item(
            "CENTRE_ON_COMMITTED_TABLE",
            "on the committed rank table: the slow tests' own figures.",
            &s.committed,
        ),
        centre_item(
            "CENTRE_ON_REBUILD",
            "on the centre's own rebuild of the rank table, which every grid point's figures are \
             compared with; against `CENTRE_ON_COMMITTED_TABLE`, the harness's resolution.",
            &s.rebuild,
        ),
    ];
    for sweep in &s.sweeps {
        items.extend(sweep_items(sweep));
    }
    items
}

/// The text of `CRITERIA`'s doc comment.
#[must_use]
fn criteria_doc() -> String {
    let mut text = format!(
        "The bounds each observable passes within, in the order of the observed rows: the slow \
         tests' criteria (`crates/hyperion-sim/tests/stellar_kick.rs`). {UNITS} The ordinary ln v \
         mean and sd are tested as |x − 5.60| ≤ 0.12 and |x − 0.68| ≤ 0.10; a lower bound alone \
         has 1 as its upper; and the share retained under 20 km/s must also lie within 0.002 of \
         the low mode's. The rows:\n",
    );
    for (i, o) in Observable::ALL.iter().enumerate() {
        // Writing into a `String` cannot fail.
        let _ = write!(text, "\n{i}. {} (`{}`)", o.label(), o.test());
    }
    text
}

/// A centre's figures as one array: the twelve observables, then the two held figures.
#[must_use]
fn centre_item(name: &str, what: &str, figures: &Figures) -> TableItem {
    TableItem::Array {
        name: name.to_owned(),
        doc: wrap(&format!(
            "The figures at the committed defaults {what} The twelve observables in the order of \
             `CRITERIA`, then the sky share under 50 km/s and the escape share. {UNITS}"
        )),
        values: figures
            .values
            .iter()
            .chain(&figures.held)
            .copied()
            .collect(),
    }
}

/// The four items of `sweep`: its grid, its observables and held figures along it, and the
/// intervals.
#[must_use]
fn sweep_items(sweep: &Sweep) -> Vec<TableItem> {
    let prefix = sweep.default.item_prefix();
    let label = sweep.default.label();
    let grid = if sweep.default == KickDefault::LowRamp {
        let width = kick_rank::LOW_RAMP.1 - kick_rank::LOW_RAMP.0;
        TableItem::Tuples {
            name: format!("{prefix}_GRID"),
            doc: wrap(
                "The low-mode ramp's grid, M☉ of carbon–oxygen core: its two ends, moved together \
                 at the committed width.",
            ),
            rows: sweep.points.iter().map(|&p| vec![p, p + width]).collect(),
        }
    } else {
        TableItem::Array {
            name: format!("{prefix}_GRID"),
            doc: wrap(&format!("The grid of {label}, {}.", sweep.default.unit())),
            values: sweep.points.clone(),
        }
    };
    let observed = TableItem::Nested {
        name: format!("{prefix}_OBSERVED"),
        doc: wrap(&format!(
            "The observables along the grid of {label}, one row an observable in the order of \
             `CRITERIA`, one column a grid point; the default's column is `CENTRE_ON_REBUILD`'s. \
             {UNITS}"
        )),
        rows: Observable::ALL
            .iter()
            .map(|&o| sweep.figures.iter().map(|f| f.value(o)).collect())
            .collect(),
    };
    let held = TableItem::Nested {
        name: format!("{prefix}_HELD"),
        doc: wrap(&format!(
            "The two held figures along the grid of {label}, as shares, against `HELD_WINDOWS`: \
             the sky share under 50 km/s, then the escape share."
        )),
        rows: Held::ALL
            .iter()
            .map(|&h| sweep.figures.iter().map(|f| f.held[h.index()]).collect())
            .collect(),
    };
    let coordinate = if sweep.default == KickDefault::LowRamp {
        " Its ends are values of the ramp's lower end."
    } else {
        ""
    };
    let intervals = TableItem::Tuples {
        name: format!("{prefix}_INTERVALS"),
        doc: wrap(&format!(
            "Each observable's interval on the grid of {label}, in the order of `CRITERIA`: the \
             outermost grid points of the run of passing points that holds the default, to one \
             grid step.{coordinate} An end at the grid's first or last point is open: no edge was \
             found there. An observable that fails at the default itself has the default for \
             both ends, and the notes' findings name it."
        )),
        rows: Observable::ALL
            .iter()
            .map(|&o| match sweep.interval(o) {
                Some((lower, upper)) => vec![sweep.points[lower], sweep.points[upper]],
                None => vec![sweep.points[sweep.centre]; 2],
            })
            .collect(),
    };
    vec![grid, observed, held, intervals]
}

/// The table's contents: its summary, its notes and its items.
#[must_use]
pub fn render(review: &Review) -> RustTable {
    RustTable {
        summary: vec![
            "The kick law's defaults review (plan 15, P15.T5.b): the kick-law tests' observables on a"
                .to_owned(),
            "grid of each of the four defaults, and the interval over which each keeps passing."
                .to_owned(),
        ],
        notes: wrap(&notes(review)),
        items: items(&review.sensitivity),
    }
}

/// The resolution in words: the largest differences of the ln-speed figures and of the shares
/// between the centre on its rebuild and on the committed table.
#[must_use]
fn resolution_text(s: &Sensitivity) -> String {
    let r = s.resolution();
    format!(
        "{:.4} in ln(v ÷ km/s) ({}) and {:.4} in a share ({})",
        r.ln_speed.0, r.ln_speed.1, r.share.0, r.share.1
    )
}

/// The table's notes, a paragraph a line, paragraphs apart.
#[must_use]
fn notes(review: &Review) -> String {
    let st = &review.settings;
    let s = &review.sensitivity;
    let paragraphs = [
        "No generator code reads this table. It is plan 15's review of the kick law's four \
         defaults, which moves none of them (decision-p15-revalidation Q2 and Q3)."
            .to_owned(),
        format!(
            "Inputs: at each grid point, `stellar::remnant::reference::kick_observables` over {} \
             stars of seed {:#018x}, the slow tests' sample, under `KickLawParams::default()` with \
             the one default moved and the rest held, on a rank table rebuilt from {} scores of \
             seed {:#018x} at that point's membership. The black-hole factor's points use the \
             centre's rebuild. The ramp's two ends move together, its width held.",
            st.stars,
            st.stars_seed.get(),
            st.rebuild_scores,
            st.rebuild_seed.get(),
        ),
        format!(
            "Resolution: the centre on its rebuild differs from the committed table by at most {} \
             (the K–S p-value aside). An interval's ends are good to one grid step. An end at the \
             grid's first or last point is open, and is not counted as an edge. A default one \
             step or more from a closed end lies within a tenth of the interval's width of it only \
             on a grid of over ten steps.",
            resolution_text(s)
        ),
        format!("Rebuilds: {}.", rebuilds_summary(review)),
        format!("Findings: {}.", findings(s).join("; ")),
        format!(
            "Edges found, the default's closed ends: {}. Every other interval is open at both \
             ends, the observable passing over the whole grid, but for any the findings name as \
             failing.",
            edges_summary(s)
        ),
        format!("Held figures along the grids: {}.", held_summary(s)),
    ];
    paragraphs.join("\n\n")
}

/// What the rebuilds found against the centre's, in words.
#[must_use]
fn rebuilds_summary(review: &Review) -> String {
    let mut parts = Vec::new();
    for default in KickDefault::ALL.into_iter().filter(|d| d.rebuilds()) {
        let of: Vec<bool> = review
            .rebuilds
            .iter()
            .filter(|r| r.default == default)
            .map(|r| r.equals_centre)
            .collect();
        let same = of.iter().filter(|&&b| b).count();
        parts.push(format!(
            "{}, {same} of {} equal to the centre's bit for bit",
            default.label(),
            of.len()
        ));
    }
    parts.join("; ")
}

/// `observable`'s interval on `sweep`'s grid in words, its closed ends named.
#[must_use]
fn interval_text(sweep: &Sweep, observable: Observable) -> String {
    let Some((lower, upper)) = sweep.interval(observable) else {
        return "none".to_owned();
    };
    let last = sweep.points.len() - 1;
    let closed = match (lower > 0, upper < last) {
        (true, true) => "closed at both ends",
        (true, false) => "closed below",
        (false, true) => "closed above",
        (false, false) => "open",
    };
    format!("{}–{} ({closed})", sweep.points[lower], sweep.points[upper])
}

/// For each default, the observables whose interval has a closed end, with the interval.
#[must_use]
fn edges_summary(s: &Sensitivity) -> String {
    let parts: Vec<String> = s
        .sweeps
        .iter()
        .map(|sweep| {
            let closed: Vec<String> = Observable::ALL
                .into_iter()
                .filter(|&o| sweep.interval(o).is_some() && !sweep.open_at_both_ends(o))
                .map(|o| format!("{} {}", o.label(), interval_text(sweep, o)))
                .collect();
            let list = if closed.is_empty() {
                "none".to_owned()
            } else {
                closed.join(", ")
            };
            format!("{}: {list}", sweep.default.label())
        })
        .collect();
    parts.join("; ")
}

/// The findings, in words; "none" if there are none: an observable that fails at the committed
/// defaults or on the centre's rebuild, and a default at or near an edge of an interval.
#[must_use]
fn findings(s: &Sensitivity) -> Vec<String> {
    let mut found = Vec::new();
    for o in s.committed.failing() {
        found.push(format!("{} fails at the committed defaults", o.label()));
    }
    for o in s.rebuild.failing() {
        found.push(format!(
            "{} fails on the centre's rebuild, so has no interval",
            o.label()
        ));
    }
    for sweep in &s.sweeps {
        for o in Observable::ALL {
            let verdict = sweep.verdict(o);
            if matches!(verdict, Verdict::AtEdge | Verdict::NearEdge) {
                found.push(format!(
                    "{} {} of {}'s interval, {}",
                    sweep.default.label(),
                    verdict.label(),
                    o.label(),
                    interval_text(sweep, o),
                ));
            }
        }
    }
    if found.is_empty() {
        found.push("none".to_owned());
    }
    found
}

/// The held figures' range along each grid and how many points lie inside their windows.
#[must_use]
fn held_summary(s: &Sensitivity) -> String {
    let mut parts = Vec::new();
    for sweep in &s.sweeps {
        for h in Held::ALL {
            let values: Vec<f64> = sweep.figures.iter().map(|f| f.held[h.index()]).collect();
            let (lo, hi) = values
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
                    (lo.min(x), hi.max(x))
                });
            let inside = values.iter().filter(|&&x| h.inside(x)).count();
            let (wlo, whi) = h.window();
            parts.push(format!(
                "{} along {}, {lo:.4}–{hi:.4}, {inside} of {} points inside {wlo:.2}–{whi:.2}",
                h.label(),
                sweep.default.label(),
                values.len(),
            ));
        }
    }
    parts.join("; ")
}

/// The header's acceptance line.
#[must_use]
fn acceptance(review: &Review) -> String {
    let s = &review.sensitivity;
    let failing = s.committed.failing();
    let verdict = if failing.is_empty() {
        "every observable passes".to_owned()
    } else {
        let names: Vec<&str> = failing.iter().map(|o| o.label()).collect();
        format!("FAILING {}", names.join(", "))
    };
    let held: Vec<String> = Held::ALL
        .iter()
        .map(|&h| {
            let (wlo, whi) = h.window();
            format!(
                "{} {:.4} against {wlo:.2}–{whi:.2}",
                h.label(),
                s.committed.held[h.index()],
            )
        })
        .collect();
    format!(
        "at the committed defaults on the committed table {verdict}, the two figures ruling 147.2 \
         holds reported as held ({}); resolution: the centre on its rebuild of {} scores within \
         {} of the committed table; rebuilds: {}; findings: {}",
        held.join(", "),
        review.settings.rebuild_scores,
        resolution_text(s),
        rebuilds_summary(review),
        findings(s).join("; "),
    )
}

/// The task: plan 15's review of the kick law's four defaults (P15.T5.b); slow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct KickDefaultsTask;

impl FitTask for KickDefaultsTask {
    fn name(&self) -> &'static str {
        "kick_defaults"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "kick_defaults.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The rank table's probes ([`KickRankTask`]'s: the tracks of massive stars, the score's
    /// scatter and plan 11's stripped share), then the four defaults, both ends of the ramp: a
    /// changed track or a moved default stales the table (decision-p15-revalidation Q3).
    ///
    /// As the ruling has it, the committed rank table is not probed: every figure but
    /// `CENTRE_ON_COMMITTED_TABLE`, the slow tests' own, comes from the review's own rebuilds. So
    /// a swap of the committed table (P06.T19.e) does not stale this one, and the swap re-emits it.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes = KickRankTask.fingerprint().probes().to_vec();
        probes.extend([
            ("kick_rank::LOW_RAMP.0".to_owned(), kick_rank::LOW_RAMP.0),
            ("kick_rank::LOW_RAMP.1".to_owned(), kick_rank::LOW_RAMP.1),
            ("kick_rank::BH_FACTOR".to_owned(), kick_rank::BH_FACTOR),
            (
                "kick_rank::EC_WINDOW_SINGLE".to_owned(),
                kick_rank::EC_WINDOW_SINGLE,
            ),
            (
                "kick_rank::EC_WINDOW_STRIPPED".to_owned(),
                kick_rank::EC_WINDOW_STRIPPED,
            ),
        ]);
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let review = review(Settings::from_manifest(manifest)?, threads)?;
        Ok(TaskOutput {
            table: render(&review),
            source: "the brainstorm's \"Testing\" list, through plan 06's kick-law tests (P06.T19.d, \
                     `crates/hyperion-sim/tests/stellar_kick.rs`): Willcox et al. (2021, ApJ Letters \
                     920, L37) for the slow isolated pulsars; Igoshev et al. (2021, MNRAS 508, 3345) \
                     for the low mode; Disberg and Mandel (2025, ApJ Letters 989, L8) for the ordinary \
                     speeds, and with Igoshev (2020, MNRAS 494, 3663) for the share under 100 km/s; \
                     Nagarajan and El-Badry (2025, PASP 137, 034203) for the light black holes; Hills \
                     (1983, ApJ 267, 322) and Brandt and Podsiadlowski (1995, MNRAS 274, 461) for the \
                     toy double neutron stars, and Grichener et al. (2026) for their observed \
                     eccentricities"
                .to_owned(),
            acceptance: acceptance(&review),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::stellar::remnant::reference::score_quantiles;
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;

    #[test]
    fn an_interval_is_the_contiguous_run_of_passing_points_about_the_default() {
        let passes = [false, true, true, true, false, true, true];
        assert_eq!(interval_of(&passes, 2), Some((1, 3)));
        assert_eq!(interval_of(&passes, 5), Some((5, 6)));
        assert_eq!(interval_of(&passes, 0), None);
        assert_eq!(interval_of(&passes, 4), None);
        assert_eq!(interval_of(&passes, 9), None);
        let points: Vec<f64> = (0..7).map(f64::from).collect();
        // Both ends closed, the default two steps from each: inside.
        let all = [false, true, true, true, true, true, false];
        assert_eq!(
            verdict_of(&points, 3, interval_of(&all, 3)),
            Verdict::Inside
        );
        // A closed end at the default: at an edge.
        assert_eq!(
            verdict_of(&points, 1, interval_of(&all, 1)),
            Verdict::AtEdge
        );
        assert_eq!(verdict_of(&points, 4, None), Verdict::FailsAtDefault);
    }

    #[test]
    fn an_end_at_the_grids_first_or_last_point_is_open_and_never_an_edge() {
        let points: Vec<f64> = (0..5).map(f64::from).collect();
        let passes = [true, true, true, true, true];
        assert_eq!(interval_of(&passes, 0), Some((0, 4)));
        // The default at the grid's first point, which passes: the end is open, not an edge.
        assert_eq!(
            verdict_of(&points, 0, interval_of(&passes, 0)),
            Verdict::Inside
        );
        assert_eq!(
            verdict_of(&points, 4, interval_of(&passes, 4)),
            Verdict::Inside
        );
        // A run to the last point, closed below at the default.
        let upper_open = [false, true, true, true, true];
        assert_eq!(interval_of(&upper_open, 1), Some((1, 4)));
        assert_eq!(
            verdict_of(&points, 1, interval_of(&upper_open, 1)),
            Verdict::AtEdge
        );
    }

    #[test]
    fn a_default_within_a_tenth_of_the_width_of_an_edge_is_near_it() {
        // 21 points, the default at 10, passing from 9 to the last: width 11, a tenth 1.1, and
        // the closed lower end 1 below the default.
        let points: Vec<f64> = (0..21).map(f64::from).collect();
        let passes: Vec<bool> = (0..21).map(|i| i >= 9).collect();
        assert_eq!(interval_of(&passes, 10), Some((9, 20)));
        assert_eq!(
            verdict_of(&points, 10, interval_of(&passes, 10)),
            Verdict::NearEdge
        );
        // From 8, width 12 and a tenth 1.2 against a distance of 2: inside.
        let wider: Vec<bool> = (0..21).map(|i| i >= 8).collect();
        assert_eq!(
            verdict_of(&points, 10, interval_of(&wider, 10)),
            Verdict::Inside
        );
        assert!(Verdict::NearEdge.is_finding() && Verdict::AtEdge.is_finding());
        assert!(!Verdict::Inside.is_finding());
    }

    #[test]
    fn a_held_figure_is_reported_against_its_window_and_has_no_interval() {
        assert!(!Held::SkyUnder50.inside(Held::SkyUnder50.measured()));
        assert!(!Held::EscapeShare.inside(Held::EscapeShare.measured()));
        assert!(Held::SkyUnder50.inside(0.05));
        assert!(Held::EscapeShare.inside(0.12) && Held::EscapeShare.inside(0.20));
        // A held figure outside its window moves no observable's verdict: the figures pass.
        let committed = committed_figures();
        assert_eq!(committed.failing(), Vec::<Observable>::new());
        let mut off = committed;
        off.held = [0.5, 0.5];
        assert_eq!(off.failing(), Vec::<Observable>::new());
    }

    /// Ruling 147.2's figures at the committed defaults and table (plan 06's "Ruling 147.2, as
    /// built").
    fn committed_figures() -> Figures {
        Figures {
            values: [
                5.557, 0.631, 0.87, 0.0903, 0.2740, 0.2740, 0.2783, 0.3232, 0.771, 0.711, 0.670,
                0.136,
            ],
            held: [0.0708, 0.1159],
        }
    }

    #[test]
    fn each_criterion_is_its_tests() {
        let f = committed_figures();
        let with = |o: Observable, x: f64| {
            let mut g = f;
            g.values[o.index()] = x;
            g.passes(o)
        };
        assert!(with(Observable::OrdinaryLnMean, 5.70));
        assert!(!with(Observable::OrdinaryLnMean, 5.73));
        assert!(!with(Observable::OrdinaryLnSd, 0.57));
        assert!(with(Observable::OrdinaryKs, ALPHA));
        assert!(!with(Observable::OrdinaryKs, 0.5 * ALPHA));
        assert!(with(Observable::BlackHolesUnkicked, 0.5));
        assert!(!with(Observable::BlackHolesUnkicked, 0.49));
        assert!(!with(Observable::LightBlackHolesFast, 0.119));
        // Retained under 20 km/s must match the low mode's share too.
        assert!(!with(Observable::RetainedUnder20, 0.2770));
        assert!(with(Observable::RetainedUnder20, 0.2755));
        assert_eq!(
            Observable::ALL.map(Observable::index),
            core::array::from_fn(|i| i)
        );
    }

    /// The stated bounds are the criteria's: centre ∓ half for the two moments, and the
    /// criterion's own ends, or 1 above a lower bound alone, for the rest.
    #[test]
    fn the_stated_bounds_are_the_criteria() {
        for o in Observable::ALL {
            let (lower, upper) = o.bounds();
            let (want_lower, want_upper) = match o.criterion() {
                Criterion::Within { centre, half } => (centre - half, centre + half),
                Criterion::Between { lower, upper } => (lower, upper),
                Criterion::AtLeast { lower } => (lower, 1.0),
            };
            assert!((lower - want_lower).abs() < 1e-12, "{o:?}");
            assert!((upper - want_upper).abs() < 1e-12, "{o:?}");
        }
    }

    #[test]
    fn each_default_at_its_committed_value_is_the_default_law() {
        for d in KickDefault::ALL {
            assert_eq!(
                d.params_at(d.committed()),
                KickLawParams::default(),
                "{d:?}"
            );
        }
        let moved = KickDefault::LowRamp.params_at(1.875);
        assert_same_bits(moved.low_ramp.0, 1.875);
        assert_same_bits(moved.low_ramp.1, 2.875);
        assert!(!KickDefault::BhFactor.rebuilds());
    }

    /// The task's K–S test is the testkit's, bit for bit, on a uniform sample and on a shifted
    /// one.
    #[test]
    fn the_ks_test_is_the_testkits() {
        let mut g = Lcg::new(11);
        for shift in [0.0, 0.02] {
            let sample: Vec<f64> = (0..5_000).map(|_| g.next_f64() * (1.0 - shift)).collect();
            let cdf = |x: f64| x.clamp(0.0, 1.0);
            let ours = ks_one_sample(&mut sample.clone(), cdf);
            let theirs = hyperion_testkit::stats::ks_one_sample(&mut sample.clone(), cdf);
            assert_same_bits(ours.statistic, theirs.statistic);
            assert_same_bits(ours.p_value, theirs.p_value);
        }
        assert_same_bits(ALPHA, hyperion_testkit::stats::ALPHA);
    }

    /// Test 1's K–S test is the slow test's, bit for bit: the same truncated log-normal through
    /// the testkit's `normal_cdf`, on log-normal speeds.
    #[test]
    fn the_ordinary_ks_test_is_the_slow_tests() {
        use hyperion_testkit::stats::normal_cdf;
        let p = KickLawParams::default();
        let mut g = Lcg::new(29);
        // Box–Muller normals of the log-normal's μ and σ, those above the truncation redrawn.
        let speeds: Vec<f64> = core::iter::repeat_with(|| {
            let (u1, u2) = (1.0 - g.next_f64(), g.next_f64());
            let z = (-2.0 * math::ln(u1)).sqrt() * math::cos(core::f64::consts::TAU * u2);
            math::mul_add(p.ln_sigma, z, p.ln_mu)
        })
        .filter(|&x| x < math::ln(p.max_speed_km_s))
        .take(4_000)
        .collect();
        let ours = ordinary_ks(&mut speeds.clone(), &p);
        let below = normal_cdf((math::ln(p.max_speed_km_s) - p.ln_mu) / p.ln_sigma);
        let theirs = hyperion_testkit::stats::ks_one_sample(&mut speeds.clone(), |x| {
            (normal_cdf((x - p.ln_mu) / p.ln_sigma) / below).clamp(0.0, 1.0)
        });
        assert_same_bits(ours.statistic, theirs.statistic);
        assert_same_bits(ours.p_value, theirs.p_value);
        assert!(ours.p_value > ALPHA, "{ours:?}");
    }

    /// A rebuild at a moved default is `score_quantiles` of its population bit for bit, on one
    /// thread and on four, its 600 scores' first round of 1,200 sample stars cut into two chunks.
    #[test]
    fn a_rebuild_is_score_quantiles_of_its_population_for_any_thread_count() {
        let params = KickDefault::LowRamp.params_at(1.875);
        let seed = Seed::new(0x0619_b000_0000_0000);
        let population = ReferencePopulation::new(params).unwrap();
        let serial = score_quantiles(&population, 600, seed);
        for threads in [1, 4] {
            let threads = NonZeroUsize::new(threads).unwrap();
            let rebuilt = rebuild(&params, 600, seed, threads).unwrap();
            for (a, b) in rebuilt.iter().zip(&serial) {
                assert_same_bits(*a, *b);
            }
        }
    }

    fn manifest(params: &str) -> Manifest {
        Manifest::from_bytes(
            std::path::Path::new("manifests/kick_defaults.toml"),
            format!("task = \"kick_defaults\"\n\n[params]\n{params}").into_bytes(),
        )
        .unwrap()
    }

    const GOOD: &str = "rebuild_scores = 16\nrebuild_seed = 1\nstars = 10\nstars_seed = 2\n\
                        low_ramp_lower_grid = [1.875, 2.0]\nbh_factor_grid = [0.75, 1.0]\n\
                        ec_window_single_grid = [0.1]\nec_window_stripped_grid = [0.8, 1.0]\n";

    #[test]
    fn the_settings_are_the_manifests() {
        let s = Settings::from_manifest(&manifest(GOOD)).unwrap();
        assert_eq!(s.rebuild_scores, 16);
        assert_eq!(s.rebuild_seed, Seed::new(1));
        assert_eq!(s.grids[1], [0.75, 1.0]);
        assert_eq!(s.grids[2], [0.1]);
    }

    #[test]
    fn the_settings_refuse_a_bad_grid() {
        for (from, to, key) in [
            ("[1.875, 2.0]", "[1.875, 2.125]", "low_ramp_lower_grid"),
            ("[0.75, 1.0]", "[1.0, 0.75]", "bh_factor_grid"),
            (
                "ec_window_single_grid = [0.1]",
                "ec_window_single_grid = [0.0, 0.1]",
                "ec_window_single_grid",
            ),
            (
                "ec_window_stripped_grid = [0.8, 1.0]",
                "ec_window_stripped_grid = 1.0",
                "ec_window_stripped_grid",
            ),
            (
                "rebuild_scores = 16",
                "rebuild_scores = 1",
                "rebuild_scores",
            ),
            ("stars = 10", "stars = 0", "stars"),
        ] {
            let text = GOOD.replace(from, to);
            assert_ne!(text, GOOD, "{from}");
            let error = Settings::from_manifest(&manifest(&text)).unwrap_err();
            assert!(
                error.to_string().contains(&format!("`{key}`")),
                "{to}: {error}"
            );
        }
    }

    #[test]
    fn the_rendered_items_are_the_tasks() {
        let f = committed_figures();
        let sweeps = KickDefault::ALL.map(|d| Sweep::new(d, vec![d.committed()], vec![f]).unwrap());
        let s = Sensitivity {
            committed: f,
            rebuild: f,
            sweeps,
        };
        let table = RustTable {
            items: items(&s),
            ..RustTable::default()
        };
        assert_eq!(table.item_names(), KickDefaultsTask.items());
        assert!(table.body().is_ok());
    }
}
