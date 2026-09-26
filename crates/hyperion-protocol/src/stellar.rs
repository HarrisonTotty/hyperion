//! Stellar messages: the brief a range-query row may carry, and the `system_summary` request that
//! describes every star of one system at a time (plan 06, P06.T33, with plan 11's P11.T13 in part).
//!
//! These are wire types only, mirroring the simulation's `stellar` types without depending on
//! them. Every quantity carries its unit in its field name, and every unit is one the bridge
//! client's style guide allows or gains with plan 06 (P06.T35.a): M☉, L☉, R☉, K, mag, dex, days
//! (`d`), gauss (`G`), km/s, Myr and years.
//!
//! # Absent, `null` and a value
//!
//! A star's optional fields take one of three states on the wire, and the server is the authority
//! on which:
//!
//! - **Absent**: this generator version does not compute the quantity at all (planetary nebulae
//!   and active events until plan 06's T16 and T28 land; rotation and activity for the objects
//!   plan 06's T25 does not model). The client shows the style guide's "Missing"
//!   state, the em dash, and never takes it for "none" (ruling 34 of 2026-09-22, for a single value
//!   the generator does not compute). Such a field is `#[serde(default)]`, skipped when `None`, and
//!   optional in TypeScript, so that it can be filled later without changing any wire form.
//! - **`null`**: the quantity is computed and this object has none: no absolute magnitude for a
//!   black hole, no remnant for a living star, no death within the clock window. This is plan 04's
//!   convention for an `Option`, whose key is always present.
//! - **A value.**
//!
//! A field that is absent today and can be `null` once it is computed (a star that does not vary,
//! one with no nebula) is a [`Modelled`], which TypeScript sees as `name?: T | null`. One that is
//! absent today and always has a value once computed is an `Option` skipped when `None`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::modelled::Modelled;
use crate::orbit::HierarchyDto;
use crate::primitives::{SystemIdHex, UniverseIdHex, UniverseTime};

/// What an object is now, as the range query's rows and the chart's symbols read it: coarser than
/// its [`PhaseDto`], and decided from the phase and the classification (plan 06, design note 17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ObjectKindDto {
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

/// A star's evolutionary phase: the stellar types 0–15 of Hurley, Pols and Tout (2000, MNRAS 315,
/// 543), with the stages plan 06 wraps around them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PhaseDto {
    /// Class 0 and I: from the onset of collapse, still gaining mass.
    Protostar,
    /// Contraction towards the zero-age main sequence: T Tauri and Herbig Ae/Be stars.
    PreMainSequence,
    /// Core hydrogen burning (types 0 and 1).
    MainSequence,
    /// The Hertzsprung gap (type 2).
    HertzsprungGap,
    /// The first giant branch (type 3).
    FirstGiantBranch,
    /// Core helium burning: the horizontal branch, the red clump and the blue loops (type 4).
    CoreHeliumBurning,
    /// The early asymptotic giant branch (type 5).
    EarlyAgb,
    /// The thermally pulsing asymptotic giant branch (type 6).
    ThermallyPulsingAgb,
    /// A naked helium star burning helium in its core (type 7).
    HeliumMainSequence,
    /// A naked helium star crossing the Hertzsprung gap (type 8).
    HeliumHertzsprungGap,
    /// A naked helium star on its giant branch (type 9).
    HeliumGiantBranch,
    /// The crossing from the end of the asymptotic giant branch to the white dwarf cooling track.
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
    /// Nothing: the star was destroyed and left no remnant (type 15).
    NoRemnant,
    /// A brown dwarf, below the hydrogen-burning limit, on the cooling fits. A star above the limit
    /// on the same fits, one of the latest M dwarfs, burns hydrogen and is sent as `MainSequence`.
    Substellar,
}

/// What a range-query row says of its system's primary: enough to draw its symbol and place it
/// on a Hertzsprung–Russell diagram.
///
/// It is sent only when the request sets `include_stellar`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StellarBriefDto {
    /// What the primary is now.
    pub kind: ObjectKindDto,
    /// Its class as an astronomer writes it: `G2V`, `M5III`, `DA4.2`, `NS`, `BH`, `NONE`.
    pub class: String,
    /// log₁₀ of its bolometric luminosity in L☉; `null` for an object with no luminosity (a black
    /// hole, or nothing).
    pub log_luminosity_lsun: Option<f32>,
    /// Its effective temperature, K; `null` for an object with no luminosity.
    pub teff_k: Option<f32>,
    /// How many stars the system has, the primary included: 1 to 4 (plan 11, P11.T13).
    pub star_count: u8,
}

/// Asks for every star of one system as it is at one time (`system_summary`), answered with a
/// [`SystemSummaryDto`].
///
/// The server resolves the system ID first: a well-formed ID that names no system is refused with
/// `unknown_system`, and a time outside the clock window with `bad_request` naming `time`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemSummaryRequest {
    /// The universe the system is in.
    pub universe: UniverseIdHex,
    /// The system.
    pub system: SystemIdHex,
    /// The instant at which the stars are described, within 1,000 years of the epoch.
    pub time: UniverseTime,
}

