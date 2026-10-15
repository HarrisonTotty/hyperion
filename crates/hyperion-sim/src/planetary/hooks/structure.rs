//! The vertical structure of a surface's air (plan 14, P14.T24.e; rendering plan R08's Design
//! note 3): a scaled adiabat from the ground up to an isothermal skin, and the one function,
//! [`temperature_at`], that the flight model's drag and R08's picture both read.
//!
//! # The profile
//!
//! T(p) = max(`T_ad`(p), `T_skin`), where the adiabat leaves the ground at the mean surface
//! temperature `T_s` and the surface pressure `p_s` (P14.T24.a's) with
//!
//! ```text
//! dln T_ad ÷ dln p = α R ÷ c_p(T_ad)
//! ```
//!
//! Robinson and Catling (2012, ApJ 757, 104, their Eq. 10, after Sagan 1962) write the
//! convective region as T = `T_0` (p ÷ `p_0`)^(α (γ − 1) ÷ γ), with α "a factor, typically around
//! 0.6–0.9, which accounts for small deviations from the dry adiabatic lapse rate, primarily
//! because of latent heat release": "the average ratio of the true lapse rate in the planet's
//! convective region to the dry adiabatic lapse rate". Their power law holds a fixed γ. No fixed
//! `c_p` serves the galaxy's 30–1,500 K (science-r08-gases-2.md §1), so here `c_p`(T) is
//! P14.T49.b's statistical sum ([`mixture_heat_capacity_over_r_at`]) over the air's published
//! mole fractions, mixed by `c_p` and never by averaging γ, and the adiabat is integrated through
//! its closed-form entropy S̃ = ∫ `c_p` ÷ (R T) dT ([`mixture_entropy_over_r`]):
//!
//! ```text
//! S̃(T_ad(p)) = S̃(T_s) + α ln(p ÷ p_s)
//! ```
//!
//! which is the power law again where `c_p` is constant.
//!
//! # The lapse factor α ([`lapse_factor`])
//!
//! Set by the condensable P14.T48.d's classifier names: [`WATER_LAPSE_FACTOR`] for water and
//! [`METHANE_LAPSE_FACTOR`] for methane (Robinson and Catling 2014, Nature Geoscience 7, 12, Table
//! 1: Earth's 0.6 and Titan's 0.77), and [`DRY_LAPSE_FACTOR`] for carbon dioxide, for every other
//! condensable and for none. A condensable sets α only where its latent heat can change the lapse
//! rate ([`latent_heat_departure`], signoff-3.md item 1.5): a Mars's water, at about 0.13 Pa (210
//! ppm of 6.4 mbar), cannot drive a moist lapse rate.
//!
//! # The skin ([`skin_temperature`])
//!
//! `T_skin` = 2^(−1/4) `T_eff`, with `T_eff`⁴ = `T_eq`⁴ + `T_int`⁴ ([`effective_temperature`]): the
//! τ → 0 limit of a grey atmosphere in radiative equilibrium that emits the absorbed starlight and
//! the internal flux (Robinson and Catling 2012, Eq. 24 at τ = 0, σT⁴ = (`F_net` + `F_i`) ÷ 2;
//! decision-composition §1.12). P14.T13.c's sunlight-only form is its `T_int` = 0 case. A solid
//! body's `T_int` is (F ÷ σ)^¼ of P14.T24.b's heat flow F ([`internal_temperature`]). The skin is
//! held at most `T_s` ([`RadiativeConvective::derive`]): the ground of a grey air emits at least
//! σ`T_eff`⁴ (R&C 2012, Eq. 25 at the ground), so only an internal flux the ground's temperature
//! does not read can put the skin above it.
//!
//! The skin is isothermal, with no ozone or haze heating, so a Titan lacks its 170 K stratosphere.
//! Robinson and Catling 2012's shortwave stratospheric term is the upgrade once P14.T24.c lands,
//! and the thermosphere above is the flight model's.
//!
//! # What is published ([`RadiativeConvective`])
//!
//! `T_s`, `p_s`, α, `T_skin` and the adiabat's (p, T) nodes, evenly in ln p from `p_s` to the
//! tropopause where the adiabat meets `T_skin`: max([`MIN_SEGMENTS`],
//! ⌈ln(`p_s` ÷ `p_trop`) ÷ [`NODE_SPACING_LN_P`]⌉) segments, each node solved to 10⁻¹² of the exact
//! adiabat. Between nodes [`temperature_at`] is the power law through them, within 5 × 10⁻⁴ of
//! the adiabat (science-r08-gases-2.md §1.7.2); below `p_s` it is the first segment's power law,
//! and above the tropopause `T_skin`. A reader needs only the nodes: R08's client copies the
//! formula and keeps no thermodynamics of its own.
//!
//! # Not on the record yet
//!
//! This module is pure: nothing on the record calls it at generator version 22
//! (decision-bump-plan-after-22 §1.3). P14.T54.b, the 23 → 24 bump, puts its value in the record's
//! surface section ([`record::Surface::vertical_structure`](crate::planetary::record::Surface::vertical_structure))
//! and on the wire. P14.T24.d adds the gas envelope's profile as a second variant of
//! [`VerticalStructure`], so that one [`temperature_at`] serves both.
//!
//! # Determinism
//!
//! Each node is a fixed count of Newton steps ([`NEWTON_STEPS`]) from the node above's
//! temperature, every sum in the mixture's given order through the crate's deterministic
//! elementary functions, and every node's pressure `p_s` e^(−(k ÷ n) L) from integers k and n.

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::derive::atmosphere::Gas;
use crate::planetary::hooks::surface::SurfaceConditions;
use crate::substance::{
    MOLAR_GAS_CONSTANT, SubstanceId, mean_molar_mass_g_per_mol, mixture_entropy_over_r,
    mixture_heat_capacity_over_r_at,
};
use crate::units::consts::STEFAN_BOLTZMANN;
use crate::units::{Kelvin, Pascals, WattsPerSquareMetre};

