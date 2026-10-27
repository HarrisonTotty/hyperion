//! The phase vocabulary the registry's phase diagrams share (decision-composition §1.1;
//! P14.T49.a): the molar gas constant, the [`Phase`] a substance lies in at a temperature and
//! pressure, and ice Ih's density.
//!
//! Where a substance condenses, and in which phase it lies, is its phase diagram's
//! ([`diagram`](super::diagram), P14.T49.b's fitted curves: each row's reference equations of
//! state, evaluated verbatim). Until the 22 → 23 batch's wiring (P14.T54.a) this module also held
//! P14.T49.a's phase columns, five constant-enthalpy Clausius–Clapeyron sets for water, methane,
//! nitrogen, carbon dioxide and argon (P14.T13.c's effective enthalpies, with methane's for
//! P14.T24.f); every caller now reads the diagrams, and the columns are retired.
//!
//! # Water's melting curves and an aqueous ocean's liquidus (P14.T51.g.b)
//!
//! A subsurface ocean's shell meets its liquid where the liquid's liquidus is (plan 14, P14.T51.g;
//! science-p14-subsurface-ocean §2.4). The crate-private functions here read it, for the ocean's
//! solve (`planetary::hooks::ocean`) and for P14.T51.m's coupled condensables after it:
//!
//! - **The pure curves**, read from the registry's water row (IAPWS R14-08(2011), Wagner,
//!   Riethmann, Feistel and Harvey 2011, J. Phys. Chem. Ref. Data 40, 043103, as its
//!   [`PhaseDiagram`] carries them; no coefficient is copied): ice Ih's melting pressure on the
//!   falling branch, down to the ice Ih–III–liquid triple point at 251.165 K and 208.566 MPa, its
//!   inverse by a fixed count of halvings, and the rising branch of the high-pressure ices III, V,
//!   VI and VII, each named by the piece of the curve that holds the temperature.
//! - **The solutes' 1-bar depressions** of the freezing point (`AqueousLiquidus`), from ice Ih's
//!   melting point at one atmosphere, 273.152 519 K (IAPWS R10-06(2009)): sodium chloride's by
//!   Bodnar 1993 and ammonia's by a published quintic, each scaled to end on its registry row's
//!   eutectic (`brine_nacl`, `nh3_h2o`), ammonia's never below water's dilute law. They add, the
//!   sum capped at the largest eutectic depression present, and a solute beyond its eutectic
//!   composition crystallises its excess. Their pressure dependence is neglected (the science
//!   ruling's §2.4).
//! - **Ice II's field and the slivers of ice V's and VI's**: whether water at a pressure and a
//!   temperature lies in another ice's field than the one named, by Hammond et al. 2016's Ih–II
//!   line, Bridgman 1912's triple points and R14-08's (decision-p14-t51gb-ice-ii): a cold
//!   solution meets ice II, whose liquidus is not modelled.

use crate::planetary::derive::atmosphere::Gas;
use crate::substance::condensed::TransitionKind;
use crate::substance::diagram::{MeltingCurve, MeltingPiece, PhaseDiagram};
use crate::substance::{SubstanceId, by_key};
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// The molar gas constant, J mol⁻¹ K⁻¹ (exact in the 2019 SI, the Avogadro constant times
/// Boltzmann's: 8.314 462 618 153 24).
pub const MOLAR_GAS_CONSTANT: f64 = 8.314_462_618_153_24;

/// The phase a substance lies in at a temperature and pressure
/// ([`phase_at`](super::diagram::phase_at)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    /// The solid: an ice, a frost or a crystal.
    Solid,
    /// The liquid.
    Liquid,
    /// The vapour, below the saturation pressure or above the critical temperature and below the
    /// critical pressure: the gas.
    Vapour,
    /// The supercritical fluid, at or above both the critical temperature and the critical
    /// pressure, which has no interface (Venus's carbon dioxide).
    Supercritical,
}

/// The melting point of ice Ih at 1 atm, kelvin: 273.152 519 K (IAPWS R10-06), to 0.01 K, where
/// [`ice_ih_density`] stops.
const ICE_IH_MELTING_K: f64 = 273.15;

/// Ice Ih's density at `temperature` and 1 atm: the Gibbs function of IAPWS R10-06(2009)
/// (Feistel and Wagner 2006, J. Phys. Chem. Ref. Data 35, 1021), as the polynomial
/// 933.790 351 + 9.911 602 x³ − 109.185 868 x⁴ + 127.136 897 x⁵ − 44.949 496 x⁶ in
/// x = T ÷ 273.15 K, fitted to it over 0.5–273.15 K within 0.02 kg m⁻³, below the release's own
/// uncertainty, 0.1 per cent below 268 K and 0.02 per cent above (its Table 5;
/// science-composition-crusts.md §3); clamped to 0 K and the melting point.
///
/// Moved here verbatim from the reference worlds' palettes by P14.T48.f, whose ice-rich crater
/// targets take it at the body's mean surface temperature (science-r09-craters.md F5.3);
/// P14.T49.b makes it water's `ρ_s`(T).
#[must_use]
pub fn ice_ih_density(temperature: Kelvin) -> KilogramsPerCubicMetre {
    let x = temperature.value().clamp(0.0, ICE_IH_MELTING_K) / 273.15;
    let x3 = x * x * x;
    KilogramsPerCubicMetre::new(
        933.790_351 + x3 * (9.911_602 + x * (-109.185_868 + x * (127.136_897 - 44.949_496 * x))),
    )
}

/// Ice Ih's melting point at one atmosphere, 101 325 Pa: 273.152 519 K (IAPWS R10-06(2009), §3
/// and Table 6; Feistel and Wagner 2006, J. Phys. Chem. Ref. Data 35, 1021), from which an aqueous
/// solute's 1-bar freezing-point depression is measured ([`AqueousLiquidus`]), so that its
/// liquidus at one atmosphere ends on its row's eutectic temperature.
pub(crate) const ICE_IH_ATMOSPHERIC_MELTING_POINT: Kelvin = Kelvin::new(273.152_519);

