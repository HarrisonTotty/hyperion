//! Classification: a star's spectral type (O to M with subclass, then L, T and Y) and luminosity
//! class (Ia⁺ to V, and the subdwarfs) from its temperature, luminosity, gravity and surface
//! composition, calibrated on Pecaut and Mamajek (2013, Astrophysical Journal Supplement 208, 9),
//! with the white dwarfs' own types and the remnants that have none (plan 06, P06.T23).
//!
//! - [`subtype_from_teff`] is the dwarf scale: the continuous subtype of a class V star at a
//!   temperature, from the mean dwarf sequence of Pecaut and Mamajek in Mamajek's maintained
//!   version 2022.04.16 (`pm13`).
//! - The luminosity class and the temperature scales of the other classes are in `luminosity`:
//!   the class from gravity (V, IV or brighter) and from luminosity at the star's temperature (III
//!   to Ia) after Straižys and Kuriliene (1981), Ia⁺ near the Humphreys–Davidson limit, and the
//!   giant and supergiant temperature scales against which a non-dwarf's subtype is read.
//! - White dwarfs take the types of Sion et al. (1983) from
//!   [`wd_spectral`](crate::stellar::remnant::wd_spectral) (P06.T20.b); neutron stars and black
//!   holes have no spectral type and read `NS` and `BH`.
//!
//! [`classify`] joins them into a [`Classification`], whose `Display` is the class as an
//! astronomer writes it: `G2V`, `M5III`, `K1.5IV`, `B8Ia`, `sdM3`, `T6`, `DA4.2`, `NS`. The classes
//! beyond the MK grid (Wolf-Rayet types, luminous blue variables, carbon and S stars, T Tauri and
//! Herbig stars, Be, Ap and Am stars: P06.T24 and P06.T25) arrive through [`ClassExtras`], which
//! is empty in this generator version.

mod luminosity;
pub(crate) mod pm13;
mod scales;
mod sk81;

use core::fmt;

use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::wd_spectral::{WhiteDwarfType, white_dwarf_type};
use crate::stellar::{Composition, Phase, StarState};
use crate::units::Kelvin;

pub use luminosity::LuminosityClass;

/// A spectral class letter: the MK sequence O to M and its extension to the brown dwarfs, L, T
/// and Y, from hottest to coolest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpectralLetter {
    /// O: ionised helium, above about 31,000 K on the dwarf scale.
    O,
    /// B: neutral helium, 10,400–31,400 K.
    B,
    /// A: the strongest hydrogen lines, 7,400–9,700 K.
    A,
    /// F: 6,000–7,200 K.
    F,
    /// G: the Sun's class, 5,400–5,900 K.
    G,
    /// K: 3,900–5,300 K.
    K,
    /// M: titanium oxide bands, 2,350–3,850 K.
    M,
    /// L: metal hydrides and alkali lines, the latest M dwarfs and the warm brown dwarfs.
    L,
    /// T: methane, the cool brown dwarfs.
    T,
    /// Y: ammonia, the coolest brown dwarfs, below about 450 K.
    Y,
}

impl SpectralLetter {
    /// Every letter, hottest first.
    pub const ALL: [Self; 10] = [
        Self::O,
        Self::B,
        Self::A,
        Self::F,
        Self::G,
        Self::K,
        Self::M,
        Self::L,
        Self::T,
        Self::Y,
    ];

    /// The letter's place in the sequence, O = 0 to Y = 9.
    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Self::O => 0,
            Self::B => 1,
            Self::A => 2,
            Self::F => 3,
            Self::G => 4,
            Self::K => 5,
            Self::M => 6,
            Self::L => 7,
            Self::T => 8,
            Self::Y => 9,
        }
    }

    /// The letter at `index` in the sequence, or `None` beyond Y.
    #[must_use]
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::O),
            1 => Some(Self::B),
            2 => Some(Self::A),
            3 => Some(Self::F),
            4 => Some(Self::G),
            5 => Some(Self::K),
            6 => Some(Self::M),
            7 => Some(Self::L),
            8 => Some(Self::T),
            9 => Some(Self::Y),
            _ => None,
        }
    }

    /// The letter as written.
    #[must_use]
    pub const fn as_char(self) -> char {
        match self {
            Self::O => 'O',
            Self::B => 'B',
            Self::A => 'A',
            Self::F => 'F',
            Self::G => 'G',
            Self::K => 'K',
            Self::M => 'M',
            Self::L => 'L',
            Self::T => 'T',
            Self::Y => 'Y',
        }
    }

    /// Whether the letter is of the MK sequence proper, O to M, which carries a luminosity class;
    /// L, T and Y dwarfs are written without one (`L5`, `T6`).
    #[must_use]
    pub const fn is_mk(self) -> bool {
        match self {
            Self::O | Self::B | Self::A | Self::F | Self::G | Self::K | Self::M => true,
            Self::L | Self::T | Self::Y => false,
        }
    }
}

