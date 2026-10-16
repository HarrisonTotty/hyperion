//! A stagnant lid's volcanic provinces and their flexure (R09.T12.c; Design notes 3 and 7;
//! `decision-r09-t12-coarse.md` items 3 and 4).
//!
//! # The provinces (this step's law, not put to main)
//!
//! Plan 14 states the constructional share of `σ_h` as `σ_volc` = 2.76 km × (3.71 ÷ g) × V ×
//! min(1, `T_e` ÷ 70 km), and bounds V where it would need a surface three-quarters covered by
//! conical edifices at the edifice cap, `h_max` = 21.9 km × (3.71 ÷ g) × min(1, `T_e` ÷ 70 km)
//! (Olympus Mons's relief, Plescia 2004, JGR 109, E03003; the lithosphere factor acts on the share
//! and the cap alone, Design note 3), whose RMS is `h_max` ÷ 4 (P14.T48.a's `VOLCANISM_SATURATION`).
//! The provinces are that cone field, below its saturation: edifices at the cap, each a cone of
//! height `h_max` and basal radius `h_max` ÷ [`EDIFICE_ASPECT`] (Olympus Mons's, no narrower than
//! the boundary diameter the cells resolve), covering the share f of the surface whose cone field
//! has `σ_volc`'s RMS before flexure, f ÷ 6 − f² ÷ 9 = (`σ_volc` ÷ `h_max`)², a quadratic that gives
//! f = ¾ (1 − √(1 − 16 (`σ_volc` ÷ `h_max`)²)): 0.10 at V = 1, ¾ at plan 14's V bound. Since `σ_volc` ÷
//! `h_max` is 2.76 V ÷ 21.9, the share is V's alone. The edifices gather into provinces: a Poisson
//! count of mean [`PROVINCES_AT_UNIT_VOLCANISM`] × V (Mars's four, Tharsis, Elysium, the
//! circum-Hellas province and Syrtis Major; Werner 2009, Icarus 201, 44, and Plescia 2004, as
//! R09.T12's science check read them), each a centre uniform on the sphere, and the edifices dealt
//! to them in turn, each set off from its province's centre by a two-dimensional normal of angular
//! s.d. `r_e` √(n ÷ P) ÷ R, so that a province holds its edifices about twice over. Overlapping
//! loads sum, held at `h_max`. The flexure then sinks the load and raises its bulge, and the
//! realised share falls short of `σ_volc` where the lithosphere bends; one correction then widens
//! the coverage by the ratio of the variances (at most to ¾) and lays the field again. A
//! labelled illustration of low confidence: one edifice size, Mars's style on every lid.
//!
//! The lid's mechanics family sets the rest (`decision-r09-t12-coarse.md` item 3): a `Silicate`
//! lid sits on a mantle of [`MANTLE_DENSITY`] and takes the silicate `T_e`; a `WaterIce` lid sits
//! on its own ductile ice, the lid entry's grain density, and takes the ice `T_e`; a lid of any
//! other family (`Metal`, `VolatileIce`, `Salt`, `Organic`) has no rule and no provinces
//! (labelled, a closed set for the composition audit). A mobile lid has no provinces, a world of no
//! volcanism none, and nor has a lid whose palette names no `Province` entry, the provinces'
//! material.
//!
//! # The flexure
//!
//! DN7's line load on a thin elastic plate over a fluid (Turcotte and Schubert 2014, §3.16–3.17,
//! not re-read; the solution checked symbolically by R09.T12's science check): a line load of
//! weight `V₀` per unit length deflects it by w = `w₀` e^(−x/α) (cos x/α + sin x/α), `w₀` = `V₀` ÷
//! (2 `k_eff` α), with α = [4D ÷ `k_eff`]^¼, D = E `T_e`³ ÷ 12(1 − ν²) and `k_eff` = (`ρ_m` −
//! `ρ_fill`) g + E `T_e` ÷ R² ([`flexural_parameter`]). The last term is a spherical shell's
//! membrane support, the shallow shell's foundation stiffness (Brotchie and Silvester 1969, JGR 74,
//! 5240, not read; "membrane stresses (which depend on ETe/R0² and are thus large for small
//! bodies)", Nimmo, Bills and Thomas 2011, JGR 116, E11001, ¶49), valid for α ≪ R; low-degree
//! membrane support is not modelled.
//!
//! **Applied to an edifice** (labelled): its deflection at a distance r from its centre is the line
//! load's convolved with its load along a diameter, as if the edifice were an infinite ridge of its
//! own cross-section (the axisymmetric load's exact solution, in Kelvin functions, is not used).
//! The cross-section is cut into [`PROFILE_STRIPS`] strips of uniform load, each convolved exactly
//! through the line load's step response ([`step_response`]), so the result holds for any α,
//! the local Airy balance at α = 0 included. A load broad against α sinks by `ρ_l` g h ÷ `k_eff`
//! (`ρ_l` h ÷ (`ρ_m` − `ρ_fill`) where the shell term is negligible), its whole compensation, and a
//! narrow one is held up. R09.T12's science check found the ruled model counting the fill twice
//! under a load (the uniform foundation assumes fill beneath it, and Δz takes w off it again);
//! built as ruled, it is recorded in the plan's deferred findings for main's ruling.
//!
//! **The moat is filled to the surrounding level** (ruled): w is the deflection, positive
//! downward, under the total load, h the load's thickness above the pre-load surface, and the
//! elevation changes by Δz = max(h − w, 0). The moat is a level plain of province material, the
//! load stands h − w above it, and the bulge rises by −w. The fill is the `Province` entry's
//! grain density × (1 − [`FILL_POROSITY`]); the load is its grain density, near Belleguic,
//! Lognonné and Wieczorek 2005's 3,200 ± 100 kg m⁻³ for Mars's large volcanoes (JGR 110,
//! E11005, the abstract). A cell takes the `Province` crust where the load or its moat's fill is
//! at least [`PROVINCE_SURFACE_MIN`] thick.
//!
//! # The draws
//!
//! `surface.coarse.relief`, keyed `surface_cell(0, 0, 0, 0, instance)` (level 0's first face
//! cell, which stands for the whole body): instance [`PROVINCE_INSTANCE`]'s word 0 the province
//! count (a Poisson draw), and from word (p + 1) × 2³² province p's centre (two words); instance
//! [`EDIFICE_INSTANCE`]'s words from e × 2⁸ edifice e's offset (one standard normal pair, two
//! words).

