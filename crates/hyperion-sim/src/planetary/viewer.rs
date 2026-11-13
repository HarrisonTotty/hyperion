//! The planet viewer's planets (rendering plan R15): one planet from the generator's own inputs,
//! a generated planet's inputs read back, and `RANDOMIZE`'s draw of a real planet's.
//!
//! # A planet from its inputs
//!
//! [`PlanetInputs`] are what the generator itself consumes of a planet and its host, each checked
//! against its range ([`InputField::range`]): the seed of every stream, the host's initial mass,
//! \[Fe/H\] and age, the planet's primordial mass and orbit, and where it formed; and, each in place
//! of its draw only where given, its primordial day and obliquity and its water, carbon and nitrogen
//! ranks. [`ViewerSystem::generate`] builds the host as a synthetic single star
//! ([`SystemContext::builder`], with plan 06's own draws of it under the input seed) and its
//! system with the planet given (`system::generate_given`): every other property of the planet, its
//! moons, rings and belts, its atmosphere, surface and climate, is derived exactly as the galaxy's
//! generator derives a placed planet's. Nothing seen is an input.
//!
//! # The ID
//!
//! The system's ID is a [`PinnedId`] whose number has bit 57 set, the planet viewer's half of the
//! pinned space, above a 57-bit hash of the inputs' canonical text ([`PlanetInputs::system_id`]).
//! Every stream is keyed on it, so the same inputs give the same planet, bit for bit, in any
//! universe of a generator version, and every input changes every draw that is not an input, as
//! each system of the galaxy has draws of its own. The seed's input stands in for the universe's
//! seed in every stream; the universe gives nothing else.
//!
//! # `RANDOMIZE`
//!
//! [`draw_inputs`] reads a real planet of a galaxy: from its draw seed, a point drawn from the
//! galaxy's own number density of systems over the whole galaxy ([`DensityGrid`]), so that a host
//! is a disc, thick-disc, bulge, bar or halo star as often as the galaxy holds one, and the system
//! nearest it; a single star with a planet present at the epoch, one of whose present planets is
//! picked uniformly, and whose host, orbit and mass, as formation gave them, are the inputs. Its
//! day, obliquity and volatile ranks are left to be drawn, and its seed is the draw's.

use std::error::Error;
use std::fmt::{self, Write as _};
use std::ops::RangeInclusive;

use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::placement::{
    LayerSpec, STELLAR_LAYERS, SystemKind, SystemRecord, generate_cell,
};
use crate::galaxy::query::{QuerySphere, cells_in_sphere, count_cells_in_sphere};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{BodyId, PinnedId, SystemId};
use crate::orbit::Eccentricity;
use crate::planetary::context::{BuildSystemContextError, SyntheticDraws, UNIVERSE_AGE};
use crate::planetary::fate::BodyState;
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::planetary::placement::{GivenOrbit, ZoneDiscInputs};
use crate::planetary::system::{
    GivenPlanet, GivenSpin, GivenVolatiles, PlanetParts, generate, generate_given,
};
use crate::planetary::{HostKind, PlanetarySystem, SystemContext};
use crate::rng::{ObjectKey, Stream, tags};
use crate::sky::disc::{HostDisc, host_discs_of};
use crate::stellar::StarState;
use crate::stellar::draws::UnitUniform;
use crate::stellar::system::{SystemSummary, synthetic_summary_at};
use crate::time::UniverseTime;
use crate::units::{
    AstronomicalUnits, Degrees, Dex, EarthMasses, LightYears, Metres, Radians, Seconds,
    SolarMasses, Years,
};

/// Where every viewer system stands, light-years along the galactic axes: the Sun-like point,
/// 26,000 ly along +y in the plane, where the statistical tests sample real systems
/// (`planetary::testing::SAMPLE_CENTRE_LY`). Its sky is the galaxy's from there.
pub const VIEWER_PLACE_LY: [f64; 3] = [0.0, 26_000.0, 0.0];

/// The planet's slot in its system: the first planet's, body index `0x0100`.
const VIEWER_PLANET_SLOT: u8 = 1;

/// The bit of a pinned number that marks the planet viewer's half of the pinned space (lean,
/// 2026-11-05): bit 57, the number's highest.
const VIEWER_PIN: u64 = 1 << 57;

/// Seconds in an hour.
const SECONDS_PER_HOUR: f64 = 3_600.0;

/// Julian years in a gigayear.
const YEARS_PER_GYR: f64 = 1.0e9;

/// The most points `RANDOMIZE` draws before it gives up: most points find a single star with a
/// present planet within a few tries, so 64 misses in a row are a galaxy with almost none.
pub const DRAW_TRIES: u64 = 64;

/// The first radius `RANDOMIZE` searches about its point for the nearest system, light-years; it
/// doubles until a sphere holds one.
pub const FIRST_SEARCH_LY: f64 = 8.0;

/// The most generation cells one search sphere may cover, summed over the stellar layers: 2¹⁸, a
/// sphere of about 315 ly in layer A, as the statistical tests' samples are bounded
/// (`planetary::testing::MAX_SAMPLE_CELLS`). A point of the far halo with no system that near is a
/// try that finds none.
pub const MAX_SEARCH_CELLS: u64 = 1 << 18;

