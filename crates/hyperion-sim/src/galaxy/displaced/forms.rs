//! The displaced classes' density forms, in light-years and normalised over the root cube: flared
//! layers, cored power laws, the ballistic and blurred-arm forms of the youngest thin-disc classes,
//! and the own-form mixtures of the barred and nuclear classes (plan 08, P08.T10 and Design notes
//! 13, 21 and 26).
//!
//! Plan 15's form table (`tables::displaced_forms`, P15.T6) gives each class's forms as
//! dimensionless parameters: lengths in thin-disc scale lengths `R_d`, speeds in the circular
//! speed `v_c` ([`GalaxyScales`]). [`FlaredLayerParams`] and [`CoredPowerLawParams`] are one row
//! of it each; the density types here carry them into light-years for one galaxy.
//!
//! - [`FlaredLayer`]: `exp(−R ÷ h_R) × exp(−(|z| ÷ h)^β) ÷ h` with `h = h₀ exp(R ÷ r_flare)`, the
//!   disc-born classes' layer. Its vertical integral is `2Γ(1 + 1 ÷ β)` at every radius.
//! - [`CoredPowerLaw`]: `(1 + (R² + z² ÷ q²) ÷ a²)^(−γ ÷ 2)`, the spheroid of every class.
//! - [`BallisticLayer`]: for the two youngest age bins (τ < 0.3 `R_d ÷ v_c`) the brainstorm's
//!   analytic form replaces the fitted layer: the young disc's own envelope, its height widened to
//!   `√(h_young² + (1.1 ⟨uτ⟩ R_d)²)` with the vertical profile's shape kept (Design note 21).
//! - The arm factor: every thin-disc class with τ below 1 and `⟨uτ⟩` at most 0.4 keeps the young
//!   disc's sharp arm at the width `√(w² + (0.8 ⟨uτ⟩ R_d)²)` on its layer ([`blurred_arm`],
//!   [`keeps_arm`]). The arm factor's azimuthal mean is 1 at any width, so it does not move the
//!   normalisation.
//! - [`OwnFormMixture`]: for bulge, bar and nuclear disc, the own-form share (which stays in the
//!   field component as retained, Design note 13) and the spheroid share, by the bar's corotation
//!   ratio. Only the spheroid is a displaced density; the mixture exists for tests and the map.
//!
//! # Normalisation over the cube
//!
//! Each form component is normalised to unit integral over the root cube, `|x|, |y|, |z| ≤
//! 65,536 ly` (Design note 26), and plan 15's `in_cube` share then scales the class. Every form is
//! axisymmetric, so the cube's integral reduces exactly to one over (R, z): the length of the
//! circle of radius R inside the square is `2πR` out to its half-width L and `4R (π ÷ 2 − 2
//! arccos(L ÷ R))` from L to `√2 L`. Design note 26 names a Cartesian octant rule of 24
//! logarithmic panels of 8 Gauss–Legendre nodes per axis; the reduction keeps those panels in R
//! and z (`L 2⁻²³ … L ÷ 2, L`, with the first panel reaching down to 0) and replaces the third axis
//! by the arc length, which is exact, and costs a two-hundredth. The corner annulus from L to
//! `√2 L` runs in the arc's angle, `R = L ÷ cos θ`, which removes the square root the arc has at
//! `R = L`.

use super::{AGE_EDGES, AgeBin, GalaxyScales};
use crate::coords::ROOT_HALF_WIDTH_LY;
use crate::galaxy::PointLy;
use crate::galaxy::Population;
use crate::galaxy::bounds::{ScalarRange, UnimodalFactor};
use crate::galaxy::fields::arms::SharpArm;
use crate::galaxy::fields::disc::ExponentialDisc;
use crate::galaxy::fields::{BuildFieldError, Fields, Shape};
use crate::math;
use crate::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};
use crate::units::LightYears;

/// The factor on `⟨uτ⟩ R_d` that widens a ballistic layer's height (plan 08, Design note 21).
pub const BALLISTIC_HEIGHT_FACTOR: f64 = 1.1;

/// The factor on `⟨uτ⟩ R_d` that blurs the arm's width (plan 08, Design note 21).
pub const ARM_BLUR_FACTOR: f64 = 0.8;