/// Whether a system exists at a time: its stars form at its birth, which may lie after the epoch
/// in populations that still form stars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SystemExistenceDto {
    /// The system has not formed yet; it has no stars at this time.
    NotYetBorn,
    /// The system exists.
    Exists,
}

/// Every star of one system at one time, and the orbits that hold them together.
///
/// `universe`, `system` and `time` echo the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemSummaryDto {
    /// The universe the system is in.
    pub universe: UniverseIdHex,
    /// The system described.
    pub system: SystemIdHex,
    /// The instant at which it is described.
    pub time: UniverseTime,
    /// Whether it exists at `time`.
    pub existence: SystemExistenceDto,
    /// Its age at `time` in megayears, counted from the onset of its stars' collapse: negative
    /// before it forms. Every star of a system is the same age.
    pub age_myr: f64,
    /// Its metallicity \[Fe/H\] in dex, drawn once for the whole system and fixed at its birth.
    pub fe_h_dex: f64,
    /// Every star, by body index: the primary first. Empty when the system has not yet formed.
    pub stars: Vec<StarSummaryDto>,
    /// How the stars are arranged in pairs and on which orbits, at `time`.
    pub hierarchy: HierarchyDto,
}

/// One star of a [`SystemSummaryDto`], as it is at the summary's time.
///
/// The module documentation says what an absent field and a `null` one mean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StarSummaryDto {
    /// Its body index: 0 for the primary, 1–15 for companions (plan 14, design note 3).
    pub body_index: u16,
    /// What it is now.
    pub kind: ObjectKindDto,
    /// Its evolutionary phase.
    pub phase: PhaseDto,
    /// Its class as an astronomer writes it: `G2V`, `M5III`, `sdM3`, `T6`, `DA4.2` (a white
    /// dwarf's class is its spectral type), `NS`, `BH`, `NONE`.
    pub class: String,
    /// Its mass at birth, M☉.
    pub initial_mass_msun: f64,
    /// Its mass now, M☉: a remnant's gravitational mass, zero where nothing is left.
    pub mass_msun: f64,
    /// Its core mass, M☉: the helium core on the giant branch, the carbon–oxygen core of a helium
    /// star, the whole mass of a remnant.
    pub core_mass_msun: f64,
    /// Its bolometric luminosity, L☉: exactly zero for a black hole and where nothing is left.
    pub luminosity_lsun: f64,
    /// Its radius, R☉: the photosphere's, a remnant's physical radius, a black hole's Schwarzschild
    /// radius, zero where nothing is left.
    pub radius_rsun: f64,
    /// Its effective temperature, K; `null` for an object with no luminosity.
    pub teff_k: Option<f64>,
    /// Its absolute visual magnitude `M_V`, mag; `null` where the bolometric corrections have no
    /// value: every white dwarf, neutron star and black hole, and objects cooler than the dwarf
    /// sequence's tables.
    pub absolute_v_mag: Option<f64>,
    /// Its colour index B − V, mag; `null` where the colour tables have no value, as for
    /// `absolute_v_mag`.
    pub colour_b_v_mag: Option<f64>,
    /// The rate at which its wind carries mass away, M☉ per year: positive for loss, negative
    /// while a protostar still gains mass.
    pub mass_loss_rate_msun_per_yr: f64,
    /// What it left when it died; `null` for a star still living.
    pub remnant: Option<RemnantDto>,
    /// The clock time of its death when that falls within the clock window, 1,000 years either
    /// side of the epoch, before or after `time`; `null` when it does not.
    pub death_time: Option<UniverseTime>,
    /// Its rotation period, days (a neutron star's spin period included); `null` for an object
    /// without one; absent where plan 06's T25 does not model it (a white dwarf, a stripped helium
    /// star, a post-AGB star, a brown dwarf).
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<f64>>", optional)]
    pub rotation_period_d: Modelled<f64>,
    /// Its magnetic activity as log₁₀ of the ratio of its X-ray to its bolometric luminosity,
    /// −3.13 when saturated (Wright et al. 2011); `null` for an object without a convective
    /// dynamo (a hot main-sequence star, a remnant); absent where plan 06's T25 does not model it
    /// (an evolved star, a brown dwarf).
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<f64>>", optional)]
    pub activity_log_lx_lbol: Modelled<f64>,
    /// How its light varies (plan 06's T26.a–c); `null` for a star that does not vary.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<VariabilityDto>>", optional)]
    pub variability: Modelled<VariabilityDto>,
    /// The planetary nebula it lights; `null` when it lights none. Absent until plan 06's T16.
    #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
    #[ts(as = "Option<Option<PlanetaryNebulaDto>>", optional)]
    pub planetary_nebula: Modelled<PlanetaryNebulaDto>,
    /// Its events in progress at the summary's time, by onset; empty when none is. Absent until
    /// plan 06's T28.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub active_events: Option<Vec<StarEventDto>>,
}

