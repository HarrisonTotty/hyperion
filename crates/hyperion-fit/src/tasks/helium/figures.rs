//! P15.T7.b's acceptance figures (plan 15, 1551–1557, as ruled on 2026-10-23).
//!
//! They are measured on plan 06's tracks built under the fitted table
//! (`stellar::sse::track_with_helium_table`):
//!
//! 1. the lifetime ratio of every fitted grid point: the track's main sequence at the point's ΔY
//!    over its own at ΔY = 0, both from their arrival on the main sequence to its end (`BaSTI`'s
//!    ages run from the zero-age main sequence), against the grid's ratio, to 3%. The fitted
//!    points' rows bracket the acceptance's \[Fe/H\] −2.2 to −0.5: each set sits at its own total
//!    Z, and the grid has no helium-enhanced set between −2.33 and −1.92, or between −0.73 and
//!    −0.49. The worst inside the range is stated too;
//! 2. the identity at ΔY = 0: tracks of ΔY = 0 under the fitted table are the committed
//!    generator's (`Track::full`) bit for bit, at every age probed;
//! 3. s(m, Z) negative at 0.1 and 150 M☉, the masses a track takes, at every node: with c₂ held
//!    at zero s is linear in log₁₀ m and in log₁₀ Z, so that bounds every mass and Z (ruled
//!    2026-10-26, decision-p15-t7b-leans L2);
//! 4. the shortening of a 0.8 M☉ star's whole life by ΔY = 0.1 at the four nodes' \[Fe/H\], in
//!    0.42–0.55, about a half (L3);
//! 5. the fitted horizontal-branch shift against the grid's zero-age shift of log₁₀ `T_eff` at
//!    fixed envelope mass, at the grid's top Y: their means over that Y's fitted points, to 10%
//!    of the grid's (L4);
//! 6. the synthetic cluster's horizontal branch ([`cluster`]): its blue end at the grid's top ΔY
//!    hotter than its first population's (L4), its figure stated beside the grid's amount, with
//!    the core masses at helium ignition beside the grid's;
//!
//! and, reported beside them, the extrapolation to ΔY = 0.18 (`HELIUM_EXCESS_MAX`) against the
//! trend of the grid's own ratios at its three ΔY (the ruling's point 6).
//!
//! The probes of figure 2 ([`IDENTITY_PROBES`], [`probe_ages_yr`]), the cluster's search
//! settings and the extrapolation's reference envelope ([`REFERENCE_ENVELOPE_MSUN`]) are fixed
//! measurement settings, not the manifest's: a change to them is a change of the task, which
//! takes a revision.

use std::fmt::Write as _;

use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::sse::{
    CandidateHeliumTable, MAX_INITIAL_MASS, MIN_INITIAL_MASS, Track, track_with_helium_table,
};
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses, Years};

use super::cluster::{self, ClusterFigures};
use super::fit::{BranchPoint, Fit, FitParams, LifetimePoint, log_z_fit, slope_at};
use super::grid::{Grid, MASS_MATCH_MSUN, SIM_Z_SOLAR};

/// The envelope mass at which the extrapolation's sentence states the branch's shift, besides
/// none, M☉: a blue horizontal-branch star's.
pub const REFERENCE_ENVELOPE_MSUN: f64 = 0.1;

/// How near a grid set's Z must be to the cluster's, in dex, for its ratios to be the
/// extrapolation's trend: the cluster's row's three sets lie within 0.06 dex of it.
const TREND_DEX: f64 = 0.1;

/// One grid point's lifetime ratio on the tracks against the grid's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LifetimeCheck {
    /// The grid point.
    pub point: LifetimePoint,
    /// The ratio of the main sequences' durations on the tracks.
    pub track_ratio: f64,
}

impl LifetimeCheck {
    /// The grid's ratio, t(Y) ÷ t(Y₀).
    #[must_use]
    pub fn grid_ratio(&self) -> f64 {
        math::exp(self.point.ln_ratio)
    }

    /// The tracks' ratio over the grid's, less one.
    #[must_use]
    pub fn error(&self) -> f64 {
        self.track_ratio / self.grid_ratio() - 1.0
    }
}