#[cfg(test)]
mod tests;

/// The lapse factor α of an air whose condensable is water and whose latent heat can drive it:
/// 0.6, Earth's (Robinson and Catling 2014, Nature Geoscience 7, 12, Table 1, read at source,
/// arXiv:1312.6859).
///
/// At Earth's temperatures `c_p`(T) equals the kinetic-theory classes R&C took within 0.3%
/// (science-r08-gases-2.md §1.2), so their α stands on the statistical sum.
pub const WATER_LAPSE_FACTOR: f64 = 0.6;

/// The lapse factor α of an air whose condensable is methane and whose latent heat can drive it:
/// 0.77, Titan's, "the average ratio of the lapse rate in Titan's troposphere to the dry adiabatic
/// lapse rate" (Robinson and Catling 2012, ApJ 757, 104, §4.3; their 2014 Table 1; both read at
/// source, arXiv:1209.1833 and 1312.6859).
pub const METHANE_LAPSE_FACTOR: f64 = 0.77;

/// The dry lapse factor α, 0.88: for carbon dioxide, for every other condensable, for none, and for
/// a water or methane whose latent heat cannot drive a moist lapse rate.
///
/// Robinson and Catling (2012, ApJ 757, 104, §4.1, read at source) found α = 0.8 for Venus with a
/// fixed γ of 1.3; with `c_p`(T) that γ no longer holds, and α is refitted on Venus's measured
/// state, 735 K and 92.5 bar with 3.505% nitrogen by number (90.4 bar of carbon dioxide and 2.09
/// of nitrogen by weight; the ruling rounds it to 3.51%), to the Venus International Reference Atmosphere's 350.5 K at 1.066 bar
/// (science-r08-gases-2.md §1.4). Its exact solution on P14.T49.b's `c_p`(T), α = (S̃(350.5 K) −
/// S̃(735 K)) ÷ ln(1.066 ÷ 92.5), is 0.884 to three places, 0.88 to the ruling's two (this module's
/// test `the_dry_factor_is_refitted_on_venus_s_measured_state`).
pub const DRY_LAPSE_FACTOR: f64 = 0.88;

/// The latent-heat departure below which a condensable cannot drive a moist lapse rate, and the
/// air takes [`DRY_LAPSE_FACTOR`]: 0.1 (signoff-3.md, item 1.5; the lane's threshold, with no
/// source beyond it).
///
/// At 0.1 the dilute pseudoadiabat's lapse rate is at least 1 ÷ 1.1 of the dry one, within 9%
/// ([`latent_heat_departure`]), inside the spread of Robinson and Catling's α, 0.6–0.9.
pub const LATENT_HEAT_GATE: f64 = 0.1;

/// The adiabat's largest node spacing, in ln p: 0.3, at which the power law between nodes lies
/// within 5 × 10⁻⁴ of the adiabat in T (science-r08-gases-2.md §1.7.2, computed there: 3.4–3.5 ×
/// 10⁻⁴ for Jupiter's and Uranus's hydrogen over 0.1–10⁴ bar; 0.35 gives up to 5.1 × 10⁻⁴).
pub const NODE_SPACING_LN_P: f64 = 0.3;

/// The fewest segments an adiabat takes: 16 (science-r08-gases-2.md §1.7.2: 3.6 × 10⁻⁴ on Venus
/// over 92–0.1 bar).
pub const MIN_SEGMENTS: u32 = 16;

/// The most segments an adiabat may take: 2,000.
///
/// That is 600 e-folds of pressure, far beyond any air the generator makes (a skin of 1 K over a
/// 2,000 K ground of nitrogen needs about 100 segments at the dry α, 150 at water's) and short of
/// where a pressure would underflow. A
/// longer one is refused as [`DeriveStructureError::TooDeep`], so that no input can ask for an
/// unbounded allocation.
pub const MAX_SEGMENTS: u32 = 2_000;

