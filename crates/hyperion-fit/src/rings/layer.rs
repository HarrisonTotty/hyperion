//! The particle layer the shadowing table traces (plan R11, R11.T9.b; Design note 15): its sizes,
//! its vertical profile, the law tying its central filling factor to its optical depth, and a
//! frozen field of hard spheres in a box periodic in x and y, with the ray casts the tracer needs.
//!
//! - **Sizes.** Radii run dN/dR ∝ R⁻³ between `R_min` and `R_max` ([`PowerLawSizes`]), so that
//!   ⟨R³⟩ ÷ ⟨R²⟩ = (`R_max` − `R_min`) ÷ ln(`R_max` ÷ `R_min`) (Salo and French 2010, Icarus 210, 785,
//!   §3.3–3.4, whose layers run R⁻³ with `R_max` = 5 m and `R_min` ÷ `R_max` of 0.02–0.2).
//! - **Profile.** The number density of centres is Gaussian in height, n(z) = n₀ exp(−z² ÷ 2σ²),
//!   with `H_eff` ≡ √(2π) σ, or uniform over a slab of thickness H ([`VerticalProfile`]). With `τ_dyn`
//!   = ∫ n π⟨R²⟩ dz the dynamical (geometric) optical depth, the central filling factor is D₀ =
//!   n₀ (4/3)π⟨R³⟩ = `τ_dyn` (4/3)⟨R³⟩ ÷ (⟨R²⟩ `H_eff`), exact for either profile ([`Layer`]).
//! - **The law** ([`FillingLaw`]): D₀ = 0.35 at `τ_dyn` = 1.5, the middle of Salo and French's
//!   0.32–0.38, so `H_eff` = 11.17 m over 0.5–5 m, held fixed, so that D₀ is proportional to `τ_dyn`,
//!   up to the cap of 0.40, above which `H_eff` grows (Design note 15; the cap a lean of low
//!   confidence, R11's Risks).
//! - **The field** (`Field`). The radii are drawn stratified in their quantile, so that the box's
//!   `τ_dyn` is the layer's to a part in 10⁴, and each centre's height from the profile; the spheres
//!   are then placed largest first by random sequential addition, each at its drawn height, its x
//!   and y redrawn until it overlaps no sphere placed before it (or an image of one across the
//!   box's sides). The vertical profile is therefore exactly the drawn one. The spheres are kept in
//!   one grid per size class, cells at least the class's largest diameter, every sphere listed in
//!   every cell its bounding box meets, and a ray walks each grid cell by cell (Amanatides and Woo
//!   1987, Eurographics '87, 3).
//!
//! Every draw is counter-based (the crate's [`Draws`]), keyed by the run's seed, the field's key and
//! the particle's index, so a field is a pure function of them; nothing here is generated output.

use core::f64::consts::PI;

use hyperion_sim::math;

use crate::atmosphere::geometry::Vec3;
use crate::tasks::displaced_forms::births::Draws;

/// A layer could not be described: a size range, profile or optical depth out of its domain.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum DescribeLayerError {
    /// The radii were not finite with 0 < `R_min` < `R_max`.
    #[error(
        "the radii must be finite with 0 < r_min < r_max, not {min_radius_m} and {max_radius_m} m"
    )]
    Radii {
        /// `R_min`, m.
        min_radius_m: f64,
        /// `R_max`, m.
        max_radius_m: f64,
    },
    /// The dynamical optical depth was not finite and positive.
    #[error("the dynamical optical depth must be finite and positive, not {0}")]
    OpticalDepth(f64),
    /// The filling factor was not in (0, 1).
    #[error("the filling factor must lie in (0, 1), not {0}")]
    FillingFactor(f64),
}

/// Particle radii distributed as dN/dR ∝ R⁻³ between `R_min` and `R_max` (Salo and French 2010, §3.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerLawSizes {
    min_radius_m: f64,
    max_radius_m: f64,
}

impl PowerLawSizes {
    /// Radii from `min_radius_m` to `max_radius_m`, m.
    ///
    /// # Errors
    ///
    /// [`DescribeLayerError::Radii`] unless both are finite with 0 < `R_min` < `R_max`.
    pub fn new(min_radius_m: f64, max_radius_m: f64) -> Result<Self, DescribeLayerError> {
        if min_radius_m.is_finite()
            && max_radius_m.is_finite()
            && min_radius_m > 0.0
            && max_radius_m > min_radius_m
        {
            Ok(Self {
                min_radius_m,
                max_radius_m,
            })
        } else {
            Err(DescribeLayerError::Radii {
                min_radius_m,
                max_radius_m,
            })
        }
    }

    /// `R_min`, m.
    #[must_use]
    pub fn min_radius_m(&self) -> f64 {
        self.min_radius_m
    }

    /// `R_max`, m.
    #[must_use]
    pub fn max_radius_m(&self) -> f64 {
        self.max_radius_m
    }

    /// W = `R_max` ÷ `R_min`.
    #[must_use]
    pub fn radius_ratio(&self) -> f64 {
        self.max_radius_m / self.min_radius_m
    }

    /// ∫ R⁻³ dR over the range, (`R_min`⁻² − `R_max`⁻²) ÷ 2, m⁻²: the number per unit of the law's
    /// constant.
    #[must_use]
    fn number_integral(&self) -> f64 {
        0.5 * (1.0 / (self.min_radius_m * self.min_radius_m)
            - 1.0 / (self.max_radius_m * self.max_radius_m))
    }

    /// ⟨R²⟩, m²: ln W ÷ ∫ R⁻³ dR.
    #[must_use]
    pub fn mean_square_radius_m2(&self) -> f64 {
        math::ln(self.radius_ratio()) / self.number_integral()
    }

    /// ⟨R³⟩, m³: (`R_max` − `R_min`) ÷ ∫ R⁻³ dR.
    #[must_use]
    pub fn mean_cube_radius_m3(&self) -> f64 {
        (self.max_radius_m - self.min_radius_m) / self.number_integral()
    }

