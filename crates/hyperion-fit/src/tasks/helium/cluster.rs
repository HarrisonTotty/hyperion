//! The synthetic cluster of P15.T7.b's acceptance (plan 15, 1551–1557).
//!
//! It is a globular cluster of 10⁶ M☉ with a helium-enhanced second population, built star by
//! star on plan 06's tracks under a candidate helium table, whose horizontal branch's blue end at
//! the grid's top ΔY must be hotter than its first population's by the grid's amount.
//!
//! - **Its stars.** The cluster's [`ClusterParams::mass_msun`] of stars formed, from the galaxy's
//!   default mass function (`MassFunctionKind::default()`, Chabrier's system function at the
//!   provisional high-mass scale, as `stellar::testing` draws masses), at one age and one Z, the
//!   sim's \[Fe/H\] = log₁₀(Z ÷ 0.02).
//! - **Its populations**, as plan 09 draws a member's (`features::interior::abundances` and
//!   `features::members`): a first population of share [`ClusterParams::first_population`]
//!   with ΔY = 0, and a second whose ΔY is [`ClusterParams::delta_y_max`] × e² with e uniform
//!   on (0, 1]. The second is split into components at the grid's three ΔY at the cluster's Z and
//!   at its largest ΔY, each holding the share of the second population whose ΔY lies nearest it.
//! - **Only the stars near the horizontal branch** are built: for each component, the initial
//!   masses whose branch spans the cluster's age at the median Reimers η are found by bisection,
//!   widened by [`ClusterParams::window_margin`] of the window's width on each side, and the
//!   stars the mass function puts there are placed at evenly spaced ranks of it. Each star's η is
//!   the standard normal's quantile at a rank of the golden-ratio sequence, so that masses and η
//!   cover their ranges evenly without a random stream. A star counts if it is burning helium in
//!   its core at the cluster's age.
//! - **The blue end** of a component is the [`ClusterParams::blue_end_quantile`] quantile of its
//!   branch stars' log₁₀ `T_eff`.
//! - **The grid's amount.** Each branch star is also placed on `BaSTI`'s zero-age horizontal branch
//!   of its component's Y at the cluster's Z, at its own envelope mass (held at the branch's ends
//!   beyond them), and the same quantile taken: the blue end that the grid's branch gives the
//!   same envelopes. The grid's amount is that blue end's rise from the first population to the
//!   grid's top ΔY, and the cluster's is its own; the acceptance holds the two within
//!   [`FitParams::hb_tolerance`](super::fit::FitParams::hb_tolerance) of the grid's.

use hyperion_sim::galaxy::imf::{MASS_LIMIT_HI, MASS_LIMIT_LO, MassFunction, MassFunctionKind};
use hyperion_sim::math;
use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use hyperion_sim::stellar::sse::{CandidateHeliumTable, Track, track_with_helium_table};
use hyperion_sim::stellar::{Composition, Phase};
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses, Years};

use super::grid::{CarriedBranch, Family, Grid};
use crate::manifest::{Manifest, ManifestParamError};

/// The golden ratio's fractional part, (√5 − 1) ÷ 2: the step of the η ranks.
const GOLDEN: f64 = 0.618_033_988_749_894_9;

/// The bisections that find a phase's boundary on a track: the age to 2⁻⁶⁰ of the track's life.
const PHASE_BISECTIONS: u32 = 60;

/// The initial masses the window search brackets, M☉. A window it does not bracket, the age of
/// the branch at both ends on the same side of the cluster's, is no window.
const SEARCH_MASSES_MSUN: (f64, f64) = (0.5, 1.5);

/// The most stars a component may build: well past the few hundred that 10⁶ M☉ puts near the
/// branch, and an exact `f64` count.
const MAX_STARS: u64 = 1_000_000;

