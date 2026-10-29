//! The spray's force (plan 10, P10.T5.b; ruling 146.3 of 2026-09-22): one bicubic Hermite table of
//! the galaxy's total potential, with its exact gradient.
//!
//! The leapfrog's force ([`Leapfrog`]) adds the dark halo, the nuclear cluster and the black hole
//! in closed form at every lookup, and beyond 2¹⁷·⁵ ly blends the tables into a far field of its
//! own: about 640 ns a force on the lane machine, some seconds for a stream's 2,000 tracers over
//! thousands of steps each (plan 10's Risks, "The force costs about 640 ns"). Ruling 146.3 gives
//! the spray one table instead. [`SprayForce`] holds the leapfrog's potential, everything
//! included, at the potential tables' nodes in `(ln R, ln |z|)`, 64 per axis from 2⁻⁴ to 2¹⁸ ly at
//! `22 ln 2 ÷ 63`, continued at that spacing to the first node past 2²⁰ ly ([`NODES`] per axis,
//! the last at 2^20.1 ly, 1.1 × 10⁶ ly), and interpolates it bicubically in the two logarithms.
//! The force is the interpolant's exact gradient, so the field is the gradient of one continuously
//! differentiable potential and conservative, as the leapfrog's own is.
//!
//! - **The nodes.** Each holds `Φ`, `R ∂Φ ÷ ∂R` and `z ∂Φ ÷ ∂z`, the leapfrog's own, and the cross
//!   derivative `R z ∂²Φ ÷ ∂R ∂z` by central differences of the first two in the logarithms, 2⁻¹²
//!   each way, averaged over the two orders. Any cross derivative gives a C¹ interpolant; these
//!   differ from the leapfrog's by its second derivatives' change over 2⁻¹², and their rounding
//!   by 2¹² ε of the slopes.
//! - **Within 2⁻⁴ ly of the plane or of the axis**, inside the first node, the potential is held
//!   at that node's, so the force across the slab is zero there. The vertical force falls to zero
//!   at the plane anyway, linearly, as `4πGρ₀ z` does: 2⁻⁴ ly from it, it is about 10⁻⁴ of its value
//!   a thousand light-years up (9 × 10⁻⁵ at 26,000 ly on the Milky Way fixture), and a body
//!   crosses the slab in a fraction of a step.
//! - **Beyond the last node** in `R` or `|z|`, the potential is continued from the table's edge as
//!   the potential tables continue theirs beyond their grid (plan 02, P02.T6.f): the value at the
//!   clamped point times that point's spherical radius over the true one, `A ÷ h` with `A = g h_c`,
//!   whose exact gradient the force is there. It is continuous, with kinks at the box's faces,
//!   which conserve energy (`PotentialTables::force`'s documentation); only the widest orphan
//!   orbits reach it, and then briefly.
//!
//! The lookup costs two logarithms, a square root and the 16 terms of each of the three sums: no
//! closed form and no `powf` is left in the spray's inner loop. Like the leapfrog's, it uses
//! `+ − × ÷`, `sqrt` and [`math`] only (P10.T2.c's source check reads this file).

use crate::coords::GalacticDisplacement;
use crate::galaxy::global_list::orbit::{
    BuildLeapfrogError, FORCE_IN_SI, Leapfrog, MAX_STEP, SQUARED_KM_S_IN_SI,
};
use crate::galaxy::potential::{
    GRID_FIRST_LY, GRID_LN_FIRST, GRID_LN_STEP, PotentialTables, grid_point, hermite, hermite_slope,
};
use crate::math;
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// Nodes per axis of the spray's table: the potential tables' 64, and six more at their spacing,
/// the last at 2^20.095 ly, the first past 2²⁰ ly (ruling 146.3).
pub const NODES: usize = 70;

/// Half the step of the cross derivative's central differences, in the logarithms (module
/// documentation, "The nodes").
const CROSS_STEP: f64 = 1.0 / 4_096.0;

/// The position in the table's cells beyond which a coordinate is clamped, just inside the last:
/// `NODES − 1` less 10⁻⁹.
const LAST_CELL: f64 = 68.999_999_999;