/// The edges of the density grid's galactocentric radii, light-years: 250 ly apart to 10,000 ly,
/// where the bar, the bulge and the disc's inner rise change fastest, then 1,000 ly apart to
/// 60,000 ly, beyond which the galaxy holds almost no system and the root cube ends.
#[must_use]
pub fn radius_edges() -> Vec<f64> {
    (0_u16..=40)
        .map(|k| 250.0 * f64::from(k))
        .chain((11_u16..=60).map(|k| 1_000.0 * f64::from(k)))
        .collect()
}

/// The density grid's azimuths: 24 of 15°, enough to resolve the bar and the arms' pitch.
pub const AZIMUTH_BINS: u16 = 24;

/// The edges of the density grid's heights above or below the plane, light-years: fine within
/// the thin disc's scale height (some 1,000 ly, Jurić et al. 2008), coarser through the thick disc
/// and out through the halo to 50,000 ly.
pub const HEIGHT_EDGES_LY: [f64; 18] = [
    0.0, 50.0, 100.0, 200.0, 300.0, 500.0, 750.0, 1_000.0, 1_500.0, 2_000.0, 3_000.0, 5_000.0,
    7_500.0, 10_000.0, 15_000.0, 20_000.0, 30_000.0, 50_000.0,
];

/// One input of [`PlanetInputs`], as the wire and the panel name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputField {
    /// The seed of every stream.
    Seed,
    /// The host's initial mass, M☉.
    StarMass,
    /// The host's \[Fe/H\], dex.
    FeH,
    /// The system's age at the epoch, Gyr.
    Age,
    /// The planet's primordial mass, M⊕.
    Mass,
    /// Its primordial semi-major axis, au.
    SemiMajorAxis,
    /// Its primordial eccentricity.
    Eccentricity,
    /// How far from its host it formed, au.
    FormedAt,
    /// Its primordial day, hours.
    Day,
    /// Its primordial obliquity, degrees.
    Tilt,
    /// Its water rank.
    Water,
    /// Its carbon rank.
    Carbon,
    /// Its nitrogen rank.
    Nitrogen,
}

impl InputField {
    /// The field's name on the wire.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::StarMass => "star_mass_msun",
            Self::FeH => "fe_h_dex",
            Self::Age => "age_gyr",
            Self::Mass => "mass_mearth",
            Self::SemiMajorAxis => "semi_major_axis_au",
            Self::Eccentricity => "eccentricity",
            Self::FormedAt => "formed_at_au",
            Self::Day => "day_hours",
            Self::Tilt => "tilt_deg",
            Self::Water => "water_rank",
            Self::Carbon => "carbon_rank",
            Self::Nitrogen => "nitrogen_rank",
        }
    }

    /// The values the field takes, in its unit; every `u64` for the seed.
    ///
    /// - The host's mass is the stellar stage's, 0.08–150 M☉ ([`MAX_STAR_MASS`](crate::stellar::system::MAX_STAR_MASS)).
    /// - \[Fe/H\] is −3 to +1 dex, wider than the galaxy's draws; the evolution formulae clamp its Z
    ///   to Hurley, Pols and Tout's (2000) fitted 10⁻⁴–0.03.
    /// - The age is 1 Myr to the universe's age ([`UNIVERSE_AGE`], 13.787 Gyr).
    /// - The mass is 10⁻⁴ M⊕, below Ceres's 1.6 × 10⁻⁴, to 4,000 M⊕, below the 13 Jupiter masses
    ///   (4,132 M⊕) above which the derivation makes no planet.
    /// - The semi-major axis and the formation distance are 10⁻³ to 10⁵ au; the system's reach and
    ///   its star's surface bound the orbit further ([`GenerateViewerError`]).
    /// - The eccentricity is 0 to 0.9, the day 0.1 h to 10⁶ h (the rotation law floors it at the
    ///   body's break-up period), the obliquity 0–180°, and each rank 0.001–0.999.
    #[must_use]
    pub const fn range(self) -> RangeInclusive<f64> {
        match self {
            // A seed's every value is a `u64`, which `f64` holds only up to 2⁵³; it is never
            // checked against this.
            Self::Seed => 0.0..=f64::MAX,
            Self::StarMass => 0.08..=150.0,
            Self::FeH => -3.0..=1.0,
            Self::Age => 0.001..=13.787,
            Self::Mass => 1.0e-4..=4_000.0,
            Self::SemiMajorAxis | Self::FormedAt => 1.0e-3..=1.0e5,
            Self::Eccentricity => 0.0..=0.9,
            Self::Day => 0.1..=1.0e6,
            Self::Tilt => 0.0..=180.0,
            Self::Water | Self::Carbon | Self::Nitrogen => 0.001..=0.999,
        }
    }
}

impl fmt::Display for InputField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A planet's inputs as given, each a plain value in the unit its name states, not yet checked:
/// what the wire and the planet viewer's string carry. An input that is `None` is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPlanetInputs {
    /// The seed of every stream, in place of the universe's.
    pub seed: u64,
    /// The host's initial mass, M☉.
    pub star_mass_msun: f64,
    /// The host's \[Fe/H\], dex.
    pub fe_h_dex: f64,
    /// The system's age at the epoch, Gyr.
    pub age_gyr: f64,
    /// The planet's primordial mass, M⊕.
    pub mass_mearth: f64,
    /// Its primordial semi-major axis about its host, au.
    pub semi_major_axis_au: f64,
    /// Its primordial eccentricity.
    pub eccentricity: f64,
    /// How far from its host it formed, au; `None` for placement's rule (its semi-major axis, or a
    /// giant's the site where giants' cores grow if that lies further out).
    pub formed_at_au: Option<f64>,
    /// Its primordial day, hours; `None` for its draw.
    pub day_hours: Option<f64>,
    /// Its primordial obliquity, degrees; `None` for its draw.
    pub tilt_deg: Option<f64>,
    /// Its water rank; `None` for its draw.
    pub water_rank: Option<f64>,
    /// Its carbon rank; `None` for its draw.
    pub carbon_rank: Option<f64>,
    /// Its nitrogen rank; `None` for its draw.
    pub nitrogen_rank: Option<f64>,
}

