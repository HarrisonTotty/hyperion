//! Silicate melts' liquid records (P14.T49.h's step 4; decision-p14-t49h-step4-melts;
//! decision-r11-palette-liquids §1.8, its "Molten silicate" row and "Lithology rows need oxides"):
//! a lithology's melt at the record's temperature and one bar, from the row's major oxides by
//! weight ([`SilicateMelt`]). The models are crate-private: rows build them, and nothing outside
//! the crate does.
//!
//! - **The viscosity** is an estimation method on the row's own oxides, `Derived`: Giordano,
//!   Russell and Dingwell 2008's model ([`giordano_2008`]) for every melt but an alkali-poor,
//!   aluminium-rich one, which takes Giordano and Russell 2007's model of the CaO–MgO–Al₂O₃–SiO₂
//!   melts ([`giordano_russell_2007`]), by decision-p14-t49h-step4-melts §2.3's rule
//!   ([`MeltViscosityModel::of`]).
//! - **The density** is Lange and Carmichael's ideal mixing of partial molar volumes
//!   ([`lange_carmichael_density`]), `Derived`, with the melt's compressibility neglected at the
//!   surface: at a bar it moves ρ by under 10⁻⁵ relative (Kress and Carmichael 1991's dV ÷ dP,
//!   about 2 × 10⁻⁴ cm³ mol⁻¹ bar⁻¹ for `SiO₂`, `Al₂O₃` and `TiO₂`, as MELTS's `liq_struct_data.h`
//!   carries it).
//! - **The index and the absorption** are a held glass's ([`HeldOptics`]), its room-temperature
//!   values neither carried to the melt's density nor to its temperature, `StandIn`.
//! - **The surface tension** is a stated constant ([`SilicateMelt::surface_tension_n_per_m`]),
//!   `StandIn`.
//! - **No scattering** (decision-r11-palette-liquids §1.8: the melt is opaque at a millimetre).
//!
//! The record's basis is `StandIn`, its index's. The range runs from the row's first melt, its
//! solidus, with no upper end ([`ResolveLiquidError::OutOfRange`]'s `below` is `None`). Above the
//! models' calibrations, 2,073.15 K for the viscosity (the authors' calculator permits 600–1,800
//! °C) and 1,873 K for the density (Lange and Carmichael's isothermal grid, 1,573–1,873 K), each
//! value extrapolates a form that stays continuous and bounded: the Vogel–Fulcher–Tammann form
//! falls towards its 10^A Pa s, and the volumes grow linearly. A lava world's day side reads them;
//! its real upper end, the melt's loss of sodium, potassium and iron towards a Ca–Al residue, is
//! not modelled. Between the solidus and the liquidus the record is the bulk liquid at the row's
//! composition, not the crystal-laden residual melt there (Shaw, Wright, Peck and Okamura 1968, Am.
//! J. Sci. 266, 225, its abstract: 8,000 poise for the Makaopuhi suspension at 1,130 °C against
//! 4,000 for its liquid).

use hyperion_surface::field::{LIQUID_BINS, LiquidBasis, LiquidProperties};

use super::super::Substance;
use super::super::element::Element;
use super::{LiquidMember, ResolveLiquidError};
use crate::math;
use crate::units::{Kelvin, KilogramsPerCubicMetre};

/// A major oxide of a silicate melt, as an analysis reports it by weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Oxide {
    /// Silica, `SiO₂`.
    Silica,
    /// Titania, `TiO₂`.
    Titania,
    /// Alumina, `Al₂O₃`.
    Alumina,
    /// Chromia, `Cr₂O₃`.
    Chromia,
    /// Ferric oxide, `Fe₂O₃`.
    Ferric,
    /// Ferrous oxide, `FeO`: an analysis's `FeO`, or its total iron as `FeO` where it reports no
    /// `Fe₂O₃`.
    Ferrous,
    /// Manganese oxide, `MnO`.
    Manganese,
    /// Magnesia, `MgO`.
    Magnesia,
    /// Lime, `CaO`.
    Lime,
    /// Nickel oxide, `NiO`.
    Nickel,
    /// Soda, `Na₂O`.
    Soda,
    /// Potash, K₂O.
    Potash,
    /// Phosphorus pentoxide, P₂O₅.
    PhosphorusPentoxide,
    /// Water, H₂O, dissolved.
    Water,
}