/// Ruling 146.3's table of the galaxy's total potential (module documentation).
///
/// # Examples
///
/// The spray's force against the leapfrog's at the Sun's radius, a kiloparsec above the plane
/// (not run: the (R, z) grid takes seconds to build):
///
/// ```no_run
/// use hyperion_sim::coords::GalacticDisplacement;
/// use hyperion_sim::galaxy::global_list::orbit::Leapfrog;
/// use hyperion_sim::galaxy::global_list::spray::SprayForce;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
/// use hyperion_sim::units::{LightYears, Megayears, Metres, Seconds};
///
/// let tables = PotentialTables::full(&MassModel::new(&GalaxyParams::milky_way_like()));
/// let force = SprayForce::new(&tables)?;
/// let leapfrog = Leapfrog::new(&tables, Seconds::from(Megayears::new(1.0)))?;
/// let [x, z] = [26_000.0, 3_262.0].map(|ly| Metres::from(LightYears::new(ly)).value());
/// let at = GalacticDisplacement::new([x, 0.0, z]);
/// let (fast, exact) = (force.acceleration_m_s2(&at), leapfrog.acceleration_m_s2(&at));
/// assert!((fast[0] / exact[0] - 1.0).abs() < 1e-3);
/// assert!((fast[2] / exact[2] - 1.0).abs() < 1e-3);
/// # Ok::<(), hyperion_sim::galaxy::global_list::orbit::BuildLeapfrogError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SprayForce {
    /// `[Φ, R ∂Φ ÷ ∂R, z ∂Φ ÷ ∂z, R z ∂²Φ ÷ ∂R ∂z]` at `(R_i, z_j)`, row `i`, column `j`, (km/s)².
    nodes: Vec<[f64; 4]>,
    /// The last node's coordinate on either axis, ly.
    last: f64,
}

impl SprayForce {
    /// The table of `tables`' total potential, as the leapfrog continues it (module
    /// documentation): 4,900 nodes, each six of the leapfrog's lookups.
    ///
    /// # Errors
    ///
    /// [`BuildLeapfrogError::NoVerticalGrid`] for tables without the (R, |z|) grid of
    /// [`Galaxy::with_full_potential`](crate::galaxy::Galaxy::with_full_potential).
    pub fn new(tables: &PotentialTables) -> Result<Self, BuildLeapfrogError> {
        let leapfrog = Leapfrog::new(tables, MAX_STEP)?;
        let coordinates: [f64; NODES] = core::array::from_fn(grid_point);
        let (up, down) = (math::exp(CROSS_STEP), math::exp(-CROSS_STEP));
        let gradient = |r: f64, z: f64| leapfrog.potential_and_gradient(r, z).1;
        let mut nodes = Vec::with_capacity(NODES * NODES);
        for &r in &coordinates {
            for &z in &coordinates {
                let (phi, [by_r, by_z]) = leapfrog.potential_and_gradient(r, z);
                // ∂(R ∂Φ/∂R) ÷ ∂ln z and ∂(z ∂Φ/∂z) ÷ ∂ln R, each R z ∂²Φ ÷ ∂R ∂z.
                let radial = r * (gradient(r, z * up)[0] - gradient(r, z * down)[0]);
                let vertical = z * (gradient(r * up, z)[1] - gradient(r * down, z)[1]);
                nodes.push([
                    phi,
                    r * by_r,
                    z * by_z,
                    f64::midpoint(radial, vertical) / (2.0 * CROSS_STEP),
                ]);
            }
        }
        Ok(Self {
            nodes,
            last: coordinates[NODES - 1],
        })
    }