/// The shortest segment an adiabat takes, in ln p: 10⁻¹². A shorter one, a skin within rounding
/// of the ground, leaves the air isothermal, so that every node's pressure lies strictly below the
/// last's.
pub const SHORTEST_SEGMENT_LN_P: f64 = 1e-12;

/// The Newton steps in ln T that solve each node, from the predictor the node above gives: 6.
///
/// Newton's error in u = ln T falls as e ← (d`c_p` ÷ du) ÷ (2 `c_p`) e², and the predictor starts
/// it under about 10⁻² for a segment of 0.3 in ln p, so four steps reach rounding; two more are
/// margin. This module's test `every_node_solves_its_adiabat_to_rounding` holds every gas row of
/// the registry, pure and mixed, from 30 to 2,000 K, to 10⁻¹².
pub const NEWTON_STEPS: u32 = 6;

/// One node of a temperature profile: a pressure and the temperature there.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ProfileNode {
    pressure: Pascals,
    temperature: Kelvin,
}

impl ProfileNode {
    /// The pressure, positive.
    #[must_use]
    pub const fn pressure(&self) -> Pascals {
        self.pressure
    }

    /// The temperature there, positive.
    #[must_use]
    pub const fn temperature(&self) -> Kelvin {
        self.temperature
    }
}

/// A body's vertical structure, read through [`temperature_at`].
///
/// One variant so far: a surface's air ([`RadiativeConvective`], P14.T24.e). P14.T24.d adds the
/// gas envelope's, a Guillot profile joined to its adiabat, so that one function serves both.
#[derive(Debug, Clone, PartialEq)]
pub enum VerticalStructure {
    /// A scaled adiabat from the ground to an isothermal skin (P14.T24.e).
    RadiativeConvective(RadiativeConvective),
}

impl VerticalStructure {
    /// The temperature at pressure `p` ([`temperature_at`]).
    #[must_use]
    pub fn temperature_at(&self, p: Pascals) -> Kelvin {
        match self {
            Self::RadiativeConvective(profile) => profile.temperature_at(p),
        }
    }
}

/// The temperature of the air of `structure` at pressure `p`, the one function both the flight
/// model's drag and rendering plan R08's client read (P14.T24.e).
///
/// So the picture and the drag read the same air: R08's `temperatureAt` copies it, held to this
/// module's golden at 10⁻¹². It is continuous in p and never rises with height (with falling p). For a
/// [`RadiativeConvective`] structure it is [`RadiativeConvective::temperature_at`].
///
/// # Examples
///
/// Earth's air, by hand: 288 K and 1 bar of nitrogen, oxygen and argon, with water its
/// condensable at 1% by number, under a 255 K equilibrium temperature and its 0.09 W m⁻² heat
/// flow. Its tropopause lies near 0.18 bar, against the 0.16 bar of Robinson and Catling's (2014)
/// Table 1 for Earth, and the stratosphere above is the skin's:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::planetary::hooks::structure::{
///     RadiativeConvective, StructureInputs, VerticalStructure, effective_temperature,
///     internal_temperature, skin_temperature, temperature_at,
/// };
/// use hyperion_sim::units::{Kelvin, Pascals, WattsPerSquareMetre};
///
/// let gases = [
///     (Gas::Nitrogen.substance(), 0.77),
///     (Gas::Oxygen.substance(), 0.21),
///     (Gas::Water.substance(), 0.01),
///     (Gas::Argon.substance(), 0.01),
/// ];
/// let internal = internal_temperature(WattsPerSquareMetre::new(0.09));
/// let skin = skin_temperature(effective_temperature(Kelvin::new(255.0), internal));
/// let earth = RadiativeConvective::derive(&StructureInputs {
///     surface_temperature: Kelvin::new(288.0),
///     surface_pressure: Pascals::new(1.0e5),
///     gases: &gases,
///     condensable: Some(Gas::Water.substance()),
///     skin_temperature: skin,
/// })?;
/// let air = VerticalStructure::RadiativeConvective(earth);
/// let tropopause = match &air {
///     VerticalStructure::RadiativeConvective(profile) => profile.tropopause_pressure(),
/// };
/// assert!((0.12e5..0.25e5).contains(&tropopause.value()));
/// assert!((temperature_at(&air, Pascals::new(1.0e5)).value() - 288.0).abs() < 1e-9);
/// assert!((temperature_at(&air, Pascals::new(1.0e3)).value() - skin.value()).abs() < 1e-9);
/// # Ok::<(), hyperion_sim::planetary::hooks::structure::DeriveStructureError>(())
/// ```
#[must_use]
pub fn temperature_at(structure: &VerticalStructure, p: Pascals) -> Kelvin {
    structure.temperature_at(p)
}

