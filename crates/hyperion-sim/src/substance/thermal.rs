//! The condensed phases' thermal columns and the regolith's thermal inertia (plan 14, P14.T49.j;
//! decision-p14-t49c-split, 2026-10-16; science-r09-drainage.md §3; science-p14-t24a-climate.md §6,
//! item 2).
//!
//! # The thermal columns
//!
//! Every registry row that can form a surface carries its solid's specific heat c(T), J kg⁻¹ K⁻¹,
//! and thermal conductivity k(T), W m⁻¹ K⁻¹ ([`ThermalColumns`], through
//! [`Substance::thermal_columns`]). Both are the dense solid's, with no pore space, as the row's
//! grain density is: porosity is how a material lies, which its form states, so that "the row
//! should carry its solid's specific heat and conductivity, and the form its layer's inertia"
//! (science-r09-drainage.md §3). The columns are a table in `thermal/rows.rs`, found by key, so
//! that [`Substance`] gains no field; [`rows`] lists it in registry order, and that file's module
//! documentation lists the rows not modelled, each with its reason.
//!
//! Each column ([`ThermalColumn`]) is a closed form its source gives or a few read points
//! ([`ThermalForm`]), over its valid range, in one piece per solid phase. Outside the range the end
//! value is held, as [`SolidDensity::Polynomial`](super::SolidDensity::Polynomial)'s density is.
//! Every column names its sources and their basis ([`Source`], [`Basis`]): a measured value or a
//! reference equation's is a fact, a mode-weighted sum is reduced, and HYPERION's own choice beside
//! the cited values is adopted. A row that takes another row's figure because its own was not read
//! is a stand-in ([`Provenance::StandIn`]): its source says why, and the figure is the same
//! constant as the row it borrows from.
//!
//! # The regolith's thermal inertia
//!
//! [`regolith_thermal_inertia`] gives a substance's thermal inertia as a dry regolith in vacuum,
//! Γ = √(`k_eff` `ρ_bulk` c), J m⁻² K⁻¹ s^−½, at a [`RegolithForm`]'s porosity ψ and grain size:
//!
//! - `ρ_bulk` = `ρ_g` (1 − ψ), the row's solid grain density at the temperature
//!   ([`Substance::grain_density`]) times one less the form's porosity
//!   (science-r09-drainage.md §3);
//! - c is the solid's c(T), the pores being empty;
//! - `k_eff` is Gundlach and Blum's granular conductivity (2013, Icarus 223, 479, arXiv:1212.3108,
//!   eq. 5, after their 2012 model, Icarus 219, 618, arXiv:1111.0535), with φ = 1 − ψ the volume
//!   filling factor, r the grain radius, `k_s`(T) the solid's conductivity, ε the form's emissivity
//!   and σ Stefan and Boltzmann's constant:
//!
//!   `k_eff` = `k_s` [(9π ÷ 4) (1 − ν²) ÷ E · γ(T) ÷ r]^⅓ · f₁ exp(f₂ φ) · χ
//!   + 8 σ ε T³ · e₁ (1 − φ) ÷ φ · r.
//!
//!   The first term is conduction through the grains' contacts: Chan and Tien's (1973) network of
//!   spheres in contact, its contact force the grains' van der Waals adhesion by the theory of
//!   Johnson, Kendall and Roberts (1971) in place of their weight, which dominates in a regolith's
//!   top layers. E, ν and γ are the grains' Young's modulus, Poisson's ratio and specific surface
//!   energy, the form's by family ([`ContactConstants`]); f₁ exp(f₂ φ) is the packing's structure
//!   and χ the reduction for irregular, polydisperse grains (`CONTACT_STRUCTURE_F1`,
//!   `CONTACT_STRUCTURE_F2`, `IRREGULAR_GRAIN_FACTOR`). The second is radiation across the pores,
//!   its photons' mean free path e₁ (1 − φ) ÷ φ · r (`PHOTON_PATH_FACTOR`).
//!
//! Gundlach and Blum's model is chosen over Sakatani et al.'s (2017, AIP Advances 7, 015310, read
//! in abstract), for three reasons: its one irregularity factor χ is calibrated on the returned
//! lunar regolith (Cremers et al. 1971; Cremers and Birkebak 1971), where Sakatani et al.'s is
//! built on glass beads; it takes the adhesion of the grains, and so no body's gravity or
//! compressional stress, which Sakatani et al.'s needs as an input, so that the inertia stays a
//! function of the substance, the form and the temperature alone; and the same equations serve
//! silicate and icy regoliths (Gundlach and Blum 2012 built it for icy bodies' dust layers).
//!
//! The law is the airless one, as ruled: it has no pore gas. Under an atmosphere, gas in the pores
//! raises a regolith's conductivity well above the vacuum's (Presley and Christensen 1997, JGR
//! 102, 6535; Piqueux and Christensen 2009, JGR 114, E09005; neither re-read), so the law
//! under-reads a regolith under air, Mars's among them. The temperature is the caller's.
//!
//! # Held at generator version 22
//!
//! Nothing on the record calls this module until P14.T54.a, which passes the inertia at P14.T51.c's
//! lid to P14.T24.a in place of its two provisional constants, the
//! `ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S` and
//! `ICE_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S` of
//! [`hooks::surface`](crate::planetary::hooks::surface). So generator version 22 publishes none
//! of it, and a change to it re-blesses
//! `tests/golden/substance/thermal.golden` at the version in force. A test,
//! `substance_thermal_nothing_on_the_record_reads_the_inertia_yet`, holds that, and P14.T54.a
//! deletes it. Rendering plan R09's R09.T13.c reuses the same law for its airless model.
//!
//! Everything here is a pure function of the registry and its arguments: no draws, no state, no
//! caches, loops in registry order and sums in a fixed order, with the crate's deterministic
//! [`math`](crate::math).

use core::f64::consts::PI;
use std::error::Error;
use std::fmt;

use hyperion_surface::field::MechanicsFamily;
use hyperion_surface::material::{MaterialClass, ShearLayer, form_row};

use super::condensed::CondensedPhase;
use super::{Basis, Source, Substance, SubstanceId, ids};
use crate::math;
use crate::planetary::hooks::surface::REGOLITH_EMISSIVITY;
use crate::units::consts::STEFAN_BOLTZMANN;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Metres, Pascals};

mod rows;
#[cfg(test)]
pub(super) mod tests;

