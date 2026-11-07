//! The space-weathering darkening: Hapke 2001's nanophase-iron absorption inside the equivalent
//! slab, applied to a palette entry's fresh albedo at a maturity (decision-p14-t51d-weathering
//! §1.1, §1.3–§1.5 and §1.7; plan R10, R10.T1.d).
//!
//! # The physics
//!
//! Space weathering reduces a silicate's Fe²⁺ to nanophase metallic iron (npFe⁰) in grain rims and
//! agglutinitic glass. Small metal spheres in a dielectric host absorb as Rayleigh particles, which
//! adds an absorption coefficient to the host's own (Hapke 2001, JGR 106, 10039, *from memory*;
//! the form the ruling re-derives from a small sphere's absorption cross-section, and the one Lucey
//! and Riner 2011, Icarus 212, 451, apply, *from memory*):
//!
//! α = (36π z(λ) ÷ λ) φ (`ρ_h` ÷ `ρ_Fe`), z = `n_h`³ n k ÷ [(n² − k² + 2 `n_h`²)² + (2 n k)²],
//!
//! with φ the npFe⁰'s mass fraction of the host, `ρ_h` the host's grain density, `ρ_Fe` iron's
//! ([`IRON_DENSITY_KG_M3`]), `n_h` the host's index and n + ik iron's. z ÷ λ falls with
//! wavelength, so weathering darkens blue more than red: it reddens, as lunar soils do.
//!
//! # The law
//!
//! For a fresh albedo a₀ of a quantity X (the normal albedo `A_N`, the spherical albedo `A_S`, or
//! the directional-hemispherical albedo `A_h` at one of the table's incidences) at wavelength λ:
//!
//! 1. **The reference form** is the entry's law's: [`PhaseLaw::Regolith`] takes the lunar
//!    regolith's tables (its `A_N` was computed under that form), [`PhaseLaw::Ice`] the fresh
//!    frost's, and [`PhaseLaw::Lambert`] is the identity: no palette source writes it, and a
//!    consolidated form's factor applies after the law.
//! 2. **The fresh grain's single-scattering albedo** w₀ = X⁻¹(a₀): the form's tabulated X(w)
//!    inverted piecewise linearly, each quantity on its own column. A fresh value above the
//!    table's top takes w₀ = 1, and one at or below 0 takes w₀ = 0.
//! 3. **The internal transmission** of Hapke's equivalent slab, Θ₀ = (w₀ − `S_e`) ÷ [(1 − `S_e`)(1
//!    − `S_i`) + `S_i` (w₀ − `S_e`)], with the form's `S_e` and `S_i` (Hapke 2012's fits at the
//!    form's host index, k = 0: [`REGOLITH_HOST_INDEX`], [`ICE_HOST_INDEX`]). An opaque grain, w₀ ≤
//!    `S_e`, has no internal path to darken, and the law is the identity.
//! 4. **The npFe⁰ optical depth** τ = m f (`M_Fe` ÷ `M_FeO`)(`ρ_h` ÷ `ρ_Fe`)(36π z(λ) ÷ λ) Λ
//!    ([`npfe_optical_depth`]): m the maturity, f the entry's `FeO_T`, and Λ = `y_sat` ⟨D⟩ the
//!    law's one fitted constant, the saturated npFe⁰ yield per unit of `FeO`-iron times the
//!    form's mean path through a grain.
//! 5. **The darkened grain**: Θ = Θ₀ exp(−τ) and w = `S_e` + (1 − `S_e`)(1 − `S_i`) Θ ÷ (1 − `S_i`
//!    Θ).
//! 6. **The weathered albedo**: a₀ X(w) ÷ X(w₀). The ratio keeps a₀ itself at τ = 0, which is
//!    returned to the bit, and cancels most of the difference between the form's one host index and
//!    each substance's own.
//!
//! X(w) between the table's nodes is linear in w. Its single-scattering albedos are uniform in
//! √(1 − w), dense towards 1, where the albedos turn fastest.
//!
//! # The table
//!
//! [`tables::space_weathering`](crate::tables::space_weathering) is `hyperion-fit`'s
//! `space_weathering`: each form's X(w) by `substance_albedos`' own Hapke solution, its slab,
//! iron's z on a 300–5,000 nm grid at the form's host index (Johnson and Christy 1974, then Ordal
//! et al. 1988 joined to it), and Λ, fitted for the regolith to the Moon's mature highland and mare
//! normal albedos in B, V and R and derived for the frost by its own mean path.
//!
//! # Determinism
//!
//! No draw, so no domain tag. Pure `f64` arithmetic and [`math::exp`], tables read by linear
//! interpolation, every clamp an explicit comparison; no `f32`, no `min` or `max`. Out-of-range
//! inputs panic (the server and the sim validate them at the palette's and the header's build).
//!
//! # On the record from 24
//!
//! The albedo scale this module enters is solved only on a header that carries a maturity, which
//! P14.T54.b gives from generator version 24; below it no header did, and nothing on the record
//! read this module.
//!
//! # The mature colour (from generator version 31)
//!
//! The slab's floor is grey: as τ grows, Θ → 0 and every band's w → `S_e`, the grain's external
//! reflection, so that a mature row whose fresh albedo is far above the Moon's (the stand-in fresh
//! rows, 5–8× the measured fresh/mature contrast) is darkened to the floor and loses its colour:
//! `basalt_fe` at m = 1 reads B/V 0.956 against the Moon's 0.831, and the bright `feldspathic` at
//! m = 1 0.715, too red (decision-p14-t51d-weathering's D1 and D2, the form's dark end and the
//! stand-in fresh rows). The Moon's mature terrains of every iron content and albedo share one
//! colour instead, maria and highlands within 1% at zero phase (Gehrels, Coffeen and Owings 1964,
//! AJ 69, 826, Table XIII), the colour Λ's anchors already take: Dollfus and Bowell's whole-Moon
//! law, R(λ) ÷ R(`λ_V`) = (λ ÷ `λ_V`)^0.83 ([`MATURE_COLOUR_EXPONENT`]).
//!
//! So from 31 (`MATURE_COLOUR_VERSION`) a weathered albedo in a band whose wavelength the law
//! was measured over ([`MATURE_COLOUR_RANGE_NM`]) is the law's own colour in the share e^(−τ) of
//! its light that the grain's interior still returns, and the mature lunar colour in the rest,
//! the light its weathered surface returns, which the slab counts grey (the lane's lean,
//! 2026-11-07; [`toward_mature_colour`]): a^(e^(−τ)) [`a_V` (λ ÷ `λ_V`)^0.83]^(1 − e^(−τ)), with
//! τ the band's npFe⁰ optical depth and `a_V` the law's weathered albedo in V, which is unchanged,
//! so that Λ's fit is unchanged in V. A fresh albedo (τ 0) keeps its own colour, so fresh basalt
//! stays grey; the fit's mare anchor at m = 1 reads B/V 0.832 and R/V 1.128 against the Moon's
//! 0.831 and 1.134, and its highland 0.800 and 1.125. Never above the fresh albedo. A band outside
//! the law's measured range keeps the slab's.

