//! The spherical average of the galaxy's extended mass: its monopole, the potential the galactic
//! centre's distribution functions are inverted in besides the black hole and the nuclear cluster
//! (plan 09, P09.T24; ruling 144 of 2026-09-22's joint revision).
//!
//! The angle average of a potential over a sphere is its monopole, `Φ̄(r) = −G M(<r) ÷ r − G W(r)`
//! with `W(r) = ∫ᵣ^∞ 4πr′ ρ̄ dr′` and `ρ̄` the density averaged over the sphere, so a spherical
//! model of the galaxy about its centre needs only `M(<r)` and `ρ̄(r)`. [`SphericalAverage`] holds
//! them for the Gaussians (the discs, the bar and the bulge), each Gaussian's exact
//! ([`Gaussian`](super::mge::Gaussian)'s quadratures), and adds the dark halo in closed form.

use super::mge::Gaussian;
use super::model::MassModel;
use super::nfw::Nfw;
use super::spherical::SphericalMass;
use crate::galaxy::consts::G;
use crate::math;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::units::{LightYears, SolarMasses};

/// The table's first radius, ly: inside it the Gaussians' mass is a power law of the first knot's
/// slope, which is 3 to within their central density's curvature.
const FIRST: f64 = 1e-3;

/// Knots per decade.
const KNOTS_PER_DECADE: u32 = 8;

/// Decades from the first knot to the last, 10⁻³ to 10⁶ ly: beyond, every Gaussian of a drawn
/// galaxy lies inside, to `e^(−40)` (the widest, the gas disc's, has σ up to 3.5 of a scale length
/// of at most 23,000 ly, and nine of them are 7.2 × 10⁵ ly).
const DECADES: u32 = 9;

/// The monopole of a mass model's Gaussians and dark halo: its enclosed mass, mean density and
/// potential at any radius from the centre (module documentation).
///
/// The Gaussians' `ln M(<r)` is tabulated at 73 knots, 8 a decade from 10⁻³ to 10⁶ ly, with its
/// exact logarithmic slope `4πr³ρ̄ ÷ M`, and read between them by the cubic Hermite interpolant in
/// `ln r`, whose own derivative is the density: `ρ̄ = M H′ ÷ 4πr³`, so that the mass and the
/// density agree exactly. `W` is integrated from that density by 16-node Gauss–Legendre panels
/// between knots, and the potential `G (M ÷ r + W)` at the knots is read between them by the cubic
/// Hermite interpolant in `ln r` with its exact slope `−G M ÷ r`. Beyond the last knot the
/// Gaussians are a point mass. Radii are light-years, potentials (km/s)² with zero at infinity.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::{MassModel, SphericalAverage};
/// use hyperion_sim::units::LightYears;
///
/// let model = MassModel::new(&GalaxyParams::milky_way_like());
/// let average = SphericalAverage::of_extended(&model);
/// // Far out the galaxy's discs and bulge pull as a point mass of their total, and the halo
/// // is added on top.
/// let far = LightYears::new(3e6);
/// let stars = average.extended_mass(far).value();
/// assert!((stars / model.expanded_enclosed_mass(far).value() - 1.0).abs() < 1e-9);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SphericalAverage {
    /// `ln r_k`, even in steps of `ln 10 ÷ 8`.
    ln_knots: Vec<f64>,
    /// `ln M_k` and `d ln M ÷ d ln r` at each knot.
    log_mass: Vec<(f64, f64)>,
    /// `Ψ_k = G (M_k ÷ r_k + W_k)` and `dΨ ÷ d ln r = −G M_k ÷ r_k` at each knot, (km/s)².
    psi: Vec<(f64, f64)>,
    dark_halo: Nfw,
}