impl Oxide {
    /// The oxide's elements and their counts per formula unit.
    #[must_use]
    pub(crate) const fn elements(self) -> &'static [(Element, u16)] {
        match self {
            Self::Silica => &[(Element::Si, 1), (Element::O, 2)],
            Self::Titania => &[(Element::Ti, 1), (Element::O, 2)],
            Self::Alumina => &[(Element::Al, 2), (Element::O, 3)],
            Self::Chromia => &[(Element::Cr, 2), (Element::O, 3)],
            Self::Ferric => &[(Element::Fe, 2), (Element::O, 3)],
            Self::Ferrous => &[(Element::Fe, 1), (Element::O, 1)],
            Self::Manganese => &[(Element::Mn, 1), (Element::O, 1)],
            Self::Magnesia => &[(Element::Mg, 1), (Element::O, 1)],
            Self::Lime => &[(Element::Ca, 1), (Element::O, 1)],
            Self::Nickel => &[(Element::Ni, 1), (Element::O, 1)],
            Self::Soda => &[(Element::Na, 2), (Element::O, 1)],
            Self::Potash => &[(Element::K, 2), (Element::O, 1)],
            Self::PhosphorusPentoxide => &[(Element::P, 2), (Element::O, 5)],
            Self::Water => &[(Element::H, 2), (Element::O, 1)],
        }
    }

    /// The oxide's molar mass, g mol⁻¹: its elements' IUPAC 2021 abridged standard atomic weights
    /// summed (Prohaska et al. 2022, Pure Appl. Chem. 94, 573), the registry's own arithmetic.
    #[must_use]
    pub(crate) fn molar_mass_g_per_mol(self) -> f64 {
        let mut sum = 0.0;
        for &(element, count) in self.elements() {
            sum += f64::from(count) * element.standard_atomic_weight_g_per_mol();
        }
        sum
    }
}

/// The moles of `FeO` in one mole of `Fe₂O₃`, dimensionless: total iron as `FeO` counts each
/// `Fe₂O₃` as two `FeO`, mole for mole of iron.
const FERROUS_PER_FERRIC: f64 = 2.0;

/// `oxide`'s weight per cent in `oxides`, 0 where it is not listed.
#[must_use]
fn weight_pct(oxides: &[(Oxide, f64)], oxide: Oxide) -> f64 {
    oxides
        .iter()
        .find(|&&(listed, _)| listed == oxide)
        .map_or(0.0, |&(_, w)| w)
}

/// `oxide`'s moles per 100 g of `oxides`' analysis: its weight per cent over its molar mass.
#[must_use]
fn moles(oxides: &[(Oxide, f64)], oxide: Oxide) -> f64 {
    weight_pct(oxides, oxide) / oxide.molar_mass_g_per_mol()
}

/// The index and the absorption a melt holds as they stand, from a glass's file at room
/// temperature: neither carried by Lorentz–Lorenz to the melt's density, since the glass's own
/// density is not stated, nor to the melt's temperature (decision-r11-palette-liquids §1.8's
/// "Molten silicate" row).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeldOptics {
    /// The real index by bin, dimensionless.
    pub(crate) index: &'static [f64; LIQUID_BINS],
    /// The absorption coefficient a = 4πk ÷ λ by bin, m⁻¹, Napierian.
    pub(crate) absorption_per_m: &'static [f64; LIQUID_BINS],
}

