//! A body's record on the wire: what it is, its state, and its sections (plan 14, P14.T35.b,
//! mirroring the simulation's `planetary::record::BodyRecord` as ruling 53 of 2026-09-22 has it).
//!
//! A record is a prefix of sections by detail level (design note 16):
//!
//! | Level | Sections |
//! | ----- | -------- |
//! | `contact` | none: the ID, the parent, the state and the position only, with the kind `unresolved` |
//! | `mass_and_orbit` | `label`, `mass_kg`, `orbit`, `moons`, `rings`, and the kind |
//! | `bulk` | `bulk`: radius, density, surface gravity, class, mass fractions, equilibrium temperature; `rotation`, `figure`, `photometry` |
//! | `surface` | `surface` |
//! | `full` | `hooks` |
//!
//! A system's list carries each body as a [`BodySummaryDto`], and `body_detail` answers with a
//! [`BodyDetailDto`] holding its whole [`BodyRecordDto`]. Every section is a [`SectionDto`], so
//! that a section above the granted level reads `not_resolved` and one this generator version does
//! not compute reads `not_modelled`, never an empty value. Every quantity's field name carries its
//! SI unit.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::planetary::{BodyOrbitDto, DetailLevelDto, OrbitHostDto, PopulationDto, SectionDto};
use crate::primitives::{BodyIdHex, DetailSeedHex, UniverseIdHex, UniverseTime};

/// What a body is (plan 14's `BodyKind`), with every variant from the start, so that a later task
/// fills a variant and never changes the type's shape.
///
/// A planet's class by composition is not its kind but its bulk section's `class`, since design
/// note 16 puts the class at the `bulk` level and the kind is shown from `mass_and_orbit`. Tagged
/// by `type`: `{"type": "planet"}`, `{"type": "moon", "origin": "captured"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum BodyKindDto {
    /// A planet, bound to a host or free-floating.
    Planet,
    /// A belt's largest member, derived as a dwarf planet (P14.T21.c).
    DwarfPlanet,
    /// A moon (P14.T17–T19).
    Moon {
        /// How it came to orbit its planet.
        origin: MoonOriginDto,
    },
    /// A ring system (P14.T20).
    Ring,
    /// A belt's population as a whole (P14.T21).
    Belt {
        /// Which kind of belt it is.
        belt_kind: BeltKindDto,
    },
    /// A cometary halo, a statistical population (P14.T21.d).
    CometaryHalo,
    /// A protoplanetary disc, before its lifetime ends (P14.T28.a).
    ProtoplanetaryDisc,
    /// A white dwarf's dusty debris disc (P14.T28.d).
    DebrisDisc,
    /// An unresolved contact, whose kind the granted detail level, `contact`, withholds.
    Unresolved,
}

/// How a moon came to orbit its planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MoonOriginDto {
    /// Formed in the planet's circumplanetary disc (P14.T17).
    Regular,
    /// Formed from a giant impact, as Earth's Moon and Charon (P14.T18).
    GiantImpact,
    /// Captured, on a distant, eccentric and often retrograde orbit (P14.T19).
    Captured,
}

/// Which kind of belt a population is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum BeltKindDto {
    /// An asteroid belt, inside the innermost giant or in a wide gap between planets.
    Asteroid,
    /// A Kuiper-like belt outside the outermost planet, with its scattered component.
    Kuiper,
}

/// What has become of a body at the record's time (plan 14's `fate::BodyState`), with every
/// variant from the start.
///
/// The states run in one order, not yet formed, present, then destroyed or unbound, and a body
/// that is not present has no position. Tagged by `type`: `{"type": "present"}`,
/// `{"type": "destroyed", "cause": "engulfed", "at": …}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum BodyStateDto {
    /// The body has not formed yet: a giant before its formation age, a small planet before its
    /// disc's lifetime ends, or any body of a system not yet born. The display reads
    /// `NOT YET FORMED` (ruling 34 of 2026-09-22).
    NotYetFormed,
    /// The body exists and orbits as its elements say.
    Present,
    /// The body was destroyed.
    Destroyed {
        /// What destroyed it.
        cause: DestructionCauseDto,
        /// When. It may lie beyond the ±(2⁵³ − 1) s that a JavaScript number holds exactly, as a
        /// disc dispersed billions of years ago does, and is for display only (P14.T35.d).
        at: UniverseTime,
    },
    /// The body was unbound from its system, and is no longer tracked.
    Unbound {
        /// When. It may lie beyond the ±(2⁵³ − 1) s that a JavaScript number holds exactly, as an
        /// unbinding billions of years ago does, and is for display only (P14.T35.d).
        at: UniverseTime,
    },
}

/// What destroyed a body.
///
/// A moon destroyed with its planet takes its planet's cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DestructionCauseDto {
    /// A protoplanetary disc dispersed at the end of its lifetime (P14.T28.a).
    Dispersed,
    /// Its host expanded over its orbit (P14.T28.b).
    Engulfed,
    /// Its pericentre fell inside its primary's Roche limit (P14.T28.c).
    TidallyDisrupted,
    /// It collided with a heavier neighbour whose orbit its own crossed after a supernova, and
    /// merged into it (P14.T28.c, ruling 71).
    Collided,
}

/// A planet's class by composition (plan 14, P14.T16.a's `PlanetClass`).
///
/// The thresholds are the generator's, by ruling 53 of 2026-09-22: an envelope of 0.1% of the mass
/// or more makes a sub-Neptune, or an ice giant from 10 Earth masses, and a gas giant from half the
/// mass; without an envelope, water from 10% makes a body icy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PlanetClassDto {
    /// Iron and rock, with no more than a trace of water or hydrogen: Mercury to Earth, and dry
    /// super-Earths.
    Rocky,
    /// Rock and iron under 10% or more of water, with no more than a trace of hydrogen: water
    /// worlds, and icy moons such as Ganymede.
    Icy,
    /// A core under a hydrogen and helium envelope of at least 0.1% of the mass, lighter than 10
    /// Earth masses.
    SubNeptune,
    /// A core of 10 Earth masses or more under an envelope of less than half the mass: Uranus
    /// and Neptune.
    IceGiant,
    /// A body mostly of hydrogen and helium: Saturn and Jupiter.
    GasGiant,
}

/// A body's mass fractions of iron, rock, water and hydrogen and helium envelope, each in `[0, 1]`
/// and summing to 1 to the rounding of the server's arithmetic.
///
/// They are fixed when the body forms, and are the bulk section's (`bulk` level); the volatile
/// inventory and the host's abundances are the hooks' bulk composition (`full`, P14.T23).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MassFractionsDto {
    /// The fraction of the mass that is iron, the core's.
    pub iron: f64,
    /// The fraction of the mass that is silicate rock.
    pub rock: f64,
    /// The fraction of the mass that is water, as ice or liquid.
    pub water: f64,
    /// The fraction of the mass in a hydrogen and helium envelope.
    pub envelope: f64,
}