/// The cubic Hermite interpolant on `[0, 1]` in `t` between `(y0, d0)` and `(y1, d1)`, slopes per
/// panel width `h`, and its derivative in the panel's variable, per unit of it.
fn hermite(t: f64, h: f64, (y0, d0): (f64, f64), (y1, d1): (f64, f64)) -> (f64, f64) {
    let (t2, t3) = (t * t, t * t * t);
    let value = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
        + (t3 - 2.0 * t2 + t) * h * d0
        + (-2.0 * t3 + 3.0 * t2) * y1
        + (t3 - t2) * h * d1;
    let slope = ((6.0 * t2 - 6.0 * t) * y0
        + (3.0 * t2 - 4.0 * t + 1.0) * h * d0
        + (-6.0 * t2 + 6.0 * t) * y1
        + (3.0 * t2 - 2.0 * t) * h * d1)
        / h;
    (value, slope)
}

impl SphericalAverage {
    /// The monopole of `model`'s Gaussians and its dark halo: everything but the black hole and
    /// the nuclear cluster, which the centre holds itself.
    ///
    /// # Panics
    ///
    /// If the Gaussians' mass inside the first knot is not positive, which a built model's
    /// discs and bulge, positive at the centre, never allow.
    #[must_use]
    pub fn of_extended(model: &MassModel) -> Self {
        Self::of(model.gaussians(), *model.dark_halo())
    }

    fn of(gaussians: &[Gaussian], dark_halo: Nfw) -> Self {
        let n = usize::try_from(KNOTS_PER_DECADE * DECADES).expect("72 panels") + 1;
        let step = core::f64::consts::LN_10 / f64::from(KNOTS_PER_DECADE);
        let lo = math::ln(FIRST);
        let ln_knots: Vec<f64> = (0..n)
            .map(|k| lo + step * f64::from(u32::try_from(k).expect("73 knots fit a u32")))
            .collect();
        let four_pi = 4.0 * core::f64::consts::PI;
        let log_mass: Vec<(f64, f64)> = ln_knots
            .iter()
            .map(|&x| {
                let r = math::exp(x);
                let (mass, density) = gaussians.iter().fold((0.0, 0.0), |(m, d), g| {
                    let (gm, gd) = g.mass_and_shell_density(r);
                    (m + gm, d + gd)
                });
                assert!(mass > 0.0, "the Gaussians' mass inside {r} ly is {mass}");
                (math::ln(mass), four_pi * r * r * r * density / mass)
            })
            .collect();
        let mut average = Self {
            ln_knots,
            log_mass,
            psi: Vec::new(),
            dark_halo,
        };
        // W from the outside in: nothing beyond the last knot, then 16-node panels in ln r of
        // `4π r² ρ̄` over the interpolant's own density.
        let mut outer = vec![0.0; n];
        for k in (0..n - 1).rev() {
            let (a, b) = (average.ln_knots[k], average.ln_knots[k + 1]);
            let half = 0.5 * (b - a);
            let mut piece = 0.0;
            for (&node, &weight) in GL16_NODES.iter().zip(&GL16_WEIGHTS) {
                let r = math::exp(a + half + half * node);
                let (_, density) = average.extended(r);
                piece += weight * half * four_pi * r * r * density;
            }
            outer[k] = outer[k + 1] + piece;
        }
        average.psi = average
            .ln_knots
            .iter()
            .zip(&average.log_mass)
            .zip(&outer)
            .map(|((&x, &(ln_m, _)), &w)| {
                let (r, m) = (math::exp(x), math::exp(ln_m));
                (G * (m / r + w), -G * m / r)
            })
            .collect();
        average
    }

