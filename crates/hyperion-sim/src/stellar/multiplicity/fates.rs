//! Plan 11's companions on plan 02's seam (P11.T1.d): [`MultiplicityFates`], the fates the
//! galaxy's mean mass per system reads, with plan 06's lifetimes and remnants
//! ([`TrackFates`]) and the companions the hierarchy draw gives.
//!
//! The companions' count is [`MultiplicityModel::drawn_companion_frequency`], and their mass
//! ratios [`MultiplicityModel::drawn_companion_mass_ratio_cdfs`], which marginalises the law over
//! the period with a pass over the periods for each primary. Plan 02's quadrature asks for the
//! distribution at thousands of primaries, so it is tabulated once ([`CompanionLaw`]) at the
//! [`TrackFates`] grid's masses and at fixed mass ratios, and interpolated.

use std::sync::OnceLock;

use super::MultiplicityModel;
use super::dist::{MOE_DI_STEFANO_MIN_MASS, TWIN_MIN_MASS_RATIO};
use crate::galaxy::fates::StellarFates;
use crate::galaxy::imf::MASS_LIMIT_LO;
use crate::math;
use crate::stellar::fates::{TrackFates, track_fates_grid};
use crate::units::{SolarMasses, Years};

/// The spacing of [`CompanionLaw`]'s mass ratios in ln q, from Moe and Di Stefano's range up.
pub const COMPANION_LAW_LN_RATIO_STEP: f64 = 1.0 / 16.0;

/// The columns of [`CompanionLaw`] below Moe and Di Stefano's range: even steps of the companion's
/// place in its own range of ln q, from the lightest companion to a twin.
pub const COMPANION_LAW_LOW_COLUMNS: u32 = 64;

/// The mass ratios [`CompanionLaw`] holds besides its even steps in ln q: the laws' breaks, where
/// the cumulative distribution has a kink (0.1 and 0.3, Moe and Di Stefano's; 0.95, their twins').
const RATIO_KINKS: [f64; 3] = [0.1, 0.3, TWIN_MIN_MASS_RATIO];

/// One part of [`CompanionLaw`]: rows by ln m₁, columns by a coordinate of q, the cumulative
/// distribution row after row.
#[derive(Debug, Clone, PartialEq)]
struct LawPart {
    ln_masses: Vec<f64>,
    columns: Vec<f64>,
    cdf: Vec<f64>,
}

impl LawPart {
    /// The value at `ln_m` and column coordinate `x`, bilinear.
    #[must_use]
    fn at(&self, ln_m: f64, x: f64) -> f64 {
        let (j, u) = locate(&self.columns, x);
        self.mix(locate(&self.ln_masses, ln_m), (j, u))
    }

    /// The bilinear value in row segment `row` and column segment `col`, each an index and the
    /// weight of the node above it.
    #[must_use]
    fn mix(&self, (row, down): (usize, f64), (col, across): (usize, f64)) -> f64 {
        let width = self.columns.len();
        let at = |r: usize, c: usize| self.cdf[r * width + c];
        let along = |r: usize| at(r, col) + across * (at(r, col + 1) - at(r, col));
        (along(row) + down * (along(row + 1) - along(row))).clamp(0.0, 1.0)
    }
}

/// [`locate`] of each of the ascending `xs` in `nodes`, one sweep: the same segments and weights.
struct Sweep<'n> {
    nodes: &'n [f64],
    /// The first node above the last `x`, as `partition_point` finds it.
    above: usize,
}

impl<'n> Sweep<'n> {
    #[must_use]
    fn new(nodes: &'n [f64]) -> Self {
        Self { nodes, above: 0 }
    }

    /// [`locate`] of `x`, which is no less than the last `x` asked.
    fn locate(&mut self, x: f64) -> (usize, f64) {
        let last = self.nodes.len() - 1;
        let x = x.clamp(self.nodes[0], self.nodes[last]);
        while self.above < self.nodes.len() && self.nodes[self.above] <= x {
            self.above += 1;
        }
        let upper = self.above.clamp(1, last);
        let (lo, hi) = (self.nodes[upper - 1], self.nodes[upper]);
        (upper - 1, (x - lo) / (hi - lo))
    }
}