    /// ⟨R³⟩ ÷ ⟨R²⟩ = (`R_max` − `R_min`) ÷ ln W, m (Design note 15).
    #[must_use]
    pub fn cube_to_square_m(&self) -> f64 {
        (self.max_radius_m - self.min_radius_m) / math::ln(self.radius_ratio())
    }

    /// The radius at quantile `u` in [0, 1], m: the inverse of F(R) = (`R_min`⁻² − R⁻²) ÷ (`R_min`⁻² −
    /// `R_max`⁻²).
    #[must_use]
    pub fn radius_at(&self, u: f64) -> f64 {
        let inverse_square =
            1.0 / (self.min_radius_m * self.min_radius_m) - 2.0 * u * self.number_integral();
        (1.0 / inverse_square.max(1.0 / (self.max_radius_m * self.max_radius_m)))
            .sqrt()
            .clamp(self.min_radius_m, self.max_radius_m)
    }
}

/// How the number density of particle centres runs with height z about the mid-plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalProfile {
    /// n(z) = n₀ exp(−z² ÷ 2σ²); `H_eff` ≡ √(2π) σ.
    Gaussian {
        /// σ, m.
        sigma_m: f64,
    },
    /// n(z) = n₀ for |z| < H ÷ 2, else 0; `H_eff` = H.
    Uniform {
        /// H, m.
        thickness_m: f64,
    },
}

/// A profile's form, without its scale: what [`Layer::with_filling_factor`] solves the scale of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileShape {
    /// [`VerticalProfile::Gaussian`].
    Gaussian,
    /// [`VerticalProfile::Uniform`].
    Uniform,
}

/// √(2π), the Gaussian's `H_eff` ÷ σ.
const SQRT_TWO_PI: f64 = 2.506_628_274_631_000_5;

impl VerticalProfile {
    /// `H_eff` = ∫ n dz ÷ n₀, m: √(2π) σ, or H.
    #[must_use]
    pub fn effective_thickness_m(&self) -> f64 {
        match *self {
            Self::Gaussian { sigma_m } => SQRT_TWO_PI * sigma_m,
            Self::Uniform { thickness_m } => thickness_m,
        }
    }

    /// The profile of `shape` whose `H_eff` is `effective_thickness_m`.
    #[must_use]
    pub fn of_thickness(shape: ProfileShape, effective_thickness_m: f64) -> Self {
        match shape {
            ProfileShape::Gaussian => Self::Gaussian {
                sigma_m: effective_thickness_m / SQRT_TWO_PI,
            },
            ProfileShape::Uniform => Self::Uniform {
                thickness_m: effective_thickness_m,
            },
        }
    }

    /// A centre's height drawn from the profile, m.
    #[must_use]
    fn height_m(&self, draws: &mut Draws) -> f64 {
        match *self {
            Self::Gaussian { sigma_m } => sigma_m * draws.normal(),
            Self::Uniform { thickness_m } => (draws.uniform() - 0.5) * thickness_m,
        }
    }
}

/// A ring's particle layer: its sizes, its vertical profile and its dynamical optical depth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    sizes: PowerLawSizes,
    profile: VerticalProfile,
    dynamical_optical_depth: f64,
}

impl Layer {
    /// The layer of `sizes` and `shape` at dynamical optical depth `tau_dyn` whose central filling
    /// factor is `filling_factor`: `H_eff` = `τ_dyn` (4/3)⟨R³⟩ ÷ (⟨R²⟩ D₀) (Design note 15).
    ///
    /// # Errors
    ///
    /// [`DescribeLayerError::OpticalDepth`] or [`DescribeLayerError::FillingFactor`] for a value
    /// out of its domain.
    pub fn with_filling_factor(
        sizes: PowerLawSizes,
        shape: ProfileShape,
        tau_dyn: f64,
        filling_factor: f64,
    ) -> Result<Self, DescribeLayerError> {
        if !(tau_dyn.is_finite() && tau_dyn > 0.0) {
            return Err(DescribeLayerError::OpticalDepth(tau_dyn));
        }
        if !(filling_factor > 0.0 && filling_factor < 1.0) {
            return Err(DescribeLayerError::FillingFactor(filling_factor));
        }
        let thickness_m = tau_dyn * (4.0 / 3.0) * sizes.cube_to_square_m() / filling_factor;
        Ok(Self {
            sizes,
            profile: VerticalProfile::of_thickness(shape, thickness_m),
            dynamical_optical_depth: tau_dyn,
        })
    }

    /// The particle sizes.
    #[must_use]
    pub fn sizes(&self) -> PowerLawSizes {
        self.sizes
    }

    /// The vertical profile.
    #[must_use]
    pub fn profile(&self) -> VerticalProfile {
        self.profile
    }

    /// `τ_dyn` = ∫ n π⟨R²⟩ dz.
    #[must_use]
    pub fn dynamical_optical_depth(&self) -> f64 {
        self.dynamical_optical_depth
    }

    /// `H_eff`, m.
    #[must_use]
    pub fn effective_thickness_m(&self) -> f64 {
        self.profile.effective_thickness_m()
    }

    /// D₀ = n₀ (4/3)π⟨R³⟩ = `τ_dyn` (4/3)⟨R³⟩ ÷ (⟨R²⟩ `H_eff`).
    #[must_use]
    pub fn central_filling_factor(&self) -> f64 {
        self.dynamical_optical_depth * (4.0 / 3.0) * self.sizes.cube_to_square_m()
            / self.effective_thickness_m()
    }

    /// The particles over a square metre of the ring plane, `τ_dyn` ÷ (π⟨R²⟩), m⁻².
    #[must_use]
    pub fn column_number_per_m2(&self) -> f64 {
        self.dynamical_optical_depth / (PI * self.sizes.mean_square_radius_m2())
    }
}