/// The halvings by which [`ice_ih_melting_temperature`] and [`AqueousLiquidus`] invert a curve: 64,
/// a fixed count, which close a bracket of tens of kelvin past f64's resolution of its ends.
pub(crate) const CURVE_HALVINGS: u32 = 64;

/// The registry's water row's phase diagram: IAPWS-95 with IAPWS R14-08's melting curves.
///
/// # Panics
///
/// Never: the water row carries its diagram, which the registry's tests hold.
#[must_use]
fn water() -> &'static PhaseDiagram {
    Gas::Water
        .substance()
        .substance()
        .phase_diagram()
        .expect("the registry's water row carries IAPWS-95's diagram")
}

/// The water row's melting curve's two branches: ice Ih's falling equation with the temperature it
/// falls to, and the rising pieces of the high-pressure ices.
///
/// # Panics
///
/// Never: water's curve falls from its triple point (IAPWS R14-08), as the registry holds it.
#[must_use]
fn water_melting() -> (Kelvin, &'static [MeltingPiece]) {
    match water().melting() {
        Some(&MeltingCurve::Falling { lowest, rising, .. }) => (lowest, rising),
        Some(MeltingCurve::Rising(_)) | None => {
            unreachable!("water's melting curve falls from its triple point (IAPWS R14-08)")
        }
    }
}

/// The foot of ice Ih's melting curve: the ice Ih–III–liquid triple point, where the falling
/// branch meets the rising one, 251.165 K and 208.566 MPa (IAPWS R14-08(2011), Table 1), as the
/// registry's water row gives it: the lowest temperature at which water is liquid, and the highest
/// pressure at which ice Ih melts.
///
/// # Panics
///
/// Never: the rising branch starts at the falling branch's foot.
#[must_use]
pub(crate) fn ice_ih_foot() -> (Kelvin, Pascals) {
    let (lowest, _) = water_melting();
    let pressure = water()
        .melting_pressure(lowest)
        .expect("the rising branch starts at the falling branch's foot");
    (lowest, pressure)
}

/// Water's triple point's temperature, 273.16 K: the top of ice Ih's melting curve, at its
/// triple pressure, 611.657 Pa (IAPWS R14-08(2011)).
#[must_use]
pub(crate) fn water_triple_temperature() -> Kelvin {
    water().triple_temperature()
}

/// Ice Ih's melting pressure at `temperature` (IAPWS R14-08(2011), Eq. 1, as the registry's water
/// row carries it): from the triple point's 611.657 Pa at 273.16 K up to 208.566 MPa at the foot,
/// 251.165 K ([`ice_ih_foot`]). Pressure melts ice Ih: at or above it the ice is liquid. `None`
/// outside the falling branch.
#[must_use]
pub(crate) fn ice_ih_melting_pressure(temperature: Kelvin) -> Option<Pascals> {
    water().falling_melting_pressure(temperature)
}

/// The temperature at which ice Ih melts under `pressure`: the inverse of
/// [`ice_ih_melting_pressure`], by [`CURVE_HALVINGS`] halvings between the foot and the triple
/// point, the curve falling monotonically between them. A pressure at or below the triple
/// pressure gives the triple temperature, and one at or above the foot's pressure the foot's
/// temperature: the curve's ends, outside which no ice Ih melts.
#[must_use]
pub(crate) fn ice_ih_melting_temperature(pressure: Pascals) -> Kelvin {
    let (foot, _) = ice_ih_foot();
    let top = water_triple_temperature();
    let at = |t: f64| ice_ih_melting_pressure(Kelvin::new(t)).map_or(f64::NAN, Pascals::value);
    let p = pressure.value();
    let (mut lo, mut hi) = (foot.value(), top.value());
    // A pressure that is not a number is no melting point: the curve's foot.
    if p.is_nan() || p >= at(lo) {
        return foot;
    }
    if p <= at(hi) {
        return top;
    }
    for _ in 0..CURVE_HALVINGS {
        let mid = f64::midpoint(lo, hi);
        if at(mid) > p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Kelvin::new(f64::midpoint(lo, hi))
}

/// A high-pressure ice of water that meets its liquid: the pieces of IAPWS R14-08(2011)'s rising
/// melting curve, in order of temperature (its Table 1's triple points: III from 251.165 K, V
/// from 256.164 K, VI from 273.31 K and VII from 355 K to 715 K).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum HighPressureIce {
    /// Ice III, from the Ih–III–liquid point to the III–V–liquid point.
    IceIii,
    /// Ice V, to the V–VI–liquid point.
    IceV,
    /// Ice VI, to the VI–VII–liquid point.
    IceVi,
    /// Ice VII, to 715 K, the highest measurement.
    IceVii,
}

/// The high-pressure ices in the order of the water row's rising pieces (IAPWS R14-08(2011)),
/// which a test holds to the pieces' starts.
const HIGH_PRESSURE_ICES: [HighPressureIce; 4] = [
    HighPressureIce::IceIii,
    HighPressureIce::IceV,
    HighPressureIce::IceVi,
    HighPressureIce::IceVii,
];

/// Water's melting pressure at `temperature` on its rising branch, with the high-pressure ice
/// whose piece of the curve holds it (IAPWS R14-08(2011)'s Eqs. 2–5, as the registry's water row
/// carries them, bit for bit [`PhaseDiagram::melting_pressure`]'s): at or above the pressure the
/// liquid freezes to that ice. `None` outside the branch, below the foot's 251.165 K and above
/// ice VII's 715 K.
#[must_use]
pub(crate) fn high_pressure_melting(temperature: Kelvin) -> Option<(Pascals, HighPressureIce)> {
    let (_, rising) = water_melting();
    let index = rising
        .iter()
        .rposition(|piece| temperature >= piece.start())?;
    let piece = &rising[index];
    if piece.end().is_some_and(|end| temperature > end) {
        return None;
    }
    let ice = *HIGH_PRESSURE_ICES.get(index)?;
    Some((piece.equation().pressure(temperature), ice))
}

/// A solution row's eutectic at one atmosphere: its temperature and its solute's mass fraction
/// there (the row's [`TransitionKind::Eutectic`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Eutectic {
    /// The eutectic's temperature.
    pub(crate) temperature: Kelvin,
    /// The solute's mass fraction of the liquid there, in (0, 1).
    pub(crate) solute_mass_fraction: f64,
}