/// A lithology's melt: its major oxides and the members it holds as stated values
/// (decision-r11-palette-liquids §1.8; decision-p14-t49h-step4-melts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SilicateMelt {
    /// The melt's major oxides, weight per cent, as its source analyses or reduces them, each oxide
    /// once: iron as the source reports it (`FeO` and `Fe₂O₃` where analysed apart, else total iron
    /// as `FeO`). They sum to the analysis's total, within a weight per cent of 100, as a test
    /// holds; the row's sources name the analysis and its basis.
    pub(crate) oxides: &'static [(Oxide, f64)],
    /// The surface tension against air, N m⁻¹, a stated constant at every temperature (its row's
    /// source names whose).
    pub(crate) surface_tension_n_per_m: f64,
    /// The index and the absorption.
    pub(crate) optics: HeldOptics,
}

/// A Vogel–Fulcher–Tammann viscosity, log₁₀ μ = A + B ÷ (T − C), μ in Pa s and T in K.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Vft {
    /// A, the high-temperature limit's log₁₀ μ, Pa s.
    pub(crate) a: f64,
    /// B, K.
    pub(crate) b_kelvin: f64,
    /// C, K.
    pub(crate) c_kelvin: f64,
}

impl Vft {
    /// log₁₀ μ at `temperature`, μ in Pa s; finite for `temperature` above C.
    #[must_use]
    pub(crate) fn log10_viscosity_pa_s(&self, temperature: Kelvin) -> f64 {
        self.a + self.b_kelvin / (temperature.value() - self.c_kelvin)
    }
}

/// Giordano, Russell and Dingwell 2008's A, log₁₀ Pa s (EPSL 271, 123, its Eq. 1 and Table 1:
/// −4.55 ± 0.21 at 95%).
const GRD_A: f64 = -4.55;

/// Giordano, Russell and Dingwell 2008's b₁ to b₇ and b₁₁ to b₁₃ on mol% (their Table 1), to the
/// two decimals the authors' calculator holds and their Table 2's demonstration uses (b₁ = 159.56:
/// b₁ × 62.79 = 10,018.8), which Table 1 prints rounded, each on its term:
///
/// - b₁ 159.56 on `SiO₂` + `TiO₂`, b₂ −173.34 on `Al₂O₃`, b₃ 72.13 on FeO(T) + `MnO` + P₂O₅, b₄
///   75.69 on `MgO`, b₅ −38.98 on `CaO`, b₆ −84.08 on `Na₂O` + V and b₇ 141.54 on V + ln(1 + H₂O);
/// - b₁₁ −2.43 on (`SiO₂` + `TiO₂`) FM, b₁₂ −0.91 on (`SiO₂` + TA + P₂O₅)(NK + H₂O) and b₁₃ 17.62
///   on `Al₂O₃` NK;
///
/// with V = H₂O + F₂O₋₁, TA = `TiO₂` + `Al₂O₃`, FM = FeO(T) + `MnO` + `MgO` and NK = `Na₂O` + K₂O
/// (the calculator `grd08.js`, Gordon and Russell 2008, from Russell's University of British
/// Columbia page, SHA-256 44806e60…; the paper's authors' copy, SHA-256 2b778bab…).
const GRD_B: [f64; 10] = [
    159.56, -173.34, 72.13, 75.69, -38.98, -84.08, 141.54, -2.43, -0.91, 17.62,
];

/// Giordano, Russell and Dingwell 2008's c₁ to c₆ and c₁₁ (their Table 1, the calculator's two
/// decimals): c₁ (`SiO₂`) 2.75, c₂ (TA) 15.72, c₃ (FM) 8.32, c₄ (`CaO`) 10.2, c₅ (NK) −12.29, c₆
/// (ln(1 + V)) −99.54 and c₁₁ ((`Al₂O₃` + FM + `CaO` − P₂O₅)(NK + V)) 0.30.
const GRD_C: [f64; 7] = [2.75, 15.72, 8.32, 10.2, -12.29, -99.54, 0.3];