/// How one piece of a thermal column follows temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalForm {
    /// One value at every temperature, in the column's unit.
    Constant(f64),
    /// Σ cᵢ Tⁱ, T in kelvin, from c₀, evaluated by Horner's rule from the highest power: two
    /// coefficients are a line.
    Polynomial(&'static [f64]),
    /// Σ cᵢ T^(i/2), T in kelvin, from c₀, evaluated by Horner's rule in √T from the highest
    /// power, as Winter and Saari 1969's silicate heat capacity in T^½, T and T^(3/2).
    HalfPowers(&'static [f64]),
    /// exp(Σ aᵢ (ln T)ⁱ), T in kelvin, from a₀, the sum by Horner's rule in ln T: a conductivity
    /// fitted in its logarithm, as Saiduzzaman et al. 2025's solid CO₂.
    LogPolynomial(&'static [f64]),
    /// a ÷ T + b, T in kelvin: a crystal's conductivity above its low-temperature peak, as ice
    /// Ih's 567 ÷ T (Klinger 1980).
    Reciprocal {
        /// a, in the column's unit times kelvin.
        a: f64,
        /// b, in the column's unit.
        b: f64,
    },
    /// Read points (T in kelvin, value), in increasing temperature, linear in T between
    /// neighbours; each point is its source's value, converted to SI, never a source's table whole.
    Points(&'static [(f64, f64)]),
    /// Ice Ih's specific isobaric heat capacity at one atmosphere, `c_p` = −T ∂²g ÷ ∂T², from the
    /// Gibbs function of IAPWS R10-06(2009) (Feistel and Wagner 2006, J. Phys. Chem. Ref. Data 35,
    /// 1021): the closed form of complex reciprocals of its Table 4, with its Table 2's constants.
    IceIh,
}

impl ThermalForm {
    /// The form's value at `temperature_k`, kelvin, in the column's unit.
    #[must_use]
    fn evaluate(self, temperature_k: f64) -> f64 {
        match self {
            Self::Constant(value) => value,
            Self::Polynomial(coefficients) => coefficients
                .iter()
                .rev()
                .fold(0.0, |sum, &c| sum * temperature_k + c),
            Self::HalfPowers(coefficients) => {
                let root = temperature_k.sqrt();
                coefficients
                    .iter()
                    .rev()
                    .fold(0.0, |sum, &c| sum * root + c)
            }
            Self::LogPolynomial(coefficients) => {
                let log = math::ln(temperature_k);
                math::exp(coefficients.iter().rev().fold(0.0, |sum, &c| sum * log + c))
            }
            Self::Reciprocal { a, b } => a / temperature_k + b,
            Self::Points(points) => interpolate(points, temperature_k),
            Self::IceIh => ice_ih_heat_capacity(temperature_k, ONE_ATMOSPHERE_PA),
        }
    }
}

/// The read points' value at `t`: the first point's at or below it, the last's at or above it,
/// and otherwise linear between the two neighbours that hold it, so that a read point returns its
/// own value to the bit; a NaN gives a NaN.
#[must_use]
fn interpolate(points: &[(f64, f64)], t: f64) -> f64 {
    if t.is_nan() {
        return t;
    }
    let mut previous: Option<(f64, f64)> = None;
    for &(t1, y1) in points {
        if t < t1 {
            return match previous {
                Some((t0, y0)) => y0 + (y1 - y0) * (t - t0) / (t1 - t0),
                None => y1,
            };
        }
        previous = Some((t1, y1));
    }
    previous.map_or(f64::NAN, |(_, y)| y)
}

/// One atmosphere, Pa, at which [`ThermalForm::IceIh`] evaluates IAPWS R10-06's Gibbs function:
/// its normal pressure p₀ (Table 1).
const ONE_ATMOSPHERE_PA: f64 = 101_325.0;

/// A complex number, for IAPWS R10-06's Gibbs function alone, in a fixed order of operations.
#[derive(Debug, Clone, Copy)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    #[must_use]
    const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    #[must_use]
    fn add(self, other: Self) -> Self {
        Self::new(self.re + other.re, self.im + other.im)
    }

    #[must_use]
    fn sub(self, other: Self) -> Self {
        Self::new(self.re - other.re, self.im - other.im)
    }

    #[must_use]
    fn scale(self, factor: f64) -> Self {
        Self::new(self.re * factor, self.im * factor)
    }

    /// The real part of the product `self × other`.
    #[must_use]
    fn real_of_product(self, other: Self) -> f64 {
        self.re * other.re - self.im * other.im
    }

    /// 1 ÷ `self`, as (a − bi) ÷ (a² + b²).
    #[must_use]
    fn recip(self) -> Self {
        let norm = self.re * self.re + self.im * self.im;
        Self::new(self.re / norm, -self.im / norm)
    }
}

/// IAPWS R10-06(2009)'s triple-point temperature `T_t`, K (Table 1).
const IAPWS_TRIPLE_TEMPERATURE_K: f64 = 273.16;
/// IAPWS R10-06(2009)'s experimental triple-point pressure `p_t`, Pa (Table 1).
const IAPWS_TRIPLE_PRESSURE_PA: f64 = 611.657;
/// IAPWS R10-06(2009)'s Table 2: t₁.
const IAPWS_T1: Complex = Complex::new(0.368_017_112_855_051e-1, 0.510_878_114_959_572e-1);
/// IAPWS R10-06(2009)'s Table 2: r₁, J kg⁻¹ K⁻¹.
const IAPWS_R1: Complex = Complex::new(0.447_050_716_285_388e2, 0.656_876_847_463_481e2);
/// IAPWS R10-06(2009)'s Table 2: t₂.
const IAPWS_T2: Complex = Complex::new(0.337_315_741_065_416, 0.335_449_415_919_309);
/// IAPWS R10-06(2009)'s Table 2: r₂₀, J kg⁻¹ K⁻¹.
const IAPWS_R20: Complex = Complex::new(-0.725_974_574_329_220e2, -0.781_008_427_112_870e2);
/// IAPWS R10-06(2009)'s Table 2: r₂₁, J kg⁻¹ K⁻¹.
const IAPWS_R21: Complex = Complex::new(-0.557_107_698_030_123e-4, 0.464_578_634_580_806e-4);
/// IAPWS R10-06(2009)'s Table 2: r₂₂, J kg⁻¹ K⁻¹.
const IAPWS_R22: Complex = Complex::new(0.234_801_409_215_913e-10, -0.285_651_142_904_972e-10);

/// Ice Ih's specific isobaric heat capacity, J kg⁻¹ K⁻¹, at `temperature_k` kelvin and
/// `pressure_pa` pascals (IAPWS R10-06(2009), Tables 3 and 4).
///
/// It is `c_p` = −T `g_TT`, with `g_TT` = (1 ÷ `T_t`) Re Σₖ rₖ [1 ÷ (tₖ − τ) + 1 ÷ (tₖ + τ) − 2 ÷
/// tₖ], τ = T ÷ `T_t` and r₂(p) = Σ r₂ₖ (π − π₀)ᵏ, π = p ÷ `p_t`. It is valid over 0–273.16 K and
/// 0–2.1 × 10⁸ Pa, within 2 per cent at or below 10⁵ Pa (its Table 5). At one atmosphere π − π₀ is
/// exactly 0, so r₂ is r₂₀.
#[must_use]
fn ice_ih_heat_capacity(temperature_k: f64, pressure_pa: f64) -> f64 {
    let tau = temperature_k / IAPWS_TRIPLE_TEMPERATURE_K;
    let excess =
        pressure_pa / IAPWS_TRIPLE_PRESSURE_PA - ONE_ATMOSPHERE_PA / IAPWS_TRIPLE_PRESSURE_PA;
    let r2 = IAPWS_R20
        .add(IAPWS_R21.scale(excess))
        .add(IAPWS_R22.scale(excess * excess));
    let term = |r: Complex, t: Complex| {
        let tau = Complex::new(tau, 0.0);
        let bracket = t
            .sub(tau)
            .recip()
            .add(t.add(tau).recip())
            .sub(t.recip().scale(2.0));
        r.real_of_product(bracket)
    };
    -tau * (term(IAPWS_R1, IAPWS_T1) + term(r2, IAPWS_T2))
}

/// One piece of a thermal column: one solid phase's form over its temperatures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalPiece {
    phase: &'static str,
    low: Kelvin,
    high: Kelvin,
    form: ThermalForm,
}

impl ThermalPiece {
    /// The solid phase it is, such as `"α"` or `"β"`, or `""` for a row's only solid.
    #[must_use]
    pub const fn phase(&self) -> &'static str {
        self.phase
    }

    /// The lowest temperature it covers.
    #[must_use]
    pub const fn low(&self) -> Kelvin {
        self.low
    }

    /// The highest temperature it covers, where the next piece, the next phase, begins.
    #[must_use]
    pub const fn high(&self) -> Kelvin {
        self.high
    }

    /// Its form.
    #[must_use]
    pub const fn form(&self) -> ThermalForm {
        self.form
    }
}