/// The largest `⟨uτ⟩` at which a thin-disc class keeps the arm factor (plan 08, Design note 21).
pub const ARM_MAX_MEAN_UT: f64 = 0.4;

/// The oldest time since death, in `R_d ÷ v_c`, at which a thin-disc class keeps the arm factor:
/// its age bin must end at or before it (plan 08, Design note 21).
pub const ARM_MAX_TAU: f64 = 1.0;

/// The age bins, from the youngest, whose form is the ballistic layer: τ below 0.3 `R_d ÷ v_c`
/// (plan 08, Design note 21; plan 15's P15.T6 format).
pub const BALLISTIC_AGE_BINS: usize = 2;

/// Logarithmic panels per axis of the cube's normalisation, each of [`GL8_NODES`] (plan 08, Design
/// note 26).
pub const NORMALISATION_PANELS: u32 = 24;

/// Panels of the corner annulus from L to `√2 L`, in the arc's angle.
const CORNER_PANELS: u32 = 4;

/// A form's parameter is not finite or outside its range: `quantity` is `value`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildFormError {
    /// What was wrong, such as `beta` or `core radius`.
    pub quantity: &'static str,
    /// The value given.
    pub value: f64,
}

impl BuildFormError {
    fn check_positive(quantity: &'static str, value: f64) -> Result<(), Self> {
        if value.is_finite() && value > 0.0 {
            Ok(())
        } else {
            Err(Self { quantity, value })
        }
    }

    fn check_fraction(quantity: &'static str, value: f64) -> Result<(), Self> {
        if (0.0..=1.0).contains(&value) {
            Ok(())
        } else {
            Err(Self { quantity, value })
        }
    }
}

impl std::fmt::Display for BuildFormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a displaced form's {} is {}, outside its range",
            self.quantity, self.value
        )
    }
}

impl std::error::Error for BuildFormError {}

impl From<BuildFieldError> for BuildFormError {
    fn from(e: BuildFieldError) -> Self {
        Self {
            quantity: e.quantity(),
            value: e.value(),
        }
    }
}

/// One flared layer of plan 15's table, dimensionless: `weight × exp(−R ÷ h_r) × exp(−(|z| ÷
/// h)^β) ÷ h` with `h = h_0 exp(R ÷ r_flare)`, lengths in `R_d` (P15.T6's `FlaredLayer`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlaredLayerParams {
    /// The layer's share of the class's bound members inside the cube, 0–1.
    pub weight: f64,
    /// The radial scale length, `R_d`.
    pub h_r: f64,
    /// The scale height on the axis, `R_d`.
    pub h_0: f64,
    /// The flare's e-folding radius, `R_d`.
    pub r_flare: f64,
    /// The vertical profile's exponent β.
    pub beta: f64,
}

/// One cored power law of plan 15's table, dimensionless: `weight × (1 + (R² + z² ÷ q²) ÷
/// a²)^(−γ ÷ 2)`, lengths in `R_d` (P15.T6's `CoredPowerLaw`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoredPowerLawParams {
    /// The spheroid's share of the class's bound members inside the cube, 0–1.
    pub weight: f64,
    /// The core radius, `R_d`.
    pub a: f64,
    /// The axis ratio, z over R.
    pub q: f64,
    /// The slope γ of the density's fall, far out.
    pub gamma: f64,
}

/// The cube's half-width L, ly.
fn half_width() -> f64 {
    f64::from(ROOT_HALF_WIDTH_LY)
}

/// The panel edges of one axis from 0 to `end`: 0, then `end × 2^−23` doubling to `end`
/// ([`NORMALISATION_PANELS`] panels).
fn panel_edges(end: f64) -> [f64; 25] {
    let mut edges = [0.0; 25];
    for (i, e) in edges.iter_mut().enumerate().skip(1) {
        let from_end = i32::try_from(24 - i).expect("at most 23");
        *e = end * math::powi(0.5, from_end);
    }
    edges
}

/// `Σ w f(x)` over the Gauss–Legendre nodes of `[a, b]`.
fn gl8(mut f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    let (half, mid) = (0.5 * (b - a), f64::midpoint(a, b));
    GL8_NODES
        .iter()
        .zip(&GL8_WEIGHTS)
        .map(|(&x, &w)| w * f(mid + half * x))
        .sum::<f64>()
        * half
}