use hyperion_surface::cube::PatchKey;
use hyperion_surface::field::{
    Crust, MechanicsFamily, PaletteEntry, boundary_diameter, cell_index,
};
use hyperion_surface::num;
use hyperion_surface::tags::SURFACE_COARSE_RELIEF;

use super::super::plates::slot;
use super::super::{Pass, Working};
use super::noise;
use super::seafloor::MANTLE_DENSITY;
use crate::math;
use crate::planetary::hooks::figures::Crust as LithosphereCrust;
use crate::planetary::hooks::figures::relief::{
    CONSTRUCTIONAL_REFERENCE_GRAVITY, CONSTRUCTIONAL_RELIEF, ICE_LITHOSPHERE_ISOTHERM,
    SUPPORTING_LITHOSPHERE, TALLEST_EDIFICE_RELIEF, lithosphere_thickness,
};
use crate::planetary::surface::inputs::CoarseInputs;
use crate::rng::ObjectKey;
use crate::substance::thermal::{ThermalColumn, ThermalForm};
use crate::tables::gauss_legendre::{GL64_NODES, GL64_WEIGHTS};
use crate::units::{
    Kelvin, KilogramsPerCubicMetre, Metres, MetresPerSecondSquared, WattsPerSquareMetre,
};

/// The lithosphere's Young's modulus E, 100 GPa, as pascals (Belleguic, Lognonné and Wieczorek
/// 2005, Table 2, as R09.T12's science check read it; Design note 7).
pub(crate) const YOUNGS_MODULUS_PA: f64 = 100e9;

/// The lithosphere's Poisson's ratio ν, 0.25 (Belleguic et al. 2005; Design note 7).
pub(crate) const POISSON_RATIO: f64 = 0.25;

/// The porosity of a province's fill, 0.2 (ruled, `decision-r09-t12-coarse.md` item 3): Mars's
/// crust has a bulk density of 2,582 ± 209 kg m⁻³ (Goossens et al. 2017, GRL 44, 7686, the abstract)
/// against the shergottites' grain density of 3,310 kg m⁻³ (Macke et al. 2011, MAPS 46, 311, the
/// abstract), a porosity of 0.22 ± 0.06; GRAIL's lunar crust's 12% lies below (Wieczorek et al.
/// 2013, Science 339, 671, the abstract: "an average crustal porosity of 12% to depths of at least
/// a few kilometers", as R09.T12's science check read it). Medium-low confidence.
pub(crate) const FILL_POROSITY: f64 = 0.2;

