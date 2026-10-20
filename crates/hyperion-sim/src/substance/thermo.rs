//! The gases' heat capacity, enthalpy and entropy by temperature: a statistical sum over each gas
//! row's molecular constants (P14.T49.b's first step; science-r08-gases-2.md §1.3–§1.4).
//!
//! No fixed heat capacity serves the galaxy's atmospheres, which run from Triton's 38 K to
//! near-runaway worlds at 700–1,500 K: equipartition holds only for the noble gases at every
//! temperature, and, within 1 per cent, for N₂ and CO below about 400 K and O₂ below about 300 K.
//! Every gas row therefore carries its [`MolecularConstants`], and its `c_p` ÷ R at a temperature T
//! is the ideal gas's
//!
//! ```text
//! c_p ÷ R = 5/2 + c_rot + Σᵢ gᵢ E(θᵢ ÷ T) + Σ (level sets)
//! ```
//!
//! - 5/2 is the translation's, with the 1 of `c_p` − `c_v`;
//! - `c_rot` is the classical rotor's, 0 for an atom, 1 for a linear molecule and 3/2 for a
//!   non-linear one ([`Rotation`]), which holds where T ≥ 5 `θ_rot`: only H₂ fails that inside
//!   30–2,000 K, so its rotation is a level set;
//! - E(x) = x² e⁻ˣ ÷ (1 − e⁻ˣ)² is a harmonic oscillator's (Einstein's) term, over each harmonic
//!   fundamental νᵢ of degeneracy gᵢ ([`Vibration`]), with θᵢ = c₂ νᵢ and c₂ = hc ÷ `k_B`
//!   ([`SECOND_RADIATION_CONSTANT_CM_K`]);
//! - a level set ([`LevelSet`]) adds (⟨E²⟩ − ⟨E⟩²) ÷ (`k_B` T)² over levels in Boltzmann
//!   equilibrium among themselves: low electronic states, a hindered internal rotor's torsional
//!   levels, or H₂'s rovibrational ladder for one nuclear-spin modification.
//!
//! Above 1,500 K, the warmest temperature its reference test holds it to NASA Glenn's at
//! (`the_heat_capacities_follow_nasa_glenn`), the sum is an extrapolation of its harmonic
//! oscillators, which leave out anharmonicity: at 2,879 K, a magma ocean's air, it lies below NASA
//! Glenn's by 1.6% for CO₂, 4.7% for H₂O and 1.1% for N₂ (plan 14's Risks, deferred).
//!
//! The enthalpy H̃ = ∫ `c_p` ÷ R dT (kelvin, zero at 0 K) and the entropy S̃ = ∫ `c_p` ÷ (R T) dT
//! have closed forms term by term: c T and c ln T for the constant parts, θ ÷ (eˣ − 1) and
//! x ÷ (eˣ − 1) − ln(1 − e⁻ˣ) for each oscillator, and ⟨E⟩ ÷ `k_B` and ln Z + ⟨E⟩ ÷ (`k_B` T) for
//! each level set. S̃'s zero is arbitrary, and only its differences are read: P14.T24.e's adiabat
//! solves S̃(`T_ad`) = S̃(`T_s`) + α ln(p ÷ `p_s`). A mixture's three are Σ xᵢ of its rows', summed in
//! its given order as [`mix`](super::mix) sums, with no entropy of mixing, which is constant at a
//! fixed composition.
//!
//! Each term is evaluated with the crate's deterministic elementary functions in a fixed order
//! and over a fixed number of levels, so a value is the same on every target (sim-determinism).
//!
//! NASA Glenn's polynomials (McBride, Zehe and Gordon 2002, `NASA/TP-2002-211556`; public domain)
//! are the tests' oracle from 200 K up, not a source; JANAF's and Shomate's fits are not sources
//! either, since they do not reach the cold worlds and decision-composition §5 allows NIST's
//! Standard Reference Data only as asserted cross-checks.
//!
//! P14.T24.a's surface conditions take them since P14.T54.a's switch: their published `c_p` ÷ R
//! and their column's specific heat are [`mixture_heat_capacity_over_r_at`] at the mean surface
//! temperature, in place of the kinetic-theory classes rows 0–8 held until then. P14.T24.e's
//! vertical structure integrates its adiabat on them.

use super::SubstanceId;
use super::mix::mixed;
use crate::math;
use crate::units::Kelvin;

/// The second radiation constant c₂ = hc ÷ `k_B`, cm K: 1.438 776 877 503 9 (CODATA 2018, Tiesinga
/// et al. 2021, Rev. Mod. Phys. 93, 025010; exact, from the defined h, c and `k_B`). A level or a
/// fundamental of ν cm⁻¹ lies c₂ ν kelvin up.
pub const SECOND_RADIATION_CONSTANT_CM_K: f64 = 1.438_776_877_503_933;

/// The highest vibrational level of a [`LevelSet::Rovibrational`] ladder.
const LADDER_V_MAX: u32 = 5;

/// The highest rotational level of a [`LevelSet::Rovibrational`] ladder in each vibrational level:
/// below the quartic's turnover, J(J + 1) = `B_v` ÷ 2`D_v`, for every v ≤ [`LADDER_V_MAX`] of H₂
/// (lowest, 649, at v = 0). The levels it leaves out lie above about 17,000 cm⁻¹ and move H₂'s `c_p` by
/// under 10⁻³ at 2,000 K.
const LADDER_J_MAX: u32 = 20;

/// How a gas's molecules rotate: its classical rotational heat capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rotation {
    /// No rotation: an atom, an atomic ion or the electron.
    None,
    /// A linear molecule's two rotational degrees, `c_rot` = 1.
    Linear,
    /// A non-linear molecule's three, `c_rot` = 3/2.
    NonLinear,
    /// Rotation carried by a level set ([`LevelSet::Rovibrational`]): H₂'s, whose rotational
    /// temperature, 87.6 K, is too high for the classical rotor below about 440 K.
    InLevels,
}

impl Rotation {
    /// The rotation's classical `c_rot` ÷ R: 0, 1, 3/2, or 0 where a level set carries it.
    #[must_use]
    pub const fn heat_capacity_over_r(self) -> f64 {
        match self {
            Self::None | Self::InLevels => 0.0,
            Self::Linear => 1.0,
            Self::NonLinear => 1.5,
        }
    }
}

/// A harmonic vibration: a fundamental's wavenumber, cm⁻¹, and its degeneracy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vibration {
    pub(super) wavenumber_cm: f64,
    pub(super) degeneracy: u8,
}

impl Vibration {
    /// The fundamental's wavenumber, cm⁻¹.
    #[must_use]
    pub const fn wavenumber_cm(&self) -> f64 {
        self.wavenumber_cm
    }

    /// How many modes share it: 1, 2 for a doubly degenerate (e or π) mode, 3 for a triply
    /// degenerate (t or f) one.
    #[must_use]
    pub const fn degeneracy(&self) -> u8 {
        self.degeneracy
    }
}