use hyperion_base::{GENERATOR_VERSION, GeneratorVersion, math};

use super::PhaseLaw;
use crate::field::{BVR_EFFECTIVE_WAVELENGTHS_NM, PaletteEntry};
use crate::tables::space_weathering as table;

/// The host's real refractive index under the regolith form: 1.7, a mafic silicate's, labelled.
///
/// Pyroxene's visible index is about 1.65–1.75 and plagioclase's 1.53–1.59 (*from memory*, the
/// mineralogical handbooks' ranges, not read here); the lunar regolith's npFe⁰ sits in rims and
/// agglutinitic glass of a mafic silicate (decision-p14-t51d-weathering §1.1, step 3). One index
/// serves every regolith entry: the law's ratios cancel most of a substance's own difference.
pub const REGOLITH_HOST_INDEX: f64 = 1.7;

/// The host's real refractive index under the frost form: 1.31, ice Ih's in the visible.
///
/// Warren and Brandt 2008 (JGR 113, D14220) give ice Ih at 266 K n = 1.3163 at 440 nm, 1.311 at
/// 550 nm and 1.3083 at 640 nm, as rendering plan R08's `water-ice.json` reduces them (read); 1.31
/// is the visible's to two places.
pub const ICE_HOST_INDEX: f64 = 1.31;