/// What [`RadiativeConvective::derive`] reads, as plain values.
///
/// The fields carry no invariant the derivation relies on: it checks each, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StructureInputs<'a> {
    /// The mean surface temperature `T_s`, where the adiabat starts (P14.T24.a's), finite and
    /// positive.
    pub surface_temperature: Kelvin,
    /// The surface pressure `p_s` (P14.T24.a's), finite and positive for an air.
    pub surface_pressure: Pascals,
    /// The air's gases as `(SubstanceId, x)` mole fractions, summed in the given order: each
    /// finite and non-negative, together 1 within [`MIXTURE_TOLERANCE`] (P14.T24.a's published
    /// list), and every one a gas row.
    pub gases: &'a [(SubstanceId, f64)],
    /// The climate's condensable (P14.T48.d's), which sets α; `None` for none.
    pub condensable: Option<SubstanceId>,
    /// The skin's temperature ([`skin_temperature`]), finite and positive; held at most
    /// `surface_temperature` ([`RadiativeConvective::derive`]).
    pub skin_temperature: Kelvin,
}

impl<'a> StructureInputs<'a> {
    /// The inputs of a surface whose conditions are `conditions`, with its climate regime's
    /// condensable, under an equilibrium temperature `equilibrium` (the record's bulk section's,
    /// from the hosts' light alone) and a heat flow `heat_flow` (P14.T24.b's global figures'), whose
    /// [`internal_temperature`] warms the skin; `None` for conditions without their climate regime
    /// ([`SurfaceConditions::with_climate`]), which no record publishes.
    #[must_use]
    pub fn of_surface(
        conditions: &'a SurfaceConditions,
        equilibrium: Kelvin,
        heat_flow: WattsPerSquareMetre,
    ) -> Option<Self> {
        let regime = conditions.climate_regime()?;
        Some(Self {
            surface_temperature: conditions.mean_surface_temperature(),
            surface_pressure: conditions.surface_pressure(),
            gases: conditions.gases(),
            condensable: regime.condensable(),
            skin_temperature: skin_temperature(effective_temperature(
                equilibrium,
                internal_temperature(heat_flow),
            )),
        })
    }
}

/// Why [`RadiativeConvective::derive`] gives no structure.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum DeriveStructureError {
    /// No air: a surface pressure that is not finite and positive, or no gases. An airless body
    /// has no vertical structure.
    NoAir(Pascals),
    /// A mole fraction that is not finite and non-negative, or fractions that do not sum to 1
    /// within [`MIXTURE_TOLERANCE`].
    Mixture,
    /// A gas with no heat capacity: a row of the mixture without gas columns.
    NoHeatCapacity(SubstanceId),
    /// A surface temperature that is not finite and positive.
    SurfaceTemperature(Kelvin),
    /// A skin temperature that is not finite and positive: no light and no internal heat.
    SkinTemperature(Kelvin),
    /// An adiabat that would take more than [`MAX_SEGMENTS`] segments to reach its skin.
    TooDeep,
}

/// How far from 1 an air's mole fractions may sum: 10⁻⁶, far beyond P14.T24.a's 10⁻⁹ and far
/// within what would move the adiabat, whose S̃ and `c_p` both scale with the sum.
pub const MIXTURE_TOLERANCE: f64 = 1e-6;

impl fmt::Display for DeriveStructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAir(p) => write!(
                f,
                "no air: a surface pressure of {} Pa, or no gases",
                p.value()
            ),
            Self::Mixture => write!(
                f,
                "a mole fraction is not finite and non-negative, or the fractions do not sum to 1"
            ),
            Self::NoHeatCapacity(id) => write!(
                f,
                "{} has no gas columns, so no heat capacity",
                id.substance().key().as_str()
            ),
            Self::SurfaceTemperature(t) => write!(
                f,
                "surface temperature {} K is not finite and positive",
                t.value()
            ),
            Self::SkinTemperature(t) => write!(
                f,
                "skin temperature {} K is not finite and positive",
                t.value()
            ),
            Self::TooDeep => write!(
                f,
                "the adiabat would take more than {MAX_SEGMENTS} segments to reach its skin"
            ),
        }
    }
}

impl Error for DeriveStructureError {}

/// A surface's air as a scaled adiabat from the ground to an isothermal skin (P14.T24.e; the
/// [module](self) documentation): `T_s`, `p_s`, α, `T_skin` and the adiabat's nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct RadiativeConvective {
    surface_temperature: Kelvin,
    surface_pressure: Pascals,
    lapse_factor: f64,
    skin_temperature: Kelvin,
    nodes: Vec<ProfileNode>,
}