/// Which levels of a homonuclear diatomic with nuclear spin ½ (H₂) a ladder holds, and their
/// nuclear-spin weights.
///
/// Ortho- and para-hydrogen convert only on a catalyst: uncatalysed conversion takes years, an
/// atmosphere's convective overturn days, so a parcel rising from depth carries the 3:1 of high
/// temperature (normal hydrogen, the adiabat's), and equilibrium hydrogen is the recorded other
/// bound (science-r08-gases-2.md §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NuclearSpin {
    /// Ortho-hydrogen alone: the odd rotational levels.
    Ortho,
    /// Para-hydrogen alone: the even rotational levels.
    Para,
    /// Both in equilibrium with each other, the odd levels at weight 3 and the even at 1.
    Equilibrium,
}

/// A diatomic's vibration–rotation term values, cm⁻¹, in the Dunham-type expansion of Huber and
/// Herzberg 1979: G(v) = `ω_e`(v + ½) − `ω_e x_e`(v + ½)² + `ω_e y_e`(v + ½)³ and
/// `F_v`(J) = `B_v` J(J + 1) − `D_v` J²(J + 1)², with `B_v` = `B_e` − `α_e`(v + ½) + `γ_e`(v + ½)²
/// and `D_v` = `D_e` + `β_e`(v + ½).
#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::struct_field_names,
    reason = "every term is a wavenumber, and each name carries its unit, cm⁻¹"
)]
pub struct RovibrationalTerms {
    pub(super) we_cm: f64,
    pub(super) wexe_cm: f64,
    pub(super) weye_cm: f64,
    pub(super) be_cm: f64,
    pub(super) ae_cm: f64,
    pub(super) ge_cm: f64,
    pub(super) de_cm: f64,
    pub(super) beta_e_cm: f64,
}

impl RovibrationalTerms {
    /// The level (v, J)'s term value, cm⁻¹ above the potential's minimum.
    #[must_use]
    pub fn term_cm(&self, v: u32, j: u32) -> f64 {
        let half = f64::from(v) + 0.5;
        let spin = f64::from(j) * (f64::from(j) + 1.0);
        let vibration = half * (self.we_cm + half * (-self.wexe_cm + half * self.weye_cm));
        let b_v = self.be_cm + half * (-self.ae_cm + half * self.ge_cm);
        let d_v = self.de_cm + half * self.beta_e_cm;
        vibration + spin * (b_v - d_v * spin)
    }
}

/// A level of a [`LevelSet::Listed`]: its energy above its set's lowest, cm⁻¹, and its
/// degeneracy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    pub(super) energy_cm: f64,
    pub(super) degeneracy: f64,
}

impl Level {
    /// Its energy above its set's lowest level, cm⁻¹.
    #[must_use]
    pub const fn energy_cm(&self) -> f64 {
        self.energy_cm
    }

    /// Its degeneracy: 2J + 1 for an atom's level, its spin–orbit and Λ-doubling's for a
    /// molecule's state, 1 or 2 for a torsional level.
    #[must_use]
    pub const fn degeneracy(&self) -> f64 {
        self.degeneracy
    }
}

/// Levels in Boltzmann equilibrium among themselves, which add their energy's variance to a gas's
/// heat capacity, weighted by the share of the gas they describe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LevelSet {
    /// Listed levels: low electronic states, or a hindered rotor's torsional levels.
    Listed {
        /// The share of the gas the set describes: 1 for every set but H₂'s modifications.
        weight: f64,
        /// The levels, the lowest first at 0.
        levels: &'static [Level],
    },
    /// A diatomic's rovibrational ladder, v ≤ 5 and J ≤ 20, for one nuclear-spin modification,
    /// its energies taken above the modification's own lowest level so that H̃(0 K) = 0.
    Rovibrational {
        /// The share of the gas the set describes: 3/4 for normal hydrogen's ortho-hydrogen.
        weight: f64,
        /// The term values.
        terms: &'static RovibrationalTerms,
        /// Which levels it holds.
        spin: NuclearSpin,
    },
}

/// A gas's three per R at one temperature: `c_p` ÷ R, H̃ ÷ R in kelvin and S̃ ÷ R.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Terms {
    c: f64,
    h: f64,
    s: f64,
}

impl Terms {
    const ZERO: Self = Self {
        c: 0.0,
        h: 0.0,
        s: 0.0,
    };

    /// `self` plus `scale` times `other`.
    #[must_use]
    fn add_scaled(self, scale: f64, other: Self) -> Self {
        Self {
            c: self.c + scale * other.c,
            h: self.h + scale * other.h,
            s: self.s + scale * other.s,
        }
    }
}

/// A harmonic oscillator's terms at `t` for θ = `theta` kelvin: E(x), θ ÷ (eˣ − 1) and
/// x ÷ (eˣ − 1) − ln(1 − e⁻ˣ), with x = θ ÷ T, written in e⁻ˣ so that a cold mode underflows to 0
/// rather than overflowing.
#[must_use]
fn oscillator(theta: f64, t: f64) -> Terms {
    let reduced = theta / t;
    let boltzmann = math::exp(-reduced);
    // A mode frozen out adds nothing: exactly 0 where e⁻ˣ underflows.
    if boltzmann == 0.0 {
        return Terms::ZERO;
    }
    // 1 − e⁻ˣ, exact for a hot mode, where e⁻ˣ nears 1.
    let unfilled = -math::exp_m1(-reduced);
    let occupancy = boltzmann / unfilled;
    Terms {
        c: reduced * reduced * occupancy / unfilled,
        h: theta * occupancy,
        s: reduced * occupancy - math::ln_1p(-boltzmann),
    }
}

/// The running Boltzmann sums of a level set: Z, Σ g x e⁻ˣ and Σ g x² e⁻ˣ.
#[derive(Debug, Clone, Copy, Default)]
struct Moments {
    z: f64,
    x1: f64,
    x2: f64,
}

impl Moments {
    fn add(&mut self, energy_cm: f64, degeneracy: f64, t: f64) {
        let x = SECOND_RADIATION_CONSTANT_CM_K * energy_cm / t;
        let w = degeneracy * math::exp(-x);
        // A level frozen out adds nothing, and skipping it keeps 0 × ∞ out of the sums when x
        // overflows at an extreme cold.
        if w == 0.0 {
            return;
        }
        self.z += w;
        self.x1 += w * x;
        self.x2 += w * x * x;
    }

    /// The set's terms: the variance of x, ⟨x⟩ T and ln Z + ⟨x⟩.
    #[must_use]
    fn terms(self, t: f64) -> Terms {
        let mean = self.x1 / self.z;
        Terms {
            c: self.x2 / self.z - mean * mean,
            h: mean * t,
            s: math::ln(self.z) + mean,
        }
    }
}

impl LevelSet {
    /// The set's weight.
    #[must_use]
    pub const fn weight(&self) -> f64 {
        match *self {
            Self::Listed { weight, .. } | Self::Rovibrational { weight, .. } => weight,
        }
    }

    /// The set's unweighted terms at `t`.
    #[must_use]
    fn terms(&self, t: f64) -> Terms {
        let mut m = Moments::default();
        match *self {
            Self::Listed { levels, .. } => {
                for level in levels {
                    m.add(level.energy_cm, level.degeneracy, t);
                }
            }
            Self::Rovibrational { terms, spin, .. } => {
                let lowest = terms.term_cm(0, u32::from(spin == NuclearSpin::Ortho));
                for v in 0..=LADDER_V_MAX {
                    for j in 0..=LADDER_J_MAX {
                        let odd = j % 2 == 1;
                        let weight = match (spin, odd) {
                            (NuclearSpin::Ortho, false) | (NuclearSpin::Para, true) => continue,
                            (NuclearSpin::Equilibrium, true) => 3.0,
                            (
                                NuclearSpin::Ortho | NuclearSpin::Para | NuclearSpin::Equilibrium,
                                _,
                            ) => 1.0,
                        };
                        m.add(
                            terms.term_cm(v, j) - lowest,
                            weight * f64::from(2 * j + 1),
                            t,
                        );
                    }
                }
            }
        }
        m.terms(t)
    }
}