/// The density of metallic iron, kg m⁻³: 7,874, α-iron's at room temperature (the CRC Handbook of
/// Chemistry and Physics' 7.874 g cm⁻³, *from memory*; decision-p14-t51d-weathering §1.1).
pub const IRON_DENSITY_KG_M3: f64 = 7_874.0;

/// The molar mass of iron, g mol⁻¹: 55.845, CIAAW's standard atomic weight of iron, 55.845(2)
/// (checked by the lane's science check, 2026-10-19).
pub const IRON_MOLAR_MASS_G_PER_MOL: f64 = 55.845;

/// The molar mass of `FeO`, g mol⁻¹: 71.844, iron's standard atomic weight, 55.845, and oxygen's
/// conventional atomic weight, 15.999 (CIAAW's; checked by the lane's science check, 2026-10-19).
pub const FEO_MOLAR_MASS_G_PER_MOL: f64 = 71.844;

/// One reference form's tables.
#[derive(Debug, Clone, Copy)]
struct Form {
    /// `A_N` at each node of [`table::W`].
    normal: &'static [f64],
    /// `A_S` at each node.
    spherical: &'static [f64],
    /// `A_h` at each node, at each of [`table::HEMISPHERICAL_INCIDENCE_COSINES`].
    hemispherical: &'static [[f64; 4]],
    /// `S_e` and `S_i` at the form's host index.
    slab: [f64; 2],
    /// Iron's z on the table's wavelength grid, at the form's host index.
    iron_z: &'static [f64],
    /// Λ, m.
    lambda_m: f64,
}

/// The lunar regolith form's tables.
const REGOLITH: Form = Form {
    normal: &table::REGOLITH_NORMAL_ALBEDO,
    spherical: &table::REGOLITH_SPHERICAL_ALBEDO,
    hemispherical: &table::REGOLITH_HEMISPHERICAL_ALBEDO,
    slab: table::REGOLITH_SLAB,
    iron_z: &table::REGOLITH_IRON_Z,
    lambda_m: table::REGOLITH_LAMBDA_M,
};

/// The fresh frost form's tables.
const FROST: Form = Form {
    normal: &table::FROST_NORMAL_ALBEDO,
    spherical: &table::FROST_SPHERICAL_ALBEDO,
    hemispherical: &table::FROST_HEMISPHERICAL_ALBEDO,
    slab: table::FROST_SLAB,
    iron_z: &table::FROST_IRON_Z,
    lambda_m: table::FROST_LAMBDA_M,
};

/// The form of `law`, or `None` for Lambert's, under which the law is the identity.
#[must_use]
const fn form_of(law: PhaseLaw) -> Option<&'static Form> {
    match law {
        PhaseLaw::Regolith => Some(&REGOLITH),
        PhaseLaw::Ice => Some(&FROST),
        PhaseLaw::Lambert => None,
    }
}

/// A tabulated quantity.
#[derive(Debug, Clone, Copy)]
enum Quantity {
    /// `A_N`.
    Normal,
    /// `A_S`.
    Spherical,
    /// `A_h` at the incidence of this index.
    Hemispherical(usize),
}

impl Form {
    /// `quantity` at node `node`.
    #[must_use]
    fn column(&self, quantity: Quantity, node: usize) -> f64 {
        match quantity {
            Quantity::Normal => self.normal[node],
            Quantity::Spherical => self.spherical[node],
            Quantity::Hemispherical(incidence) => self.hemispherical[node][incidence],
        }
    }