/// The law tying a Gaussian layer's central filling factor to its dynamical optical depth: D₀ =
/// `D_ref` `τ_dyn` ÷ `τ_ref` at the `H_eff` that gives `D_ref` at `τ_ref`, up to a cap (Design note 15).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillingLaw {
    reference_optical_depth: f64,
    reference_filling_factor: f64,
    cap: f64,
}

/// The table's law: D₀ = 0.35 at `τ_dyn` = 1.5, the middle of Salo and French 2010's central filling
/// factors of 0.32–0.38 (§3.4), proportional to `τ_dyn` at fixed `H_eff`, and capped at 0.40 (Design
/// note 15; the cap a lean of low confidence, R11's Risks).
pub const SALO_FRENCH_LAW: FillingLaw = FillingLaw {
    reference_optical_depth: 1.5,
    reference_filling_factor: 0.35,
    cap: 0.40,
};

impl FillingLaw {
    /// `τ_ref`, at which D₀ = `D_ref`.
    #[must_use]
    pub fn reference_optical_depth(&self) -> f64 {
        self.reference_optical_depth
    }

    /// `D_ref`.
    #[must_use]
    pub fn reference_filling_factor(&self) -> f64 {
        self.reference_filling_factor
    }

    /// The cap on D₀.
    #[must_use]
    pub fn cap(&self) -> f64 {
        self.cap
    }

    /// D₀ at `tau_dyn`: min(`D_ref` `τ_dyn` ÷ `τ_ref`, cap).
    #[must_use]
    pub fn filling_factor(&self, tau_dyn: f64) -> f64 {
        (self.reference_filling_factor * tau_dyn / self.reference_optical_depth).min(self.cap)
    }

    /// The Gaussian layer of `sizes` at `tau_dyn` under the law.
    ///
    /// # Errors
    ///
    /// As [`Layer::with_filling_factor`].
    pub fn layer(&self, sizes: PowerLawSizes, tau_dyn: f64) -> Result<Layer, DescribeLayerError> {
        Layer::with_filling_factor(
            sizes,
            ProfileShape::Gaussian,
            tau_dyn,
            self.filling_factor(tau_dyn),
        )
    }
}

/// A field could not be built.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum BuildFieldError {
    /// The box's side was not finite or not above four of the largest diameters.
    #[error("the box's side must be finite and above 8 r_max ({min_m} m), not {box_m} m")]
    Box {
        /// The side asked for, m.
        box_m: f64,
        /// The least side, m.
        min_m: f64,
    },
    /// The layer asks for more particles than a field holds.
    #[error("the field would hold {0} particles, more than {MAX_PARTICLES}")]
    TooManyParticles(f64),
    /// The layer asks for less than one particle over the box: a box too small for its optical depth.
    #[error("the field would hold {0} particles, less than one")]
    TooFewParticles(f64),
    /// A sphere found no room in [`MAX_PLACEMENT_ATTEMPTS`] draws: the layer is too dense for
    /// random sequential addition.
    #[error(
        "sphere {index} (radius {radius_m} m at height {height_m} m) found no room in \
         {MAX_PLACEMENT_ATTEMPTS} draws: the layer is jammed"
    )]
    Jammed {
        /// The sphere's index, in the order of the draws.
        index: usize,
        /// Its radius, m.
        radius_m: f64,
        /// Its centre's height, m.
        height_m: f64,
    },
}

/// The most particles a field holds.
pub const MAX_PARTICLES: usize = 50_000_000;

/// How many x, y draws a sphere is given before the field is declared jammed.
pub const MAX_PLACEMENT_ATTEMPTS: u32 = 100_000;

/// The ratio of a size class's largest radius to its smallest, at most.
const CLASS_RATIO: f64 = 2.5;

/// The mean number of spheres a cell of a class's grid aims at in the mid-plane, where a cell is
/// not held larger by the class's diameter.
const SPHERES_PER_CELL: f64 = 2.0;

/// The longest path a ray is followed, m; past it, a ray still in the layer is lost.
pub const MAX_PATH_M: f64 = 2.0e5;

/// What a ray does, from where it starts: hits a sphere, or leaves the layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Flight {
    /// It hits the sphere of centre `centre` (as seen from the ray's frame, across the box's
    /// sides) and radius `radius_m` at distance `distance_m`.
    Hit {
        /// The distance along the ray, m.
        distance_m: f64,
        /// The sphere's centre, m, in the ray's (unwrapped) frame.
        centre: Vec3,
        /// Its radius, m.
        radius_m: f64,
    },
    /// It leaves through the top of the layer.
    Top,
    /// It leaves through the bottom.
    Bottom,
    /// It is still in the layer after [`MAX_PATH_M`]: a near-horizontal ray in the sparse wings.
    Lost,
}

/// A sphere as listed in a cell: its centre in the frame of the cell's place in the box (shifted by
/// a side across the box's edges), and its radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sphere {
    pub(crate) centre: Vec3,
    pub(crate) radius_m: f64,
}

/// The distance along the unit ray `origin + t direction` to its entry into `sphere`, if it enters
/// it ahead (t > 0); a ray starting inside a sphere is taken to miss it.
#[must_use]
fn entry_distance(origin: Vec3, direction: Vec3, centre: Vec3, radius_m: f64) -> Option<f64> {
    let oc = origin - centre;
    let b = oc.dot(direction);
    let c = oc.dot(oc) - radius_m * radius_m;
    if c <= 0.0 || b >= 0.0 {
        // Inside, or moving away from the centre while outside: no entry ahead.
        return None;
    }
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        return None;
    }
    // The nearer root of t² + 2bt + c = 0 without cancellation: c ÷ (−b + √disc) (Press et al.
    // 2007, Numerical Recipes, 3rd ed., §5.6).
    let t = c / (-b + discriminant.sqrt());
    (t > 0.0).then_some(t)
}