impl RadiativeConvective {
    /// The structure of the air `inputs` describe.
    ///
    /// The adiabat's first node is the ground, (`p_s`, `T_s`), and its last the tropopause,
    /// (`p_trop`, `T_skin`) exactly, with ln(`p_s` ÷ `p_trop`) = (S̃(`T_s`) − S̃(`T_skin`)) ÷ α in
    /// closed form; the nodes between are evenly spaced in ln p, each solved by
    /// [`NEWTON_STEPS`] Newton steps in ln T.
    ///
    /// The skin is held at most `T_s`. A grey air with no shortwave absorption warms steadily
    /// downward and its ground emits at least σ`T_eff`⁴, so `T_s` ≥ `T_eff` > `T_skin` (Robinson and
    /// Catling 2012, Eqs. 16, 17 and 24); a skin warmer than the ground comes only from an internal
    /// flux the ground's temperature does not read (P14.T13.c warms `T_s` from `T_eq` alone), and
    /// would put warm air on a colder ground. Such an air, and one whose skin lies within
    /// [`SHORTEST_SEGMENT_LN_P`] a segment of its ground, is isothermal at `T_s`: its adiabat is
    /// its ground node alone, and its skin `T_s`.
    ///
    /// # Errors
    ///
    /// As [`DeriveStructureError`]'s variants: no air, a mixture that is not one, a gas row
    /// without gas columns, a surface or skin temperature that is not finite and positive, or an
    /// adiabat deeper than [`MAX_SEGMENTS`] segments.
    pub fn derive(inputs: &StructureInputs<'_>) -> Result<Self, DeriveStructureError> {
        let p_s = inputs.surface_pressure;
        if !is_positive(p_s.value()) || inputs.gases.is_empty() {
            return Err(DeriveStructureError::NoAir(p_s));
        }
        let mut total = 0.0;
        for &(_, x) in inputs.gases {
            if !(x.is_finite() && x >= 0.0) {
                return Err(DeriveStructureError::Mixture);
            }
            total += x;
        }
        if (total - 1.0).abs().is_nan() || (total - 1.0).abs() > MIXTURE_TOLERANCE {
            return Err(DeriveStructureError::Mixture);
        }
        if let Some(&(id, _)) = inputs
            .gases
            .iter()
            .find(|(id, _)| id.substance().gas().is_none())
        {
            return Err(DeriveStructureError::NoHeatCapacity(id));
        }
        let t_s = inputs.surface_temperature;
        if !is_positive(t_s.value()) {
            return Err(DeriveStructureError::SurfaceTemperature(t_s));
        }
        let skin = inputs.skin_temperature;
        if !is_positive(skin.value()) {
            return Err(DeriveStructureError::SkinTemperature(skin));
        }
        // A skin warmer than the ground is held at it (the doc comment's energy balance).
        let skin = if skin > t_s { t_s } else { skin };
        let gases = inputs.gases;
        let alpha = lapse_factor(gases, t_s, inputs.condensable);
        let ground = ProfileNode {
            pressure: p_s,
            temperature: t_s,
        };
        let surface_entropy = entropy(gases, t_s);
        let span = (surface_entropy - entropy(gases, skin)) / alpha;
        let isothermal = Self {
            surface_temperature: t_s,
            surface_pressure: p_s,
            lapse_factor: alpha,
            skin_temperature: t_s,
            nodes: vec![ground],
        };
        // A skin at the ground's temperature gives no segment.
        if span.is_nan() || span <= 0.0 {
            return Ok(isothermal);
        }
        let segments = segment_count(span).ok_or(DeriveStructureError::TooDeep)?;
        let n = f64::from(segments);
        let step = -span / n;
        // Nor does a skin within rounding of the ground.
        if -step < SHORTEST_SEGMENT_LN_P {
            return Ok(isothermal);
        }
        // A capacity hint only: at most MAX_SEGMENTS + 1 nodes, which fits any usize.
        let mut nodes = Vec::with_capacity(usize::try_from(segments).map_or(0, |n| n + 1));
        nodes.push(ground);
        // Each node's Newton steps start from the node before it, one segment deeper, carried up
        // that node's local power law.
        let (mut previous_temperature, mut previous_heat_capacity) =
            (t_s, heat_capacity(gases, t_s));
        for k in 1..segments {
            let ln_ratio = -(f64::from(k) / n) * span;
            let target = surface_entropy + alpha * ln_ratio;
            let start = Kelvin::new(
                previous_temperature.value() * math::exp(alpha * step / previous_heat_capacity),
            );
            let temperature = adiabat_temperature(gases, target, start);
            nodes.push(ProfileNode {
                pressure: Pascals::new(p_s.value() * math::exp(ln_ratio)),
                temperature,
            });
            previous_temperature = temperature;
            previous_heat_capacity = heat_capacity(gases, temperature);
        }
        nodes.push(ProfileNode {
            pressure: Pascals::new(p_s.value() * math::exp(-span)),
            temperature: skin,
        });
        Ok(Self {
            surface_temperature: t_s,
            surface_pressure: p_s,
            lapse_factor: alpha,
            skin_temperature: skin,
            nodes,
        })
    }

    /// The mean surface temperature `T_s`, the adiabat's start.
    #[must_use]
    pub const fn surface_temperature(&self) -> Kelvin {
        self.surface_temperature
    }

    /// The surface pressure `p_s`.
    #[must_use]
    pub const fn surface_pressure(&self) -> Pascals {
        self.surface_pressure
    }

    /// The lapse factor α ([`lapse_factor`]), dimensionless and positive.
    #[must_use]
    pub const fn lapse_factor(&self) -> f64 {
        self.lapse_factor
    }