    /// `quantity` at single-scattering albedo `w` in [0, 1], linear in w between the nodes.
    #[must_use]
    fn albedo_at(&self, quantity: Quantity, w: f64) -> f64 {
        let nodes = &table::W;
        let last = nodes.len() - 1;
        let above = nodes.partition_point(|&node| node <= w);
        if above == 0 {
            return self.column(quantity, 0);
        }
        if above > last {
            return self.column(quantity, last);
        }
        let k = above - 1;
        let (a0, a1) = (self.column(quantity, k), self.column(quantity, k + 1));
        a0 + (w - nodes[k]) / (nodes[k + 1] - nodes[k]) * (a1 - a0)
    }

    /// The single-scattering albedo whose `quantity` is `fresh`: the column inverted linearly
    /// between its nodes; 1 above its top and 0 at or below 0.
    #[must_use]
    fn grain_albedo_of(&self, quantity: Quantity, fresh: f64) -> f64 {
        let nodes = &table::W;
        let last = nodes.len() - 1;
        if fresh <= 0.0 {
            return 0.0;
        }
        if fresh > self.column(quantity, last) {
            return 1.0;
        }
        // The partition point of the ascending column: the nodes whose value is at most `fresh`.
        let (mut lo, mut hi) = (0, nodes.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.column(quantity, mid) <= fresh {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return nodes[0];
        }
        if lo > last {
            return nodes[last];
        }
        let k = lo - 1;
        let (a0, a1) = (self.column(quantity, k), self.column(quantity, k + 1));
        nodes[k] + (fresh - a0) / (a1 - a0) * (nodes[k + 1] - nodes[k])
    }

    /// Iron's z at `wavelength_nm`: linear in λ between the grid's nodes, and the end node's below
    /// the grid's first or above its last.
    #[must_use]
    fn iron_z_at(&self, wavelength_nm: f64) -> f64 {
        let last = self.iron_z.len() - 1;
        let last_node = f64::from(u32::try_from(last).expect("the grid holds a few hundred nodes"));
        let t = (wavelength_nm - table::IRON_Z_FIRST_NM) / table::IRON_Z_STEP_NM;
        if t <= 0.0 {
            return self.iron_z[0];
        }
        if t >= last_node {
            return self.iron_z[last];
        }
        let below = t.floor();
        let k = node_index(below);
        let (z0, z1) = (self.iron_z[k], self.iron_z[k + 1]);
        z0 + (t - below) * (z1 - z0)
    }

    /// τ at `wavelength_nm` for grains of density `grain_density_kg_m3`, `FeO_T`
    /// `feo_mass_fraction` and maturity `maturity` (the law's step 4, in the module's
    /// documentation).
    #[must_use]
    fn optical_depth(
        &self,
        wavelength_nm: f64,
        grain_density_kg_m3: f64,
        feo_mass_fraction: f64,
        maturity: f64,
    ) -> f64 {
        let iron =
            maturity * feo_mass_fraction * IRON_MOLAR_MASS_G_PER_MOL / FEO_MOLAR_MASS_G_PER_MOL;
        let volume = grain_density_kg_m3 / IRON_DENSITY_KG_M3;
        let absorption =
            36.0 * core::f64::consts::PI * self.iron_z_at(wavelength_nm) / (wavelength_nm * 1.0e-9);
        iron * volume * absorption * self.lambda_m
    }

    /// `fresh` of `quantity` weathered by npFe⁰ of optical depth `tau` (steps 2, 3, 5 and 6).
    #[must_use]
    fn weathered(&self, quantity: Quantity, fresh: f64, tau: f64) -> f64 {
        if tau <= 0.0 {
            return fresh;
        }
        let w0 = self.grain_albedo_of(quantity, fresh);
        let [s_e, s_i] = self.slab;
        if w0 <= s_e {
            return fresh;
        }
        let excess = w0 - s_e;
        let theta0 = excess / ((1.0 - s_e) * (1.0 - s_i) + s_i * excess);
        let theta = theta0 * math::exp(-tau);
        let w = s_e + (1.0 - s_e) * (1.0 - s_i) * theta / (1.0 - s_i * theta);
        // A τ too small to move w past rounding leaves the albedo as it was.
        if w >= w0 {
            return fresh;
        }
        let weathered = fresh * self.albedo_at(quantity, w) / self.albedo_at(quantity, w0);
        // The quotient's rounding may not carry it past the fresh value.
        if weathered > fresh { fresh } else { weathered }
    }
}

/// The index of grid node `below`, a whole number from 0 to below the grid's last node, which the
/// caller bounds: cast to a `u32`, never through a `usize`'s saturation, then widened.
///
/// # Panics
///
/// Never on a target this crate builds for, whose `usize` holds any `u32`.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "below is a whole number from 0 to below the grid's last node, a few hundred, which a \
              u32 holds exactly: the caller compares it with the last node first"
)]
fn node_index(below: f64) -> usize {
    let node = below as u32;
    usize::try_from(node).expect("a usize holds a u32 on every target")
}

