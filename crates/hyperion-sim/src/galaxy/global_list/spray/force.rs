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
//! A force's lookup costs two logarithms, a square root and the 16 terms of each of the two
//! slopes' sums (the potential's own sum is read only beyond the box and by
//! [`SprayForce::potential_j_kg`]): no closed form and no `powf` is left in the spray's inner
//! loop. Like the leapfrog's, it uses `+ − × ÷`, `sqrt` and [`math`] only (P10.T2.c's source
//! check reads this file).
//!
//! **Four lookups side by side** ([`SprayForce::locate_x4`], [`LocatedX4::logs`] and
//! [`SprayForce::acceleration_at_x4`]): the lookup's three parts over four bodies at once, a lane
//! of [`F64x4`] each, for the spray's lanes (`spray::step_in_lanes`). Each lane is the lookup's
//! own, bit for bit: the same operations in the same order, [`lanes::ln`] for [`math::ln`], and
//! each lane's own lookup wherever a lane lies beyond the table's box.

use crate::coords::GalacticDisplacement;
use crate::galaxy::global_list::orbit::{
    BuildLeapfrogError, FORCE_IN_SI, Leapfrog, MAX_STEP, SQUARED_KM_S_IN_SI,
};
use crate::galaxy::potential::{
    GRID_FIRST_LY, GRID_LN_FIRST, GRID_LN_STEP, PotentialTables, grid_point, hermite, hermite_slope,
};
use crate::math;
use crate::math::lanes::{self, F64x4, Mask4};
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