/// What a dead star left, by kind, with what is known of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum RemnantDto {
    /// A white dwarf. Its spectral type is the star's `class`, and its composition its `phase`.
    WhiteDwarf {
        /// The time since it formed, in megayears.
        cooling_age_myr: f64,
        /// The kick it was born with. Absent until plan 06's T19.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        natal_kick: Option<NatalKickDto>,
    },
    /// A neutron star.
    NeutronStar {
        /// Its spin and field as a pulsar (plan 06's T21).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        pulsar: Option<PulsarDto>,
        /// The kick it was born with. Absent until plan 06's T19.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        natal_kick: Option<NatalKickDto>,
    },
    /// A black hole.
    BlackHole {
        /// Its dimensionless spin c J ÷ (G M²), in `[0, 0.998)` (plan 06's T22).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        dimensionless_spin: Option<f64>,
        /// The kick it was born with. Absent until plan 06's T19.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        natal_kick: Option<NatalKickDto>,
    },
    /// Nothing: the star was destroyed, by pair instability or by the carbon ignition of a
    /// degenerate core.
    NoRemnant,
}

/// A neutron star seen as a pulsar: its spin, its spin-down and its field at the summary's time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PulsarDto {
    /// The spin period P, seconds: positive, from milliseconds for the fastest pulsars to about ten
    /// seconds for a magnetar.
    pub spin_period_s: f64,
    /// The period derivative Ṗ, seconds per second: non-negative, since the spin only slows.
    pub period_derivative_s_per_s: f64,
    /// The dipole field at the surface, gauss: positive, about 10⁸–10¹⁵ G.
    pub magnetic_field_g: f64,
    /// Whether it is above the death line, still shining as a radio pulsar.
    pub alive: bool,
    /// Whether it is a magnetar, powered by the decay of its field.
    pub magnetar: bool,
}

/// The kick a remnant was born with.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NatalKickDto {
    /// Its speed relative to the progenitor's rest frame just before the collapse, km/s:
    /// non-negative, and zero for a black hole that swallowed its whole fallback.
    pub speed_km_s: f64,
    /// Which branch of the kick law gave it.
    pub mode: KickModeDto,
}

/// The branch of plan 06's kick law that gave a remnant its kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum KickModeDto {
    /// The ordinary kick of an iron-core collapse.
    Ordinary,
    /// The low kick of an electron-capture or ultra-stripped collapse.
    Low,
    /// No kick: the collapse fell back entirely onto a black hole.
    FallbackNone,
    /// The small kick of a white dwarf's asymmetric mass loss.
    WhiteDwarf,
}

/// How a star's light varies: the kind of variable, its period and its amplitude at the
/// summary's time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VariabilityDto {
    /// The kind of variable.
    pub kind: VariableKindDto,
    /// The period of its main cycle, days: positive, from minutes for a pulsating white dwarf to
    /// decades for the S Doradus cycles of a luminous blue variable.
    pub period_d: f64,
    /// Its full amplitude, peak to peak, in the V band, mag: non-negative, and zero at the edges
    /// of an instability strip.
    pub amplitude_mag: f64,
}

/// The kinds of variable star plan 06 names (P06.T26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum VariableKindDto {
    /// δ Scuti: main-sequence and Hertzsprung-gap pulsators of 1.5–2.5 M☉.
    DeltaScuti,
    /// RR Lyrae: low-mass core-helium-burning pulsators.
    RrLyrae,
    /// A classical Cepheid, on a blue loop or crossing the gap, 3–20 M☉.
    ClassicalCepheid,
    /// A type II Cepheid of the BL Herculis class, the shortest periods.
    BlHerculis,
    /// A type II Cepheid of the W Virginis class.
    WVirginis,
    /// A type II Cepheid of the RV Tauri class, the longest periods.
    RvTauri,
    /// A Mira: a long-period variable of the asymptotic giant branch with a large amplitude.
    Mira,
    /// A semiregular giant of class `SRa`, with persistent periodicity.
    SemiregularA,
    /// A semiregular giant of class `SRb`, with poorly defined periodicity.
    SemiregularB,
    /// A semiregular supergiant of class `SRc`.
    SemiregularC,
    /// A slow irregular supergiant of class `Lc`.
    SlowIrregular,
    /// A β Cephei star: an early-B pulsator.
    BetaCephei,
    /// A slowly pulsating B star.
    SlowlyPulsatingB,
    /// A γ Doradus star: an early-F pulsator.
    GammaDoradus,
    /// A ZZ Ceti star: a pulsating DA white dwarf of 10,500–12,500 K.
    ZzCeti,
    /// A V777 Herculis star: a pulsating DB white dwarf of 22,000–29,000 K.
    V777Herculis,
    /// A GW Virginis star: a pulsating hot pre-white dwarf.
    GwVirginis,
    /// An α Cygni variable: a pulsating supergiant.
    AlphaCygni,
    /// The S Doradus cycles of a luminous blue variable.
    SDoradus,
    /// A BY Draconis variable: spots on an active cool dwarf, modulated by its rotation.
    ByDraconis,
    /// An α² Canum Venaticorum variable: a chemically peculiar star modulated by its rotation.
    Alpha2CanumVenaticorum,
}