/// Checks that `value` is finite and in [0, 1].
///
/// # Panics
///
/// If it is not, naming it `what`.
pub(super) fn check_unit(what: &str, value: f64) {
    assert!(
        (0.0..=1.0).contains(&value),
        "{what} must be in [0, 1], got {value}"
    );
}

/// Checks the inputs of τ.
///
/// # Panics
///
/// As [`npfe_optical_depth`].
fn check_inputs(
    wavelength_nm: f64,
    grain_density_kg_m3: f64,
    feo_mass_fraction: f64,
    maturity: f64,
) {
    assert!(
        wavelength_nm.is_finite() && wavelength_nm > 0.0,
        "a wavelength must be finite and positive, got {wavelength_nm} nm"
    );
    assert!(
        grain_density_kg_m3.is_finite() && grain_density_kg_m3 > 0.0,
        "a grain density must be finite and positive, got {grain_density_kg_m3} kg m⁻³"
    );
    check_unit("an FeO_T mass fraction", feo_mass_fraction);
    check_unit("a maturity", maturity);
}

/// Checks a fresh albedo.
///
/// # Panics
///
/// If it is not finite and non-negative.
fn check_fresh(fresh: f64) {
    assert!(
        fresh.is_finite() && fresh >= 0.0,
        "a fresh albedo must be finite and non-negative, got {fresh}"
    );
}

/// The npFe⁰ optical depth τ, dimensionless, of grains under `law` at `wavelength_nm` (nm): m f
/// (`M_Fe` ÷ `M_FeO`)(`ρ_h` ÷ `ρ_Fe`)(36π z(λ) ÷ λ) Λ (the law's step 4,
/// decision-p14-t51d-weathering §1.1); 0 for [`PhaseLaw::Lambert`].
///
/// `grain_density_kg_m3` is the host's grain density `ρ_h` (kg m⁻³), `feo_mass_fraction` its
/// `FeO_T` f and `maturity` m, both dimensionless in [0, 1]. Iron's z is the table's at the form's
/// host index, linear in λ between its 10 nm nodes and, outside its 300–5,000 nm, the end node's
/// held (λ itself still divides it). It is linear in m and in f.
///
/// # Panics
///
/// If the wavelength or the density is not finite and positive, or the `FeO_T` or the maturity is
/// not in [0, 1] (a NaN included).
#[must_use]
pub fn npfe_optical_depth(
    law: PhaseLaw,
    wavelength_nm: f64,
    grain_density_kg_m3: f64,
    feo_mass_fraction: f64,
    maturity: f64,
) -> f64 {
    check_inputs(
        wavelength_nm,
        grain_density_kg_m3,
        feo_mass_fraction,
        maturity,
    );
    form_of(law).map_or(0.0, |form| {
        form.optical_depth(
            wavelength_nm,
            grain_density_kg_m3,
            feo_mass_fraction,
            maturity,
        )
    })
}