/// The synthetic cluster's parameters, every one its manifest's.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterParams {
    /// The mass of stars formed, M☉ (`cluster_mass`; plan 15: 10⁶).
    pub mass_msun: f64,
    /// The age, years (`cluster_age`).
    pub age_yr: f64,
    /// The metal fraction Z (`cluster_z`).
    pub z: f64,
    /// The largest ΔY (`cluster_delta_y_max`; plan 09's `HELIUM_EXCESS_MAX`, 0.18, at 10⁶ M☉).
    pub delta_y_max: f64,
    /// The first population's share (`cluster_first_population`; plan 09's
    /// `first_population_share` at 10⁶ M☉, 0.32).
    pub first_population: f64,
    /// How far each component's mass window is widened on each side, in its width
    /// (`cluster_window_margin`).
    pub window_margin: f64,
    /// The bisections of each end of a component's mass window (`cluster_window_bisections`).
    pub window_bisections: u32,
    /// The quantile of log₁₀ `T_eff` that is a component's blue end (`blue_end_quantile`).
    pub blue_end_quantile: f64,
}

impl ClusterParams {
    /// Reads the cluster's parameters from `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] for a parameter that is missing or out of its range.
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let positive = |key: &str, hi: f64| -> Result<f64, ManifestParamError> {
            let value = manifest.f64(key)?;
            if value > 0.0 && value <= hi {
                Ok(value)
            } else {
                Err(ManifestParamError::new(
                    key,
                    &format!("a number above 0 and at most {hi}"),
                ))
            }
        };
        let bisections = manifest.u64("cluster_window_bisections")?;
        let window_bisections = u32::try_from(bisections)
            .ok()
            .filter(|b| (1..=100).contains(b))
            .ok_or_else(|| ManifestParamError::new("cluster_window_bisections", "1 to 100"))?;
        let first_population = positive("cluster_first_population", 1.0)?;
        if first_population >= 1.0 {
            return Err(ManifestParamError::new(
                "cluster_first_population",
                "below 1: the cluster has a second population",
            ));
        }
        Ok(Self {
            mass_msun: positive("cluster_mass", 1e8)?,
            age_yr: positive("cluster_age", 1.4e10)?,
            z: positive("cluster_z", 0.03)?,
            delta_y_max: positive("cluster_delta_y_max", 0.5)?,
            first_population,
            window_margin: positive("cluster_window_margin", 10.0)?,
            window_bisections,
            blue_end_quantile: positive("blue_end_quantile", 1.0)?,
        })
    }

    /// The sim's \[Fe/H\] of the cluster: log₁₀(Z ÷ 0.02).
    #[must_use]
    pub fn fe_h(&self) -> f64 {
        math::log10(self.z / super::grid::SIM_Z_SOLAR)
    }
}

/// One population of the cluster: its ΔY, its share of the stars, and the grid's family of its
/// branch, if the grid has one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Component {
    /// Its helium excess.
    pub delta_y: f64,
    /// Its share of the cluster's stars.
    pub share: f64,
    /// The grid's family for its branch: the canonical sets for the first population, the
    /// helium-enhanced sets of one Y, or `None` beyond the grid.
    pub family: Option<Family>,
}

/// The cluster's components: the first population, then the second split at the grid's
/// helium-enhanced Y at the cluster's Z (ascending), then at `delta_y_max`, each with the share of
/// the second population whose ΔY = `delta_y_max` × e² lies nearest it.
#[must_use]
pub fn components(grid: &Grid, params: &ClusterParams) -> Option<Vec<Component>> {
    let y0 = grid.canonical_y(params.z)?;
    let mut ys: Vec<f64> = grid
        .sets()
        .iter()
        .filter(|s| !s.is_canonical())
        .map(|s| s.y)
        .collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup_by(|a, b| a.total_cmp(b).is_eq());
    let mut second: Vec<(f64, Option<Family>)> = ys
        .iter()
        .map(|&y| (y - y0, Some(Family::Enhanced { y })))
        .filter(|&(dy, _)| dy > 0.0 && dy < params.delta_y_max)
        .collect();
    second.push((params.delta_y_max, None));
    // P(ΔY ≤ d) = √(d ÷ ΔY_max) for e uniform.
    let below = |d: f64| (d / params.delta_y_max).clamp(0.0, 1.0).sqrt();
    let mut out = vec![Component {
        delta_y: 0.0,
        share: params.first_population,
        family: Some(Family::Canonical),
    }];
    for (i, &(dy, family)) in second.iter().enumerate() {
        let lo = if i == 0 {
            0.0
        } else {
            f64::midpoint(second[i - 1].0, dy)
        };
        let hi = second
            .get(i + 1)
            .map_or(params.delta_y_max, |next| f64::midpoint(dy, next.0));
        out.push(Component {
            delta_y: dy,
            share: (1.0 - params.first_population) * (below(hi) - below(lo)),
            family,
        });
    }
    Some(out)
}

