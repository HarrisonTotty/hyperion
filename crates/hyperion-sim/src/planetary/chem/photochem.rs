//! An envelope's photochemical rules (plan 14, P14.T52.b; decision-composition §1.3): its
//! photochemical SO₂, its sulphur, hydrocarbon, ice-giant and chromophore hazes, and the thermal
//! dissociation of H₂O, TiO and VO on ultra-hot daysides, each with its source and range, on the
//! Surface path.
//!
//! Nothing on the record reads them until P14.T54.b (P14.T52.d calls them on the quenched gas,
//! [`quench`](super::quench)), so no generated output moves with them. P14.T24.c's ozone and
//! methane-haze rules, for secondary airs, stay its own, and a desiccated world's CO and O₂ are
//! P14.T53.a's.
//!
//! # The rules
//!
//! Each is a closed form of the inputs it names, continuous in every one of them: where a source
//! gives a range and no form at its edges, the rule tapers linearly to zero inside the range (the
//! builder's choice, stated with each), so that it holds only in its range and never jumps.
//!
//! | Rule | Source | Condition and range | Gives |
//! | --- | --- | --- | --- |
//! | [`sulphur_dioxide`] | Tsai et al. 2023 | `T_eq` 900–2,500 K, 5–20 × solar | an SO₂ layer, 10–0.01 mbar |
//! | [`sulphur_haze`] | Gao et al. 2017 | `T_eq` 250–700 K; S₈ saturated aloft | an S₈ haze, 100–0.1 mbar |
//! | [`soot`], [`hydrocarbon_share`] | Gao et al. 2020 | `T_eq` below 950 K | a soot haze from 1 µbar |
//! | [`ice_giant_haze`] | Irwin et al. 2022 | a CH₄ layer | a haze above it |
//! | [`chromophore`] | Braude et al. 2020 | an NH₃ layer | a red layer at 0.2 bar or above |
//! | [`Dissociating`] | Parmentier et al. 2018 | 2,000–4,000 K, 1 µbar–200 bar | H₂O, TiO and VO remaining |
//!
//! - **Photochemical SO₂** (S.-M. Tsai et al., "Photochemically produced SO2 in the atmosphere of
//!   WASP-39b", Nature 617, 483, 2023, arXiv:2211.10490v2). Water's photolysis gives the OH that
//!   oxidises H₂S's sulphur, H₂S + 2 H₂O → SO₂ + 3 H₂ in net; "For 10 ×solar metallicity, the sweet
//!   spot temperature for producing observable SO2 is 1000 K ≲ Teq ≲ 1600 K. For Teq ≲ 1000 K, SO2
//!   production below the 0.01 mbar level ceased and Sx ... is more favoured. For Teq ≳ 1600 K, SH
//!   becomes the predominant sulphur-bearing molecular" (Methods). The mole fraction is their
//!   Extended Data Fig. 10b's average between 10 and 0.01 mbar at 10 × solar, read by the lane
//!   ([`SULPHUR_DIOXIDE_BY_TEMPERATURE`]), times their Fig. 4a's metallicity dependence
//!   ([`SULPHUR_DIOXIDE_BY_METALLICITY`]), at most the gas's sulphur and half its water.
//! - **The S₈ haze** (P. Gao et al., "Sulfur hazes in giant exoplanet atmospheres: impacts on
//!   reflected light spectra", AJ 153, 139, 2017, arXiv:1701.00318v2, after Zahnle et al. 2016,
//!   ApJ 824, 137): "For temperate (250 K < Teq < 700 K) Jupiter–mass planets, photochemical
//!   destruction of H2S results in the production of ∼1 ppmv of S8 between 100 and 0.1 mbar, which,
//!   if cool enough, will condense to form a haze". Two conditions, kept apart: the vapour by
//!   `T_eq`, and its condensation by the stratosphere's temperature against S₈'s vapour pressure
//!   ([`sulphur_saturation_pressure`]): "sulfur hazes may arise on temperate giant exoplanets with
//!   stratospheric temperatures <325K but warmer than Jupiter".
//! - **The soot haze** (P. Gao et al., "Aerosol composition of hot giant exoplanets dominated by
//!   silicates and hydrocarbon hazes", Nature Astronomy 4, 951, 2020, arXiv:2005.11939v1):
//!   "silicates dominate aerosol opacity above planetary equilibrium temperatures of 950 K ...
//!   while hydrocarbon aerosols dominate below 950 K due to an increase in methane abundance". The
//!   silicates are P14.T52.c's decks; this rule gives the haze, formed at 1 µbar from the methane
//!   there at their diffusion-limited rate.
//! - **The ice giants' haze** (P. G. J. Irwin et al., "Hazy blue worlds: a holistic aerosol model
//!   for Uranus and Neptune, including dark spots", JGR Planets 127, e2022JE007189, 2022,
//!   arXiv:2201.04516): their Aerosol-3, "an extended layer of photochemically-produced haze" from
//!   the methane layer up, of small particles and small optical depth (their Table 2).
//! - **The chromophore** (J. A. Braude et al., "Colour and tropospheric cloud structure of Jupiter
//!   from MUSE/VLT: retrieving a universal chromophore", Icarus 338, 113589, 2020,
//!   arXiv:1912.00918v1): "We retrieved this chromophore to be located no deeper than 0.2 ± 0.1
//!   bars in the Great Red Spot and 0.7 ± 0.1 bars elsewhere on Jupiter", a Gaussian layer in ln p
//!   above the ammonia cloud; Braude et al. retrieve it on Jupiter alone, and the rule takes any
//!   giant with an ammonia layer for a cold giant like it.
//! - **Thermal dissociation** (V. Parmentier et al., "From thermal dissociation to condensation in
//!   the atmospheres of ultra hot Jupiters: WASP-121b in context", A&A 617, A110, 2018,
//!   arXiv:1805.00096v2; J. D. Lothringer, T. Barman and T. Koskinen, "Extremely irradiated hot
//!   Jupiters: non-oxide inversions, H− opacity, and thermal dissociation of molecules", ApJ 866,
//!   27, 2018): Parmentier et al.'s fit to NASA CEA of the share of H₂O, TiO and VO left on an
//!   ultra-hot dayside. P14.T52.a's network dissociates them itself, in its own equilibrium to
//!   6,000 K (its atoms and ions), so the fit is never applied to its gas: it is the rule's closed
//!   form, for a consumer that needs the share without a solve, and the check the tests hold the
//!   network to.
//!
//! # What is not modelled
//!
//! Each rule gives only what its source states. Tsai et al.'s other products in the SO₂ layer (S,
//! S₂, SO and SH; the S atom joins the registry with P14.T52.a's commit B) are not written: the rest
//! of the layer's sulphur stays H₂S. Hazes' microphysics (growth, settling,
//! coagulation) is not modelled: each haze's amount is its source's, or for soot and S₈ the
//! closed estimate each rule states. Photochemistry's dependence on the host's ultraviolet is not
//! read: Tsai et al. find that "a 10x UV reduction has almost no effect" on the SO₂, Gao et al.
//! (2017) that S₈'s mixing ratio "is largely independent of stellar UV fluxes", and Gao et al.
//! (2020) that the diffusion-limited supply "is always lower than the photon-limited production
//! rate" on their grid.
//!
//! # Determinism
//!
//! Closed forms, every transcendental through [`math`], every table read in its order: the same
//! bits on every target.