/// Giordano, Russell and Dingwell 2008's model of a natural silicate melt's viscosity (EPSL 271,
/// 123; GRD), its A, B and C for `oxides`, as the authors' calculator evaluates it: total iron as
/// `FeO` (the paper's: a redox term did not improve the fit), the anhydrous oxides it takes
/// (`SiO₂`, `TiO₂`, `Al₂O₃`, FeO(T), `MnO`, `MgO`, `CaO`, `Na₂O`, K₂O, P₂O₅) normalised to 100 −
/// H₂O wt% with the water kept as given, then mole per cent, each with the registry's molar masses;
/// `Cr₂O₃` and `NiO`, which it does not take, are dropped by that normalisation, and fluorine is
/// not carried (no melt row holds it, so F₂O₋₁ is 0).
///
/// Its error is 0.40 log units rms over the 1,774 measurements it was fitted to (its Table 3; 0.34
/// for the anhydrous melts). Its calibration spans `SiO₂` 41–79, `TiO₂` 0–3, `Al₂O₃` 0–23, FeO(T)
/// 0–12, `MnO` 0–0.3, `MgO` 0–32, `CaO` 0–26, `Na₂O` 0–11, K₂O 0.3–9, P₂O₅ 0–1.2 and H₂O 0–8 wt% at
/// one bar, and μ from 10⁻¹ to 10¹⁴ Pa s (its §2). Outside them it extrapolates; it fails for an
/// alkali-free, aluminium-rich melt, which [`MeltViscosityModel::of`] sends to
/// [`giordano_russell_2007`] instead (decision-p14-t49h-step4-melts §1: its `Al₂O₃` term is
/// negative, and the alkali-balanced term that offsets it is zero there).
#[must_use]
pub(crate) fn giordano_2008(oxides: &[(Oxide, f64)]) -> Vft {
    let wt = |oxide| weight_pct(oxides, oxide);
    let total_iron = wt(Oxide::Ferrous)
        + FERROUS_PER_FERRIC * Oxide::Ferrous.molar_mass_g_per_mol() * wt(Oxide::Ferric)
            / Oxide::Ferric.molar_mass_g_per_mol();
    let anhydrous = [
        (Oxide::Silica, wt(Oxide::Silica)),
        (Oxide::Titania, wt(Oxide::Titania)),
        (Oxide::Alumina, wt(Oxide::Alumina)),
        (Oxide::Ferrous, total_iron),
        (Oxide::Manganese, wt(Oxide::Manganese)),
        (Oxide::Magnesia, wt(Oxide::Magnesia)),
        (Oxide::Lime, wt(Oxide::Lime)),
        (Oxide::Soda, wt(Oxide::Soda)),
        (Oxide::Potash, wt(Oxide::Potash)),
        (Oxide::PhosphorusPentoxide, wt(Oxide::PhosphorusPentoxide)),
    ];
    let water_wt = wt(Oxide::Water);
    let mut anhydrous_total = 0.0;
    for &(_, value) in &anhydrous {
        anhydrous_total += value;
    }
    let scale = (100.0 - water_wt) / anhydrous_total;
    let mut moles = [0.0; 11];
    for (mole, &(oxide, value)) in moles.iter_mut().zip(&anhydrous) {
        *mole = value * scale / oxide.molar_mass_g_per_mol();
    }
    moles[10] = water_wt / Oxide::Water.molar_mass_g_per_mol();
    let mut total = 0.0;
    for mole in moles {
        total += mole;
    }
    let [si, ti, al, fe, mn, mg, ca, na, k2o, p2o5, h2o] = moles.map(|mole| 100.0 * mole / total);
    // V = H₂O + F₂O₋₁, fluorine not carried.
    let volatiles = h2o;
    let siti = si + ti;
    let fm = fe + mn + mg;
    let nk = na + k2o;
    let b_terms = [
        siti,
        al,
        fe + mn + p2o5,
        mg,
        ca,
        na + volatiles,
        volatiles + math::ln_1p(h2o),
        siti * fm,
        (siti + al + p2o5) * (nk + h2o),
        al * nk,
    ];
    let c_terms = [
        si,
        ti + al,
        fm,
        ca,
        nk,
        math::ln_1p(volatiles),
        (al + fm + ca - p2o5) * (nk + volatiles),
    ];
    let mut b_kelvin = 0.0;
    for (coefficient, term) in GRD_B.iter().zip(b_terms) {
        b_kelvin += coefficient * term;
    }
    let mut c_kelvin = 0.0;
    for (coefficient, term) in GRD_C.iter().zip(c_terms) {
        c_kelvin += coefficient * term;
    }
    Vft {
        a: GRD_A,
        b_kelvin,
        c_kelvin,
    }
}