/// The first age (years since the onset of collapse) at which `track`'s phase is past every
/// phase of `before`, by bisection on its life: the phases run in order, so the boundary is one
/// age.
fn boundary_yr(track: &Track, before: &[Phase]) -> Option<f64> {
    let end = track.lifetime()?.value();
    let inside = |age: f64| before.contains(&track.state_at(Years::new(age)).phase());
    let (mut lo, mut hi) = (0.0, end);
    if !inside(lo) {
        return Some(0.0);
    }
    for _ in 0..PHASE_BISECTIONS {
        let mid = f64::midpoint(lo, hi);
        if inside(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(hi)
}

/// The phases before the horizontal branch, in the order a low-mass star passes them.
const BEFORE_BRANCH: [Phase; 5] = [
    Phase::Protostar,
    Phase::PreMainSequence,
    Phase::MainSequence,
    Phase::HertzsprungGap,
    Phase::FirstGiantBranch,
];

/// The phases up to the end of the main sequence.
const UP_TO_MAIN_SEQUENCE: [Phase; 3] = [
    Phase::Protostar,
    Phase::PreMainSequence,
    Phase::MainSequence,
];

/// The start and end of the main sequence on `track`, years since the onset of collapse.
#[must_use]
pub fn main_sequence_years(track: &Track) -> Option<(f64, f64)> {
    let start = boundary_yr(track, &UP_TO_MAIN_SEQUENCE[..2])?;
    let end = boundary_yr(track, &UP_TO_MAIN_SEQUENCE)?;
    (end > start).then_some((start, end))
}

/// The start and end of the horizontal branch (core helium burning) on `track`, years since the
/// onset of collapse, or `None` if the star has none.
#[must_use]
pub fn horizontal_branch_years(track: &Track) -> Option<(f64, f64)> {
    let start = boundary_yr(track, &BEFORE_BRANCH)?;
    if track.state_at(Years::new(start)).phase() != Phase::CoreHeliumBurning {
        return None;
    }
    let mut through = BEFORE_BRANCH.to_vec();
    through.push(Phase::CoreHeliumBurning);
    let end = boundary_yr(track, &through)?;
    Some((start, end))
}

/// Draws whose Reimers η is the standard normal's quantile `z`, every other draw the median.
fn draws_with_eta(z: f64) -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        eta: StandardNormal::new(z).expect("a quantile of (0, 1) is finite"),
        ..StarDrawsParts::MEDIAN
    })
}

/// The initial mass, M☉, by bisection in `SEARCH_MASSES_MSUN`, at which `age_of` (years,
/// decreasing in mass, infinite for a star with no branch) reaches `age_yr`; `None` unless the
/// bracket's two ends lie on either side of it.
fn initial_mass_at(
    age_yr: f64,
    bisections: u32,
    mut age_of: impl FnMut(f64) -> f64,
) -> Option<f64> {
    let (mut lo, mut hi) = SEARCH_MASSES_MSUN;
    if !(age_of(lo) > age_yr && age_of(hi) <= age_yr) {
        return None;
    }
    for _ in 0..bisections {
        let mid = f64::midpoint(lo, hi);
        if age_of(mid) > age_yr {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(f64::midpoint(lo, hi))
}

/// One star on the horizontal branch at the cluster's age.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BranchStar {
    /// log₁₀ `T_eff` ÷ K.
    pub log_teff: f64,
    /// Its envelope mass, M☉.
    pub envelope_msun: f64,
}

/// One component's measurements.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentFigures {
    /// The component.
    pub component: Component,
    /// The initial masses built, M☉: the branch's window at the median η, widened; `None` if the
    /// search found none.
    pub window_msun: Option<(f64, f64)>,
    /// The stars built.
    pub built: u64,
    /// The stars on the branch at the cluster's age.
    pub branch: Vec<BranchStar>,
    /// The blue end, log₁₀ `T_eff`, or `None` without branch stars.
    pub blue_end: Option<f64>,
    /// The grid's blue end for the same envelopes, or `None` beyond the grid.
    pub grid_blue_end: Option<f64>,
    /// Plan 06's helium core mass at the helium flash, M☉, of the median-η star at the middle of
    /// the unwidened window, or `None` without a window.
    pub ignition_core_msun: Option<f64>,
    /// The core mass of the grid's zero-age branch of the component's Y at the cluster's Z, M☉,
    /// or `None` beyond the grid.
    pub grid_core_msun: Option<f64>,
}