/// A gas row's molecular constants: what its heat capacity, enthalpy and entropy are summed over.
///
/// # Examples
///
/// Carbon dioxide's bending mode wakes above about 100 K, so no fixed `c_p` serves both Mars and
/// Venus:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::heat_capacity_over_r_at;
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let co2 = Gas::CarbonDioxide.substance();
/// let cold = heat_capacity_over_r_at(co2, Kelvin::new(50.0))?;
/// let venus = heat_capacity_over_r_at(co2, Kelvin::new(735.0))?;
/// assert!((cold - 3.5).abs() < 1e-3);
/// assert!(venus > 6.0);
/// # Some(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MolecularConstants {
    pub(super) rotation: Rotation,
    pub(super) vibrations: &'static [Vibration],
    pub(super) levels: &'static [LevelSet],
}

impl MolecularConstants {
    /// How the molecule rotates.
    #[must_use]
    pub const fn rotation(&self) -> Rotation {
        self.rotation
    }

    /// Its harmonic fundamentals.
    #[must_use]
    pub const fn vibrations(&self) -> &'static [Vibration] {
        self.vibrations
    }

    /// Its level sets.
    #[must_use]
    pub const fn levels(&self) -> &'static [LevelSet] {
        self.levels
    }

    /// The gas's three at `temperature`, or `None` for a temperature that is not above 0 K and
    /// finite.
    #[must_use]
    fn terms(&self, temperature: Kelvin) -> Option<Terms> {
        let t = temperature.value();
        if !(t > 0.0 && t.is_finite()) {
            return None;
        }
        let constant = 2.5 + self.rotation.heat_capacity_over_r();
        let mut sum = Terms {
            c: constant,
            h: constant * t,
            s: constant * math::ln(t),
        };
        for vibration in self.vibrations {
            sum = sum.add_scaled(
                f64::from(vibration.degeneracy),
                oscillator(SECOND_RADIATION_CONSTANT_CM_K * vibration.wavenumber_cm, t),
            );
        }
        for set in self.levels {
            sum = sum.add_scaled(set.weight(), set.terms(t));
        }
        Some(sum)
    }

    /// The ideal gas's `c_p` ÷ R at `temperature`, or `None` for a temperature that is not above
    /// 0 K and finite.
    #[must_use]
    pub fn heat_capacity_over_r(&self, temperature: Kelvin) -> Option<f64> {
        self.terms(temperature).map(|terms| terms.c)
    }

    /// H̃ = ∫₀ᵀ `c_p` ÷ R dT, kelvin, zero at 0 K, or `None` for a temperature that is not above
    /// 0 K and finite.
    #[must_use]
    pub fn enthalpy_over_r(&self, temperature: Kelvin) -> Option<Kelvin> {
        self.terms(temperature).map(|terms| Kelvin::new(terms.h))
    }

    /// S̃ = ∫ `c_p` ÷ (R T) dT, whose zero is arbitrary, or `None` for a temperature that is not
    /// above 0 K and finite.
    #[must_use]
    pub fn entropy_over_r(&self, temperature: Kelvin) -> Option<f64> {
        self.terms(temperature).map(|terms| terms.s)
    }
}

/// The molecular constants of the substance `id`, if it is a gas row.
#[must_use]
fn molecular(id: SubstanceId) -> Option<&'static MolecularConstants> {
    id.substance().gas().map(super::GasColumns::molecular)
}

/// The ideal-gas `c_p` ÷ R of the substance `id` at `temperature`
/// ([`MolecularConstants::heat_capacity_over_r`]), or `None` for a row without gas columns or a
/// temperature that is not above 0 K and finite.
///
/// # Examples
///
/// Molecular nitrogen holds equipartition's 7/2 from Triton's 38 K to Earth's air, and gains its
/// vibration on a hot world:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::heat_capacity_over_r_at;
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let n2 = Gas::Nitrogen.substance();
/// assert!((heat_capacity_over_r_at(n2, Kelvin::new(38.0))? - 3.5).abs() < 1e-9);
/// assert!(heat_capacity_over_r_at(n2, Kelvin::new(1_000.0))? > 3.9);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn heat_capacity_over_r_at(id: SubstanceId, temperature: Kelvin) -> Option<f64> {
    molecular(id)?.heat_capacity_over_r(temperature)
}

/// H̃ = ∫₀ᵀ `c_p` ÷ R dT of the substance `id`, kelvin ([`MolecularConstants::enthalpy_over_r`]),
/// or `None` for a row without gas columns or a temperature that is not above 0 K and finite.
#[must_use]
pub fn enthalpy_over_r(id: SubstanceId, temperature: Kelvin) -> Option<Kelvin> {
    molecular(id)?.enthalpy_over_r(temperature)
}

/// S̃ = ∫ `c_p` ÷ (R T) dT of the substance `id` ([`MolecularConstants::entropy_over_r`]), whose
/// zero is arbitrary, or `None` for a row without gas columns or a temperature that is not above
/// 0 K and finite.
#[must_use]
pub fn entropy_over_r(id: SubstanceId, temperature: Kelvin) -> Option<f64> {
    molecular(id)?.entropy_over_r(temperature)
}

/// The mixture's ideal-gas `c_p` ÷ R at `temperature`, Σ xᵢ (`c_p,ᵢ` ÷ R) summed in its order,
/// whose reciprocal is the dry adiabat's local exponent R ÷ `c_p`; `None` for an empty mixture, a
/// row without gas columns or a temperature that is not above 0 K and finite. The fractions are
/// mole fractions summing to 1, not renormalised.
///
/// # Examples
///
/// Earth's dry air at its surface, against kinetic theory's 2/7:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::mixture_heat_capacity_over_r_at;
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let air = [
///     (Gas::Nitrogen.substance(), 0.780_84),
///     (Gas::Oxygen.substance(), 0.209_46),
///     (Gas::Argon.substance(), 0.009_34),
///     (Gas::CarbonDioxide.substance(), 0.000_36),
/// ];
/// let r_over_cp = 1.0 / mixture_heat_capacity_over_r_at(&air, Kelvin::new(288.0))?;
/// assert!((r_over_cp - 2.0 / 7.0).abs() < 1e-3);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn mixture_heat_capacity_over_r_at(
    mixture: &[(SubstanceId, f64)],
    temperature: Kelvin,
) -> Option<f64> {
    mixed(mixture, |id| heat_capacity_over_r_at(id, temperature))
}

/// The mixture's H̃ ÷ R, kelvin, Σ xᵢ H̃ᵢ summed in its order; `None` as for
/// [`mixture_heat_capacity_over_r_at`].
#[must_use]
pub fn mixture_enthalpy_over_r(
    mixture: &[(SubstanceId, f64)],
    temperature: Kelvin,
) -> Option<Kelvin> {
    mixed(mixture, |id| {
        enthalpy_over_r(id, temperature).map(Kelvin::value)
    })
    .map(Kelvin::new)
}