/// Giordano and Russell 2007's A for the VFT form, log₁₀ Pa s (J. Phys.: Condens. Matter 19,
/// 205148, Table 1: −4.67 ± 0.064).
const GR07_A: f64 = -4.67;

/// Giordano and Russell 2007's B coefficients on mol% (their Eq. 4 and Table 1's VFT column): b₁
/// `SiO₂` 107.384, b₂ `Al₂O₃` 58.607, b₃ `MgO` 85.317, b₄ `CaO` 73.746, b₁,₃ `SiO₂` (`MgO` + `CaO`)
/// −1.943 and b₂,₃ `Al₂O₃` (`MgO` + `CaO`) −1.251 (the full text, from Russell's University of
/// British Columbia page, SHA-256 dfd45a6e…).
const GR07_B: [f64; 6] = [107.384, 58.607, 85.317, 73.746, -1.943, -1.251];

/// Giordano and Russell 2007's C coefficients on mol% (their Eq. 5 and Table 1's VFT column): c₁
/// `SiO₂` 6.042, c₂ `Al₂O₃` 10.190, c₃ `MgO` 7.213 and c₄ `CaO` 9.626.
const GR07_C: [f64; 4] = [6.042, 10.190, 7.213, 9.626];

/// Giordano and Russell 2007's model of a CaO–MgO–Al₂O₃–SiO₂ melt's viscosity (J. Phys.: Condens.
/// Matter 19, 205148; GR07), the VFT form's A, B and C for `oxides`, with S, A, M and K the mole
/// per cent of `SiO₂`, `Al₂O₃`, `MgO` and `CaO` summing to 100 (decision-p14-t49h-step4-melts
/// §2.1):
///
/// - B = b₁ S + b₂ A + b₃ M + b₄ K + b₁,₃ S (M + K) + b₂,₃ A (M + K);
/// - C = c₁ S + c₂ A + c₃ M + c₄ K.
///
/// **The mapping onto its four oxides** (§2.1): FeO(T) (`FeO`, and each `Fe₂O₃` as two) and `MnO`
/// count as `MgO`, mole for mole, as Giordano, Russell and Dingwell 2008 treat them with `MgO`;
/// `Na₂O`, K₂O, `TiO₂`, `Cr₂O₃`, P₂O₅, `NiO` and H₂O are dropped and the four renormalised.
/// Counting the iron and manganese as `CaO` instead moves the Apollo 16 soil's log μ by at most
/// 0.03; dropping An95's sodium lowers μ by about 0.07 log units (Russell and Giordano 2005's
/// Di–An–Ab model, `An95Ab5` against An100).
///
/// Its standard error is 0.224 log units over 968 measurements (Table 1). Its database spans
/// 660–2,175 °C; for the anorthite-rich melts 1,053–2,449 K (Russell and Giordano 2005, GCA 69,
/// 5333, Table 1). On Scarfe, Cronin, Wenzel and Kauffman 1983's diopside–anorthite melts (Am.
/// Mineral. 68, 1083, Table 1) its 75 points lie at 0.18 rms (the ruling's §2.2).
#[must_use]
pub(crate) fn giordano_russell_2007(oxides: &[(Oxide, f64)]) -> Vft {
    let at = |oxide| moles(oxides, oxide);
    let silica = at(Oxide::Silica);
    let alumina = at(Oxide::Alumina);
    let magnesia = at(Oxide::Magnesia)
        + at(Oxide::Ferrous)
        + FERROUS_PER_FERRIC * at(Oxide::Ferric)
        + at(Oxide::Manganese);
    let lime = at(Oxide::Lime);
    let total = silica + alumina + magnesia + lime;
    let [si, al, mg, ca] = [silica, alumina, magnesia, lime].map(|mole| 100.0 * mole / total);
    let modifiers = mg + ca;
    let b_terms = [si, al, mg, ca, si * modifiers, al * modifiers];
    let c_terms = [si, al, mg, ca];
    let mut b_kelvin = 0.0;
    for (coefficient, term) in GR07_B.iter().zip(b_terms) {
        b_kelvin += coefficient * term;
    }
    let mut c_kelvin = 0.0;
    for (coefficient, term) in GR07_C.iter().zip(c_terms) {
        c_kelvin += coefficient * term;
    }
    Vft {
        a: GR07_A,
        b_kelvin,
        c_kelvin,
    }
}