// `cell` takes a cell's index as a `u8`.
const _: () = assert!(NODES <= 256, "a cell's index fits a u8");

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
        let at = self.locate(position.metres());
        self.acceleration_at(&at, at.logs())
    }

    /// The potential at `position` (about the galactic centre), J/kg, zero at infinity.
    #[must_use]
    pub fn potential_j_kg(&self, position: &GalacticDisplacement) -> f64 {
        let at = self.locate(position.metres());
        self.potential_and_gradient(&at, at.logs()).0 * SQUARED_KM_S_IN_SI
    }

    /// A lookup's first part: where `position_m` (m) reads the table ([`Located`]).
    // The lookup's parts and `Patch`'s are forced inline into the shell orbits' lanes
    // (`spray::radii_over_ly`), where LLVM's own choice left them out of line: a lane's step cost
    // 61 ns out of line, 64 ns with `#[inline]` hints and 49 ns inlined, on the Milky Way
    // fixture's 5.1 × 10⁶ steps (P10.T4's list bench, pod, 16 vCPU).
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    pub(super) fn locate(&self, position_m: [f64; 3]) -> Located {
        let p = position_m.map(|c| c / METRES_PER_LIGHT_YEAR);
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let height = p[2].abs();
        let last = self.last;
        let inside = r <= last && height <= last;
        let point = if inside {
            [r.max(GRID_FIRST_LY), height.max(GRID_FIRST_LY)]
        } else {
            [
                r.clamp(GRID_FIRST_LY, last),
                height.clamp(GRID_FIRST_LY, last),
            ]
        };
        Located {
            p_ly: p,
            r_ly: r,
            height_ly: height,
            inside,
            point_ly: point,
        }
    }

    /// A lookup's last part: the acceleration at `at`, m/s² along the galactic axes, from `logs`,
    /// [`Located::logs`].
    ///
    /// Inside the table it reads the interpolant's slopes alone ([`Patch::slopes`]), which are
    /// [`potential_and_gradient`](Self::potential_and_gradient)'s bit for bit, and skips the
    /// potential's sum, which a force never reads.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    pub(super) fn acceleration_at(&self, at: &Located, logs: [f64; 2]) -> [f64; 3] {
        let p = at.p_ly;
        let (horizontal, vertical) = if at.inside {
            let [by_ln_r, by_ln_z] = Patch::new(&self.nodes, logs).slopes();
            let (horizontal, by_height) = within(at.r_ly, at.height_ly, by_ln_r, by_ln_z);
            (horizontal, if p[2] < 0.0 { -by_height } else { by_height })
        } else {
            let (_, horizontal, vertical) = self.potential_and_gradient(at, logs);
            (horizontal, vertical)
        };
        [-horizontal * p[0], -horizontal * p[1], -vertical].map(|a| a * FORCE_IN_SI)
    }

    /// The potential at `at`, (km/s)², and its gradient as `(∂Φ ÷ ∂R) ÷ R`, (km/s)² per ly², and
    /// `∂Φ ÷ ∂z`, (km/s)² per ly, from `logs`, [`Located::logs`] (module documentation).
    #[must_use]
    fn potential_and_gradient(&self, at: &Located, logs: [f64; 2]) -> (f64, f64, f64) {
        let Located {
            p_ly: p,
            r_ly: r,
            height_ly: height,
            inside,
            point_ly: [rc, zc],
        } = *at;
        let last = self.last;
        let patch = Patch::new(&self.nodes, logs);
        let [by_ln_r, by_ln_z] = patch.slopes();
        let phi = patch.sum(patch.radial, patch.vertical);
        let (horizontal, by_height, phi) = if inside {
            let (horizontal, by_height) = within(r, height, by_ln_r, by_ln_z);
            (horizontal, by_height, phi)
        } else {
            // Beyond the box: Φ = A ÷ h, A = g h_c from the clamped point, A changing along
            // whichever coordinate is inside the table's range.
            let (g, g_by_ln_r, g_by_ln_z) = (phi, by_ln_r, by_ln_z);
            let h = (r * r + height * height).sqrt();
            let hc = (rc * rc + zc * zc).sqrt();
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
}

impl SprayForce {
    /// [`locate`](Self::locate) of four positions, a lane each, `position_m` (m) by axis.
    ///
    /// `locate`'s two branches give the same point of the table lane by lane: inside the box
    /// neither distance is NaN or beyond the last node, so its floor at the first node is the
    /// clamp to the box there.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    pub(super) fn locate_x4(&self, position_m: [F64x4; 3]) -> LocatedX4 {
        let ly = F64x4::splat(METRES_PER_LIGHT_YEAR);
        let p = [position_m[0] / ly, position_m[1] / ly, position_m[2] / ly];
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let height = p[2].abs();
        let (first, last) = (F64x4::splat(GRID_FIRST_LY), F64x4::splat(self.last));
        LocatedX4 {
            p_ly: p,
            r_ly: r,
            height_ly: height,
            inside: r.le(last) & height.le(last),
            point_ly: [
                r.at_least(first).at_most(last),
                height.at_least(first).at_most(last),
            ],
        }
    }

    /// [`acceleration_at`](Self::acceleration_at) of four lookups, a lane each, m/s² by axis:
    /// side by side where every lane lies inside the table's box, else each lane's own.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    pub(super) fn acceleration_at_x4(&self, at: &LocatedX4, logs: [F64x4; 2]) -> [F64x4; 3] {
        if !at.inside.all() {
            return self.acceleration_lane_by_lane(at, logs);
        }
        let p = at.p_ly;
        let [by_ln_r, by_ln_z] = PatchX4::new(&self.nodes, logs).slopes();
        let (horizontal, by_height) = within_x4(at.r_ly, at.height_ly, by_ln_r, by_ln_z);
        let vertical = p[2].lt(F64x4::splat(0.0)).select(-by_height, by_height);
        let force = F64x4::splat(FORCE_IN_SI);
        [
            -horizontal * p[0] * force,
            -horizontal * p[1] * force,
            -vertical * force,
        ]
    }

    /// [`acceleration_at`](Self::acceleration_at) of each of four lookups alone, for lanes
    /// beyond the table's box.
    #[must_use]
    #[cold]
    #[inline(never)]
    fn acceleration_lane_by_lane(&self, at: &LocatedX4, logs: [F64x4; 2]) -> [F64x4; 3] {
        let lane = |v: F64x4, k: usize| v.to_array()[k];
        let lanes: [[f64; 3]; 4] = core::array::from_fn(|k| {
            let located = Located {
                p_ly: at.p_ly.map(|c| lane(c, k)),
                r_ly: lane(at.r_ly, k),
                height_ly: lane(at.height_ly, k),
                inside: at.inside.to_array()[k],
                point_ly: at.point_ly.map(|c| lane(c, k)),
            };
            self.acceleration_at(&located, logs.map(|c| lane(c, k)))
        });
        core::array::from_fn(|c| F64x4::new(lanes.map(|a| a[c])))
    }
}