use std::error::Error;
use std::fmt;

use crate::math;
use crate::substance::{
    BuildSizeDistributionError, CondensedPhase, Element, SizeDistribution, SubstanceId, by_key,
};
use crate::units::{
    Kelvin, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, Micrometres, Pascals,
};

/// Pascals per bar.
const PASCALS_PER_BAR: f64 = 1.0e5;

/// Metres per micrometre.
const METRES_PER_MICROMETRE: f64 = 1.0e-6;

/// An envelope's upper atmosphere, as plain values: what the rules read of the body and its air.
///
/// The fields are public, as P14.T50.b's inputs are; each rule checks those it reads.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct UpperAtmosphere {
    /// The body's equilibrium temperature, at full redistribution and its Bond albedo, which the
    /// sources' rules are written in.
    pub equilibrium_temperature: Kelvin,
    /// The temperature of the isothermal stratosphere above the photosphere (P14.T24.d's skin),
    /// where the S₈ haze would condense.
    pub stratosphere_temperature: Kelvin,
    /// The gas's metallicity as a multiple of solar (P14.T52.d passes the envelope's enrichment
    /// times 10^\[Fe/H\]).
    pub metallicity: f64,
    /// The gravity, taken as constant over the upper atmosphere.
    pub gravity: MetresPerSecondSquared,
    /// The gas's mean molar mass, g mol⁻¹.
    pub molar_mass_g_per_mol: f64,
}

/// A rule could not be evaluated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvaluateRuleError {
    /// A temperature is not finite and positive, K.
    Temperature(f64),
    /// The metallicity is not finite and positive.
    Metallicity(f64),
    /// The gravity is not finite and positive, m s⁻².
    Gravity(f64),
    /// The molar mass is not finite and positive, g mol⁻¹.
    MolarMass(f64),
    /// A pressure is not finite and positive, Pa.
    Pressure(f64),
    /// A mole fraction is not finite and within 0–1.
    MoleFraction(f64),
    /// A row the rule names is not in the registry.
    MissingRow {
        /// Its key.
        key: &'static str,
    },
    /// The haze's size distribution could not be built, which the rules' constants exclude.
    Size(BuildSizeDistributionError),
}

impl fmt::Display for EvaluateRuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temperature(t) => write!(f, "temperature {t} K is not finite and positive"),
            Self::Metallicity(m) => write!(f, "metallicity {m} is not finite and positive"),
            Self::Gravity(g) => write!(f, "gravity {g} m/s² is not finite and positive"),
            Self::MolarMass(m) => write!(f, "molar mass {m} g/mol is not finite and positive"),
            Self::Pressure(p) => write!(f, "pressure {p} Pa is not finite and positive"),
            Self::MoleFraction(x) => write!(f, "mole fraction {x} is not within 0 to 1"),
            Self::MissingRow { key } => write!(f, "{key} is not a registry row"),
            Self::Size(error) => write!(f, "the haze's size distribution: {error}"),
        }
    }
}

impl Error for EvaluateRuleError {}

/// Checks a temperature.
fn check_temperature(t: Kelvin) -> Result<f64, EvaluateRuleError> {
    let t = t.value();
    if t.is_finite() && t > 0.0 {
        Ok(t)
    } else {
        Err(EvaluateRuleError::Temperature(t))
    }
}

/// Checks a pressure.
fn check_pressure(p: Pascals) -> Result<f64, EvaluateRuleError> {
    let p = p.value();
    if p.is_finite() && p > 0.0 {
        Ok(p)
    } else {
        Err(EvaluateRuleError::Pressure(p))
    }
}

/// Checks a mole fraction.
fn check_mole_fraction(x: f64) -> Result<f64, EvaluateRuleError> {
    if x.is_finite() && (0.0..=1.0).contains(&x) {
        Ok(x)
    } else {
        Err(EvaluateRuleError::MoleFraction(x))
    }
}