/// `∫` of `f` over the panels `edges`.
fn over_panels(mut f: impl FnMut(f64) -> f64, edges: &[f64]) -> f64 {
    edges.windows(2).map(|w| gl8(&mut f, w[0], w[1])).sum()
}

/// The integral over the root cube of an axisymmetric density `shape(R, |z|)` (ly⁻³ times its
/// own units): module documentation, "Normalisation over the cube".
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::forms::cube_integral;
///
/// // A uniform density fills the cube's (2 × 65,536 ly)³.
/// let volume = cube_integral(|_, _| 1.0);
/// assert!((volume / 131_072_f64.powi(3) - 1.0).abs() < 1e-12);
/// ```
#[must_use]
pub fn cube_integral(shape: impl Fn(f64, f64) -> f64) -> f64 {
    let l = half_width();
    let edges = panel_edges(l);
    let column = |r: f64| over_panels(|z| shape(r, z), &edges);
    let inner = over_panels(|r| core::f64::consts::FRAC_PI_2 * r * column(r), &edges);
    let quarter = core::f64::consts::FRAC_PI_4;
    let corner_edges: Vec<f64> = (0..=CORNER_PANELS)
        .map(|i| quarter * f64::from(i) / f64::from(CORNER_PANELS))
        .collect();
    let corner = over_panels(
        |theta| {
            let (sin, cos) = math::sin_cos(theta);
            let r = l / cos;
            let arc = r * (core::f64::consts::FRAC_PI_2 - 2.0 * theta);
            arc * column(r) * l * sin / (cos * cos)
        },
        &corner_edges,
    );
    // Four quadrants of the plane, two sides of it.
    8.0 * (inner + corner)
}

/// The inverse of a form's integral over the cube, checked finite and positive.
fn inverse(integral: f64, quantity: &'static str) -> Result<f64, BuildFormError> {
    BuildFormError::check_positive(quantity, integral)?;
    Ok(1.0 / integral)
}

/// A flared layer in light-years, normalised to unit integral over the root cube (module
/// documentation).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::GalaxyScales;
/// use hyperion_sim::galaxy::displaced::forms::{FlaredLayer, FlaredLayerParams};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
///
/// # fn main() -> Result<(), hyperion_sim::galaxy::displaced::forms::BuildFormError> {
/// let params = GalaxyParams::milky_way_like();
/// let scales = GalaxyScales::new(&params, &PotentialTables::in_plane(&MassModel::new(&params)));
/// let row = FlaredLayerParams { weight: 1.0, h_r: 1.2, h_0: 0.05, r_flare: 4.0, beta: 1.3 };
/// let layer = FlaredLayer::new(&row, &scales)?;
/// // The layer flares: it is taller at the Sun than at the centre.
/// assert!(layer.scale_height(26_000.0) > layer.scale_height(0.0));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlaredLayer {
    /// `1 ÷ h_R`, ly⁻¹.
    inv_h_r: f64,
    /// `h₀`, ly.
    h_0: f64,
    /// `1 ÷ r_flare`, ly⁻¹.
    inv_r_flare: f64,
    beta: f64,
    /// The inverse of the unnormalised form's integral over the cube, ly⁻³.
    norm: f64,
}

impl FlaredLayer {
    /// The layer of the table row `row` in the galaxy of `scales`.
    ///
    /// # Errors
    ///
    /// [`BuildFormError`] if a length or β is not positive and finite, or the layer's integral over
    /// the cube is not.
    pub fn new(row: &FlaredLayerParams, scales: &GalaxyScales) -> Result<Self, BuildFormError> {
        BuildFormError::check_positive("radial scale", row.h_r)?;
        BuildFormError::check_positive("scale height", row.h_0)?;
        BuildFormError::check_positive("flare radius", row.r_flare)?;
        BuildFormError::check_positive("beta", row.beta)?;
        let r_d = scales.r_d().value();
        let mut layer = Self {
            inv_h_r: 1.0 / (row.h_r * r_d),
            h_0: row.h_0 * r_d,
            inv_r_flare: 1.0 / (row.r_flare * r_d),
            beta: row.beta,
            norm: 1.0,
        };
        layer.norm = inverse(cube_integral(|r, z| layer.shape(r, z)), "layer integral")?;
        Ok(layer)
    }