/// `fresh` of `quantity` weathered under `law`, the inputs checked.
#[must_use]
fn weathered(
    quantity: Quantity,
    fresh: f64,
    wavelength_nm: f64,
    law: PhaseLaw,
    grain_density_kg_m3: f64,
    feo_mass_fraction: f64,
    maturity: f64,
) -> f64 {
    check_fresh(fresh);
    check_inputs(
        wavelength_nm,
        grain_density_kg_m3,
        feo_mass_fraction,
        maturity,
    );
    let Some(form) = form_of(law) else {
        return fresh;
    };
    let tau = form.optical_depth(
        wavelength_nm,
        grain_density_kg_m3,
        feo_mass_fraction,
        maturity,
    );
    form.weathered(quantity, fresh, tau)
}

/// The normal albedo `A_N` of ground whose fresh normal albedo is `fresh`, weathered to `maturity`
/// under `law` at `wavelength_nm` (decision-p14-t51d-weathering §1.1's law): a₀ `A_N`(w) ÷
/// `A_N`(w₀), dimensionless, never above `fresh`.
///
/// It is `fresh` itself, to the bit, at m = 0, at f = 0, under [`PhaseLaw::Lambert`], for an opaque
/// grain (w₀ at or below the slab's `S_e`), and wherever τ is too small to move w; and it does not
/// rise with m or f. The other inputs are [`npfe_optical_depth`]'s.
///
/// # Panics
///
/// If `fresh` is not finite and non-negative, or as [`npfe_optical_depth`].
#[must_use]
pub fn weathered_normal_albedo(
    fresh: f64,
    wavelength_nm: f64,
    law: PhaseLaw,
    grain_density_kg_m3: f64,
    feo_mass_fraction: f64,
    maturity: f64,
) -> f64 {
    weathered(
        Quantity::Normal,
        fresh,
        wavelength_nm,
        law,
        grain_density_kg_m3,
        feo_mass_fraction,
        maturity,
    )
}

/// The spherical albedo `A_S` (a band's Bond albedo) of ground whose fresh spherical albedo is
/// `fresh`, weathered as [`weathered_normal_albedo`] weathers `A_N`, on the form's own `A_S`(w):
/// a₀ `A_S`(w) ÷ `A_S`(w₀).
///
/// # Panics
///
/// As [`weathered_normal_albedo`].
#[must_use]
pub fn weathered_spherical_albedo(
    fresh: f64,
    wavelength_nm: f64,
    law: PhaseLaw,
    grain_density_kg_m3: f64,
    feo_mass_fraction: f64,
    maturity: f64,
) -> f64 {
    weathered(
        Quantity::Spherical,
        fresh,
        wavelength_nm,
        law,
        grain_density_kg_m3,
        feo_mass_fraction,
        maturity,
    )
}

/// The directional-hemispherical albedos `A_h` of ground whose fresh ones are `fresh`, at the
/// incidence cosines 1, 0.5, 0.2 and 0.05 in that order (the table's
/// [`HEMISPHERICAL_INCIDENCE_COSINES`](table::HEMISPHERICAL_INCIDENCE_COSINES), the sim's), each
/// weathered as [`weathered_normal_albedo`] weathers `A_N`, on the form's own `A_h`(w) at its
/// incidence.
///
/// # Panics
///
/// If any fresh value is not finite and non-negative, or as [`npfe_optical_depth`].
#[must_use]
pub fn weathered_hemispherical_albedos(
    fresh: [f64; 4],
    wavelength_nm: f64,
    law: PhaseLaw,
    grain_density_kg_m3: f64,
    feo_mass_fraction: f64,
    maturity: f64,
) -> [f64; 4] {
    let mut out = fresh;
    for (incidence, value) in out.iter_mut().enumerate() {
        *value = weathered(
            Quantity::Hemispherical(incidence),
            *value,
            wavelength_nm,
            law,
            grain_density_kg_m3,
            feo_mass_fraction,
            maturity,
        );
    }
    out
}

