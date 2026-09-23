//! The expected number of systems per layer in a sphere: what the census decides from, before
//! anything is generated (plan 03, P03.T9.c).
//!
//! Each component's density is integrated over the sphere once, and a layer's count is then
//! `Σ share × Iᶜ` over the components in their fixed order, the sum
//! [`Fields::layer_density`](crate::galaxy::fields::Fields::layer_density) and plan 02's bound take.
//! The sphere is the query's unpadded one and the epoch is the only time asked about, because the
//! process is stationary: padding for motion moves systems between neighbouring cells, not into or
//! out of a sphere's worth of expectation.
//!
//! # The rule
//!
//! Fixed nodes, chosen by the radius and the centre's height alone, so that the same query always
//! integrates the same points in the same order (plan 03, Design note 11; the note says "the radius
//! and whether the sphere crosses z = 0", and the panel count of each part of a split range follows
//! that part's own width):
//!
//! - The z range `[z₀ − R, z₀ + R]` is split at `z = 0` when the plane lies strictly inside it,
//!   because the bulge's and the bar's profiles have a kink there. The discs are cored in height and
//!   no longer do, but the split stays: the quadrature belongs to the generator version in the weak
//!   sense that changing it can change which layers a query returns.
//! - Each part is divided into `n = ⌈width ÷ 64 ly⌉` equal panels, held between 1 and 16, and
//!   integrated by Gauss–Legendre in z on each panel.
//! - At each z node the disc of radius `√(R² − (z − z₀)²)` about `(x₀, y₀)` is integrated by
//!   Gauss–Legendre in radius, with the polar weight `r`, and by equally spaced azimuths offset by
//!   half a step, which is the midpoint rule in azimuth and exact for a density that does not
//!   depend on it.
//! - The orders are 4 × 4 × 8 (z, radius, azimuth) for `R ≤ 256 ly` and 8 × 8 × 16 above.
//!
//! So a 50 ly sphere costs two panels of 128 nodes and a 5,000 ly one sixteen panels a part of
//! 1,024. The rule is exact for the volume itself, since the area of a disc is quadratic in z and
//! the polar integrand is linear in r.
//!
//! Nothing is clipped to the root cube, so a sphere that pokes outside it is counted as though the
//! galaxy went on. That errs high, which can only drop a layer from a census early — the direction
//! the brainstorm accepts for the features' share.

use super::LayerCounts;
use crate::coords::GalacticPosition;
use crate::galaxy::fields::MAX_COMPONENTS;
use crate::galaxy::placement::STELLAR_LAYERS;
use crate::galaxy::{Galaxy, PointLy};
use crate::math;
use crate::tables::gauss_legendre::{GL4_NODES, GL4_WEIGHTS, GL8_NODES, GL8_WEIGHTS};
use crate::units::LightYears;

/// The largest radius the coarse rule serves, light-years: 4 × 4 × 8 nodes up to here and
/// 8 × 8 × 16 above (plan 03, Design note 11).
const COARSE_RADIUS_LY: f64 = 256.0;

/// The widest a z panel may be, light-years.
const PANEL_LY: f64 = 64.0;

/// The most panels one part of the z range is divided into.
const MAX_PANELS: u32 = 16;

/// The most azimuths the rule uses, the length of the fine rule's ring: twice the highest order
/// [`rule_for`] can return.
const MAX_AZIMUTHS: usize = GL8_NODES.len() * 2;