/// A body's bulk section: what its derivation gives, less its mass, which the `mass_and_orbit`
/// level shows in a section of its own (ruling 53 of 2026-09-22).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BulkPropertiesDto {
    /// The mean radius at the record's time, in metres, positive: a giant's contracts as it
    /// cools, and an envelope's shrinks as it escapes.
    pub radius_m: f64,
    /// The mean density, in kg m⁻³, positive.
    pub density_kg_m3: f64,
    /// The gravitational acceleration at the mean radius, in m s⁻², positive.
    pub surface_gravity_m_s2: f64,
    /// The class by composition.
    pub class: PlanetClassDto,
    /// The mass fractions.
    pub mass_fractions: MassFractionsDto,
    /// The equilibrium temperature at the record's time, in kelvin, not negative: from its hosts'
    /// light alone, at the Bond albedo its surface state gives, with no internal heat (ruling
    /// 112.7).
    pub equilibrium_temperature_k: f64,
    /// The effective temperature at the record's time, in kelvin, positive, of a body with a
    /// luminosity of its own, a giant's: its internal heat added to its equilibrium temperature,
    /// T⁴ = `T_eq`⁴ + `L_int` ÷ (4πR²σ) (ruling 112.7). `null` for a body with none, whose
    /// effective temperature is its equilibrium one.
    pub effective_temperature_k: Option<f64>,
}

/// A body's surface section: atmosphere and surface conditions (design note 16), which P14.T13
/// and T24 compute; the rotation and the figure are sections of their own at `bulk` (P14.T46.f).
///
/// None of them is on the wire yet, so the type has no value, as the simulation's `record::Surface`
/// has none, and no record can carry a surface section that is `ok`: every surface is
/// `not_modelled`, or `not_applicable` for a giant. The tasks that compute it give it its fields.
/// TypeScript sees it as `never`. It derives only what a type of floats and lists can keep once it
/// has its fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum BodySurfaceDto {}

/// A body's hooks section, what the generators of surfaces, life and civilisations read (plan
/// 14's `hooks::BodyHooks`): its detail seed, and with P14.T23–T26 its bulk composition,
/// habitability and resources.
///
/// No hook is on the wire yet, so every hooks section is `not_modelled`. The body's surface seed
/// (P14.T23) is never sent: a client holding it could synthesise a surface the ship has not seen,
/// so the server keeps it, and the client gets the detail seed instead (plan 14 as amended by the
/// rendering plans' R04.T3.a; R04, Design note 17). The detail seed is a section of its own
/// because the hooks section becomes `ok` with the bulk composition while the detail seed waits for
/// R09 to derive it; the other hooks join it as the tasks that compute them land.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyHooksDto {
    /// The seed of the client's local terrain synthesis, derived on `body.surface.detail` (R09);
    /// `not_modelled` until R09 computes it.
    pub detail_seed: SectionDto<DetailSeedHex>,
}

/// The spin–orbit state a body's tides lock it into (plan 14's `SpinOrbitResonance`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SpinResonanceDto {
    /// One rotation per orbit, as the Moon.
    Synchronous,
    /// Three rotations per two orbits, as Mercury.
    ThreeToTwo,
}

/// A body's body-fixed frame and its rotation law (plan 14, P14.T14.c and P14.T46.f; design note
/// 23): the pole and the equator's axes, and every parameter of the rotation angle W(t), so that
/// a client evaluates the orientation at any time in the clock window with no re-request.
///
/// With s the seconds from the epoch to t, d = `locking_age_s` − `age_at_epoch_s` and
/// Δ = max(s, 0), W is reduced into `[0, 2π)` and is:
///
/// - **locked**, at or after `locks_at`: `sub_primary_angle_rad` + p M(t), with p 1 for a
///   synchronous body and 3 for a 3:2 one, and M(t) = `clock_mean_anomaly_at_epoch_rad` +
///   2π s ÷ `clock_period_s`;
/// - **with no lock in the clock's range** (`locks_at` `null`): `phase_at_epoch_rad` + the angle
///   swept from 0 to s;
/// - **before the lock**, d > 0: `phase_at_epoch_rad` + the angle swept from 0 to s +
///   `capture_phase_rad` (Δ ÷ d)²;
/// - **before the lock**, d ≤ 0: the locked angle at `locks_at` less the angle swept from s to d.
///
/// The angle swept from a to b is ω(`age_at_epoch_s` + a) (b − a) + (`ω_L` − ω₀) (b − a)² ÷ 2τ, with
/// ω(x) = ω₀ + (`ω_L` − ω₀) clamp(x ÷ τ, 0, 1), ω₀ the initial and `ω_L` the locked rate and τ the
/// locking age; ω₀ (b − a) for a body that never locks. The body-fixed axes at t are the prime
/// meridian cos W node + sin W quarter, the axis a quarter-turn east of it, and the pole.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyRotationDto {
    /// The pole, the spin axis's unit vector along the galactic axes, by the right-hand rule.
    pub pole: [f64; 3],
    /// The ascending node of the equator on the orbital plane, a unit vector along the galactic
    /// axes, which W is measured from about the pole.
    pub equator_node: [f64; 3],
    /// The equator's axis a quarter-turn east of the node, pole × node, a unit vector along the
    /// galactic axes: where W is π ÷ 2.
    pub equator_quarter: [f64; 3],
    /// The obliquity, rad, in `[0, π]`: the angle between the pole and the orbit's normal.
    pub obliquity_rad: f64,
    /// The primordial spin rate ω₀, rad s⁻¹, positive: prograde about the pole.
    pub initial_rate_rad_s: f64,
    /// The spin rate after the lock, `ω_L`, rad s⁻¹, positive: the mean motion n, or 1.5 n in the
    /// 3:2 state.
    pub locked_rate_rad_s: f64,
    /// The system's age at the epoch, s, positive.
    pub age_at_epoch_s: f64,
    /// The system age at which the body locks, τ, s from the system's birth, positive; `null` for
    /// a body that never locks. Never `null` where `locks_at` is set.
    pub locking_age_s: Option<f64>,
    /// When the body locks; `null` when that is not within the clock's range.
    pub locks_at: Option<UniverseTime>,
    /// The state the body locks into.
    pub resonance: SpinResonanceDto,
    /// The period of the locked angle's clock, s, positive: the orbit's for a synchronous body,
    /// two orbits for a 3:2 one.
    pub clock_period_s: f64,
    /// The clock's mean anomaly at the epoch, rad, in `[0, 2π)`.
    pub clock_mean_anomaly_at_epoch_rad: f64,
    /// `W_p`, the rotation angle at which the prime meridian faces the primary at pericentre, rad,
    /// in `[−π, π]`: measured as W is, from the equator's node about the pole.
    pub sub_primary_angle_rad: f64,
    /// The drawn rotation angle at the epoch, rad, in `[0, 2π)`.
    pub phase_at_epoch_rad: f64,
    /// δ, the phase the capture into the resonance takes up before the lock, rad, in `[−π, π)`.
    pub capture_phase_rad: f64,
}