    /// The scale height `h(R) = h₀ exp(R ÷ r_flare)` at cylindrical radius `r` (ly), ly.
    #[must_use]
    pub fn scale_height(&self, r: f64) -> f64 {
        self.h_0 * math::exp(r * self.inv_r_flare)
    }

    /// The radial envelope `exp(−R ÷ h_R)` at `r` (ly).
    #[must_use]
    pub fn radial(&self, r: f64) -> f64 {
        math::exp(-r * self.inv_h_r)
    }

    /// The vertical factor `exp(−(|z| ÷ h)^β) ÷ h` at height `z` for scale height `h` (ly), ly⁻¹.
    #[must_use]
    pub fn vertical(&self, h: f64, z: f64) -> f64 {
        let x = z.abs() / h;
        let power = if x > 0.0 {
            math::powf_positive(x, self.beta)
        } else {
            0.0
        };
        math::exp(-power) / h
    }

    /// The unnormalised form at `(R, z)` (ly), ly⁻¹.
    #[must_use]
    pub fn shape(&self, r: f64, z: f64) -> f64 {
        self.radial(r) * self.vertical(self.scale_height(r), z)
    }

    /// The exponent β.
    #[must_use]
    pub fn beta(&self) -> f64 {
        self.beta
    }

    /// `h_R`, ly.
    #[must_use]
    pub fn radial_scale(&self) -> f64 {
        1.0 / self.inv_h_r
    }

    /// The unnormalised form's column through the whole of z at radius `r` (ly), by quadrature
    /// over the vertical factor from 0 to 256 scale heights: `exp(−R ÷ h_R) × 2Γ(1 + 1 ÷ β)` to
    /// the quadrature's accuracy (module documentation).
    #[must_use]
    pub fn column(&self, r: f64) -> f64 {
        let h = self.scale_height(r);
        let edges: Vec<f64> = core::iter::once(0.0)
            .chain((-24..=8).map(|k| h * math::powi(2.0, k)))
            .collect();
        2.0 * self.radial(r) * over_panels(|z| self.vertical(h, z), &edges)
    }

    /// The density at `p`, ly⁻³: unit integral over the root cube.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        self.norm * self.shape(math::hypot(p.x, p.y), p.z)
    }

    /// The normalisation, the inverse of the unnormalised form's integral over the cube, ly⁻³.
    #[must_use]
    pub fn norm(&self) -> f64 {
        self.norm
    }

    /// The flare factor over the heights `h(R)` of a cell's radii: [`FlareFactor`] at the cell's
    /// least `|z|`, `z_min` (ly).
    #[must_use]
    pub fn flare_factor(&self, z_min: f64) -> FlareFactor {
        FlareFactor {
            z_min,
            beta: self.beta,
            h_0: self.h_0,
            inv_r_flare: self.inv_r_flare,
        }
    }
}

/// A cored power law in light-years, normalised to unit integral over the root cube (module
/// documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoredPowerLaw {
    /// `1 ÷ a²`, ly⁻².
    inv_a_sq: f64,
    /// `1 ÷ q²`.
    inv_q_sq: f64,
    /// `−γ ÷ 2`.
    exponent: f64,
    norm: f64,
}

impl CoredPowerLaw {
    /// The spheroid of the table row `row` in the galaxy of `scales`.
    ///
    /// # Errors
    ///
    /// [`BuildFormError`] if the core radius, the axis ratio or γ is not positive and finite, or
    /// the spheroid's integral over the cube is not.
    pub fn new(row: &CoredPowerLawParams, scales: &GalaxyScales) -> Result<Self, BuildFormError> {
        BuildFormError::check_positive("core radius", row.a)?;
        BuildFormError::check_positive("axis ratio", row.q)?;
        BuildFormError::check_positive("slope", row.gamma)?;
        let a = row.a * scales.r_d().value();
        let mut spheroid = Self {
            inv_a_sq: 1.0 / (a * a),
            inv_q_sq: 1.0 / (row.q * row.q),
            exponent: -0.5 * row.gamma,
            norm: 1.0,
        };
        spheroid.norm = inverse(
            cube_integral(|r, z| spheroid.shape(r, z)),
            "spheroid integral",
        )?;
        Ok(spheroid)
    }