    /// The panel `k` with `ln_knots[k] ≤ x < ln_knots[k + 1]` and the position `t` in it, for `x`
    /// inside the table.
    fn locate(&self, x: f64) -> (usize, f64) {
        let last = self.ln_knots.len() - 2;
        let step = self.ln_knots[1] - self.ln_knots[0];
        #[expect(
            clippy::cast_precision_loss,
            reason = "the last panel's index, 71, is exact"
        )]
        let at = ((x - self.ln_knots[0]) / step)
            .clamp(0.0, last as f64)
            .floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is clamped to [0, 71] and floored, a small non-negative integer"
        )]
        let mut k = (at as usize).min(last);
        while k > 0 && self.ln_knots[k] > x {
            k -= 1;
        }
        while k < last && self.ln_knots[k + 1] <= x {
            k += 1;
        }
        (
            k,
            (x - self.ln_knots[k]) / (self.ln_knots[k + 1] - self.ln_knots[k]),
        )
    }

    /// The Gaussians' mass inside `r` ly and their mean density on its sphere.
    #[expect(
        clippy::many_single_char_names,
        reason = "radius, knot and panel in their usual symbols"
    )]
    fn extended(&self, r: f64) -> (f64, f64) {
        let four_pi = 4.0 * core::f64::consts::PI;
        let n = self.ln_knots.len();
        if r <= 0.0 {
            return (0.0, self.extended(math::exp(self.ln_knots[0])).1);
        }
        let x = math::ln(r);
        if x >= self.ln_knots[n - 1] {
            return (math::exp(self.log_mass[n - 1].0), 0.0);
        }
        if x <= self.ln_knots[0] {
            let (ln_m, slope) = self.log_mass[0];
            let m = math::exp(ln_m + slope * (x - self.ln_knots[0]));
            return (m, slope * m / (four_pi * r * r * r));
        }
        let (k, t) = self.locate(x);
        let h = self.ln_knots[k + 1] - self.ln_knots[k];
        let (ln_m, slope) = hermite(t, h, self.log_mass[k], self.log_mass[k + 1]);
        let m = math::exp(ln_m);
        (m, slope * m / (four_pi * r * r * r))
    }

    /// The Gaussians' relative potential `Ψ = −Φ̄` at `r` ly, (km/s)².
    #[expect(
        clippy::many_single_char_names,
        reason = "radius, knot and panel in their usual symbols"
    )]
    fn extended_psi(&self, r: f64) -> f64 {
        let n = self.ln_knots.len();
        let x = math::ln(r.max(f64::MIN_POSITIVE));
        if x >= self.ln_knots[n - 1] {
            return G * math::exp(self.log_mass[n - 1].0) / r;
        }
        if x <= self.ln_knots[0] {
            // `Ψ₀ + G ∫_r^{r₀} M(<r′) ÷ r′² dr′` of the power law `M₀ (r ÷ r₀)^s`.
            let (ln_m, s) = self.log_mass[0];
            let r0 = math::exp(self.ln_knots[0]);
            let m0 = math::exp(ln_m);
            let rise = if (s - 1.0).abs() < 1e-9 {
                m0 / r0 * math::ln(r0 / r.max(f64::MIN_POSITIVE))
            } else {
                m0 / r0 * (1.0 - math::powf(r / r0, s - 1.0)) / (s - 1.0)
            };
            return self.psi[0].0 + G * rise;
        }
        let (k, t) = self.locate(x);
        let h = self.ln_knots[k + 1] - self.ln_knots[k];
        hermite(t, h, self.psi[k], self.psi[k + 1]).0
    }

    /// The Gaussians' mass inside `r` ly: the discs', the bar's and the bulge's.
    #[must_use]
    pub fn extended_mass(&self, r: LightYears) -> SolarMasses {
        SolarMasses::new(self.extended(r.value()).0)
    }

    /// The relative potential `Ψ(r) = −Φ̄(r)` at `r` ly, the Gaussians' and the dark halo's,
    /// (km/s)²: positive, falling outward.
    #[must_use]
    pub fn psi(&self, r: f64) -> f64 {
        self.extended_psi(r) - self.dark_halo.potential(LightYears::new(r))
    }
}