/// Whether a column is the row's own or another's standing in for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Provenance {
    /// The row's own: measured on it, its reference equation's, or reduced from its stated mode.
    Own,
    /// Another row's or a class's figure, taken because the row's own was not read: its source
    /// says why, and the figure is the same constant as the one it borrows.
    StandIn,
}

/// One thermal column, the specific heat or the conductivity of a row's solid: its pieces, in
/// increasing temperature, each beginning where the last ends, with its provenance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalColumn {
    pieces: &'static [ThermalPiece],
    provenance: Provenance,
}

impl ThermalColumn {
    /// Its pieces, in increasing temperature: one for a row's only solid, one per phase otherwise.
    #[must_use]
    pub const fn pieces(&self) -> &'static [ThermalPiece] {
        self.pieces
    }

    /// Whether it is the row's own or a stand-in.
    #[must_use]
    pub const fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// Whether it stands in for the row's own figure.
    #[must_use]
    pub const fn is_stand_in(&self) -> bool {
        matches!(self.provenance, Provenance::StandIn)
    }

    /// The lowest temperature of its valid range.
    ///
    /// # Panics
    ///
    /// Never: every column has a piece, which `substance_thermal_ranges_and_values_are_physical`
    /// holds.
    #[must_use]
    pub fn low(&self) -> Kelvin {
        self.pieces
            .first()
            .expect("every column has a piece, as the rows' tests hold")
            .low
    }

    /// The highest temperature of its valid range.
    ///
    /// # Panics
    ///
    /// Never, as [`low`](Self::low).
    #[must_use]
    pub fn high(&self) -> Kelvin {
        self.pieces
            .last()
            .expect("every column has a piece, as the rows' tests hold")
            .high
    }

    /// Its value at `temperature`, in its unit: its piece's form at the temperature, the end
    /// value held outside its range, and at a piece's upper bound the lower piece's. A NaN gives
    /// a NaN.
    ///
    /// # Panics
    ///
    /// Never, as [`low`](Self::low): the rows' tests hold each range finite and increasing.
    #[must_use]
    pub fn at(&self, temperature: Kelvin) -> f64 {
        if temperature.value().is_nan() {
            return f64::NAN;
        }
        let t = temperature
            .value()
            .clamp(self.low().value(), self.high().value());
        let piece = self
            .pieces
            .iter()
            .find(|piece| t <= piece.high.value())
            .or_else(|| self.pieces.last())
            .expect("every column has a piece, as the rows' tests hold");
        piece.form.evaluate(t)
    }
}

/// A row's thermal columns (the [module documentation](self)): its solid's specific heat and
/// conductivity, each value named with its sources in [`sources`](Self::sources).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalColumns {
    heat_capacity: ThermalColumn,
    conductivity: ThermalColumn,
    sources: &'static [Source],
}

impl ThermalColumns {
    /// The solid's specific heat at `temperature`, J kg⁻¹ K⁻¹, its end value held outside its
    /// range ([`ThermalColumn::at`]).
    #[must_use]
    pub fn heat_capacity_j_per_kg_k(&self, temperature: Kelvin) -> f64 {
        self.heat_capacity.at(temperature)
    }

    /// The dense solid's thermal conductivity at `temperature`, W m⁻¹ K⁻¹, its end value held
    /// outside its range ([`ThermalColumn::at`]).
    #[must_use]
    pub fn conductivity_w_per_m_k(&self, temperature: Kelvin) -> f64 {
        self.conductivity.at(temperature)
    }

    /// The specific heat's column, J kg⁻¹ K⁻¹.
    #[must_use]
    pub const fn heat_capacity(&self) -> &ThermalColumn {
        &self.heat_capacity
    }

    /// The conductivity's column, W m⁻¹ K⁻¹.
    #[must_use]
    pub const fn conductivity(&self) -> &ThermalColumn {
        &self.conductivity
    }

    /// Where its values come from: each source's `covers` names `heat capacity` or
    /// `conductivity`.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }

    /// Whether either column stands in for the row's own figure.
    #[must_use]
    pub const fn is_stand_in(&self) -> bool {
        self.heat_capacity.is_stand_in() || self.conductivity.is_stand_in()
    }
}

impl Substance {
    /// Its thermal columns, for a row that can form a surface and whose figures were read (the
    /// [`thermal`](self) module's table); `None` where they are not modelled.
    ///
    /// # Examples
    ///
    /// Ice Ih stores less heat at Europa's night than at its melting point, and conducts better:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::Kelvin;
    ///
    /// # fn main() {
    /// #     assert!(example().is_some(), "every value the example reads is there");
    /// # }
    /// # fn example() -> Option<()> {
    /// let ice = by_key("H2O")?.substance().thermal_columns()?;
    /// let (night, thaw) = (Kelvin::new(90.0), Kelvin::new(270.0));
    /// assert!(ice.heat_capacity_j_per_kg_k(night) < ice.heat_capacity_j_per_kg_k(thaw));
    /// assert!(ice.conductivity_w_per_m_k(night) > ice.conductivity_w_per_m_k(thaw));
    /// # Some(())
    /// # }
    /// ```
    #[must_use]
    pub fn thermal_columns(&self) -> Option<&'static ThermalColumns> {
        rows::THERMAL_ROWS
            .iter()
            .find(|(key, _)| *key == self.key)
            .map(|(_, columns)| columns)
    }
}