/// The distribution of the mass ratio of the companions the hierarchy draw gives, tabulated for
/// plan 02's quadrature (P11.T1.d).
///
/// It holds [`MultiplicityModel::drawn_companion_mass_ratio_cdfs`] at the masses of
/// [`track_fates_grid`] and at Moe and Di Stefano's lowest mass, 0.8 M☉, where the law changes
/// from Duchêne and Kraus's single slope to theirs; each side of it is its own part, the lower
/// holding the limit from below. Below 0.8 M☉ the columns are
/// [`COMPANION_LAW_LOW_COLUMNS`] even steps of `ln(q ÷ q_min) ÷ ln(1 ÷ q_min)`, the companion's
/// place in its own range from `q_min` = 0.08 M☉ ÷ m₁, so that the range's lower end, which moves
/// with m₁, is a column; from 0.8 M☉ up they are every [`COMPANION_LAW_LN_RATIO_STEP`] in ln q from
/// 0.08 M☉ ÷ 150 M☉ to 1, with the laws' breaks (0.1, 0.3, 0.95) among them. Each part is
/// interpolated linearly in ln m₁ and its column. Every mass of the grid is a kink of the
/// interpolant, and so a break of [`MultiplicityFates`], as it is of [`TrackFates`].
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionLaw {
    low: LawPart,
    high: LawPart,
}

impl CompanionLaw {
    /// The table of `model`'s drawn companions.
    #[must_use]
    pub fn new(model: &MultiplicityModel) -> Self {
        let grid = track_fates_grid();
        let switch = MOE_DI_STEFANO_MIN_MASS;
        // Below: the grid's masses under 0.8 M☉ and the limit from below at it.
        let mut low_masses: Vec<f64> = grid.iter().copied().filter(|&m| m < switch).collect();
        low_masses.push(switch);
        let steps = f64::from(COMPANION_LAW_LOW_COLUMNS);
        let low_columns: Vec<f64> = (0..=COMPANION_LAW_LOW_COLUMNS)
            .map(|k| f64::from(k) / steps)
            .collect();
        let mut low_cdf = Vec::with_capacity(low_masses.len() * low_columns.len());
        for &m in &low_masses {
            // The limits at the part's ends are the laws just inside them: at 0.08 M☉ the range
            // of q shrinks to nothing, and at 0.8 M☉ the law changes.
            let at = if m >= switch {
                switch * (1.0 - 1e-12)
            } else {
                m.max(MASS_LIMIT_LO * (1.0 + 1e-9))
            };
            let ln_lowest = math::ln(MASS_LIMIT_LO / at).min(0.0);
            let qs: Vec<f64> = low_columns
                .iter()
                .map(|&t| math::exp((1.0 - t) * ln_lowest))
                .collect();
            low_cdf.extend(model.drawn_companion_mass_ratio_cdfs(SolarMasses::new(at), &qs));
        }
        // From 0.8 M☉ up.
        let mut high_masses = vec![switch];
        high_masses.extend(grid.iter().copied().filter(|&m| m > switch));
        let lowest = math::ln(MASS_LIMIT_LO / grid[grid.len() - 1]);
        let mut ratios: Vec<f64> = Vec::new();
        let mut k = 0_u32;
        loop {
            let ln_q = lowest + COMPANION_LAW_LN_RATIO_STEP * f64::from(k);
            if ln_q >= 0.0 {
                break;
            }
            ratios.push(math::exp(ln_q));
            k += 1;
        }
        ratios.extend(RATIO_KINKS);
        ratios.push(1.0);
        ratios.sort_by(f64::total_cmp);
        ratios.dedup();
        let mut high_cdf = Vec::with_capacity(high_masses.len() * ratios.len());
        for &m in &high_masses {
            high_cdf.extend(model.drawn_companion_mass_ratio_cdfs(SolarMasses::new(m), &ratios));
        }
        Self {
            low: LawPart {
                ln_masses: low_masses.iter().map(|&m| math::ln(m)).collect(),
                columns: low_columns,
                cdf: low_cdf,
            },
            high: LawPart {
                ln_masses: high_masses.iter().map(|&m| math::ln(m)).collect(),
                columns: ratios.iter().map(|&q| math::ln(q)).collect(),
                cdf: high_cdf,
            },
        }
    }