impl SphericalMass for SphericalAverage {
    fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        SolarMasses::new(self.extended(r.value()).0) + self.dark_halo.enclosed_mass(r)
    }

    fn density(&self, r: LightYears) -> f64 {
        self.extended(r.value()).1 + self.dark_halo.density(r)
    }

    fn potential(&self, r: LightYears) -> f64 {
        -self.psi(r.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::quad::gl_log_panels;

    fn milky_way() -> (MassModel, SphericalAverage) {
        let model = MassModel::new(&GalaxyParams::milky_way_like());
        let average = SphericalAverage::of_extended(&model);
        (model, average)
    }

    /// The table's mass against the Gaussians' own quadrature, and its density against the mass's
    /// finite difference, from inside the first knot to beyond the last.
    #[test]
    fn the_table_holds_the_gaussians_mass_and_density() {
        let (model, average) = milky_way();
        for r in [3e-4, 0.07, 1.0, 13.0, 100.0, 777.0, 5e3, 26_000.0, 2e5, 3e6] {
            let direct = model.expanded_enclosed_mass(LightYears::new(r)).value();
            let (table, density) = average.extended(r);
            // The holed thin disc's Gaussians of both signs make M(<r) turn most near 5,000 ly,
            // where 8 knots a decade hold it to 2.7 × 10⁻⁵; inside 1,000 ly, to 10⁻⁵. Inside the
            // first knot the power law of its slope is 5 × 10⁻⁵ off at 3 × 10⁻⁴ ly, where the
            // black hole outweighs this mass 10¹⁵ times.
            let tolerance = if (FIRST..1e3).contains(&r) {
                1e-5
            } else {
                1e-4
            };
            assert!(
                (table / direct - 1.0).abs() < tolerance,
                "M({r}) {table} against {direct}"
            );
            let difference = |h: f64| {
                let (up, down) = (
                    average.extended(r * (1.0 + h)).0,
                    average.extended(r * (1.0 - h)).0,
                );
                (up - down) / (2.0 * h * r) / (4.0 * core::f64::consts::PI * r * r)
            };
            // Rounding floor: ΔM ÷ M ≈ 3e-10 at 2e5 ly, so an ulp of ln M is ~2e-5 at h = 1e-4.
            let finite = if r < 1e5 {
                difference(1e-4)
            } else {
                let h = 4e-3;
                (4.0 * difference(h / 2.0) - difference(h)) / 3.0
            };
            let density_tolerance = if r < 1e5 { 1e-6 } else { 1e-5 };
            if density > 0.0 {
                assert!(
                    (finite / density - 1.0).abs() < density_tolerance,
                    "ρ̄({r}) {density} against {finite}"
                );
            }
        }
    }

    /// The potential is `G (M ÷ r + W)`: against a quadrature of `G M(<r) ÷ r²` from r out, and
    /// against the model's own potential averaged over directions.
    #[test]
    fn the_potential_is_the_monopole() {
        let (model, average) = milky_way();
        for r in [0.5_f64, 30.0, 300.0, 3_000.0, 30_000.0] {
            // Panels at the table's knots, where the interpolant's second derivative jumps.
            let edges: Vec<f64> = [r]
                .into_iter()
                .chain(
                    average
                        .ln_knots
                        .iter()
                        .map(|&x| math::exp(x))
                        .filter(|&k| k > r),
                )
                .chain([1e7, 1e9, 1e12])
                .collect();
            let integral = gl_log_panels(|x| G * average.extended(x).0 / (x * x), &edges);
            let psi = average.extended_psi(r);
            assert!(
                (integral / psi - 1.0).abs() < 1e-5,
                "Ψ({r}) {psi} against {integral}"
            );
            // The angle average of the Gaussians' potential, by 32 nodes in cos θ.
            let mean = crate::galaxy::quad::gl32(
                |mu| {
                    let s = (1.0 - mu * mu).sqrt();
                    model
                        .gaussians()
                        .iter()
                        .map(|g| g.potential(LightYears::new(r * s), LightYears::new(r * mu)))
                        .sum::<f64>()
                },
                0.0,
                1.0,
            );
            assert!(
                (-mean / psi - 1.0).abs() < 1e-4,
                "Ψ({r}) {psi} against the average {}",
                -mean
            );
        }
    }
}