/// The mixture's S̃ ÷ R, Σ xᵢ S̃ᵢ summed in its order, without the entropy of mixing, which is
/// constant at a fixed composition; `None` as for [`mixture_heat_capacity_over_r_at`].
///
/// S̃'s zero is arbitrary: only its differences mean anything. A dry adiabat keeps
/// S̃(T) − ln p constant, so the temperature at a pressure is where S̃ has risen by
/// ln(p ÷ `p_s`) from the surface's (P14.T24.e, with its α).
///
/// # Examples
///
/// Nitrogen rising from 288 K and 1 bar to 0.5 bar cools as kinetic theory's power law,
/// T ∝ p^(2/7), since its `c_p` is 7/2 there:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::math;
/// use hyperion_sim::substance::mixture_entropy_over_r;
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let n2 = [(Gas::Nitrogen.substance(), 1.0)];
/// let target = mixture_entropy_over_r(&n2, Kelvin::new(288.0))? + math::ln(0.5);
/// let (mut cold, mut warm) = (150.0, 288.0);
/// for _ in 0..60 {
///     let mid = 0.5 * (cold + warm);
///     if mixture_entropy_over_r(&n2, Kelvin::new(mid))? < target {
///         cold = mid;
///     } else {
///         warm = mid;
///     }
/// }
/// let power_law = 288.0 * math::powf(0.5, 2.0 / 7.0);
/// assert!((cold - power_law).abs() < 0.01);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn mixture_entropy_over_r(mixture: &[(SubstanceId, f64)], temperature: Kelvin) -> Option<f64> {
    mixed(mixture, |id| entropy_over_r(id, temperature))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::derive::atmosphere::Gas;
    use crate::substance::{EQUILIBRIUM_HYDROGEN, SubstanceKind, by_key, ids};
    use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};

    /// The row whose key is `key`.
    fn row(key: &str) -> SubstanceId {
        by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
    }

    /// The row `key`'s molecular constants.
    fn constants(key: &str) -> MolecularConstants {
        *row(key)
            .substance()
            .gas()
            .unwrap_or_else(|| panic!("{key} is a gas row"))
            .molecular()
    }

    /// The row `key`'s `c_p` ÷ R at `t` kelvin.
    fn cp(key: &str, t: f64) -> f64 {
        heat_capacity_over_r_at(row(key), Kelvin::new(t))
            .expect("a gas row, a positive temperature")
    }

    /// NASA Glenn's ideal-gas `c_p` ÷ R, the oracle (McBride, Zehe and Gordon 2002,
    /// `NASA/TP-2002-211556`, public domain): each row's `(T, c_p ÷ R)` to six significant figures,
    /// evaluated from its NASA-9 polynomial in `data/thermo.inp` of NASA's `nasa/cea` repository
    /// (Apache 2.0; SHA-256 fa7746572952…28884363, fetched 2026-10-14) inside the polynomial's
    /// range, from 200 or 300 K up. Its H₂ is equilibrium hydrogen (science-r08-gases-2.md §1.5);
    /// it holds no `FeH`.
    const NASA_GLENN: &[(&str, &[(f64, f64)])] = &[
        (
            "H2",
            &[
                (200.0, 3.30086),
                (300.0, 3.46973),
                (400.0, 3.5106),
                (500.0, 3.51837),
                (600.0, 3.52614),
                (700.0, 3.5413),
                (800.0, 3.56356),
                (1000.0, 3.63296),
                (1200.0, 3.72633),
                (1500.0, 3.88533),
            ],
        ),
        (
            "He",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "H2O",
            &[
                (200.0, 4.011),
                (300.0, 4.04064),
                (400.0, 4.12114),
                (500.0, 4.23655),
                (600.0, 4.36874),
                (700.0, 4.51009),
                (800.0, 4.65787),
                (1000.0, 4.96614),
                (1200.0, 5.27304),
                (1500.0, 5.69107),
            ],
        ),
        (
            "CH4",
            &[
                (200.0, 4.0298),
                (300.0, 4.30097),
                (400.0, 4.88516),
                (500.0, 5.60283),
                (600.0, 6.33721),
                (700.0, 7.04107),
                (800.0, 7.69891),
                (1000.0, 8.86119),
                (1200.0, 9.8175),
                (1500.0, 10.9286),
            ],
        ),
        (
            "NH3",
            &[
                (200.0, 4.05975),
                (300.0, 4.29108),
                (400.0, 4.65182),
                (500.0, 5.05041),
                (600.0, 5.43971),
                (700.0, 5.80566),
                (800.0, 6.14734),
                (1000.0, 6.76466),
                (1200.0, 7.29298),
                (1500.0, 7.93258),
            ],
        ),
        (
            "N2",
            &[
                (200.0, 3.5008),
                (300.0, 3.50294),
                (400.0, 3.51779),
                (500.0, 3.55788),
                (600.0, 3.62129),
                (700.0, 3.69881),
                (800.0, 3.78064),
                (1000.0, 3.93246),
                (1200.0, 4.05607),
                (1500.0, 4.1905),
            ],
        ),
        (
            "O2",
            &[
                (200.0, 3.50303),
                (300.0, 3.53449),
                (400.0, 3.62194),
                (500.0, 3.73944),
                (600.0, 3.85953),
                (700.0, 3.96773),
                (800.0, 4.05857),
                (1000.0, 4.19538),
                (1200.0, 4.29309),
                (1500.0, 4.39627),
            ],
        ),
        (
            "CO2",
            &[
                (200.0, 3.89214),
                (300.0, 4.47652),
                (400.0, 4.97024),
                (500.0, 5.36706),
                (600.0, 5.69156),
                (700.0, 5.96083),
                (800.0, 6.18585),
                (1000.0, 6.5318),
                (1200.0, 6.77697),
                (1500.0, 7.02072),
            ],
        ),
        (
            "Ar",
            &[
                (200.0, 2.5),
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "Ne",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "Kr",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "Xe",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.49999),
                (1500.0, 2.50001),
            ],
        ),
        (
            "N2O",
            &[
                (300.0, 4.6559),
                (400.0, 5.13555),
                (500.0, 5.51698),
                (600.0, 5.83013),
                (700.0, 6.091),
                (800.0, 6.30886),
                (1000.0, 6.64277),
                (1200.0, 6.87868),
                (1500.0, 7.11657),
            ],
        ),
        (
            "CO",
            &[
                (200.0, 3.50098),
                (300.0, 3.50506),
                (400.0, 3.52893),
                (500.0, 3.58334),
                (600.0, 3.66084),
                (700.0, 3.74895),
                (800.0, 3.83669),
                (1000.0, 3.99047),
                (1200.0, 4.10972),
                (1500.0, 4.23517),
            ],
        ),
        (
            "SO2",
            &[
                (300.0, 4.79998),
                (400.0, 5.22369),
                (500.0, 5.59134),
                (600.0, 5.8859),
                (700.0, 6.11356),
                (800.0, 6.2881),
                (1000.0, 6.52965),
                (1200.0, 6.68288),
                (1500.0, 6.82633),
            ],
        ),
        (
            "H2S",
            &[
                (300.0, 4.12256),
                (400.0, 4.28965),
                (500.0, 4.48636),
                (600.0, 4.69894),
                (700.0, 4.91774),
                (800.0, 5.13445),
                (1000.0, 5.53208),
                (1200.0, 5.8617),
                (1500.0, 6.23527),
            ],
        ),
        (
            "O3",
            &[
                (300.0, 4.74638),
                (400.0, 5.2873),
                (500.0, 5.72337),
                (600.0, 6.05005),
                (700.0, 6.2911),
                (800.0, 6.4728),
                (1000.0, 6.72615),
                (1200.0, 6.94406),
                (1500.0, 7.03747),
            ],
        ),
        (
            "HCN",
            &[
                (300.0, 4.32115),
                (400.0, 4.71773),
                (500.0, 5.0191),
                (600.0, 5.26888),
                (700.0, 5.4896),
                (800.0, 5.69059),
                (1000.0, 6.04099),
                (1200.0, 6.32726),
                (1500.0, 6.65471),
            ],
        ),
        (
            "C2H2",
            &[
                (300.0, 5.30869),
                (400.0, 6.05357),
                (500.0, 6.57809),
                (600.0, 6.98325),
                (700.0, 7.32508),
                (800.0, 7.63076),
                (1000.0, 8.16699),
                (1200.0, 8.61617),
                (1500.0, 9.14572),
            ],
        ),
        (
            "C2H4",
            &[
                (200.0, 4.2534),
                (300.0, 5.17939),
                (400.0, 6.37873),
                (500.0, 7.51306),
                (600.0, 8.5003),
                (700.0, 9.34774),
                (800.0, 10.0842),
                (1000.0, 11.2968),
                (1200.0, 12.2271),
                (1500.0, 13.2318),
            ],
        ),
        (
            "C2H6",
            &[
                (300.0, 6.34138),
                (400.0, 7.87271),
                (500.0, 9.37113),
                (600.0, 10.725),
                (700.0, 11.9214),
                (800.0, 12.979),
                (1000.0, 14.7381),
                (1200.0, 16.092),
                (1500.0, 17.5478),
            ],
        ),
        (
            "PH3",
            &[
                (300.0, 4.47183),
                (400.0, 5.0245),
                (500.0, 5.59037),
                (600.0, 6.12314),
                (700.0, 6.60695),
                (800.0, 7.03688),
                (1000.0, 7.7334),
                (1200.0, 8.24311),
                (1500.0, 8.75801),
            ],
        ),
        (
            "CH3OH",
            &[
                (200.0, 4.77211),
                (300.0, 5.31089),
                (400.0, 6.20854),
                (500.0, 7.17821),
                (600.0, 8.08128),
                (700.0, 8.88378),
                (800.0, 9.59161),
                (1000.0, 10.769),
                (1200.0, 11.681),
                (1500.0, 12.6769),
            ],
        ),
        (
            "H",
            &[
                (200.0, 2.5),
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "O",
            &[
                (200.0, 2.73425),
                (300.0, 2.63406),
                (400.0, 2.58377),
                (500.0, 2.55664),
                (600.0, 2.54069),
                (700.0, 2.53056),
                (800.0, 2.52375),
                (1000.0, 2.51552),
                (1200.0, 2.51052),
                (1500.0, 2.50745),
            ],
        ),
        (
            "e-",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "H-",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "Na",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.49735),
                (1500.0, 2.50183),
            ],
        ),
        (
            "K",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.50001),
                (1200.0, 2.50767),
                (1500.0, 2.49672),
            ],
        ),
        (
            "Fe",
            &[
                (300.0, 3.08887),
                (400.0, 3.07081),
                (500.0, 2.99358),
                (600.0, 2.9108),
                (700.0, 2.83955),
                (800.0, 2.78239),
                (1000.0, 2.70475),
                (1200.0, 2.66764),
                (1500.0, 2.67333),
            ],
        ),
        (
            "Mg",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.49948),
                (1500.0, 2.50006),
            ],
        ),
        (
            "Si",
            &[
                (300.0, 2.67415),
                (400.0, 2.59947),
                (500.0, 2.5637),
                (600.0, 2.54417),
                (700.0, 2.53257),
                (800.0, 2.52568),
                (1000.0, 2.52188),
                (1200.0, 2.52937),
                (1500.0, 2.55988),
            ],
        ),
        (
            "Ca",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.48409),
                (1500.0, 2.5134),
            ],
        ),
        (
            "Ti",
            &[
                (300.0, 2.9345),
                (400.0, 2.77878),
                (500.0, 2.68932),
                (600.0, 2.6356),
                (700.0, 2.60176),
                (800.0, 2.58034),
                (1000.0, 2.56455),
                (1200.0, 2.57996),
                (1500.0, 2.6756),
            ],
        ),
        (
            "SiO",
            &[
                (300.0, 3.59865),
                (400.0, 3.74523),
                (500.0, 3.89315),
                (600.0, 4.0177),
                (700.0, 4.11568),
                (800.0, 4.19159),
                (1000.0, 4.29745),
                (1200.0, 4.36537),
                (1500.0, 4.42799),
            ],
        ),
        (
            "TiO",
            &[
                (300.0, 3.83626),
                (400.0, 3.96326),
                (500.0, 4.08782),
                (600.0, 4.18902),
                (700.0, 4.26927),
                (800.0, 4.33574),
                (1000.0, 4.44353),
                (1200.0, 4.52697),
                (1500.0, 4.62961),
            ],
        ),
        (
            "VO",
            &[
                (300.0, 3.69562),
                (400.0, 3.87956),
                (500.0, 4.03166),
                (600.0, 4.14595),
                (700.0, 4.23118),
                (800.0, 4.29804),
                (1000.0, 4.4029),
                (1200.0, 4.48884),
                (1500.0, 4.60896),
            ],
        ),
        (
            "Al",
            &[
                (300.0, 2.57177),
                (400.0, 2.53978),
                (500.0, 2.52512),
                (600.0, 2.51727),
                (700.0, 2.51258),
                (800.0, 2.50956),
                (1000.0, 2.50606),
                (1200.0, 2.50408),
                (1500.0, 2.50261),
            ],
        ),
        (
            "V",
            &[
                (300.0, 3.12444),
                (400.0, 2.96423),
                (500.0, 2.91049),
                (600.0, 2.9203),
                (700.0, 2.95638),
                (800.0, 2.99396),
                (1000.0, 3.03543),
                (1200.0, 3.03127),
                (1500.0, 2.99154),
            ],
        ),
        (
            "Mn",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.49713),
                (1500.0, 2.50187),
            ],
        ),
        (
            "Ni",
            &[
                (300.0, 2.8104),
                (400.0, 2.86965),
                (500.0, 2.93145),
                (600.0, 2.97488),
                (700.0, 2.99779),
                (800.0, 3.00443),
                (1000.0, 2.98768),
                (1200.0, 2.95194),
                (1500.0, 2.88744),
            ],
        ),
        (
            "Zn",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.50043),
                (1500.0, 2.4997),
            ],
        ),
        (
            "Na+",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "Mg+",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.50007),
                (1500.0, 2.49996),
            ],
        ),
        (
            "Al+",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5001),
                (1500.0, 2.49995),
            ],
        ),
        (
            "Si+",
            &[
                (300.0, 2.9229),
                (400.0, 2.7593),
                (500.0, 2.67003),
                (600.0, 2.61861),
                (700.0, 2.58692),
                (800.0, 2.56619),
                (1000.0, 2.54187),
                (1200.0, 2.52875),
                (1500.0, 2.51819),
            ],
        ),
        (
            "K+",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.5),
                (1500.0, 2.5),
            ],
        ),
        (
            "Ca+",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.49659),
                (1500.0, 2.50523),
            ],
        ),
        (
            "Ti+",
            &[
                (300.0, 3.14908),
                (400.0, 3.14289),
                (500.0, 3.10844),
                (600.0, 3.04978),
                (700.0, 2.98497),
                (800.0, 2.9251),
                (1000.0, 2.83494),
                (1200.0, 2.78414),
                (1500.0, 2.75348),
            ],
        ),
        (
            "V+",
            &[
                (300.0, 2.78126),
                (400.0, 2.67642),
                (500.0, 2.64647),
                (600.0, 2.66759),
                (700.0, 2.72195),
                (800.0, 2.79292),
                (1000.0, 2.93465),
                (1200.0, 3.03722),
                (1500.0, 3.09958),
            ],
        ),
        (
            "Cr+",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.50003),
                (1200.0, 2.49761),
                (1500.0, 2.50806),
            ],
        ),
        (
            "Mn+",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.50001),
                (1000.0, 2.50016),
                (1200.0, 2.502),
                (1500.0, 2.50702),
            ],
        ),
        (
            "Fe+",
            &[
                (300.0, 3.13635),
                (400.0, 3.14052),
                (500.0, 3.11886),
                (600.0, 3.10672),
                (700.0, 3.1032),
                (800.0, 3.10145),
                (1000.0, 3.0863),
                (1200.0, 3.0544),
                (1500.0, 2.99225),
            ],
        ),
        (
            "Ni+",
            &[
                (300.0, 2.52531),
                (400.0, 2.58617),
                (500.0, 2.66121),
                (600.0, 2.72643),
                (700.0, 2.77227),
                (800.0, 2.7988),
                (1000.0, 2.81118),
                (1200.0, 2.80693),
                (1500.0, 2.78583),
            ],
        ),
        (
            "Zn+",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.5),
                (600.0, 2.5),
                (700.0, 2.5),
                (800.0, 2.5),
                (1000.0, 2.5),
                (1200.0, 2.50095),
                (1500.0, 2.49937),
            ],
        ),
        (
            "HCl",
            &[
                (300.0, 3.50433),
                (400.0, 3.50879),
                (500.0, 3.52449),
                (600.0, 3.55739),
                (700.0, 3.60665),
                (800.0, 3.66808),
                (1000.0, 3.80528),
                (1200.0, 3.93679),
                (1500.0, 4.10171),
            ],
        ),
        (
            "Cr",
            &[
                (300.0, 2.5),
                (400.0, 2.5),
                (500.0, 2.49999),
                (600.0, 2.50001),
                (700.0, 2.50009),
                (800.0, 2.50053),
                (1000.0, 2.50591),
                (1200.0, 2.5374),
                (1500.0, 2.60724),
            ],
        ),
    ];

    /// The rows the ruling holds closest: within 1 per cent to 1,000 K and 2 per cent to 1,500 K
    /// (science-r08-gases-2.md §1.3).
    const CLOSE: [&str; 8] = ["H2", "He", "N2", "O2", "CO", "CO2", "H2O", "Ar"];

    /// Every gas row's sum against NASA Glenn: H₂ (in equilibrium), He, N₂, O₂, CO, CO₂, H₂O and
    /// Ar within 1 per cent from 200 to 1,000 K and 2 per cent to 1,500 K, and every other row
    /// NASA Glenn has within 2 per cent to 700 K and 3 per cent to 1,000 K.
    #[test]
    fn the_heat_capacities_follow_nasa_glenn() {
        for &(key, oracle) in NASA_GLENN {
            let close = CLOSE.contains(&key);
            let molecular = if key == "H2" {
                EQUILIBRIUM_HYDROGEN
            } else {
                constants(key)
            };
            for &(t, expected) in oracle {
                let within = match (close, t) {
                    (true, t) if t <= 1_000.0 => 0.01,
                    (true, _) => 0.02,
                    (false, t) if t <= 700.0 => 0.02,
                    (false, t) if t <= 1_000.0 => 0.03,
                    (false, _) => continue,
                };
                let actual = molecular
                    .heat_capacity_over_r(Kelvin::new(t))
                    .expect("t > 0");
                assert!(
                    (actual / expected - 1.0).abs() < within,
                    "{key} at {t} K: {actual} against NASA Glenn's {expected}"
                );
            }
        }
        // Every gas row but FeH, which NASA Glenn does not hold, has an oracle.
        for id in ids().filter(|id| id.substance().gas().is_some()) {
            let key = id.substance().key();
            assert!(
                key.as_str() == "FeH" || NASA_GLENN.iter().any(|&(k, _)| k == key.as_str()),
                "{key} has an oracle"
            );
        }
    }

    /// The noble gases and the bare charges hold kinetic theory's 5/2 to the bit at every
    /// temperature: the noble gases' first excited levels lie above 8 eV, and e⁻ and H⁻ have none.
    #[test]
    fn the_noble_gases_hold_five_halves_exactly() {
        for key in ["He", "Ne", "Ar", "Kr", "Xe", "e-", "H-"] {
            for t in [10.0, 38.0, 300.0, 1_500.0, 5_000.0] {
                assert_same_bits(cp(key, t), 2.5);
            }
        }
    }

    /// Normal and equilibrium hydrogen below 200 K against Woolley, Scott and Brickwedde 1948 (J.
    /// Res. NBS 41, 379), within 0.1 per cent: normal hydrogen's `C_p` from their Table 8 over
    /// their R, 1.98714 cal mol⁻¹ K⁻¹, and equilibrium hydrogen's from their para- and
    /// ortho-hydrogen's `C_p` and H − E₀ (Table 4) at their equilibrium para fractions (Table 12),
    /// as `x_p C_p + x_o C_o + x_p x_o (H_o − H_p)² ÷ (R T²)`, since they print none.
    #[test]
    fn hydrogen_follows_woolley_scott_and_brickwedde_below_200_k() {
        let normal = [
            (30.0, 2.500_1),
            (50.0, 2.505_1),
            (70.0, 2.546_9),
            (80.0, 2.590_7),
            (100.0, 2.714_0),
            (150.0, 3.053_1),
            (200.0, 3.280_1),
        ];
        let equilibrium = [
            (30.0, 3.432_8),
            (50.0, 4.566_9),
            (70.0, 4.033_9),
            (80.0, 3.746_7),
            (100.0, 3.385_8),
            (150.0, 3.193_9),
            (200.0, 3.301_6),
        ];
        for (t, expected) in normal {
            let actual = cp("H2", t);
            assert!(
                (actual / expected - 1.0).abs() < 1e-3,
                "normal H₂ at {t} K: {actual}"
            );
        }
        for (t, expected) in equilibrium {
            let actual = EQUILIBRIUM_HYDROGEN
                .heat_capacity_over_r(Kelvin::new(t))
                .expect("t > 0");
            assert!(
                (actual / expected - 1.0).abs() < 1e-3,
                "equilibrium H₂ at {t} K: {actual}"
            );
        }
    }

    /// The hindered rotors at 100 K, within 0.5 per cent: ethane against Chao, Wilhoit and
    /// Zwolinski 1973's 8.52 cal mol⁻¹ K⁻¹ (J. Phys. Chem. Ref. Data 2, 427, Table 6; 4.287 R), the
    /// table read in place of the plan's Gurvich et al. 1989 (35.70 J mol⁻¹ K⁻¹, 4.294 R, from
    /// memory and unread), and methanol against Chen, Wilhoit and Zwolinski 1977's 8.82 (J. Phys.
    /// Chem. Ref. Data 6, 105, Table 4; 4.438 R), whose A and E levels it weighs alike, as they
    /// do. Kinetic theory's fixed N = 6, 4, is 7 per cent low.
    #[test]
    fn the_hindered_rotors_meet_their_tables_at_100_k() {
        for (key, expected) in [("C2H6", 4.287_4), ("CH3OH", 4.438_4)] {
            let actual = cp(key, 100.0);
            assert!(
                (actual / expected - 1.0).abs() < 5e-3,
                "{key}: {actual} against {expected}"
            );
        }
    }

    /// The classes are the sum's cold limit: N₂ holds 7/2 to 10⁻⁴ from 30 to 200 K, and CO₂'s
    /// limit is 7/2 too, to 10⁻³ at 50 K, so the held class's 13/3 (R ÷ `c_p` = 3/13) is a fitted
    /// value that the sum reaches only between 250 and 300 K. T49.a's test of R ÷ `c_p` for pure N₂
    /// and CO₂ moves here.
    #[test]
    fn the_classes_are_the_sum_s_cold_limit() {
        for t in [30.0, 38.0, 63.0, 100.0, 150.0, 200.0] {
            let pure = [(Gas::Nitrogen.substance(), 1.0)];
            let c =
                mixture_heat_capacity_over_r_at(&pure, Kelvin::new(t)).expect("N₂ is a gas row");
            assert!((c - 3.5).abs() < 1e-4, "N₂ at {t} K: {c}");
            assert!((1.0 / c - 2.0 / 7.0).abs() < 1e-5, "N₂'s R ÷ c_p at {t} K");
        }
        assert!((cp("CO2", 50.0) - 3.5).abs() < 1e-3);
        assert!(cp("CO2", 250.0) < 13.0 / 3.0 && cp("CO2", 300.0) > 13.0 / 3.0);
    }

    /// H̃ and S̃ are the integrals of `c_p` ÷ R and `c_p` ÷ (R T): against a 16-point Gauss–Legendre
    /// quadrature of the sum over 200 equal steps, to 10⁻¹⁰ of the integral, for a gas of each
    /// kind of term (the ladder, none, an oscillator with states, oscillators, a torsion, an
    /// atom's levels, a diatomic's states).
    #[test]
    fn the_enthalpy_and_entropy_integrate_the_heat_capacity() {
        for key in ["H2", "He", "O2", "CO2", "C2H6", "Fe", "TiO"] {
            let molecular = constants(key);
            for (lo, hi) in [(30.0, 300.0), (200.0, 2_000.0)] {
                let (mut enthalpy, mut entropy) = (0.0, 0.0);
                let steps = 200;
                let width = (hi - lo) / f64::from(steps);
                for step in 0..steps {
                    let mid = lo + (f64::from(step) + 0.5) * width;
                    for (node, weight) in GL16_NODES.iter().zip(GL16_WEIGHTS) {
                        let t = mid + 0.5 * width * node;
                        let cp = molecular
                            .heat_capacity_over_r(Kelvin::new(t))
                            .expect("t > 0");
                        enthalpy += 0.5 * width * weight * cp;
                        entropy += 0.5 * width * weight * cp / t;
                    }
                }
                let at = |t: f64| {
                    let t = Kelvin::new(t);
                    (
                        molecular.enthalpy_over_r(t).expect("t > 0").value(),
                        molecular.entropy_over_r(t).expect("t > 0"),
                    )
                };
                let ((h_lo, s_lo), (h_hi, s_hi)) = (at(lo), at(hi));
                assert!(
                    ((h_hi - h_lo) / enthalpy - 1.0).abs() < 1e-10,
                    "{key}'s H̃ over {lo}–{hi} K"
                );
                assert!(
                    ((s_hi - s_lo) / entropy - 1.0).abs() < 1e-10,
                    "{key}'s S̃ over {lo}–{hi} K"
                );
            }
        }
    }

    /// H̃ is zero at 0 K: at 1 K it is the constant parts' c T, every oscillator and level set
    /// frozen out, and normal hydrogen's ortho and para each sit in their own lowest level.
    /// Ethane's torsional ground is split by tunnelling, by 0.006 cm⁻¹, so its levels hold 0.0055 K
    /// at 1 K, and it is left out.
    #[test]
    fn the_enthalpy_vanishes_with_the_temperature() {
        for key in ["N2", "O2", "CO2", "Fe", "CH4"] {
            let m = constants(key);
            let h = m.enthalpy_over_r(Kelvin::new(1.0)).expect("t > 0").value();
            let constant = 2.5 + m.rotation().heat_capacity_over_r();
            assert!((h - constant).abs() < 1e-9, "{key}: {h}");
        }
        let h = enthalpy_over_r(row("H2"), Kelvin::new(1.0))
            .expect("t > 0")
            .value();
        assert!((h - 2.5).abs() < 1e-9, "{h}");
    }

    /// A mixture's three are Σ xᵢ of its rows', in the order given; Earth's dry air keeps kinetic
    /// theory's 2/7 within 0.3 per cent (science-r08-gases-2.md §1.2: R ÷ `c_p` 0.2864 at 214 K and
    /// 0.2859 at 288 K, against the classes' 0.2865).
    #[test]
    fn mixtures_sum_their_rows_in_order() {
        let air = [
            (Gas::Nitrogen.substance(), 0.780_84),
            (Gas::Oxygen.substance(), 0.209_46),
            (Gas::Argon.substance(), 0.009_34),
            (Gas::CarbonDioxide.substance(), 0.000_36),
        ];
        for (t, expected) in [(214.0, 0.286_4), (288.0, 0.285_9)] {
            let r_over_cp =
                1.0 / mixture_heat_capacity_over_r_at(&air, Kelvin::new(t)).expect("gas rows");
            assert!((r_over_cp - expected).abs() < 1e-4, "{t} K: {r_over_cp}");
        }
        let t = Kelvin::new(735.0);
        let sum = |value: &dyn Fn(SubstanceId) -> f64| {
            air.iter().fold(0.0, |sum, &(id, x)| sum + x * value(id))
        };
        assert_same_bits(
            mixture_heat_capacity_over_r_at(&air, t).expect("gas rows"),
            sum(&|id| heat_capacity_over_r_at(id, t).expect("a gas row")),
        );
        assert_same_bits(
            mixture_enthalpy_over_r(&air, t).expect("gas rows").value(),
            sum(&|id| enthalpy_over_r(id, t).expect("a gas row").value()),
        );
        assert_same_bits(
            mixture_entropy_over_r(&air, t).expect("gas rows"),
            sum(&|id| entropy_over_r(id, t).expect("a gas row")),
        );
        assert_eq!(mixture_heat_capacity_over_r_at(&[], t), None);
        assert_eq!(
            mixture_heat_capacity_over_r_at(&[(row("basalt"), 1.0)], t),
            None
        );
    }

    /// Venus's air wakes carbon dioxide's bends: `c_p` ÷ R 4.05, 4.44, 4.70, 5.31 and 5.96 at 230,
    /// 300, 350, 500 and 735 K for 96.5 per cent CO₂ and 3.5 per cent N₂ (science-r08-gases-2.md
    /// §1.5), to 0.02.
    #[test]
    fn venus_s_air_follows_its_temperature() {
        let venus = [
            (Gas::CarbonDioxide.substance(), 0.965),
            (Gas::Nitrogen.substance(), 0.035),
        ];
        for (t, expected) in [
            (230.0, 4.05),
            (300.0, 4.44),
            (350.0, 4.70),
            (500.0, 5.31),
            (735.0, 5.96),
        ] {
            let c = mixture_heat_capacity_over_r_at(&venus, Kelvin::new(t)).expect("gas rows");
            assert!((c - expected).abs() < 0.02, "{t} K: {c}");
        }
    }

    #[test]
    fn a_temperature_not_above_zero_has_no_heat_capacity() {
        let n2 = Gas::Nitrogen.substance();
        for t in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let t = Kelvin::new(t);
            assert_eq!(heat_capacity_over_r_at(n2, t), None);
            assert_eq!(enthalpy_over_r(n2, t), None);
            assert_eq!(entropy_over_r(n2, t), None);
        }
        assert_eq!(
            heat_capacity_over_r_at(row("basalt"), Kelvin::new(300.0)),
            None
        );
        // At an extreme cold, where θ ÷ T overflows, every mode and level is frozen out, and no
        // 0 × ∞ reaches the sums.
        for (key, cold_limit) in [("O2", 3.5), ("Fe", 2.5), ("H2", 2.5), ("C2H6", 4.0)] {
            assert_same_bits(cp(key, 1e-307), cold_limit);
        }
    }

    /// The eigenvalues of F p² + (V₃ ÷ 2)(1 − cos 3φ) in the class r of e^{i(r + 3k)φ}, |k| ≤ 60:
    /// a tridiagonal matrix, by bisection on its Sturm sequence, a fixed 200 halvings each.
    fn torsion_class(f: f64, v3: f64, r: i32) -> Vec<f64> {
        let diag: Vec<f64> = (-60..=60)
            .map(|k| {
                let m = f64::from(r + 3 * k);
                f * m * m + v3 / 2.0
            })
            .collect();
        let off = -v3 / 4.0;
        let below = |x: f64| {
            let mut count = 0;
            let mut q = diag[0] - x;
            if q < 0.0 {
                count += 1;
            }
            for d in &diag[1..] {
                if q == 0.0 {
                    q = 1e-300;
                }
                q = d - x - off * off / q;
                if q < 0.0 {
                    count += 1;
                }
            }
            count
        };
        let lo = diag.iter().copied().fold(f64::INFINITY, f64::min) - 2.0 * off.abs() - 1.0;
        let hi = diag.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 2.0 * off.abs() + 1.0;
        (0..diag.len())
            .map(|k| {
                let (mut a, mut b) = (lo, hi);
                for _ in 0..200 {
                    let mid = f64::midpoint(a, b);
                    if below(mid) > k {
                        b = mid;
                    } else {
                        a = mid;
                    }
                }
                f64::midpoint(a, b)
            })
            .collect()
    }

    /// The committed torsional levels are their Hamiltonians' eigenvalues below 25,000 cm⁻¹ above
    /// the lowest, each with its class's degeneracy, to 10⁻⁶ cm⁻¹.
    #[test]
    fn the_torsions_are_their_hamiltonians_eigenvalues() {
        use crate::substance::rows::{ETHANE_TORSION, METHANOL_TORSION};
        for (key, (f, v3)) in [("C2H6", ETHANE_TORSION), ("CH3OH", METHANOL_TORSION)] {
            let mut levels: Vec<(f64, f64)> = torsion_class(f, v3, 0)
                .into_iter()
                .map(|e| (e, 1.0))
                .chain(torsion_class(f, v3, 1).into_iter().map(|e| (e, 2.0)))
                .collect();
            levels.sort_by(|a, b| a.0.total_cmp(&b.0));
            let lowest = levels[0].0;
            let expected: Vec<(f64, f64)> = levels
                .into_iter()
                .map(|(e, g)| (e - lowest, g))
                .filter(|&(e, _)| e < 25_000.0)
                .collect();
            let Some(&LevelSet::Listed {
                levels: committed, ..
            }) = constants(key).levels().first()
            else {
                panic!("{key}'s torsion is a listed level set");
            };
            assert_eq!(committed.len(), expected.len(), "{key}");
            for (level, &(want_e, want_g)) in committed.iter().zip(&expected) {
                let e = level.energy_cm();
                assert!((e - want_e).abs() < 1e-6, "{key}: {e} against {want_e}");
                assert_same_bits(level.degeneracy(), want_g);
            }
        }
    }

    /// Each molecule's modes number 3N − 5 if linear and 3N − 6 if not, a torsion standing for
    /// the mode it replaces; atoms and charges do not rotate; only H₂'s rotation is a level set.
    #[test]
    fn every_gas_row_counts_its_modes() {
        for id in ids() {
            let row = id.substance();
            let Some(gas) = row.gas() else {
                continue;
            };
            let m = gas.molecular();
            let atoms: u32 = row.elements().iter().map(|&(_, n)| u32::from(n)).sum();
            let modes: u32 = m
                .vibrations()
                .iter()
                .map(|v| u32::from(v.degeneracy()))
                .sum();
            let torsions = u32::from(matches!(row.key().as_str(), "C2H6" | "CH3OH"));
            match (row.kind(), m.rotation()) {
                (SubstanceKind::Atom | SubstanceKind::Ion, Rotation::None) => {
                    assert!(atoms <= 1 && modes == 0, "{:?}", row.key());
                }
                (SubstanceKind::Molecule, Rotation::Linear) => {
                    assert_eq!(modes + torsions, 3 * atoms - 5, "{:?}", row.key());
                }
                (SubstanceKind::Molecule, Rotation::NonLinear) => {
                    assert_eq!(modes + torsions, 3 * atoms - 6, "{:?}", row.key());
                }
                (SubstanceKind::Molecule, Rotation::InLevels) => {
                    assert_eq!(row.key().as_str(), "H2");
                }
                (kind, rotation) => panic!("{:?}: a {kind:?} with {rotation:?}", row.key()),
            }
        }
    }

    /// c₂ = hc ÷ `k_B` from the 2019 SI's defined h, c and `k_B`, in cm K.
    #[test]
    fn the_second_radiation_constant_is_the_si_s() {
        let (h, c, k) = (6.626_070_15e-34, 299_792_458.0, 1.380_649e-23);
        assert!((SECOND_RADIATION_CONSTANT_CM_K / (100.0 * h * c / k) - 1.0).abs() < 1e-15);
    }
}
