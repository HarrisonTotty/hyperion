//! Bounds on the displaced classes' densities over a cell: the flare factor's supremum and the
//! radial envelope's nearest corner (plan 08, P08.T11; brainstorm, "Exact placement by thinning").
//!
//! Every bound follows plan 02's rule ([`UnimodalFactor`]): an envelope that never rises with |x|,
//! |y| or |z| is taken at the cell's nearest corner, and a factor that is unimodal in one scalar at
//! its supremum over that scalar's range across the cell.
//!
//! - A flared layer is its radial envelope `exp(−R ÷ h_R)` at the nearest corner times the flare
//!   factor's supremum ([`FlareFactor`]): at the cell's least |z|, `exp(−(z₁ ÷ h)^β) ÷ h` over the
//!   heights `h(R)` of the cell's radii, which peaks at `h★ = z₁ β^(1 ÷ β)`.
//! - A cored power law falls with R and |z|, so it is its value at the nearest corner.
//! - The ballistic layer is the young disc's envelope bound (its hole's factor at the farthest
//!   radius, plan 02) at the stretched height.
//! - A class that keeps the young disc's arm multiplies its layer's bound by plan 02's arm bound
//!   at the blurred width ([`SharpArm::sup`] over the cell's radii and phases), the rule of plan
//!   02's own young disc.
//!
//! Each bound carries [`BOUND_SLACK`] over its factors, so that rounding in the densities' own
//! arithmetic (their radius is `hypot(x, y)`, the cells' `√(x² + y²)`) never passes it. P08.T12's
//! `DisplacedFields::bound` weights these by each class's weight and in-cube share, and adds the
//! debug assertion in candidate evaluation.

use super::forms::{BallisticLayer, CoredPowerLaw, DiscBornForm, FlareFactor, FlaredLayer};
use crate::galaxy::bounds::{BOUND_MARGIN, CellBox, UnimodalFactor};
use crate::galaxy::fields::arms::SharpArm;

/// The relative margin every displaced bound carries over the product of its factors: four of
/// plan 02's [`BOUND_MARGIN`], about 3.6 × 10⁻¹².
pub const BOUND_SLACK: f64 = 1.0 + 4.0 * BOUND_MARGIN;

/// An upper bound on `layer`'s density (normalised, no arm) over `cell`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::bounds::CellBox;
/// use hyperion_sim::galaxy::displaced::GalaxyScales;
/// use hyperion_sim::galaxy::displaced::bound::layer_bound;
/// use hyperion_sim::galaxy::displaced::forms::{FlaredLayer, FlaredLayerParams};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let params = GalaxyParams::milky_way_like();
/// let scales = GalaxyScales::new(&params, &PotentialTables::in_plane(&MassModel::new(&params)));
/// let row = FlaredLayerParams { weight: 1.0, h_r: 1.2, h_0: 0.05, r_flare: 4.0, beta: 1.3 };
/// let layer = FlaredLayer::new(&row, &scales)?;
/// let cell = CellBox::new([25_984, 128, 512], 128)?;
/// let bound = layer_bound(&layer, &cell);
/// assert!(layer.density(&cell.centre()) <= bound);
/// assert!(layer.density(&PointLy::new(26_000.0, 200.0, 600.0)) <= bound);
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn layer_bound(layer: &FlaredLayer, cell: &CellBox) -> f64 {
    let radii = cell.r_cyl_range();
    let z_min = cell.nearest_corner().z.abs();
    let flare: FlareFactor = layer.flare_factor(z_min);
    layer.norm() * layer.radial(radii.lo) * flare.sup(radii) * BOUND_SLACK
}

/// An upper bound on `spheroid`'s density over `cell`: its value at the nearest corner.
#[must_use]
pub fn spheroid_bound(spheroid: &CoredPowerLaw, cell: &CellBox) -> f64 {
    let corner = cell.nearest_corner();
    let r = cell.r_cyl_range().lo;
    spheroid.norm() * spheroid.shape(r, corner.z.abs()) * BOUND_SLACK
}