/// ⌊x⌋ as an index: x is a coordinate over a cell's side, within ±[`MAX_PATH_M`] ÷ 10⁻³ of 0.
#[must_use]
fn floor_index(x: f64) -> i64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "positions and cells keep |x| below 10¹², far inside i64"
    )]
    let i = x.floor() as i64;
    i
}

/// The cells of one size class over the box: periodic in x and y, bounded in z.
#[derive(Debug, Clone, PartialEq)]
struct Lattice {
    box_m: f64,
    n_xy: i64,
    cell_xy_m: f64,
    n_z: i64,
    cell_z_m: f64,
    z_lo_m: f64,
}

impl Lattice {
    /// Cells of side at least `min_cell_m` over a box of side `box_m` and heights `z_lo_m` to
    /// `z_hi_m`.
    #[must_use]
    fn new(box_m: f64, z_lo_m: f64, z_hi_m: f64, min_cell_m: f64) -> Self {
        let n_xy = floor_index(box_m / min_cell_m).max(1);
        let n_z = floor_index((z_hi_m - z_lo_m) / min_cell_m).max(1);
        #[expect(clippy::cast_precision_loss, reason = "cell counts are far below 2⁵³")]
        let (cell_xy_m, cell_z_m) = (box_m / n_xy as f64, (z_hi_m - z_lo_m) / n_z as f64);
        Self {
            box_m,
            n_xy,
            cell_xy_m,
            n_z,
            cell_z_m,
            z_lo_m,
        }
    }

    #[must_use]
    fn cell_count(&self) -> usize {
        usize::try_from(self.n_xy * self.n_xy * self.n_z).expect("cell counts fit a usize")
    }

    /// The flat index of the cell at unwrapped (i, j) and k in range, and the shift, m, that takes
    /// a centre listed there into the frame of the unwrapped cell.
    #[must_use]
    fn cell(&self, i: i64, j: i64, k: i64) -> (usize, f64, f64) {
        let (wi, si) = (i.rem_euclid(self.n_xy), i.div_euclid(self.n_xy));
        let (wj, sj) = (j.rem_euclid(self.n_xy), j.div_euclid(self.n_xy));
        let index = usize::try_from((k * self.n_xy + wj) * self.n_xy + wi)
            .expect("a cell in range has a non-negative index");
        #[expect(
            clippy::cast_precision_loss,
            reason = "box counts along a ray stay far below 2⁵³"
        )]
        let shift = (si as f64 * self.box_m, sj as f64 * self.box_m);
        (index, shift.0, shift.1)
    }

    /// The unwrapped cell ranges, inclusive, that the box [lo, hi] meets; z clamped to the layer.
    #[must_use]
    fn cells_of_box(&self, lo: Vec3, hi: Vec3) -> ([i64; 2], [i64; 2], [i64; 2]) {
        let k = |z: f64| floor_index((z - self.z_lo_m) / self.cell_z_m).clamp(0, self.n_z - 1);
        (
            [
                floor_index(lo.x / self.cell_xy_m),
                floor_index(hi.x / self.cell_xy_m),
            ],
            [
                floor_index(lo.y / self.cell_xy_m),
                floor_index(hi.y / self.cell_xy_m),
            ],
            [k(lo.z), k(hi.z)],
        )
    }
}

/// How a cast stops: at the nearest sphere, or at any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Nearest,
    Any,
}

/// One size class's spheres, listed by cell (compressed rows: cell c's spheres are
/// `spheres[starts[c]..starts[c + 1]]`).
#[derive(Debug, Clone, PartialEq)]
struct ClassGrid {
    lattice: Lattice,
    starts: Vec<usize>,
    spheres: Vec<Sphere>,
}

impl ClassGrid {
    /// The spheres of cell (i, j, k), in the unwrapped cell's frame.
    fn spheres_of(&self, i: i64, j: i64, k: i64) -> impl Iterator<Item = Sphere> + '_ {
        let (index, sx, sy) = self.lattice.cell(i, j, k);
        self.spheres[self.starts[index]..self.starts[index + 1]]
            .iter()
            .map(move |s| Sphere {
                centre: Vec3::new(s.centre.x + sx, s.centre.y + sy, s.centre.z),
                radius_m: s.radius_m,
            })
    }

    /// The first sphere the ray `origin + t direction`, t in [`t_start`, `t_end`], enters, walking the
    /// cells in order; with [`Stop::Any`], any sphere it enters before `t_end`.
    #[must_use]
    fn cast(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_start: f64,
        t_end: f64,
        stop: Stop,
    ) -> Option<(f64, Sphere)> {
        let lattice = &self.lattice;
        let start = origin + direction * t_start;
        let mut i = floor_index(start.x / lattice.cell_xy_m);
        let mut j = floor_index(start.y / lattice.cell_xy_m);
        let mut k =
            floor_index((start.z - lattice.z_lo_m) / lattice.cell_z_m).clamp(0, lattice.n_z - 1);
        // Per axis: the step, the t of the next boundary and the t across a cell.
        #[expect(
            clippy::cast_precision_loss,
            reason = "cell indices along a ray stay far below 2⁵³"
        )]
        let axis = |d: f64, x: f64, cell: f64, index: i64, base: f64| -> (i64, f64, f64) {
            if d > 0.0 {
                let next = base + (index + 1) as f64 * cell;
                (1, t_start + (next - x) / d, cell / d)
            } else if d < 0.0 {
                let next = base + index as f64 * cell;
                (-1, t_start + (next - x) / d, -cell / d)
            } else {
                (0, f64::INFINITY, f64::INFINITY)
            }
        };
        let (step_x, mut next_x, delta_x) = axis(direction.x, start.x, lattice.cell_xy_m, i, 0.0);
        let (step_y, mut next_y, delta_y) = axis(direction.y, start.y, lattice.cell_xy_m, j, 0.0);
        let (step_z, mut next_z, delta_z) =
            axis(direction.z, start.z, lattice.cell_z_m, k, lattice.z_lo_m);
        let mut best: Option<(f64, Sphere)> = None;
        loop {
            let cell_exit = next_x.min(next_y).min(next_z).min(t_end);
            for sphere in self.spheres_of(i, j, k) {
                if let Some(t) = entry_distance(origin, direction, sphere.centre, sphere.radius_m)
                    && t >= t_start
                    && t < best.map_or(t_end, |(b, _)| b)
                {
                    if stop == Stop::Any {
                        return Some((t, sphere));
                    }
                    best = Some((t, sphere));
                }
            }
            if let Some((t, _)) = best
                && t <= cell_exit
            {
                return best;
            }
            if cell_exit >= t_end {
                return best;
            }
            if next_x <= next_y && next_x <= next_z {
                i += step_x;
                next_x += delta_x;
            } else if next_y <= next_z {
                j += step_y;
                next_y += delta_y;
            } else {
                k += step_z;
                next_z += delta_z;
                if k < 0 || k >= lattice.n_z {
                    return best;
                }
            }
        }
    }
}