/// How a body's figure was found (plan 14's `FigureLaw`, P14.T46.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FigureLawDto {
    /// Below the hydrostatic size: a sphere of the mean radius.
    Sphere,
    /// The spin's Darwin–Radau flattening.
    Rotational,
    /// A synchronous body's: the spin's flattening times the synchronous tidal factor, 2.5.
    RotationalAndTidal,
    /// Held at the flattening cap, 0.2.
    Capped,
}

/// What a body's heights are measured from (plan 14's `Datum`, P14.T46.e).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FigureDatumDto {
    /// The solid or liquid surface.
    SolidSurface,
    /// The 1-bar level of a hydrogen and helium envelope, which its radius is quoted at.
    OneBar,
}

/// A body's figure (plan 14, P14.T46.d–f): the reference spheroid (a, a, c) about its pole that
/// every height is measured along the normal of, of the volume of the bulk section's mean radius.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyFigureDto {
    /// The equatorial radius a, m, positive.
    pub equatorial_radius_m: f64,
    /// The polar radius c, m, positive, at most a.
    pub polar_radius_m: f64,
    /// The flattening (a − c) ÷ a, in `[0, 0.2]`.
    pub flattening: f64,
    /// The rotation's pole, the spheroid's symmetry axis, a unit vector along the galactic axes.
    pub pole: [f64; 3],
    /// The moment of inertia factor C ÷ M a² the flattening was found with, in `(0, 0.4]` (0.4 a
    /// uniform sphere).
    pub moment_of_inertia_factor: f64,
    /// How the figure was found.
    pub law: FigureLawDto,
    /// What heights are measured from.
    pub datum: FigureDatumDto,
}

/// A quantity in the Johnson B, V and R bands.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BandsDto {
    /// B.
    pub b: f64,
    /// V.
    pub v: f64,
    /// R.
    pub r: f64,
}

/// Which measured phase curve a body's law is fitted to (plan 14's `PhaseTemplate`, P14.T47.a;
/// rendering plan R07's `PhaseTemplateId`, in its order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PhaseTemplateDto {
    /// The Moon's curve (a test analogue: airless rock takes Mercury's).
    Moon,
    /// Mercury's.
    Mercury,
    /// Mars's.
    Mars,
    /// Venus's.
    Venus,
    /// Earth's.
    Earth,
    /// Jupiter's.
    Jupiter,
    /// Saturn's globe.
    Saturn,
    /// Uranus's (a test analogue: an ice giant takes Neptune's).
    Uranus,
    /// Neptune's.
    Neptune,
    /// Airless ice: the Moon's curve, q from Ganymede's (provisional).
    AirlessIce,
    /// A snowball: the Moon's curve, q from Europa's (provisional).
    Snowball,
    /// A thin magma ocean: Mercury's curve (provisional).
    Magma,
}

/// A body's photometry (plan 14, P14.T47): the disc-integrated brightness law a client draws it
/// with, per band, `Φ_c` = `Φ_t`^s blended with the Lommel–Seeliger share L, against π a c.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyPhotometryDto {
    /// The geometric albedo p per band, against π a c, positive, with p q ≤ 1.
    pub geometric_albedo: BandsDto,
    /// The phase-curve template.
    pub phase_template: PhaseTemplateDto,
    /// The template's exponent s per band, positive.
    pub phase_exponent: BandsDto,
    /// The law's Lommel–Seeliger share L, in `[0, 1]`.
    pub lunar_lambert_share: f64,
    /// The Bond albedo of the body's surface state, in `[0, 1)` (P14.T13.c).
    pub bond_albedo: f64,
    /// `p_V` `q_V` ÷ `A_Bond`, a check only, positive: 1 for a law whose spherical albedo is the
    /// Bond albedo.
    pub bond_ratio: f64,
    /// Whether the curve is borrowed (magma, airless ice, a snowball) or the body a hot giant with
    /// no Solar System analogue.
    pub provisional: bool,
}

/// One body as a system's list carries it: its identity and state, and the sections the list and
/// the orbit map read (plan 14, P14.T35.b).
///
/// `id`, `kind`, `parent`, `state` and `position_m` are present at every detail level; every other
/// field is a section, tagged with its state. The system's bodies are listed flat, in index order,
/// and the tree is rebuilt from `parent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodySummaryDto {
    /// The body's ID.
    pub id: BodyIdHex,
    /// What it is; `unresolved` at the `contact` level.
    pub kind: BodyKindDto,
    /// Its label for people (design note 22): host letter, planets lettered from `b` by
    /// semi-major axis, moons in Roman numerals, belts numbered, such as `A b` or `A d II`, never
    /// empty. `not_modelled` until P14.T30.c labels systems; its designation of record is always
    /// its system's designation and its body index.
    pub label: SectionDto<String>,
    /// What it orbits (ruling 53 of 2026-09-22), the same host as its orbit's `parent`: a star, a
    /// pair or the system's barycentre for a planet, its planet for a moon or a ring, its belt for
    /// a belt's member; `null` only for a system's root body, a free-floating object.
    pub parent: Option<OrbitHostDto>,
    /// What has become of it at the record's time.
    pub state: BodyStateDto,
    /// Where it is at the record's time, in metres from the system's barycentre along the
    /// galactic axes (the system frame); `null` for a body that is not present, and for a
    /// population, which has no single position.
    pub position_m: Option<[f64; 3]>,
    /// Its mass, in kilograms, positive (`mass_and_orbit`).
    pub mass_kg: SectionDto<f64>,
    /// Its orbit about its parent (`mass_and_orbit`); `not_applicable` for what orbits nothing,
    /// such as a free-floating object.
    pub orbit: SectionDto<BodyOrbitDto>,
    /// Its moons, by ID in index order (`mass_and_orbit`); `ok` with an empty list for a body with
    /// none, and `not_applicable` for a moon, a ring or a population.
    pub moons: SectionDto<Vec<BodyIdHex>>,
    /// Its rings, by ID in index order (`mass_and_orbit`); `ok` with an empty list for a planet with
    /// none, `not_modelled` for a dwarf planet, and `not_applicable` for a moon, a ring or a
    /// population.
    pub rings: SectionDto<Vec<BodyIdHex>>,
    /// What a population body is and where it lies, a ring, a belt or the cometary halo
    /// (`mass_and_orbit`); `not_applicable` for every other body.
    pub population: SectionDto<PopulationDto>,
    /// Its bulk properties (`bulk`).
    pub bulk: SectionDto<BulkPropertiesDto>,
    /// Its body-fixed frame and rotation law (`bulk`): `not_applicable` for a ring, a belt or the
    /// halo. The server always sends it; it is optional only so that adding it left
    /// [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION) at 2, and a client reads it absent as an older
    /// server's `not_modelled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rotation: Option<SectionDto<BodyRotationDto>>,
    /// Its figure (`bulk`): `not_applicable` for a ring, a belt or the halo, and `not_modelled`
    /// where the bulk or the rotation is. Optional on the wire, as `rotation` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub figure: Option<SectionDto<BodyFigureDto>>,
    /// Its photometry (`bulk`): `not_applicable` for a ring, a belt or the halo, and
    /// `not_modelled` where the bulk is. Optional on the wire, as `rotation` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub photometry: Option<SectionDto<BodyPhotometryDto>>,
}