/// An edifice's height over its basal radius, 21.9 ÷ 370: Olympus Mons's relief of 21.9 km over
/// the mean half of its 840 × 640 km base (Plescia 2004, JGR 109, E03003, Table 1 and ¶32, as
/// R09.T12's science check read it). An illustration of low confidence: one shield's shape for
/// every edifice.
pub(crate) const EDIFICE_ASPECT: f64 = 21.9 / 370.0;

/// The mean count of provinces at a volcanism level of 1, 4: Mars's, Tharsis, Elysium and the
/// circum-Hellas province (Werner 2009, Icarus 201, 44, the abstract) and Syrtis Major (Plescia
/// 2004), V being plan 14's level that Mars calibrates at 1. An illustration.
pub(crate) const PROVINCES_AT_UNIT_VOLCANISM: f64 = 4.0;

/// The provinces' count and centres' instance on `surface.coarse.relief`, 2.
pub(super) const PROVINCE_INSTANCE: u16 = 2;

/// The edifices' offsets' instance on `surface.coarse.relief`, 3.
pub(super) const EDIFICE_INSTANCE: u16 = 3;

/// The most edifices a field holds, 4,096: a bound on the step's work, which no body below plan
/// 14's V bound with edifices of a coarse cell's size reaches.
const MAX_EDIFICES: usize = 4_096;

/// The strips an edifice's cross-section is cut into for its deflection, 64.
const PROFILE_STRIPS: usize = 64;

/// The nodes of an edifice's deflection table, from its centre out to its reach, 512.
const TABLE_NODES: usize = 512;

/// The flexural lengths beyond an edifice's base its deflection is followed, 2π: there a line
/// load's deflection is under e^(−2π) ≈ 0.2% of its peak.
const REACH_LENGTHS: f64 = core::f64::consts::TAU;

/// The thickness of a province's load or moat fill from which a cell's crust is the province's,
/// 10 m: thinner, the flexure's far lobes would make a province of any lid (an illustration).
pub(crate) const PROVINCE_SURFACE_MIN: Metres = Metres::new(10.0);

/// The largest coverage of the cone field, ¾: plan 14's saturation (P14.T48.a).
const SATURATED_COVERAGE: f64 = 0.75;

/// The elastic thickness `T_e` of the lid of `inputs` (`decision-r09-t12-coarse.md` item 4), by its
/// palette entry's mechanics family, or `None` for a family with no rule (see the module
/// documentation): every `T_e` is the law's held to the body's radius, since both laws are planar
/// conductive geotherms with no internal heat, and a body whose law passes its radius never
/// reaches the isotherm, and is rigid throughout on the loads' time scale. There the thin-shell
/// flexure is an extrapolation (it wants `T_e` ≪ α ≪ R, and α nears R; labelled), which only
/// makes the loads stand: no reference world with provinces reaches it.
///
/// - `Silicate`: plan 14's law, k (870 K − `T_s`) ÷ F with k = 3.1 W m⁻¹ K⁻¹ ([`lithosphere_thickness`];
///   T0.b's rulings, Design note 3), zero where `T_s` ≥ 870 K. 107.7 km at Mars's 19 mW m⁻² and
///   210 K, and 107.0 km at the reference Mars's 214 K.
/// - `WaterIce`: ∫ k dT ÷ F from `T_s` to plan 14's 170 K (`ICE_LITHOSPHERE_ISOTHERM`), k the lid
///   substance's own conductivity row ([`conductivity_integral`]): (567 W m⁻¹ ÷ F) ln(170 K ÷ `T_s`)
///   for water ice Ih's 567 ÷ T (Klinger 1980, via the `H2O` row), zero where `T_s` ≥ 170 K. The
///   isotherm is rheological, where ice creeps on the loads' time scale, so a conductivity moves its
///   depth, not its temperature (ruled): 2.52 km at 200 mW m⁻² and 1.86 km at 270, inside Giese et
///   al. 2008's 2.5 km mechanical thickness at 200–270 mW m⁻² on Enceladus (GRL 35, L24204, the
///   abstract). Low confidence, labelled.
#[must_use]
pub(crate) fn elastic_thickness(inputs: &CoarseInputs) -> Option<Metres> {
    let entry = inputs.palette().get(inputs.lid_entry())?;
    let t_s = inputs.mean_surface_temperature();
    let flux = inputs.heat_flow();
    let law = match entry.mechanics {
        MechanicsFamily::Silicate => lithosphere_thickness(LithosphereCrust::Silicate, t_s, flux),
        MechanicsFamily::WaterIce => ice_thickness(entry, t_s, flux)?,
        MechanicsFamily::Metal
        | MechanicsFamily::VolatileIce
        | MechanicsFamily::Salt
        | MechanicsFamily::Organic => return None,
    };
    Some(Metres::new(num::min(law.value(), inputs.radius().value())))
}