/// The expected number of systems of each layer inside the sphere of `radius` about `centre`, at the
/// epoch.
///
/// The two substellar entries of the result are zero: plan 13 places those layers (plan 03, Design
/// note 17).
///
/// # Panics
///
/// If a layer's count comes out non-finite or negative ([`LayerCounts::set`]), which a density that
/// is a finite non-negative number everywhere cannot produce.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::query::expected_counts;
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(5));
/// // A 50 ly sphere at the Sun's distance from the centre holds a few thousand systems, most of
/// // them M dwarfs.
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
/// let counts = expected_counts(&galaxy, &sun, LightYears::new(50.0));
/// let total: f64 = counts.to_array().iter().sum();
/// assert!((500.0..8_000.0).contains(&total));
/// assert!(counts.get(Layer::A) > 0.5 * total);
/// // Layer E, above 8 M☉, is a fraction of a per cent, and no substellar layer is counted.
/// assert!(counts.get(Layer::E) < 0.02 * total);
/// assert_eq!(counts.get(Layer::BrownDwarf), 0.0);
/// ```
#[must_use]
pub fn expected_counts(
    galaxy: &Galaxy,
    centre: &GalacticPosition,
    radius: LightYears,
) -> LayerCounts {
    let fields = galaxy.fields();
    let components = fields.components();
    let mut integrals = [0.0; MAX_COMPONENTS];
    let mut densities = [0.0; MAX_COMPONENTS];
    for_each_node(&PointLy::from(centre), radius.value(), |point, weight| {
        fields.densities(point, &mut densities);
        for (integral, density) in integrals.iter_mut().zip(densities) {
            *integral += weight * density;
        }
    });
    let shares = galaxy.shares();
    let mut counts = LayerCounts::ZERO;
    for spec in STELLAR_LAYERS {
        let count = components
            .iter()
            .zip(integrals)
            .fold(0.0, |sum, (component, integral)| {
                sum + shares.component_share(spec.band(), component) * integral
            });
        counts.set(spec.layer(), count);
    }
    counts
}

/// Calls `sample(point, weight)` at every node of the sphere's quadrature, so that
/// `Σ weight × f(point)` is `∫ f dV` over the sphere.
///
/// The order is fixed: the parts of the z range, then the panels of each part, then the z nodes of
/// each panel, then the radius nodes, then the azimuths. A weight is
/// `z weight × (radius weight × r) × azimuth step`, multiplied in that order.
///
/// A radius that is not positive samples nothing.
fn for_each_node(centre: &PointLy, radius_ly: f64, mut sample: impl FnMut(&PointLy, f64)) {
    if radius_ly.is_nan() || radius_ly <= 0.0 {
        return;
    }
    let (nodes, weights) = rule_for(radius_ly);
    let azimuths = nodes.len() * 2;
    debug_assert!(
        azimuths <= MAX_AZIMUTHS,
        "the ring holds {MAX_AZIMUTHS} azimuths, not {azimuths}"
    );
    let azimuth_step = core::f64::consts::TAU / usize_as_f64(azimuths);
    // The ring of azimuths, offset by half a step, is the same at every node.
    let mut ring = [(0.0, 0.0); MAX_AZIMUTHS];
    for (k, slot) in ring[..azimuths].iter_mut().enumerate() {
        *slot = math::sin_cos(azimuth_step * (usize_as_f64(k) + 0.5));
    }
    let ring = &ring[..azimuths];
    let (parts, used) = z_parts(centre.z, radius_ly);
    for &(lo, hi) in &parts[..used] {
        let panels = panel_count(hi - lo);
        let width = (hi - lo) / f64::from(panels);
        for panel in 0..panels {
            let a = lo + f64::from(panel) * width;
            // The last panel ends exactly at the part's end, whatever the widths rounded to.
            let b = if panel + 1 == panels {
                hi
            } else {
                lo + f64::from(panel + 1) * width
            };
            let half = 0.5 * (b - a);
            let mid = a + half;
            for (&x, &w) in nodes.iter().zip(weights) {
                let z = mid + half * x;
                let height = z - centre.z;
                let rho_sq = radius_ly * radius_ly - height * height;
                if rho_sq <= 0.0 {
                    continue;
                }
                let rho = rho_sq.sqrt();
                let z_weight = w * half;
                let r_half = 0.5 * rho;
                for (&xr, &wr) in nodes.iter().zip(weights) {
                    let r = r_half * (1.0 + xr);
                    let weight = z_weight * (wr * r_half * r) * azimuth_step;
                    for &(sin, cos) in ring {
                        let point = PointLy::new(centre.x + r * cos, centre.y + r * sin, z);
                        sample(&point, weight);
                    }
                }
            }
        }
    }
}