/// One body's whole record at one time: every section of a [`BodySummaryDto`], and the surface and
/// hooks sections (plan 14, P14.T35.b; the simulation's `BodyRecord`).
///
/// Its fields are a [`BodySummaryDto`]'s and two more, so that a client's code for a list entry
/// also reads a whole record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyRecordDto {
    /// The body's ID.
    pub id: BodyIdHex,
    /// What it is; `unresolved` at the `contact` level.
    pub kind: BodyKindDto,
    /// Its label for people, as [`BodySummaryDto::label`].
    pub label: SectionDto<String>,
    /// What it orbits, as [`BodySummaryDto::parent`].
    pub parent: Option<OrbitHostDto>,
    /// What has become of it at the record's time.
    pub state: BodyStateDto,
    /// Where it is at the record's time, in metres from the system's barycentre along the galactic
    /// axes; `null` for a body that is not present, and for a population.
    pub position_m: Option<[f64; 3]>,
    /// Its mass, in kilograms, positive (`mass_and_orbit`).
    pub mass_kg: SectionDto<f64>,
    /// Its orbit about its parent (`mass_and_orbit`); `not_applicable` for what orbits nothing,
    /// such as a free-floating object.
    pub orbit: SectionDto<BodyOrbitDto>,
    /// Its moons, by ID in index order (`mass_and_orbit`); `ok` with an empty list for a body with
    /// none, and `not_applicable` for a moon, a ring or a population.
    pub moons: SectionDto<Vec<BodyIdHex>>,
    /// Its rings, by ID in index order (`mass_and_orbit`); `ok` with an empty list for a planet with
    /// none, `not_modelled` for a dwarf planet, and `not_applicable` for a moon, a ring or a
    /// population.
    pub rings: SectionDto<Vec<BodyIdHex>>,
    /// What a population body is and where it lies, a ring, a belt or the cometary halo
    /// (`mass_and_orbit`); `not_applicable` for every other body.
    pub population: SectionDto<PopulationDto>,
    /// Its bulk properties (`bulk`).
    pub bulk: SectionDto<BulkPropertiesDto>,
    /// Its body-fixed frame and rotation law (`bulk`): `not_applicable` for a ring, a belt or the
    /// halo. The server always sends it; it is optional only so that adding it left
    /// [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION) at 2, and a client reads it absent as an older
    /// server's `not_modelled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rotation: Option<SectionDto<BodyRotationDto>>,
    /// Its figure (`bulk`): `not_applicable` for a ring, a belt or the halo, and `not_modelled`
    /// where the bulk or the rotation is. Optional on the wire, as `rotation` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub figure: Option<SectionDto<BodyFigureDto>>,
    /// Its photometry (`bulk`): `not_applicable` for a ring, a belt or the halo, and
    /// `not_modelled` where the bulk is. Optional on the wire, as `rotation` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub photometry: Option<SectionDto<BodyPhotometryDto>>,
    /// Its surface (`surface`): `not_applicable` for a giant, which has none, and `not_modelled`
    /// for every other body in this generator version.
    pub surface: SectionDto<BodySurfaceDto>,
    /// Its hooks (`full`): `not_modelled` for every body in this generator version.
    pub hooks: SectionDto<BodyHooksDto>,
}

