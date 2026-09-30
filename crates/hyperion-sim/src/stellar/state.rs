//! What a star is at one age: its evolutionary [`Phase`], and the [`StarState`] of masses,
//! luminosity, radius and temperature that the evolution code returns for it.
//!
//! The phases are those of Hurley, Pols and Tout (2000, MNRAS 315, 543; "HPT"), whose stellar
//! types 0–15 each have a variant, with the stages this plan wraps around them: the protostar and
//! the pre-main sequence before, the post-AGB crossing between the asymptotic giant branch and the
//! white dwarf, and substellar objects below 0.1 M☉. [`ObjectKind`] is the coarser answer to "what
//! is this object now" that the range query and the chart symbols read.

use crate::math;
use crate::units::consts::{GM_SUN, SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_RADIUS_M};
use crate::units::{
    Dex, Kelvin, SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years,
};

/// A star's evolutionary phase.
///
/// The HPT stellar type is named in each variant's documentation where one applies. HPT's types 0
/// and 1 (main-sequence stars deeply or fully convective below about 0.7 M☉, with little or no
/// convective envelope above; HPT section 4) are one variant here, since their luminosity, radius
/// and lifetime formulae are the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    /// Class 0 and I: from the onset of collapse, still gaining mass (the first 0.5 Myr, the
    /// brainstorm's "Covering every class of star", row "Before the main sequence").
    Protostar,
    /// Contraction towards the zero-age main sequence: T Tauri and Herbig Ae/Be stars.
    PreMainSequence,
    /// Core hydrogen burning (HPT types 0 and 1).
    MainSequence,
    /// The Hertzsprung gap, from core hydrogen exhaustion to the base of the giant branch (type 2).
    HertzsprungGap,
    /// The first giant branch, hydrogen burning in a shell around an inert helium core (type 3).
    FirstGiantBranch,
    /// Core helium burning: the horizontal branch, the red clump and the blue loops (type 4).
    CoreHeliumBurning,
    /// The early asymptotic giant branch, helium burning in a shell (type 5).
    EarlyAgb,
    /// The thermally pulsing asymptotic giant branch (type 6).
    ThermallyPulsingAgb,
    /// A naked helium star burning helium in its core (type 7).
    HeliumMainSequence,
    /// A naked helium star crossing the Hertzsprung gap (type 8).
    HeliumHertzsprungGap,
    /// A naked helium star on its giant branch (type 9).
    HeliumGiantBranch,
    /// The crossing from the end of the asymptotic giant branch to the white dwarf cooling track,
    /// at nearly constant luminosity; a star here may light a planetary nebula.
    PostAgb,
    /// A helium white dwarf (type 10).
    HeliumWhiteDwarf,
    /// A carbon–oxygen white dwarf (type 11).
    CarbonOxygenWhiteDwarf,
    /// An oxygen–neon white dwarf (type 12).
    OxygenNeonWhiteDwarf,
    /// A neutron star (type 13).
    NeutronStar,
    /// A black hole (type 14).
    BlackHole,
    /// Nothing: the star was destroyed and left no remnant (type 15, a massless remnant).
    ///
    /// In HPT this follows the degenerate carbon ignition of a carbon–oxygen core that reaches
    /// the Chandrasekhar mass on the asymptotic giant branch (section 6, after equation 75); under
    /// the generator's default it also follows pair instability (plan 06, design note 10).
    NoRemnant,
    /// An object below 0.1 M☉ on the cooling fits of Burrows et al. (2001): the latest M dwarfs,
    /// and below the hydrogen-burning limit the brown dwarfs of classes L, T and Y.
    Substellar,
}