/// The least `Al₂O₃` of a melt that takes [`giordano_russell_2007`], wt%: where Giordano, Russell
/// and Dingwell 2008's miss on Scarfe et al. 1983's alkali-free join passes 0.4 log units
/// (decision-p14-t49h-step4-melts §1 and §2.3).
pub(crate) const GR07_LEAST_ALUMINA_WT_PCT: f64 = 15.0;

/// The most `Na₂O` + K₂O of a melt that takes [`giordano_russell_2007`], wt% (§2.3).
pub(crate) const GR07_MOST_ALKALIS_WT_PCT: f64 = 1.0;

/// The most a melt that takes [`giordano_russell_2007`] may hold of the oxides its mapping drops,
/// wt% (§2.3): all but `SiO₂`, `Al₂O₃`, `MgO`, `CaO`, `FeO`, `Fe₂O₃` (which counts as FeO(T)) and
/// `MnO`.
pub(crate) const GR07_MOST_DROPPED_WT_PCT: f64 = 3.0;

/// Which viscosity model a melt takes, by decision-p14-t49h-step4-melts §2.3's rule on its oxides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MeltViscosityModel {
    /// [`giordano_2008`]: any melt the rule does not send elsewhere.
    Giordano2008,
    /// [`giordano_russell_2007`]: `Al₂O₃` at least [`GR07_LEAST_ALUMINA_WT_PCT`], `Na₂O` + K₂O at
    /// most [`GR07_MOST_ALKALIS_WT_PCT`] and the dropped oxides at most
    /// [`GR07_MOST_DROPPED_WT_PCT`].
    GiordanoRussell2007,
    /// The rule's first two conditions hold but not its third: §2.3 makes that a question for a
    /// ruling, not a silent choice, so the viscosity is absent until one is made. A test holds that
    /// no row is here.
    Unruled,
}

impl MeltViscosityModel {
    /// The model `oxides` take: an alkali-poor, aluminium-rich melt Giordano and Russell 2007's,
    /// any other Giordano, Russell and Dingwell 2008's.
    #[must_use]
    pub(crate) fn of(oxides: &[(Oxide, f64)]) -> Self {
        let w = |oxide| weight_pct(oxides, oxide);
        let alkalis = w(Oxide::Soda) + w(Oxide::Potash);
        let mut dropped = 0.0;
        for &(oxide, value) in oxides {
            let kept = matches!(
                oxide,
                Oxide::Silica
                    | Oxide::Alumina
                    | Oxide::Magnesia
                    | Oxide::Lime
                    | Oxide::Ferrous
                    | Oxide::Ferric
                    | Oxide::Manganese
            );
            if !kept {
                dropped += value;
            }
        }
        if w(Oxide::Alumina) >= GR07_LEAST_ALUMINA_WT_PCT && alkalis <= GR07_MOST_ALKALIS_WT_PCT {
            if dropped <= GR07_MOST_DROPPED_WT_PCT {
                Self::GiordanoRussell2007
            } else {
                Self::Unruled
            }
        } else {
            Self::Giordano2008
        }
    }