/// The normal albedos in B, V and R of palette entry `entry` weathered to `maturity`
/// (dimensionless, in [0, 1]), each dimensionless and at most the entry's own: the
/// [`weathered_normal_albedo`] of each of its
/// [`normal_albedo_bvr`](PaletteEntry::normal_albedo_bvr) at
/// [`BVR_EFFECTIVE_WAVELENGTHS_NM`], at its grain density and its
/// [`feo_mass_fraction`](PaletteEntry::feo_mass_fraction).
///
/// The law is the one its [`phase_row`](PaletteEntry::phase_row) names, or, where it names none
/// this build knows, its family's default ([`PhaseLaw::for_family`]), as
/// [`properties()`](super::properties) takes it for a particulate form. An entry whose `FeO_T` is
/// `None` (not modelled, or an observed body's albedo that is weathered already) keeps its own
/// albedos to the bit, so that nothing darkens twice (decision-p14-t51d-weathering §1.5), as does
/// one of `FeO_T` 0 (an ice, an iron-free solid). From generator version 31 its B and R are drawn
/// toward the mature lunar colour on its V ([`toward_mature_colour`], `MATURE_COLOUR_VERSION`).
///
/// # Panics
///
/// If the maturity is not in [0, 1], or as [`weathered_normal_albedo`] for an entry whose values a
/// [`MaterialPalette`](crate::field::MaterialPalette) would refuse.
#[must_use]
pub fn weathered_entry_bvr(entry: &PaletteEntry, maturity: f64) -> [f64; 3] {
    check_unit("a maturity", maturity);
    let Some(feo_mass_fraction) = entry.feo_mass_fraction else {
        return entry.normal_albedo_bvr;
    };
    let law = PhaseLaw::try_from(entry.phase_row)
        .unwrap_or_else(|_| PhaseLaw::for_family(entry.mechanics));
    let mut out = entry.normal_albedo_bvr;
    for (value, wavelength_nm) in out.iter_mut().zip(BVR_EFFECTIVE_WAVELENGTHS_NM) {
        *value = weathered_normal_albedo(
            *value,
            wavelength_nm,
            law,
            entry.grain_density_kg_m3,
            feo_mass_fraction,
            maturity,
        );
    }
    match MatureColourRules::in_force() {
        MatureColourRules::Slab => out,
        MatureColourRules::Mature => {
            let v = (out[1], BVR_EFFECTIVE_WAVELENGTHS_NM[1]);
            for band in [0, 2] {
                let wavelength_nm = BVR_EFFECTIVE_WAVELENGTHS_NM[band];
                let tau = npfe_optical_depth(
                    law,
                    wavelength_nm,
                    entry.grain_density_kg_m3,
                    feo_mass_fraction,
                    maturity,
                );
                out[band] = toward_mature_colour(
                    out[band],
                    entry.normal_albedo_bvr[band],
                    wavelength_nm,
                    tau,
                    v,
                );
            }
            out
        }
    }
}

/// The mature lunar regolith's colour exponent β, dimensionless: 0.83, the slope of Dollfus and
/// Bowell 1971's whole-Moon spectral law (A&A 10, 29, eq. 11, p. 50: log Ā = 0.83 log λ − 0.80,
/// read for decision-r10-t1d-anchor-colour), R(λ) ÷ R(600 nm) = (λ ÷ 600 nm)^0.83, which Λ's
/// anchors take for both terrains (the fit's `colour_exponent`), and which the mature terrains
/// share within 1% at zero phase (Gehrels, Coffeen and Owings 1964, AJ 69, 826, Table XIII:
/// maria B/V 0.826, highlands 0.831): the colour a weathered regolith tends to from generator
/// version 31 ([`toward_mature_colour`]).
pub const MATURE_COLOUR_EXPONENT: f64 = 0.83;

/// The wavelengths over which the mature colour is measured, nm: 360 to 940, the four colours of
/// Gehrels et al. 1964 to which Dollfus and Bowell fit eq. 11 ("in 4 colours spanning the
/// wavelength range 0.36 µ to 0.94 µ", as decision-r10-t1d-anchor-colour reads it). A band beyond
/// them keeps the slab's weathered albedo ([`toward_mature_colour`]).
pub const MATURE_COLOUR_RANGE_NM: [f64; 2] = [360.0, 940.0];