/// A place on the spectral sequence as one continuous number: ten per class from O0 at 0, so
/// G2.3 is 42.3, M9.5 is 69.5, L0 is 70 and Y4 is 94.
///
/// The code is continuous in temperature, and is written to the nearest half subtype (G2.3 as
/// `G2.5`, G2.2 as `G2`), the finest step the catalogues use. It spans the dwarf table, O3 to Y4:
/// hotter and cooler temperatures are held at its ends.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SpectralCode(f64);

impl SpectralCode {
    /// The hottest code the scales reach, O3 (the first row of the dwarf table).
    pub const HOTTEST: Self = Self(3.0);

    /// The coolest code, Y4 (the last row of the dwarf table).
    pub const COOLEST: Self = Self(94.0);

    /// The last code of the MK sequence proper, M9.5: the coolest a giant or supergiant is typed.
    pub const LAST_MK: Self = Self(69.5);

    /// The code `value`, or `None` outside [`HOTTEST`](Self::HOTTEST) to
    /// [`COOLEST`](Self::COOLEST) or if it is NaN.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (Self::HOTTEST.0..=Self::COOLEST.0)
            .contains(&value)
            .then_some(Self(value))
    }

    /// The code clamped to O3–Y4; NaN gives Y4.
    #[must_use]
    fn clamped(value: f64) -> Self {
        if value.is_nan() {
            return Self::COOLEST;
        }
        Self(value.clamp(Self::HOTTEST.0, Self::COOLEST.0))
    }

    /// The code's value, 3–94.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }

    /// The code rounded to the nearest half subtype, as it is written; a half-way value rounds up
    /// the sequence (towards cooler), so G9.75 is K0.
    #[must_use]
    pub fn to_half_subtype(self) -> Self {
        Self::clamped((self.0 * 2.0 + 0.5).floor() / 2.0)
    }

    /// The class letter.
    #[must_use]
    pub fn letter(self) -> SpectralLetter {
        SpectralLetter::from_index(self.letter_index()).unwrap_or(SpectralLetter::Y)
    }

    /// The subtype within the class, 0 to below 10 (up to 4 for Y).
    #[must_use]
    pub fn subtype(self) -> f64 {
        self.0 - 10.0 * f64::from(self.letter_index())
    }

    /// ⌊code ÷ 10⌋, 0–9.
    #[must_use]
    fn letter_index(self) -> u8 {
        let tens = (self.0 / 10.0).floor().clamp(0.0, 9.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number clamped to 0–9"
        )]
        let index = tens as u8;
        index
    }
}

/// Written to the nearest half subtype: `G2`, `M0.5`, `O9.5`, `T4.5`.
impl fmt::Display for SpectralCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let half = self.to_half_subtype();
        let subtype = half.subtype();
        // `subtype` is a multiple of one half below ten, exact in binary.
        let whole = subtype.floor();
        write!(f, "{}", half.letter().as_char())?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number 0–9"
        )]
        let digits = whole as u8;
        if subtype > whole {
            write!(f, "{digits}.5")
        } else {
            write!(f, "{digits}")
        }
    }
}

/// What kind of neutron star an object is, which is how a neutron star is classified.
///
/// Every neutron star is [`NeutronStar`](Self::NeutronStar) in this generator version: telling a
/// radio pulsar and a magnetar from the rest needs the spin-down of P06.T21, which is deferred
/// (ruling 33's vertical slice). T21 decides the other two from its `PulsarState` and passes them
/// in through [`ClassExtras`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NeutronStarClass {
    /// A neutron star seen as neither a pulsar nor a magnetar: `NS`.
    NeutronStar,
    /// A radio pulsar, alive above the death line: `PSR`.
    Pulsar,
    /// A magnetar, powered by its field's decay: `MAG`.
    Magnetar,
}

impl NeutronStarClass {
    /// The class as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeutronStar => "NS",
            Self::Pulsar => "PSR",
            Self::Magnetar => "MAG",
        }
    }
}

/// An object's spectral type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpectralType {
    /// A place on the sequence O to M, or L, T or Y.
    Sequence(SpectralCode),
    /// A white dwarf's type after Sion et al. (1983), such as `DA4.2` (P06.T20.b).
    WhiteDwarf(WhiteDwarfType),
    /// A neutron star, which has no spectral type: `NS`, `PSR` or `MAG`.
    NeutronStar(NeutronStarClass),
    /// A black hole, dark and without a spectrum: `BH`.
    BlackHole,
    /// Nothing: the star was destroyed and left no remnant, written `NONE`.
    NoRemnant,
}

/// A class beyond the MK grid, which P06.T24 derives from the state and track (Wolf-Rayet types,
/// hot subdwarfs, luminous blue variables, carbon and S stars, T Tauri and Herbig Ae/Be stars) and
/// P06.T25 from rotation and magnetism (Be, Ap and Am stars).
///
/// It has no variants in this generator version (ruling 33): T24 and T25 add them, with a version
/// bump, and each must then be written by [`Classification`]'s `Display`, which the exhaustive
/// match there enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PeculiarClass {}