/// An ice lid's `T_e` under `flux` at `t_s` from its entry's substance's conductivity row, or
/// `None` where the substance has no row (see [`elastic_thickness`]).
#[must_use]
pub(super) fn ice_thickness(
    entry: &PaletteEntry,
    t_s: Kelvin,
    flux: WattsPerSquareMetre,
) -> Option<Metres> {
    let columns = crate::substance::by_key(entry.substance.as_str())?
        .substance()
        .thermal_columns()?;
    let isotherm = ICE_LITHOSPHERE_ISOTHERM.value();
    if t_s.value().is_nan() || t_s.value() >= isotherm {
        return Some(Metres::ZERO);
    }
    let integral = conductivity_integral(columns.conductivity(), t_s.value(), isotherm);
    Some(Metres::new(if flux.value() > 0.0 {
        integral / flux.value()
    } else {
        f64::INFINITY
    }))
}

/// ∫ k dT over [`low`, `high`] kelvin, W m⁻¹, of the conductivity column `column`, piece by piece:
/// each piece's form integrated in closed form where it has one (`Reciprocal`, a ln(T₂ ÷ T₁) +
/// b (T₂ − T₁), and `Constant`), the first piece's form extended below its range and the last's
/// above it (labelled: water ice Ih's 567 ÷ T below its 25 K floor); any other form by 64-point
/// Gauss–Legendre over the column's held values ([`ThermalColumn::at`]).
#[must_use]
pub(crate) fn conductivity_integral(column: &ThermalColumn, low: f64, high: f64) -> f64 {
    let pieces = column.pieces();
    let last = pieces.len().saturating_sub(1);
    let mut sum = 0.0;
    for (index, piece) in pieces.iter().enumerate() {
        let from = if index == 0 {
            low
        } else {
            num::max(low, piece.low().value())
        };
        let to = if index == last {
            high
        } else {
            num::min(high, piece.high().value())
        };
        if to <= from {
            continue;
        }
        sum += match piece.form() {
            ThermalForm::Reciprocal { a, b } => a * math::ln(to / from) + b * (to - from),
            ThermalForm::Constant(k) => k * (to - from),
            ThermalForm::Polynomial(_)
            | ThermalForm::HalfPowers(_)
            | ThermalForm::LogPolynomial(_)
            | ThermalForm::Points(_)
            | ThermalForm::IceIh => {
                let (half, mid) = (0.5 * (to - from), f64::midpoint(to, from));
                let mut quadrature = 0.0;
                for (node, weight) in GL64_NODES.iter().zip(&GL64_WEIGHTS) {
                    quadrature += weight * column.at(Kelvin::new(mid + half * node));
                }
                half * quadrature
            }
        };
    }
    sum
}

/// The density beneath a lid entry `entry`'s elastic lithosphere, by its mechanics family
/// (ruled, `decision-r09-t12-coarse.md` item 3): [`MANTLE_DENSITY`] under silicate; the entry's
/// own grain density under water ice, its own ductile ice beneath; `None` for any other family.
#[must_use]
pub(crate) fn mantle_density(entry: &PaletteEntry) -> Option<KilogramsPerCubicMetre> {
    match entry.mechanics {
        MechanicsFamily::Silicate => Some(MANTLE_DENSITY),
        MechanicsFamily::WaterIce => Some(KilogramsPerCubicMetre::new(entry.grain_density_kg_m3)),
        MechanicsFamily::Metal
        | MechanicsFamily::VolatileIce
        | MechanicsFamily::Salt
        | MechanicsFamily::Organic => None,
    }
}

/// The flexural stiffness's foundation `k_eff` = (`ρ_m` − `ρ_fill`) g + E `T_e` ÷ R², N m⁻³, of an
/// elastic lid `elastic` thick over a mantle of density `mantle`, its moat filled at density
/// `fill`, under `gravity` on a body of radius `radius` (the module documentation).
#[must_use]
pub(crate) fn foundation_n_per_m3(
    elastic: Metres,
    mantle: KilogramsPerCubicMetre,
    fill: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    radius: Metres,
) -> f64 {
    let r = radius.value();
    (mantle.value() - fill.value()) * gravity.value()
        + YOUNGS_MODULUS_PA * elastic.value() / (r * r)
}