impl Phase {
    /// Every phase, in declaration order.
    pub const ALL: [Self; 19] = [
        Self::Protostar,
        Self::PreMainSequence,
        Self::MainSequence,
        Self::HertzsprungGap,
        Self::FirstGiantBranch,
        Self::CoreHeliumBurning,
        Self::EarlyAgb,
        Self::ThermallyPulsingAgb,
        Self::HeliumMainSequence,
        Self::HeliumHertzsprungGap,
        Self::HeliumGiantBranch,
        Self::PostAgb,
        Self::HeliumWhiteDwarf,
        Self::CarbonOxygenWhiteDwarf,
        Self::OxygenNeonWhiteDwarf,
        Self::NeutronStar,
        Self::BlackHole,
        Self::NoRemnant,
        Self::Substellar,
    ];

    /// Whether this is what a star leaves when it dies: a white dwarf, a neutron star, a black
    /// hole, or nothing.
    ///
    /// Exactly the phases for which [`Phase::is_living`] is false.
    #[must_use]
    pub const fn is_remnant(self) -> bool {
        match self {
            Self::HeliumWhiteDwarf
            | Self::CarbonOxygenWhiteDwarf
            | Self::OxygenNeonWhiteDwarf
            | Self::NeutronStar
            | Self::BlackHole
            | Self::NoRemnant => true,
            Self::Protostar
            | Self::PreMainSequence
            | Self::MainSequence
            | Self::HertzsprungGap
            | Self::FirstGiantBranch
            | Self::CoreHeliumBurning
            | Self::EarlyAgb
            | Self::ThermallyPulsingAgb
            | Self::HeliumMainSequence
            | Self::HeliumHertzsprungGap
            | Self::HeliumGiantBranch
            | Self::PostAgb
            | Self::Substellar => false,
        }
    }

    /// Whether the object has not yet died: every phase from the protostar to the post-AGB
    /// crossing, and substellar objects, which never die.
    ///
    /// Exactly the phases for which [`Phase::is_remnant`] is false.
    #[must_use]
    pub const fn is_living(self) -> bool {
        match self {
            Self::Protostar
            | Self::PreMainSequence
            | Self::MainSequence
            | Self::HertzsprungGap
            | Self::FirstGiantBranch
            | Self::CoreHeliumBurning
            | Self::EarlyAgb
            | Self::ThermallyPulsingAgb
            | Self::HeliumMainSequence
            | Self::HeliumHertzsprungGap
            | Self::HeliumGiantBranch
            | Self::PostAgb
            | Self::Substellar => true,
            Self::HeliumWhiteDwarf
            | Self::CarbonOxygenWhiteDwarf
            | Self::OxygenNeonWhiteDwarf
            | Self::NeutronStar
            | Self::BlackHole
            | Self::NoRemnant => false,
        }
    }
}

/// The inputs from which a [`StarState`] is built; the state derives its envelope mass and
/// effective temperature from them.
///
/// Every part is required and [`StarState::new`] checks them, so a struct literal, which fails to
/// compile when a part is missing, stands in for a builder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarStateParts {
    /// The evolutionary phase.
    pub phase: Phase,
    /// Age since the onset of collapse (plan 06, design note 4), Julian years, finite and
    /// non-negative.
    pub age: Years,
    /// Current total mass, M☉, non-negative.
    pub mass: SolarMasses,
    /// Current core mass, M☉, from zero to [`StarStateParts::mass`]: the helium core on the giant
    /// branch, the carbon–oxygen core of a helium star, the whole mass of a remnant.
    pub core_mass: SolarMasses,
    /// Bolometric luminosity, L☉, non-negative.
    pub luminosity: SolarLuminosities,
    /// Photospheric radius (a remnant's physical radius), R☉, non-negative and positive whenever
    /// the luminosity is.
    pub radius: SolarRadii,
    /// Rate of mass loss by the wind, M☉ per year; positive for loss, negative while a protostar
    /// is still gaining mass.
    pub mass_loss_rate: SolarMassesPerYear,
    /// How far the star is through its current phase, 0–1.
    pub phase_fraction: f64,
}