/// Every row with thermal columns, in registry order.
pub fn rows() -> impl Iterator<Item = (SubstanceId, &'static ThermalColumns)> {
    ids().filter_map(|id| {
        id.substance()
            .thermal_columns()
            .map(|columns| (id, columns))
    })
}

/// Gundlach and Blum's f₁, the structure factor's scale, dimensionless: (5.18 ± 3.45) × 10⁻², their
/// fit f₁ exp(f₂ φ) to Chan and Tien 1973's simple, body-centred and face-centred cubic packings
/// (filling factors 0.52, 0.68 and 0.74; Gundlach and Blum 2012, Icarus 219, 618, arXiv:1111.0535,
/// eq. 18 and Fig. 14), which their 2013 paper applies at filling factors 0.1–0.6 (Icarus 223,
/// 479, arXiv:1212.3108, §2–§3).
pub(crate) const CONTACT_STRUCTURE_F1: f64 = 5.18e-2;

/// Gundlach and Blum's f₂, the structure factor's exponent, dimensionless: 5.26 ± 0.94 (Gundlach
/// and Blum 2012, eq. 18), as `CONTACT_STRUCTURE_F1`.
pub(crate) const CONTACT_STRUCTURE_F2: f64 = 5.26;

/// Gundlach and Blum's χ, the contact conduction's reduction for irregular, polydisperse grains,
/// dimensionless: 0.41 ± 0.02 (statistical) ± 0.10 (systematic), calibrated on the Apollo 11 and
/// 12 regolith's measured conductivity in vacuum (Cremers et al. 1971; Cremers and Birkebak 1971)
/// at a grain radius of 20 µm, a filling factor of 0.43 and a solid conductivity of 1.78 W m⁻¹ K⁻¹,
/// the mean of silica's 1.37 and the ordinary chondrites' 2.18, which give 0.50 and 0.31 alone
/// (Gundlach and Blum 2013, §4 and Table 2, read).
///
/// The calibration pairs χ with that solid conductivity, so a row's own `k_s` carries the
/// calibration's spread, about ±25 per cent, into the contact term.
pub(crate) const IRREGULAR_GRAIN_FACTOR: f64 = 0.41;

/// Gundlach and Blum's e₁, the photons' mean free path in the pores per grain radius at a filling
/// factor of one half, dimensionless: 1.34 ± 0.01, their fit of Dullien 1991's form e₁ (1 − φ) ÷ φ
/// to Skorov et al. 2011's computed paths (Gundlach and Blum 2012, eq. 13, read; kept in their
/// 2013 eq. 3 over a two-parameter refit, e₁ 1.66 ± 0.21 with χ 0.37).
pub(crate) const PHOTON_PATH_FACTOR: f64 = 1.34;

/// How a grain material's specific surface energy γ follows temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceEnergy {
    /// γ = d₁ T, J m⁻²: zero at 0 K, as Gundlach and Blum assume for silica (2012, eq. 11).
    ProportionalToTemperature {
        /// d₁, J m⁻² K⁻¹.
        j_per_m2_k: f64,
    },
    /// One value at every temperature, J m⁻².
    Constant {
        /// γ, J m⁻².
        j_per_m2: f64,
    },
}

impl SurfaceEnergy {
    /// γ at `temperature_k` kelvin, J m⁻².
    #[must_use]
    pub fn j_per_m2_at(self, temperature_k: f64) -> f64 {
        match self {
            Self::ProportionalToTemperature { j_per_m2_k } => j_per_m2_k * temperature_k,
            Self::Constant { j_per_m2 } => j_per_m2,
        }
    }
}

/// The grain material's constants of the contact term, by the theory of Johnson, Kendall and
/// Roberts (1971): a regolith form's, by family, each cited in its form's sources. They are not
/// per-row columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactConstants {
    youngs_modulus: Pascals,
    poisson_ratio: f64,
    surface_energy: SurfaceEnergy,
}

impl ContactConstants {
    /// The grains' Young's modulus E.
    #[must_use]
    pub const fn youngs_modulus(&self) -> Pascals {
        self.youngs_modulus
    }

    /// The grains' Poisson's ratio ν.
    #[must_use]
    pub const fn poisson_ratio(&self) -> f64 {
        self.poisson_ratio
    }

    /// The grains' specific surface energy γ(T).
    #[must_use]
    pub const fn surface_energy(&self) -> SurfaceEnergy {
        self.surface_energy
    }
}

/// A family's grain material: its contact constants and its emissivity, with their sources.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FamilyMaterial {
    contact: ContactConstants,
    sources: &'static [Source],
}

/// A family's reference layer: where its porosity and grain size come from.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ReferenceLayer {
    /// R10's regolith form of the family ([`form_row`]): its shear table's narrowest layer from
    /// the surface that holds `depth`, whose porosity it takes, with a grain diameter of its own.
    FormRow {
        depth: Metres,
        grain_diameter: Metres,
    },
    /// A porosity and a grain diameter from the family's own sources.
    Fixed {
        porosity: f64,
        grain_diameter: Metres,
    },
}

/// The depth at which the silicate form reads R10's regolith of silicate: 7 cm, the deep end of
/// the Moon's diurnal skin depth, the "4–7 cm" over which Hayne et al. 2017 fit their 55 ± 2 J m⁻²
/// K⁻¹ s^−½ (JGR Planets 122, 2371, abstract). The narrowest of the Lunar Sourcebook's layers
/// from the surface that holds it, at either end of 4–7 cm, is its 0–15 cm layer.
pub(crate) const LUNAR_DIURNAL_SKIN_DEPTH: Metres = Metres::new(0.07);

/// The lunar soil's grain diameter, m: 70 µm, the Lunar Sourcebook's average, its median spanning
/// 40–130 µm (Carrier, Olhoeft and Mendell 1991, ch. 9, p. 478).
pub(crate) const LUNAR_SOIL_GRAIN_DIAMETER: Metres = Metres::new(70e-6);

/// The silicate grains' Young's modulus, Pa: (7.8 ± 1.9) × 10¹⁰, a basaltic rock's, as Gundlach
/// and Blum 2013 take it (Table 1, "Schultz, 1995", not re-read).
const SILICATE_YOUNGS_MODULUS_PA: f64 = 7.8e10;