    /// The unnormalised form `(1 + (R² + z² ÷ q²) ÷ a²)^(−γ ÷ 2)` at `(R, z)` (ly): 1 at the
    /// centre.
    #[must_use]
    pub fn shape(&self, r: f64, z: f64) -> f64 {
        let m2 = (r * r + z * z * self.inv_q_sq) * self.inv_a_sq;
        math::powf_positive(1.0 + m2, self.exponent)
    }

    /// The density at `p`, ly⁻³: unit integral over the root cube.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        self.norm * self.shape(math::hypot(p.x, p.y), p.z)
    }

    /// The normalisation, ly⁻³.
    #[must_use]
    pub fn norm(&self) -> f64 {
        self.norm
    }
}

/// Whether a thin-disc class of age bin `age` and weighted mean `⟨uτ⟩` `mean_ut` keeps the arm
/// factor: its age bin ends at or before τ = 1 and `⟨uτ⟩ ≤ 0.4` (plan 08, Design note 21).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::AgeBin;
/// use hyperion_sim::galaxy::displaced::forms::keeps_arm;
///
/// let young = AgeBin::new(1).expect("the second bin");
/// let old = AgeBin::new(3).expect("the fourth bin");
/// assert!(keeps_arm(young, 0.1) && !keeps_arm(young, 0.5) && !keeps_arm(old, 0.1));
/// ```
#[must_use]
pub fn keeps_arm(age: AgeBin, mean_ut: f64) -> bool {
    let end = AGE_EDGES.get(age.index()).copied().unwrap_or(f64::INFINITY);
    end <= ARM_MAX_TAU && mean_ut <= ARM_MAX_MEAN_UT
}

/// The young disc's sharp arm `young` blurred for a class of weighted mean `⟨uτ⟩` `mean_ut`:
/// width `√(w² + (0.8 ⟨uτ⟩ R_d)²)` (plan 08, Design note 21).
///
/// # Errors
///
/// [`BuildFormError`] if the blurred width is not positive and finite (a `mean_ut` not finite).
pub fn blurred_arm(
    young: &SharpArm,
    mean_ut: f64,
    scales: &GalaxyScales,
) -> Result<SharpArm, BuildFormError> {
    let w = young.width().value();
    let blur = ARM_BLUR_FACTOR * mean_ut * scales.r_d().value();
    Ok(young.with_width(LightYears::new(math::hypot(w, blur)))?)
}

/// The young disc of `fields`: its density, and its sharp arm.
///
/// # Panics
///
/// Never for a built galaxy, which always has a young disc with arms.
#[must_use]
pub fn young_disc(fields: &Fields) -> (&ExponentialDisc, SharpArm) {
    let young = fields
        .components()
        .iter()
        .find(|c| c.population() == Population::YoungThinDisc)
        .expect("every galaxy has a young disc");
    match young.shape() {
        Shape::Disc(disc) => match disc.arm() {
            Some(crate::galaxy::fields::arms::Arm::Sharp(arm)) => (disc, *arm),
            Some(crate::galaxy::fields::arms::Arm::Gentle(_)) | None => {
                unreachable!("the young disc has the sharp arm")
            }
        },
        Shape::Bulge(_) | Shape::Bar(_) | Shape::Halo(_) => {
            unreachable!("the young disc is a disc")
        }
    }
}

/// The ballistic form of the two youngest age bins: the young disc's envelope with its height
/// widened, normalised over the cube; with the arm factor, blurred, where [`keeps_arm`] says so
/// (plan 08, Design note 21; module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct BallisticLayer {
    disc: ExponentialDisc,
    /// `h_young ÷ H`, the factor on `|z|` and on the envelope that widens the profile to `H`.
    squeeze: f64,
    arm: Option<SharpArm>,
    norm: f64,
}