/// The generator version from which a weathered entry's colour tends to the mature lunar colour
/// (the [module documentation](self), "The mature colour"): 31, held with the 31 train
/// (main's ruling of 2026-11-07: train 30's list froze first).
///
/// Below it [`MatureColourRules::in_force`] is [`MatureColourRules::Slab`], and
/// [`weathered_entry_bvr`] is the slab's alone, bit for bit. 31's bump deletes this hold,
/// [`MatureColourRules`] with its `Slab` arm and its match in [`weathered_entry_bvr`], the tests'
/// switch and its tripwire (`the_mature_colour_is_held_only_until_the_31_bump`). The sim's area
/// mix takes [`toward_mature_colour`] under its own hold at 31 (`photometry::SURFACE_COLOUR_VERSION`).
pub(crate) const MATURE_COLOUR_VERSION: GeneratorVersion = GeneratorVersion::new(31);

/// Which colour a weathered palette entry takes (its hold, [`MATURE_COLOUR_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MatureColourRules {
    /// Version 30's and before: the slab's in every band.
    Slab,
    /// Version 31's: the slab's drawn toward the mature lunar colour ([`toward_mature_colour`]).
    Mature,
}

impl MatureColourRules {
    /// The rules this generator version gives: the mature colour from [`MATURE_COLOUR_VERSION`],
    /// the slab's below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_mature_colour`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = MATURE_COLOUR_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= MATURE_COLOUR_VERSION {
            Self::Mature
        } else {
            Self::Slab
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The mature colour's rules a test sets on its thread, `None` for the version's.
    static MATURE_COLOUR_RULES: core::cell::Cell<Option<MatureColourRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the mature colour's rules `rules` on this thread, for the tests to reach
/// 31's colours ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. 31's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_mature_colour<R>(rules: MatureColourRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<MatureColourRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            MATURE_COLOUR_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(MATURE_COLOUR_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The weathered albedo `weathered` at `wavelength_nm` (nm) drawn toward the mature lunar colour
/// (the [module documentation](self), "The mature colour"; from generator version 31): the
/// slab's own colour in the share e^(−τ) of the light the grain's interior still returns, τ the
/// band's npFe⁰ optical depth `tau` ([`npfe_optical_depth`]), and in the rest the mature colour
/// on the band V's weathered albedo `v_weathered` at `v_wavelength_nm`: `a`^(e^(−τ)) [`a_V`
/// (λ ÷ `λ_V`)^β]^(1 − e^(−τ)), β [`MATURE_COLOUR_EXPONENT`], dimensionless, and at most `fresh`,
/// the band's fresh albedo, as the slab's is.
///
/// It is `weathered` itself, to the bit, at τ 0 (a fresh or iron-free grain), for a wavelength
/// outside [`MATURE_COLOUR_RANGE_NM`] and for an albedo or a V albedo that is not above 0; the V
/// band's own caller keeps its V. Its inputs are the slab's outputs, which are finite: a value
/// that is not is returned as it is.
#[must_use]
pub fn toward_mature_colour(
    weathered: f64,
    fresh: f64,
    wavelength_nm: f64,
    tau: f64,
    (v_weathered, v_wavelength_nm): (f64, f64),
) -> f64 {
    let [shortest, longest] = MATURE_COLOUR_RANGE_NM;
    let measured = wavelength_nm >= shortest && wavelength_nm <= longest;
    if !(measured && tau > 0.0 && weathered > 0.0 && v_weathered > 0.0 && v_wavelength_nm > 0.0)
        || !(tau.is_finite() && weathered.is_finite() && v_weathered.is_finite())
    {
        return weathered;
    }
    let interior = math::exp(-tau);
    let mature = v_weathered * math::powf(wavelength_nm / v_wavelength_nm, MATURE_COLOUR_EXPONENT);
    let drawn = math::powf(weathered, interior) * math::powf(mature, 1.0 - interior);
    if drawn > fresh { fresh } else { drawn }
}

#[cfg(test)]
mod tests;