/// A star's state at one age: phase, masses, luminosity, radius, effective temperature, wind and
/// progress through the phase.
///
/// The effective temperature is derived from luminosity and radius by Stefan–Boltzmann,
/// T = T☉ (L ÷ L☉)^¼ (R ÷ R☉)^−½, with the nominal T☉ = 5,772 K of IAU 2015 Resolution B3
/// ([`SOLAR_EFFECTIVE_TEMPERATURE_K`]); an object with no luminosity has none.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::{Phase, StarState, StarStateParts};
/// use hyperion_sim::units::{SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years};
///
/// // A red giant: fifty times the Sun's luminosity spread over ten times its radius is cooler,
/// // about 4,850 K.
/// let giant = StarState::new(StarStateParts {
///     phase: Phase::FirstGiantBranch,
///     age: Years::new(1.2e10),
///     mass: SolarMasses::new(1.0),
///     core_mass: SolarMasses::new(0.3),
///     luminosity: SolarLuminosities::new(50.0),
///     radius: SolarRadii::new(10.0),
///     mass_loss_rate: SolarMassesPerYear::new(1e-9),
///     phase_fraction: 0.4,
/// });
/// assert!((giant.effective_temperature().value() - 4_853.65).abs() < 0.01);
/// assert!((giant.envelope_mass().value() - 0.7).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarState {
    phase: Phase,
    age: Years,
    mass: SolarMasses,
    core_mass: SolarMasses,
    envelope_mass: SolarMasses,
    luminosity: SolarLuminosities,
    radius: SolarRadii,
    effective_temperature: Kelvin,
    mass_loss_rate: SolarMassesPerYear,
    phase_fraction: f64,
}

impl StarState {
    /// Builds the state from its parts, deriving the envelope mass (mass − core mass, never
    /// negative) and the effective temperature.
    ///
    /// # Panics
    ///
    /// In debug builds, if a part is not finite or is outside the range [`StarStateParts`]
    /// documents for it.
    #[must_use]
    pub fn new(parts: StarStateParts) -> Self {
        let StarStateParts {
            phase,
            age,
            mass,
            core_mass,
            luminosity,
            radius,
            mass_loss_rate,
            phase_fraction,
        } = parts;
        debug_assert!(
            age.value().is_finite() && age.value() >= 0.0,
            "age must be finite and non-negative: {age:?}"
        );
        debug_assert!(
            !phase.is_remnant() || (mass.value() - core_mass.value()).abs() <= mass.value() * 1e-12,
            "a remnant is all core: {core_mass:?} of {mass:?}"
        );
        debug_assert!(
            phase != Phase::NoRemnant || (mass.value() <= 0.0 && luminosity.value() <= 0.0),
            "nothing is left where there is no remnant"
        );
        debug_assert!(
            mass.value().is_finite() && mass.value() >= 0.0,
            "mass must be finite and non-negative: {mass:?}"
        );
        debug_assert!(
            core_mass.value() >= 0.0 && core_mass.value() <= mass.value() * (1.0 + 1e-12),
            "core mass must lie in [0, mass]: {core_mass:?} of {mass:?}"
        );
        debug_assert!(
            luminosity.value().is_finite() && luminosity.value() >= 0.0,
            "luminosity must be finite and non-negative: {luminosity:?}"
        );
        debug_assert!(
            radius.value().is_finite() && radius.value() >= 0.0,
            "radius must be finite and non-negative: {radius:?}"
        );
        debug_assert!(
            luminosity.value() <= 0.0 || radius.value() > 0.0,
            "a luminous object has a radius"
        );
        debug_assert!(
            mass_loss_rate.value().is_finite(),
            "mass-loss rate must be finite"
        );
        debug_assert!(
            (0.0..=1.0).contains(&phase_fraction),
            "phase fraction must lie in [0, 1]: {phase_fraction}"
        );
        // Not `f64::max`, which may return either zero when both are zeros of opposite sign.
        let outside = mass.value() - core_mass.value();
        Self {
            phase,
            age,
            mass,
            core_mass,
            envelope_mass: SolarMasses::new(if outside > 0.0 { outside } else { 0.0 }),
            luminosity,
            radius,
            effective_temperature: effective_temperature(luminosity, radius),
            mass_loss_rate,
            phase_fraction,
        }
    }