impl UpperAtmosphere {
    /// Checks every field.
    fn check(&self) -> Result<(), EvaluateRuleError> {
        check_temperature(self.equilibrium_temperature)?;
        check_temperature(self.stratosphere_temperature)?;
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(self.metallicity) {
            return Err(EvaluateRuleError::Metallicity(self.metallicity));
        }
        if !positive(self.gravity.value()) {
            return Err(EvaluateRuleError::Gravity(self.gravity.value()));
        }
        if !positive(self.molar_mass_g_per_mol) {
            return Err(EvaluateRuleError::MolarMass(self.molar_mass_g_per_mol));
        }
        Ok(())
    }

    /// The gas's column mass between pressures `top` and `base` (`top` ≤ `base`), kg m⁻²: (base −
    /// top) ÷ g, hydrostatic.
    fn column(&self, top: f64, base: f64) -> f64 {
        (base - top) / self.gravity.value()
    }
}

/// 0 at `from` rising linearly to 1 at `to` (either order), held at each end: a positive zero
/// there, never the −0 a quotient of (`from` − `from`) and a negative width gives.
fn ramp(x: f64, from: f64, to: f64) -> f64 {
    let s = (x - from) / (to - from);
    if s > 0.0 { s.min(1.0) } else { 0.0 }
}

/// Linear interpolation of log₁₀ y in `x` through `nodes` (x ascending), held at the end values
/// outside them.
fn log_interpolate(nodes: &[(f64, f64)], x: f64) -> f64 {
    let (first, last) = (nodes[0], nodes[nodes.len() - 1]);
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    let mut value = last.1;
    for pair in nodes.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if x <= x1 {
            let s = (x - x0) / (x1 - x0);
            value = math::exp10(math::log10(y0) + s * (math::log10(y1) - math::log10(y0)));
            break;
        }
    }
    value
}

/// The registry row `key`.
fn row(key: &'static str) -> Result<SubstanceId, EvaluateRuleError> {
    by_key(key).ok_or(EvaluateRuleError::MissingRow { key })
}

/// Photochemical SO₂'s mole fraction at 10 × solar, by `T_eq` (K, SO₂): Tsai et al.'s (2023)
/// Extended Data Fig. 10b, its average between 10 and 0.01 mbar on their grid of `T_eq` with
/// WASP-39's ultraviolet (`T_int` 100 K, g = 10 m s⁻²), read by the lane (to about 0.2–0.3 dex),
/// interpolated in log₁₀ x; "SO2 drops below 1e-9 between 1000 and 900 K" (the lane's reading).
pub const SULPHUR_DIOXIDE_BY_TEMPERATURE: [(f64, f64); 6] = [
    (1_000.0, 4.5e-6),
    (1_200.0, 5.0e-6),
    (1_400.0, 2.5e-6),
    (1_600.0, 9.0e-7),
    (1_800.0, 3.5e-7),
    (2_000.0, 1.7e-7),
];

/// Where photochemical SO₂ goes to zero, `T_eq` K: from none at 900 K to the table's first node at
/// 1,000 K (Tsai et al.'s grid point at 800 K has under 10⁻⁹), and from its last node at 2,000 K, the
/// grid's last, to none at 2,500 K, the builder's extrapolation, where H₂O, the oxidant's source,
/// is half dissociated at 1 mbar ([`Dissociating::Water`]).
pub const SULPHUR_DIOXIDE_EDGES: (Kelvin, Kelvin) = (Kelvin::new(900.0), Kelvin::new(2_500.0));

/// SO₂'s mole fraction against metallicity relative to its value at 10 × solar: Tsai et al.'s
/// (2023) Fig. 4a, the 10–0.01 mbar average for WASP-39b, about 8 × 10⁻⁸, 1.2 × 10⁻⁵ and 9 × 10⁻⁵ at
/// 5, 10 and 20 × solar (read by the lane): "the increase in the SO2 feature amplitude between 5×
/// and 20× solar metallicity is much greater than that of CO2 and H2O". Interpolated in log–log;
/// above 20 × solar held, and below 5 × extrapolated on the 5–10 × slope, so that SO₂ falls away
/// with the metals (both outside their range, the builder's).
pub const SULPHUR_DIOXIDE_BY_METALLICITY: [(f64, f64); 3] =
    [(5.0, 8.0e-8 / 1.2e-5), (10.0, 1.0), (20.0, 9.0e-5 / 1.2e-5)];

/// The SO₂ layer, the pressures between which [`SULPHUR_DIOXIDE_BY_TEMPERATURE`] is Tsai et al.'s
/// average: 10 to 0.01 mbar (its peak "around 0.1 mbar").
pub const SULPHUR_DIOXIDE_LAYER: (Pascals, Pascals) = (Pascals::new(1.0e3), Pascals::new(1.0));

/// A layer of photochemical SO₂ ([`sulphur_dioxide`]): a constant mole fraction between a base and
/// a top pressure, its sulphur taken from the H₂S there.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SulphurDioxideLayer {
    base: Pascals,
    top: Pascals,
    mole_fraction: f64,
}

impl SulphurDioxideLayer {
    /// The layer's base, its deeper pressure.
    #[must_use]
    pub const fn base(&self) -> Pascals {
        self.base
    }

    /// The layer's top.
    #[must_use]
    pub const fn top(&self) -> Pascals {
        self.top
    }

    /// SO₂'s mole fraction in the layer; the H₂S there loses as much.
    #[must_use]
    pub const fn mole_fraction(&self) -> f64 {
        self.mole_fraction
    }
}