#[cfg(test)]
impl SprayForce {
    /// The acceleration at `position`, m/s², from
    /// [`potential_and_gradient`](Self::potential_and_gradient) in full: what
    /// [`acceleration_m_s2`](Self::acceleration_m_s2) reads inside the table without the
    /// potential's sum, which tests hold it to bit for bit.
    #[must_use]
    pub(super) fn acceleration_in_full_m_s2(&self, position: &GalacticDisplacement) -> [f64; 3] {
        let at = self.locate(position.metres());
        let (_, horizontal, vertical) = self.potential_and_gradient(&at, at.logs());
        let p = at.p_ly;
        [-horizontal * p[0], -horizontal * p[1], -vertical].map(|a| a * FORCE_IN_SI)
    }
}

/// Where a point reads the table, the first part of a lookup ([`SprayForce::locate`]): the
/// point in light-years, its distances from the axis and from the plane, whether it lies inside
/// the table's box, and the point of the table it reads, `(R, |z|)` clamped to the table.
///
/// A lookup is split in three so that a caller stepping several bodies at once can take every
/// body's [`logs`](Self::logs) together: they are the longest link in a step's chain of
/// latencies, and a body's next step waits on its last, while different bodies' steps do not
/// wait on each other.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Located {
    /// The point.
    p_ly: [f64; 3],
    /// Its distance from the axis.
    r_ly: f64,
    /// Its distance from the plane.
    height_ly: f64,
    /// Whether it lies inside the table's box.
    inside: bool,
    /// The point of the table it reads, `(R, |z|)`.
    point_ly: [f64; 2],
}

impl Located {
    /// The lookup's middle part: `(ln R, ln |z|)` of the point of the table it reads.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    pub(super) fn logs(&self) -> [f64; 2] {
        self.point_ly.map(math::ln)
    }
}

/// Four lookups' first parts side by side, [`Located`]'s a lane each
/// ([`SprayForce::locate_x4`]).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LocatedX4 {
    /// The points.
    p_ly: [F64x4; 3],
    /// Their distances from the axis.
    r_ly: F64x4,
    /// Their distances from the plane.
    height_ly: F64x4,
    /// Whether each lies inside the table's box.
    inside: Mask4,
    /// The points of the table they read, `(R, |z|)`.
    point_ly: [F64x4; 2],
}

impl LocatedX4 {
    /// The lookups' middle part: [`Located::logs`] of each lane.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    pub(super) fn logs(&self) -> [F64x4; 2] {
        [lanes::ln(self.point_ly[0]), lanes::ln(self.point_ly[1])]
    }
}

/// The gradient inside the table at `(r, height)`, ly, from the interpolant's derivatives there
/// in `ln R` and `ln |z|`: `(∂Φ ÷ ∂R) ÷ R`, (km/s)² per ly², and `∂Φ ÷ ∂|z|`, (km/s)² per ly,
/// each zero within the first node of the axis or of the plane (module documentation).
#[must_use]
#[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
#[inline(always)]
fn within(r: f64, height: f64, by_ln_r: f64, by_ln_z: f64) -> (f64, f64) {
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
    )
}

/// The four nodes of the table's cell about a point inside it, and the cubic Hermite bases at the
/// point along each axis with their rates.
struct Patch {
    /// The nodes at `(i, j)`, `(i, j + 1)`, `(i + 1, j)` and `(i + 1, j + 1)`, the cell's lower
    /// corner at row `i`, column `j`.
    nodes: [[f64; 4]; 4],
    radial: [f64; 4],
    radial_rate: [f64; 4],
    vertical: [f64; 4],
    vertical_rate: [f64; 4],
}