    /// The bytes the table owns on the heap: 157 kB.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.nodes.capacity() * size_of::<[f64; 4]>()
    }

    /// The acceleration `−∇Φ` at `position` (about the galactic centre), m/s² along the galactic
    /// axes: the exact gradient of [`potential_j_kg`](Self::potential_j_kg).
    #[must_use]
    pub fn acceleration_m_s2(&self, position: &GalacticDisplacement) -> [f64; 3] {
        self.acceleration_ly(position.metres().map(|c| c / METRES_PER_LIGHT_YEAR))
            .map(|a| a * FORCE_IN_SI)
    }

    /// The potential at `position` (about the galactic centre), J/kg, zero at infinity.
    #[must_use]
    pub fn potential_j_kg(&self, position: &GalacticDisplacement) -> f64 {
        let p = position.metres().map(|c| c / METRES_PER_LIGHT_YEAR);
        self.potential_and_gradient(p).0 * SQUARED_KM_S_IN_SI
    }

    /// The acceleration at `p` (ly), (km/s)² per light-year along the galactic axes.
    #[must_use]
    pub(super) fn acceleration_ly(&self, p: [f64; 3]) -> [f64; 3] {
        let (_, horizontal, vertical) = self.potential_and_gradient(p);
        [-horizontal * p[0], -horizontal * p[1], -vertical]
    }

    /// The potential at `p` (ly), (km/s)², and its gradient as `(∂Φ ÷ ∂R) ÷ R`, (km/s)² per ly²,
    /// and `∂Φ ÷ ∂z`, (km/s)² per ly (module documentation).
    #[must_use]
    fn potential_and_gradient(&self, p: [f64; 3]) -> (f64, f64, f64) {
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let height = p[2].abs();
        let last = self.last;
        let (horizontal, by_height, phi) = if r <= last && height <= last {
            let [phi, by_ln_r, by_ln_z] = self.at(r.max(GRID_FIRST_LY), height.max(GRID_FIRST_LY));
            (
                if r > GRID_FIRST_LY {
                    by_ln_r / (r * r)
                } else {
                    0.0
                },
                if height > GRID_FIRST_LY {
                    by_ln_z / height
                } else {
                    0.0
                },
                phi,
            )
        } else {
            // Beyond the box: Φ = A ÷ h, A = g h_c from the clamped point, A changing along
            // whichever coordinate is inside the table's range.
            let (rc, zc) = (
                r.clamp(GRID_FIRST_LY, last),
                height.clamp(GRID_FIRST_LY, last),
            );
            let h = (r * r + height * height).sqrt();
            let hc = (rc * rc + zc * zc).sqrt();
            let [g, g_by_ln_r, g_by_ln_z] = self.at(rc, zc);
            let amplitude = g * hc;
            let a_by_r = if r > GRID_FIRST_LY && r < last {
                hc * g_by_ln_r / rc + g * rc / hc
            } else {
                0.0
            };
            let a_by_z = if height > GRID_FIRST_LY && height < last {
                hc * g_by_ln_z / zc + g * zc / hc
            } else {
                0.0
            };
            let h3 = h * h * h;
            let radial = if r > 0.0 { a_by_r / (h * r) } else { 0.0 };
            (
                radial - amplitude / h3,
                a_by_z / h - amplitude * height / h3,
                amplitude / h,
            )
        };
        let vertical = if p[2] < 0.0 { -by_height } else { by_height };
        (phi, horizontal, vertical)
    }

    /// The bicubic interpolant at `(r, height)` inside the table, ly: `Φ` and its derivatives in
    /// `ln R` and `ln |z|`, (km/s)².
    #[must_use]
    fn at(&self, r: f64, height: f64) -> [f64; 3] {
        let (row, across) = cell(r);
        let (column, up) = cell(height);
        let (radial, radial_rate) = (hermite(across), hermite_slope(across));
        let (vertical, vertical_rate) = (hermite(up), hermite_slope(up));
        // Basis weights of the value and of the slope, along each axis: `[value, slope]` pairs.
        let weights = |basis: [f64; 4], k: usize| [basis[k], basis[k + 2]];
        let (mut phi, mut by_ln_r, mut by_ln_z) = (0.0, 0.0, 0.0);
        for di in 0..2 {
            for dj in 0..2 {
                let node = self.nodes[(row + di) * NODES + column + dj];
                let term = |[r_value, r_slope]: [f64; 2], [z_value, z_slope]: [f64; 2]| {
                    let [value, by_r, by_z, by_both] = node;
                    r_value * z_value * value
                        + GRID_LN_STEP * (r_slope * z_value * by_r + r_value * z_slope * by_z)
                        + GRID_LN_STEP * GRID_LN_STEP * r_slope * z_slope * by_both
                };
                phi += term(weights(radial, di), weights(vertical, dj));
                by_ln_r += term(weights(radial_rate, di), weights(vertical, dj));
                by_ln_z += term(weights(radial, di), weights(vertical_rate, dj));
            }
        }
        // The basis's rates are per cell; one cell is the step in the logarithm.
        [phi, by_ln_r / GRID_LN_STEP, by_ln_z / GRID_LN_STEP]
    }
}

/// The cell of the table holding `ln x` for `x` inside it, and the position in it, 0–1.
#[must_use]
fn cell(x: f64) -> (usize, f64) {
    let position = ((math::ln(x) - GRID_LN_FIRST) / GRID_LN_STEP).clamp(0.0, LAST_CELL);
    let index = position.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number clamped to 0–68"
    )]
    let i = index as usize;
    (i, position - index)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table's nodes continue the potential tables' spacing past their grid's edge at 2¹⁸ ly,
    /// and the last is the first past 2²⁰ ly; a coordinate beyond either end is clamped to it.
    #[test]
    fn the_nodes_continue_the_potential_tables_spacing() {
        assert!((grid_point(63) - 262_144.0).abs() < f64::EPSILON);
        assert!(grid_point(NODES - 2) < 1_048_576.0 && grid_point(NODES - 1) > 1_048_576.0);
        let ratio = grid_point(NODES - 1) / grid_point(NODES - 2);
        let spacing = grid_point(10) / grid_point(9);
        assert!((ratio / spacing - 1.0).abs() < 1e-12);
        assert!((LAST_CELL.ceil() - 69.0).abs() < f64::EPSILON);
        let (at, t) = cell(0.0625);
        assert_eq!(at, 0);
        assert!(t.abs() < 1e-12);
        let (at, t) = cell(grid_point(NODES - 1) * 2.0);
        assert_eq!(at, NODES - 2);
        assert!((t - 1.0).abs() < 1e-8);
        let (at, t) = cell(0.01);
        assert_eq!(at, 0);
        assert!(t.abs() < f64::EPSILON);
    }
}