/// What [`classify`] reads beyond the state, the composition and the draws: the peculiar classes
/// of P06.T24 and P06.T25, and the pulsar state of P06.T21 that tells `PSR` and `MAG` from `NS`.
///
/// Empty in this generator version (ruling 33's vertical slice): [`ClassExtras::NONE`] is the only
/// value, and the classification is the MK class alone. The tasks that fill it add fields here,
/// and the caller that builds a star's model (P06.T29) fills them from the track.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ClassExtras {}

impl ClassExtras {
    /// No extras: the classification from the state, composition and draws alone.
    pub const NONE: Self = Self {};
}

/// An object's classification: its spectral type, its luminosity class where it has one, and any
/// peculiar class.
///
/// Its `Display` is the class as written: the subdwarf prefix, then the type, then the
/// luminosity class (`G2V`, `K3III`, `M2Iab`, `B1Ia+`, `sdM3`, `esdK7`); L, T and Y dwarfs,
/// white dwarfs, neutron stars and black holes carry no luminosity class (`T6`, `DA4.2`, `NS`,
/// `BH`).
///
/// # Examples
///
/// The Sun at its present age:
///
/// ```
/// use hyperion_sim::stellar::classify::{ClassExtras, classify};
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::{Composition, Phase, StarState, StarStateParts};
/// use hyperion_sim::units::{SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years};
///
/// let sun = StarState::new(StarStateParts {
///     phase: Phase::MainSequence,
///     age: Years::new(4.57e9),
///     mass: SolarMasses::new(1.0),
///     core_mass: SolarMasses::ZERO,
///     luminosity: SolarLuminosities::new(1.0),
///     radius: SolarRadii::new(1.0),
///     mass_loss_rate: SolarMassesPerYear::ZERO,
///     phase_fraction: 0.45,
/// });
/// let class = classify(&sun, &Composition::SOLAR, &StarDraws::median(), &ClassExtras::NONE);
/// assert_eq!(class.to_string(), "G2V");
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Classification {
    spectral: SpectralType,
    luminosity: Option<LuminosityClass>,
    peculiar: Option<PeculiarClass>,
}

impl Classification {
    /// The spectral type.
    #[must_use]
    pub const fn spectral_type(&self) -> SpectralType {
        self.spectral
    }

    /// The luminosity class: `Some` for every living star of type O to M, `None` for L, T and Y
    /// dwarfs (except a metal-poor main-sequence star, an `sdL` or `esdL`), white dwarfs, neutron
    /// stars, black holes and nothing.
    #[must_use]
    pub const fn luminosity_class(&self) -> Option<LuminosityClass> {
        self.luminosity
    }

    /// The peculiar class of P06.T24 and T25; always `None` in this generator version.
    #[must_use]
    pub const fn peculiar_class(&self) -> Option<PeculiarClass> {
        self.peculiar
    }

    /// A classification of a place on the sequence and a luminosity class, as [`classify`] gives
    /// it: a luminosity class only for O to M, or a subdwarf prefix, which L and T subdwarfs also
    /// carry (`sdL7`, `esdL1`; Burgasser et al. 2007, ApJ 657, 494).
    #[must_use]
    fn sequence(code: SpectralCode, luminosity: Option<LuminosityClass>) -> Self {
        Self {
            spectral: SpectralType::Sequence(code),
            luminosity: luminosity
                .filter(|class| class.is_prefix() || code.to_half_subtype().letter().is_mk()),
            peculiar: None,
        }
    }

    /// A classification with a spectral type and nothing else.
    #[must_use]
    const fn bare(spectral: SpectralType) -> Self {
        Self {
            spectral,
            luminosity: None,
            peculiar: None,
        }
    }
}

impl fmt::Display for Classification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.spectral {
            SpectralType::Sequence(code) => match self.luminosity {
                Some(class) if class.is_prefix() => write!(f, "{}{code}", class.as_str())?,
                Some(class) => write!(f, "{code}{}", class.as_str())?,
                None => write!(f, "{code}")?,
            },
            SpectralType::WhiteDwarf(wd) => write!(f, "{wd}")?,
            SpectralType::NeutronStar(class) => f.write_str(class.as_str())?,
            SpectralType::BlackHole => f.write_str("BH")?,
            SpectralType::NoRemnant => f.write_str("NONE")?,
        }
        if let Some(peculiar) = self.peculiar {
            match peculiar {}
        }
        Ok(())
    }
}