    /// The model's A, B and C for `oxides`, or `None` where it is [`Self::Unruled`].
    #[must_use]
    pub(crate) fn vft(self, oxides: &[(Oxide, f64)]) -> Option<Vft> {
        match self {
            Self::Giordano2008 => Some(giordano_2008(oxides)),
            Self::GiordanoRussell2007 => Some(giordano_russell_2007(oxides)),
            Self::Unruled => None,
        }
    }
}

/// The reference temperature of [`LANGE_CARMICHAEL`]'s volumes, 1,673 K.
const LANGE_CARMICHAEL_REFERENCE: Kelvin = Kelvin::new(1_673.0);

/// Lange and Carmichael's partial molar volumes of the oxides at 1,673 K and one bar, cm³ mol⁻¹,
/// with their dV ÷ dT, 10⁻³ cm³ mol⁻¹ K⁻¹, for the nine oxides the model takes, as MELTS's
/// `liq_struct_data.h` carries them, credited there to Lange and Carmichael 1990 (Rev. Mineral. 24,
/// ch. 2; closed, as is Lange and Carmichael 1987, GCA 51, 2931): its liquid components' "V liq"
/// and their `EOS_KRESS` first coefficients, J bar⁻¹ (10 cm³ mol⁻¹) and J bar⁻¹ K⁻¹, `SiO₂` 2.690,
/// `TiO₂` 2.316, `Al₂O₃` 3.711 and `Fe₂O₃` 4.213 directly, and `FeO`, `MgO`, `CaO`, `Na₂O` and K₂O
/// from `Fe₂SiO₄` 5.420, `Mg₂SiO₄` 4.980, `CaSiO₃` 4.347, `Na₂SiO₃` 5.568 and `KAlSiO₄` 6.8375 less
/// their `SiO₂` and `Al₂O₃` (the lane's copy of the MAGMA repository's file, SHA-256 206b298d…).
/// `TiO₂`'s 23.16 is that set's (decision-p14-t49h-step4-melts §6, corroborated by Ghiorso and
/// Kress 2004's 23.448, AJS 304, 679, Table 3): `DensityX`'s 28.32 at 1,773 K (Iacovino and Till
/// 2019, Volcanica 2, 1, Table 1, credited to Lange and Carmichael 1987) belongs to another set and
/// is not mixed in.
const LANGE_CARMICHAEL: [(Oxide, f64, f64); 9] = [
    (Oxide::Silica, 26.90, 0.0),
    (Oxide::Titania, 23.16, 7.246),
    (Oxide::Alumina, 37.11, 2.62),
    (Oxide::Ferric, 42.13, 9.09),
    (Oxide::Ferrous, 13.65, 2.92),
    (Oxide::Magnesia, 11.45, 2.62),
    (Oxide::Lime, 16.57, 2.92),
    (Oxide::Soda, 28.78, 7.41),
    (Oxide::Potash, 45.84, 11.91),
];