/// The planetary nebula a post-AGB star lights (plan 06, P06.T16.b).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlanetaryNebulaDto {
    /// The shell's radius, light-years: positive, and under 2.7 ly (0.8 pc).
    pub radius_ly: f64,
    /// The speed at which it expands, km/s: 20–40 km/s.
    pub expansion_speed_km_s: f64,
    /// The time since it was ejected, years: non-negative, and under its visibility time of some
    /// 20,000–40,000 years.
    pub age_yr: f64,
    /// The mass of its ionised gas, M☉: positive.
    pub ionised_mass_msun: f64,
    /// Its excitation class, which rises with its central star's temperature, on the scale
    /// P06.T16.b adopts and records with its source.
    pub excitation_class: u8,
}

/// One of a star's events in progress at the summary's time (plan 06, P06.T28).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StarEventDto {
    /// What kind of event it is.
    pub kind: StarEventKindDto,
    /// When it began.
    pub onset: UniverseTime,
    /// How long it lasts from its onset, seconds: positive, from a fraction of a second for a
    /// magnetar's burst to a century or more for an FU Orionis outburst.
    pub duration_s: f64,
}

/// The kinds of event a single star can have (plan 06, P06.T28).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StarEventKindDto {
    /// A flare on a star with a convective envelope.
    Flare,
    /// A pulsar's glitch: a sudden rise in its spin frequency.
    Glitch,
    /// A short burst of a magnetar.
    MagnetarBurst,
    /// A magnetar's giant flare.
    MagnetarGiantFlare,
    /// An FU Orionis outburst of a young star's disc.
    FuOrionisOutburst,
    /// A giant eruption of a luminous blue variable.
    GiantEruption,
    /// A thermal pulse of an asymptotic-giant-branch star's helium shell.
    ThermalPulse,
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::envelope::{
        ClientMessage, ErrorCode, RequestBody, RequestError, RequestId, ResponseBody, ServerMessage,
    };
    use crate::orbit::{HierarchyNodeDto, OrbitDto};
    use crate::testing::{assert_wire_form, assert_wire_strings};

    const UNIVERSE: u64 = 0x0123_4567_89ab_cdef;
    const SYSTEM: u64 = 0x0200_0800_2000_0000;

    fn epoch_plus_a_century() -> UniverseTime {
        UniverseTime {
            seconds: 3_155_760_000,
            nanos: 0,
        }
    }

    fn request() -> SystemSummaryRequest {
        SystemSummaryRequest {
            universe: UniverseIdHex::from_u64(UNIVERSE),
            system: SystemIdHex::from_u64(SYSTEM),
            time: epoch_plus_a_century(),
        }
    }

    /// A Sun-like primary, with every field this generator version computes and none it does not.
    pub(crate) fn sunlike() -> StarSummaryDto {
        StarSummaryDto {
            body_index: 0,
            kind: ObjectKindDto::Dwarf,
            phase: PhaseDto::MainSequence,
            class: "G2V".to_owned(),
            initial_mass_msun: 1.0,
            mass_msun: 0.999_75,
            core_mass_msun: 0.0,
            luminosity_lsun: 1.0,
            radius_rsun: 1.0,
            teff_k: Some(5_772.0),
            absolute_v_mag: Some(4.825),
            colour_b_v_mag: Some(0.65),
            mass_loss_rate_msun_per_yr: 2.5e-14,
            remnant: None,
            death_time: None,
            rotation_period_d: Modelled::NotModelled,
            activity_log_lx_lbol: Modelled::NotModelled,
            variability: Modelled::NotModelled,
            planetary_nebula: Modelled::NotModelled,
            active_events: None,
        }
    }

    pub(crate) fn sunlike_json() -> Value {
        json!({
            "body_index": 0,
            "kind": "dwarf",
            "phase": "main_sequence",
            "class": "G2V",
            "initial_mass_msun": 1.0,
            "mass_msun": 0.999_75,
            "core_mass_msun": 0.0,
            "luminosity_lsun": 1.0,
            "radius_rsun": 1.0,
            "teff_k": 5_772.0,
            "absolute_v_mag": 4.825,
            "colour_b_v_mag": 0.65,
            "mass_loss_rate_msun_per_yr": 2.5e-14,
            "remnant": null,
            "death_time": null,
        })
    }

    /// A cool white dwarf primary, star 0, from a 2.5 M☉ star dead some 4 Gyr: the heaviest star
    /// at birth, as body 0 always is.
    fn white_dwarf() -> StarSummaryDto {
        StarSummaryDto {
            body_index: 0,
            kind: ObjectKindDto::WhiteDwarf,
            phase: PhaseDto::CarbonOxygenWhiteDwarf,
            class: "DA9.2".to_owned(),
            initial_mass_msun: 2.5,
            mass_msun: 0.687_5,
            core_mass_msun: 0.687_5,
            luminosity_lsun: 0.000_107_5,
            radius_rsun: 0.011_5,
            teff_k: Some(5_480.0),
            absolute_v_mag: None,
            colour_b_v_mag: None,
            mass_loss_rate_msun_per_yr: 0.0,
            remnant: Some(RemnantDto::WhiteDwarf {
                cooling_age_myr: 3_950.5,
                natal_kick: None,
            }),
            death_time: None,
            rotation_period_d: Modelled::NotModelled,
            activity_log_lx_lbol: Modelled::NotModelled,
            variability: Modelled::NotModelled,
            planetary_nebula: Modelled::NotModelled,
            active_events: None,
        }
    }

    fn white_dwarf_json() -> Value {
        json!({
            "body_index": 0,
            "kind": "white_dwarf",
            "phase": "carbon_oxygen_white_dwarf",
            "class": "DA9.2",
            "initial_mass_msun": 2.5,
            "mass_msun": 0.687_5,
            "core_mass_msun": 0.687_5,
            "luminosity_lsun": 0.000_107_5,
            "radius_rsun": 0.011_5,
            "teff_k": 5_480.0,
            "absolute_v_mag": null,
            "colour_b_v_mag": null,
            "mass_loss_rate_msun_per_yr": 0.0,
            "remnant": { "type": "white_dwarf", "cooling_age_myr": 3_950.5 },
            "death_time": null,
        })
    }

    /// The Sun-like companion, star 1.
    fn companion() -> StarSummaryDto {
        StarSummaryDto {
            body_index: 1,
            ..sunlike()
        }
    }

    fn companion_json() -> Value {
        let mut wire = sunlike_json();
        wire["body_index"] = json!(1);
        wire
    }

    /// The pair's orbit, a = 23.5 au with μ for the stars' 3.5 M☉ at birth and the period Kepler's
    /// third law gives them, 60.9 yr.
    fn pair_orbit() -> OrbitDto {
        OrbitDto {
            period_s: 1_921_736_528.404_686,
            semi_major_axis_m: 3_515_625_000_000.0,
            eccentricity: 0.5,
            inclination_rad: 1.5,
            ascending_node_rad: 3.5,
            argument_of_periapsis_rad: 4.25,
            mean_anomaly_at_epoch_rad: 0.125,
            mu_m3_s2: 4.644_935_401_444_779_6e20,
        }
    }

    fn pair_orbit_json() -> Value {
        json!({
            "period_s": 1_921_736_528.404_686,
            "semi_major_axis_m": 3_515_625_000_000.0,
            "eccentricity": 0.5,
            "inclination_rad": 1.5,
            "ascending_node_rad": 3.5,
            "argument_of_periapsis_rad": 4.25,
            "mean_anomaly_at_epoch_rad": 0.125,
            "mu_m3_s2": 4.644_935_401_444_779_6e20,
        })
    }

    /// A white dwarf with a Sun-like companion, as Sirius is but older.
    fn summary() -> SystemSummaryDto {
        SystemSummaryDto {
            universe: UniverseIdHex::from_u64(UNIVERSE),
            system: SystemIdHex::from_u64(SYSTEM),
            time: epoch_plus_a_century(),
            existence: SystemExistenceDto::Exists,
            age_myr: 4_600.5,
            fe_h_dex: -0.125,
            stars: vec![white_dwarf(), companion()],
            hierarchy: HierarchyDto {
                nodes: vec![
                    HierarchyNodeDto::Pair {
                        inner: 1,
                        outer: 2,
                        orbit: pair_orbit(),
                    },
                    HierarchyNodeDto::Star {
                        body_index: 0,
                        mass_msun: 2.5,
                    },
                    HierarchyNodeDto::Star {
                        body_index: 1,
                        mass_msun: 1.0,
                    },
                ],
            },
        }
    }

    fn summary_json() -> Value {
        json!({
            "universe": "0123456789abcdef",
            "system": "0200080020000000",
            "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
            "existence": "exists",
            "age_myr": 4_600.5,
            "fe_h_dex": -0.125,
            "stars": [white_dwarf_json(), companion_json()],
            "hierarchy": {
                "nodes": [
                    { "type": "pair", "inner": 1, "outer": 2, "orbit": pair_orbit_json() },
                    { "type": "star", "body_index": 0, "mass_msun": 2.5 },
                    { "type": "star", "body_index": 1, "mass_msun": 1.0 },
                ],
            },
        })
    }

    #[test]
    fn system_summary_request_wire_form() {
        assert_wire_form(
            &ClientMessage::Request {
                id: RequestId(12),
                body: RequestBody::SystemSummary(request()),
            },
            json!({
                "type": "request",
                "id": 12,
                "body": {
                    "kind": "system_summary",
                    "universe": "0123456789abcdef",
                    "system": "0200080020000000",
                    "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                },
            }),
        );
    }

    #[test]
    fn system_summary_request_refuses_a_malformed_system_id() {
        let error = serde_json::from_value::<SystemSummaryRequest>(json!({
            "universe": "0123456789abcdef",
            "system": "200080020000000",
            "time": { "seconds": 0, "nanos": 0 },
        }))
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("expected 16 lowercase hexadecimal digits"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn system_summary_response_wire_form() {
        let mut body = summary_json();
        body["kind"] = json!("system_summary");
        assert_wire_form(
            &ServerMessage::Response {
                id: RequestId(12),
                body: ResponseBody::SystemSummary(summary()),
            },
            json!({ "type": "response", "id": 12, "body": body }),
        );
    }

    #[test]
    fn a_system_not_yet_born_has_no_stars() {
        assert_wire_form(
            &SystemSummaryDto {
                existence: SystemExistenceDto::NotYetBorn,
                age_myr: -0.000_5,
                stars: Vec::new(),
                hierarchy: HierarchyDto { nodes: Vec::new() },
                ..summary()
            },
            json!({
                "universe": "0123456789abcdef",
                "system": "0200080020000000",
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "existence": "not_yet_born",
                "age_myr": -0.000_5,
                "fe_h_dex": -0.125,
                "stars": [],
                "hierarchy": { "nodes": [] },
            }),
        );
    }

    #[test]
    fn unknown_system_request_error_wire_form() {
        assert_wire_form(
            &ServerMessage::RequestError {
                id: RequestId(12),
                error: RequestError {
                    code: ErrorCode::UnknownSystem,
                    message: "no system 0200080020000000 in this universe".to_owned(),
                    field: Some("system".to_owned()),
                },
            },
            json!({
                "type": "request_error",
                "id": 12,
                "error": {
                    "code": "unknown_system",
                    "message": "no system 0200080020000000 in this universe",
                    "field": "system",
                },
            }),
        );
    }

    #[test]
    fn a_living_star_leaves_every_uncomputed_field_absent() {
        let wire = serde_json::to_value(sunlike()).unwrap();
        for absent in [
            "rotation_period_d",
            "activity_log_lx_lbol",
            "variability",
            "planetary_nebula",
            "active_events",
        ] {
            assert_eq!(wire.get(absent), None, "{absent} is on the wire");
        }
        assert_eq!(sunlike_json(), wire);
    }

    #[test]
    fn a_star_with_every_later_field_filled() {
        // What plan 06's T16 and T25–T28 add once they land: every field present, one of them
        // null because the star has no nebula.
        let star = StarSummaryDto {
            rotation_period_d: Modelled::Value(25.375),
            activity_log_lx_lbol: Modelled::Value(-6.25),
            variability: Modelled::Value(VariabilityDto {
                kind: VariableKindDto::ByDraconis,
                period_d: 25.375,
                amplitude_mag: 0.001_5,
            }),
            planetary_nebula: Modelled::Null,
            active_events: Some(vec![StarEventDto {
                kind: StarEventKindDto::Flare,
                onset: UniverseTime {
                    seconds: 3_155_759_400,
                    nanos: 250_000_000,
                },
                duration_s: 900.0,
            }]),
            ..sunlike()
        };
        let mut wire = sunlike_json();
        wire["rotation_period_d"] = json!(25.375);
        wire["activity_log_lx_lbol"] = json!(-6.25);
        wire["variability"] = json!({
            "kind": "by_draconis",
            "period_d": 25.375,
            "amplitude_mag": 0.001_5,
        });
        wire["planetary_nebula"] = Value::Null;
        wire["active_events"] = json!([{
            "kind": "flare",
            "onset": { "seconds": 3_155_759_400_i64, "nanos": 250_000_000 },
            "duration_s": 900.0,
        }]);
        assert_wire_form(&star, wire);
    }

    #[test]
    fn a_later_field_tells_absent_from_null() {
        // Absent: not computed. Null: computed, and the star has none. The two must not collapse
        // into one another on the way in.
        let mut wire = sunlike_json();
        let absent: StarSummaryDto = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(absent.variability, Modelled::NotModelled);
        assert_eq!(absent.rotation_period_d, Modelled::NotModelled);
        wire["variability"] = Value::Null;
        wire["rotation_period_d"] = Value::Null;
        let null: StarSummaryDto = serde_json::from_value(wire).unwrap();
        assert_eq!(null.variability, Modelled::Null);
        assert_eq!(null.rotation_period_d, Modelled::Null);
        assert_eq!(
            serde_json::to_value(&null).unwrap()["variability"],
            Value::Null
        );
    }

    #[test]
    fn remnant_wire_forms() {
        assert_wire_form(
            &RemnantDto::NeutronStar {
                pulsar: None,
                natal_kick: None,
            },
            json!({ "type": "neutron_star" }),
        );
        assert_wire_form(
            &RemnantDto::NeutronStar {
                pulsar: Some(PulsarDto {
                    spin_period_s: 0.089_25,
                    period_derivative_s_per_s: 1.25e-13,
                    magnetic_field_g: 3.375e12,
                    alive: true,
                    magnetar: false,
                }),
                natal_kick: Some(NatalKickDto {
                    speed_km_s: 265.5,
                    mode: KickModeDto::Ordinary,
                }),
            },
            json!({
                "type": "neutron_star",
                "pulsar": {
                    "spin_period_s": 0.089_25,
                    "period_derivative_s_per_s": 1.25e-13,
                    "magnetic_field_g": 3.375e12,
                    "alive": true,
                    "magnetar": false,
                },
                "natal_kick": { "speed_km_s": 265.5, "mode": "ordinary" },
            }),
        );
        assert_wire_form(
            &RemnantDto::BlackHole {
                dimensionless_spin: None,
                natal_kick: None,
            },
            json!({ "type": "black_hole" }),
        );
        assert_wire_form(
            &RemnantDto::BlackHole {
                dimensionless_spin: Some(0.062_5),
                natal_kick: Some(NatalKickDto {
                    speed_km_s: 0.0,
                    mode: KickModeDto::FallbackNone,
                }),
            },
            json!({
                "type": "black_hole",
                "dimensionless_spin": 0.062_5,
                "natal_kick": { "speed_km_s": 0.0, "mode": "fallback_none" },
            }),
        );
        assert_wire_form(
            &RemnantDto::WhiteDwarf {
                cooling_age_myr: 12.5,
                natal_kick: Some(NatalKickDto {
                    speed_km_s: 0.75,
                    mode: KickModeDto::WhiteDwarf,
                }),
            },
            json!({
                "type": "white_dwarf",
                "cooling_age_myr": 12.5,
                "natal_kick": { "speed_km_s": 0.75, "mode": "white_dwarf" },
            }),
        );
        assert_wire_form(&RemnantDto::NoRemnant, json!({ "type": "no_remnant" }));
    }

    #[test]
    fn a_black_hole_has_no_light_and_no_colour() {
        let black_hole = StarSummaryDto {
            body_index: 0,
            kind: ObjectKindDto::BlackHole,
            phase: PhaseDto::BlackHole,
            class: "BH".to_owned(),
            initial_mass_msun: 40.0,
            mass_msun: 12.5,
            core_mass_msun: 12.5,
            luminosity_lsun: 0.0,
            radius_rsun: 5.3e-5,
            teff_k: None,
            absolute_v_mag: None,
            colour_b_v_mag: None,
            mass_loss_rate_msun_per_yr: 0.0,
            remnant: Some(RemnantDto::BlackHole {
                dimensionless_spin: None,
                natal_kick: None,
            }),
            death_time: Some(UniverseTime {
                seconds: -20_000_000_000,
                nanos: 0,
            }),
            ..sunlike()
        };
        assert_wire_form(
            &black_hole,
            json!({
                "body_index": 0,
                "kind": "black_hole",
                "phase": "black_hole",
                "class": "BH",
                "initial_mass_msun": 40.0,
                "mass_msun": 12.5,
                "core_mass_msun": 12.5,
                "luminosity_lsun": 0.0,
                "radius_rsun": 5.3e-5,
                "teff_k": null,
                "absolute_v_mag": null,
                "colour_b_v_mag": null,
                "mass_loss_rate_msun_per_yr": 0.0,
                "remnant": { "type": "black_hole" },
                "death_time": { "seconds": -20_000_000_000_i64, "nanos": 0 },
            }),
        );
    }

    #[test]
    fn planetary_nebula_wire_form() {
        assert_wire_form(
            &PlanetaryNebulaDto {
                radius_ly: 0.625,
                expansion_speed_km_s: 25.5,
                age_yr: 7_500.0,
                ionised_mass_msun: 0.187_5,
                excitation_class: 6,
            },
            json!({
                "radius_ly": 0.625,
                "expansion_speed_km_s": 25.5,
                "age_yr": 7_500.0,
                "ionised_mass_msun": 0.187_5,
                "excitation_class": 6,
            }),
        );
    }

    #[test]
    fn stellar_brief_wire_forms() {
        assert_wire_form(
            &StellarBriefDto {
                kind: ObjectKindDto::Giant,
                class: "K1.5III".to_owned(),
                log_luminosity_lsun: Some(2.125),
                teff_k: Some(4_286.0),
                star_count: 2,
            },
            json!({
                "kind": "giant",
                "class": "K1.5III",
                "log_luminosity_lsun": 2.125,
                "teff_k": 4_286.0,
                "star_count": 2,
            }),
        );
        assert_wire_form(
            &StellarBriefDto {
                kind: ObjectKindDto::BlackHole,
                class: "BH".to_owned(),
                log_luminosity_lsun: None,
                teff_k: None,
                star_count: 1,
            },
            json!({
                "kind": "black_hole",
                "class": "BH",
                "log_luminosity_lsun": null,
                "teff_k": null,
                "star_count": 1,
            }),
        );
    }

    #[test]
    fn object_kind_strings() {
        assert_wire_strings(&[
            (ObjectKindDto::Protostar, "protostar"),
            (ObjectKindDto::PreMainSequence, "pre_main_sequence"),
            (ObjectKindDto::Dwarf, "dwarf"),
            (ObjectKindDto::Subgiant, "subgiant"),
            (ObjectKindDto::Giant, "giant"),
            (ObjectKindDto::Supergiant, "supergiant"),
            (ObjectKindDto::WolfRayet, "wolf_rayet"),
            (ObjectKindDto::HotSubdwarf, "hot_subdwarf"),
            (ObjectKindDto::WhiteDwarf, "white_dwarf"),
            (ObjectKindDto::NeutronStar, "neutron_star"),
            (ObjectKindDto::BlackHole, "black_hole"),
            (ObjectKindDto::NoRemnant, "no_remnant"),
            (ObjectKindDto::Substellar, "substellar"),
        ]);
    }

    #[test]
    fn phase_strings() {
        assert_wire_strings(&[
            (PhaseDto::Protostar, "protostar"),
            (PhaseDto::PreMainSequence, "pre_main_sequence"),
            (PhaseDto::MainSequence, "main_sequence"),
            (PhaseDto::HertzsprungGap, "hertzsprung_gap"),
            (PhaseDto::FirstGiantBranch, "first_giant_branch"),
            (PhaseDto::CoreHeliumBurning, "core_helium_burning"),
            (PhaseDto::EarlyAgb, "early_agb"),
            (PhaseDto::ThermallyPulsingAgb, "thermally_pulsing_agb"),
            (PhaseDto::HeliumMainSequence, "helium_main_sequence"),
            (PhaseDto::HeliumHertzsprungGap, "helium_hertzsprung_gap"),
            (PhaseDto::HeliumGiantBranch, "helium_giant_branch"),
            (PhaseDto::PostAgb, "post_agb"),
            (PhaseDto::HeliumWhiteDwarf, "helium_white_dwarf"),
            (
                PhaseDto::CarbonOxygenWhiteDwarf,
                "carbon_oxygen_white_dwarf",
            ),
            (PhaseDto::OxygenNeonWhiteDwarf, "oxygen_neon_white_dwarf"),
            (PhaseDto::NeutronStar, "neutron_star"),
            (PhaseDto::BlackHole, "black_hole"),
            (PhaseDto::NoRemnant, "no_remnant"),
            (PhaseDto::Substellar, "substellar"),
        ]);
    }

    #[test]
    fn system_existence_strings() {
        assert_wire_strings(&[
            (SystemExistenceDto::NotYetBorn, "not_yet_born"),
            (SystemExistenceDto::Exists, "exists"),
        ]);
    }

    #[test]
    fn kick_mode_strings() {
        assert_wire_strings(&[
            (KickModeDto::Ordinary, "ordinary"),
            (KickModeDto::Low, "low"),
            (KickModeDto::FallbackNone, "fallback_none"),
            (KickModeDto::WhiteDwarf, "white_dwarf"),
        ]);
    }

    #[test]
    fn variable_kind_strings() {
        assert_wire_strings(&[
            (VariableKindDto::DeltaScuti, "delta_scuti"),
            (VariableKindDto::RrLyrae, "rr_lyrae"),
            (VariableKindDto::ClassicalCepheid, "classical_cepheid"),
            (VariableKindDto::BlHerculis, "bl_herculis"),
            (VariableKindDto::WVirginis, "w_virginis"),
            (VariableKindDto::RvTauri, "rv_tauri"),
            (VariableKindDto::Mira, "mira"),
            (VariableKindDto::SemiregularA, "semiregular_a"),
            (VariableKindDto::SemiregularB, "semiregular_b"),
            (VariableKindDto::SemiregularC, "semiregular_c"),
            (VariableKindDto::SlowIrregular, "slow_irregular"),
            (VariableKindDto::BetaCephei, "beta_cephei"),
            (VariableKindDto::SlowlyPulsatingB, "slowly_pulsating_b"),
            (VariableKindDto::GammaDoradus, "gamma_doradus"),
            (VariableKindDto::ZzCeti, "zz_ceti"),
            (VariableKindDto::V777Herculis, "v777_herculis"),
            (VariableKindDto::GwVirginis, "gw_virginis"),
            (VariableKindDto::AlphaCygni, "alpha_cygni"),
            (VariableKindDto::SDoradus, "s_doradus"),
            (VariableKindDto::ByDraconis, "by_draconis"),
            (
                VariableKindDto::Alpha2CanumVenaticorum,
                "alpha2_canum_venaticorum",
            ),
        ]);
    }

    #[test]
    fn star_event_kind_strings() {
        assert_wire_strings(&[
            (StarEventKindDto::Flare, "flare"),
            (StarEventKindDto::Glitch, "glitch"),
            (StarEventKindDto::MagnetarBurst, "magnetar_burst"),
            (StarEventKindDto::MagnetarGiantFlare, "magnetar_giant_flare"),
            (StarEventKindDto::FuOrionisOutburst, "fu_orionis_outburst"),
            (StarEventKindDto::GiantEruption, "giant_eruption"),
            (StarEventKindDto::ThermalPulse, "thermal_pulse"),
        ]);
    }
}