impl BallisticLayer {
    /// The ballistic form of a class of age bin `age` and weighted mean `⟨uτ⟩` `mean_ut`, from
    /// the young disc `young` and its arm `young_arm` in the galaxy of `scales`.
    ///
    /// # Errors
    ///
    /// [`BuildFormError`] if `mean_ut` is negative or not finite, or the form's integral over the
    /// cube is not positive and finite.
    pub fn new(
        young: &ExponentialDisc,
        young_arm: &SharpArm,
        age: AgeBin,
        mean_ut: f64,
        scales: &GalaxyScales,
    ) -> Result<Self, BuildFormError> {
        if !(mean_ut.is_finite() && mean_ut >= 0.0) {
            return Err(BuildFormError {
                quantity: "mean speed times age",
                value: mean_ut,
            });
        }
        let h_young = young.height().value();
        let spread = BALLISTIC_HEIGHT_FACTOR * mean_ut * scales.r_d().value();
        let squeeze = h_young / math::hypot(h_young, spread);
        let arm = if keeps_arm(age, mean_ut) {
            Some(blurred_arm(young_arm, mean_ut, scales)?)
        } else {
            None
        };
        let mut layer = Self {
            disc: young.clone(),
            squeeze,
            arm,
            norm: 1.0,
        };
        layer.norm = inverse(
            cube_integral(|r, z| layer.envelope(r, z)),
            "ballistic integral",
        )?;
        Ok(layer)
    }

    /// The layer's height `H = √(h_young² + (1.1 ⟨uτ⟩ R_d)²)`, ly.
    #[must_use]
    pub fn height(&self) -> f64 {
        self.disc.height().value() / self.squeeze
    }

    /// The blurred arm, if the class keeps it.
    #[must_use]
    pub fn arm(&self) -> Option<&SharpArm> {
        self.arm.as_ref()
    }

    /// The unnormalised envelope at `(R, z)` (ly): the young disc's, stretched in z by `H ÷
    /// h_young` at the same column.
    #[must_use]
    pub fn envelope(&self, r: f64, z: f64) -> f64 {
        self.squeeze * self.disc.envelope(r, z * self.squeeze)
    }

    /// The density at `p`, ly⁻³, arm factor included: unit integral over the root cube to the
    /// arm's azimuthal mean.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        let envelope = self.norm * self.envelope(math::hypot(p.x, p.y), p.z);
        match &self.arm {
            Some(arm) => envelope * arm.factor(&arm.geometry().point(p.x, p.y)),
            None => envelope,
        }
    }

    /// The normalisation, ly⁻³.
    #[must_use]
    pub fn norm(&self) -> f64 {
        self.norm
    }

    /// An upper bound on the normalised envelope (no arm) over radii `r_min` to `r_max` at heights
    /// from `z_min` (ly): the young disc's own envelope bound at the stretched height, which never
    /// falls below its envelope there, bit for bit ([`ExponentialDisc`]'s hole and profile).
    #[must_use]
    pub(crate) fn envelope_sup(&self, r_min: f64, r_max: f64, z_min: f64) -> f64 {
        let height = crate::galaxy::fields::vertical::locate(z_min * self.squeeze);
        self.norm * (self.squeeze * self.disc.envelope_sup(r_min, r_max, height))
    }
}

/// One disc-born class's density, normalised over the cube: the ballistic layer in the two
/// youngest age bins; otherwise the flared layer, with its blurred arm where the class keeps it,
/// plus the spheroid, by their weights (plan 08, Design notes 21 and 26).
#[derive(Debug, Clone, PartialEq)]
pub enum DiscBornForm {
    /// τ below 0.3: the ballistic layer, the whole class.
    Ballistic(BallisticLayer),
    /// Older: `w_layer × layer × arm + w_spheroid × spheroid`.
    Fitted {
        /// The layer and its weight; `None` where the fit left none.
        layer: Option<(f64, FlaredLayer)>,
        /// The layer's blurred arm, where [`keeps_arm`] says so.
        arm: Option<SharpArm>,
        /// The spheroid and its weight; `None` where the fit set it to zero.
        spheroid: Option<(f64, CoredPowerLaw)>,
    },
}