/// The photochemical SO₂ of an envelope with upper atmosphere `air`, whose gas below the layer
/// holds sulphur atoms at mole fraction `sulphur` and water at `water` (the quenched gas's), or
/// `None` where it makes none (the [module](self)'s table: Tsai et al. 2023).
///
/// # Errors
///
/// [`EvaluateRuleError`] for a field of `air` out of its range, or a mole fraction not within 0–1.
///
/// # Examples
///
/// WASP-39b at 10 × solar makes SO₂; a cooler giant keeps its sulphur as allotropes, and none:
///
/// ```
/// use hyperion_sim::planetary::chem::photochem::{UpperAtmosphere, sulphur_dioxide};
/// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared};
///
/// # fn main() -> Result<(), hyperion_sim::planetary::chem::photochem::EvaluateRuleError> {
/// let air = |t_eq: f64| UpperAtmosphere {
///     equilibrium_temperature: Kelvin::new(t_eq),
///     stratosphere_temperature: Kelvin::new(0.84 * t_eq),
///     metallicity: 10.0,
///     gravity: MetresPerSecondSquared::new(4.3),
///     molar_mass_g_per_mol: 2.5,
/// };
/// let (sulphur, water) = (2.6e-4, 4e-3);
/// let warm = sulphur_dioxide(&air(1_170.0), sulphur, water)?.map(|layer| layer.mole_fraction());
/// assert!(warm > Some(1e-6));
/// assert_eq!(sulphur_dioxide(&air(800.0), sulphur, water)?, None);
/// # Ok(())
/// # }
/// ```
pub fn sulphur_dioxide(
    air: &UpperAtmosphere,
    sulphur: f64,
    water: f64,
) -> Result<Option<SulphurDioxideLayer>, EvaluateRuleError> {
    air.check()?;
    let (sulphur, water) = (check_mole_fraction(sulphur)?, check_mole_fraction(water)?);
    let t = air.equilibrium_temperature.value();
    let (cold, hot) = SULPHUR_DIOXIDE_EDGES;
    let (first, last) = (
        SULPHUR_DIOXIDE_BY_TEMPERATURE[0],
        SULPHUR_DIOXIDE_BY_TEMPERATURE[SULPHUR_DIOXIDE_BY_TEMPERATURE.len() - 1],
    );
    let by_temperature = if t < first.0 {
        first.1 * ramp(t, cold.value(), first.0)
    } else if t > last.0 {
        last.1 * ramp(t, hot.value(), last.0)
    } else {
        log_interpolate(&SULPHUR_DIOXIDE_BY_TEMPERATURE, t)
    };
    let by_metallicity = {
        let [low, middle, _] = SULPHUR_DIOXIDE_BY_METALLICITY;
        if air.metallicity < low.0 {
            let slope = math::log10(middle.1 / low.1) / math::log10(middle.0 / low.0);
            low.1 * math::powf(air.metallicity / low.0, slope)
        } else {
            log_interpolate(&SULPHUR_DIOXIDE_BY_METALLICITY, air.metallicity)
        }
    };
    let mole_fraction = (by_temperature * by_metallicity)
        .min(sulphur)
        .min(water / 2.0);
    if mole_fraction > 0.0 {
        let (base, top) = SULPHUR_DIOXIDE_LAYER;
        Ok(Some(SulphurDioxideLayer {
            base,
            top,
            mole_fraction,
        }))
    } else {
        Ok(None)
    }
}

/// Which rule formed a [`Haze`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HazeRule {
    /// [`sulphur_haze`]: S₈.
    Sulphur,
    /// [`soot`]: hydrocarbon soot.
    Soot,
    /// [`ice_giant_haze`]: above the methane layer.
    IceGiant,
    /// [`chromophore`]: a cold giant's red layer.
    Chromophore,
}

/// A haze's vertical extent.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum HazeProfile {
    /// Between `base` and `top` (`top` < `base`) the haze's mixing ratio goes as (p ÷
    /// `base`)^`exponent`: 0 for a well-mixed slab, P14.T24.c's form.
    Layer {
        /// The deeper bound.
        base: Pascals,
        /// The shallower bound.
        top: Pascals,
        /// The power of p ÷ `base`.
        exponent: f64,
    },
    /// Its optical depth per unit ln p goes as exp(−(ln(p ÷ `peak`) ÷ `width`)²), Braude et al.'s
    /// (2020) layer.
    Gaussian {
        /// The pressure of its peak.
        peak: Pascals,
        /// Its width in ln p.
        width: f64,
    },
}

/// How much of a haze there is: its column mass, or the optical depth its source retrieved at a
/// wavelength.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum HazeAmount {
    /// A column mass, from which P14.T24.c's τ(550) follows through P14.T49.e's mass extinction.
    ColumnMass(KilogramsPerSquareMetre),
    /// An optical depth at `wavelength`, the source's.
    OpticalDepth {
        /// The column's optical depth.
        optical_depth: f64,
        /// Where it was retrieved.
        wavelength: Micrometres,
    },
}

/// A haze a photochemical rule forms: its material, phase, particles, extent and amount.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Haze {
    rule: HazeRule,
    material: SubstanceId,
    size: SizeDistribution,
    profile: HazeProfile,
    amount: HazeAmount,
}

impl Haze {
    /// The rule that formed it.
    #[must_use]
    pub const fn rule(&self) -> HazeRule {
        self.rule
    }

    /// Its material, a registry row: `S8`, `soot`, `ice_giant_haze` or `chromophore`.
    #[must_use]
    pub const fn material(&self) -> SubstanceId {
        self.material
    }

    /// Its phase: every rule's haze is solid.
    #[must_use]
    pub const fn phase(&self) -> CondensedPhase {
        CondensedPhase::Solid
    }

    /// Its particles' size distribution, of the volume-to-area radius (P14.T24.c).
    #[must_use]
    pub const fn size(&self) -> SizeDistribution {
        self.size
    }

    /// Its vertical extent.
    #[must_use]
    pub const fn profile(&self) -> HazeProfile {
        self.profile
    }

    /// How much of it there is.
    #[must_use]
    pub const fn amount(&self) -> HazeAmount {
        self.amount
    }
}