/// The answer to `body_detail`: one body's whole record at one time, at the detail level the
/// server granted (plan 14, P14.T35.b–c).
///
/// `universe` and `time` echo the request, and `granted` may be below the level asked for; every
/// section of the record above it is `not_resolved`. The record is a field of its own, because a
/// response's `kind` is the request's (`body_detail`) and the record has a `kind` of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyDetailDto {
    /// The universe the body is in.
    pub universe: UniverseIdHex,
    /// The instant the record describes.
    pub time: UniverseTime,
    /// The detail level the record holds.
    pub granted: DetailLevelDto,
    /// The body's record.
    pub record: BodyRecordDto,
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::orbit::OrbitDto;
    use crate::testing::{assert_wire_form, assert_wire_strings};

    pub(crate) const SYSTEM: u64 = 0x0200_0800_2000_0000;

    /// An Earth's orbit about a Sun: 1 au, e = 0.0167, in a system plane inclined 1 rad to the
    /// galactic plane with its node at 2.5 rad, and μ = GM☉ + GM⊕, whose period Kepler's third law
    /// gives. Every number is its `f64` in full, as in the shared fixture.
    pub(crate) fn earth_orbit() -> BodyOrbitDto {
        BodyOrbitDto {
            parent: OrbitHostDto::Star { body_index: 0 },
            orbit: OrbitDto {
                period_s: 31_558_148.628_135_167,
                semi_major_axis_m: 149_597_870_700.0,
                eccentricity: 0.0167,
                inclination_rad: 1.0,
                ascending_node_rad: 2.5,
                argument_of_periapsis_rad: 1.75,
                mean_anomaly_at_epoch_rad: 0.5,
                mu_m3_s2: 1.327_128_386_004e20,
            },
            valid_until: None,
            drift: None,
        }
    }

    pub(crate) fn earth_orbit_json() -> Value {
        json!({
            "parent": { "type": "star", "body_index": 0 },
            "orbit": {
                "period_s": 31_558_148.628_135_167,
                "semi_major_axis_m": 149_597_870_700.0,
                "eccentricity": 0.0167,
                "inclination_rad": 1.0,
                "ascending_node_rad": 2.5,
                "argument_of_periapsis_rad": 1.75,
                "mean_anomaly_at_epoch_rad": 0.5,
                "mu_m3_s2": 1.327_128_386_004e20,
            },
            "valid_until": null,
        })
    }

    /// Where [`earth_orbit`] puts the planet at the epoch, with its star at the barycentre.
    const EARTH_AT_EPOCH_M: [f64; 3] = [
        39_081_101_553.427_47,
        -105_520_721_023.434_39,
        95_232_836_817.832_58,
    ];

    /// Earth's bulk as the generator derives it (P14.T16.a's Solar System): 1 R⊕, the density
    /// and gravity of GM⊕ at that radius, a core mass fraction of 0.323 and 254.6 K.
    pub(crate) fn earth_bulk() -> BulkPropertiesDto {
        BulkPropertiesDto {
            radius_m: 6_371_000.0,
            density_kg_m3: 5_513.413_711_557_57,
            surface_gravity_m_s2: 9.820_249_457_244_522,
            class: PlanetClassDto::Rocky,
            mass_fractions: MassFractionsDto {
                iron: 0.323,
                rock: 0.677,
                water: 0.0,
                envelope: 0.0,
            },
            equilibrium_temperature_k: 254.6,
            effective_temperature_k: None,
        }
    }

    pub(crate) fn earth_bulk_json() -> Value {
        json!({
            "radius_m": 6_371_000.0,
            "density_kg_m3": 5_513.413_711_557_57,
            "surface_gravity_m_s2": 9.820_249_457_244_522,
            "class": "rocky",
            "mass_fractions": { "iron": 0.323, "rock": 0.677, "water": 0.0, "envelope": 0.0 },
            "equilibrium_temperature_k": 254.6,
            "effective_temperature_k": null,
        })
    }

    /// An Earth's frame: its pole tilted 0.409 rad from an orbit normal along +z, spinning once a
    /// sidereal day and never locking. Every number is its `f64` in full.
    pub(crate) fn earth_rotation() -> BodyRotationDto {
        BodyRotationDto {
            pole: [0.0, -0.397_740_291_376_376_3, 0.917_498_356_048_534_5],
            equator_node: [1.0, 0.0, 0.0],
            equator_quarter: [0.0, 0.917_498_356_048_534_5, 0.397_740_291_376_376_3],
            obliquity_rad: 0.409,
            initial_rate_rad_s: 7.292_115e-5,
            locked_rate_rad_s: 1.990_986_6e-7,
            age_at_epoch_s: 1.44e17,
            locking_age_s: None,
            locks_at: None,
            resonance: SpinResonanceDto::Synchronous,
            clock_period_s: 31_558_148.628_135_167,
            clock_mean_anomaly_at_epoch_rad: 0.5,
            sub_primary_angle_rad: 1.25,
            phase_at_epoch_rad: 4.894_961_212_735_792,
            capture_phase_rad: -0.75,
        }
    }

    pub(crate) fn earth_rotation_json() -> Value {
        json!({
            "pole": [0.0, -0.397_740_291_376_376_3, 0.917_498_356_048_534_5],
            "equator_node": [1.0, 0.0, 0.0],
            "equator_quarter": [0.0, 0.917_498_356_048_534_5, 0.397_740_291_376_376_3],
            "obliquity_rad": 0.409,
            "initial_rate_rad_s": 7.292_115e-5,
            "locked_rate_rad_s": 1.990_986_6e-7,
            "age_at_epoch_s": 1.44e17,
            "locking_age_s": null,
            "locks_at": null,
            "resonance": "synchronous",
            "clock_period_s": 31_558_148.628_135_167,
            "clock_mean_anomaly_at_epoch_rad": 0.5,
            "sub_primary_angle_rad": 1.25,
            "phase_at_epoch_rad": 4.894_961_212_735_792,
            "capture_phase_rad": -0.75,
        })
    }

    /// An Earth's figure: WGS 84's a and c (NIMA TR8350.2) about [`earth_rotation`]'s pole.
    pub(crate) fn earth_figure() -> BodyFigureDto {
        BodyFigureDto {
            equatorial_radius_m: 6_378_137.0,
            polar_radius_m: 6_356_752.314_245_179,
            flattening: 0.003_352_810_664_747_480_5,
            pole: earth_rotation().pole,
            moment_of_inertia_factor: 0.33,
            law: FigureLawDto::Rotational,
            datum: FigureDatumDto::SolidSurface,
        }
    }

    pub(crate) fn earth_figure_json() -> Value {
        json!({
            "equatorial_radius_m": 6_378_137.0,
            "polar_radius_m": 6_356_752.314_245_179,
            "flattening": 0.003_352_810_664_747_480_5,
            "pole": [0.0, -0.397_740_291_376_376_3, 0.917_498_356_048_534_5],
            "moment_of_inertia_factor": 0.33,
            "law": "rotational",
            "datum": "solid_surface",
        })
    }

    /// An Earth's photometry, illustrative: Earth's template and the generator's temperate Bond
    /// albedo of 0.306.
    pub(crate) fn earth_photometry() -> BodyPhotometryDto {
        BodyPhotometryDto {
            geometric_albedo: BandsDto {
                b: 0.44,
                v: 0.434,
                r: 0.411,
            },
            phase_template: PhaseTemplateDto::Earth,
            phase_exponent: BandsDto {
                b: 1.0,
                v: 1.0,
                r: 1.0,
            },
            lunar_lambert_share: 0.0,
            bond_albedo: 0.306,
            bond_ratio: 1.859_43,
            provisional: false,
        }
    }

    pub(crate) fn earth_photometry_json() -> Value {
        json!({
            "geometric_albedo": { "b": 0.44, "v": 0.434, "r": 0.411 },
            "phase_template": "earth",
            "phase_exponent": { "b": 1.0, "v": 1.0, "r": 1.0 },
            "lunar_lambert_share": 0.0,
            "bond_albedo": 0.306,
            "bond_ratio": 1.859_43,
            "provisional": false,
        })
    }

    /// The slice's record of an Earth, body `0x0300` about star 0, at the epoch.
    pub(crate) fn planet_summary() -> BodySummaryDto {
        BodySummaryDto {
            id: BodyIdHex::from_parts(SYSTEM, 0x0300),
            kind: BodyKindDto::Planet,
            label: SectionDto::NotModelled,
            parent: Some(OrbitHostDto::Star { body_index: 0 }),
            state: BodyStateDto::Present,
            position_m: Some(EARTH_AT_EPOCH_M),
            mass_kg: SectionDto::Ok(5.972_167_867_791_379e24),
            orbit: SectionDto::Ok(earth_orbit()),
            moons: SectionDto::NotModelled,
            rings: SectionDto::NotModelled,
            population: SectionDto::NotApplicable,
            bulk: SectionDto::Ok(earth_bulk()),
            rotation: Some(SectionDto::Ok(earth_rotation())),
            figure: Some(SectionDto::Ok(earth_figure())),
            photometry: Some(SectionDto::Ok(earth_photometry())),
        }
    }

    pub(crate) fn planet_summary_json() -> Value {
        json!({
            "id": "0200080020000000.0300",
            "kind": { "type": "planet" },
            "label": { "state": "not_modelled" },
            "parent": { "type": "star", "body_index": 0 },
            "state": { "type": "present" },
            "position_m": EARTH_AT_EPOCH_M,
            "mass_kg": { "state": "ok", "value": 5.972_167_867_791_379e24 },
            "orbit": { "state": "ok", "value": earth_orbit_json() },
            "moons": { "state": "not_modelled" },
            "rings": { "state": "not_modelled" },
            "population": { "state": "not_applicable" },
            "bulk": { "state": "ok", "value": earth_bulk_json() },
            "rotation": { "state": "ok", "value": earth_rotation_json() },
            "figure": { "state": "ok", "value": earth_figure_json() },
            "photometry": { "state": "ok", "value": earth_photometry_json() },
        })
    }

    /// The whole record of the planet of [`planet_summary`], as the slice tags it.
    pub(crate) fn planet_record() -> BodyRecordDto {
        let summary = planet_summary();
        BodyRecordDto {
            id: summary.id,
            kind: summary.kind,
            label: summary.label,
            parent: summary.parent,
            state: summary.state,
            position_m: summary.position_m,
            mass_kg: summary.mass_kg,
            orbit: summary.orbit,
            moons: summary.moons,
            rings: summary.rings,
            population: summary.population,
            bulk: summary.bulk,
            rotation: summary.rotation,
            figure: summary.figure,
            photometry: summary.photometry,
            surface: SectionDto::NotModelled,
            hooks: SectionDto::NotModelled,
        }
    }

    pub(crate) fn planet_record_json() -> Value {
        let mut wire = planet_summary_json();
        let fields = wire.as_object_mut().unwrap();
        fields.insert("surface".to_owned(), json!({ "state": "not_modelled" }));
        fields.insert("hooks".to_owned(), json!({ "state": "not_modelled" }));
        wire
    }

    /// The `body_detail` answer for [`planet_record`] at the full level.
    pub(crate) fn planet_detail() -> BodyDetailDto {
        BodyDetailDto {
            universe: UniverseIdHex::from_u64(42),
            time: UniverseTime::default(),
            granted: DetailLevelDto::Full,
            record: planet_record(),
        }
    }

    pub(crate) fn planet_detail_json() -> Value {
        json!({
            "universe": "000000000000002a",
            "time": { "seconds": 0, "nanos": 0 },
            "granted": "full",
            "record": planet_record_json(),
        })
    }

    /// Every key of `value` at any depth, objects' keys only.
    fn keys(value: &Value) -> Vec<String> {
        match value {
            Value::Object(fields) => fields
                .iter()
                .flat_map(|(key, inner)| std::iter::once(key.clone()).chain(keys(inner)))
                .collect(),
            Value::Array(items) => items.iter().flat_map(keys).collect(),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Vec::new(),
        }
    }

    #[test]
    fn body_summary_wire_form() {
        assert_wire_form(&planet_summary(), planet_summary_json());
    }

    #[test]
    fn body_record_wire_form() {
        assert_wire_form(&planet_record(), planet_record_json());
    }

    #[test]
    fn body_detail_wire_form() {
        assert_wire_form(&planet_detail(), planet_detail_json());
    }

    #[test]
    fn a_record_holds_every_field_of_a_summary() {
        let record = serde_json::to_value(planet_record()).unwrap();
        let summary = serde_json::to_value(planet_summary()).unwrap();
        for (key, value) in summary.as_object().unwrap() {
            assert_eq!(&record[key], value, "{key}");
        }
    }

    #[test]
    fn a_gas_giant_s_surface_is_not_applicable() {
        let saturn = BodyRecordDto {
            bulk: SectionDto::Ok(BulkPropertiesDto {
                class: PlanetClassDto::GasGiant,
                ..earth_bulk()
            }),
            surface: SectionDto::NotApplicable,
            ..planet_record()
        };
        let wire = serde_json::to_value(&saturn).unwrap();
        assert_eq!(wire["surface"], json!({ "state": "not_applicable" }));
        assert_eq!(wire["bulk"]["value"]["class"], json!("gas_giant"));
        assert_eq!(
            serde_json::from_value::<BodyRecordDto>(wire).unwrap(),
            saturn
        );
    }

    #[test]
    fn a_mass_and_orbit_record_holds_no_radius_temperature_or_composition() {
        // Plan 14's T34 test, whose JSON half moved here: the sections above the level are
        // withheld, so no key of theirs is on the wire at all.
        let record = BodyDetailDto {
            granted: DetailLevelDto::MassAndOrbit,
            record: BodyRecordDto {
                bulk: SectionDto::NotResolved,
                surface: SectionDto::NotResolved,
                hooks: SectionDto::NotResolved,
                ..planet_record()
            },
            ..planet_detail()
        };
        let wire = serde_json::to_value(&record).unwrap();
        let keys = keys(&wire);
        for withheld in [
            "radius_m",
            "density_kg_m3",
            "equilibrium_temperature_k",
            "effective_temperature_k",
            "mass_fractions",
            "class",
            "detail_seed",
        ] {
            assert!(
                !keys.iter().any(|key| key == withheld),
                "{withheld} in {wire}"
            );
        }
        assert_eq!(wire["record"]["bulk"], json!({ "state": "not_resolved" }));
        assert_eq!(wire["record"]["mass_kg"]["state"], json!("ok"));
        assert_eq!(
            serde_json::from_value::<BodyDetailDto>(wire).unwrap(),
            record
        );
    }

    #[test]
    fn a_contact_keeps_its_id_parent_state_and_position_and_nothing_else() {
        let contact = BodySummaryDto {
            kind: BodyKindDto::Unresolved,
            label: SectionDto::NotResolved,
            mass_kg: SectionDto::NotResolved,
            orbit: SectionDto::NotResolved,
            moons: SectionDto::NotResolved,
            rings: SectionDto::NotResolved,
            population: SectionDto::NotResolved,
            bulk: SectionDto::NotResolved,
            rotation: Some(SectionDto::NotResolved),
            figure: Some(SectionDto::NotResolved),
            photometry: Some(SectionDto::NotResolved),
            ..planet_summary()
        };
        let withheld = json!({ "state": "not_resolved" });
        assert_wire_form(
            &contact,
            json!({
                "id": "0200080020000000.0300",
                "kind": { "type": "unresolved" },
                "label": withheld,
                "parent": { "type": "star", "body_index": 0 },
                "state": { "type": "present" },
                "position_m": EARTH_AT_EPOCH_M,
                "mass_kg": withheld,
                "orbit": withheld,
                "moons": withheld,
                "rings": withheld,
                "population": withheld,
                "bulk": withheld,
                "rotation": withheld,
                "figure": withheld,
                "photometry": withheld,
            }),
        );
    }

    #[test]
    fn a_moon_lists_under_its_planet_with_its_label() {
        let moon = BodySummaryDto {
            id: BodyIdHex::from_parts(SYSTEM, 0x0301),
            kind: BodyKindDto::Moon {
                origin: MoonOriginDto::GiantImpact,
            },
            label: SectionDto::Ok("A b I".to_owned()),
            parent: Some(OrbitHostDto::Body {
                id: BodyIdHex::from_parts(SYSTEM, 0x0300),
            }),
            moons: SectionDto::NotApplicable,
            rings: SectionDto::NotApplicable,
            bulk: SectionDto::NotModelled,
            ..planet_summary()
        };
        let wire = serde_json::to_value(&moon).unwrap();
        assert_eq!(
            wire["kind"],
            json!({ "type": "moon", "origin": "giant_impact" })
        );
        assert_eq!(wire["label"], json!({ "state": "ok", "value": "A b I" }));
        assert_eq!(
            wire["parent"],
            json!({ "type": "body", "id": "0200080020000000.0300" })
        );
        assert_eq!(
            serde_json::from_value::<BodySummaryDto>(wire).unwrap(),
            moon
        );
    }

    #[test]
    fn a_planet_with_no_moons_lists_none_and_one_with_moons_lists_them() {
        let with = BodySummaryDto {
            moons: SectionDto::Ok(vec![
                BodyIdHex::from_parts(SYSTEM, 0x0301),
                BodyIdHex::from_parts(SYSTEM, 0x0302),
            ]),
            rings: SectionDto::Ok(Vec::new()),
            ..planet_summary()
        };
        let wire = serde_json::to_value(&with).unwrap();
        assert_eq!(
            wire["moons"],
            json!({
                "state": "ok",
                "value": ["0200080020000000.0301", "0200080020000000.0302"],
            })
        );
        assert_eq!(wire["rings"], json!({ "state": "ok", "value": [] }));
    }

    #[test]
    fn a_free_floating_object_orbits_nothing_and_sits_at_its_system_s_origin() {
        // A rogue planet is its system's root body (plan 14, design note 3): no parent, and no
        // orbit, but present, so it has a position.
        let rogue = BodySummaryDto {
            id: BodyIdHex::from_parts(SYSTEM, 0x0000),
            parent: None,
            position_m: Some([0.0; 3]),
            orbit: SectionDto::NotApplicable,
            ..planet_summary()
        };
        let wire = serde_json::to_value(&rogue).unwrap();
        assert_eq!(wire["parent"], Value::Null);
        assert_eq!(wire["position_m"], json!([0.0, 0.0, 0.0]));
        assert_eq!(wire["orbit"], json!({ "state": "not_applicable" }));
        assert_eq!(
            serde_json::from_value::<BodySummaryDto>(wire).unwrap(),
            rogue
        );
    }

    #[test]
    fn body_kind_wire_forms() {
        for (kind, wire) in [
            (BodyKindDto::Planet, json!({ "type": "planet" })),
            (BodyKindDto::DwarfPlanet, json!({ "type": "dwarf_planet" })),
            (
                BodyKindDto::Moon {
                    origin: MoonOriginDto::Regular,
                },
                json!({ "type": "moon", "origin": "regular" }),
            ),
            (
                BodyKindDto::Moon {
                    origin: MoonOriginDto::Captured,
                },
                json!({ "type": "moon", "origin": "captured" }),
            ),
            (BodyKindDto::Ring, json!({ "type": "ring" })),
            (
                BodyKindDto::Belt {
                    belt_kind: BeltKindDto::Asteroid,
                },
                json!({ "type": "belt", "belt_kind": "asteroid" }),
            ),
            (
                BodyKindDto::Belt {
                    belt_kind: BeltKindDto::Kuiper,
                },
                json!({ "type": "belt", "belt_kind": "kuiper" }),
            ),
            (
                BodyKindDto::CometaryHalo,
                json!({ "type": "cometary_halo" }),
            ),
            (
                BodyKindDto::ProtoplanetaryDisc,
                json!({ "type": "protoplanetary_disc" }),
            ),
            (BodyKindDto::DebrisDisc, json!({ "type": "debris_disc" })),
            (BodyKindDto::Unresolved, json!({ "type": "unresolved" })),
        ] {
            assert_wire_form(&kind, wire);
        }
    }

    #[test]
    fn moon_origin_strings() {
        assert_wire_strings(&[
            (MoonOriginDto::Regular, "regular"),
            (MoonOriginDto::GiantImpact, "giant_impact"),
            (MoonOriginDto::Captured, "captured"),
        ]);
    }

    #[test]
    fn belt_kind_strings() {
        assert_wire_strings(&[
            (BeltKindDto::Asteroid, "asteroid"),
            (BeltKindDto::Kuiper, "kuiper"),
        ]);
    }

    #[test]
    fn body_state_wire_forms() {
        let at = UniverseTime {
            seconds: -31_557_600,
            nanos: 250_000_000,
        };
        let at_json = json!({ "seconds": -31_557_600, "nanos": 250_000_000 });
        assert_wire_form(
            &BodyStateDto::NotYetFormed,
            json!({ "type": "not_yet_formed" }),
        );
        assert_wire_form(&BodyStateDto::Present, json!({ "type": "present" }));
        assert_wire_form(
            &BodyStateDto::Destroyed {
                cause: DestructionCauseDto::Engulfed,
                at,
            },
            json!({ "type": "destroyed", "cause": "engulfed", "at": at_json }),
        );
        assert_wire_form(
            &BodyStateDto::Unbound { at },
            json!({ "type": "unbound", "at": at_json }),
        );
    }

    #[test]
    fn destruction_cause_strings() {
        assert_wire_strings(&[
            (DestructionCauseDto::Dispersed, "dispersed"),
            (DestructionCauseDto::Engulfed, "engulfed"),
            (DestructionCauseDto::TidallyDisrupted, "tidally_disrupted"),
            (DestructionCauseDto::Collided, "collided"),
        ]);
    }

    #[test]
    fn planet_class_strings() {
        assert_wire_strings(&[
            (PlanetClassDto::Rocky, "rocky"),
            (PlanetClassDto::Icy, "icy"),
            (PlanetClassDto::SubNeptune, "sub_neptune"),
            (PlanetClassDto::IceGiant, "ice_giant"),
            (PlanetClassDto::GasGiant, "gas_giant"),
        ]);
    }

    #[test]
    fn bulk_properties_wire_form() {
        assert_wire_form(&earth_bulk(), earth_bulk_json());
    }

    /// Ruling 112.7: a giant carries its effective temperature beside its equilibrium one.
    #[test]
    fn a_giant_s_bulk_carries_its_effective_temperature() {
        let jupiter = BulkPropertiesDto {
            class: PlanetClassDto::GasGiant,
            equilibrium_temperature_k: 109.9,
            effective_temperature_k: Some(124.4),
            ..earth_bulk()
        };
        let mut expected = earth_bulk_json();
        expected["class"] = json!("gas_giant");
        expected["equilibrium_temperature_k"] = json!(109.9);
        expected["effective_temperature_k"] = json!(124.4);
        assert_wire_form(&jupiter, expected);
    }

    #[test]
    fn body_hooks_wire_form() {
        assert_wire_form(
            &BodyHooksDto {
                detail_seed: SectionDto::NotModelled,
            },
            json!({ "detail_seed": { "state": "not_modelled" } }),
        );
        assert_wire_form(
            &BodyHooksDto {
                detail_seed: SectionDto::Ok(DetailSeedHex::from_u64(0x0123_4567_89ab_cdef)),
            },
            json!({ "detail_seed": { "state": "ok", "value": "0123456789abcdef" } }),
        );
    }

    #[test]
    fn a_surface_section_cannot_claim_a_value_yet() {
        // The surface has no fields until P14.T13, T14 and T24, so no `ok` surface parses.
        let error = serde_json::from_value::<SectionDto<BodySurfaceDto>>(json!({
            "state": "ok",
            "value": { "atmosphere": [] },
        }))
        .unwrap_err();
        assert!(
            error.to_string().contains("there are no variants"),
            "unexpected error: {error}"
        );
        assert_wire_form(
            &SectionDto::<BodySurfaceDto>::NotApplicable,
            json!({ "state": "not_applicable" }),
        );
    }

    #[test]
    fn an_unknown_body_kind_is_rejected() {
        let error = serde_json::from_value::<BodyKindDto>(json!({ "type": "comet" })).unwrap_err();
        assert!(
            error.to_string().contains("unknown variant `comet`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn body_rotation_wire_form() {
        assert_wire_form(&earth_rotation(), earth_rotation_json());
    }

    #[test]
    fn a_rotation_that_locks_in_the_window_carries_its_lock() {
        let locking = BodyRotationDto {
            locking_age_s: Some(1.5e17),
            locks_at: Some(UniverseTime {
                seconds: 6_000_000_000_000_000,
                nanos: 250,
            }),
            resonance: SpinResonanceDto::ThreeToTwo,
            ..earth_rotation()
        };
        let mut expected = earth_rotation_json();
        expected["locking_age_s"] = json!(1.5e17);
        expected["locks_at"] = json!({ "seconds": 6_000_000_000_000_000_i64, "nanos": 250 });
        expected["resonance"] = json!("three_to_two");
        assert_wire_form(&locking, expected);
    }

    #[test]
    fn spin_resonance_strings() {
        assert_wire_strings(&[
            (SpinResonanceDto::Synchronous, "synchronous"),
            (SpinResonanceDto::ThreeToTwo, "three_to_two"),
        ]);
    }

    #[test]
    fn body_figure_wire_form() {
        assert_wire_form(&earth_figure(), earth_figure_json());
    }

    #[test]
    fn figure_law_strings() {
        assert_wire_strings(&[
            (FigureLawDto::Sphere, "sphere"),
            (FigureLawDto::Rotational, "rotational"),
            (FigureLawDto::RotationalAndTidal, "rotational_and_tidal"),
            (FigureLawDto::Capped, "capped"),
        ]);
    }

    #[test]
    fn figure_datum_strings() {
        assert_wire_strings(&[
            (FigureDatumDto::SolidSurface, "solid_surface"),
            (FigureDatumDto::OneBar, "one_bar"),
        ]);
    }

    #[test]
    fn bands_wire_form() {
        assert_wire_form(
            &BandsDto {
                b: 0.25,
                v: 0.5,
                r: 0.75,
            },
            json!({ "b": 0.25, "v": 0.5, "r": 0.75 }),
        );
    }

    #[test]
    fn phase_template_strings() {
        assert_wire_strings(&[
            (PhaseTemplateDto::Moon, "moon"),
            (PhaseTemplateDto::Mercury, "mercury"),
            (PhaseTemplateDto::Mars, "mars"),
            (PhaseTemplateDto::Venus, "venus"),
            (PhaseTemplateDto::Earth, "earth"),
            (PhaseTemplateDto::Jupiter, "jupiter"),
            (PhaseTemplateDto::Saturn, "saturn"),
            (PhaseTemplateDto::Uranus, "uranus"),
            (PhaseTemplateDto::Neptune, "neptune"),
            (PhaseTemplateDto::AirlessIce, "airless_ice"),
            (PhaseTemplateDto::Snowball, "snowball"),
            (PhaseTemplateDto::Magma, "magma"),
        ]);
    }

    #[test]
    fn body_photometry_wire_form() {
        assert_wire_form(&earth_photometry(), earth_photometry_json());
    }

    /// Each new field in each section state, on a summary and on a record: `ok` is the fixture's;
    /// the three others carry their tag alone.
    #[test]
    fn the_rotation_figure_and_photometry_sections_in_every_state() {
        fn withheld<T>(tag: &str) -> SectionDto<T> {
            match tag {
                "not_resolved" => SectionDto::NotResolved,
                "not_modelled" => SectionDto::NotModelled,
                _ => SectionDto::NotApplicable,
            }
        }
        for tag in ["not_resolved", "not_modelled", "not_applicable"] {
            let tagged = json!({ "state": tag });
            let summary = BodySummaryDto {
                rotation: Some(withheld(tag)),
                figure: Some(withheld(tag)),
                photometry: Some(withheld(tag)),
                ..planet_summary()
            };
            let mut expected = planet_summary_json();
            for field in ["rotation", "figure", "photometry"] {
                expected[field] = tagged.clone();
            }
            assert_wire_form(&summary, expected);
            let record = BodyRecordDto {
                rotation: summary.rotation,
                figure: summary.figure,
                photometry: summary.photometry,
                ..planet_record()
            };
            let mut expected = planet_record_json();
            for field in ["rotation", "figure", "photometry"] {
                expected[field] = tagged.clone();
            }
            assert_wire_form(&record, expected);
        }
    }

    /// An older server's summary and record, with none of the three fields, still deserialise,
    /// as absent; and absent fields are not written.
    #[test]
    fn a_summary_without_the_rotation_figure_and_photometry_still_deserialises() {
        let older = |mut wire: Value| {
            let fields = wire.as_object_mut().unwrap();
            for field in ["rotation", "figure", "photometry"] {
                fields.remove(field);
            }
            wire
        };
        let summary = BodySummaryDto {
            rotation: None,
            figure: None,
            photometry: None,
            ..planet_summary()
        };
        assert_wire_form(&summary, older(planet_summary_json()));
        let record = BodyRecordDto {
            rotation: None,
            figure: None,
            photometry: None,
            ..planet_record()
        };
        assert_wire_form(&record, older(planet_record_json()));
    }
}