impl Eutectic {
    /// The eutectic of the registry row `id`, `None` for a row whose first melt is no eutectic.
    #[must_use]
    pub(crate) fn of_row(id: SubstanceId) -> Option<Self> {
        let transition = id.substance().condensed()?.transition();
        match transition.kind() {
            TransitionKind::Eutectic {
                solute_mass_fraction,
            } => Some(Self {
                temperature: transition.temperature(),
                solute_mass_fraction,
            }),
            TransitionKind::Melting
            | TransitionKind::Solidus
            | TransitionKind::Decomposition
            | TransitionKind::Sublimation
            | TransitionKind::TriplePoint => None,
        }
    }
}

/// Water's cryoscopic constant, K kg mol⁻¹: 1.86, the freezing-point depression of a dilute
/// aqueous solution per mole of solute in a kilogram of water (R `T_m`² `M_w` ÷ `ΔH_fus`, which
/// the registry's water row gives as 1.860 at 273.15 K; decision-p14-t51g Q8).
pub(crate) const WATER_CRYOSCOPIC_CONSTANT: f64 = 1.86;

/// Bodnar 1993's salinity of the NaCl–H₂O system at its freezing point, wt% NaCl, at the
/// depression `depression` in kelvin below pure water's: 1.78θ − 0.0442θ² + 0.000557θ³
/// (Bodnar 1993, Geochim. Cosmochim. Acta 57, 683, for 0–23.3 wt%; decision-p14-t51g Q8). It
/// rises monotonically (its slope's discriminant is negative), reaching 23.3 wt% at θ = 21.39 K.
#[must_use]
fn bodnar_salinity(depression: f64) -> f64 {
    depression * (1.78 + depression * (-0.044_2 + depression * 0.000_557))
}