    /// The skin's temperature `T_skin`, the temperature above the tropopause, at most `T_s`.
    #[must_use]
    pub const fn skin_temperature(&self) -> Kelvin {
        self.skin_temperature
    }

    /// The adiabat's nodes from the ground to the tropopause, the pressures descending evenly in
    /// ln p: the ground (`p_s`, `T_s`) first and (`p_trop`, `T_skin`) last, or the ground alone where
    /// the air is isothermal.
    #[must_use]
    pub fn nodes(&self) -> &[ProfileNode] {
        &self.nodes
    }

    /// The tropopause's pressure, where the adiabat meets the skin: the last node's; `p_s` where
    /// the air is isothermal.
    #[must_use]
    pub fn tropopause_pressure(&self) -> Pascals {
        self.nodes
            .last()
            .map_or(self.surface_pressure, ProfileNode::pressure)
    }

    /// The temperature at pressure `p`: max(`T_ad`(p), `T_skin`), with `T_ad` the power law
    /// between the two nodes about p, below the ground the first segment's power law, and above
    /// the tropopause the skin's.
    ///
    /// At a node's own pressure it is the node's temperature, bit for bit: the power law of the
    /// segment the node starts, at that segment's own end.
    ///
    /// A pressure at most the tropopause's, zero and negative ones included, gives `T_skin`, and
    /// a NaN pressure a NaN temperature. A structure whose adiabat is its ground node alone is
    /// isothermal at its skin, which is then `T_s`.
    #[must_use]
    pub fn temperature_at(&self, p: Pascals) -> Kelvin {
        let p = p.value();
        if p.is_nan() {
            return Kelvin::new(f64::NAN);
        }
        let skin = self.skin_temperature;
        // The first node above p in height, so that a node's own pressure takes the segment it
        // starts, where the power law gives its temperature exactly.
        let upper = self
            .nodes
            .partition_point(|node| node.pressure.value() >= p);
        let (from, to) = match (upper, self.nodes.len()) {
            (_, 0 | 1) => return skin,
            (0, _) => (0, 1),
            (upper, len) if upper >= len => return skin,
            (upper, _) => (upper - 1, upper),
        };
        let (a, b) = (self.nodes[from], self.nodes[to]);
        let exponent = math::ln(b.temperature.value() / a.temperature.value())
            / math::ln(b.pressure.value() / a.pressure.value());
        let t = a.temperature.value() * math::exp(exponent * math::ln(p / a.pressure.value()));
        if t < skin.value() {
            skin
        } else {
            Kelvin::new(t)
        }
    }
}

/// The lapse factor α of an air of mole fractions `gases` over a ground at `surface_temperature`
/// whose climate's condensable is `condensable` (P14.T48.d's; the [module](self) documentation).
///
/// [`WATER_LAPSE_FACTOR`] for water and [`METHANE_LAPSE_FACTOR`] for methane where their
/// [`latent_heat_departure`] at the ground is at least [`LATENT_HEAT_GATE`], and
/// [`DRY_LAPSE_FACTOR`] otherwise: for carbon dioxide (Robinson and Catling 2012's Venus), for
/// every other condensable, for none, and for a water or methane too scarce or too warm to drive a
/// moist lapse rate.
///
/// # Examples
///
/// A Mars's air holds water at about 210 ppm, against Earth's percent, and takes the dry factor:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::planetary::hooks::structure::{DRY_LAPSE_FACTOR, lapse_factor};
/// use hyperion_sim::units::Kelvin;
///
/// let water = Gas::Water.substance();
/// let mars = [
///     (Gas::CarbonDioxide.substance(), 0.951),
///     (Gas::Nitrogen.substance(), 0.028),
///     (Gas::Argon.substance(), 0.0208),
///     (water, 2.1e-4),
/// ];
/// assert_eq!(lapse_factor(&mars, Kelvin::new(210.0), Some(water)), DRY_LAPSE_FACTOR);
/// ```
#[must_use]
pub fn lapse_factor(
    gases: &[(SubstanceId, f64)],
    surface_temperature: Kelvin,
    condensable: Option<SubstanceId>,
) -> f64 {
    let Some(id) = condensable else {
        return DRY_LAPSE_FACTOR;
    };
    let moist = if id == Gas::Water.substance() {
        WATER_LAPSE_FACTOR
    } else if id == Gas::Methane.substance() {
        METHANE_LAPSE_FACTOR
    } else {
        return DRY_LAPSE_FACTOR;
    };
    match latent_heat_departure(gases, surface_temperature, id) {
        Some(departure) if departure >= LATENT_HEAT_GATE => moist,
        Some(_) | None => DRY_LAPSE_FACTOR,
    }
}