    /// The generator's table: the default model's, built once and shared.
    #[must_use]
    pub fn generator() -> &'static Self {
        static LAW: OnceLock<CompanionLaw> = OnceLock::new();
        LAW.get_or_init(|| Self::new(&MultiplicityModel::default_v1()))
    }

    /// The interpolated probability that a companion of a primary of `m1` has a mass ratio of
    /// at most `q`: 0 below the lightest companion, 1 from q = 1 up.
    #[must_use]
    pub fn cdf(&self, m1: SolarMasses, q: f64) -> f64 {
        let m1 = m1.value();
        if q >= 1.0 {
            return 1.0;
        }
        let ln_lowest = math::ln(MASS_LIMIT_LO / m1);
        if q <= 0.0 || ln_lowest >= 0.0 || math::ln(q) <= ln_lowest {
            return 0.0;
        }
        if m1 < MOE_DI_STEFANO_MIN_MASS {
            let place = 1.0 - math::ln(q) / ln_lowest;
            self.low.at(math::ln(m1), place)
        } else {
            self.high.at(math::ln(m1), math::ln(q))
        }
    }

    /// [`cdf`](Self::cdf) at each of the ascending ratios `qs`, into `out`, bit for bit, sweeping
    /// the columns once.
    pub fn cdfs(&self, m1: SolarMasses, qs: &[f64], out: &mut [f64]) {
        let m1 = m1.value();
        let ln_lowest = math::ln(MASS_LIMIT_LO / m1);
        let low = m1 < MOE_DI_STEFANO_MIN_MASS;
        let part = if low { &self.low } else { &self.high };
        let row = locate(&part.ln_masses, math::ln(m1));
        let mut columns = Sweep::new(&part.columns);
        for (value, &q) in out.iter_mut().zip(qs) {
            *value = if q >= 1.0 {
                1.0
            } else if q <= 0.0 || ln_lowest >= 0.0 || math::ln(q) <= ln_lowest {
                0.0
            } else if low {
                part.mix(row, columns.locate(1.0 - math::ln(q) / ln_lowest))
            } else {
                part.mix(row, columns.locate(math::ln(q)))
            };
        }
    }
}

/// The segment of the ascending `nodes` holding `x` (clamped to them) and the weight of its upper
/// node.
#[must_use]
fn locate(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 1;
    let x = x.clamp(nodes[0], nodes[last]);
    let upper = nodes.partition_point(|&node| node <= x).clamp(1, last);
    let (lo, hi) = (nodes[upper - 1], nodes[upper]);
    (upper - 1, (x - lo) / (hi - lo))
}

/// Plan 06's fates with plan 11's companions (P11.T1.d): what plan 06's
/// [`fates_for`](crate::galaxy::fates::fates_for) returns for every population.
///
/// Lifetimes and remnant masses are [`TrackFates`]' at the population's reference metallicity;
/// the mean number of stellar companions is
/// [`MultiplicityModel::drawn_companion_frequency`] and their mass ratios [`CompanionLaw`]'s. The
/// companions do not depend on the metallicity, so every value of this type has the same.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::Population;
/// use hyperion_sim::galaxy::fates::{StellarFates, fates_for};
///
/// let disc = fates_for(Population::OldThinDisc);
/// // A Sun-like primary has about 0.6 stellar companions, an O star over two.
/// assert!((0.55..0.7).contains(&disc.mean_companions(1.0)));
/// assert!(disc.mean_companions(40.0) > 2.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MultiplicityFates {
    track: TrackFates,
    model: MultiplicityModel,
    law: &'static CompanionLaw,
    breaks: Vec<f64>,
}

impl MultiplicityFates {
    /// The fates of `track` with the default model's companions.
    #[must_use]
    pub fn new(track: TrackFates) -> Self {
        let model = MultiplicityModel::default_v1();
        let mut breaks: Vec<f64> = track
            .breaks()
            .iter()
            .copied()
            .chain(model.drawn_kinks())
            .filter(|&m| m > MASS_LIMIT_LO && m < crate::galaxy::imf::MASS_LIMIT_HI)
            .collect();
        breaks.sort_by(f64::total_cmp);
        breaks.dedup();
        Self {
            track,
            model,
            law: CompanionLaw::generator(),
            breaks,
        }
    }