/// The duration of the main sequence on a track of initial mass `mass_msun` (M☉) and
/// `composition`, years, under `table`.
fn main_sequence_duration_yr(
    mass_msun: f64,
    composition: &Composition,
    table: &CandidateHeliumTable,
) -> Option<f64> {
    let track = track_with_helium_table(
        SolarMasses::new(mass_msun),
        composition,
        &StarDraws::median(),
        table,
    );
    cluster::main_sequence_years(&track).map(|(start, end)| end - start)
}

/// The composition of a star of the sim's \[Fe/H\] `fe_h` and helium excess `delta_y`.
fn composition(fe_h: f64, delta_y: f64) -> Composition {
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(delta_y))
}

/// Whether two tracks give the same state, bit for bit, at every one of `ages_yr`: phase, mass,
/// luminosity and radius, compared by `total_cmp`, which tells every bit pattern of a number
/// apart.
fn same_states(a: &Track, b: &Track, ages_yr: &[f64]) -> bool {
    ages_yr.iter().all(|&age| {
        let (s, t) = (a.state_at(Years::new(age)), b.state_at(Years::new(age)));
        s.phase() == t.phase()
            && s.mass().value().total_cmp(&t.mass().value()).is_eq()
            && s.luminosity()
                .value()
                .total_cmp(&t.luminosity().value())
                .is_eq()
            && s.radius().value().total_cmp(&t.radius().value()).is_eq()
    })
}

/// The ages a track is probed at for the identity, years since the onset of collapse: 120
/// evenly spaced in log₁₀ from 10⁵ to 3 × 10¹⁰.
#[must_use]
pub fn probe_ages_yr() -> Vec<f64> {
    (0..120)
        .map(|i| math::exp10(5.0 + 5.477 * f64::from(i) / 119.0))
        .collect()
}

/// The initial masses, M☉, and \[Fe/H\] of the identity's probes: plan 06's helium tests' spread.
pub const IDENTITY_PROBES: [(f64, f64); 6] = [
    (0.6, -2.0),
    (0.8, -1.5),
    (0.9, -0.7),
    (1.0, 0.0),
    (3.0, -0.5),
    (20.0, 0.2),
];

/// A count, for the acceptance line: a `u64`, whatever the platform's `usize`.
fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The largest |error| of `checks`, a NaN counted as infinite.
fn worst(checks: impl Iterator<Item = LifetimeCheck>) -> f64 {
    checks.map(|c| c.error().abs()).fold(0.0, |worst, e| {
        if e.is_nan() {
            f64::INFINITY
        } else if e > worst {
            e
        } else {
            worst
        }
    })
}

/// The fitted shift of the zero-age horizontal branch against the grid's, at the grid's top Y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BranchShift {
    /// The top Y among the fitted branch points.
    pub y: f64,
    /// The fitted branch points of that Y.
    pub points: u64,
    /// Their mean ΔY.
    pub delta_y: f64,
    /// The mean of the fitted shift, (a + b `M_env`) ΔY, dex.
    pub fitted: f64,
    /// The mean of the grid's shift at the same envelope masses, dex.
    pub grid: f64,
}

