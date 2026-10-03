//! The discs of the observer's own stars (rendering plan R06, Design note 16): their limb darkening,
//! read from the fitted table [`tables::limb_darkening`].
//!
//! The power-2 law, I(μ) ÷ I(1) = 1 − c (1 − μ^α) (Hestroffer 1997, A&A 327, 199; Maxted 2018, A&A
//! 616, A39), with c and α in Johnson B, V and R for the display's blue, green and red, from
//! Claret and Southworth (2022, 2023) for stars and Claret et al. (2020) for white dwarfs. The
//! table is interpolated bilinearly in log₁₀ `T_eff` and log₁₀ g within the grid the star's kind
//! selects and clamped at its edges: at 50,000 K for O stars, at the least gravity tabulated for
//! hot giants, at 2,300 K below, and at 100,000 K for white dwarfs.
//!
//! [`tables::limb_darkening`]: crate::tables::limb_darkening

use crate::math;
use crate::sky::colour::AtmosphereGrid;
use crate::tables::limb_darkening::{
    NORMAL, NORMAL_LOG_G, NORMAL_LOG_TEFF, WHITE_DWARF, WHITE_DWARF_LOG_G, WHITE_DWARF_LOG_TEFF,
};
use crate::units::Kelvin;

/// One row of the fitted limb-darkening table: c and α in B, V and R.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct LimbRow {
    /// c in B.
    pub c_b: f64,
    /// α in B.
    pub alpha_b: f64,
    /// c in V.
    pub c_v: f64,
    /// α in V.
    pub alpha_v: f64,
    /// c in R.
    pub c_r: f64,
    /// α in R.
    pub alpha_r: f64,
}

/// The power-2 limb-darkening law of one band: I(μ) ÷ I(1) = 1 − c (1 − μ^α).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct PowerTwo {
    c: f64,
    alpha: f64,
}

impl PowerTwo {
    /// The law with coefficients `c` and `alpha` (the catalogues' g and h).
    #[must_use]
    pub const fn new(c: f64, alpha: f64) -> Self {
        Self { c, alpha }
    }

    /// The coefficient c: one less the limb's intensity at μ = 0, relative to the centre's.
    #[must_use]
    pub const fn c(&self) -> f64 {
        self.c
    }

    /// The exponent α.
    #[must_use]
    pub const fn alpha(&self) -> f64 {
        self.alpha
    }

    /// The intensity at `mu` (the cosine of the angle from the surface normal, clamped to 0–1)
    /// relative to the centre's.
    #[must_use]
    pub fn intensity(&self, mu: f64) -> f64 {
        1.0 - self.c * (1.0 - math::powf(mu.clamp(0.0, 1.0), self.alpha))
    }

    /// The disc's mean intensity relative to the centre's, ∫ I(μ) 2μ dμ ÷ I(1) = 1 − c α ÷
    /// (α + 2): 0.799 for the Sun in V.
    #[must_use]
    pub fn disc_average(&self) -> f64 {
        1.0 - self.c * self.alpha / (self.alpha + 2.0)
    }
}

/// The power-2 laws of a star of `teff` and `log_g` (log₁₀ g, cgs) in B, V and R, for the
/// display's blue, green and red, interpolated bilinearly in log₁₀ `T_eff` and log₁₀ g within
/// `grid` and clamped at its edges.
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::colour::AtmosphereGrid;
/// use hyperion_sim::sky::disc::limb_coefficients;
/// use hyperion_sim::units::Kelvin;
///
/// // The Sun's disc in V keeps about 80% of its central intensity on average.
/// let [_, v, _] = limb_coefficients(Kelvin::new(5_772.0), 4.438, AtmosphereGrid::MainSequence);
/// assert!((v.disc_average() - 0.799).abs() < 0.005);
/// // The limb is darker in blue than in red.
/// let [b, _, r] = limb_coefficients(Kelvin::new(5_772.0), 4.438, AtmosphereGrid::MainSequence);
/// assert!(b.intensity(0.1) < r.intensity(0.1));
/// ```
#[must_use]
pub fn limb_coefficients(teff: Kelvin, log_g: f64, grid: AtmosphereGrid) -> [PowerTwo; 3] {
    let (log_teff_nodes, log_g_nodes, rows): (&[f64], &[f64], &[LimbRow]) = match grid {
        AtmosphereGrid::MainSequence | AtmosphereGrid::Giant => {
            (&NORMAL_LOG_TEFF, &NORMAL_LOG_G, &NORMAL)
        }
        AtmosphereGrid::WhiteDwarf => (&WHITE_DWARF_LOG_TEFF, &WHITE_DWARF_LOG_G, &WHITE_DWARF),
    };
    let (ti, tf) = bracket(log_teff_nodes, math::log10(teff.value()));
    let (gi, gf) = bracket(log_g_nodes, log_g);
    let width = log_g_nodes.len();
    let at = |a: usize, b: usize| rows[a * width + b];
    let mix = |field: fn(&LimbRow) -> f64| {
        let low = field(&at(ti, gi)) * (1.0 - gf) + field(&at(ti, gi + 1)) * gf;
        let high = field(&at(ti + 1, gi)) * (1.0 - gf) + field(&at(ti + 1, gi + 1)) * gf;
        low * (1.0 - tf) + high * tf
    };
    [
        PowerTwo::new(mix(|r| r.c_b), mix(|r| r.alpha_b)),
        PowerTwo::new(mix(|r| r.c_v), mix(|r| r.alpha_v)),
        PowerTwo::new(mix(|r| r.c_r), mix(|r| r.alpha_r)),
    ]
}