/// The depression θ at which [`bodnar_salinity`] is `mass_fraction` × 100 wt%, by
/// [`CURVE_HALVINGS`] halvings of [0, 30 K] (Bodnar's curve reaches 28.7 wt% at 30 K, past every
/// fraction it is read at).
#[must_use]
fn bodnar_depression(mass_fraction: f64) -> f64 {
    let salinity = 100.0 * mass_fraction;
    // Fresh water's depression is none, exactly.
    if salinity.is_nan() || salinity <= 0.0 {
        return 0.0;
    }
    let (mut lo, mut hi) = (0.0, 30.0);
    for _ in 0..CURVE_HALVINGS {
        let mid = f64::midpoint(lo, hi);
        if bodnar_salinity(mid) < salinity {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::midpoint(lo, hi)
}

/// The NH₃–H₂O liquidus's depression below pure water's freezing point, K, at the ammonia mass
/// fraction `x`: 53.07x + 1651.4x² − 11,842x³ + 46,269x⁴ − 56,695x⁵, Croft, Lunine and Kargel
/// 1988's eq. (3a) (Icarus 73, 279), as the PISTES code (`core/ammonia_melting.m`) transcribes it
/// "valid for X<0.329", the same coefficients as the science ruling's prototype; the printed page
/// was not read, so they stand unverified (decision-p14-t51g Q8). It rises monotonically to
/// x = 0.335, reaching
/// 93.55 K at the `nh3_h2o` row's 0.321, and its own 97.0 K (176.15 K) at 0.327; its slope at 0,
/// 53 K, is half the dilute law's, which [`AqueousLiquidus::ammonia_depression_k`] keeps as its
/// floor.
#[must_use]
fn ammonia_quintic(x: f64) -> f64 {
    x * (53.07 + x * (1_651.4 + x * (-11_842.0 + x * (46_269.0 - 56_695.0 * x))))
}

/// The 1-bar liquidus of an aqueous ocean's solutes, sodium chloride and ammonia (P14.T51.g.b;
/// science-p14-subsurface-ocean §2.4, decision-p14-t51g Q8), as depressions of the freezing point
/// below ice Ih's at one atmosphere ([`ICE_IH_ATMOSPHERIC_MELTING_POINT`]), each ending on its
/// registry row's eutectic: built once from the registry, then read at each composition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AqueousLiquidus {
    /// The `brine_nacl` row.
    sodium_chloride_row: SubstanceId,
    /// Its eutectic, 251.9 K at 0.233 (Tsironi et al. 2020).
    sodium_chloride: Eutectic,
    /// The factor that ends Bodnar's curve on it: (273.152 519 − 251.9) ÷ 21.386 = 0.99374 (the
    /// ruling's 0.9936 divides by the rounded 21.39).
    bodnar_scale: f64,
    /// The `nh3_h2o` row.
    ammonia_row: SubstanceId,
    /// Its eutectic, 176.15 K at 0.321 (P14.T49.b's row).
    ammonia: Eutectic,
    /// The factor that ends the quintic on it: 97.0025 ÷ 93.55 = 1.037.
    quintic_scale: f64,
    /// Ammonia's molar mass, kg mol⁻¹: the registry's 17.031 g mol⁻¹.
    ammonia_molar_mass_kg_per_mol: f64,
}

impl AqueousLiquidus {
    /// The liquidus as the registry gives it: the two solution rows' eutectics and ammonia's molar
    /// mass.
    ///
    /// # Panics
    ///
    /// Never: the registry holds both rows with their eutectics, and ammonia's molar mass, which
    /// this module's tests read.
    #[must_use]
    pub(crate) fn of_registry() -> Self {
        let row = |key: &str| {
            let id = by_key(key).expect("the registry holds both solution rows");
            let eutectic = Eutectic::of_row(id).expect("both solution rows end at a eutectic");
            (id, eutectic)
        };
        let (sodium_chloride_row, sodium_chloride) = row("brine_nacl");
        let (ammonia_row, ammonia) = row("nh3_h2o");
        let reference = ICE_IH_ATMOSPHERIC_MELTING_POINT.value();
        let bodnar_scale = (reference - sodium_chloride.temperature.value())
            / bodnar_depression(sodium_chloride.solute_mass_fraction);
        let quintic_scale = (reference - ammonia.temperature.value())
            / ammonia_quintic(ammonia.solute_mass_fraction);
        let ammonia_molar_mass_kg_per_mol = Gas::Ammonia
            .substance()
            .substance()
            .molar_mass_g_per_mol()
            .expect("ammonia's row has its molar mass")
            / 1_000.0;
        Self {
            sodium_chloride_row,
            sodium_chloride,
            bodnar_scale,
            ammonia_row,
            ammonia,
            quintic_scale,
            ammonia_molar_mass_kg_per_mol,
        }
    }

    /// The `brine_nacl` row, the liquid of an ocean whose sodium chloride depresses it most.
    #[must_use]
    pub(crate) const fn sodium_chloride_row(&self) -> SubstanceId {
        self.sodium_chloride_row
    }

    /// The `nh3_h2o` row, the liquid of an ocean whose ammonia depresses it most.
    #[must_use]
    pub(crate) const fn ammonia_row(&self) -> SubstanceId {
        self.ammonia_row
    }

    /// Sodium chloride's eutectic.
    #[must_use]
    pub(crate) const fn sodium_chloride_eutectic(&self) -> Eutectic {
        self.sodium_chloride
    }

    /// Ammonia's eutectic.
    #[must_use]
    pub(crate) const fn ammonia_eutectic(&self) -> Eutectic {
        self.ammonia
    }

    /// The freezing-point depression of water holding sodium chloride at the mass fraction
    /// `mass_fraction`, K: Bodnar 1993's, scaled by 0.9937 to end on the `brine_nacl` row's
    /// eutectic, 251.9 K at 0.233; the fraction held at the eutectic's beyond it, where the excess
    /// crystallises, and at 0 below 0.
    #[must_use]
    pub(crate) fn sodium_chloride_depression_k(&self, mass_fraction: f64) -> f64 {
        let w = held(mass_fraction, self.sodium_chloride.solute_mass_fraction);
        self.bodnar_scale * bodnar_depression(w)
    }

    /// The freezing-point depression of water holding ammonia at the mass fraction
    /// `mass_fraction`, K: the larger of the quintic scaled by 1.037 to end on the `nh3_h2o` row's
    /// eutectic, 176.15 K at 0.321, and water's dilute law, `K_f` w ÷ (`M_NH₃` (1 − w))
    /// ([`WATER_CRYOSCOPIC_CONSTANT`]), which a liquidus may not fall below (the two cross near
    /// w = 0.05); the fraction held at the eutectic's beyond it and at 0 below 0.
    #[must_use]
    pub(crate) fn ammonia_depression_k(&self, mass_fraction: f64) -> f64 {
        let w = held(mass_fraction, self.ammonia.solute_mass_fraction);
        let quintic = self.quintic_scale * ammonia_quintic(w);
        let dilute =
            WATER_CRYOSCOPIC_CONSTANT * w / (self.ammonia_molar_mass_kg_per_mol * (1.0 - w));
        if quintic > dilute { quintic } else { dilute }
    }

    /// The largest eutectic depression among the solutes present, K: sodium chloride's 21.25 K
    /// where its amount `sodium_chloride` (a mass or a fraction) is positive, and ammonia's 97.0 K
    /// where `ammonia` is; 0 with neither.
    #[must_use]
    pub(crate) fn eutectic_depression_k(&self, sodium_chloride: f64, ammonia: f64) -> f64 {
        let reference = ICE_IH_ATMOSPHERIC_MELTING_POINT.value();
        let of = |amount: f64, eutectic: Eutectic| {
            if amount > 0.0 {
                reference - eutectic.temperature.value()
            } else {
                0.0
            }
        };
        let (n, a) = (
            of(sodium_chloride, self.sodium_chloride),
            of(ammonia, self.ammonia),
        );
        if n > a { n } else { a }
    }

    /// The freezing-point depression of water holding sodium chloride and ammonia at the mass
    /// fractions `sodium_chloride` and `ammonia` of the liquid, K: their own depressions added,
    /// the sum capped at the largest eutectic depression of the solutes present
    /// ([`eutectic_depression_k`](Self::eutectic_depression_k)), where the liquid is eutectic.
    #[must_use]
    pub(crate) fn depression_k(&self, sodium_chloride: f64, ammonia: f64) -> f64 {
        let sum =
            self.sodium_chloride_depression_k(sodium_chloride) + self.ammonia_depression_k(ammonia);
        let cap = self.eutectic_depression_k(sodium_chloride, ammonia);
        if sum < cap { sum } else { cap }
    }
}

/// Ice II's boundary with ice Ih at low pressure, P = 14 MPa + 0.827 MPa K⁻¹ × T (Hammond, Barr
/// and Parmentier 2016, GRL 43, 6775, eq. 5, after Bridgman 1912, Whalley 1984 and Leon et al.
/// 2002; read in its arXiv text, 1606.04840): its intercept, Pa.
const ICE_II_INTERCEPT_PA: f64 = 14e6;

/// The slope of ice II's boundary with ice Ih ([`ICE_II_INTERCEPT_PA`]), Pa K⁻¹.
const ICE_II_SLOPE_PA_PER_K: f64 = 0.827e6;

/// One kilogram-force per square centimetre in pascals, Bridgman's unit: 98,066.5 Pa (exact, by the
/// standard gravity).
const PA_PER_KGF_PER_CM2: f64 = 98_066.5;

/// A triple point as Bridgman gives it, (°C, kg cm⁻²), in (Pa, K).
#[must_use]
const fn bridgman(celsius: f64, kgf_per_cm2: f64) -> (f64, f64) {
    (kgf_per_cm2 * PA_PER_KGF_PER_CM2, celsius + 273.15)
}

/// Ice Ih–II–III's triple point, (Pa, K): "II–III–I −34.7 2170" (Bridgman 1912, Proc. Am. Acad.
/// Arts Sci. 47, 441, Table XXVIII, p. 524, measured; read from its pages for
/// decision-p14-t51gb-ice-ii), 212.80 MPa and 238.45 K.
pub(crate) const ICE_IH_II_III: (f64, f64) = bridgman(-34.7, 2_170.0);

/// Ice II–III–V's triple point, (Pa, K): "V–II–III −24.3 3510" (Bridgman 1912, Table XXVIII,
/// measured), 344.21 MPa and 248.85 K, the warmest point of ice II's field.
pub(crate) const ICE_II_III_V: (f64, f64) = bridgman(-24.3, 3_510.0);

/// Ice II–V–VI's triple point, (Pa, K): Bridgman's estimate, "apparently … at about −65° and 6300
/// kgm" (1912, p. 513), never measured (`SeaFreeze` predicts 209.1 K at 670.8 MPa, Journaux et al.
/// 2020): 617.82 MPa and 208.15 K, where ice II's field ends.
pub(crate) const ICE_II_V_VI: (f64, f64) = bridgman(-65.0, 6_300.0);

/// The temperature on the straight line through `(p0, t0)` and `(p1, t1)` at the pressure `p`.
#[must_use]
fn temperature_on(p: f64, (p0, t0): (f64, f64), (p1, t1): (f64, f64)) -> f64 {
    t0 + (t1 - t0) * (p - p0) / (p1 - p0)
}

/// The pressure on the straight line through `(p0, t0)` and `(p1, t1)` at the temperature `t`.
#[must_use]
fn pressure_on(t: f64, (p0, t0): (f64, f64), (p1, t1): (f64, f64)) -> f64 {
    p0 + (p1 - p0) * (t - t0) / (t1 - t0)
}

/// Where Hammond et al.'s Ih–II line meets the II–III line through Bridgman's Ih–II–III and
/// II–III–V points, (Pa, K): 211.09 MPa and 238.31 K, below which ice II's warm edge is Hammond's
/// line (decision-p14-t51gb-ice-ii §1.1).
#[must_use]
pub(crate) fn ice_ii_edge_meeting() -> (f64, f64) {
    let ((p0, t0), (p1, t1)) = (ICE_IH_II_III, ICE_II_III_V);
    // T = (p − a) ÷ s on Hammond's line, T = t0 + m (p − p0) on the II–III line.
    let (a, s, m) = (
        ICE_II_INTERCEPT_PA,
        ICE_II_SLOPE_PA_PER_K,
        (t1 - t0) / (p1 - p0),
    );
    let p = (t0 - m * p0 + a / s) / (1.0 / s - m);
    (p, (p - a) / s)
}

/// Ice II's warm edge `E_II` at `pressure`, the warmest temperature of its field there
/// (decision-p14-t51gb-ice-ii §4.1): Hammond et al. 2016's Ih–II line up to its meeting with the
/// II–III line ([`ice_ii_edge_meeting`]), then straight lines through Bridgman's Ih–II–III and
/// II–III–V points ([`ICE_IH_II_III`], [`ICE_II_III_V`]), and on to his II–V–VI estimate
/// ([`ICE_II_V_VI`]). `None` where no ice II stands: below the Ih–II line's 14 MPa, beyond the
/// II–V–VI point, and for a pressure that is not a number.
#[must_use]
pub(crate) fn ice_ii_warmest(pressure: Pascals) -> Option<Kelvin> {
    let p = pressure.value();
    if p.is_nan() || p < ICE_II_INTERCEPT_PA || p > ICE_II_V_VI.0 {
        return None;
    }
    let (meeting, _) = ice_ii_edge_meeting();
    let t = if p <= meeting {
        (p - ICE_II_INTERCEPT_PA) / ICE_II_SLOPE_PA_PER_K
    } else if p <= ICE_II_III_V.0 {
        temperature_on(p, ICE_IH_II_III, ICE_II_III_V)
    } else {
        temperature_on(p, ICE_II_III_V, ICE_II_V_VI)
    };
    Some(Kelvin::new(t))
}

/// The ice III–V line's pressure at `temperature`, Pa, `P_III–V`: straight from Bridgman's II–III–V
/// point to IAPWS R14-08's III–V–liquid point (350.1 MPa at 256.164 K, the registry's water row's
/// ice V piece's start), run on as a straight line beyond them.
///
/// # Panics
///
/// Never: the registry's water row carries the rising branch, which a test holds.
#[must_use]
pub(crate) fn ice_iii_v_pressure(temperature: Kelvin) -> Pascals {
    Pascals::new(pressure_on(
        temperature.value(),
        ICE_II_III_V,
        liquid_triple_point(1),
    ))
}

/// The ice V–VI line's pressure at `temperature`, Pa, `P_V–VI`: straight from Bridgman's II–V–VI
/// estimate to IAPWS R14-08's V–VI–liquid point (632.4 MPa at 273.31 K, the registry's water row's
/// ice VI piece's start), run on as a straight line beyond them.
///
/// # Panics
///
/// Never: the registry's water row carries the rising branch, which a test holds.
#[must_use]
pub(crate) fn ice_v_vi_pressure(temperature: Kelvin) -> Pascals {
    Pascals::new(pressure_on(
        temperature.value(),
        ICE_II_V_VI,
        liquid_triple_point(2),
    ))
}

/// The liquid triple point where the rising branch's piece `index` starts (IAPWS R14-08's, as the
/// registry's water row carries them), (Pa, K): 1 is III–V–liquid, 2 is V–VI–liquid.
///
/// # Panics
///
/// For an index past the rising branch's four pieces, a broken invariant.
#[must_use]
fn liquid_triple_point(index: usize) -> (f64, f64) {
    let (_, rising) = water_melting();
    let start = rising[index].start();
    (
        rising[index].equation().pressure(start).value(),
        start.value(),
    )
}

/// The ice whose field liquid water's ice would lie in at (`pressure`, `temperature`) in place of
/// the ice `named` (decision-p14-t51gb-ice-ii §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ForeignIce {
    /// Ice II's field: colder than its warm edge ([`ice_ii_warmest`]). Ice II meets the liquid of a
    /// cold solution, whose liquidus is not modelled.
    IceIi,
    /// Ice V's field, for ice III named beyond the II–III–V point's pressure and the III–V line.
    IceV,
    /// Ice VI's field, beyond the V–VI line.
    IceVi,
}