/// An input outside its field's range ([`InputField::range`]), NaN and the infinities included.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidInputError {
    field: InputField,
    value: f64,
}

impl InvalidInputError {
    /// The input refused.
    #[must_use]
    pub const fn field(&self) -> InputField {
        self.field
    }

    /// Its value as given.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }
}

impl fmt::Display for InvalidInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let range = self.field.range();
        write!(
            f,
            "{} {} is not within {} to {}",
            self.field,
            self.value,
            range.start(),
            range.end()
        )
    }
}

impl Error for InvalidInputError {}

/// A planet's inputs, each within its range ([`InputField::range`]), with no negative zero.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::viewer::{PlanetInputs, RawPlanetInputs};
///
/// let earth = RawPlanetInputs {
///     seed: 7,
///     star_mass_msun: 1.0,
///     fe_h_dex: 0.0,
///     age_gyr: 4.57,
///     mass_mearth: 1.0,
///     semi_major_axis_au: 1.0,
///     eccentricity: 0.0167,
///     formed_at_au: None,
///     day_hours: None,
///     tilt_deg: None,
///     water_rank: None,
///     carbon_rank: None,
///     nitrogen_rank: None,
/// };
/// let a = PlanetInputs::try_from(earth)?;
/// // The same inputs name the same system; another mass names another.
/// assert_eq!(a.system_id(), PlanetInputs::try_from(earth)?.system_id());
/// let heavier = PlanetInputs::try_from(RawPlanetInputs { mass_mearth: 1.1, ..earth })?;
/// assert_ne!(a.system_id(), heavier.system_id());
/// assert!(PlanetInputs::try_from(RawPlanetInputs { eccentricity: 0.95, ..earth }).is_err());
/// # Ok::<(), hyperion_sim::planetary::viewer::InvalidInputError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanetInputs {
    raw: RawPlanetInputs,
}

impl TryFrom<RawPlanetInputs> for PlanetInputs {
    type Error = InvalidInputError;

    /// The inputs of `raw`, checked field by field in [`InputField`]'s order.
    ///
    /// # Errors
    ///
    /// [`InvalidInputError`] for the first input outside its field's range.
    fn try_from(raw: RawPlanetInputs) -> Result<Self, Self::Error> {
        let given = |field, value: Option<f64>| value.map(|v| checked(field, v)).transpose();
        Ok(Self {
            raw: RawPlanetInputs {
                seed: raw.seed,
                star_mass_msun: checked(InputField::StarMass, raw.star_mass_msun)?,
                fe_h_dex: checked(InputField::FeH, raw.fe_h_dex)?,
                age_gyr: checked(InputField::Age, raw.age_gyr)?,
                mass_mearth: checked(InputField::Mass, raw.mass_mearth)?,
                semi_major_axis_au: checked(InputField::SemiMajorAxis, raw.semi_major_axis_au)?,
                eccentricity: checked(InputField::Eccentricity, raw.eccentricity)?,
                formed_at_au: given(InputField::FormedAt, raw.formed_at_au)?,
                day_hours: given(InputField::Day, raw.day_hours)?,
                tilt_deg: given(InputField::Tilt, raw.tilt_deg)?,
                water_rank: given(InputField::Water, raw.water_rank)?,
                carbon_rank: given(InputField::Carbon, raw.carbon_rank)?,
                nitrogen_rank: given(InputField::Nitrogen, raw.nitrogen_rank)?,
            },
        })
    }
}

/// `value` where `field`'s range holds it, a negative zero made positive so that one value has one
/// bit pattern.
fn checked(field: InputField, value: f64) -> Result<f64, InvalidInputError> {
    if field.range().contains(&value) {
        Ok(value + 0.0)
    } else {
        Err(InvalidInputError { field, value })
    }
}

impl PlanetInputs {
    /// The inputs as plain values.
    #[must_use]
    pub const fn raw(&self) -> &RawPlanetInputs {
        &self.raw
    }

    /// The seed of every stream.
    #[must_use]
    pub const fn seed(&self) -> Seed {
        Seed::new(self.raw.seed)
    }

    /// The inputs' canonical text, the one the planet viewer's string carries: `PV1;`, then each
    /// input given as `name=value` in [`InputField`]'s order, `;`-separated, the seed as 16
    /// lower-case hexadecimal digits and every other value in the shortest decimal that reads back
    /// to its bits (Rust's `Display`, which never writes an exponent), an input left to its draw
    /// left out.
    #[must_use]
    pub fn canonical(&self) -> String {
        let raw = &self.raw;
        let mut text = format!(
            "PV1;seed={:016x};star_mass_msun={};fe_h_dex={};age_gyr={};mass_mearth={};\
             semi_major_axis_au={};eccentricity={}",
            raw.seed,
            raw.star_mass_msun,
            raw.fe_h_dex,
            raw.age_gyr,
            raw.mass_mearth,
            raw.semi_major_axis_au,
            raw.eccentricity
        );
        let optional = [
            (InputField::FormedAt, raw.formed_at_au),
            (InputField::Day, raw.day_hours),
            (InputField::Tilt, raw.tilt_deg),
            (InputField::Water, raw.water_rank),
            (InputField::Carbon, raw.carbon_rank),
            (InputField::Nitrogen, raw.nitrogen_rank),
        ];
        for (field, value) in optional {
            if let Some(value) = value {
                write!(text, ";{field}={value}").expect("writing to a string never fails");
            }
        }
        text
    }