impl BranchShift {
    /// The means of the fitted and the grid's shifts over the fitted branch points of the top Y,
    /// in the points' order, or `None` without points.
    #[must_use]
    pub fn of(table: &CandidateHeliumTable, points: &[BranchPoint]) -> Option<Self> {
        let y = points
            .iter()
            .map(|p| p.set.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let [a, b] = table.hb_temperature_shift();
        let (mut n, mut delta_y, mut fitted, mut grid) = (0_u32, 0.0, 0.0, 0.0);
        // The points' Y are the file's, so the top Y's are matched exactly.
        for p in points.iter().filter(|p| p.set.y.total_cmp(&y).is_eq()) {
            n += 1;
            delta_y += p.delta_y;
            fitted += (a + b * p.envelope_msun) * p.delta_y;
            grid += p.shift;
        }
        if n == 0 {
            return None;
        }
        let count = f64::from(n);
        Some(Self {
            y,
            points: u64::from(n),
            delta_y: delta_y / count,
            fitted: fitted / count,
            grid: grid / count,
        })
    }
}

/// The masses at which the slope's sign is held, M☉: the lightest and the heaviest a track takes.
pub const SLOPE_REACH_MSUN: [f64; 2] = [MIN_INITIAL_MASS.value(), MAX_INITIAL_MASS.value()];

/// Every figure.
#[derive(Debug, Clone, PartialEq)]
pub struct Figures {
    /// The lifetime checks, at every fitted grid point.
    pub lifetime: Vec<LifetimeCheck>,
    /// Whether every ΔY = 0 probe is `Track::full`'s bit for bit under the fitted table.
    pub identity_at_zero: bool,
    /// s(m, Z) at [`SLOPE_REACH_MSUN`] at each node, node by node.
    pub slope_reach: Vec<f64>,
    /// The fitted shift against the grid's at the grid's top Y.
    pub branch_shift: Option<BranchShift>,
    /// The shortening of the shortening star's life at each node's \[Fe/H\]: (\[Fe/H\],
    /// 1 − t(ΔY) ÷ t(0)).
    pub shortening: Vec<(f64, f64)>,
    /// The cluster's.
    pub cluster: Option<ClusterFigures>,
}

impl Figures {
    /// The figures of `fit` against `grid` under `params`.
    #[must_use]
    pub fn of(fit: &Fit, grid: &Grid, params: &FitParams) -> Self {
        let table = &fit.table;
        let lifetime = fit
            .lifetime
            .iter()
            .filter_map(|&point| {
                let fe_h = point.set.fe_h();
                let rich = main_sequence_duration_yr(
                    point.mass_msun,
                    &composition(fe_h, point.delta_y),
                    table,
                )?;
                let plain =
                    main_sequence_duration_yr(point.mass_msun, &composition(fe_h, 0.0), table)?;
                Some(LifetimeCheck {
                    point,
                    track_ratio: rich / plain,
                })
            })
            .collect();
        let ages = probe_ages_yr();
        let identity_at_zero = IDENTITY_PROBES.iter().all(|&(m, fe_h)| {
            let plain = composition(fe_h, 0.0);
            let draws = StarDraws::median();
            let full = Track::full(SolarMasses::new(m), &plain, &draws);
            let candidate = track_with_helium_table(SolarMasses::new(m), &plain, &draws, table);
            same_states(&full, &candidate, &ages)
        });
        let shortening = table
            .log_z_nodes()
            .iter()
            .map(|&log_z| {
                let fe_h = log_z - math::log10(SIM_Z_SOLAR);
                let life = |delta_y: f64| {
                    track_with_helium_table(
                        SolarMasses::new(params.shortening_mass_msun),
                        &composition(fe_h, delta_y),
                        &StarDraws::median(),
                        table,
                    )
                    .lifetime()
                    .map(Years::value)
                };
                let ratio = match (life(params.shortening_delta_y), life(0.0)) {
                    (Some(rich), Some(plain)) => rich / plain,
                    _ => f64::NAN,
                };
                (fe_h, 1.0 - ratio)
            })
            .collect();
        let nodes = table.log_z_nodes();
        let slope_reach = nodes
            .iter()
            .flat_map(|&log_z| {
                SLOPE_REACH_MSUN.map(|m| slope_at(&table.lifetime_slope(), &nodes, log_z, m))
            })
            .collect();
        Self {
            lifetime,
            identity_at_zero,
            slope_reach,
            branch_shift: BranchShift::of(table, &fit.branch),
            shortening,
            cluster: cluster::synthesise(grid, table, &params.cluster),
        }
    }

    /// The largest |error| of a lifetime ratio over every fitted point, a NaN counted as
    /// infinite.
    #[must_use]
    pub fn worst_lifetime(&self) -> f64 {
        worst(self.lifetime.iter().copied())
    }

    /// The checks inside `params.acceptance_fe_h`.
    fn inside<'a>(&'a self, params: &'a FitParams) -> impl Iterator<Item = LifetimeCheck> + 'a {
        let (lo, hi) = params.acceptance_fe_h;
        self.lifetime.iter().copied().filter(move |c| {
            let fe_h = c.point.set.fe_h();
            fe_h >= lo && fe_h <= hi
        })
    }