/// Whether water at `pressure` and `temperature` on the ice `named` (`None` for rock, or for a
/// shell's base, which only ice II's field can take) lies in another ice's field, and whose: ice
/// II's, colder than its warm edge; ice VI's, beyond the V–VI line ([`ice_v_vi_pressure`]); and,
/// for ice III named, ice V's, beyond the II–III–V point's pressure and the III–V line
/// ([`ice_iii_v_pressure`]). The one test of a subsurface ocean's base and floor
/// (decision-p14-t51gb-ice-ii §4.1): nothing between them is tested, since a column that only
/// crosses ice II's field, its floor beyond it, is right.
#[must_use]
pub(crate) fn foreign_ice(
    pressure: Pascals,
    temperature: Kelvin,
    named: Option<HighPressureIce>,
) -> Option<ForeignIce> {
    let (p, t) = (pressure.value(), temperature.value());
    if ice_ii_warmest(pressure).is_some_and(|edge| t < edge.value()) {
        return Some(ForeignIce::IceIi);
    }
    if p > ice_v_vi_pressure(temperature).value() {
        return Some(ForeignIce::IceVi);
    }
    let named_iii = matches!(named, Some(HighPressureIce::IceIii));
    if named_iii && p > ICE_II_III_V.0 && p > ice_iii_v_pressure(temperature).value() {
        return Some(ForeignIce::IceV);
    }
    None
}