    /// The evolutionary phase.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Age since the onset of collapse, Julian years.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// Current total mass, M☉.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// Current core mass, M☉.
    #[must_use]
    pub const fn core_mass(&self) -> SolarMasses {
        self.core_mass
    }

    /// Envelope mass, M☉: the mass outside the core, never negative.
    #[must_use]
    pub const fn envelope_mass(&self) -> SolarMasses {
        self.envelope_mass
    }

    /// Bolometric luminosity, L☉: positive for a living star, and exactly zero for a black hole
    /// and where nothing is left ([`Phase::NoRemnant`]), so a consumer that takes its logarithm
    /// (a classification, an irradiation) must check it first (ruling 40 of 2026-09-22).
    #[must_use]
    pub const fn luminosity(&self) -> SolarLuminosities {
        self.luminosity
    }

    /// Radius, R☉.
    #[must_use]
    pub const fn radius(&self) -> SolarRadii {
        self.radius
    }

    /// Effective temperature, K, from luminosity and radius; zero for an object with no
    /// luminosity.
    #[must_use]
    pub const fn effective_temperature(&self) -> Kelvin {
        self.effective_temperature
    }

    /// Rate of mass loss by the wind, M☉ per year; negative while a protostar gains mass.
    #[must_use]
    pub const fn mass_loss_rate(&self) -> SolarMassesPerYear {
        self.mass_loss_rate
    }

    /// How far the star is through its current phase, 0–1.
    #[must_use]
    pub const fn phase_fraction(&self) -> f64 {
        self.phase_fraction
    }

    /// Surface gravity as the astronomer's log g: log₁₀ of g = GM ÷ R² in cm s⁻² (not m s⁻²).
    ///
    /// The nominal GM☉ and R☉ of IAU 2015 Resolution B3 give the Sun 4.438. `None` for an object
    /// with no mass or no radius, which has no surface.
    #[must_use]
    pub fn surface_gravity(&self) -> Option<Dex> {
        if self.mass.value() <= 0.0 || self.radius.value() <= 0.0 {
            return None;
        }
        let r_m = self.radius.value() * SOLAR_RADIUS_M;
        let g_si = GM_SUN * self.mass.value() / (r_m * r_m);
        Some(Dex::new(math::log10(g_si * CENTIMETRES_PER_METRE)))
    }
}

/// Centimetres in one metre: log g is quoted in cgs units.
const CENTIMETRES_PER_METRE: f64 = 100.0;

/// Stefan–Boltzmann in solar units: T = T☉ L^¼ R^−½. The fourth root is two square roots, which
/// IEEE 754 rounds exactly on every target, so no transcendental is needed. The arithmetic form,
/// (T☉ × √√L) ÷ √R, is output: an algebraically equal rewrite moves every temperature.
#[must_use]
fn effective_temperature(luminosity: SolarLuminosities, radius: SolarRadii) -> Kelvin {
    let l = luminosity.value();
    if l <= 0.0 {
        return Kelvin::ZERO;
    }
    Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K * l.sqrt().sqrt() / radius.value().sqrt())
}

/// What an object is now, in the terms of the range query's rows and the chart's symbols (plan
/// 06, design note 17): coarser than [`Phase`], and read from the phase and the classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectKind {
    /// A protostar, Class 0 or I.
    Protostar,
    /// A pre-main-sequence star: T Tauri or Herbig Ae/Be.
    PreMainSequence,
    /// A main-sequence star, luminosity class V.
    Dwarf,
    /// A subgiant, luminosity class IV.
    Subgiant,
    /// A giant, luminosity class III or II.
    Giant,
    /// A supergiant, luminosity class I.
    Supergiant,
    /// A Wolf-Rayet star: a hot, stripped helium star with a dense wind.
    WolfRayet,
    /// A hot subdwarf (sdB, sdO): a helium-burning core with almost no envelope.
    HotSubdwarf,
    /// A white dwarf of any composition.
    WhiteDwarf,
    /// A neutron star.
    NeutronStar,
    /// A black hole.
    BlackHole,
    /// Nothing: the star left no remnant.
    NoRemnant,
    /// A brown dwarf or other object below the hydrogen-burning limit.
    Substellar,
}