/// The `T_eq` window of S₈'s photochemical production, K: "temperate (250 K < Teq < 700 K)
/// Jupiter–mass planets" (Gao et al. 2017, after Zahnle et al. 2016). Below, "NH4SH clouds are
/// expected to form at temperatures <200 K ... which removes H2S from the atmosphere and arrests
/// the photochemical production of S8"; above, "The growth of elemental sulphur allotropes beyond
/// S2 effectively stops for temperatures higher than ∼ 750 K" (Tsai et al. 2023).
pub const SULPHUR_HAZE_RANGE: (Kelvin, Kelvin) = (Kelvin::new(250.0), Kelvin::new(700.0));

/// The width inside each edge of [`SULPHUR_HAZE_RANGE`] over which the S₈ vapour tapers to zero,
/// K: the builder's.
pub const SULPHUR_HAZE_TAPER: f64 = 50.0;

/// S₈'s mole fraction in its layer at solar metallicity: "∼1 ppmv", robust to "within a factor
/// of 2" over the ultraviolet and the mixing (Gao et al. 2017, §2, after Zahnle et al. 2016), and
/// "if the equilibrium S8 mixing ratio is 10 ppm rather than 1 ppm" at higher metallicity, taken
/// in proportion to it, at most an eighth of the gas's sulphur.
pub const SULPHUR_HAZE_VAPOUR: f64 = 1.0e-6;

/// The S₈ vapour's layer, "between 100 and 0.1 mbar" (Gao et al. 2017's abstract): the haze lies
/// where the vapour is supersaturated within it.
pub const SULPHUR_HAZE_LAYER: (Pascals, Pascals) = (Pascals::new(1.0e4), Pascals::new(10.0));

/// The S₈ haze's particles: Gao et al.'s (2017, §3.2) log-normal, "the mean particle radius is set
/// to 0.1 µm", `r_g`, with σ = 2, the geometric standard deviation.
pub const SULPHUR_HAZE_PARTICLES: (Micrometres, f64) = (Micrometres::new(0.1), 2.0);

/// S₈'s saturation vapour pressure over its solid at `temperature` (Zahnle et al. 2016, as Gao et
/// al. 2017's eq. 1 gives it): exp(20 − 11,800 ÷ T) below 413 K and exp(9.6 − 7,510 ÷ T) above, in
/// bar (their units are not stated; with bar, 1 ppmv saturates at 0.1 bar near 327 K, Gao et al.'s
/// "<325K", the lane's check). The two branches cross at 412.5 K, (11,800 − 7,510) ÷ (20 − 9.6),
/// and the lower is taken, so that the curve is continuous.
#[must_use]
pub fn sulphur_saturation_pressure(temperature: Kelvin) -> Pascals {
    let t = temperature.value();
    let low = 20.0 - 11_800.0 / t;
    let high = 9.6 - 7_510.0 / t;
    Pascals::new(math::exp(low.min(high)) * PASCALS_PER_BAR)
}

/// The log-normal of geometric mean radius `r_g` µm and geometric standard deviation `sigma`, by
/// Hansen and Travis's (1974) `v_eff` = exp(ln² σ) − 1 and `r_eff` = `r_g` (1 + `v_eff`)^(5/2).
fn log_normal(r_g: Micrometres, sigma: f64) -> Result<SizeDistribution, EvaluateRuleError> {
    let ln_sigma = math::ln(sigma);
    let variance = math::exp(ln_sigma * ln_sigma) - 1.0;
    let radius = r_g.value() * math::powf(1.0 + variance, 2.5) * METRES_PER_MICROMETRE;
    SizeDistribution::log_normal(Metres::new(radius), variance).map_err(EvaluateRuleError::Size)
}

/// The gamma distribution of effective radius `radius` µm and effective variance `variance`.
fn gamma(radius: Micrometres, variance: f64) -> Result<SizeDistribution, EvaluateRuleError> {
    SizeDistribution::gamma(
        Metres::new(radius.value() * METRES_PER_MICROMETRE),
        variance,
    )
    .map_err(EvaluateRuleError::Size)
}