/// The silicate grains' Poisson's ratio: 0.25, a basaltic rock's (Gundlach and Blum 2013, Table 1,
/// "Schultz, 1995", not re-read).
const SILICATE_POISSON_RATIO: f64 = 0.25;

/// The silicate grains' specific surface energy per kelvin, J m⁻² K⁻¹: d₁ = 6.67 × 10⁻⁵, so that
/// γ = d₁ T is zero at 0 K and silica's measured 0.02 J m⁻² at 300 K (Kendall et al. 1987, Heim
/// et al. 1999 and Gundlach et al. 2011, as Gundlach and Blum 2012 cite them, eq. 11, read), an
/// assumption there; Gundlach and Blum 2013 take it for every silicate (Table 1).
const SILICATE_SURFACE_ENERGY_J_PER_M2_K: f64 = 6.67e-5;

/// The emissivity's source, for every family's form.
const EMISSIVITY: Source = source(
    "emissivity",
    "`planetary::hooks::surface::REGOLITH_EMISSIVITY`, 0.95, the lunar regolith's (Hayne et al. \
     2017, JGR Planets 122, 2371, Table A1), read by path: a per-row infrared emissivity is not \
     modelled",
    Basis::Fact,
);

/// The silicate form's grain material and layer.
const SILICATE_MATERIAL: FamilyMaterial = FamilyMaterial {
    contact: ContactConstants {
        youngs_modulus: Pascals::new(SILICATE_YOUNGS_MODULUS_PA),
        poisson_ratio: SILICATE_POISSON_RATIO,
        surface_energy: SurfaceEnergy::ProportionalToTemperature {
            j_per_m2_k: SILICATE_SURFACE_ENERGY_J_PER_M2_K,
        },
    },
    sources: &[
        source(
            "porosity",
            "R10's regolith of silicate (`hyperion_surface::material::form_row`), read through its \
             public API: the narrowest layer of its shear table from the surface that holds the \
             Moon's diurnal skin depth, 7 cm (Hayne et al. 2017's 4–7 cm), the 0–15 cm layer, \
             n = 1 − 1.50 ÷ 3.1 (Carrier, Olhoeft and Mendell 1991, Lunar Sourcebook, ch. 9, Table \
             9.4, p. 484, and p. 492)",
            Basis::Reduced,
        ),
        source(
            "grain diameter",
            "70 µm: \"The median particle size is 40 to 130 µm, with an average of 70 µm\" \
             (Carrier, Olhoeft and Mendell 1991, Lunar Sourcebook, ch. 9, p. 478); Gundlach and \
             Blum 2013 find a grain radius of 48 (+115/−27) µm for the Moon from its inertia (§6)",
            Basis::Fact,
        ),
        EMISSIVITY,
        source(
            "Young's modulus, Poisson's ratio",
            "a basaltic rock's, E (7.8 ± 1.9) × 10¹⁰ Pa and ν 0.25, as Gundlach and Blum 2013 \
             (Icarus 223, 479, arXiv:1212.3108, Table 1) take them from Schultz 1995, not re-read",
            Basis::Fact,
        ),
        source(
            "surface energy",
            "γ = 6.67 × 10⁻⁵ J m⁻² K⁻¹ × T, zero at 0 K and silica's measured 0.02 J m⁻² at 300 K \
             (Gundlach and Blum 2012, Icarus 219, 618, arXiv:1111.0535, eq. 11, an assumption \
             there), which Gundlach and Blum 2013 take for every silicate (Table 1)",
            Basis::Adopted,
        ),
    ],
};

/// Water ice's grains' Young's modulus, Pa: 9.332 × 10⁹, an isotropic polycrystal's, averaged from
/// single crystals' adiabatic elastic constants by Brillouin spectroscopy at −16 °C, "E = 93.32 ×
/// 10⁸ N m⁻²" (Gammon, Kiefte, Clouter and Denner 1983, J. Glaciol. 29, 433, Table VIII, read),
/// held at every temperature: Gundlach and Blum 2015 take 7 × 10⁹ Pa and Ferrari and Lucas 2016
/// 9 × 10⁹.
const ICE_YOUNGS_MODULUS_PA: f64 = 9.332e9;

/// Water ice's grains' Poisson's ratio: 0.325, Gammon et al. 1983's isotropic polycrystal average,
/// 0.32521 at −16 °C (Table VIII, read).
const ICE_POISSON_RATIO: f64 = 0.325;

/// Water ice's grains' specific surface energy, J m⁻²: 0.19, from the rolling friction of
/// micrometre ice grains at 189–226 K, "γice = 0.190 J m−2" (Gundlach, Kilias, Beitz and Blum
/// 2011, Icarus 214, 717, arXiv:1102.0430, abstract, read), ± 0.04 within a literature span of
/// 0.07–0.37 (Gundlach and Blum 2015, ApJ 798, 34, arXiv:1410.7199, §3.2, read), held at every
/// temperature as they hold it.
const ICE_SURFACE_ENERGY_J_PER_M2: f64 = 0.19;

/// The icy regolith's porosity: 0.64 ± 0.08, Europa's globally averaged at ALMA's 1.25 mm, its band
/// within the diurnal wave's top few centimetres (Thelen et al. 2024, arXiv:2402.01952, Table 3,
/// read).
pub(crate) const ICY_REGOLITH_POROSITY: f64 = 0.64;

/// The icy regolith's grain diameter, m: 100 µm, Thelen et al. 2024's "intermediate surface grain
/// size of 100 µm" (§3, read; tested over 50 µm–1 mm, a radius or a diameter not stated, taken
/// as a diameter).
pub(crate) const ICY_REGOLITH_GRAIN_DIAMETER: Metres = Metres::new(100e-6);