impl ObjectKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 13] = [
        Self::Protostar,
        Self::PreMainSequence,
        Self::Dwarf,
        Self::Subgiant,
        Self::Giant,
        Self::Supergiant,
        Self::WolfRayet,
        Self::HotSubdwarf,
        Self::WhiteDwarf,
        Self::NeutronStar,
        Self::BlackHole,
        Self::NoRemnant,
        Self::Substellar,
    ];
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn state(luminosity: f64, radius: f64) -> StarState {
        StarState::new(StarStateParts {
            phase: Phase::MainSequence,
            age: Years::new(4.57e9),
            mass: SolarMasses::new(1.0),
            core_mass: SolarMasses::ZERO,
            luminosity: SolarLuminosities::new(luminosity),
            radius: SolarRadii::new(radius),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    #[test]
    fn the_suns_luminosity_and_radius_give_its_nominal_temperature() {
        let sun = state(1.0, 1.0);
        assert!((sun.effective_temperature().value() - 5_772.0).abs() < 1.0);
        assert_same_bits(sun.effective_temperature().value(), 5_772.0);
    }

    #[test]
    fn temperature_follows_stefan_boltzmann() {
        // L ∝ R²T⁴: sixteen times the luminosity at the same radius doubles the temperature, and
        // four times the radius at the same luminosity halves it.
        assert_same_bits(state(16.0, 1.0).effective_temperature().value(), 11_544.0);
        assert_same_bits(state(1.0, 4.0).effective_temperature().value(), 2_886.0);
        let t = state(3.7, 0.42).effective_temperature().value();
        let expected = 5_772.0 * math::powf(3.7, 0.25) * math::powf(0.42, -0.5);
        assert!((t - expected).abs() < 1e-9, "{t} against {expected}");
    }

    #[test]
    fn an_object_without_luminosity_has_no_temperature() {
        let dark = StarState::new(StarStateParts {
            phase: Phase::BlackHole,
            age: Years::new(1e9),
            mass: SolarMasses::new(10.0),
            core_mass: SolarMasses::new(10.0),
            luminosity: SolarLuminosities::ZERO,
            radius: SolarRadii::new(4.2e-5),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.0,
        });
        assert_same_bits(dark.effective_temperature().value(), 0.0);
        assert_same_bits(dark.envelope_mass().value(), 0.0);
        let nothing = StarState::new(StarStateParts {
            phase: Phase::NoRemnant,
            age: Years::new(1e9),
            mass: SolarMasses::ZERO,
            core_mass: SolarMasses::ZERO,
            luminosity: SolarLuminosities::ZERO,
            radius: SolarRadii::ZERO,
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.0,
        });
        assert_same_bits(nothing.effective_temperature().value(), 0.0);
        assert_eq!(nothing.surface_gravity(), None);
    }

    #[test]
    fn the_suns_surface_gravity_is_log_g_4_44() {
        let log_g = state(1.0, 1.0).surface_gravity().unwrap().value();
        assert!((log_g - 4.438).abs() < 5e-4, "{log_g}");
        // A 1 M☉ giant of 10 R☉ is a hundred times weaker.
        let giant = state(50.0, 10.0).surface_gravity().unwrap().value();
        assert!((log_g - giant - 2.0).abs() < 1e-12);
    }

    #[test]
    fn envelope_mass_is_what_lies_outside_the_core() {
        let s = StarState::new(StarStateParts {
            phase: Phase::FirstGiantBranch,
            age: Years::new(1.1e10),
            mass: SolarMasses::new(0.9),
            core_mass: SolarMasses::new(0.35),
            luminosity: SolarLuminosities::new(80.0),
            radius: SolarRadii::new(15.0),
            mass_loss_rate: SolarMassesPerYear::new(2e-9),
            phase_fraction: 0.6,
        });
        assert!((s.envelope_mass().value() - 0.55).abs() < 1e-15);
        assert_eq!(s.phase(), Phase::FirstGiantBranch);
        assert_same_bits(s.age().value(), 1.1e10);
        assert_same_bits(s.mass().value(), 0.9);
        assert_same_bits(s.core_mass().value(), 0.35);
        assert_same_bits(s.luminosity().value(), 80.0);
        assert_same_bits(s.radius().value(), 15.0);
        assert_same_bits(s.mass_loss_rate().value(), 2e-9);
        assert_same_bits(s.phase_fraction(), 0.6);
    }

    #[test]
    fn remnant_and_living_partition_the_phases() {
        let mut remnants = 0;
        for phase in Phase::ALL {
            assert_ne!(phase.is_remnant(), phase.is_living(), "{phase:?}");
            // The exhaustive match: a new variant must be placed here as well as in both methods.
            let expected_remnant = match phase {
                Phase::HeliumWhiteDwarf
                | Phase::CarbonOxygenWhiteDwarf
                | Phase::OxygenNeonWhiteDwarf
                | Phase::NeutronStar
                | Phase::BlackHole
                | Phase::NoRemnant => true,
                Phase::Protostar
                | Phase::PreMainSequence
                | Phase::MainSequence
                | Phase::HertzsprungGap
                | Phase::FirstGiantBranch
                | Phase::CoreHeliumBurning
                | Phase::EarlyAgb
                | Phase::ThermallyPulsingAgb
                | Phase::HeliumMainSequence
                | Phase::HeliumHertzsprungGap
                | Phase::HeliumGiantBranch
                | Phase::PostAgb
                | Phase::Substellar => false,
            };
            assert_eq!(phase.is_remnant(), expected_remnant, "{phase:?}");
            remnants += usize::from(phase.is_remnant());
        }
        assert_eq!(remnants, 6);
    }

    /// An exhaustive match gives each variant's place, so a new variant fails to compile here
    /// until it has one, and `ALL` must then list it there.
    #[test]
    fn every_phase_is_listed_once_in_declaration_order() {
        let index = |phase: Phase| match phase {
            Phase::Protostar => 0,
            Phase::PreMainSequence => 1,
            Phase::MainSequence => 2,
            Phase::HertzsprungGap => 3,
            Phase::FirstGiantBranch => 4,
            Phase::CoreHeliumBurning => 5,
            Phase::EarlyAgb => 6,
            Phase::ThermallyPulsingAgb => 7,
            Phase::HeliumMainSequence => 8,
            Phase::HeliumHertzsprungGap => 9,
            Phase::HeliumGiantBranch => 10,
            Phase::PostAgb => 11,
            Phase::HeliumWhiteDwarf => 12,
            Phase::CarbonOxygenWhiteDwarf => 13,
            Phase::OxygenNeonWhiteDwarf => 14,
            Phase::NeutronStar => 15,
            Phase::BlackHole => 16,
            Phase::NoRemnant => 17,
            Phase::Substellar => 18,
        };
        for (i, phase) in Phase::ALL.into_iter().enumerate() {
            assert_eq!(index(phase), i, "{phase:?}");
        }
    }

    #[test]
    fn every_object_kind_is_listed_once_in_declaration_order() {
        let index = |kind: ObjectKind| match kind {
            ObjectKind::Protostar => 0,
            ObjectKind::PreMainSequence => 1,
            ObjectKind::Dwarf => 2,
            ObjectKind::Subgiant => 3,
            ObjectKind::Giant => 4,
            ObjectKind::Supergiant => 5,
            ObjectKind::WolfRayet => 6,
            ObjectKind::HotSubdwarf => 7,
            ObjectKind::WhiteDwarf => 8,
            ObjectKind::NeutronStar => 9,
            ObjectKind::BlackHole => 10,
            ObjectKind::NoRemnant => 11,
            ObjectKind::Substellar => 12,
        };
        for (i, kind) in ObjectKind::ALL.into_iter().enumerate() {
            assert_eq!(index(kind), i, "{kind:?}");
        }
    }
}