/// The nodes and weights in z and in radius: the 4-point rule up to [`COARSE_RADIUS_LY`] and the
/// 8-point rule above it. The azimuth count is twice the order.
#[must_use]
fn rule_for(radius_ly: f64) -> (&'static [f64], &'static [f64]) {
    if radius_ly <= COARSE_RADIUS_LY {
        (&GL4_NODES, &GL4_WEIGHTS)
    } else {
        (&GL8_NODES, &GL8_WEIGHTS)
    }
}

/// The parts of the z range and how many there are: one, or two split at `z = 0` when the plane
/// lies strictly inside the sphere's range of height.
#[must_use]
fn z_parts(z0: f64, radius_ly: f64) -> ([(f64, f64); 2], usize) {
    let (lo, hi) = (z0 - radius_ly, z0 + radius_ly);
    if lo < 0.0 && hi > 0.0 {
        ([(lo, 0.0), (0.0, hi)], 2)
    } else {
        ([(lo, hi), (0.0, 0.0)], 1)
    }
}

/// How many equal panels a part of the z range is divided into: `⌈width ÷ 64 ly⌉`, between 1 and
/// [`MAX_PANELS`].
///
/// It counts up instead of rounding a quotient, so that no float reaches an integer conversion; the
/// loop runs at most 16 times.
#[must_use]
fn panel_count(width_ly: f64) -> u32 {
    let mut panels = 1;
    while panels < MAX_PANELS && f64::from(panels) * PANEL_LY < width_ly {
        panels += 1;
    }
    panels
}