/// How far the condensable `condensable`'s latent heat can move the lapse rate of an air of mole
/// fractions `gases` at the ground, at `surface_temperature`: ε L² r ÷ (`c_p` R T²), the moist
/// adiabat's departure from the dry one (signoff-3.md, item 1.5); `None` where it cannot be read
/// (a condensable with no phase data or molar mass, an air with no molar mass or heat capacity, a
/// temperature that is not finite and positive).
///
/// It is the term L² r ÷ (`c_pa` `R_c` T²) of the dilute pseudoadiabat, the limit of Ding and
/// Pierrehumbert's (2016, ApJ 822, 24, Eq. 12, read at source, arXiv:1608.03216) where the
/// condensable's partial pressure is small against the air's:
///
/// ```text
/// dln T ÷ dln p = (R_a ÷ c_pa) (1 + L r ÷ (R_a T)) ÷ (1 + L² r ÷ (c_pa R_c T²))
/// ```
///
/// with L, `c_pa`, `R_a` and `R_c` = R ÷ `M_c` per unit mass and r the mass mixing ratio over the
/// rest of the air. Since ε ÷ `R_a` = 1 ÷ `R_c`, the gate's form is that term exactly where `c_p`
/// is the whole air's, and Ding and Pierrehumbert's, whose `c_pa` is the non-condensable's, to
/// first order in x (0.9% apart for Earth's water, 3.2% for Titan's methane). Per mole it is
///
/// ```text
/// (L ÷ (R T))² x ÷ (c_p ÷ R × (1 − x M_c ÷ M))
/// ```
///
/// with L the molar latent heat, x the condensable's mole fraction, `M_c` its molar mass and M and
/// `c_p` the air's. L is the registry row's enthalpy of the branch at the ground
/// ([`PhaseColumns`](crate::substance::PhaseColumns)): vaporisation at or above the triple point,
/// sublimation below, and zero at or above the critical temperature, where no liquid forms. For a
/// condensable that makes up the whole air (x `M_c` ≥ M) the departure diverges: its lapse rate is
/// then its saturation curve's, which no α draws (Robinson and Catling 2012: "very moist
/// atmospheres will have temperature varying as 1/ln(p/p0)").
///
/// Signoff-3.md's figures, by hand: about 1.6 for Earth's water, 1.8 for Titan's methane and 0.05
/// for a Mars's water; the registry's rows give 1.56, 2.07 and 0.046 (this module's tests).
///
/// Its limits, recorded in plan 14's Risks:
/// - the row's branch enthalpy is a constant, its triple point's or an effective value, which
///   overstates L as T nears the critical temperature, where L falls to zero; so the departure
///   steps to zero at `T_c` rather than falling to it. P14.T49.b's fitted enthalpies take the
///   constants' place at the 22 → 23 batch's wiring (P14.T54.a), when they replace the phase
///   columns;
/// - it is read at the ground alone: an air whose ground is above its condensable's critical
///   temperature but whose upper air condenses takes the dry α.
///
/// # Examples
///
/// Earth's water, saturated at 288 K under 1 bar, can drive a moist lapse rate; a Mars's 210 ppm
/// cannot:
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::planetary::hooks::structure::{LATENT_HEAT_GATE, latent_heat_departure};
/// use hyperion_sim::substance::saturation_pressure;
/// use hyperion_sim::units::Kelvin;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// let water = Gas::Water.substance();
/// let x = saturation_pressure(water, Kelvin::new(288.0))?.value() / 1.0e5;
/// let earth = [
///     (Gas::Nitrogen.substance(), 0.78),
///     (Gas::Oxygen.substance(), 0.21 - x),
///     (water, x),
///     (Gas::Argon.substance(), 0.01),
/// ];
/// assert!(latent_heat_departure(&earth, Kelvin::new(288.0), water)? > LATENT_HEAT_GATE);
/// let mars = [
///     (Gas::CarbonDioxide.substance(), 0.951),
///     (Gas::Nitrogen.substance(), 0.028),
///     (Gas::Argon.substance(), 0.0208),
///     (water, 2.1e-4),
/// ];
/// assert!(latent_heat_departure(&mars, Kelvin::new(210.0), water)? < LATENT_HEAT_GATE);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn latent_heat_departure(
    gases: &[(SubstanceId, f64)],
    surface_temperature: Kelvin,
    condensable: SubstanceId,
) -> Option<f64> {
    let t = surface_temperature.value();
    if !is_positive(t) {
        return None;
    }
    let phase = condensable.substance().phase()?;
    let condensable_mass = condensable.substance().molar_mass_g_per_mol()?;
    let x = gases
        .iter()
        .find(|&&(id, _)| id == condensable)
        .map_or(0.0, |&(_, x)| x);
    let air_mass = mean_molar_mass_g_per_mol(gases)?;
    let heat_capacity = mixture_heat_capacity_over_r_at(gases, surface_temperature)?;
    let latent = if t >= phase.critical_temperature().value() {
        0.0
    } else if t < phase.triple_temperature().value() {
        phase.sublimation_enthalpy_j_per_mol()
    } else {
        phase.vaporisation_enthalpy_j_per_mol()
    };
    let rest = 1.0 - x * condensable_mass / air_mass;
    if rest.is_nan() || rest <= 0.0 {
        return Some(f64::INFINITY);
    }
    let ratio = latent / (MOLAR_GAS_CONSTANT * t);
    Some(ratio * ratio * x / (heat_capacity * rest))
}