/// The S₈ haze of an envelope with upper atmosphere `air` whose gas holds sulphur atoms at mole
/// fraction `sulphur`, or `None` where none forms (Gao et al. 2017).
///
/// The vapour is [`SULPHUR_HAZE_VAPOUR`] times the metallicity, tapered over
/// [`SULPHUR_HAZE_RANGE`]; the haze is the vapour in excess of saturation at the stratosphere's
/// temperature, all of it condensed, as Gao et al. estimate its mass ("assuming that all S8 that is
/// supersaturated condenses"): ∫ (x − `p_sat` ÷ p) dp ÷ g × M(S₈) ÷ μ between the layer's base and
/// the higher of its top and `p_sat` ÷ x, a well-mixed slab there.
///
/// # Errors
///
/// [`EvaluateRuleError`] for a field of `air` out of its range, or a mole fraction not within 0–1.
///
/// # Examples
///
/// Gao et al.'s Gamma Cephei Ab, about 260 K in its stratosphere, has a haze; a giant whose
/// stratosphere is 400 K keeps its S₈ as vapour:
///
/// ```
/// use hyperion_sim::planetary::chem::photochem::{UpperAtmosphere, sulphur_haze};
/// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared};
///
/// # fn main() -> Result<(), hyperion_sim::planetary::chem::photochem::EvaluateRuleError> {
/// let air = |stratosphere: f64| UpperAtmosphere {
///     equilibrium_temperature: Kelvin::new(330.0),
///     stratosphere_temperature: Kelvin::new(stratosphere),
///     metallicity: 1.0,
///     gravity: MetresPerSecondSquared::new(25.0),
///     molar_mass_g_per_mol: 2.3,
/// };
/// assert!(sulphur_haze(&air(260.0), 2.7e-5)?.is_some());
/// assert_eq!(sulphur_haze(&air(400.0), 2.7e-5)?, None);
/// # Ok(())
/// # }
/// ```
pub fn sulphur_haze(
    air: &UpperAtmosphere,
    sulphur: f64,
) -> Result<Option<Haze>, EvaluateRuleError> {
    air.check()?;
    let sulphur = check_mole_fraction(sulphur)?;
    let t = air.equilibrium_temperature.value();
    let (cold, hot) = (SULPHUR_HAZE_RANGE.0.value(), SULPHUR_HAZE_RANGE.1.value());
    let window =
        ramp(t, cold, cold + SULPHUR_HAZE_TAPER).min(ramp(t, hot, hot - SULPHUR_HAZE_TAPER));
    let vapour = (SULPHUR_HAZE_VAPOUR * air.metallicity).min(sulphur / 8.0) * window;
    if vapour <= 0.0 {
        return Ok(None);
    }
    let saturation = sulphur_saturation_pressure(air.stratosphere_temperature).value();
    let (base, top) = (SULPHUR_HAZE_LAYER.0.value(), SULPHUR_HAZE_LAYER.1.value());
    let onset = saturation / vapour;
    let top = if onset > top { onset } else { top };
    if top >= base {
        return Ok(None);
    }
    let condensed = vapour * (base - top) - saturation * math::ln(base / top);
    let material = row("S8")?;
    let molar_mass = material
        .substance()
        .molar_mass_g_per_mol()
        .ok_or(EvaluateRuleError::MissingRow { key: "S8" })?;
    let column = condensed / air.gravity.value() * molar_mass / air.molar_mass_g_per_mol;
    if column <= 0.0 {
        return Ok(None);
    }
    let (r_g, sigma) = SULPHUR_HAZE_PARTICLES;
    Ok(Some(Haze {
        rule: HazeRule::Sulphur,
        material,
        size: log_normal(r_g, sigma)?,
        profile: HazeProfile::Layer {
            base: Pascals::new(base),
            top: Pascals::new(top),
            exponent: 0.0,
        },
        amount: HazeAmount::ColumnMass(KilogramsPerSquareMetre::new(column)),
    }))
}

/// Where the upper atmosphere's aerosol opacity passes from hydrocarbon haze to silicate cloud,
/// `T_eq` K: Gao et al.'s (2020) 950 K, and the width below it over which the haze's share falls
/// from 1 to 0, the builder's 100 K (their Fig. 2a, read by the lane at 10 × solar and 10 m s⁻²:
/// about 0.55 near 890 K and none by about 990 K).
pub const HYDROCARBON_EDGE: (Kelvin, f64) = (Kelvin::new(950.0), 100.0);

/// The pressure at which the soot haze forms: 1 µbar (Gao et al. 2020, Methods: "We set the
/// pressure of haze formation to 1 μbar, consistent with previous exoplanet photochemical and haze
/// formation models").
pub const SOOT_FORMATION: Pascals = Pascals::new(0.1);

/// The soot's particles: Gao et al.'s (2020) "minimum particle radius of 50 nm", the mean radius
/// at the haze's level on their coolest planet (their Fig. 2a, read by the lane), a narrow gamma
/// distribution of the builder's variance.
pub const SOOT_PARTICLES: (Micrometres, f64) = (Micrometres::new(0.05), 0.05);

/// The share of an upper atmosphere's aerosol opacity that is hydrocarbon haze at equilibrium
/// temperature `equilibrium_temperature`, the rest P14.T52.c's silicate cloud: 1 up to 850 K,
/// falling linearly to 0 at 950 K and above ([`HYDROCARBON_EDGE`]; Gao et al. 2020).
#[must_use]
pub fn hydrocarbon_share(equilibrium_temperature: Kelvin) -> f64 {
    let (edge, width) = HYDROCARBON_EDGE;
    ramp(
        equilibrium_temperature.value(),
        edge.value(),
        edge.value() - width,
    )
}

/// The soot haze of an envelope with upper atmosphere `air` whose gas holds methane at mole
/// fraction `methane` at the haze's level (the quenched gas's), or `None` where none forms (Gao
/// et al. 2020).
///
/// Gao et al. produce it at the diffusion-limited supply of methane at [`SOOT_FORMATION`], N `K_zz`
/// `f_CH4` ÷ H (their eq. 3). Held for a mixing time over a scale height, H² ÷ `K_zz` (the builder's),
/// its column is the methane's carbon over that scale height, `f_CH4` (M(C) ÷ μ) p ÷ g, whatever the
/// mixing, times [`hydrocarbon_share`], a well-mixed slab from the formation level down a scale
/// height. Its trigger is theirs: "The formation of hydrocarbon hazes below ∼950 K is also expected
/// due to the transition of the primary carbon reservoir from carbon monoxide to methane", which
/// the methane carries.
///
/// # Errors
///
/// [`EvaluateRuleError`] for a field of `air` out of its range, or a mole fraction not within 0–1.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::chem::photochem::{UpperAtmosphere, soot};
/// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared};
///
/// # fn main() -> Result<(), hyperion_sim::planetary::chem::photochem::EvaluateRuleError> {
/// let air = |t_eq: f64| UpperAtmosphere {
///     equilibrium_temperature: Kelvin::new(t_eq),
///     stratosphere_temperature: Kelvin::new(0.84 * t_eq),
///     metallicity: 1.0,
///     gravity: MetresPerSecondSquared::new(10.0),
///     molar_mass_g_per_mol: 2.3,
/// };
/// assert!(soot(&air(800.0), 4e-4)?.is_some());
/// assert_eq!(soot(&air(1_000.0), 4e-4)?, None);
/// # Ok(())
/// # }
/// ```
pub fn soot(air: &UpperAtmosphere, methane: f64) -> Result<Option<Haze>, EvaluateRuleError> {
    air.check()?;
    let methane = check_mole_fraction(methane)?;
    let share = hydrocarbon_share(air.equilibrium_temperature);
    let formation = SOOT_FORMATION.value();
    let column = share * methane * Element::C.standard_atomic_weight_g_per_mol()
        / air.molar_mass_g_per_mol
        * air.column(0.0, formation);
    if column <= 0.0 {
        return Ok(None);
    }
    let (radius, variance) = SOOT_PARTICLES;
    Ok(Some(Haze {
        rule: HazeRule::Soot,
        material: row("soot")?,
        size: gamma(radius, variance)?,
        profile: HazeProfile::Layer {
            base: Pascals::new(formation * core::f64::consts::E),
            top: SOOT_FORMATION,
            exponent: 0.0,
        },
        amount: HazeAmount::ColumnMass(KilogramsPerSquareMetre::new(column)),
    }))
}