/// The `rank` quantile (0–1) of `values`, linear between order statistics; `None` if there are
/// none or `rank` is not a number.
#[must_use]
pub fn quantile(mut values: Vec<f64>, rank: f64) -> Option<f64> {
    if values.is_empty() || rank.is_nan() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let last = values.len() - 1;
    // A count of a few thousand stars: the index's conversion to f64 and back is exact.
    #[expect(
        clippy::cast_precision_loss,
        reason = "an index of a few thousand values, exact in an f64"
    )]
    let position = rank.clamp(0.0, 1.0) * last as f64;
    let below = position.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a finite position in 0 ..= len − 1, an index of the sorted values"
    )]
    let index = below as usize;
    let next = values.get(index + 1).copied().unwrap_or(values[index]);
    Some(values[index] + (position - below) * (next - values[index]))
}

/// The cluster's measurements under a table.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterFigures {
    /// Each component's, in [`components`]' order, the first population first.
    pub components: Vec<ComponentFigures>,
    /// The index of the grid's top ΔY among them: the largest with a grid family.
    pub top: usize,
}

impl ClusterFigures {
    /// The rise of the blue end from the first population to the grid's top ΔY, dex: the
    /// cluster's and the grid's, or `None` if either lacks a blue end.
    #[must_use]
    pub fn heating(&self) -> Option<(f64, f64)> {
        let (first, top) = (self.components.first()?, self.components.get(self.top)?);
        Some((
            top.blue_end? - first.blue_end?,
            top.grid_blue_end? - first.grid_blue_end?,
        ))
    }
}

/// One component of the cluster, measured: its window, its branch stars and their blue ends.
fn measure(
    grid: &Grid,
    table: &CandidateHeliumTable,
    params: &ClusterParams,
    component: &Component,
    imf: &dyn MassFunction,
    stars_per_integral: f64,
) -> ComponentFigures {
    let composition = Composition::from_fe_h(
        Dex::new(params.fe_h()),
        HeliumExcess::new(component.delta_y),
    );
    let track = |m: f64, draws: &StarDraws| {
        track_with_helium_table(SolarMasses::new(m), &composition, draws, table)
    };
    let median = draws_with_eta(0.0);
    let branch_of = |m: f64| horizontal_branch_years(&track(m, &median));
    // The lightest star on the branch at the cluster's age has just reached it; the heaviest is
    // just leaving it.
    let lightest = initial_mass_at(params.age_yr, params.window_bisections, |m| {
        branch_of(m).map_or(f64::INFINITY, |(start, _)| start)
    });
    let heaviest = initial_mass_at(params.age_yr, params.window_bisections, |m| {
        branch_of(m).map_or(f64::INFINITY, |(_, end)| end)
    });
    let window_msun = match (lightest, heaviest) {
        (Some(lo), Some(hi)) if hi > lo => {
            let margin = params.window_margin * (hi - lo);
            Some((lo - margin, hi + margin))
        }
        _ => None,
    };
    let expected = window_msun.map_or(0.0, |(lo, hi)| {
        stars_per_integral * component.share * imf.integral(lo, hi)
    });
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "held to 0 ..= MAX_STARS before the cast, so the count is exact"
    )]
    let built = expected.round().clamp(0.0, 1e6) as u64;
    let built = built.min(MAX_STARS);
    let mut branch = Vec::new();
    if let Some((lo, hi)) = window_msun {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count of at most MAX_STARS, exact in an f64"
        )]
        let n = built as f64;
        for i in 0..built {
            #[expect(
                clippy::cast_precision_loss,
                reason = "an index below MAX_STARS, exact in an f64"
            )]
            let k = i as f64;
            let m = imf.quantile_in(lo, hi, (k + 0.5) / n);
            let eta_rank = ((k + 0.5) * GOLDEN).fract();
            let state = track(m, &draws_with_eta(math::normal_quantile(eta_rank)))
                .state_at(Years::new(params.age_yr));
            if state.phase() == Phase::CoreHeliumBurning {
                branch.push(BranchStar {
                    log_teff: math::log10(state.effective_temperature().value()),
                    envelope_msun: state.envelope_mass().value(),
                });
            }
        }
    }
    let blue_end = quantile(
        branch.iter().map(|s| s.log_teff).collect(),
        params.blue_end_quantile,
    );
    let carried = component
        .family
        .and_then(|family| grid.carried_branch(family, params.z));
    let grid_blue_end = carried.as_ref().and_then(|carried| {
        let mapped: Option<Vec<f64>> = branch
            .iter()
            .map(|s| carried.log_teff_held(s.envelope_msun))
            .collect();
        quantile(mapped?, params.blue_end_quantile)
    });
    let ignition_core_msun = match (lightest, heaviest) {
        (Some(lo), Some(hi)) => {
            let star = track(f64::midpoint(lo, hi), &median);
            horizontal_branch_years(&star)
                .map(|(start, _)| star.state_at(Years::new(start)).core_mass().value())
        }
        _ => None,
    };
    ComponentFigures {
        component: *component,
        window_msun,
        built,
        branch,
        blue_end,
        grid_blue_end,
        ignition_core_msun,
        grid_core_msun: carried.as_ref().and_then(CarriedBranch::core_mass_msun),
    }
}