    /// The viewer system these inputs name: a [`PinnedId`] of the viewer's half, bit 57 set, above
    /// a 57-bit hash of the inputs' [canonical](Self::canonical) text (the [module
    /// documentation](self)), a pure function of the inputs.
    ///
    /// # Panics
    ///
    /// Never: the number is below 2⁵⁸.
    #[must_use]
    pub fn system_id(&self) -> SystemId {
        let text = self.canonical();
        let length = u64::try_from(text.len()).expect("a short text");
        let hash = text
            .as_bytes()
            .chunks(8)
            .fold(splitmix(length), |hash, chunk| {
                let mut word = [0_u8; 8];
                word[..chunk.len()].copy_from_slice(chunk);
                splitmix(hash ^ u64::from_le_bytes(word))
            });
        let number = VIEWER_PIN | (hash & (VIEWER_PIN - 1));
        SystemId::from(PinnedId::new(number).expect("a number below 2^58"))
    }

    /// The planet as [`generate_given`] takes it: its orbit in metres, its spin in seconds and
    /// radians, its ranks as uniforms.
    fn given_planet(&self) -> GivenPlanet {
        let raw = &self.raw;
        let au = |value: f64| Metres::from(AstronomicalUnits::new(value));
        let rank = |value: Option<f64>| {
            value.map(|v| UnitUniform::new(v).expect("a rank in 0.001-0.999 is in (0, 1)"))
        };
        GivenPlanet {
            orbit: GivenOrbit {
                mass: EarthMasses::new(raw.mass_mearth),
                semi_major_axis: au(raw.semi_major_axis_au),
                eccentricity: Eccentricity::new(raw.eccentricity)
                    .expect("an eccentricity in 0-0.9 is an ellipse's"),
                formation_distance: raw.formed_at_au.map(au),
            },
            spin: GivenSpin {
                period: raw.day_hours.map(|h| Seconds::new(h * SECONDS_PER_HOUR)),
                obliquity: raw.tilt_deg.map(|d| Radians::from(Degrees::new(d))),
            },
            volatiles: GivenVolatiles {
                water: rank(raw.water_rank),
                carbon: rank(raw.carbon_rank),
                nitrogen: rank(raw.nitrogen_rank),
            },
        }
    }
}

/// One round of `SplitMix64` (Steele, Lea and Flood 2014, OOPSLA; the finaliser's constants are
/// Vigna's `splitmix64.c`): the golden-ratio increment, then the mixing function, so that a fold of
/// it over a text's words is a hash in which every bit of every word reaches every bit of the
/// result.
#[must_use]
const fn splitmix(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Why a planet's inputs make no system.
#[derive(Debug, Clone, PartialEq)]
pub enum GenerateViewerError {
    /// The host cannot be built: a context's refusal, which checked inputs never reach.
    Host(BuildSystemContextError),
    /// The orbit's apoapsis lies beyond the system's reach (0.49 of its sphere of influence,
    /// design note 14), au.
    BeyondReach {
        /// The orbit's apoapsis, au.
        apoapsis_au: f64,
        /// The system's reach, au.
        reach_au: f64,
    },
    /// The orbit's periapsis lies inside its star's zero-age surface, au.
    InsideStar {
        /// The orbit's periapsis, au.
        periapsis_au: f64,
        /// The star's zero-age radius, au.
        radius_au: f64,
    },
    /// The star has no disc to form a planet in.
    NoDisc,
}

impl GenerateViewerError {
    /// The input the refusal is of: the orbit's for its reach and its periapsis, the host's mass
    /// for its disc and the host itself.
    #[must_use]
    pub const fn field(&self) -> InputField {
        match self {
            Self::Host(_) | Self::NoDisc => InputField::StarMass,
            Self::BeyondReach { .. } | Self::InsideStar { .. } => InputField::SemiMajorAxis,
        }
    }
}

impl fmt::Display for GenerateViewerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host(error) => write!(f, "the host cannot be built: {error}"),
            Self::BeyondReach {
                apoapsis_au,
                reach_au,
            } => write!(
                f,
                "the orbit's apoapsis at {apoapsis_au} au lies beyond the system's reach of {reach_au} au"
            ),
            Self::InsideStar {
                periapsis_au,
                radius_au,
            } => write!(
                f,
                "the orbit's periapsis at {periapsis_au} au lies inside its star's radius of {radius_au} au"
            ),
            Self::NoDisc => f.write_str("the star has no disc to form a planet in"),
        }
    }
}

impl Error for GenerateViewerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Host(error) => Some(error),
            Self::BeyondReach { .. } | Self::InsideStar { .. } | Self::NoDisc => None,
        }
    }
}