impl DiscBornForm {
    /// The form of a disc-born class of age bin `age` and weighted mean `⟨uτ⟩` `mean_ut` from its
    /// table row (`layer`, `spheroid`), in the galaxy of `scales` whose young disc is `young`.
    ///
    /// A component of zero weight is left out; the weights are those of the row, which sum to 1.
    ///
    /// # Errors
    ///
    /// [`BuildFormError`] for a weight outside 0–1, and as [`FlaredLayer::new`],
    /// [`CoredPowerLaw::new`] and [`BallisticLayer::new`].
    pub fn new(
        layer: &FlaredLayerParams,
        spheroid: &CoredPowerLawParams,
        age: AgeBin,
        mean_ut: f64,
        young: (&ExponentialDisc, &SharpArm),
        scales: &GalaxyScales,
    ) -> Result<Self, BuildFormError> {
        BuildFormError::check_fraction("layer weight", layer.weight)?;
        BuildFormError::check_fraction("spheroid weight", spheroid.weight)?;
        if age.index() < BALLISTIC_AGE_BINS {
            return Ok(Self::Ballistic(BallisticLayer::new(
                young.0, young.1, age, mean_ut, scales,
            )?));
        }
        let arm = if keeps_arm(age, mean_ut) {
            Some(blurred_arm(young.1, mean_ut, scales)?)
        } else {
            None
        };
        Ok(Self::Fitted {
            layer: if layer.weight > 0.0 {
                Some((layer.weight, FlaredLayer::new(layer, scales)?))
            } else {
                None
            },
            arm,
            spheroid: if spheroid.weight > 0.0 {
                Some((spheroid.weight, CoredPowerLaw::new(spheroid, scales)?))
            } else {
                None
            },
        })
    }

    /// The density at `p`, ly⁻³.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        match self {
            Self::Ballistic(layer) => layer.density(p),
            Self::Fitted {
                layer,
                arm,
                spheroid,
            } => {
                let flat = layer.as_ref().map_or(0.0, |(w, l)| {
                    let arm_factor = arm
                        .as_ref()
                        .map_or(1.0, |a| a.factor(&a.geometry().point(p.x, p.y)));
                    w * l.density(p) * arm_factor
                });
                flat + spheroid.as_ref().map_or(0.0, |(w, s)| w * s.density(p))
            }
        }
    }

    /// The blurred arm, if the class keeps one.
    #[must_use]
    pub fn arm(&self) -> Option<&SharpArm> {
        match self {
            Self::Ballistic(layer) => layer.arm(),
            Self::Fitted { arm, .. } => arm.as_ref(),
        }
    }
}

/// The corotation ratios, bar corotation over half-length, at which plan 15 fits the own-form
/// shares (P15.T6's `COROTATION_RATIO_NODES`).
pub const COROTATION_RATIO_NODES: [f64; 3] = [1.0, 1.2, 1.4];

/// A barred or nuclear class's mixture of its own form, the field component's shape, and a
/// spheroid, by the bar's corotation ratio (plan 08, Design note 13; brainstorm, "Displaced
/// objects: kicks and runaways").
///
/// The own-form share stays in the field component as retained; only the spheroid share is a
/// displaced class. The mixture is here for the tests and the map.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::forms::OwnFormMixture;
///
/// // The brainstorm's bar in its second speed bin at a ratio of 1.2: 61% keeps the bar's shape.
/// let mixture = OwnFormMixture::new([0.66, 0.61, 0.55])?;
/// assert!((mixture.own_share(1.2) - 0.61).abs() < 1e-15);
/// assert!((mixture.own_share(1.2) + mixture.spheroid_share(1.2) - 1.0).abs() < 1e-15);
/// assert!(mixture.own_share(1.3) < 0.61 && mixture.own_share(1.3) > 0.55);
/// # Ok::<(), hyperion_sim::galaxy::displaced::forms::BuildFormError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnFormMixture {
    own_share: [f64; 3],
}

impl OwnFormMixture {
    /// The mixture whose own-form shares at [`COROTATION_RATIO_NODES`] are `own_share`.
    ///
    /// # Errors
    ///
    /// [`BuildFormError`] if a share lies outside 0–1.
    pub fn new(own_share: [f64; 3]) -> Result<Self, BuildFormError> {
        for s in own_share {
            BuildFormError::check_fraction("own-form share", s)?;
        }
        Ok(Self { own_share })
    }

    /// The own-form share at the corotation ratio `ratio`: linear between the nodes, clamped at
    /// the ends (P15.T6's format).
    #[must_use]
    pub fn own_share(&self, ratio: f64) -> f64 {
        let nodes = COROTATION_RATIO_NODES;
        if ratio <= nodes[0] {
            return self.own_share[0];
        }
        for i in 0..nodes.len() - 1 {
            if ratio <= nodes[i + 1] {
                let t = (ratio - nodes[i]) / (nodes[i + 1] - nodes[i]);
                return self.own_share[i] + t * (self.own_share[i + 1] - self.own_share[i]);
            }
        }
        self.own_share[nodes.len() - 1]
    }