/// A class's grid while the field is placed: one list per cell.
#[derive(Debug, Clone, PartialEq)]
struct ClassBuilder {
    lattice: Lattice,
    cells: Vec<Vec<Sphere>>,
    count: usize,
}

impl ClassBuilder {
    #[must_use]
    fn new(lattice: Lattice) -> Self {
        let cells = vec![Vec::new(); lattice.cell_count()];
        Self {
            lattice,
            cells,
            count: 0,
        }
    }

    /// Whether a sphere of `radius_m` at `centre` (x and y in the box) overlaps one of the class.
    #[must_use]
    fn overlaps(&self, centre: Vec3, radius_m: f64) -> bool {
        if self.count == 0 {
            return false;
        }
        let r = Vec3::new(radius_m, radius_m, radius_m);
        let (is, js, ks) = self.lattice.cells_of_box(centre - r, centre + r);
        for k in ks[0]..=ks[1] {
            for j in js[0]..=js[1] {
                for i in is[0]..=is[1] {
                    let (index, sx, sy) = self.lattice.cell(i, j, k);
                    for s in &self.cells[index] {
                        let d = centre - Vec3::new(s.centre.x + sx, s.centre.y + sy, s.centre.z);
                        let reach = radius_m + s.radius_m;
                        if d.dot(d) < reach * reach {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Lists a sphere at `centre` (x and y in the box) in every cell its bounding box meets, in
    /// that cell's frame.
    fn insert(&mut self, centre: Vec3, radius_m: f64) {
        let r = Vec3::new(radius_m, radius_m, radius_m);
        let (is, js, ks) = self.lattice.cells_of_box(centre - r, centre + r);
        for k in ks[0]..=ks[1] {
            for j in js[0]..=js[1] {
                for i in is[0]..=is[1] {
                    let (index, sx, sy) = self.lattice.cell(i, j, k);
                    self.cells[index].push(Sphere {
                        centre: Vec3::new(centre.x - sx, centre.y - sy, centre.z),
                        radius_m,
                    });
                }
            }
        }
        self.count += 1;
    }

    #[must_use]
    fn freeze(self) -> ClassGrid {
        let mut starts = Vec::with_capacity(self.cells.len() + 1);
        let total = self.cells.iter().map(Vec::len).sum();
        let mut spheres = Vec::with_capacity(total);
        starts.push(0);
        for cell in self.cells {
            spheres.extend(cell);
            starts.push(spheres.len());
        }
        ClassGrid {
            lattice: self.lattice,
            starts,
            spheres,
        }
    }
}

/// The stream a field's, or a trace's, draws come from: the run's seed and the item's own key
/// (the crate's [`Draws`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FieldKey {
    /// The run's seed.
    pub(crate) seed: u64,
    /// The item's own key within the run.
    pub(crate) key: u64,
}

/// A frozen field of hard spheres over a box periodic in x and y: the layer the tracer walks.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Field {
    box_m: f64,
    z_lo_m: f64,
    z_hi_m: f64,
    count: usize,
    area_sum_m2: f64,
    volume_sum_m3: f64,
    effective_thickness_m: f64,
    spheres: Vec<Sphere>,
    grids: Vec<ClassGrid>,
}

/// The radii and heights of `count` spheres of `layer`, with each sphere's stream left at its next
/// draw: radii stratified in their quantile and heights from the profile, both from the sphere's
/// own stream, so that index order is radius order, ascending.
#[must_use]
fn draw_spheres(layer: &Layer, count: usize, key: FieldKey) -> Vec<(f64, f64, Draws)> {
    let sizes = layer.sizes();
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts below 5 × 10⁷ are exact in an f64"
    )]
    let n = count as f64;
    (0_u64..)
        .zip(0..count)
        .map(|(index, _)| {
            let mut draws = Draws::new(key.seed, key.key, index);
            #[expect(
                clippy::cast_precision_loss,
                reason = "indices below 5 × 10⁷ are exact in an f64"
            )]
            let u = (index as f64 + draws.uniform()) / n;
            let radius_m = sizes.radius_at(u);
            let height_m = layer.profile().height_m(&mut draws);
            (radius_m, height_m, draws)
        })
        .collect()
}

/// The size classes of a field: [`CLASS_RATIO`]-wide bins of radius, largest first.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Classes {
    min_radius_m: f64,
    max_radius_m: f64,
    ratio: f64,
    count: usize,
}

impl Classes {
    #[must_use]
    fn of(sizes: PowerLawSizes) -> Self {
        let ratio = sizes.radius_ratio();
        let count = usize::try_from(floor_index(math::ln(ratio) / math::ln(CLASS_RATIO)).max(0))
            .expect("a non-negative count")
            + 1;
        Self {
            min_radius_m: sizes.min_radius_m(),
            max_radius_m: sizes.max_radius_m(),
            ratio,
            count,
        }
    }