/// What the generator drew for the inputs left to it, and where the planet formed, as the panel
/// shows an `AUTO` input's value: each its draw, or the input where one was given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnInputs {
    /// Where it formed, au.
    pub formed_at_au: f64,
    /// Its primordial day, hours, floored at its break-up period; `None` where it has no rotation
    /// law (a body not present at the moon generators' time).
    pub day_hours: Option<f64>,
    /// Its primordial obliquity, degrees; `None` where it has no rotation law.
    pub tilt_deg: Option<f64>,
    /// Its water rank.
    pub water_rank: f64,
    /// Its carbon rank.
    pub carbon_rank: f64,
    /// Its nitrogen rank.
    pub nitrogen_rank: f64,
}

/// A planet the viewer generated from its inputs: its system's context and bodies, with its inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewerSystem {
    inputs: PlanetInputs,
    ctx: SystemContext,
    planets: PlanetarySystem,
}

impl ViewerSystem {
    /// The system of `inputs` (the [module documentation](self)).
    ///
    /// # Errors
    ///
    /// [`GenerateViewerError::BeyondReach`] for an apoapsis beyond the system's reach,
    /// [`GenerateViewerError::InsideStar`] for a periapsis inside its star's zero-age radius, and
    /// [`GenerateViewerError::NoDisc`] for a star with no disc; [`GenerateViewerError::Host`]
    /// for a host the builder refuses, which checked inputs never are.
    ///
    /// # Panics
    ///
    /// Never: a built context's single star has its zone and a positive, finite zero-age state.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::viewer::{PlanetInputs, RawPlanetInputs, ViewerSystem};
    /// use hyperion_sim::time::UniverseTime;
    ///
    /// let raw = RawPlanetInputs {
    ///     seed: 7,
    ///     star_mass_msun: 1.0,
    ///     fe_h_dex: 0.0,
    ///     age_gyr: 4.57,
    ///     mass_mearth: 1.0,
    ///     semi_major_axis_au: 1.0,
    ///     eccentricity: 0.0167,
    ///     formed_at_au: None,
    ///     day_hours: Some(24.0),
    ///     tilt_deg: Some(23.4),
    ///     water_rank: None,
    ///     carbon_rank: None,
    ///     nitrogen_rank: None,
    /// };
    /// let system = ViewerSystem::generate(PlanetInputs::try_from(raw)?)?;
    /// let index = system.planet().body_index().try_into()?;
    /// let record = system.planets().body_at(system.context(), index, UniverseTime::EPOCH)?;
    /// // Given at birth, held to the bit: the day and the tilt are the inputs'.
    /// let drawn = system.drawn();
    /// assert_eq!(drawn.day_hours, Some(24.0));
    /// assert!((drawn.tilt_deg.ok_or("a rotation law")? - 23.4).abs() < 1e-12);
    /// assert_eq!(record.identity().id(), system.planet());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn generate(inputs: PlanetInputs) -> Result<Self, GenerateViewerError> {
        let raw = inputs.raw;
        let id = inputs.system_id();
        let seed = inputs.seed();
        // A gigayear count's product can round a whisker past the universe's age.
        let age = Years::new((raw.age_gyr * YEARS_PER_GYR).min(UNIVERSE_AGE.value()));
        let ctx = SystemContext::builder()
            .system(id)
            .star(SolarMasses::new(raw.star_mass_msun))
            .fe_h(Dex::new(raw.fe_h_dex))
            .age_at_epoch(age)
            .star_draws(SyntheticDraws::OfUniverse(seed))
            .build()
            .map_err(GenerateViewerError::Host)?;
        let given = inputs.given_planet();
        let a = given.orbit.semi_major_axis;
        let e = given.orbit.eccentricity.value();
        let au = |m: Metres| AstronomicalUnits::from(m).value();
        let reach = ctx.strip_radius();
        let apoapsis = a * (1.0 + e);
        if apoapsis > reach {
            return Err(GenerateViewerError::BeyondReach {
                apoapsis_au: au(apoapsis),
                reach_au: au(reach),
            });
        }
        let zone = ctx
            .zones()
            .into_iter()
            .next()
            .expect("a single star has its zone");
        let radius = Metres::from(
            ZoneDiscInputs::for_zone(seed, id, &zone, &ctx.zone_stars(), ctx.fe_h())
                .expect("a built context gives its star a positive, finite zero-age state")
                .host()
                .zams_radius(),
        );
        let periapsis = a * (1.0 - e);
        if periapsis <= radius {
            return Err(GenerateViewerError::InsideStar {
                periapsis_au: au(periapsis),
                radius_au: au(radius),
            });
        }
        let planets = generate_given(seed, &ctx, &given).ok_or(GenerateViewerError::NoDisc)?;
        Ok(Self {
            inputs,
            ctx,
            planets,
        })
    }

    /// The inputs it was generated from.
    #[must_use]
    pub const fn inputs(&self) -> &PlanetInputs {
        &self.inputs
    }

    /// The system's context: its synthetic star.
    #[must_use]
    pub const fn context(&self) -> &SystemContext {
        &self.ctx
    }

    /// The system's bodies as they were born.
    #[must_use]
    pub const fn planets(&self) -> &PlanetarySystem {
        &self.planets
    }

    /// The system's ID ([`PlanetInputs::system_id`]).
    #[must_use]
    pub fn id(&self) -> SystemId {
        self.ctx.id()
    }

    /// The planet: body `0x0100` of the system.
    #[must_use]
    pub fn planet(&self) -> BodyId {
        viewer_planet_index().body_id(self.id())
    }

    /// The context and the bodies, for a caller that keeps them as a generated system.
    #[must_use]
    pub fn into_parts(self) -> (SystemContext, PlanetarySystem) {
        (self.ctx, self.planets)
    }

    /// The host's summary at `t`: what `SystemStars::summary_at` gives a galaxy system's stars,
    /// for its single synthetic star.
    ///
    /// # Panics
    ///
    /// In debug builds, for a `t` after the clock window's end.
    #[must_use]
    pub fn summary_at(&self, t: UniverseTime) -> SystemSummary {
        summary_of(&self.ctx, self.inputs.seed(), t)
    }

    /// The discs of the host at `t`, as the sky's `host_discs` gives a galaxy system's; none before
    /// it is born.
    #[must_use]
    pub fn host_discs_at(&self, t: UniverseTime) -> Vec<HostDisc> {
        host_discs_of_context(&self.ctx, t)
    }

    /// What the generator drew for the inputs left to it ([`DrawnInputs`]).
    ///
    /// # Panics
    ///
    /// Never: the system holds its planet.
    #[must_use]
    pub fn drawn(&self) -> DrawnInputs {
        let parts = self
            .planets
            .planet_parts(viewer_planet_index())
            .expect("a viewer system holds its planet");
        drawn_of(&parts)
    }
}