/// The synthetic cluster of `params` on plan 06's tracks under `table`, measured against `grid`.
#[must_use]
pub fn synthesise(
    grid: &Grid,
    table: &CandidateHeliumTable,
    params: &ClusterParams,
) -> Option<ClusterFigures> {
    let parts = components(grid, params)?;
    let imf = MassFunctionKind::default().to_mass_function();
    // The stars formed per unit of the mass function's integral.
    let stars_per_integral = params.mass_msun
        / mean_mass_msun(imf.as_ref())
        / imf.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
    let components: Vec<ComponentFigures> = parts
        .iter()
        .map(|c| measure(grid, table, params, c, imf.as_ref(), stars_per_integral))
        .collect();
    let top = components
        .iter()
        .rposition(|c| c.component.family.is_some())?;
    Some(ClusterFigures { components, top })
}

/// The mass function's mean mass, M☉: ∫ m ξ dm ÷ ∫ ξ dm over its stellar range.
#[must_use]
pub fn mean_mass_msun(imf: &dyn MassFunction) -> f64 {
    let (count, mass) = moments(imf);
    mass / count
}

/// ∫ ξ dm and ∫ m ξ dm over the stellar range, by Simpson's rule in ln m on 2,048 panels between
/// each pair of the function's breaks.
fn moments(imf: &dyn MassFunction) -> (f64, f64) {
    moments_on(imf, 2_048)
}