/// An upper bound on the factor of the blurred arm `arm` over `cell`: plan 02's
/// [`SharpArm::sup`] over the cell's radii and arm phases.
#[must_use]
pub fn arm_bound(arm: &SharpArm, cell: &CellBox) -> f64 {
    arm.sup(cell.r_cyl_range(), arm.geometry().phase_range(cell))
}

/// An upper bound on `layer`'s density, arm factor included, over `cell`.
#[must_use]
pub fn ballistic_bound(layer: &BallisticLayer, cell: &CellBox) -> f64 {
    let radii = cell.r_cyl_range();
    let z_min = cell.nearest_corner().z.abs();
    let envelope = layer.envelope_sup(radii.lo, radii.hi, z_min) * BOUND_SLACK;
    match layer.arm() {
        Some(arm) => envelope * arm_bound(arm, cell),
        None => envelope,
    }
}

/// An upper bound on a disc-born class's `form` over `cell`: the weighted sum of its parts'
/// bounds, the layer's times the arm's where it keeps one (module documentation).
#[must_use]
pub fn form_bound(form: &DiscBornForm, cell: &CellBox) -> f64 {
    match form {
        DiscBornForm::Ballistic(layer) => ballistic_bound(layer, cell),
        DiscBornForm::Fitted {
            layer,
            arm,
            spheroid,
        } => {
            let flat = layer.as_ref().map_or(0.0, |(w, l)| {
                let arm_factor = arm.as_ref().map_or(1.0, |a| arm_bound(a, cell));
                w * layer_bound(l, cell) * arm_factor
            });
            let round = spheroid
                .as_ref()
                .map_or(0.0, |(w, s)| w * spheroid_bound(s, cell));
            (flat + round) * BOUND_SLACK
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::bounds::ScalarRange;
    use crate::galaxy::displaced::GalaxyScales;
    use crate::galaxy::displaced::forms::FlaredLayerParams;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::potential::{MassModel, PotentialTables};
    use crate::math;

    fn layer(beta: f64) -> FlaredLayer {
        let params = GalaxyParams::milky_way_like();
        let scales = GalaxyScales::new(
            &params,
            &PotentialTables::in_plane(&MassModel::new(&params)),
        );
        let row = FlaredLayerParams {
            weight: 1.0,
            h_r: 1.2,
            h_0: 0.04,
            r_flare: 3.0,
            beta,
        };
        FlaredLayer::new(&row, &scales).unwrap()
    }

    /// The flare factor's supremum is its peak where the peak's height lies in the range, the
    /// nearer end otherwise, and `1 ÷ h(R₁)` in the plane; never below g over the range.
    #[test]
    fn the_flare_factor_peaks_at_z_beta_to_the_one_over_beta() {
        for beta in [0.8, 1.0, 1.7, 3.0] {
            let layer = layer(beta);
            for z_min in [0.0, 50.0, 300.0, 2_000.0] {
                let flare = layer.flare_factor(z_min);
                for (lo, hi) in [(0.0, 181.0), (8_000.0, 8_181.0), (20_000.0, 30_000.0)] {
                    let sup = flare.sup(ScalarRange::new(lo, hi));
                    let mut best: f64 = 0.0;
                    for i in 0..=1_000 {
                        let r = lo + (hi - lo) * f64::from(i) / 1_000.0;
                        best = best.max(layer.vertical(layer.scale_height(r), z_min));
                    }
                    assert!(
                        best <= sup,
                        "β {beta} z {z_min} R {lo}–{hi}: {best} over {sup}"
                    );
                    // The supremum is attained on the range, so a fine scan comes close to it.
                    assert!(best >= sup * (1.0 - 1e-3), "loose: {best} against {sup}");
                }
            }
            let h_star = 300.0 * math::powf(beta, 1.0 / beta);
            let g = |h: f64| math::exp(-math::powf(300.0 / h, beta)) / h;
            assert!(g(h_star) >= g(h_star * 1.01) && g(h_star) >= g(h_star * 0.99));
        }
    }
}