/// The flexural parameter α = [4D ÷ `k_eff`]^¼ of an elastic lid `elastic` thick (D = E `T_e`³ ÷
/// 12(1 − ν²)) over a mantle of density `mantle` with fill of density `fill` under `gravity` on a
/// body of radius `radius` ([`foundation_n_per_m3`]); zero where the lid has no thickness, whose
/// loads are compensated locally.
///
/// # Panics
///
/// If `mantle` is not denser than `fill`, which [`run`] never passes it.
#[must_use]
pub(crate) fn flexural_parameter(
    elastic: Metres,
    mantle: KilogramsPerCubicMetre,
    fill: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared,
    radius: Metres,
) -> Metres {
    let (m, f) = (mantle.value(), fill.value());
    assert!(
        m - f > 0.0,
        "a lid's mantle, {m} kg m⁻³, must be denser than its fill, {f} kg m⁻³"
    );
    let t = elastic.value();
    if t <= 0.0 {
        return Metres::ZERO;
    }
    let rigidity = YOUNGS_MODULUS_PA * t * t * t / (12.0 * (1.0 - POISSON_RATIO * POISSON_RATIO));
    let k = foundation_n_per_m3(elastic, mantle, fill, gravity, radius);
    Metres::new((4.0 * rigidity / k).sqrt().sqrt())
}

/// A line load's deflection at `x` metres from it, metres, positive downward: w = `w₀` e^(−|x|/α)
/// (cos |x|/α + sin |x|/α), `w₀` = `line_weight` ÷ (2 `k_eff` α), for a load of weight
/// `line_weight` newtons a metre on a lid of flexural parameter `alpha` and foundation
/// `foundation` (N m⁻³). Its bulge crest lies at πα, −e^(−π) `w₀`.
#[cfg(test)]
#[must_use]
pub(crate) fn line_load_deflection(
    x: f64,
    alpha: Metres,
    foundation: f64,
    line_weight: f64,
) -> f64 {
    let a = alpha.value();
    let t = x.abs() / a;
    line_weight / (2.0 * foundation * a) * math::exp(-t) * (math::cos(t) + math::sin(t))
}

/// The line load's step response at `u` metres past the edge of a uniform load of unit pressure
/// on a lid of flexural parameter `alpha`, times `k_eff`: its deflection's share of the load's
/// whole compensation, 1 − ½ e^(−t) cos t under the load (u ≥ 0, t = u ÷ α) and ½ e^(−t) cos t
/// beyond it, so that a strip [a, b] of load q deflects the lid at x by q ÷ `k_eff` (S(x − a) −
/// S(x − b)); at α = 0 a step, the local Airy balance.
#[must_use]
pub(crate) fn step_response(u: f64, alpha: Metres) -> f64 {
    let a = alpha.value();
    if a <= 0.0 {
        return if u >= 0.0 { 1.0 } else { 0.0 };
    }
    let t = u.abs() / a;
    let half = 0.5 * math::exp(-t) * math::cos(t);
    if u >= 0.0 { 1.0 - half } else { half }
}

/// An edifice's cone: its height and its basal radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Cone {
    /// Its height.
    pub(super) height: Metres,
    /// Its basal radius.
    pub(super) radius: Metres,
}

/// The lid an edifice bends: its flexural parameter, its foundation and its gravity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Lid {
    /// Its flexural parameter α.
    pub(super) alpha: Metres,
    /// Its foundation `k_eff`, N m⁻³ ([`foundation_n_per_m3`]).
    pub(super) foundation_n_per_m3: f64,
    /// The body's surface gravity.
    pub(super) gravity: MetresPerSecondSquared,
}

/// An edifice: a cone with its deflection of its lid tabulated out to its reach (see the module
/// documentation).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Edifice {
    /// Its height, metres.
    pub(super) height_m: f64,
    /// Its basal radius, metres.
    pub(super) radius_m: f64,
    /// The distance out to which its deflection is followed, metres.
    pub(super) reach_m: f64,
    /// Its deflection at `TABLE_NODES` distances evenly from 0 to its reach, metres, positive down.
    deflection: Vec<f64>,
}