/// The water-ice form's grain material.
const WATER_ICE_MATERIAL: FamilyMaterial = FamilyMaterial {
    contact: ContactConstants {
        youngs_modulus: Pascals::new(ICE_YOUNGS_MODULUS_PA),
        poisson_ratio: ICE_POISSON_RATIO,
        surface_energy: SurfaceEnergy::Constant {
            j_per_m2: ICE_SURFACE_ENERGY_J_PER_M2,
        },
    },
    sources: &[
        source(
            "porosity",
            "an icy satellite's regolith, Europa's, at the top few centimetres the diurnal wave \
             samples: 0.64 ± 0.08, the global weighted average of ALMA's 1.25 mm band (Thelen, de \
             Kleer, Camarca, Akins, Gurwell, Butler and de Pater 2024, arXiv:2402.01952, Table 3; \
             its 3.05 and 0.87 mm bands 0.50 (+0.20/−0.10) and 0.52 ± 0.07, its hemispheres \
             0.75–0.50 leading and 0.50–0.40 trailing, abstract), which the infrared bound agrees \
             with: at least 0.61 ± 0.1 in the upper centimetres from Galileo PPR, about 5 per \
             cent more for grains under 1 mm (Lange et al. 2026, arXiv:2604.14374, abstract and \
             §3.3). Ferrari and Lucas 2016 (A&A 588, A133, §3.3) find that crystalline grains in \
             tight contact give the Saturnian moons' inertias under 60 only above 0.8. The band's \
             choice is HYPERION's; no Saturnian moon's porosity was read",
            Basis::Adopted,
        ),
        source(
            "grain diameter",
            "100 µm, Thelen et al. 2024's \"intermediate surface grain size of 100 µm\" for \
             Europa, tested over 50 µm–1 mm (§3), a diameter by assumption; Europa's \"∼50-100 \
             µm\" (Lange et al. 2026, §2.4, after Hansen and McCord 2004), and Enceladus's \
             15 ± 5 to 30 ± 10 µm by Cassini VIMS (Jaumann et al. 2008, Icarus 193, 407), \
             recorded",
            Basis::Adopted,
        ),
        EMISSIVITY,
        source(
            "Young's modulus, Poisson's ratio",
            "an isotropic polycrystal's average of single crystals' adiabatic elastic constants \
             by Brillouin spectroscopy at −16 °C, E 93.32 × 10⁸ N m⁻² and ν 0.32521 (Gammon, \
             Kiefte, Clouter and Denner 1983, J. Glaciol. 29, 433, Table VIII), held at every \
             temperature",
            Basis::Fact,
        ),
        source(
            "surface energy",
            "0.19 J m⁻², from the rolling friction of micrometre ice grains at 189–226 K \
             (Gundlach, Kilias, Beitz and Blum 2011, Icarus 214, 717, arXiv:1102.0430, abstract), \
             ± 0.04 within the literature's 0.07–0.37 (Gundlach and Blum 2015, ApJ 798, 34, \
             arXiv:1410.7199, §3.2), held at every temperature as they hold it",
            Basis::Fact,
        ),
    ],
};

/// The grain material and reference layer of `family`'s form, `None` for a family with none.
#[must_use]
const fn family_form(family: MechanicsFamily) -> Option<(FamilyMaterial, ReferenceLayer)> {
    match family {
        MechanicsFamily::Silicate => Some((
            SILICATE_MATERIAL,
            ReferenceLayer::FormRow {
                depth: LUNAR_DIURNAL_SKIN_DEPTH,
                grain_diameter: LUNAR_SOIL_GRAIN_DIAMETER,
            },
        )),
        MechanicsFamily::WaterIce => Some((
            WATER_ICE_MATERIAL,
            ReferenceLayer::Fixed {
                porosity: ICY_REGOLITH_POROSITY,
                grain_diameter: ICY_REGOLITH_GRAIN_DIAMETER,
            },
        )),
        MechanicsFamily::Metal
        | MechanicsFamily::VolatileIce
        | MechanicsFamily::Salt
        | MechanicsFamily::Organic => None,
    }
}

/// A regolith form: how a substance's grains lie in a regolith, its porosity and grain size, with
/// the grains' emissivity and the granular model's material constants by family.
///
/// [`RegolithForm::reference`] gives each family's reference form; [`RegolithForm::new`] a
/// layer of the caller's own porosity and grain size, as rendering plan R09's R09.T13.c passes its
/// layers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegolithForm {
    family: MechanicsFamily,
    porosity: f64,
    grain_diameter: Metres,
    emissivity: f64,
    contact: ContactConstants,
    sources: &'static [Source],
}

/// Why a [`RegolithForm`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildRegolithFormError {
    /// The family has no regolith form: no cited grain material constants.
    NoForm {
        /// The family asked for.
        family: MechanicsFamily,
    },
    /// The porosity is not above 0 and below 1.
    Porosity {
        /// The value given.
        porosity: f64,
    },
    /// The grain diameter is not positive and finite.
    GrainDiameter {
        /// The value given.
        grain_diameter: Metres,
    },
}

impl fmt::Display for BuildRegolithFormError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoForm { family } => write!(f, "the {family:?} family has no regolith form"),
            Self::Porosity { porosity } => {
                write!(f, "a porosity of {porosity} is not between 0 and 1")
            }
            Self::GrainDiameter { grain_diameter } => write!(
                f,
                "a grain diameter of {} m is not positive and finite",
                grain_diameter.value()
            ),
        }
    }
}

impl Error for BuildRegolithFormError {}

impl RegolithForm {
    /// The reference form of `family`'s regolith, `None` for a family with none:
    /// [`MechanicsFamily::Metal`], [`MechanicsFamily::VolatileIce`], [`MechanicsFamily::Salt`] and
    /// [`MechanicsFamily::Organic`], none with cited grain-contact constants.
    ///
    /// The silicate form takes the porosity of R10's regolith of silicate, the Lunar
    /// Sourcebook's, through [`form_row`], so that the two cannot disagree.
    ///
    /// # Panics
    ///
    /// Never: R10's regolith of silicate has a layer from the surface that holds the Moon's
    /// diurnal skin depth, which `substance_thermal_silicate_form_is_r10_s_regolith` holds.
    ///
    /// # Examples
    ///
    /// The Moon's soil is about half pore space:
    ///
    /// ```
    /// use hyperion_sim::substance::RegolithForm;
    /// use hyperion_surface::field::MechanicsFamily;
    ///
    /// # fn main() {
    /// #     assert!(example().is_some(), "every value the example reads is there");
    /// # }
    /// # fn example() -> Option<()> {
    /// let lunar = RegolithForm::reference(MechanicsFamily::Silicate)?;
    /// assert!((lunar.porosity() - (1.0 - 1.50 / 3.1)).abs() < 1e-12);
    /// assert!(RegolithForm::reference(MechanicsFamily::Metal).is_none());
    /// # Some(())
    /// # }
    /// ```
    #[must_use]
    pub fn reference(family: MechanicsFamily) -> Option<Self> {
        let (material, layer) = family_form(family)?;
        let (porosity, grain_diameter) = match layer {
            ReferenceLayer::FormRow {
                depth,
                grain_diameter,
            } => (
                narrowest_surface_layer(form_row(MaterialClass::Regolith, family).shear(), depth)
                    .expect(
                        "R10's regolith of silicate has a surface layer holding the skin depth, as \
                         substance_thermal_silicate_form_is_r10_s_regolith holds",
                    )
                    .porosity(),
                grain_diameter,
            ),
            ReferenceLayer::Fixed {
                porosity,
                grain_diameter,
            } => (porosity, grain_diameter),
        };
        Some(Self {
            family,
            porosity,
            grain_diameter,
            emissivity: REGOLITH_EMISSIVITY,
            contact: material.contact,
            sources: material.sources,
        })
    }