    /// Whether each criterion passes, in the module's order: the lifetime ratios, the identity,
    /// the slope's sign, the shortening, the branch's shift and the cluster's blue end.
    #[must_use]
    pub fn verdicts(&self, params: &FitParams) -> [bool; 6] {
        let (lo, hi) = params.shortening;
        let shortening = self.shortening.iter().all(|&(_, s)| s >= lo && s <= hi);
        let shift = self
            .branch_shift
            .is_some_and(|b| (b.fitted - b.grid).abs() <= params.hb_tolerance * b.grid.abs());
        let hotter = self
            .cluster
            .as_ref()
            .and_then(ClusterFigures::heating)
            .is_some_and(|(own, _)| own > 0.0);
        [
            !self.lifetime.is_empty() && self.worst_lifetime() <= params.lifetime_tolerance,
            self.identity_at_zero,
            !self.slope_reach.is_empty() && self.slope_reach.iter().all(|&s| s < 0.0),
            shortening,
            shift,
            hotter,
        ]
    }

    /// Whether every criterion passes.
    #[must_use]
    pub fn passed(&self, params: &FitParams) -> bool {
        self.verdicts(params).iter().all(|&ok| ok)
    }

    /// Writes the acceptance line into `out`: each criterion's figure and verdict, then the
    /// extrapolation to `params.extrapolation_delta_y`.
    pub fn describe(&self, fit: &Fit, params: &FitParams, out: &mut String) {
        // Writing into a `String` cannot fail, so each `write!`'s result is dropped.
        let verdict = |ok: bool| if ok { "passes" } else { "MISSES" };
        let [lifetime, identity, sign, shortening, shift, hotter] = self.verdicts(params);
        let fe_h = |pick: fn(f64, f64) -> f64, start: f64| {
            self.lifetime
                .iter()
                .map(|c| c.point.set.fe_h())
                .fold(start, pick)
        };
        let _ = write!(
            out,
            "lifetime ratios on plan 06's tracks against the grid's, {} points of {}–{} M☉ at \
             [Fe/H] {:.2} to {:.2} (the rows bracketing {} to {}), ΔY to {:.3}: within {:.2}% \
             ({}% allowed; {}), and the {} inside {} to {} within {:.2}%; ΔY = 0 tracks are \
             Track::full's bit for bit at {} probes ({}); s at {} and {} M☉ at the four nodes \
             from {:.2} to {:.2} (negative; {}); ΔY = {} shortens a {} M☉ life by ",
            count(self.lifetime.len()),
            params.masses_msun.0,
            params.masses_msun.1,
            fe_h(f64::min, f64::INFINITY),
            fe_h(f64::max, f64::NEG_INFINITY),
            params.acceptance_fe_h.0,
            params.acceptance_fe_h.1,
            self.lifetime
                .iter()
                .map(|c| c.point.delta_y)
                .fold(0.0, f64::max),
            100.0 * self.worst_lifetime(),
            100.0 * params.lifetime_tolerance,
            verdict(lifetime),
            count(self.inside(params).count()),
            params.acceptance_fe_h.0,
            params.acceptance_fe_h.1,
            100.0 * worst(self.inside(params)),
            count(IDENTITY_PROBES.len() * probe_ages_yr().len()),
            verdict(identity),
            SLOPE_REACH_MSUN[0],
            SLOPE_REACH_MSUN[1],
            self.slope_reach
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min),
            self.slope_reach
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max),
            verdict(sign),
            params.shortening_delta_y,
            params.shortening_mass_msun,
        );
        let parts: Vec<String> = self
            .shortening
            .iter()
            .map(|&(fe_h, s)| format!("{s:.3} at [Fe/H] {fe_h:.1}"))
            .collect();
        let _ = write!(
            out,
            "{} ({}–{}; {})",
            parts.join(", "),
            params.shortening.0,
            params.shortening.1,
            verdict(shortening)
        );
        self.describe_shift(params, verdict(shift), out);
        self.describe_cluster(params, verdict(hotter), out);
        describe_extrapolation(fit, params, out);
    }

    /// The branch shift's sentence of the acceptance line, its verdict `verdict`.
    fn describe_shift(&self, params: &FitParams, verdict: &str, out: &mut String) {
        // Writing into a `String` cannot fail.
        match self.branch_shift {
            Some(b) => {
                let _ = write!(
                    out,
                    "; the fitted shift of the zero-age horizontal branch at Y = {} (ΔY {:.3} on \
                     average), over its {} points at fixed envelope mass, {:.4} dex on average \
                     against the grid's {:.4}, within {:.1}% ({}% allowed; {})",
                    b.y,
                    b.delta_y,
                    b.points,
                    b.fitted,
                    b.grid,
                    100.0 * (b.fitted - b.grid).abs() / b.grid.abs(),
                    100.0 * params.hb_tolerance,
                    verdict,
                );
            }
            None => {
                let _ = write!(out, "; the zero-age branch has no fitted points (MISSES)");
            }
        }
    }

    /// The cluster's sentence of the acceptance line, its verdict `verdict`.
    fn describe_cluster(&self, params: &FitParams, verdict: &str, out: &mut String) {
        // Writing into a `String` cannot fail.
        let blue = self.cluster.as_ref().and_then(|c| {
            let (own, grid) = c.heating()?;
            Some((c.components.first()?, c.components.get(c.top)?, own, grid))
        });
        match blue {
            Some((first, top, own, grid)) => {
                let core =
                    |m: Option<f64>| m.map_or_else(|| "none".to_owned(), |m| format!("{m:.4}"));
                let _ = write!(
                    out,
                    "; a {:e} M☉ cluster at {} Gyr and Z = {} ([Fe/H] {:.2}), ΔY up to {}: its \
                     horizontal branch's blue end ({} quantile of log T_eff) at ΔY = {:.3} is \
                     {:.3} dex hotter than the first population's ({} and {} branch stars; \
                     hotter, {}), beside the grid's branch's {:.3} dex for the same envelopes \
                     (the excess is plan 06's branch, a recorded finding); the core mass at \
                     helium ignition on plan 06's tracks {} M☉ at ΔY 0 and {} at ΔY {:.3}, \
                     against the grid's zero-age {} and {}",
                    params.cluster.mass_msun,
                    params.cluster.age_yr / 1e9,
                    params.cluster.z,
                    params.cluster.fe_h(),
                    params.cluster.delta_y_max,
                    params.cluster.blue_end_quantile,
                    top.component.delta_y,
                    own,
                    count(top.branch.len()),
                    count(first.branch.len()),
                    verdict,
                    grid,
                    core(first.ignition_core_msun),
                    core(top.ignition_core_msun),
                    top.component.delta_y,
                    core(first.grid_core_msun),
                    core(top.grid_core_msun),
                );
            }
            None => {
                let _ = write!(
                    out,
                    "; the synthetic cluster has no blue end to compare (MISSES)"
                );
            }
        }
    }
}