impl Edifice {
    /// The edifice of `cone`, whose load of density `load` bends `lid`, its reach held to `limit`.
    #[must_use]
    fn new(cone: Cone, load: KilogramsPerCubicMetre, lid: Lid, limit: Metres) -> Self {
        let (height, radius) = (cone.height.value(), cone.radius.value());
        let alpha = lid.alpha;
        let reach = num::min(radius + REACH_LENGTHS * alpha.value(), limit.value());
        let strip = 2.0 * radius / count_f64(PROFILE_STRIPS);
        let pressure_over_k = load.value() * lid.gravity.value() / lid.foundation_n_per_m3;
        let strips: Vec<(f64, f64, f64)> = (0..PROFILE_STRIPS)
            .map(|j| {
                let a = -radius + strip * count_f64(j);
                let b = a + strip;
                let mid = f64::midpoint(a, b);
                (a, b, height * (1.0 - mid.abs() / radius))
            })
            .collect();
        let deflection = (0..TABLE_NODES)
            .map(|i| {
                let x = reach * count_f64(i) / count_f64(TABLE_NODES - 1);
                let mut w = 0.0;
                for &(a, b, h) in &strips {
                    w += h * (step_response(x - a, alpha) - step_response(x - b, alpha));
                }
                num::assert_finite(pressure_over_k * w)
            })
            .collect();
        Self {
            height_m: height,
            radius_m: radius,
            reach_m: reach,
            deflection,
        }
    }

    /// Its load's thickness at `r` metres from its centre, metres: the cone.
    #[must_use]
    pub(super) fn load_m(&self, r: f64) -> f64 {
        if r < self.radius_m {
            self.height_m * (1.0 - r / self.radius_m)
        } else {
            0.0
        }
    }

    /// Its deflection at `r` metres from its centre, metres, positive down: the table's, linear
    /// between its nodes, and zero beyond its reach.
    #[must_use]
    pub(super) fn deflection_m(&self, r: f64) -> f64 {
        if r >= self.reach_m || self.reach_m <= 0.0 {
            return 0.0;
        }
        let position = r / self.reach_m * count_f64(TABLE_NODES - 1);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a node index within the table, from a finite non-negative position below its length"
        )]
        let i = position.floor() as usize;
        let i = i.min(TABLE_NODES - 2);
        let t = position - count_f64(i);
        self.deflection[i] + t * (self.deflection[i + 1] - self.deflection[i])
    }
}

/// `n` as a float.
#[must_use]
fn count_f64(n: usize) -> f64 {
    f64::from(u32::try_from(n).expect("a strip or node count fits a u32"))
}

/// The provinces of a pass as laid out: what the tests and the record read.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Provinces {
    /// The lid's elastic thickness, metres.
    pub(super) elastic: Metres,
    /// The flexural parameter, metres.
    pub(super) alpha: Metres,
    /// Plan 14's constructional share `σ_volc`.
    pub(super) target: Metres,
    /// The edifice cap `h_max`.
    pub(super) cap: Metres,
    /// The cone field's coverage, its last.
    pub(super) coverage: f64,
    /// The provinces' count.
    pub(super) count: u64,
    /// The edifices' count, its last.
    pub(super) edifices: usize,
    /// The realised relief's area-weighted s.d., its last.
    pub(super) realised: Metres,
    /// The realised relief's s.d. before the correction.
    pub(super) first: Metres,
    /// Each cell's lift, metres.
    pub(super) lift: Vec<f64>,
}