/// The host's summary at `t` of a viewer system's context `ctx`, whose streams' seed is `seed`
/// ([`ViewerSystem::summary_at`]).
///
/// # Panics
///
/// For a context with no star, which no viewer system has.
#[must_use]
pub fn summary_of(ctx: &SystemContext, seed: Seed, t: UniverseTime) -> SystemSummary {
    let hierarchy = ctx
        .hierarchy()
        .expect("a viewer system's host has a hierarchy");
    synthetic_summary_at(seed, ctx.stars(), hierarchy, ctx.existence_at(t), t)
}

/// The host's discs at `t` of a viewer system's context `ctx` ([`ViewerSystem::host_discs_at`]);
/// none before it is born.
#[must_use]
pub fn host_discs_of_context(ctx: &SystemContext, t: UniverseTime) -> Vec<HostDisc> {
    let states: Option<Vec<StarState>> = ctx.stars().iter().map(|star| star.state_at(t)).collect();
    match (states, ctx.stars().first()) {
        (Some(states), Some(primary)) => host_discs_of(primary.rules(), &states),
        (None, _) | (_, None) => Vec::new(),
    }
}

/// Whether `id` is a viewer system's: a pinned ID with the viewer's bit set.
#[must_use]
pub fn is_viewer_system(id: SystemId) -> bool {
    match id.kind() {
        crate::id::SystemIdKind::Pinned(pin) => pin.number() & VIEWER_PIN != 0,
        crate::id::SystemIdKind::Grid(_)
        | crate::id::SystemIdKind::FeatureMember(_)
        | crate::id::SystemIdKind::Centre(_)
        | crate::id::SystemIdKind::Stream(_)
        | crate::id::SystemIdKind::DwarfCore(_)
        | crate::id::SystemIdKind::Catalogue(_) => false,
    }
}

/// The viewer planet's body index, `0x0100`.
#[must_use]
fn viewer_planet_index() -> BodyIndex {
    BodyIndex::new(BodySlot::Planet(VIEWER_PLANET_SLOT), BodySub::Primary)
        .expect("slot 1 is a planet's")
}

/// The drawn values of the planet `parts` holds.
#[must_use]
fn drawn_of(parts: &PlanetParts) -> DrawnInputs {
    DrawnInputs {
        formed_at_au: AstronomicalUnits::from(parts.placed.formation_distance()).value(),
        day_hours: parts
            .rotation
            .map(|rotation| rotation.primordial_period().value() / SECONDS_PER_HOUR),
        tilt_deg: parts
            .rotation
            .map(|rotation| Degrees::from(rotation.obliquity()).value()),
        water_rank: parts.volatiles.water.value(),
        carbon_rank: parts.volatiles.carbon.value(),
        nitrogen_rank: parts.volatiles.nitrogen.value(),
    }
}

/// The inputs of the planet `index` of the system `planets` of `ctx`, read back: its host's initial
/// mass (its zone's, a pair's total for a circumbinary planet), \[Fe/H\] and age, its primordial
/// mass, orbit and formation distance, its primordial day and obliquity where it has a rotation
/// law, its water, carbon and nitrogen ranks, and its system's seed. `None` for any body but a
/// planet.
///
/// The planet viewer regenerates a planet from them as a viewer system of its own: its host is a
/// single synthetic star, and every draw not among them (the radius rank, the formation time, the
/// moons) its own ID's. The values are as the system holds them, and may lie outside the inputs'
/// ranges (a system younger than 1 Myr, an eccentricity above 0.9).
#[must_use]
pub fn inputs_of(
    ctx: &SystemContext,
    planets: &PlanetarySystem,
    index: BodyIndex,
) -> Option<RawPlanetInputs> {
    let parts = planets.planet_parts(index)?;
    let drawn = drawn_of(&parts);
    let orbit = parts.placed.orbit();
    Some(RawPlanetInputs {
        seed: planets.seed().get(),
        star_mass_msun: parts.host_mass.value(),
        fe_h_dex: ctx.fe_h().value(),
        age_gyr: ctx.age_at_epoch().value() / YEARS_PER_GYR,
        mass_mearth: parts.placed.mass().value(),
        semi_major_axis_au: AstronomicalUnits::from(orbit.semi_major_axis()).value(),
        eccentricity: orbit.eccentricity().value(),
        formed_at_au: Some(drawn.formed_at_au),
        day_hours: drawn.day_hours,
        tilt_deg: drawn.tilt_deg,
        water_rank: Some(drawn.water_rank),
        carbon_rank: Some(drawn.carbon_rank),
        nitrogen_rank: Some(drawn.nitrogen_rank),
    })
}