impl Patch {
    /// The patch of the table `nodes` about the point inside it whose `(ln R, ln |z|)` are `logs`.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    fn new(nodes: &[[f64; 4]], [ln_r, ln_z]: [f64; 2]) -> Self {
        let (row, across) = cell(ln_r);
        let (column, up) = cell(ln_z);
        let corner = row * NODES + column;
        let (lower, upper) = (
            &nodes[corner..corner + 2],
            &nodes[corner + NODES..corner + NODES + 2],
        );
        Self {
            nodes: [lower[0], lower[1], upper[0], upper[1]],
            radial: hermite(across),
            radial_rate: hermite_slope(across),
            vertical: hermite(up),
            vertical_rate: hermite_slope(up),
        }
    }

    /// The interpolant's derivatives in `ln R` and `ln |z|`, (km/s)².
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    fn slopes(&self) -> [f64; 2] {
        // The basis's rates are per cell; one cell is the step in the logarithm.
        [
            self.sum(self.radial_rate, self.vertical) / GRID_LN_STEP,
            self.sum(self.radial, self.vertical_rate) / GRID_LN_STEP,
        ]
    }

    /// The sum over the four nodes of the bicubic terms with the bases `radial` and `vertical`
    /// along the two axes, in node order.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    fn sum(&self, radial: [f64; 4], vertical: [f64; 4]) -> f64 {
        // Basis weights of the value and of the slope, along each axis: `[value, slope]` pairs.
        let weights = |basis: [f64; 4], k: usize| [basis[k], basis[k + 2]];
        let mut total = 0.0;
        for (n, [value, by_r, by_z, by_both]) in self.nodes.into_iter().enumerate() {
            let ([r_value, r_slope], [z_value, z_slope]) =
                (weights(radial, n / 2), weights(vertical, n % 2));
            total += r_value * z_value * value
                + GRID_LN_STEP * (r_slope * z_value * by_r + r_value * z_slope * by_z)
                + GRID_LN_STEP * GRID_LN_STEP * r_slope * z_slope * by_both;
        }
        total
    }
}

/// The cell of the table holding a point inside it whose coordinate's logarithm is `ln_x`, and
/// the position in it, 0–1.
#[must_use]
#[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
#[inline(always)]
fn cell(ln_x: f64) -> (usize, f64) {
    let position = ((ln_x - GRID_LN_FIRST) / GRID_LN_STEP).clamp(0.0, LAST_CELL);
    // Clamped below 69, the position's truncation is its floor, and a NaN, which no finite point
    // gives, lands in cell 0 as the floor's cast left it. `f64::floor` is a library call on
    // x86-64's baseline, which lacks SSE4.1's `roundsd`: with it, a shell orbit's step cost 52 ns
    // against 49 (P10.T4's list bench, pod, 16 vCPU).
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "truncates a position clamped to 0–68.999999999, as the floor would"
    )]
    let index = position as u8;
    (usize::from(index), position - f64::from(index))
}

/// [`within`] of four lanes.
#[must_use]
#[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
#[inline(always)]
fn within_x4(r: F64x4, height: F64x4, by_ln_r: F64x4, by_ln_z: F64x4) -> (F64x4, F64x4) {
    let (first, zero) = (F64x4::splat(GRID_FIRST_LY), F64x4::splat(0.0));
    (
        r.gt(first).select(by_ln_r / (r * r), zero),
        height.gt(first).select(by_ln_z / height, zero),
    )
}

/// Four [`Patch`]es side by side, a lane each: each value of the four nodes of each lane's cell,
/// and the bases at each lane's point.
struct PatchX4 {
    /// The nodes at `(i, j)`, `(i, j + 1)`, `(i + 1, j)` and `(i + 1, j + 1)` of each lane's
    /// cell, as [`Patch`]'s, each of their four values a lane each.
    nodes: [[F64x4; 4]; 4],
    radial: [F64x4; 4],
    radial_rate: [F64x4; 4],
    vertical: [F64x4; 4],
    vertical_rate: [F64x4; 4],
}