/// Runs the provinces on `working`'s stagnant lid (see the module documentation), returning their
/// layout, or `None` where the lid has none.
pub(super) fn run(pass: &Pass<'_>, working: &mut Working) -> Option<Provinces> {
    let inputs = pass.inputs();
    if !working.plates.is_empty() || inputs.volcanism() <= 0.0 {
        return None;
    }
    let province_entry = inputs.crust_palette()[usize::from(u8::from(Crust::Province))]?;
    let province = inputs.palette().get(province_entry)?;
    let lid_entry = inputs.palette().get(inputs.lid_entry())?;
    let mantle = mantle_density(lid_entry)?;
    let fill = KilogramsPerCubicMetre::new(province.grain_density_kg_m3 * (1.0 - FILL_POROSITY));
    // A fill no lighter than the mantle beneath (a rocky province on an ice lid, which plan 14's
    // crusts never pair) has no flexural rule: no provinces (labelled).
    if mantle.value() - fill.value() <= 0.0 {
        return None;
    }
    let elastic = elastic_thickness(inputs)?;
    if elastic.value() <= 0.0 {
        return None;
    }
    let gravity = inputs.gravity();
    // The share and the cap are plan 14's figures, so they read plan 14's own lithosphere (Design
    // note 3: never left to drift), where the flexure reads the ruled one.
    let support = num::min(
        published_thickness(lid_entry, inputs).value() / SUPPORTING_LITHOSPHERE.value(),
        1.0,
    );
    let strength = CONSTRUCTIONAL_REFERENCE_GRAVITY.value() / gravity.value() * support;
    let target = CONSTRUCTIONAL_RELIEF.value() * strength * inputs.volcanism();
    let cap = TALLEST_EDIFICE_RELIEF.value() * strength;
    let alpha = flexural_parameter(elastic, mantle, fill, gravity, inputs.radius());
    let bending = Lid {
        alpha,
        foundation_n_per_m3: foundation_n_per_m3(elastic, mantle, fill, gravity, inputs.radius()),
        gravity,
    };
    let radius_m = inputs.radius().value();
    let base = num::max(
        cap / EDIFICE_ASPECT,
        boundary_diameter(pass.level(), inputs.radius()).value(),
    );
    let cone = Cone {
        height: Metres::new(cap),
        radius: Metres::new(base),
    };
    let load = KilogramsPerCubicMetre::new(province.grain_density_kg_m3);
    let edifice = Edifice::new(
        cone,
        load,
        bending,
        Metres::new(core::f64::consts::PI * radius_m),
    );
    let mut count_stream = pass.seed().stream(
        SURFACE_COARSE_RELIEF,
        ObjectKey::surface_cell(0, 0, 0, 0, PROVINCE_INSTANCE).expect("a level-0 cell is a key"),
    );
    let provinces = count_stream.poisson(PROVINCES_AT_UNIT_VOLCANISM * inputs.volcanism());
    if provinces == 0 {
        return None;
    }
    let centres: Vec<[f64; 3]> = (0..provinces)
        .map(|p| {
            count_stream.seek((p + 1) << 32);
            uniform_direction(&mut count_stream)
        })
        .collect();
    let ratio = target / cap;
    let mut coverage = cone_coverage(ratio * ratio);
    let mut laid = lay(pass, &edifice, &centres, coverage);
    let first = laid.realised;
    if first > 0.0 && first < target {
        coverage = num::min(
            coverage * (target / first) * (target / first),
            SATURATED_COVERAGE,
        );
        laid = lay(pass, &edifice, &centres, coverage);
    }
    for (n, cell) in working.cells.iter_mut().enumerate() {
        let z = Metres::new(num::assert_finite(cell.elevation.value() + laid.lift[n]));
        cell.elevation = z;
        cell.water_surface = z;
        let (h, w) = (laid.load[n], laid.deflection[n]);
        if h >= PROVINCE_SURFACE_MIN.value() || w - h >= PROVINCE_SURFACE_MIN.value() {
            cell.crust = Crust::Province;
        }
    }
    Some(Provinces {
        elastic,
        alpha,
        target: Metres::new(target),
        cap: Metres::new(cap),
        coverage,
        count: provinces,
        edifices: laid.edifices,
        realised: Metres::new(laid.realised),
        first: Metres::new(first),
        lift: laid.lift,
    })
}

/// Plan 14's own lithosphere for the lid entry `lid` of the body of `inputs`, by which its
/// constructional share and edifice cap are gated: its `lithosphere_thickness` of the crust class
/// the lid's family stands for, `IceRich` for water ice (its 651 ÷ T) and `Silicate` for every
/// other family the provinces reach.
#[must_use]
fn published_thickness(lid: &PaletteEntry, inputs: &CoarseInputs) -> Metres {
    let crust = match lid.mechanics {
        MechanicsFamily::WaterIce => LithosphereCrust::IceRich,
        MechanicsFamily::Silicate
        | MechanicsFamily::Metal
        | MechanicsFamily::VolatileIce
        | MechanicsFamily::Salt
        | MechanicsFamily::Organic => LithosphereCrust::Silicate,
    };
    lithosphere_thickness(crust, inputs.mean_surface_temperature(), inputs.heat_flow())
}

/// A cone field laid on a pass's cells ([`lay`]).
struct Laid {
    /// Each cell's lift, metres: max(min(h, `h_max`) − w, 0).
    lift: Vec<f64>,
    /// Each cell's summed load h, metres.
    load: Vec<f64>,
    /// Each cell's summed deflection w, metres, positive down.
    deflection: Vec<f64>,
    /// The lift's area-weighted s.d., metres.
    realised: f64,
    /// The edifices' count.
    edifices: usize,
}

/// The cone field's coverage f whose RMS is `ratio2` (σ ÷ `h_max`)² of its height: f ÷ 6 − f² ÷ 9 =
/// `ratio2`, f = ¾ (1 − √(1 − 16 `ratio2`)), at most ¾ (the module documentation).
#[must_use]
fn cone_coverage(ratio2: f64) -> f64 {
    let discriminant = 1.0 - 16.0 * ratio2;
    if discriminant <= 0.0 {
        SATURATED_COVERAGE
    } else {
        SATURATED_COVERAGE * (1.0 - discriminant.sqrt())
    }
}