    /// The lifetimes and remnants.
    #[must_use]
    pub fn track(&self) -> &TrackFates {
        &self.track
    }
}

impl StellarFates for MultiplicityFates {
    fn lifetime(&self, m: f64) -> Years {
        self.track.lifetime(m)
    }

    fn remnant_mass(&self, m: f64) -> f64 {
        self.track.remnant_mass(m)
    }

    fn mean_companions(&self, m: f64) -> f64 {
        self.model
            .drawn_companion_frequency(SolarMasses::new(m.max(MASS_LIMIT_LO)))
    }

    fn companion_mass_ratio_cdf(&self, m1: f64, q: f64) -> f64 {
        self.law.cdf(SolarMasses::new(m1), q)
    }

    fn companion_mass_ratio_cdfs(&self, m1: f64, qs: &[f64], out: &mut [f64]) {
        self.law.cdfs(SolarMasses::new(m1), qs, out);
    }

    fn breaks(&self) -> &[f64] {
        &self.breaks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table reproduces the model at its own nodes and stays within a few 10⁻³ of it between
    /// them, at primaries across the stellar range.
    #[test]
    fn the_companion_law_follows_the_model() {
        let model = MultiplicityModel::default_v1();
        let law = CompanionLaw::generator();
        let mut worst: f64 = 0.0;
        for m in [0.09, 0.3, 0.77, 1.0, 1.9, 2.6, 5.0, 11.0, 40.0, 140.0] {
            let qs: Vec<f64> = (1..40)
                .map(|k| f64::from(k) / 40.0)
                .filter(|&q| q * m >= MASS_LIMIT_LO)
                .collect();
            let exact = model.drawn_companion_mass_ratio_cdfs(SolarMasses::new(m), &qs);
            for (&q, &e) in qs.iter().zip(&exact) {
                let err = (law.cdf(SolarMasses::new(m), q) - e).abs();
                if err > 5e-3 {
                    println!(
                        "{m} M☉, q {q}: table {} model {e}",
                        law.cdf(SolarMasses::new(m), q)
                    );
                }
                worst = worst.max(err);
            }
        }
        println!("the companion law's worst error: {worst:.2e}");
        assert!(worst < 5e-3, "{worst}");
    }

    /// The shared table is a pure function of constants: a fresh build equals it.
    #[test]
    fn the_shared_law_is_a_fresh_build() {
        assert_eq!(
            CompanionLaw::generator(),
            &CompanionLaw::new(&MultiplicityModel::default_v1())
        );
    }

    /// The batch sweep gives the single lookups' values, bit for bit.
    #[test]
    fn the_sweep_is_the_lookup() {
        let law = CompanionLaw::generator();
        let qs: Vec<f64> = (0..=400)
            .map(|k| math::exp(-8.0 + 0.021 * f64::from(k)))
            .collect();
        let mut out = vec![0.0; qs.len()];
        for m in [0.08, 0.1, 0.5, 0.79, 0.8, 1.7, 40.0, 150.0] {
            let m = SolarMasses::new(m);
            law.cdfs(m, &qs, &mut out);
            for (&q, &v) in qs.iter().zip(&out) {
                hyperion_testkit::float::assert_same_bits(v, law.cdf(m, q));
            }
        }
    }

    #[test]
    fn the_companion_law_is_a_distribution() {
        let law = CompanionLaw::generator();
        for m in [0.08, 0.2, 1.0, 3.0, 20.0, 150.0] {
            let mut last = 0.0;
            for k in 0..=200 {
                let q = f64::from(k) / 200.0;
                let f = law.cdf(SolarMasses::new(m), q);
                assert!(f >= last - 1e-15 && f <= 1.0, "{f} at {m} M☉, q {q}");
                last = f;
            }
            assert!((law.cdf(SolarMasses::new(m), 1.0) - 1.0).abs() < f64::EPSILON);
        }
    }
}