/// The continuous subtype of a class V star at effective temperature `teff`: the dwarf scale of
/// Pecaut and Mamajek (2013) in Mamajek's version 2022.04.16, interpolated linearly in log₁₀
/// T<sub>eff</sub> between its rows.
///
/// It is monotone in temperature, falling from O3 at 44,900 K and above to Y4 at 250 K and below,
/// where it is held. `None` for a temperature that is not finite and positive.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::classify::subtype_from_teff;
/// use hyperion_sim::units::Kelvin;
///
/// // The Sun, 5,772 K, lies just above G2V's 5,770 K: G1.98, written G2.
/// let sun = subtype_from_teff(Kelvin::new(5_772.0)).ok_or("the Sun has a temperature")?;
/// assert!((sun.value() - 41.98).abs() < 0.01);
/// assert_eq!(sun.to_string(), "G2");
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn subtype_from_teff(teff: Kelvin) -> Option<SpectralCode> {
    let t = teff.value();
    (t.is_finite() && t > 0.0).then(|| SpectralCode::clamped(pm13::code_at(t)))
}

/// Classifies an object from its state, its composition and its draws (plan 06, P06.T23.d).
///
/// - A living star is typed on the sequence O to Y from its effective temperature, with a
///   luminosity class from its gravity and luminosity at that temperature (see
///   [`LuminosityClass`]); a class V star is read on the dwarf scale of [`subtype_from_teff`],
///   every other class on its own temperature scale. A main-sequence dwarf with [Fe/H] below −1.0
///   is a subdwarf (`sd`) and below −1.7 an extreme subdwarf (`esd`), after Lépine, Rich and
///   Shara (2007, ApJ 669, 1235). L, T and Y dwarfs have no luminosity class.
/// - A white dwarf is typed by [`white_dwarf_type`] from its temperature and its three
///   `star.wd.*` marks (P06.T20.b).
/// - A neutron star is `NS`, a black hole `BH` and a star that left nothing `NONE`.
///
/// `extras` carries the classes beyond the MK grid (P06.T24, T25) and the pulsar state (P06.T21);
/// it is empty in this generator version, and the classification is then the MK class alone.
///
/// Total over every state [`StarState::new`] accepts: it never panics, and nothing here takes the
/// logarithm of a luminosity that may be zero (ruling 40: a black hole's is).
#[must_use]
pub fn classify(
    state: &StarState,
    composition: &Composition,
    draws: &StarDraws,
    extras: &ClassExtras,
) -> Classification {
    // The seam of P06.T21, T24 and T25: nothing to read yet.
    let ClassExtras {} = *extras;
    match state.phase() {
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
            Classification::bare(SpectralType::WhiteDwarf(white_dwarf_type(
                state.effective_temperature(),
                draws,
            )))
        }
        Phase::NeutronStar => {
            Classification::bare(SpectralType::NeutronStar(NeutronStarClass::NeutronStar))
        }
        Phase::BlackHole => Classification::bare(SpectralType::BlackHole),
        Phase::NoRemnant => Classification::bare(SpectralType::NoRemnant),
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
        | Phase::Substellar => {
            let (code, class) = luminosity::classify_living(state, composition);
            Classification::sequence(code, Some(class))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use hyperion_testkit::float::bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::math;
    use crate::rng::Mark;
    use crate::stellar::StarStateParts;
    use crate::stellar::draws::StarDrawsParts;
    use crate::stellar::remnant::wd_spectral::{WhiteDwarfAtmosphere, WhiteDwarfClass};
    use crate::units::consts::{GM_SUN, SOLAR_RADIUS_M};
    use crate::units::{
        Dex, HeliumExcess, SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years,
    };

    /// A state of `phase` with mass `m` (M☉), luminosity `l` (L☉) and radius `r` (R☉).
    pub(crate) fn state(phase: Phase, m: f64, l: f64, r: f64) -> StarState {
        let mass = SolarMasses::new(m);
        StarState::new(StarStateParts {
            phase,
            age: Years::new(1e9),
            mass,
            core_mass: if phase.is_remnant() {
                mass
            } else {
                SolarMasses::new(0.1 * m)
            },
            luminosity: SolarLuminosities::new(l),
            radius: SolarRadii::new(r),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    /// A living star of `phase` and mass `m` (M☉) at T<sub>eff</sub> `t` (K) and luminosity `l`
    /// (L☉): the radius from Stefan–Boltzmann.
    pub(crate) fn star(phase: Phase, m: f64, l: f64, t: f64) -> StarState {
        let r = l.sqrt() * (5_772.0 / t) * (5_772.0 / t);
        state(phase, m, l, r)
    }

    /// A living star of `phase` and mass `m` (M☉) at T<sub>eff</sub> `t` (K) and surface gravity
    /// `log_g` (cgs): the radius from g = GM ÷ R², the luminosity from Stefan–Boltzmann.
    pub(crate) fn star_g(phase: Phase, m: f64, log_g: f64, t: f64) -> StarState {
        let g_si = math::exp10(log_g) / 100.0;
        let r = (GM_SUN * m / g_si).sqrt() / SOLAR_RADIUS_M;
        let l = r * r * math::powi(t / 5_772.0, 4);
        state(phase, m, l, r)
    }

    fn class_at(state: &StarState, fe_h: f64) -> Classification {
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        classify(
            state,
            &composition,
            &StarDraws::median(),
            &ClassExtras::NONE,
        )
    }

    fn written(state: &StarState) -> String {
        class_at(state, 0.0).to_string()
    }

    /// The plan's test: the Sun is G2V (its M<sub>V</sub> is `photometry`'s test).
    #[test]
    fn the_sun_is_g2v() {
        let sun = state(Phase::MainSequence, 1.0, 1.0, 1.0);
        let class = class_at(&sun, 0.0);
        assert_eq!(class.to_string(), "G2V");
        assert_eq!(class.luminosity_class(), Some(LuminosityClass::Dwarf));
        let SpectralType::Sequence(code) = class.spectral_type() else {
            panic!("{class:?}");
        };
        assert!((code.value() - 41.98).abs() < 0.01, "{code:?}");
    }

    /// The plan's tests: Vega-like (9,600 K, log g 4.0) is A0V; a 4,300 K star of log g 1.7 is a
    /// K giant; 3,600 K at 10⁵ L☉ is an M supergiant, Iab or Ia.
    #[test]
    fn the_plans_named_stars_take_their_classes() {
        assert_eq!(
            written(&star_g(Phase::MainSequence, 2.1, 4.0, 9_600.0)),
            "A0V"
        );
        let arcturus = class_at(&star_g(Phase::FirstGiantBranch, 1.1, 1.7, 4_300.0), 0.0);
        assert_eq!(arcturus.luminosity_class(), Some(LuminosityClass::Giant));
        assert!(arcturus.to_string().starts_with('K'), "{arcturus}");
        for mass in [12.0, 15.0, 20.0, 25.0] {
            let rsg = class_at(&star(Phase::CoreHeliumBurning, mass, 1e5, 3_600.0), 0.0);
            assert!(
                matches!(
                    rsg.luminosity_class(),
                    Some(LuminosityClass::Supergiant | LuminosityClass::LuminousSupergiant)
                ),
                "{mass} M☉: {rsg}"
            );
            assert!(rsg.to_string().starts_with('M'), "{rsg}");
        }
    }

    /// Well-measured stars, from their published T<sub>eff</sub>, luminosity and mass: the classes
    /// they take are pinned here, each within a subtype and a half and one luminosity class of
    /// the catalogue's in its label, the spread of the calibrations.
    #[test]
    fn familiar_stars_take_classes_near_their_catalogue_ones() {
        let cases = [
            // (name, phase, M☉, L☉, T_eff, expected)
            // Sirius is A0mA1Va: its metal lines are an A1 star's, its temperature an A0V's.
            (
                "Sirius A0mA1Va",
                Phase::MainSequence,
                2.06,
                25.4,
                9_940.0,
                "A0V",
            ),
            (
                "Procyon F5IV-V",
                Phase::MainSequence,
                1.5,
                6.9,
                6_530.0,
                "F5IV",
            ),
            ("ε Eri K2V", Phase::MainSequence, 0.82, 0.34, 5_084.0, "K2V"),
            (
                "Pollux K0III",
                Phase::CoreHeliumBurning,
                1.9,
                43.0,
                4_666.0,
                "K0.5III",
            ),
            (
                "Aldebaran K5III",
                Phase::FirstGiantBranch,
                1.16,
                440.0,
                3_900.0,
                "K5III",
            ),
            (
                "Betelgeuse M1-2Ia-ab",
                Phase::CoreHeliumBurning,
                18.0,
                1.26e5,
                3_600.0,
                "M3Ia",
            ),
            (
                "Antares M1.5Iab",
                Phase::CoreHeliumBurning,
                13.0,
                7.5e4,
                3_660.0,
                "M2Iab",
            ),
            (
                "Rigel B8Ia",
                Phase::CoreHeliumBurning,
                21.0,
                1.2e5,
                12_100.0,
                "B8Ia",
            ),
            (
                "Deneb A2Ia",
                Phase::CoreHeliumBurning,
                19.0,
                1.96e5,
                8_525.0,
                "A3Ia",
            ),
            (
                "Spica B1III-IV",
                Phase::MainSequence,
                11.4,
                20_500.0,
                25_300.0,
                "B1III",
            ),
            (
                "Polaris F7Ib",
                Phase::CoreHeliumBurning,
                5.4,
                2_500.0,
                6_015.0,
                "F8.5Ib",
            ),
        ];
        let wrong: Vec<String> = cases
            .into_iter()
            .filter_map(|(name, phase, m, l, t, expected)| {
                let got = written(&star(phase, m, l, t));
                (got != expected).then(|| format!("{name}: {got}, not {expected}"))
            })
            .collect();
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// The code is written to the nearest half subtype, and carries into the next class.
    #[test]
    fn codes_are_written_to_the_nearest_half_subtype() {
        let cases = [
            (42.3, "G2.5"),
            (42.2, "G2"),
            (42.25, "G2.5"),
            (49.8, "K0"),
            (69.6, "M9.5"),
            (69.8, "L0"),
            (3.0, "O3"),
            (9.7, "O9.5"),
            (9.8, "B0"),
            (84.5, "T4.5"),
            (94.0, "Y4"),
        ];
        for (value, expected) in cases {
            assert_eq!(SpectralCode(value).to_string(), expected, "{value}");
        }
        assert_eq!(SpectralCode::new(2.9), None);
        assert_eq!(SpectralCode::new(94.1), None);
        assert_eq!(SpectralCode::new(f64::NAN), None);
        let code = SpectralCode::new(57.25).unwrap();
        assert_eq!(code.letter(), SpectralLetter::K);
        assert!((code.subtype() - 7.25).abs() < 1e-12);
        for (i, letter) in SpectralLetter::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(letter.index()), i);
            assert_eq!(SpectralLetter::from_index(letter.index()), Some(letter));
        }
        assert_eq!(SpectralLetter::from_index(10), None);
    }

    /// The dwarf scale's subtype is monotone in temperature from 60,000 K to 100 K, and hits
    /// every row of the table exactly.
    #[test]
    fn the_dwarf_subtype_is_monotone_in_teff() {
        let mut previous = SpectralCode::HOTTEST.value();
        let mut t = 60_000.0_f64;
        while t > 100.0 {
            let code = subtype_from_teff(Kelvin::new(t)).unwrap().value();
            assert!(code >= previous, "{t} K: {code} after {previous}");
            previous = code;
            t *= 0.9995;
        }
        for row in pm13::DWARF_SEQUENCE {
            let code = subtype_from_teff(Kelvin::new(row.teff_k)).unwrap();
            assert_eq!(bits(code.value()), bits(row.code()), "{row:?}");
        }
        assert_eq!(subtype_from_teff(Kelvin::ZERO), None);
        assert_eq!(subtype_from_teff(Kelvin::new(f64::INFINITY)), None);
    }

    /// Neutron stars, black holes and nothing have no spectral type; a black hole's zero
    /// luminosity is never logged.
    #[test]
    fn remnants_without_spectra_read_ns_bh_and_none() {
        let ns = state(Phase::NeutronStar, 1.4, 1e-3, 1.75e-5);
        let bh = state(Phase::BlackHole, 10.0, 0.0, 4.2e-5);
        let none = StarState::new(StarStateParts {
            phase: Phase::NoRemnant,
            age: Years::new(1e9),
            mass: SolarMasses::ZERO,
            core_mass: SolarMasses::ZERO,
            luminosity: SolarLuminosities::ZERO,
            radius: SolarRadii::ZERO,
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.0,
        });
        assert_eq!(written(&ns), "NS");
        assert_eq!(written(&bh), "BH");
        assert_eq!(written(&none), "NONE");
        for s in [ns, bh, none] {
            assert_eq!(class_at(&s, 0.0).luminosity_class(), None);
        }
        assert_eq!(NeutronStarClass::Pulsar.as_str(), "PSR");
        assert_eq!(NeutronStarClass::Magnetar.as_str(), "MAG");
    }

    /// White dwarfs take their Sion type from their temperature and marks.
    #[test]
    fn white_dwarfs_take_their_sion_types() {
        // 0.6 M☉ of 0.0126 R☉ at 3 × 10⁻³ L☉: 12,030 K.
        let wd = state(Phase::CarbonOxygenWhiteDwarf, 0.6, 3e-3, 0.0126);
        let class = class_at(&wd, 0.0);
        let SpectralType::WhiteDwarf(wd_type) = class.spectral_type() else {
            panic!("{class:?}");
        };
        assert_eq!(wd_type.class(), WhiteDwarfClass::Da);
        assert_eq!(class.to_string(), "DA4.2");
        assert_eq!(class.luminosity_class(), None);
    }

    /// L, T and Y dwarfs are written without a luminosity class; the latest M dwarfs with one.
    #[test]
    fn brown_dwarfs_carry_no_luminosity_class() {
        let t6 = class_at(&star(Phase::Substellar, 0.04, 1e-5, 950.0), 0.0);
        assert_eq!(t6.to_string(), "T6");
        assert_eq!(t6.luminosity_class(), None);
        let l3 = class_at(&star(Phase::Substellar, 0.07, 2e-4, 1_920.0), 0.0);
        assert_eq!(l3.to_string(), "L3");
        let m9 = class_at(&star(Phase::MainSequence, 0.09, 5e-4, 2_380.0), 0.0);
        assert_eq!(m9.to_string(), "M9V");
        // A metal-poor star at L temperatures keeps its subdwarf prefix, as L subdwarfs do.
        let sd_l = class_at(&star(Phase::MainSequence, 0.085, 1.5e-4, 2_000.0), -1.5);
        assert_eq!(sd_l.to_string(), "sdL2.5");
        assert_eq!(sd_l.luminosity_class(), Some(LuminosityClass::Subdwarf));
    }

    /// A metal-poor main-sequence dwarf is a subdwarf, sd below −1.0 and esd below −1.7; a giant or
    /// a subgiant is not.
    #[test]
    fn metal_poor_dwarfs_are_subdwarfs() {
        let dwarf = star(Phase::MainSequence, 0.3, 0.012, 3_430.0);
        assert_eq!(class_at(&dwarf, -0.5).to_string(), "M3V");
        assert_eq!(class_at(&dwarf, -1.2).to_string(), "sdM3");
        assert_eq!(class_at(&dwarf, -2.0).to_string(), "esdM3");
        let giant = star_g(Phase::FirstGiantBranch, 0.8, 2.0, 4_500.0);
        assert!(class_at(&giant, -2.0).to_string().ends_with("III"));
        let turn_off = star_g(Phase::MainSequence, 0.85, 3.9, 6_300.0);
        assert!(class_at(&turn_off, -2.0).to_string().ends_with("IV"));
    }

    /// The same inputs give the same classification, bit for bit.
    #[test]
    fn classification_is_a_pure_function() {
        let crossing = star(Phase::HertzsprungGap, 3.0, 150.0, 8_000.0);
        let first = class_at(&crossing, 0.1);
        let second = class_at(&crossing, 0.1);
        let (SpectralType::Sequence(one), SpectralType::Sequence(other)) =
            (first.spectral_type(), second.spectral_type())
        else {
            panic!("{first:?}");
        };
        assert_eq!(bits(one.value()), bits(other.value()));
        assert_eq!(first.luminosity_class(), second.luminosity_class());
    }

    /// The test parser: a classification from its written form, with the subtype at the half
    /// step it is written with. `None` where the string is not one `Display` writes.
    pub(crate) fn parse(text: &str) -> Option<Classification> {
        match text {
            "NS" => {
                return Some(Classification::bare(SpectralType::NeutronStar(
                    NeutronStarClass::NeutronStar,
                )));
            }
            "PSR" => {
                return Some(Classification::bare(SpectralType::NeutronStar(
                    NeutronStarClass::Pulsar,
                )));
            }
            "MAG" => {
                return Some(Classification::bare(SpectralType::NeutronStar(
                    NeutronStarClass::Magnetar,
                )));
            }
            "BH" => return Some(Classification::bare(SpectralType::BlackHole)),
            "NONE" => return Some(Classification::bare(SpectralType::NoRemnant)),
            _ => {}
        }
        if let Some(rest) = text.strip_prefix('D') {
            return parse_white_dwarf(rest);
        }
        let (prefix, rest) = if let Some(rest) = text.strip_prefix("esd") {
            (Some(LuminosityClass::ExtremeSubdwarf), rest)
        } else if let Some(rest) = text.strip_prefix("sd") {
            (Some(LuminosityClass::Subdwarf), rest)
        } else {
            (None, text)
        };
        let first = rest.chars().next()?;
        let letter = SpectralLetter::ALL
            .into_iter()
            .find(|l| l.as_char() == first)?;
        let rest = &rest[1..];
        let digits = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        let (number, suffix) = rest.split_at(digits);
        let subtype: f64 = number.parse().ok()?;
        let class = match (prefix, suffix) {
            (Some(prefix), "") => Some(prefix),
            (Some(_), _) => return None,
            (None, "") => None,
            (None, suffix) => Some(
                LuminosityClass::ALL
                    .into_iter()
                    .find(|c| !c.is_prefix() && c.as_str() == suffix)?,
            ),
        };
        let code = SpectralCode::new(10.0 * f64::from(letter.index()) + subtype)?;
        Some(Classification::sequence(code, class))
    }

    fn parse_white_dwarf(rest: &str) -> Option<Classification> {
        let mut chars = rest.chars();
        let (class, atmosphere) = match chars.next()? {
            'A' => (WhiteDwarfClass::Da, WhiteDwarfAtmosphere::Hydrogen),
            'B' => (WhiteDwarfClass::Db, WhiteDwarfAtmosphere::Helium),
            'O' => (WhiteDwarfClass::Do, WhiteDwarfAtmosphere::Helium),
            'C' => (WhiteDwarfClass::Dc, WhiteDwarfAtmosphere::Helium),
            'Q' => (WhiteDwarfClass::Dq, WhiteDwarfAtmosphere::Helium),
            'Z' => (WhiteDwarfClass::Dz, WhiteDwarfAtmosphere::Helium),
            _ => return None,
        };
        let rest = &rest[1..];
        let (metals, index) = match rest.strip_prefix('Z') {
            Some(index) => (true, index),
            None => (class == WhiteDwarfClass::Dz, rest),
        };
        let index = if index.is_empty() {
            None
        } else {
            Some(index.parse::<f64>().ok()?)
        };
        Some(Classification::bare(SpectralType::WhiteDwarf(
            WhiteDwarfType::from_parts(class, atmosphere, metals, index),
        )))
    }

    /// The plan's test: every state of a 10⁵-star sample classifies without panic and
    /// round-trips through `Display` and the test parser.
    ///
    /// The track integrator (P06.T10.c–e) is another lane's this round, so the sample is of
    /// states built directly, over every phase: masses, luminosities and radii log-uniform over
    /// wide ranges for each (hot helium stars and central stars, white dwarfs from 10⁻⁵ to 10³
    /// L☉, brown dwarfs to 10⁻⁷ L☉, neutron stars and black holes without light), \[Fe/H\] from
    /// −3 to +0.5, and random white-dwarf marks.
    #[test]
    fn every_state_of_a_sample_classifies_and_round_trips() {
        let mut lcg = Lcg::new(0x5eed_c1a5_5000_0001);
        let mut written = std::collections::BTreeMap::<String, u32>::new();
        for i in 0..100_000 {
            let phase = Phase::ALL[i % Phase::ALL.len()];
            // log₁₀ ranges of mass (M☉), luminosity (L☉) and radius (R☉) per phase.
            let ranges = match phase {
                Phase::Protostar | Phase::PreMainSequence => {
                    [(-1.1, 1.0), (-3.0, 4.0), (-0.3, 1.5)]
                }
                Phase::MainSequence => [(-1.1, 2.2), (-4.0, 6.8), (-1.0, 1.4)],
                Phase::HertzsprungGap
                | Phase::FirstGiantBranch
                | Phase::CoreHeliumBurning
                | Phase::EarlyAgb
                | Phase::ThermallyPulsingAgb => [(-0.4, 2.0), (0.0, 6.7), (0.0, 3.4)],
                Phase::HeliumMainSequence
                | Phase::HeliumHertzsprungGap
                | Phase::HeliumGiantBranch => [(-0.5, 1.7), (0.0, 6.0), (-1.5, 1.0)],
                Phase::PostAgb => [(-0.3, 0.1), (3.0, 4.2), (-2.0, 2.0)],
                Phase::HeliumWhiteDwarf
                | Phase::CarbonOxygenWhiteDwarf
                | Phase::OxygenNeonWhiteDwarf => [(-0.7, 0.15), (-5.5, 3.0), (-2.3, -1.5)],
                Phase::NeutronStar => [(0.05, 0.35), (-99.0, -99.0), (-4.76, -4.76)],
                Phase::BlackHole => [(0.5, 2.0), (-99.0, -99.0), (-4.0, -4.0)],
                Phase::NoRemnant => [(-99.0, -99.0); 3],
                Phase::Substellar => [(-2.0, -1.0), (-7.0, -2.0), (-1.1, -0.5)],
            };
            let [mass, luminosity, radius] = ranges.map(|(lo, hi): (f64, f64)| {
                // A range at −99 is a zero: no light, or nothing at all.
                if lo < -90.0 {
                    0.0
                } else {
                    math::exp10(lo + (hi - lo) * lcg.next_f64())
                }
            });
            let sampled = state(phase, mass, luminosity, radius);
            let draws = StarDraws::from_parts(StarDrawsParts {
                wd_atmosphere: Mark::from_word(lcg.next_u64()),
                wd_carbon: Mark::from_word(lcg.next_u64()),
                wd_metals: Mark::from_word(lcg.next_u64()),
                ..StarDrawsParts::MEDIAN
            });
            let composition =
                Composition::from_fe_h(Dex::new(-3.0 + 3.5 * lcg.next_f64()), HeliumExcess::ZERO);
            let class = classify(&sampled, &composition, &draws, &ClassExtras::NONE);
            let text = class.to_string();
            let parsed =
                parse(&text).unwrap_or_else(|| panic!("{text} does not parse: {sampled:?}"));
            assert_eq!(parsed.to_string(), text, "{sampled:?}");
            assert_eq!(
                parsed.luminosity_class(),
                class.luminosity_class(),
                "{text}"
            );
            *written.entry(text).or_default() += 1;
        }
        // The sample reaches every kind of string the classifier writes.
        for expected in ["V", "IV", "III", "II", "Ib", "Iab", "Ia", "Ia+"] {
            assert!(
                written.keys().any(|k| k.ends_with(expected)),
                "no class {expected}"
            );
        }
        for expected in [
            "sd", "esd", "DA", "DB", "DO", "DC", "DQ", "DZ", "L", "T", "Y", "NS", "BH", "NONE",
        ] {
            assert!(
                written.keys().any(|k| k.starts_with(expected)),
                "no type {expected}"
            );
        }
        assert!(written.keys().any(|k| k.contains("AZ")), "no DAZ");
    }
}