/// [`moments`] on `panels` panels a piece.
fn moments_on(imf: &dyn MassFunction, panels: u32) -> (f64, f64) {
    let mut edges = vec![MASS_LIMIT_LO];
    edges.extend(
        imf.breaks()
            .iter()
            .copied()
            .filter(|&b| b > MASS_LIMIT_LO && b < MASS_LIMIT_HI),
    );
    edges.push(MASS_LIMIT_HI);
    let (mut mass, mut count) = (0.0, 0.0);
    for pair in edges.windows(2) {
        let (a, b) = (math::ln(pair[0]), math::ln(pair[1]));
        let h = (b - a) / f64::from(panels);
        for j in 0..=panels {
            let weight = if j == 0 || j == panels {
                1.0
            } else if j % 2 == 1 {
                4.0
            } else {
                2.0
            };
            // A piece's ends are read just inside it: `exp(ln m)` may round out of the stellar
            // range, where ξ is 0, and ξ may jump at a break.
            let m = if j == 0 {
                pair[0] * (1.0 + f64::EPSILON)
            } else if j == panels {
                pair[1] * (1.0 - f64::EPSILON)
            } else {
                math::exp(a + h * f64::from(j))
            };
            // dm = m d(ln m).
            let density = imf.pdf(m) * m;
            mass += weight * density * m * h / 3.0;
            count += weight * density * h / 3.0;
        }
    }
    (count, mass)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_sim::galaxy::features::interior::abundances::{
        HELIUM_EXCESS_MAX, first_population_share, helium_excess_max,
    };

    #[test]
    fn the_quantile_is_linear_between_order_statistics() {
        assert_eq!(quantile(Vec::new(), 0.5), None);
        assert_eq!(quantile(vec![1.0], f64::NAN), None);
        assert_eq!(quantile(vec![3.0, 1.0, 2.0], 0.5), Some(2.0));
        assert_eq!(quantile(vec![3.0, 1.0, 2.0], 1.0), Some(3.0));
        assert_eq!(quantile(vec![1.0, 2.0], 0.25), Some(1.25));
    }

    /// The quadrature integrates the mass function's count to its exact integral, to 10⁻⁹, and the
    /// mean mass is Chabrier's (2003, PASP 115, 763, eq. 18 and Table 1) system function's at the
    /// provisional high-mass scale over 0.08–150 M☉: 0.771 M☉.
    #[test]
    fn the_mean_mass_is_the_mass_functions_first_moment() {
        let imf = MassFunctionKind::default().to_mass_function();
        let (count, _) = moments(imf.as_ref());
        let exact = imf.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
        assert!(
            (count / exact - 1.0).abs() < 1e-9,
            "{count} against {exact}"
        );
        // Converged: four times the panels moves the mean by under 10⁻¹².
        let (finer_count, finer_mass) = moments_on(imf.as_ref(), 8_192);
        let mean = mean_mass_msun(imf.as_ref());
        assert!((mean - finer_mass / finer_count).abs() < 1e-12, "{mean}");
        assert!((mean - 0.771).abs() < 5e-4, "{mean}");
    }

    /// The manifest's cluster draws plan 09's populations at 10⁶ M☉.
    #[test]
    fn the_production_cluster_is_plan_nines_at_its_mass() {
        let text = include_str!("../../../manifests/helium.production.toml");
        let manifest = Manifest::from_bytes(
            std::path::Path::new("helium.production.toml"),
            text.as_bytes().to_vec(),
        )
        .unwrap();
        let params = ClusterParams::from_manifest(&manifest).unwrap();
        let mass = SolarMasses::new(params.mass_msun);
        assert!((params.first_population - first_population_share(mass)).abs() < 1e-12);
        assert!((params.delta_y_max - helium_excess_max(mass)).abs() < 1e-12);
        assert!((params.delta_y_max - HELIUM_EXCESS_MAX).abs() < 1e-15);
    }

    #[test]
    fn the_components_share_the_cluster() {
        let turnoff = "z_row,z,y,mass,log_age_ms_end\n\
            0.001,0.0008,0.4000,0.80,9.6\n\
            0.001,0.00092,0.3000,0.80,9.9\n\
            0.001,0.001,0.2460,0.80,10.1\n\
            0.002,0.002,0.2480,0.80,10.2\n";
        let zahb = "z_row,z,y,core_mass,hb_mass,envelope_mass,log_l,log_teff\n";
        let grid = Grid::parse(turnoff, zahb).unwrap();
        let params = ClusterParams {
            mass_msun: 1e6,
            age_yr: 12e9,
            z: 0.001,
            delta_y_max: 0.18,
            first_population: 0.32,
            window_margin: 0.5,
            window_bisections: 30,
            blue_end_quantile: 0.95,
        };
        let parts = components(&grid, &params).unwrap();
        assert_eq!(parts.len(), 4);
        let total: f64 = parts.iter().map(|c| c.share).sum();
        assert!((total - 1.0).abs() < 1e-12, "{parts:?}");
        assert!((parts[1].delta_y - 0.054).abs() < 1e-12);
        assert!((parts[2].delta_y - 0.154).abs() < 1e-12);
        assert_eq!(parts[0].family, Some(Family::Canonical));
        assert_eq!(parts[3].family, None);
        assert!(parts.windows(2).all(|w| w[0].delta_y < w[1].delta_y));
    }

    /// A bracket that the age does not cross is no window.
    #[test]
    fn a_window_outside_the_search_is_none() {
        assert_eq!(initial_mass_at(1e10, 30, |_| 1e9), None);
        assert_eq!(initial_mass_at(1e10, 30, |_| f64::INFINITY), None);
        let m = initial_mass_at(1e10, 50, |m| 1e10 * math::powf(m, -3.0)).unwrap();
        assert!((m - 1.0).abs() < 1e-12, "{m}");
    }
}