/// A real planet's inputs as `RANDOMIZE` drew them, and the planet they were read from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnPlanet {
    /// The inputs: the planet's host, orbit, mass and formation distance, with its day, obliquity
    /// and ranks left to be drawn, and the draw's seed.
    pub inputs: PlanetInputs,
    /// The planet they were read from, in the galaxy drawn from.
    pub source: BodyId,
}

/// Why `RANDOMIZE` found no planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawInputsError {
    tries: u64,
}

impl DrawInputsError {
    /// The points drawn.
    #[must_use]
    pub const fn tries(&self) -> u64 {
        self.tries
    }
}

impl fmt::Display for DrawInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no single star with a present planet nearest {} points drawn from the galaxy",
            self.tries
        )
    }
}

impl Error for DrawInputsError {}

/// The galaxy's number density of systems on a galactocentric grid of radius, azimuth and height
/// ([`radius_edges`], [`AZIMUTH_BINS`], [`HEIGHT_EDGES_LY`], both sides of the plane), each bin's
/// expected count its density at its centre times its volume: `RANDOMIZE`'s point is drawn from it,
/// a bin by its count and then uniformly in the bin's volume, so that it follows the galaxy's own
/// density to a bin (plan 02's [`Fields::densities`](crate::galaxy::fields::Fields::densities), all
/// mass bands together, at the epoch).
#[derive(Debug, Clone, PartialEq)]
pub struct DensityGrid {
    radii: Vec<f64>,
    heights: Vec<f64>,
    /// Each bin's cumulative expected count, by radius, then azimuth, then height.
    cumulative: Vec<f64>,
}

impl DensityGrid {
    /// The grid of `galaxy`: some 73,000 density evaluations, a second or so on one core.
    #[must_use]
    pub fn of(galaxy: &Galaxy) -> Self {
        let radii = radius_edges();
        let heights: Vec<f64> = HEIGHT_EDGES_LY
            .iter()
            .rev()
            .map(|h| -h)
            .chain(HEIGHT_EDGES_LY.iter().skip(1).copied())
            .collect();
        let step = core::f64::consts::TAU / f64::from(AZIMUTH_BINS);
        let mut out = [0.0; crate::galaxy::fields::MAX_COMPONENTS];
        let mut total = 0.0;
        let mut cumulative =
            Vec::with_capacity((radii.len() - 1) * usize::from(AZIMUTH_BINS) * (heights.len() - 1));
        for r in radii.windows(2) {
            let r_mid = f64::midpoint(r[0], r[1]);
            let area = 0.5 * (r[1] * r[1] - r[0] * r[0]) * step;
            for k in 0..AZIMUTH_BINS {
                let phi = step * (f64::from(k) + 0.5);
                let (x, y) = (r_mid * crate::math::cos(phi), r_mid * crate::math::sin(phi));
                for z in heights.windows(2) {
                    let p = PointLy::new(x, y, f64::midpoint(z[0], z[1]));
                    total += galaxy.fields().densities(&p, &mut out) * area * (z[1] - z[0]);
                    cumulative.push(total);
                }
            }
        }
        Self {
            radii,
            heights,
            cumulative,
        }
    }

    /// The point `draws` place, light-years along the galactic axes: the bin whose cumulative
    /// count first passes `draws[0]` of the whole, and in it, uniformly in volume, `draws[1]` of
    /// its area's radius squared, `draws[2]` of its azimuth and `draws[3]` of its height. Each draw
    /// is in `[0, 1)`.
    ///
    /// # Panics
    ///
    /// Never: the grid has bins, fewer than 2¹⁶ azimuths among them.
    #[must_use]
    pub fn point(&self, draws: [f64; 4]) -> [f64; 3] {
        let total = self.cumulative.last().copied().unwrap_or(0.0);
        let target = draws[0] * total;
        let bin = self
            .cumulative
            .partition_point(|&count| count <= target)
            .min(self.cumulative.len() - 1);
        let heights = self.heights.len() - 1;
        let per_radius = usize::from(AZIMUTH_BINS) * heights;
        let (radius_bin, rest) = (bin / per_radius, bin % per_radius);
        let (azimuth_bin, height_bin) = (rest / heights, rest % heights);
        let (inner, outer) = (self.radii[radius_bin], self.radii[radius_bin + 1]);
        let radius = (inner * inner + draws[1] * (outer * outer - inner * inner)).sqrt();
        let step = core::f64::consts::TAU / f64::from(AZIMUTH_BINS);
        let azimuth = u16::try_from(azimuth_bin).expect("fewer than 2^16 azimuths");
        let phi = step * (f64::from(azimuth) + draws[2]);
        let (below, above) = (self.heights[height_bin], self.heights[height_bin + 1]);
        [
            radius * crate::math::cos(phi),
            radius * crate::math::sin(phi),
            below + draws[3] * (above - below),
        ]
    }
}