    #[must_use]
    fn count_f64(&self) -> f64 {
        f64::from(u32::try_from(self.count).expect("a handful of classes"))
    }

    /// The class of a sphere of radius `radius_m`, 0 the largest.
    #[must_use]
    fn class_of(&self, radius_m: f64) -> usize {
        let from_small = floor_index(
            math::ln(radius_m / self.min_radius_m) / math::ln(self.ratio) * self.count_f64(),
        );
        let last = i64::try_from(self.count).expect("a handful of classes") - 1;
        self.count - 1 - usize::try_from(from_small.clamp(0, last)).expect("clamped non-negative")
    }

    /// The largest radius of class `c`, m.
    #[must_use]
    fn largest_m(&self, c: usize) -> f64 {
        let from_small = f64::from(u32::try_from(self.count - c).expect("a handful of classes"));
        (self.min_radius_m * math::powf(self.ratio, from_small / self.count_f64()))
            .min(self.max_radius_m)
    }

    /// Each class's empty grid over a box of side `box_m` and heights `span_m`, its cells at least
    /// the class's largest diameter and about [`SPHERES_PER_CELL`] spheres of the class in the
    /// mid-plane.
    #[must_use]
    fn builders(
        &self,
        radii_m: impl Iterator<Item = f64>,
        box_m: f64,
        span_m: (f64, f64),
        thickness_m: f64,
    ) -> Vec<ClassBuilder> {
        let mut counts = vec![0_u32; self.count];
        for r in radii_m {
            counts[self.class_of(r)] += 1;
        }
        counts
            .iter()
            .enumerate()
            .map(|(c, &n)| {
                let occupancy_m = math::cbrt(
                    SPHERES_PER_CELL * box_m * box_m * thickness_m / f64::from(n.max(1)),
                );
                ClassBuilder::new(Lattice::new(
                    box_m,
                    span_m.0,
                    span_m.1,
                    (2.0 * self.largest_m(c)).max(occupancy_m),
                ))
            })
            .collect()
    }
}