/// The ice giants' haze above the methane layer, Irwin et al.'s (2022) Aerosol-3: its base at the
/// methane layer (theirs "fixed at 1.6 bar", Uranus's methane condensing near 1.5 bar and
/// Neptune's Aerosol-2 at 2.0–2.1), a fractional scale height of 2.0, so that its mixing ratio goes
/// as (p ÷ `p_base`)^(1 ÷ 2 − 1), to the builder's top at 1 mbar (above which a haze of that
/// scale height holds about 2.5% of its optical depth).
pub const ICE_GIANT_HAZE_PROFILE: (f64, Pascals) = (-0.5, Pascals::new(100.0));

/// The ice giants' haze's optical depth at 0.8 µm: 0.035, between Irwin et al.'s 0.03 ± 0.01 for
/// Uranus and 0.04 ± 0.01 for Neptune (their Table 2).
pub const ICE_GIANT_HAZE_DEPTH: (f64, Micrometres) = (0.035, Micrometres::new(0.8));

/// The ice giants' haze's particles: Irwin et al.'s fixed r = 0.05 µm and variance 0.05, a gamma
/// distribution.
pub const ICE_GIANT_HAZE_PARTICLES: (Micrometres, f64) = (Micrometres::new(0.05), 0.05);

/// The haze above an ice giant's methane layer, whose base (the methane's condensation level,
/// P14.T52.c's deck) lies at `methane_layer` (Irwin et al. 2022).
///
/// # Errors
///
/// [`EvaluateRuleError::Pressure`] unless `methane_layer` is finite and positive.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::chem::photochem::{HazeProfile, ice_giant_haze};
/// use hyperion_sim::units::Pascals;
///
/// # fn main() -> Result<(), hyperion_sim::planetary::chem::photochem::EvaluateRuleError> {
/// let haze = ice_giant_haze(Pascals::new(1.5e5))?;
/// assert!(matches!(haze.profile(), HazeProfile::Layer { exponent, .. } if exponent < 0.0));
/// # Ok(())
/// # }
/// ```
pub fn ice_giant_haze(methane_layer: Pascals) -> Result<Haze, EvaluateRuleError> {
    let base = check_pressure(methane_layer)?;
    let (exponent, top) = ICE_GIANT_HAZE_PROFILE;
    let top = if top.value() < base {
        top
    } else {
        Pascals::new(base)
    };
    let (optical_depth, wavelength) = ICE_GIANT_HAZE_DEPTH;
    let (radius, variance) = ICE_GIANT_HAZE_PARTICLES;
    Ok(Haze {
        rule: HazeRule::IceGiant,
        material: row("ice_giant_haze")?,
        size: gamma(radius, variance)?,
        profile: HazeProfile::Layer {
            base: methane_layer,
            top,
            exponent,
        },
        amount: HazeAmount::OpticalDepth {
            optical_depth,
            wavelength,
        },
    })
}

/// The deepest the chromophore's peak lies, 0.2 bar: the plan's "no deeper than about 0.2 bar",
/// Braude et al.'s (2020) Great Red Spot, "no deeper than 0.2 ± 0.1 bars" (peak 0.19 ± 0.02 bar in
/// 2018, their Table 5); elsewhere on Jupiter they place it no deeper than 0.6–0.7 bar, and at 0.30
/// and 0.35 bar in the Equatorial Zone and the North Temperate Belts.
pub const CHROMOPHORE_DEEPEST: Pascals = Pascals::new(2.0e4);

/// The chromophore layer's width in ln p, Braude et al.'s ∆c = 0.25 ("Little information can be
/// derived on the chromophore vertical extent ∆c from the MUSE spectra, and so we chose to use a
/// value ∆c = 0.25").
pub const CHROMOPHORE_WIDTH: f64 = 0.25;

/// The chromophore's column, kg m⁻²: Braude et al.'s Great Red Spot in 2018, (6.3 ± 0.3) × 10⁻⁵
/// g cm⁻² (their Table 4), for a particle density of 0.87 g cm⁻³.
pub const CHROMOPHORE_COLUMN: KilogramsPerSquareMetre = KilogramsPerSquareMetre::new(6.3e-4);

/// The chromophore's particles: Braude et al.'s selected `r_c` = 0.05 µm, a gamma distribution of
/// "fixed variance of 0.05" (their §4.1; their Table 2 prints 0.1).
pub const CHROMOPHORE_PARTICLES: (Micrometres, f64) = (Micrometres::new(0.05), 0.05);