/// The extrapolation's sentence: the fitted lifetime factor and shift at ΔY = 0.18 at the
/// cluster's Z, beside the grid's ratios at its three ΔY there.
fn describe_extrapolation(fit: &Fit, params: &FitParams, out: &mut String) {
    let table = &fit.table;
    let z = params.cluster.z;
    let mass = params.shortening_mass_msun;
    let log_z = log_z_fit(params.cluster.fe_h());
    let s = slope_at(&table.lifetime_slope(), &table.log_z_nodes(), log_z, mass);
    let dy = params.extrapolation_delta_y;
    let mut row: Vec<&LifetimePoint> = fit
        .lifetime
        .iter()
        .filter(|p| {
            (p.mass_msun - mass).abs() < MASS_MATCH_MSUN
                && (math::log10(p.set.z) - math::log10(z)).abs() < TREND_DEX
        })
        .collect();
    row.sort_by(|p, q| p.delta_y.total_cmp(&q.delta_y));
    let trend: Vec<String> = row
        .iter()
        .map(|p| {
            format!(
                "{:.3} at ΔY {:.3} (s {:.3})",
                math::exp(p.ln_ratio),
                p.delta_y,
                p.ln_ratio / p.delta_y
            )
        })
        .collect();
    let [a, b] = table.hb_temperature_shift();
    // Writing into a `String` cannot fail.
    let _ = write!(
        out,
        "; extrapolated to ΔY = {dy} (beyond the grid's reach, not clamped): a {mass} M☉ \
         star's timescales at Z = {z} scale by {:.3} (s = {s:.3}), beside the grid's ratios \
         there of {}; the branch's shift (a + b M_env) ΔY = {:.3} dex at M_env 0 and {:.3} \
         at {} M☉",
        math::exp(s * dy),
        if trend.is_empty() {
            "none".to_owned()
        } else {
            trend.join(", ")
        },
        a * dy,
        (a + b * REFERENCE_ENVELOPE_MSUN) * dy,
        REFERENCE_ENVELOPE_MSUN,
    );
}