/// `RANDOMIZE`'s inputs: a real planet of `galaxy`, drawn from `draw_seed` (the [module
/// documentation](self)), a pure function of the two; `grid` is the galaxy's own
/// ([`DensityGrid::of`]), which a caller drawing again keeps.
///
/// Each try opens [`tags::VIEWER_DRAW`] with `draw_seed` and the try's number, and reads its words
/// in order: four uniforms, the point ([`DensityGrid::point`]); the planet picked among the
/// nearest system's present ones, by index; and the new seed. The nearest system is the nearest
/// at the epoch of every stellar layer's systems, ties broken by ID, searched in spheres from
/// [`FIRST_SEARCH_LY`] doubling while one covers at most [`MAX_SEARCH_CELLS`] cells. A try moves
/// on where it finds no system, a system of more than one star or with no planet present at the
/// epoch, or a planet whose inputs lie outside their ranges.
///
/// # Errors
///
/// [`DrawInputsError`] after [`DRAW_TRIES`] tries that find none.
///
/// # Panics
///
/// Never: every point drawn lies inside the root cube.
pub fn draw_inputs(
    galaxy: &Galaxy,
    grid: &DensityGrid,
    draw_seed: u64,
) -> Result<DrawnPlanet, DrawInputsError> {
    let mut cell = Vec::new();
    for attempt in 0..DRAW_TRIES {
        let mut stream = Stream::open(
            Seed::new(draw_seed),
            tags::VIEWER_DRAW,
            ObjectKey::galaxy_item(attempt),
        );
        let point = grid.point([0; 4].map(|_| stream.uniform()));
        let planet_word = stream.next_u64();
        let seed = stream.next_u64();
        let centre = GalacticPosition::from_light_years(point)
            .expect("a point within 60,000 ly of the centre lies inside the root cube");
        let Some(record) = nearest_system(galaxy, &centre, &mut cell) else {
            continue;
        };
        if record.kind() != SystemKind::Stellar {
            continue;
        }
        let ctx = SystemContext::from_record(galaxy, &record);
        if ctx.host_kind() != HostKind::Stellar || ctx.stars().len() != 1 {
            continue;
        }
        let planets = generate(galaxy.seed(), &ctx);
        let present: Vec<BodyIndex> = planets
            .planets()
            .map(crate::planetary::Body::index)
            .filter(|&index| {
                planets
                    .body_at(&ctx, index, UniverseTime::EPOCH)
                    .is_ok_and(|body| body.identity().state() == BodyState::Present)
            })
            .collect();
        let Some(&index) = pick(&present, planet_word) else {
            continue;
        };
        let Some(raw) = inputs_of(&ctx, &planets, index) else {
            continue;
        };
        let raw = RawPlanetInputs {
            seed,
            day_hours: None,
            tilt_deg: None,
            water_rank: None,
            carbon_rank: None,
            nitrogen_rank: None,
            ..raw
        };
        if let Ok(inputs) = PlanetInputs::try_from(raw) {
            return Ok(DrawnPlanet {
                inputs,
                source: index.body_id(record.id()),
            });
        }
    }
    Err(DrawInputsError { tries: DRAW_TRIES })
}

/// The system of `galaxy`'s stellar layers nearest `centre` at the epoch, ties broken by ID, in
/// spheres from [`FIRST_SEARCH_LY`] doubling while one covers at most [`MAX_SEARCH_CELLS`] cells;
/// `None` if none holds one. `cell` is scratch.
fn nearest_system(
    galaxy: &Galaxy,
    centre: &GalacticPosition,
    cell: &mut Vec<SystemRecord>,
) -> Option<SystemRecord> {
    let mut radius = FIRST_SEARCH_LY;
    loop {
        let sphere = QuerySphere::new(
            *centre,
            LightYears::new(radius),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a positive, finite radius at the epoch, where no pad is needed");
        let layers = STELLAR_LAYERS.iter().map(LayerSpec::layer);
        if layers
            .clone()
            .map(|layer| count_cells_in_sphere(layer, &sphere))
            .sum::<u64>()
            > MAX_SEARCH_CELLS
        {
            return None;
        }
        let limit = Metres::from(LightYears::new(radius));
        let mut nearest: Option<(Metres, SystemRecord)> = None;
        for layer in layers {
            for key in cells_in_sphere(layer, &sphere) {
                generate_cell(galaxy, key, cell);
                for record in cell.iter() {
                    let distance = centre.distance_to(record.epoch_position());
                    let nearer = nearest.as_ref().is_none_or(|(best, kept)| {
                        distance
                            .value()
                            .total_cmp(&best.value())
                            .then_with(|| record.id().cmp(&kept.id()))
                            .is_lt()
                    });
                    if distance <= limit && nearer {
                        nearest = Some((distance, *record));
                    }
                }
            }
        }
        if let Some((_, record)) = nearest {
            return Some(record);
        }
        radius *= 2.0;
    }
}

/// The item of `items` that `word` picks uniformly (Lemire's multiply-shift, 2019, ACM TOMACS 29,
/// 3: ⌊word × n ÷ 2⁶⁴⌋), or `None` from none.
#[must_use]
fn pick<T>(items: &[T], word: u64) -> Option<&T> {
    let n = u64::try_from(items.len()).ok()?;
    let k = (u128::from(word) * u128::from(n)) >> 64;
    items.get(usize::try_from(k).ok()?)
}

#[cfg(all(test, not(target_family = "wasm")))]
mod colour_census;
#[cfg(test)]
mod tests;