    /// A layer of `family`'s grain material at `porosity` (the void share of its volume, above 0
    /// and below 1) and `grain_diameter`, with its family's contact constants and emissivity.
    ///
    /// # Errors
    ///
    /// [`BuildRegolithFormError::NoForm`] for a family without a form,
    /// [`BuildRegolithFormError::Porosity`] for a porosity that is not above 0 and below 1, and
    /// [`BuildRegolithFormError::GrainDiameter`] for a grain diameter that is not positive and
    /// finite.
    pub fn new(
        family: MechanicsFamily,
        porosity: f64,
        grain_diameter: Metres,
    ) -> Result<Self, BuildRegolithFormError> {
        let reference = Self::reference(family).ok_or(BuildRegolithFormError::NoForm { family })?;
        if !(porosity > 0.0 && porosity < 1.0) {
            return Err(BuildRegolithFormError::Porosity { porosity });
        }
        let diameter = grain_diameter.value();
        if !(diameter.is_finite() && diameter > 0.0) {
            return Err(BuildRegolithFormError::GrainDiameter { grain_diameter });
        }
        Ok(Self {
            porosity,
            grain_diameter,
            ..reference
        })
    }

    /// The family of the grains.
    #[must_use]
    pub const fn family(&self) -> MechanicsFamily {
        self.family
    }

    /// The porosity ψ, the void share of the layer's volume.
    #[must_use]
    pub const fn porosity(&self) -> f64 {
        self.porosity
    }

    /// The grains' diameter.
    #[must_use]
    pub const fn grain_diameter(&self) -> Metres {
        self.grain_diameter
    }

    /// The grains' infrared emissivity ε: [`REGOLITH_EMISSIVITY`], the lunar regolith's, for every
    /// family, a per-row emissivity being not modelled.
    #[must_use]
    pub const fn emissivity(&self) -> f64 {
        self.emissivity
    }

    /// The grain material's contact constants.
    #[must_use]
    pub const fn contact(&self) -> ContactConstants {
        self.contact
    }

    /// Where the reference form's figures come from: each source's `covers` names `porosity`,
    /// `grain diameter`, `emissivity`, `Young's modulus`, `Poisson's ratio` or `surface energy`. A
    /// form built by [`new`](Self::new) keeps them, its porosity and grain size being its caller's.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }
}

/// The narrowest layer of `shear` from the surface (its top at 0 m) that holds `depth`: R10's
/// rule for the layer at a depth (`MaterialProperties::shear_layer_at`), among the layers that
/// reach the surface.
#[must_use]
fn narrowest_surface_layer(shear: &[ShearLayer], depth: Metres) -> Option<ShearLayer> {
    shear
        .iter()
        .filter(|layer| layer.top().value() <= 0.0 && depth.value() < layer.bottom().value())
        .min_by(|a, b| a.bottom().total_cmp(&b.bottom()))
        .copied()
}

/// A regolith's thermal inertia, with its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegolithInertia {
    inertia_j_per_m2_k_sqrt_s: f64,
    conductivity_w_per_m_k: f64,
    contact_conductivity_w_per_m_k: f64,
    radiative_conductivity_w_per_m_k: f64,
    solid_conductivity_w_per_m_k: f64,
    grain_density: KilogramsPerCubicMetre,
    bulk_density: KilogramsPerCubicMetre,
    heat_capacity_j_per_kg_k: f64,
}

impl RegolithInertia {
    /// Γ = √(`k_eff` `ρ_bulk` c), J m⁻² K⁻¹ s^−½.
    #[must_use]
    pub const fn inertia_j_per_m2_k_sqrt_s(&self) -> f64 {
        self.inertia_j_per_m2_k_sqrt_s
    }

    /// `k_eff`, the regolith's conductivity, W m⁻¹ K⁻¹: the contact term plus the radiative.
    #[must_use]
    pub const fn conductivity_w_per_m_k(&self) -> f64 {
        self.conductivity_w_per_m_k
    }

    /// The conduction through the grains' contacts, W m⁻¹ K⁻¹.
    #[must_use]
    pub const fn contact_conductivity_w_per_m_k(&self) -> f64 {
        self.contact_conductivity_w_per_m_k
    }

    /// The radiation across the pores, W m⁻¹ K⁻¹.
    #[must_use]
    pub const fn radiative_conductivity_w_per_m_k(&self) -> f64 {
        self.radiative_conductivity_w_per_m_k
    }

    /// The dense solid's conductivity `k_s`, W m⁻¹ K⁻¹, the row's column at the temperature.
    #[must_use]
    pub const fn solid_conductivity_w_per_m_k(&self) -> f64 {
        self.solid_conductivity_w_per_m_k
    }

    /// The solid's grain density `ρ_g` at the temperature.
    #[must_use]
    pub const fn grain_density(&self) -> KilogramsPerCubicMetre {
        self.grain_density
    }

    /// `ρ_bulk` = `ρ_g` (1 − ψ).
    #[must_use]
    pub const fn bulk_density(&self) -> KilogramsPerCubicMetre {
        self.bulk_density
    }

    /// c, the solid's specific heat at the temperature, J kg⁻¹ K⁻¹.
    #[must_use]
    pub const fn heat_capacity_j_per_kg_k(&self) -> f64 {
        self.heat_capacity_j_per_kg_k
    }

    /// The dense solid's own inertia √(`k_s` `ρ_g` c), J m⁻² K⁻¹ s^−½: bedrock's, which the
    /// regolith's is below at every porosity.
    #[must_use]
    pub fn solid_inertia_j_per_m2_k_sqrt_s(&self) -> f64 {
        (self.solid_conductivity_w_per_m_k
            * self.grain_density.value()
            * self.heat_capacity_j_per_kg_k)
            .sqrt()
    }
}

/// Why a regolith's thermal inertia could not be evaluated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvaluateRegolithInertiaError {
    /// The temperature is not positive and finite.
    Temperature {
        /// The value given.
        temperature: Kelvin,
    },
    /// The row has no condensed columns: a gas, or a row whose condensed figures were not read.
    NotCondensed {
        /// The row.
        id: SubstanceId,
    },
    /// The row has no solid at the temperature: it is above its transition, or its solid's density
    /// there was not read.
    NoSolid {
        /// The row.
        id: SubstanceId,
        /// The temperature asked for.
        temperature: Kelvin,
    },
    /// The row has no thermal columns: not modelled.
    NoThermalColumns {
        /// The row.
        id: SubstanceId,
    },
    /// The row's family has no regolith form ([`reference_regolith_thermal_inertia`]).
    NoRegolithForm {
        /// The row's family.
        family: MechanicsFamily,
    },
    /// The form is of another family than the row's, whose contact constants are not its grains'.
    FamilyMismatch {
        /// The row's family.
        row: MechanicsFamily,
        /// The form's family.
        form: MechanicsFamily,
    },
}