impl Field {
    /// Draws and places the spheres of `layer` over a box of side `box_m` (module documentation).
    ///
    /// # Errors
    ///
    /// [`BuildFieldError::Box`] for a box under eight of the largest radii,
    /// [`BuildFieldError::TooManyParticles`] past [`MAX_PARTICLES`] (or a count that is not a
    /// number), [`BuildFieldError::TooFewParticles`] for less than one particle over the box, and
    /// [`BuildFieldError::Jammed`] if a sphere finds no room.
    pub(crate) fn build(layer: &Layer, box_m: f64, key: FieldKey) -> Result<Self, BuildFieldError> {
        let sizes = layer.sizes();
        let min_box_m = 8.0 * sizes.max_radius_m();
        if !(box_m.is_finite() && box_m > min_box_m) {
            return Err(BuildFieldError::Box {
                box_m,
                min_m: min_box_m,
            });
        }
        let expected = layer.column_number_per_m2() * box_m * box_m;
        #[expect(
            clippy::cast_precision_loss,
            reason = "MAX_PARTICLES is exact in an f64"
        )]
        let most = MAX_PARTICLES as f64;
        if expected.is_nan() || expected > most {
            return Err(BuildFieldError::TooManyParticles(expected));
        }
        if expected < 1.0 {
            return Err(BuildFieldError::TooFewParticles(expected));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "checked to lie in [1, MAX_PARTICLES] above"
        )]
        let count = expected.round() as usize;
        let drawn = draw_spheres(layer, count, key);
        let z_lo_m = drawn
            .iter()
            .map(|&(r, z, _)| z - r)
            .fold(f64::INFINITY, f64::min);
        let z_hi_m = drawn
            .iter()
            .map(|&(r, z, _)| z + r)
            .fold(f64::NEG_INFINITY, f64::max);
        let classes = Classes::of(sizes);
        let mut builders = classes.builders(
            drawn.iter().map(|&(r, _, _)| r),
            box_m,
            (z_lo_m, z_hi_m),
            layer.effective_thickness_m(),
        );
        let mut area_sum_m2 = 0.0;
        let mut volume_sum_m3 = 0.0;
        let mut spheres = Vec::with_capacity(count);
        for (index, (radius_m, height_m, mut draws)) in drawn.into_iter().enumerate().rev() {
            let placed = (0..MAX_PLACEMENT_ATTEMPTS).find_map(|_| {
                let centre = Vec3::new(box_m * draws.uniform(), box_m * draws.uniform(), height_m);
                (!builders.iter().any(|b| b.overlaps(centre, radius_m))).then_some(centre)
            });
            let Some(centre) = placed else {
                return Err(BuildFieldError::Jammed {
                    index,
                    radius_m,
                    height_m,
                });
            };
            builders[classes.class_of(radius_m)].insert(centre, radius_m);
            spheres.push(Sphere { centre, radius_m });
            area_sum_m2 += PI * radius_m * radius_m;
            volume_sum_m3 += (4.0 / 3.0) * PI * radius_m * radius_m * radius_m;
        }
        Ok(Self {
            box_m,
            z_lo_m,
            z_hi_m,
            count,
            area_sum_m2,
            volume_sum_m3,
            effective_thickness_m: layer.effective_thickness_m(),
            spheres,
            grids: builders.into_iter().map(ClassBuilder::freeze).collect(),
        })
    }

    /// The box's side, m.
    #[must_use]
    pub(crate) fn box_m(&self) -> f64 {
        self.box_m
    }

    /// The highest point of any sphere, m.
    #[must_use]
    pub(crate) fn top_m(&self) -> f64 {
        self.z_hi_m
    }

    /// The number of spheres.
    #[must_use]
    pub(crate) fn count(&self) -> usize {
        self.count
    }

    /// The field's own `τ_dyn`, Σ πR² ÷ L².
    #[must_use]
    pub(crate) fn dynamical_optical_depth(&self) -> f64 {
        self.area_sum_m2 / (self.box_m * self.box_m)
    }

    /// The field's own D₀, Σ (4/3)πR³ ÷ (L² `H_eff`): the spheres' volume over the profile's
    /// effective volume, the definition of Design note 15.
    #[must_use]
    pub(crate) fn central_filling_factor(&self) -> f64 {
        self.volume_sum_m3 / (self.box_m * self.box_m * self.effective_thickness_m)
    }

    /// The span [`t_start`, `t_end`] of the unit ray `origin + t direction` within the layer's heights,
    /// t ≥ 0, ended at [`MAX_PATH_M`]; `None` where it never enters, with where it goes instead.
    fn span(&self, origin: Vec3, direction: Vec3) -> Result<(f64, f64, Flight), Flight> {
        let (lo, hi) = (self.z_lo_m, self.z_hi_m);
        let (t_start, t_end, leaves) = if direction.z > 0.0 {
            if origin.z >= hi {
                return Err(Flight::Top);
            }
            (
                ((lo - origin.z) / direction.z).max(0.0),
                (hi - origin.z) / direction.z,
                Flight::Top,
            )
        } else if direction.z < 0.0 {
            if origin.z <= lo {
                return Err(Flight::Bottom);
            }
            (
                ((hi - origin.z) / direction.z).max(0.0),
                (lo - origin.z) / direction.z,
                Flight::Bottom,
            )
        } else if origin.z > lo && origin.z < hi {
            (0.0, f64::INFINITY, Flight::Lost)
        } else if origin.z >= hi {
            return Err(Flight::Top);
        } else {
            return Err(Flight::Bottom);
        };
        if t_end > MAX_PATH_M {
            Ok((t_start, MAX_PATH_M, Flight::Lost))
        } else {
            Ok((t_start, t_end, leaves))
        }
    }

    /// Where the unit ray from `origin` along `direction` goes: the first sphere it enters, or out
    /// of the layer.
    #[must_use]
    pub(crate) fn flight(&self, origin: Vec3, direction: Vec3) -> Flight {
        let (t_start, mut t_end, leaves) = match self.span(origin, direction) {
            Ok(span) => span,
            Err(leaves) => return leaves,
        };
        let mut best = None;
        for grid in &self.grids {
            if let Some((t, sphere)) = grid.cast(origin, direction, t_start, t_end, Stop::Nearest) {
                t_end = t;
                best = Some((t, sphere));
            }
        }
        match best {
            Some((distance_m, sphere)) => Flight::Hit {
                distance_m,
                centre: sphere.centre,
                radius_m: sphere.radius_m,
            },
            None => leaves,
        }
    }

    /// Whether the unit ray from `origin` along `direction` enters any sphere before it leaves the
    /// layer; a lost ray counts as blocked.
    #[must_use]
    pub(crate) fn blocked(&self, origin: Vec3, direction: Vec3) -> bool {
        let Ok((t_start, t_end, leaves)) = self.span(origin, direction) else {
            return false;
        };
        leaves == Flight::Lost
            || self.grids.iter().any(|g| {
                g.cast(origin, direction, t_start, t_end, Stop::Any)
                    .is_some()
            })
    }

    /// Every sphere once, its centre's x and y in the box, in the order they were placed.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn spheres(&self) -> &[Sphere] {
        &self.spheres
    }

    /// Wraps a point's x and y into the box.
    #[must_use]
    pub(crate) fn wrapped(&self, point: Vec3) -> Vec3 {
        Vec3::new(
            point.x.rem_euclid(self.box_m),
            point.y.rem_euclid(self.box_m),
            point.z,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizes() -> PowerLawSizes {
        PowerLawSizes::new(0.5, 5.0).expect("valid radii")
    }

    #[test]
    fn the_moments_of_an_r_minus_three_law_are_the_closed_forms() {
        let s = sizes();
        // ⟨R²⟩ = ln 10 ÷ 1.98, ⟨R³⟩ = 4.5 ÷ 1.98, their ratio 4.5 ÷ ln 10 = 1.954 m.
        assert!((s.mean_square_radius_m2() - core::f64::consts::LN_10 / 1.98).abs() < 1e-12);
        assert!((s.mean_cube_radius_m3() - 4.5 / 1.98).abs() < 1e-12);
        assert!((s.cube_to_square_m() - 1.954_325_168).abs() < 1e-8);
        assert!((s.radius_at(0.0) - 0.5).abs() < 1e-12);
        assert!((s.radius_at(1.0) - 5.0).abs() < 1e-12);
        // The median: R⁻² = 4 − 0.5 × 3.96.
        assert!((s.radius_at(0.5) - 1.0 / (4.0_f64 - 1.98).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn the_law_gives_d0_of_0_35_at_tau_1_5_with_h_eff_of_11_2_m() {
        let layer = SALO_FRENCH_LAW.layer(sizes(), 1.5).expect("valid layer");
        assert!((layer.central_filling_factor() - 0.35).abs() < 1e-12);
        assert!((layer.effective_thickness_m() - 11.17).abs() < 0.01);
        let VerticalProfile::Gaussian { sigma_m } = layer.profile() else {
            panic!("the law's layer is Gaussian");
        };
        assert!((sigma_m - 4.46).abs() < 0.01, "σ {sigma_m}");
        // Proportional to τ at fixed H_eff, and capped at 0.40.
        let half = SALO_FRENCH_LAW.layer(sizes(), 0.75).expect("valid layer");
        assert!((half.central_filling_factor() - 0.175).abs() < 1e-12);
        assert!((half.effective_thickness_m() - layer.effective_thickness_m()).abs() < 1e-9);
        let dense = SALO_FRENCH_LAW.layer(sizes(), 3.0).expect("valid layer");
        assert!((dense.central_filling_factor() - 0.40).abs() < 1e-12);
        // 0.1–5 m: H_eff 7.2 m at the same D₀ (Design note 15).
        let wide = PowerLawSizes::new(0.1, 5.0).expect("valid radii");
        let wide = SALO_FRENCH_LAW.layer(wide, 1.5).expect("valid layer");
        assert!((wide.effective_thickness_m() - 7.16).abs() < 0.01);
    }

    #[test]
    fn layers_out_of_their_domain_are_refused() {
        assert_eq!(
            PowerLawSizes::new(5.0, 0.5),
            Err(DescribeLayerError::Radii {
                min_radius_m: 5.0,
                max_radius_m: 0.5
            })
        );
        assert_eq!(
            Layer::with_filling_factor(sizes(), ProfileShape::Gaussian, 1.5, 1.0),
            Err(DescribeLayerError::FillingFactor(1.0))
        );
        assert_eq!(
            Layer::with_filling_factor(sizes(), ProfileShape::Uniform, 0.0, 0.1),
            Err(DescribeLayerError::OpticalDepth(0.0))
        );
    }

    #[test]
    fn a_ray_enters_a_sphere_at_the_nearer_root_and_never_from_inside() {
        let c = Vec3::new(0.0, 0.0, 0.0);
        let t = entry_distance(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), c, 1.0);
        assert!((t.expect("a hit") - 2.0).abs() < 1e-15);
        assert_eq!(
            entry_distance(Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), c, 1.0),
            None
        );
        assert_eq!(
            entry_distance(Vec3::new(-3.0, 1.5, 0.0), Vec3::new(1.0, 0.0, 0.0), c, 1.0),
            None
        );
        assert_eq!(
            entry_distance(Vec3::new(3.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), c, 1.0),
            None
        );
    }

    /// A small dense field, quick to build in a debug build.
    fn small_field() -> Field {
        let layer = SALO_FRENCH_LAW
            .layer(PowerLawSizes::new(0.5, 2.0).expect("valid radii"), 1.5)
            .expect("valid layer");
        Field::build(&layer, 40.0, FieldKey { seed: 7, key: 1 }).expect("room for every sphere")
    }

    #[test]
    fn a_field_keeps_its_layer_and_no_two_spheres_overlap() {
        let field = small_field();
        assert!((field.dynamical_optical_depth() - 1.5).abs() < 0.01);
        assert!((field.central_filling_factor() - 0.35).abs() < 0.01);
        let l = field.box_m();
        let spheres = field.spheres();
        assert_eq!(spheres.len(), field.count());
        // Every pair apart at its nearest images across the box's sides.
        for (n, a) in spheres.iter().enumerate() {
            for b in &spheres[n + 1..] {
                let mut d = a.centre - b.centre;
                d.x -= l * (d.x / l).round();
                d.y -= l * (d.y / l).round();
                let reach = a.radius_m + b.radius_m;
                assert!(d.dot(d) >= reach * reach, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn a_field_out_of_its_domain_is_refused() {
        let key = FieldKey { seed: 1, key: 1 };
        let layer = SALO_FRENCH_LAW.layer(sizes(), 1.5).expect("valid layer");
        assert_eq!(
            Field::build(&layer, 30.0, key),
            Err(BuildFieldError::Box {
                box_m: 30.0,
                min_m: 40.0
            })
        );
        let thin = SALO_FRENCH_LAW.layer(sizes(), 1e-4).expect("valid layer");
        assert!(matches!(
            Field::build(&thin, 41.0, key),
            Err(BuildFieldError::TooFewParticles(n)) if n < 1.0
        ));
        let fine = PowerLawSizes::new(1e-3, 5.0).expect("valid radii");
        let crowded = SALO_FRENCH_LAW.layer(fine, 1.5).expect("valid layer");
        assert!(matches!(
            Field::build(&crowded, 1e4, key),
            Err(BuildFieldError::TooManyParticles(n)) if n > 5.0e7
        ));
    }

    #[test]
    fn a_field_is_the_same_for_the_same_key_and_differs_for_another() {
        let a = small_field();
        let b = small_field();
        assert_eq!(a, b);
        let layer = SALO_FRENCH_LAW
            .layer(PowerLawSizes::new(0.5, 2.0).expect("valid radii"), 1.5)
            .expect("valid layer");
        let c = Field::build(&layer, 40.0, FieldKey { seed: 7, key: 2 }).expect("room");
        assert_ne!(a, c);
    }

    #[test]
    fn a_cast_finds_the_nearest_sphere_across_the_box_side_and_agrees_with_brute_force() {
        let field = small_field();
        let all = field.spheres();
        let l = field.box_m();
        let mut draws = Draws::new(1, 2, 3);
        for _ in 0..200 {
            let origin = Vec3::new(
                l * draws.uniform(),
                l * draws.uniform(),
                field.top_m() + 1.0,
            );
            let dir = Vec3::new(
                draws.uniform() - 0.5,
                draws.uniform() - 0.5,
                -0.2 - draws.uniform(),
            )
            .normalised();
            // Brute force over the images of as many boxes as the ray crosses.
            let mut best = f64::INFINITY;
            for s in all {
                for ix in -6..=6 {
                    for iy in -6..=6 {
                        let c = s.centre + Vec3::new(f64::from(ix) * l, f64::from(iy) * l, 0.0);
                        if let Some(t) = entry_distance(origin, dir, c, s.radius_m) {
                            best = best.min(t);
                        }
                    }
                }
            }
            match field.flight(origin, dir) {
                Flight::Hit { distance_m, .. } => {
                    assert!(
                        (distance_m - best).abs() < 1e-9,
                        "{distance_m} against {best}"
                    );
                    assert!(field.blocked(origin, dir));
                }
                other => {
                    assert!(
                        best.is_infinite(),
                        "{other:?} where a sphere lies at {best}"
                    );
                    assert!(!field.blocked(origin, dir));
                }
            }
        }
    }
}