/// `w` held to [0, `eutectic`]: a solute's mass fraction of the liquid, its excess beyond its
/// eutectic composition crystallised; a fraction that is not a number reads as none.
#[must_use]
fn held(w: f64, eutectic: f64) -> f64 {
    if w > eutectic {
        eutectic
    } else if w > 0.0 {
        w
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// Ice Ih's density follows IAPWS R10-06(2009) to 0.03 kg m⁻³ (science-composition-crusts.md
    /// §3), clamped to its melting point and to 0 K.
    #[test]
    fn ice_ih_s_density_follows_iapws() {
        let at = |k: f64| ice_ih_density(Kelvin::new(k)).value();
        for (k, rho) in [(273.15, 916.72), (100.0, 933.05), (160.0, 929.87)] {
            assert!((at(k) - rho).abs() < 0.03, "{} kg m⁻³ at {k} K", at(k));
        }
        assert_same_bits(at(300.0), at(ICE_IH_MELTING_K));
        assert_same_bits(at(-1.0), at(0.0));
    }

    /// Ice Ih's curve runs from the triple point's 611.657 Pa at 273.16 K to the foot's 208.566 MPa
    /// at 251.165 K (IAPWS R14-08(2011), Table 1, to 10⁻⁵), and its inverse returns each pressure's
    /// temperature to 10⁻⁹ K, its ends beyond the curve.
    #[test]
    fn ice_ih_s_melting_temperature_inverts_iapws() {
        let (foot, foot_p) = ice_ih_foot();
        assert_same_bits(foot.value(), 251.165);
        assert!(
            (foot_p.value() / 208.566e6 - 1.0).abs() < 1e-12,
            "{foot_p:?}"
        );
        let top = water_triple_temperature();
        assert_same_bits(top.value(), 273.16);
        let p_at = |t: f64| ice_ih_melting_pressure(Kelvin::new(t)).unwrap().value();
        assert!((p_at(273.16) / 611.657 - 1.0).abs() < 1e-12);
        assert!(
            (p_at(251.165) / 208.566e6 - 1.0).abs() < 1e-5,
            "{}",
            p_at(251.165)
        );
        for t in [251.2, 255.0, 260.0, 265.0, 270.0, 272.5, 273.1, 273.159] {
            let back = ice_ih_melting_temperature(Pascals::new(p_at(t))).value();
            assert!((back - t).abs() < 1e-9, "{t} K: {back}");
        }
        assert_eq!(ice_ih_melting_temperature(Pascals::new(100.0)), top);
        assert_eq!(ice_ih_melting_temperature(Pascals::new(300e6)), foot);
        assert_eq!(ice_ih_melting_temperature(Pascals::new(f64::NAN)), foot);
        assert_eq!(ice_ih_melting_pressure(Kelvin::new(250.0)), None);
    }

    /// The rising branch is the registry's to the bit, each high-pressure ice named by its piece:
    /// the pieces meet at IAPWS R14-08's triple points (III–V–liquid 256.164 K and 350.1 MPa,
    /// V–VI–liquid 273.31 K and 632.4 MPa, VI–VII–liquid 355 K and 2,216 MPa) to 10⁻⁵, and ice V
    /// melts at 629.34 MPa at 273.16 K, the hard bound of an ocean's floor (decision-p14-t51g Q6).
    #[test]
    fn the_high_pressure_ices_meet_the_liquid_at_iapws_s_triple_points() {
        let (_, rising) = water_melting();
        assert_eq!(rising.len(), HIGH_PRESSURE_ICES.len());
        for (piece, start) in rising.iter().zip([251.165, 256.164, 273.31, 355.0]) {
            assert_same_bits(piece.start().value(), start);
        }
        for t in [
            251.165, 252.0, 256.164, 260.0, 273.16, 273.31, 300.0, 355.0, 715.0,
        ] {
            let (p, _) = high_pressure_melting(Kelvin::new(t)).unwrap();
            let registry = water().melting_pressure(Kelvin::new(t)).unwrap();
            assert_same_bits(p.value(), registry.value());
        }
        let ice = |t: f64| high_pressure_melting(Kelvin::new(t)).unwrap().1;
        assert_eq!(ice(251.165), HighPressureIce::IceIii);
        assert_eq!(ice(256.0), HighPressureIce::IceIii);
        assert_eq!(ice(256.164), HighPressureIce::IceV);
        assert_eq!(ice(273.16), HighPressureIce::IceV);
        assert_eq!(ice(273.31), HighPressureIce::IceVi);
        assert_eq!(ice(355.0), HighPressureIce::IceVii);
        let below =
            |t: f64, index: usize| rising[index].equation().pressure(Kelvin::new(t)).value();
        for (index, (t, mpa)) in [(256.164, 350.1), (273.31, 632.4), (355.0, 2_216.0)]
            .into_iter()
            .enumerate()
        {
            let p = below(t, index);
            assert!((p / (mpa * 1e6) - 1.0).abs() < 1e-5, "{t} K: {p} Pa");
        }
        let (bound, _) = high_pressure_melting(Kelvin::new(273.16)).unwrap();
        assert!((bound.value() / 1e6 - 629.34).abs() < 0.005, "{bound:?}");
        assert_eq!(high_pressure_melting(Kelvin::new(251.0)), None);
        assert_eq!(high_pressure_melting(Kelvin::new(716.0)), None);
    }

    /// Each solute's liquidus ends on its registry row's eutectic at one atmosphere to 10⁻⁹ K
    /// (`brine_nacl`'s 251.9 K at 0.233, `nh3_h2o`'s 176.15 K at 0.321), holding there beyond it;
    /// Bodnar's curve takes 0.9936 to end there (it reaches 0.233 at 21.39 K), and the quintic
    /// 1.037 (decision-p14-t51g Q8).
    #[test]
    fn each_liquidus_ends_at_its_eutectic() {
        let liquidus = AqueousLiquidus::of_registry();
        let reference = ICE_IH_ATMOSPHERIC_MELTING_POINT.value();
        let nacl = liquidus.sodium_chloride_eutectic();
        let nh3 = liquidus.ammonia_eutectic();
        assert_same_bits(nacl.temperature.value(), 251.9);
        assert_same_bits(nacl.solute_mass_fraction, 0.233);
        assert_same_bits(nh3.temperature.value(), 176.15);
        assert_same_bits(nh3.solute_mass_fraction, 0.321);
        let end = |depression: f64| reference - depression;
        let at_nacl = end(liquidus.sodium_chloride_depression_k(0.233));
        let at_nh3 = end(liquidus.ammonia_depression_k(0.321));
        assert!((at_nacl - 251.9).abs() < 1e-9, "{at_nacl}");
        assert!((at_nh3 - 176.15).abs() < 1e-9, "{at_nh3}");
        assert_same_bits(
            liquidus.sodium_chloride_depression_k(0.5),
            liquidus.sodium_chloride_depression_k(0.233),
        );
        assert_same_bits(
            liquidus.ammonia_depression_k(0.9),
            liquidus.ammonia_depression_k(0.321),
        );
        assert_same_bits(liquidus.sodium_chloride_depression_k(0.0), 0.0);
        assert_same_bits(liquidus.ammonia_depression_k(-1.0), 0.0);
        assert!((bodnar_depression(0.233) - 21.39).abs() < 0.005);
        assert!(
            (liquidus.bodnar_scale - 0.9936).abs() < 5e-4,
            "{}",
            liquidus.bodnar_scale
        );
        assert!(
            (liquidus.quintic_scale - 1.037).abs() < 5e-4,
            "{}",
            liquidus.quintic_scale
        );
        // Bodnar's own curve: 3.5 wt% (seawater's salt) freezes about 2.1 K below pure water.
        let sea = liquidus.sodium_chloride_depression_k(0.035);
        assert!((sea - 2.08).abs() < 0.05, "{sea}");
    }

    /// The ammonia depression is never below water's dilute law, `K_f` w ÷ (`M_NH₃` (1 − w)), with
    /// `K_f` the registry water row's R `T_m`² `M_w` ÷ `ΔH_fus` = 1.86 K kg mol⁻¹; the two cross near
    /// w = 0.05; the scaled quintic puts 25 wt% at 214.23 K, 2.1 K under its unscaled 216.3 K
    /// (decision-p14-t51g Q8 wrote 1.4 K), and the unscaled one reaches its own 176.15 K at 0.327.
    #[test]
    fn the_ammonia_depression_is_at_least_the_dilute_law() {
        let water = Gas::Water.substance().substance();
        let diagram = water.phase_diagram().unwrap();
        let m_w = water.molar_mass_g_per_mol().unwrap() / 1_000.0;
        let k_f = MOLAR_GAS_CONSTANT * 273.15 * 273.15 * m_w
            / diagram.triple_fusion_enthalpy_j_per_mol().unwrap();
        assert!((k_f - WATER_CRYOSCOPIC_CONSTANT).abs() < 0.001, "{k_f}");
        let liquidus = AqueousLiquidus::of_registry();
        let dilute = |w: f64| WATER_CRYOSCOPIC_CONSTANT * w / (0.017_031 * (1.0 - w));
        let mut crossed = None;
        for step in 1..=320 {
            let w = f64::from(step) * 0.001;
            let depression = liquidus.ammonia_depression_k(w);
            // To the rounding of the registry's 17.031 g mol⁻¹ in kilograms.
            assert!(depression >= dilute(w) * (1.0 - 1e-12), "{w}: {depression}");
            if crossed.is_none() && depression > dilute(w) * (1.0 + 1e-9) {
                crossed = Some(w);
            }
        }
        let crossed = crossed.unwrap();
        assert!((0.04..0.07).contains(&crossed), "{crossed}");
        let unscaled = 273.15 - ammonia_quintic(0.25);
        assert!((unscaled - 216.3).abs() < 0.05, "{unscaled}");
        let scaled = ICE_IH_ATMOSPHERIC_MELTING_POINT.value() - liquidus.ammonia_depression_k(0.25);
        assert!((scaled - 214.23).abs() < 0.05, "{scaled}");
        assert!((273.15 - ammonia_quintic(0.327) - 176.15).abs() < 0.1);
        // Monotone over the whole range.
        let mut last = 0.0;
        for step in 0..=321 {
            let w = f64::from(step) * 0.001;
            let depression = liquidus.ammonia_depression_k(w);
            assert!(depression >= last, "{w}");
            last = depression;
        }
    }

    /// Bridgman's kg cm⁻² and °C in pascals and kelvin (98,066.5 Pa per kg cm⁻², 273.15 K at 0 °C):
    /// Ih–II–III at 212.80 MPa and 238.45 K, II–III–V at 344.21 MPa and 248.85 K, and II–V–VI at
    /// 617.82 MPa and 208.15 K; Hammond's line meets the II–III line at 211.09 MPa and 238.31 K.
    #[test]
    fn bridgman_s_triple_points_convert_to_pascals_and_kelvin() {
        for ((p, t), (mpa, k)) in [
            (ICE_IH_II_III, (212.80, 238.45)),
            (ICE_II_III_V, (344.21, 248.85)),
            (ICE_II_V_VI, (617.82, 208.15)),
        ] {
            assert!((p / 1e6 - mpa).abs() < 0.005, "{p} Pa");
            assert!((t - k).abs() < 1e-9, "{t} K");
        }
        assert_same_bits(ICE_IH_II_III.0, 2_170.0 * 98_066.5);
        let (p, t) = ice_ii_edge_meeting();
        assert!(
            (p / 1e6 - 211.09).abs() < 0.005 && (t - 238.31).abs() < 0.005,
            "{p} {t}"
        );
    }

    /// Ice II's warm edge `E_II` is Hammond et al.'s line to its 211.09 MPa meeting, and passes
    /// through Bridgman's three points to 10⁻⁹ K; it is continuous, rises to 344.21 MPa and falls
    /// after it, with none below 14 MPa or beyond 617.82 MPa. The III–V and V–VI lines run through
    /// their ends, R14-08's from the registry bit for bit (decision-p14-t51gb-ice-ii §4.4 (1)).
    #[test]
    fn ice_ii_s_field_has_its_warm_edge_through_bridgman_s_points() {
        let at = |p: f64| ice_ii_warmest(Pascals::new(p)).map(Kelvin::value);
        assert_eq!(at(13.9e6), None);
        assert_eq!(at(ICE_II_V_VI.0 * (1.0 + 1e-12)), None);
        assert!((at(208.566e6).unwrap() - (208.566 - 14.0) / 0.827).abs() < 1e-9);
        let (meeting, t_meeting) = ice_ii_edge_meeting();
        for (p, t) in [
            (meeting, t_meeting),
            ICE_IH_II_III,
            ICE_II_III_V,
            ICE_II_V_VI,
        ] {
            assert!((at(p).unwrap() - t).abs() < 1e-9, "{p} Pa");
        }
        for p in [meeting, ICE_II_III_V.0] {
            let (below, above) = (at(p - 1.0).unwrap(), at(p + 1.0).unwrap());
            assert!((below - above).abs() < 1e-5, "{p} Pa");
        }
        // From 0 K at the Ih–II line's 14 MPa, rising to II–III–V, then falling.
        let mut before = -1.0;
        for step in 0..=330 {
            let t = at(14e6 + f64::from(step) * 1e6).unwrap();
            assert!(t > before, "{step}");
            before = t;
        }
        assert!(at(500e6).unwrap() < at(400e6).unwrap());
        let (_, rising) = water_melting();
        let piece = |i: usize| {
            let start = rising[i].start();
            (start, rising[i].equation().pressure(start))
        };
        let (t_iii_v, p_iii_v) = piece(1);
        let (t_v_vi, p_v_vi) = piece(2);
        assert_same_bits(ice_iii_v_pressure(t_iii_v).value(), p_iii_v.value());
        assert_same_bits(ice_v_vi_pressure(t_v_vi).value(), p_v_vi.value());
        let ii_iii_v = ice_iii_v_pressure(Kelvin::new(ICE_II_III_V.1)).value();
        assert!((ii_iii_v / ICE_II_III_V.0 - 1.0).abs() < 1e-12);
        let ii_v_vi = ice_v_vi_pressure(Kelvin::new(ICE_II_V_VI.1)).value();
        assert!((ii_v_vi / ICE_II_V_VI.0 - 1.0).abs() < 1e-12);
        assert!(
            (p_iii_v.value() / 1e6 - 350.1).abs() < 0.01
                && (p_v_vi.value() / 1e6 - 632.4).abs() < 0.01
        );
    }

    /// The one test of a floor: ice II's field below its edge, ice VI's beyond the V–VI line, and
    /// ice V's for ice III named past II–III–V and the III–V line.
    #[test]
    fn a_floor_outside_its_own_ice_s_field_is_named() {
        let at =
            |mpa: f64, k: f64, named| foreign_ice(Pascals::new(mpa * 1e6), Kelvin::new(k), named);
        let iii = Some(HighPressureIce::IceIii);
        let v = Some(HighPressureIce::IceV);
        assert_eq!(at(223.9, 236.0, iii), Some(ForeignIce::IceIi));
        assert_eq!(at(223.9, 245.0, iii), None);
        assert_eq!(at(206.87, 233.0, None), Some(ForeignIce::IceIi));
        assert_eq!(at(206.87, 236.35, None), None);
        assert_eq!(at(500.0, 235.0, v), None);
        assert_eq!(at(347.06, 250.0, iii), Some(ForeignIce::IceV));
        assert_eq!(at(347.06, 250.0, v), None);
        assert_eq!(at(625.0, 235.0, v), Some(ForeignIce::IceVi));
        assert_eq!(at(625.0, 272.0, v), None);
        assert_eq!(at(f64::NAN, 250.0, v), None);
    }

    /// The solutes' depressions add, and their sum holds at the largest eutectic depression
    /// present, the liquid then eutectic; with no solute there is none.
    #[test]
    fn the_depressions_add_to_the_largest_eutectic() {
        let liquidus = AqueousLiquidus::of_registry();
        let (n, a) = (0.01, 0.02);
        let sum = liquidus.sodium_chloride_depression_k(n) + liquidus.ammonia_depression_k(a);
        assert_same_bits(liquidus.depression_k(n, a), sum);
        let reference = ICE_IH_ATMOSPHERIC_MELTING_POINT.value();
        assert_same_bits(liquidus.depression_k(0.2, 0.3), reference - 176.15);
        assert_same_bits(liquidus.depression_k(0.233, 0.0), reference - 251.9);
        // NaCl alone caps at its own eutectic, never at ammonia's.
        assert!(liquidus.depression_k(1.0, 0.0) < 21.3);
        assert_same_bits(liquidus.depression_k(0.0, 0.0), 0.0);
        assert_same_bits(liquidus.eutectic_depression_k(0.0, 0.0), 0.0);
        assert_same_bits(liquidus.eutectic_depression_k(0.1, 0.1), reference - 176.15);
        assert_eq!(by_key("brine_nacl"), Some(liquidus.sodium_chloride_row()));
        assert_eq!(by_key("nh3_h2o"), Some(liquidus.ammonia_row()));
    }
}