impl fmt::Display for EvaluateRegolithInertiaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temperature { temperature } => {
                write!(f, "{} K is not a temperature", temperature.value())
            }
            Self::NotCondensed { id } => {
                write!(f, "{} has no condensed columns", id.substance().key())
            }
            Self::NoSolid { id, temperature } => write!(
                f,
                "{} has no solid at {} K",
                id.substance().key(),
                temperature.value()
            ),
            Self::NoThermalColumns { id } => {
                write!(f, "{} has no thermal columns", id.substance().key())
            }
            Self::NoRegolithForm { family } => {
                write!(f, "the {family:?} family has no regolith form")
            }
            Self::FamilyMismatch { row, form } => {
                write!(f, "a {form:?} form cannot hold a {row:?} row's grains")
            }
        }
    }
}

impl Error for EvaluateRegolithInertiaError {}

/// The thermal inertia of the substance `id` as a dry regolith in vacuum of `form`, at
/// `temperature` (the [module documentation](self)).
///
/// # Errors
///
/// [`EvaluateRegolithInertiaError::Temperature`] for a temperature that is not positive and finite;
/// [`NotCondensed`](EvaluateRegolithInertiaError::NotCondensed) for a row without condensed
/// columns; [`FamilyMismatch`](EvaluateRegolithInertiaError::FamilyMismatch) for a form of another
/// family than the row's; [`NoThermalColumns`](EvaluateRegolithInertiaError::NoThermalColumns)
/// for a row without thermal columns; and [`NoSolid`](EvaluateRegolithInertiaError::NoSolid)
/// above the row's transition, or where its solid's density was not read.
///
/// # Examples
///
/// The Moon's highland soil at noon of an equatorial day stores tens of SI units, bedrock
/// thousands:
///
/// ```
/// use hyperion_sim::substance::{RegolithForm, by_key, regolith_thermal_inertia};
/// use hyperion_sim::units::Kelvin;
/// use hyperion_surface::field::MechanicsFamily;
///
/// let highlands = by_key("feldspathic").ok_or("the registry holds the highlands")?;
/// let soil = RegolithForm::reference(MechanicsFamily::Silicate).ok_or("silicate has a form")?;
/// let gamma = regolith_thermal_inertia(highlands, &soil, Kelvin::new(273.15))?;
/// assert!((30.0..100.0).contains(&gamma.inertia_j_per_m2_k_sqrt_s()));
/// assert!(gamma.solid_inertia_j_per_m2_k_sqrt_s() > 1_000.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn regolith_thermal_inertia(
    id: SubstanceId,
    form: &RegolithForm,
    temperature: Kelvin,
) -> Result<RegolithInertia, EvaluateRegolithInertiaError> {
    let t = temperature.value();
    if !(t.is_finite() && t > 0.0) {
        return Err(EvaluateRegolithInertiaError::Temperature { temperature });
    }
    let row = id.substance();
    let condensed = row
        .condensed()
        .ok_or(EvaluateRegolithInertiaError::NotCondensed { id })?;
    if condensed.mechanics() != form.family {
        return Err(EvaluateRegolithInertiaError::FamilyMismatch {
            row: condensed.mechanics(),
            form: form.family,
        });
    }
    let columns = row
        .thermal_columns()
        .ok_or(EvaluateRegolithInertiaError::NoThermalColumns { id })?;
    let no_solid = EvaluateRegolithInertiaError::NoSolid { id, temperature };
    if t > condensed.transition().temperature().value() {
        return Err(no_solid);
    }
    let grain_density = row
        .grain_density(CondensedPhase::Solid, temperature)
        .ok_or(no_solid)?;
    let bulk_density = KilogramsPerCubicMetre::new(grain_density.value() * (1.0 - form.porosity));
    let heat_capacity = columns.heat_capacity_j_per_kg_k(temperature);
    let solid = columns.conductivity_w_per_m_k(temperature);
    let filling = 1.0 - form.porosity;
    let radius = form.grain_diameter.value() / 2.0;
    let contact = form.contact;
    let nu = contact.poisson_ratio;
    let hertz = math::cbrt(
        9.0 * PI / 4.0 * (1.0 - nu * nu) / contact.youngs_modulus.value()
            * contact.surface_energy.j_per_m2_at(t)
            / radius,
    );
    let contact_conductivity = solid
        * hertz
        * (CONTACT_STRUCTURE_F1 * math::exp(CONTACT_STRUCTURE_F2 * filling))
        * IRREGULAR_GRAIN_FACTOR;
    let radiative_conductivity = 8.0
        * STEFAN_BOLTZMANN
        * form.emissivity
        * (t * t * t)
        * (PHOTON_PATH_FACTOR * (1.0 - filling) / filling * radius);
    let conductivity = contact_conductivity + radiative_conductivity;
    Ok(RegolithInertia {
        inertia_j_per_m2_k_sqrt_s: (conductivity * bulk_density.value() * heat_capacity).sqrt(),
        conductivity_w_per_m_k: conductivity,
        contact_conductivity_w_per_m_k: contact_conductivity,
        radiative_conductivity_w_per_m_k: radiative_conductivity,
        solid_conductivity_w_per_m_k: solid,
        grain_density,
        bulk_density,
        heat_capacity_j_per_kg_k: heat_capacity,
    })
}

/// The thermal inertia of the substance `id` as a dry regolith in vacuum of its family's
/// reference form ([`RegolithForm::reference`]) at `temperature`: the law at a crust's lid, as
/// P14.T54.a passes it to P14.T24.a.
///
/// # Errors
///
/// [`EvaluateRegolithInertiaError::NotCondensed`] for a row without condensed columns,
/// [`EvaluateRegolithInertiaError::NoRegolithForm`] for a family without a form, and otherwise
/// [`regolith_thermal_inertia`]'s.
pub fn reference_regolith_thermal_inertia(
    id: SubstanceId,
    temperature: Kelvin,
) -> Result<RegolithInertia, EvaluateRegolithInertiaError> {
    let family = id
        .substance()
        .condensed()
        .ok_or(EvaluateRegolithInertiaError::NotCondensed { id })?
        .mechanics();
    let form = RegolithForm::reference(family)
        .ok_or(EvaluateRegolithInertiaError::NoRegolithForm { family })?;
    regolith_thermal_inertia(id, &form, temperature)
}

/// A source of the values `covers` names, for the rows' and the forms' tables.
#[must_use]
const fn source(covers: &'static str, citation: &'static str, basis: Basis) -> Source {
    Source {
        covers,
        citation,
        basis,
    }
}