/// The interval of rising `nodes` that holds `x` and the fraction along it, clamped to the first
/// or last interval's end; a NaN takes the first node.
fn bracket(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 2;
    let i = nodes
        .partition_point(|&node| node <= x)
        .saturating_sub(1)
        .min(last);
    let t = (x - nodes[i]) / (nodes[i + 1] - nodes[i]);
    (i, if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun() -> [PowerTwo; 3] {
        limb_coefficients(Kelvin::new(5_772.0), 4.438, AtmosphereGrid::MainSequence)
    }

    /// The solar row in V: c = 0.7837 and α = 0.6893 from Claret and Southworth's (2022) ATLAS
    /// table at 5,772 K and log g 4.44, and a disc average of 0.799 (Design note 16).
    #[test]
    fn the_solar_row_is_claret_and_southworths() {
        let [_, v, _] = sun();
        assert!((v.c() - 0.784).abs() < 0.005, "c {}", v.c());
        assert!((v.alpha() - 0.689).abs() < 0.005, "α {}", v.alpha());
        assert!(
            (v.disc_average() - 0.799).abs() < 0.005,
            "{}",
            v.disc_average()
        );
        assert!((v.intensity(1.0) - 1.0).abs() < 1e-12);
        assert!((v.intensity(0.0) - (1.0 - v.c())).abs() < 1e-12);
    }

    /// At μ = 0.1 the Sun's law in V lies within 0.015 of the observed quadratic fit at 5,522 Å,
    /// I(μ) ÷ I(1) = 0.29462 + 0.98032 μ − 0.27494 μ², which gives 0.390 (Pierce and Slaughter
    /// 1977, Solar Phys. 51, 25, Table III, eq. 10; the source of Cox 2000's polynomial in
    /// *Allen's Astrophysical Quantities*, 4th ed., §14.7). The quadratic overestimates the limb by
    /// about 0.01 against their fifth-degree fits and Neckel and Labs 1994's (0.382 at 550 nm).
    #[test]
    fn the_solar_limb_follows_the_observed_polynomial() {
        let [_, v, _] = sun();
        let mu: f64 = 0.1;
        let observed = 0.294_62 + 0.980_32 * mu - 0.274_94 * mu * mu;
        assert!(
            (v.intensity(mu) - observed).abs() < 0.015,
            "{}",
            v.intensity(mu)
        );
    }

    #[test]
    fn every_clamp_returns_a_finite_row() {
        for (teff, log_g, grid) in [
            (1_000.0, 5.0, AtmosphereGrid::MainSequence),
            (90_000.0, 4.0, AtmosphereGrid::MainSequence),
            (45_000.0, 0.0, AtmosphereGrid::Giant),
            (3_000.0, 7.0, AtmosphereGrid::MainSequence),
            (200_000.0, 8.0, AtmosphereGrid::WhiteDwarf),
            (2_000.0, 10.0, AtmosphereGrid::WhiteDwarf),
            (f64::NAN, f64::NAN, AtmosphereGrid::Giant),
        ] {
            for law in limb_coefficients(Kelvin::new(teff), log_g, grid) {
                assert!(
                    law.c().is_finite() && law.alpha().is_finite(),
                    "{teff} {log_g}"
                );
                assert!((0.0..=1.0).contains(&law.disc_average()), "{teff} {log_g}");
            }
        }
        // Beyond the edge is the edge.
        assert_eq!(
            limb_coefficients(Kelvin::new(90_000.0), 4.5, AtmosphereGrid::MainSequence),
            limb_coefficients(Kelvin::new(50_000.0), 4.5, AtmosphereGrid::MainSequence)
        );
        assert_eq!(
            limb_coefficients(Kelvin::new(200_000.0), 8.0, AtmosphereGrid::WhiteDwarf),
            limb_coefficients(Kelvin::new(100_000.0), 8.0, AtmosphereGrid::WhiteDwarf)
        );
    }

    #[test]
    fn a_white_dwarf_reads_its_own_grid() {
        let wd = limb_coefficients(Kelvin::new(10_000.0), 8.0, AtmosphereGrid::WhiteDwarf);
        let star = limb_coefficients(Kelvin::new(10_000.0), 8.0, AtmosphereGrid::MainSequence);
        assert_ne!(wd, star);
    }
}