impl PatchX4 {
    /// [`Patch::new`] of four points, a lane each.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    fn new(nodes: &[[f64; 4]], [ln_r, ln_z]: [F64x4; 2]) -> Self {
        let (rows, across) = cell_x4(ln_r);
        let (columns, up) = cell_x4(ln_z);
        let cell = |k: usize| {
            let corner = rows[k] * NODES + columns[k];
            &nodes[corner..corner + NODES + 2]
        };
        let cells = [cell(0), cell(1), cell(2), cell(3)];
        let value = |at: usize, value: usize| {
            F64x4::new([
                cells[0][at][value],
                cells[1][at][value],
                cells[2][at][value],
                cells[3][at][value],
            ])
        };
        let node = |at: usize| [value(at, 0), value(at, 1), value(at, 2), value(at, 3)];
        Self {
            nodes: [node(0), node(1), node(NODES), node(NODES + 1)],
            radial: hermite_x4(across),
            radial_rate: hermite_slope_x4(across),
            vertical: hermite_x4(up),
            vertical_rate: hermite_slope_x4(up),
        }
    }

    /// [`Patch::slopes`] of each lane.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    fn slopes(&self) -> [F64x4; 2] {
        let step = F64x4::splat(GRID_LN_STEP);
        [
            self.sum(self.radial_rate, self.vertical) / step,
            self.sum(self.radial, self.vertical_rate) / step,
        ]
    }

    /// [`Patch::sum`] of each lane, in node order.
    #[must_use]
    #[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
    #[inline(always)]
    fn sum(&self, radial: [F64x4; 4], vertical: [F64x4; 4]) -> F64x4 {
        let weights = |basis: [F64x4; 4], k: usize| [basis[k], basis[k + 2]];
        // `Patch::sum`'s `GRID_LN_STEP * GRID_LN_STEP * …` multiplies the two constants first.
        let (step, step_squared) = (
            F64x4::splat(GRID_LN_STEP),
            F64x4::splat(GRID_LN_STEP * GRID_LN_STEP),
        );
        let mut total = F64x4::splat(0.0);
        for (n, &[value, by_r, by_z, by_both]) in self.nodes.iter().enumerate() {
            let ([r_value, r_slope], [z_value, z_slope]) =
                (weights(radial, n / 2), weights(vertical, n % 2));
            total = total
                + (r_value * z_value * value
                    + step * (r_slope * z_value * by_r + r_value * z_slope * by_z)
                    + step_squared * r_slope * z_slope * by_both);
        }
        total
    }
}

/// [`cell`] of four lanes: each lane's cell, and the positions in them.
#[must_use]
#[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
#[inline(always)]
fn cell_x4(ln_x: F64x4) -> ([usize; 4], F64x4) {
    let position = ((ln_x - F64x4::splat(GRID_LN_FIRST)) / F64x4::splat(GRID_LN_STEP))
        .at_least(F64x4::splat(0.0))
        .at_most(F64x4::splat(LAST_CELL));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "truncates a position clamped to 0–68.999999999, as `cell` does"
    )]
    let index = {
        let p = position.to_array();
        [p[0] as u8, p[1] as u8, p[2] as u8, p[3] as u8]
    };
    (
        [
            usize::from(index[0]),
            usize::from(index[1]),
            usize::from(index[2]),
            usize::from(index[3]),
        ],
        position
            - F64x4::new([
                f64::from(index[0]),
                f64::from(index[1]),
                f64::from(index[2]),
                f64::from(index[3]),
            ]),
    )
}

/// [`hermite`] of four lanes.
#[must_use]
#[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
#[inline(always)]
fn hermite_x4(t: F64x4) -> [F64x4; 4] {
    let (one, two, three, minus_two) = (
        F64x4::splat(1.0),
        F64x4::splat(2.0),
        F64x4::splat(3.0),
        F64x4::splat(-2.0),
    );
    let t2 = t * t;
    let t3 = t2 * t;
    [
        two * t3 - three * t2 + one,
        minus_two * t3 + three * t2,
        t3 - two * t2 + t,
        t3 - t2,
    ]
}

/// [`hermite_slope`] of four lanes.
#[must_use]
#[expect(clippy::inline_always, reason = "measured: see `SprayForce::locate`")]
#[inline(always)]
fn hermite_slope_x4(t: F64x4) -> [F64x4; 4] {
    let (one, two, three) = (F64x4::splat(1.0), F64x4::splat(2.0), F64x4::splat(3.0));
    let (four, six, minus_six) = (F64x4::splat(4.0), F64x4::splat(6.0), F64x4::splat(-6.0));
    let t2 = t * t;
    [
        six * t2 - six * t,
        minus_six * t2 + six * t,
        three * t2 - four * t + one,
        three * t2 - two * t,
    ]
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
        let (at, t) = cell(math::ln(0.0625));
        assert_eq!(at, 0);
        assert!(t.abs() < 1e-12);
        let (at, t) = cell(math::ln(grid_point(NODES - 1) * 2.0));
        assert_eq!(at, NODES - 2);
        assert!((t - 1.0).abs() < 1e-8);
        let (at, t) = cell(math::ln(0.01));
        assert_eq!(at, 0);
        assert!(t.abs() < f64::EPSILON);
    }
}