/// A small count as an `f64`.
#[must_use]
fn usize_as_f64(n: usize) -> f64 {
    let n = u32::try_from(n).expect("a node count is far below 2^32");
    f64::from(n)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::params::GalaxyParams;
    use crate::id::Layer;
    use crate::rng::Seed;

    /// The seed of the galaxy these tests count over.
    const SEED: u64 = 0x0309_c007_0000_0000;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    fn sunlike() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    /// `∫ exp(−|z| ÷ h) dV` over the sphere of radius `R` about the origin, in closed form:
    /// `2π{R² h (1 − e^−R/h) − h³[2 − e^−R/h (2 + 2R/h + R²/h²)]}`.
    fn exponential_in_height(radius: f64, h: f64) -> f64 {
        let u = radius / h;
        let e = math::exp(-u);
        2.0 * core::f64::consts::PI
            * (radius * radius * h * (1.0 - e) - h * h * h * (2.0 - e * (2.0 + 2.0 * u + u * u)))
    }

    /// `∫ f dV` over the sphere by the rule.
    fn integral(centre: &PointLy, radius_ly: f64, f: impl Fn(&PointLy) -> f64) -> f64 {
        let mut total = 0.0;
        for_each_node(centre, radius_ly, |point, weight| {
            total += weight * f(point);
        });
        total
    }

    #[test]
    fn expected_the_rule_integrates_a_constant_to_the_spheres_volume() {
        for centre in [
            PointLy::new(0.0, 26_000.0, 0.0),
            PointLy::new(300.0, -400.0, 120.0),
            PointLy::new(0.0, 0.0, 0.0),
            PointLy::new(-12_000.0, 3_000.0, -8_000.0),
        ] {
            for radius in [1.0, 10.0, 50.0, 256.0, 257.0, 500.0, 5_000.0, 40_000.0] {
                let volume = 4.0 / 3.0 * core::f64::consts::PI * radius * radius * radius;
                let taken = integral(&centre, radius, |_| 1.0);
                assert!(
                    (taken / volume - 1.0).abs() < 1e-12,
                    "R = {radius} at {centre:?}: {taken} against {volume}"
                );
            }
        }
    }

    #[test]
    fn expected_the_rule_integrates_an_exponential_in_height_to_its_closed_form() {
        // Scale heights at or above the tightest the galaxy has — the nuclear disc's 90–150 ly and
        // the young disc's 150 ly — so that a 64 ly panel resolves the profile.
        for (radius, h) in [
            (30.0, 90.0),
            (50.0, 150.0),
            (100.0, 90.0),
            (500.0, 150.0),
            (2_000.0, 300.0),
        ] {
            let closed = exponential_in_height(radius, h);
            let taken = integral(&PointLy::new(0.0, 0.0, 0.0), radius, |p| {
                math::exp(-p.z.abs() / h)
            });
            assert!(
                (taken / closed - 1.0).abs() < 1e-6,
                "R = {radius}, h = {h}: {taken} against {closed}"
            );
        }
    }

    /// A profile tighter than anything in the galaxy, where a single 50 ly panel has to cover 2.5
    /// scale heights: the rule is still good to 5 × 10⁻⁶, far inside the 2% the layer totals are
    /// checked to.
    #[test]
    fn expected_the_rule_holds_on_a_profile_tighter_than_the_galaxy_has() {
        let (radius, h) = (50.0, 20.0);
        let closed = exponential_in_height(radius, h);
        let taken = integral(&PointLy::new(0.0, 0.0, 0.0), radius, |p| {
            math::exp(-p.z.abs() / h)
        });
        let error = (taken / closed - 1.0).abs();
        assert!(
            error < 5e-6,
            "R = {radius}, h = {h}: relative error {error}"
        );
    }

    #[test]
    fn expected_the_plane_splits_the_range_only_when_it_lies_strictly_inside() {
        assert_eq!(z_parts(0.0, 50.0), ([(-50.0, 0.0), (0.0, 50.0)], 2));
        assert_eq!(z_parts(20.0, 50.0), ([(-30.0, 0.0), (0.0, 70.0)], 2));
        // The plane at an end of the range is not strictly inside, so the range stays whole.
        assert_eq!(z_parts(50.0, 50.0), ([(0.0, 100.0), (0.0, 0.0)], 1));
        assert_eq!(z_parts(-50.0, 50.0), ([(-100.0, 0.0), (0.0, 0.0)], 1));
        assert_eq!(z_parts(500.0, 50.0), ([(450.0, 550.0), (0.0, 0.0)], 1));
    }

    #[test]
    fn expected_panels_are_sixty_four_light_years_at_most_and_sixteen_at_most() {
        assert_eq!(panel_count(1.0), 1);
        assert_eq!(panel_count(64.0), 1);
        assert_eq!(panel_count(64.5), 2);
        assert_eq!(panel_count(128.0), 2);
        assert_eq!(panel_count(129.0), 3);
        assert_eq!(panel_count(1_024.0), 16);
        assert_eq!(panel_count(100_000.0), MAX_PANELS);
    }

    #[test]
    fn expected_the_rule_changes_order_at_two_hundred_and_fifty_six_light_years() {
        assert_eq!(rule_for(256.0).0.len(), 4);
        assert_eq!(rule_for(256.000_001).0.len(), 8);
    }

    #[test]
    fn expected_counts_are_bit_identical_on_repeated_calls() {
        let galaxy = galaxy();
        for radius in [10.0, 50.0, 500.0] {
            let radius = LightYears::new(radius);
            let first = expected_counts(&galaxy, &sunlike(), radius);
            let second = expected_counts(&galaxy, &sunlike(), radius);
            for (a, b) in first.to_array().into_iter().zip(second.to_array()) {
                assert_same_bits(a, b);
            }
        }
    }

    #[test]
    fn expected_counts_fall_as_the_centre_rises_out_of_the_plane() {
        let galaxy = galaxy();
        let radius = LightYears::new(50.0);
        let in_plane = expected_counts(&galaxy, &sunlike(), radius).get(Layer::A);
        let above = GalacticPosition::from_light_years([0.0, 26_000.0, 2_000.0]).unwrap();
        let high = expected_counts(&galaxy, &above, radius).get(Layer::A);
        assert!(
            high > 0.0 && high < 0.5 * in_plane,
            "{high} above the plane against {in_plane} in it"
        );
    }

    #[test]
    fn expected_counts_leave_the_substellar_layers_at_zero() {
        let galaxy = galaxy();
        let counts = expected_counts(&galaxy, &sunlike(), LightYears::new(100.0));
        assert_same_bits(counts.get(Layer::BrownDwarf), 0.0);
        assert_same_bits(counts.get(Layer::RoguePlanet), 0.0);
        for spec in STELLAR_LAYERS {
            assert!(counts.get(spec.layer()) > 0.0, "{:?}", spec.layer());
        }
    }
}