/// A point uniform on the unit sphere, from two words: z uniform in [−1, 1) and the longitude
/// uniform in [0, 2π), as R09.T11's plates draw theirs.
fn uniform_direction(stream: &mut crate::rng::Stream) -> [f64; 3] {
    let z = 2.0 * stream.uniform() - 1.0;
    let longitude = core::f64::consts::TAU * stream.uniform();
    let (sin, cos) = math::sin_cos(longitude);
    let r = (1.0 - z * z).sqrt();
    [r * cos, r * sin, z]
}

/// The edifices of `pass`'s body at `coverage` about the province `centres`, laid on its cells in
/// edifice order, each over the cells its reach covers (a flood through edge neighbours from the
/// cell holding its centre).
#[must_use]
fn lay(pass: &Pass<'_>, edifice: &Edifice, centres: &[[f64; 3]], coverage: f64) -> Laid {
    let grid = pass.grid();
    let radius_m = pass.inputs().radius().value();
    let footprint = core::f64::consts::PI * edifice.radius_m * edifice.radius_m;
    let sphere = 4.0 * core::f64::consts::PI * radius_m * radius_m;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of edifices, finite, non-negative and held below MAX_EDIFICES"
    )]
    let count = num::min(
        (coverage * sphere / footprint).round(),
        count_f64(MAX_EDIFICES),
    ) as usize;
    let provinces = u64::try_from(centres.len()).expect("a province count fits a u64");
    let spread = edifice.radius_m * (count_f64(count) / count_f64(centres.len())).sqrt() / radius_m;
    let mut offsets = pass.seed().stream(
        SURFACE_COARSE_RELIEF,
        ObjectKey::surface_cell(0, 0, 0, 0, EDIFICE_INSTANCE).expect("a level-0 cell is a key"),
    );
    let cells = grid.len();
    let size = slot(cells);
    let (mut load, mut deflection) = (vec![0.0; size], vec![0.0; size]);
    let mut stamp = vec![u32::MAX; size];
    let mut frontier: Vec<u32> = Vec::new();
    for e in 0..count {
        let index = u64::try_from(e).expect("an edifice index fits a u64");
        offsets.seek(index << 8);
        let home = centres[usize::try_from(index % provinces).expect("a province index fits")];
        let (a, b) = offsets.standard_normal_pair();
        let centre = offset_direction(home, a * spread, b * spread);
        let reach_angle = edifice.reach_m / radius_m;
        let start = cell_index(
            PatchKey::containing(grid.level(), centre).expect("a coarse level is a patch level"),
        );
        let mark = u32::try_from(e).expect("an edifice index fits a u32");
        frontier.clear();
        frontier.push(start);
        stamp[slot(start)] = mark;
        while let Some(c) = frontier.pop() {
            let x = grid.centre(c);
            let gap = [x[0] - centre[0], x[1] - centre[1], x[2] - centre[2]];
            let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
            let angle = 2.0 * math::asin(num::min(0.5 * chord, 1.0));
            let r = angle * radius_m;
            load[slot(c)] += edifice.load_m(r);
            deflection[slot(c)] += edifice.deflection_m(r);
            // A cell two links beyond the reach may still neighbour one within it.
            if angle > reach_angle + 2.0 * grid.arc(c, grid.edge_neighbours(c)[0]).value() {
                continue;
            }
            for m in grid.edge_neighbours(c) {
                if stamp[slot(m)] != mark {
                    stamp[slot(m)] = mark;
                    frontier.push(m);
                }
            }
        }
    }
    let lift: Vec<f64> = load
        .iter()
        .zip(&deflection)
        .map(|(&h, &w)| num::max(num::min(h, edifice.height_m) - w, 0.0))
        .collect();
    let (_, realised) = noise::mean_and_spread(&lift, grid.solid_angles_sr());
    Laid {
        lift,
        load,
        deflection,
        realised,
        edifices: count,
    }
}

/// The unit direction of `home` moved by the tangent offsets `east` and `north` (radians) along a
/// basis of its tangent plane, put back on the sphere.
#[must_use]
fn offset_direction(home: [f64; 3], east: f64, north: f64) -> [f64; 3] {
    let axis = if home[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let e1 = unit(cross(axis, home));
    let e2 = cross(home, e1);
    unit([
        home[0] + east * e1[0] + north * e2[0],
        home[1] + east * e1[1] + north * e2[1],
        home[2] + east * e1[2] + north * e2[2],
    ])
}

/// a × b.
#[must_use]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// a ÷ |a|.
#[must_use]
fn unit(a: [f64; 3]) -> [f64; 3] {
    let norm = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / norm, a[1] / norm, a[2] / norm]
}