/// The skin temperature of a grey atmosphere of effective temperature `effective`:
/// 2^(−1/4) `T_eff`, the τ → 0 limit of its radiative equilibrium (Robinson and Catling 2012, ApJ
/// 757, 104, Eq. 24 at τ = 0: σT⁴ = (`F_net` + `F_i`) ÷ 2, with σ`T_eff`⁴ = `F_net` + `F_i`; read at
/// source; decision-composition §1.12).
///
/// # Examples
///
/// Earth's 255 K gives a 214.4 K skin, as P14.T13.c's grey Eddington atmosphere has it:
///
/// ```
/// use hyperion_sim::planetary::hooks::structure::skin_temperature;
/// use hyperion_sim::units::Kelvin;
///
/// let skin = skin_temperature(Kelvin::new(255.0));
/// assert!((skin.value() - 214.43).abs() < 0.01);
/// ```
#[must_use]
pub fn skin_temperature(effective: Kelvin) -> Kelvin {
    Kelvin::new(effective.value() * math::powf(0.5, 0.25))
}

/// The effective temperature of a body of equilibrium temperature `equilibrium` and internal
/// temperature `internal`: (`T_eq`⁴ + `T_int`⁴)^¼, what it radiates, the absorbed starlight and the
/// internal flux together (decision-composition §1.12; ruling 112.7's split).
#[must_use]
pub fn effective_temperature(equilibrium: Kelvin, internal: Kelvin) -> Kelvin {
    let (eq, int) = (equilibrium.value(), internal.value());
    Kelvin::new(math::powf(math::powi(eq, 4) + math::powi(int, 4), 0.25))
}

/// The internal temperature of a solid body whose heat flow is `heat_flow`: (F ÷ σ)^¼
/// (decision-composition §1.12), with σ the Stefan–Boltzmann constant
/// ([`STEFAN_BOLTZMANN`]); zero for a heat flow that is not finite and positive.
///
/// # Examples
///
/// Earth's 0.09 W m⁻² is 35.5 K:
///
/// ```
/// use hyperion_sim::planetary::hooks::structure::internal_temperature;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let t = internal_temperature(WattsPerSquareMetre::new(0.09));
/// assert!((t.value() - 35.5).abs() < 0.05);
/// ```
#[must_use]
pub fn internal_temperature(heat_flow: WattsPerSquareMetre) -> Kelvin {
    let f = heat_flow.value();
    if !is_positive(f) {
        return Kelvin::new(0.0);
    }
    Kelvin::new(math::powf(f / STEFAN_BOLTZMANN, 0.25))
}

/// The segments an adiabat spanning `span` e-folds of pressure takes: max([`MIN_SEGMENTS`],
/// ⌈`span` ÷ [`NODE_SPACING_LN_P`]⌉); `None` beyond [`MAX_SEGMENTS`] or for a span that is not
/// finite and positive.
#[must_use]
fn segment_count(span: f64) -> Option<u32> {
    let needed = (span / NODE_SPACING_LN_P).ceil();
    if needed.is_nan() || needed <= 0.0 || needed > f64::from(MAX_SEGMENTS) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number in 1..=MAX_SEGMENTS, checked above, converts exactly"
    )]
    let needed = needed as u32;
    Some(needed.max(MIN_SEGMENTS))
}

/// The temperature at which the air `gases` has the entropy `target` (S̃ ÷ R), by
/// [`NEWTON_STEPS`] Newton steps in ln T from `start`.
///
/// In u = ln T, dS̃ ÷ du = `c_p` ÷ R, at least 5/2 of a gas, so each step is
/// u ← u − (S̃(T) − target) ÷ (`c_p` ÷ R).
#[must_use]
fn adiabat_temperature(gases: &[(SubstanceId, f64)], target: f64, start: Kelvin) -> Kelvin {
    let mut u = math::ln(start.value());
    for _ in 0..NEWTON_STEPS {
        let t = Kelvin::new(math::exp(u));
        u -= (entropy(gases, t) - target) / heat_capacity(gases, t);
    }
    Kelvin::new(math::exp(u))
}

/// The air's S̃ ÷ R at `temperature`, every row a gas and the temperature positive (checked by
/// [`RadiativeConvective::derive`]); NaN otherwise.
#[must_use]
fn entropy(gases: &[(SubstanceId, f64)], temperature: Kelvin) -> f64 {
    mixture_entropy_over_r(gases, temperature).unwrap_or(f64::NAN)
}

/// The air's `c_p` ÷ R at `temperature`, as [`entropy`].
#[must_use]
fn heat_capacity(gases: &[(SubstanceId, f64)], temperature: Kelvin) -> f64 {
    mixture_heat_capacity_over_r_at(gases, temperature).unwrap_or(f64::NAN)
}

/// Whether `x` is finite and positive.
#[must_use]
fn is_positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}