/// The red chromophore of a cold giant whose ammonia layer (P14.T52.c's NH₃ deck) lies at
/// `ammonia_layer`, its peak at that layer or at [`CHROMOPHORE_DEEPEST`], whichever is higher
/// (Braude et al. 2020).
///
/// # Errors
///
/// [`EvaluateRuleError::Pressure`] unless `ammonia_layer` is finite and positive.
///
/// # Examples
///
/// Jupiter's ammonia cloud near 0.7 bar puts its chromophore at 0.2 bar:
///
/// ```
/// use hyperion_sim::planetary::chem::photochem::{HazeProfile, chromophore};
/// use hyperion_sim::units::Pascals;
///
/// # fn main() -> Result<(), hyperion_sim::planetary::chem::photochem::EvaluateRuleError> {
/// let haze = chromophore(Pascals::new(7e4))?;
/// assert!(matches!(haze.profile(), HazeProfile::Gaussian { peak, .. } if (peak.value() - 2e4).abs() < 1e-9));
/// # Ok(())
/// # }
/// ```
pub fn chromophore(ammonia_layer: Pascals) -> Result<Haze, EvaluateRuleError> {
    let layer = check_pressure(ammonia_layer)?;
    let peak = if layer < CHROMOPHORE_DEEPEST.value() {
        ammonia_layer
    } else {
        CHROMOPHORE_DEEPEST
    };
    let (radius, variance) = CHROMOPHORE_PARTICLES;
    Ok(Haze {
        rule: HazeRule::Chromophore,
        material: row("chromophore")?,
        size: gamma(radius, variance)?,
        profile: HazeProfile::Gaussian {
            peak,
            width: CHROMOPHORE_WIDTH,
        },
        amount: HazeAmount::ColumnMass(CHROMOPHORE_COLUMN),
    })
}

/// The temperatures and pressures Parmentier et al.'s (2018) fit is stated for: "Valid for a
/// solar-composition gas with 2000 K < T < 4000 K and 200 bar < P < 1µ bar" (their Table 1's note,
/// the bounds printed in reverse). Beyond them the closed form runs on, smooth and monotonic.
pub const DISSOCIATION_RANGE: ((Kelvin, Kelvin), (Pascals, Pascals)) = (
    (Kelvin::new(2_000.0), Kelvin::new(4_000.0)),
    (Pascals::new(0.1), Pascals::new(2.0e7)),
);

/// A molecule whose thermal dissociation Parmentier et al. (2018) fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dissociating {
    /// H₂O ⇌ 2H + O.
    Water,
    /// TiO ⇌ Ti + O.
    TitaniumOxide,
    /// VO ⇌ V + O.
    VanadiumOxide,
}

impl Dissociating {
    /// Every molecule the rule covers.
    pub const ALL: [Self; 3] = [Self::Water, Self::TitaniumOxide, Self::VanadiumOxide];

    /// The molecule's registry key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Water => "H2O",
            Self::TitaniumOxide => "TiO",
            Self::VanadiumOxide => "VO",
        }
    }

    /// Parmentier et al.'s (2018) Table 1 coefficients (α, β in K, γ, log₁₀ `A_0`), transcribed
    /// from arXiv:1805.00096v2 (the table prints 10⁻⁴ β): H₂O (2, 48,300, 15.9, −3.3), TiO (1.6,
    /// 59,400, 23.0, −7.1), VO (1.5, 54,000, 23.8, −9.2).
    #[must_use]
    pub const fn coefficients(self) -> (f64, f64, f64, f64) {
        match self {
            Self::Water => (2.0, 48_300.0, 15.9, -3.3),
            Self::TitaniumOxide => (1.6, 59_400.0, 23.0, -7.1),
            Self::VanadiumOxide => (1.5, 54_000.0, 23.8, -9.2),
        }
    }

    /// The share of the molecule's deep abundance left at `temperature` and total pressure
    /// `pressure`, A ÷ `A_0` (Parmentier et al. 2018, their eq. 1 and Table 1's note): 1 ÷ √A = 1 ÷
    /// √`A_0` + 1 ÷ √`A_d`, with log₁₀ `A_d` = α log₁₀ p + β ÷ T − γ, p in bar, so that A ÷ `A_0` =
    /// (1 + √(`A_0` ÷ `A_d`))⁻². Their eq. 2 prints "+ γ", with which nothing dissociates anywhere; the
    /// note's 10^−γ reproduces their own CEA curves, and Gandhi, Madhusudhan and Mandell (2020, AJ
    /// 159, 232, eq. 2) apply it so.
    ///
    /// P14.T52.a's network dissociates the molecule in its own equilibrium: this is the rule's
    /// closed form, never applied to that gas (the [module](self)'s documentation).
    ///
    /// # Errors
    ///
    /// [`EvaluateRuleError::Temperature`] or [`EvaluateRuleError::Pressure`] unless each is
    /// finite and positive.
    ///
    /// # Examples
    ///
    /// Water is all but gone at 1 mbar on a 3,500 K dayside, and kept at 2,000 K (Lothringer,
    /// Barman and Koskinen 2018: dissociation mutes H₂O, TiO and VO "in planets above 2500 K"):
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::photochem::Dissociating;
    /// use hyperion_sim::units::{Kelvin, Pascals};
    ///
    /// # fn main() -> Result<(), hyperion_sim::planetary::chem::photochem::EvaluateRuleError> {
    /// let left = |t: f64| Dissociating::Water.remaining(Kelvin::new(t), Pascals::new(100.0));
    /// assert!(left(3_500.0)? < 1e-4);
    /// assert!(left(2_000.0)? > 0.99);
    /// # Ok(())
    /// # }
    /// ```
    pub fn remaining(
        self,
        temperature: Kelvin,
        pressure: Pascals,
    ) -> Result<f64, EvaluateRuleError> {
        let t = check_temperature(temperature)?;
        let p = check_pressure(pressure)? / PASCALS_PER_BAR;
        let (alpha, beta, gamma, log_deep) = self.coefficients();
        let log_dissociated = alpha * math::log10(p) + beta / t - gamma;
        let half = (log_deep - log_dissociated) / 2.0;
        let root = 1.0 + math::exp10(half);
        Ok(1.0 / (root * root))
    }
}

#[cfg(test)]
mod tests;