    /// The spheroid's share at `ratio`: 1 less the own-form share.
    #[must_use]
    pub fn spheroid_share(&self, ratio: f64) -> f64 {
        1.0 - self.own_share(ratio)
    }

    /// The mixture's density at `ratio` given the own form's normalised density `own` and the
    /// spheroid's `spheroid` at the same point, ly⁻³.
    #[must_use]
    pub fn density(&self, ratio: f64, own: f64, spheroid: f64) -> f64 {
        let s = self.own_share(ratio);
        s * own + (1.0 - s) * spheroid
    }
}

/// The flare factor `g(h) = exp(−(z₁ ÷ h)^β) ÷ h` over a range of scale heights, for a cell whose
/// least `|z|` is `z₁` (plan 08, P08.T11): unimodal in `h`, with its peak at `h★ = z₁ β^(1 ÷ β)`.
///
/// Its [`UnimodalFactor::sup`] takes the range of the cell's radii (nearest and farthest corners,
/// since no cell straddles an axis plane) and bounds g over the heights `h(R)` there: `g(h★)` if
/// `h★` lies in `[h(R₁), h(R₂)]`, the value at the nearer end otherwise; for `z₁ = 0`, `1 ÷
/// h(R₁)`. Every value is raised by [`BOUND_MARGIN`](crate::galaxy::bounds::BOUND_MARGIN), so that
/// rounding in the density's own evaluation cannot pass it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlareFactor {
    z_min: f64,
    beta: f64,
    h_0: f64,
    inv_r_flare: f64,
}

impl FlareFactor {
    /// `g(h)`, as the layer computes its vertical factor.
    fn g(&self, h: f64) -> f64 {
        let x = self.z_min / h;
        let power = if x > 0.0 {
            math::powf_positive(x, self.beta)
        } else {
            0.0
        };
        math::exp(-power) / h
    }
}

impl UnimodalFactor for FlareFactor {
    /// The supremum over the cell's radii `range` (ly), module documentation above.
    fn sup(&self, range: ScalarRange) -> f64 {
        let slack = 1.0 + 4.0 * crate::galaxy::bounds::BOUND_MARGIN;
        // h(R) rises with R; widen the ends by a rounding's worth so the bound holds bit for bit.
        let h_lo = self.h_0 * math::exp(range.lo * self.inv_r_flare) / slack;
        let h_hi = self.h_0 * math::exp(range.hi * self.inv_r_flare) * slack;
        if self.z_min <= 0.0 {
            return slack / h_lo;
        }
        let peak = self.z_min * math::powf_positive(self.beta, 1.0 / self.beta);
        let value = if peak < h_lo {
            self.g(h_lo)
        } else if peak > h_hi {
            self.g(h_hi)
        } else {
            self.g(peak)
        };
        value * slack
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panels_double_up_to_the_end() {
        let edges = panel_edges(65_536.0);
        assert!(edges[0].abs() < f64::EPSILON);
        assert!((edges[1] - 65_536.0 / 8_388_608.0).abs() < 1e-18);
        assert!((edges[24] - 65_536.0).abs() < f64::EPSILON);
        assert!(
            edges
                .windows(2)
                .skip(1)
                .all(|w| (w[1] / w[0] - 2.0).abs() < 1e-15)
        );
    }

    /// A Gaussian blob well inside the cube integrates to its closed form, and a uniform density
    /// to the cube's volume: the corner annulus is right.
    #[test]
    fn the_cube_integral_is_exact_for_known_forms() {
        let s = 3_000.0;
        let gauss = cube_integral(|r, z| math::exp(-0.5 * (r * r + z * z) / (s * s)));
        let expected = math::powf(2.0 * core::f64::consts::PI, 1.5) * s * s * s;
        assert!(
            (gauss / expected - 1.0).abs() < 1e-9,
            "{gauss} against {expected}"
        );
        let l = half_width();
        let volume = cube_integral(|_, _| 1.0);
        assert!((volume / (8.0 * l * l * l) - 1.0).abs() < 1e-12);
        // A radial ramp, which weights the corners.
        let ramp = cube_integral(|r, _| r * r);
        let exact = 2.0 * l * (2.0 * l) * (2.0 * l) * (2.0 * l * l / 3.0);
        assert!((ramp / exact - 1.0).abs() < 1e-12, "{ramp} against {exact}");
    }
}