/// A melt's density at `temperature` and one bar, by Lange and Carmichael's ideal mixing of partial
/// molar volumes ([`LANGE_CARMICHAEL`]): ρ = Σ xᵢ Mᵢ ÷ Σ xᵢ [Vᵢ + (dVᵢ ÷ dT)(T − 1,673 K)], xᵢ the
/// mole fractions of the nine oxides it takes, renormalised over them, so that `MnO`, P₂O₅,
/// `Cr₂O₃`, `NiO` and H₂O are dropped (as `DensityX` does, Iacovino and Till 2019); `FeO` and
/// `Fe₂O₃` as the analysis gives them, so total iron as `FeO` where it reports no `Fe₂O₃`. The
/// compressibility is neglected at the surface (the module's note).
///
/// Lange and Carmichael's fit misses its own data by 0.117 cm³ mol⁻¹ rms for the iron-free melts
/// and 0.162 for the iron-bearing (Ghiorso and Kress 2004, Table 4), about 0.5% of ρ, over
/// 1,573–1,873 K; Alae lava lake's melt at 1,130 °C (Peck 1978, USGS Professional Paper 935-B, p.
/// B37: 2,740 kg m⁻³ for the melt fraction, from the lake's level surveys and its core's density,
/// ±30 on the bulk) it meets within 1%, as a test holds.
#[must_use]
pub(crate) fn lange_carmichael_density(
    oxides: &[(Oxide, f64)],
    temperature: Kelvin,
) -> KilogramsPerCubicMetre {
    let above = temperature.value() - LANGE_CARMICHAEL_REFERENCE.value();
    let mut total = 0.0;
    for &(oxide, _, _) in &LANGE_CARMICHAEL {
        total += moles(oxides, oxide);
    }
    let mut mass = 0.0;
    let mut volume = 0.0;
    for &(oxide, v_cm3, dv_dt_milli) in &LANGE_CARMICHAEL {
        let x = moles(oxides, oxide) / total;
        mass += x * oxide.molar_mass_g_per_mol();
        volume += x * (v_cm3 + 1e-3 * dv_dt_milli * above);
    }
    KilogramsPerCubicMetre::new(1e3 * mass / volume)
}

/// A melt's record at `temperature` ([`super::liquid_properties`]), `row` its lithology: the
/// density [`lange_carmichael_density`]'s, `Derived`; the viscosity
/// [`MeltViscosityModel::of`]'s model's, `Derived`, and absent where it is
/// [`MeltViscosityModel::Unruled`]; the index and the absorption [`HeldOptics`]', and the surface
/// tension the stated constant, each `StandIn`; no scattering. The basis is `StandIn`.
///
/// # Errors
///
/// [`ResolveLiquidError::OutOfRange`] below the row's first melt, its condensed columns' solidus,
/// or at a temperature that is not finite, with no upper end (`below` is `None`);
/// [`ResolveLiquidError::NotModelled`] for the density where the row has no condensed columns,
/// which a registry test holds never happens for a melt.
pub(super) fn melt_record(
    row: &Substance,
    melt: &SilicateMelt,
    temperature: Kelvin,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let key = row.key();
    let condensed = row.condensed().ok_or(ResolveLiquidError::NotModelled {
        key,
        member: LiquidMember::Density,
    })?;
    let lowest = condensed.transition().temperature();
    if !temperature.value().is_finite() || temperature < lowest {
        return Err(ResolveLiquidError::OutOfRange {
            key,
            temperature,
            lowest,
            below: None,
        });
    }
    let density = lange_carmichael_density(melt.oxides, temperature);
    let viscosity_pa_s = MeltViscosityModel::of(melt.oxides)
        .vft(melt.oxides)
        .map(|vft| math::exp10(vft.log10_viscosity_pa_s(temperature)));
    Ok(LiquidProperties {
        temperature,
        density_kg_m3: density.value(),
        basis: LiquidBasis::StandIn,
        refractive_index: *melt.optics.index,
        absorption_per_m: Some(*melt.optics.absorption_per_m),
        scattering_per_m: None,
        surface_tension_n_per_m: Some(melt.surface_tension_n_per_m),
        viscosity_pa_s,
    })
}

#[cfg(test)]
mod tests;
