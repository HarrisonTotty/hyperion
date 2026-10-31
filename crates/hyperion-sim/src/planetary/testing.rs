//! Test helpers of the planetary stage (plan 14's Provides): synthetic hosts and samples of real
//! systems, for this crate's tests and, behind its `testing` feature, for other crates'.
//!
//! - [`synthetic_star`] and [`synthetic_binary`] are [`SystemContext::builder`] with the median
//!   star's draws, under a caller-supplied [`SystemId`] so that the stage's streams differ between
//!   samples. The builder itself gives the other choices: plan 06's own draws of each star
//!   ([`SyntheticDraws::OfUniverse`](super::context::SyntheticDraws::OfUniverse)), another
//!   sphere of influence, an encounter environment.
//! - [`sample_contexts`] draws real systems from a galaxy of the Milky Way's parameters: the
//!   systems nearest a Sun-like point that pass a [`SampleFilter`], for the statistical tests
//!   (design note 20).
//! - For this crate's tests alone, `with_every_25_hold` runs a closure with every landed hold at
//!   generator version 25 in force together, so that a test of a 25 figure reads 25 as it will
//!   ship (decision-t24i-airless-state D3.3).
//! - [`partial_pressures`] states an air by hand, for other crates' tests of what a derivation
//!   reads of one.
//! - [`envelope_by_hand`] builds a record's envelope from inputs stated by hand
//!   ([`EnvelopeByHand`]), through the record's own chain, for other crates' tests of its
//!   conversion and the shared fixtures' envelopes (P14.T54.b.i).

use std::error::Error;
use std::fmt;

use super::chem::HostAbundances;
use super::chem::envelope_abundances::{EnvelopeDraws, Migration};
use super::chem::equilibrium::Network;
use super::context::{BuildSystemContextError, SystemContext};
use super::derive::atmosphere::{Gas, PartialPressures};
use super::derive::{FormationZone, MassFractions};
use super::record::Envelope;
pub use super::system::envelope::DeriveEnvelopeError;
use super::system::envelope::{EnvelopeOf, envelope_of};
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{LayerSpec, STELLAR_LAYERS, SystemRecord, generate_cell};
use crate::galaxy::query::{QuerySphere, cells_in_sphere, count_cells_in_sphere};
use crate::galaxy::{BuildGalaxyError, Galaxy};
use crate::id::SystemId;
use crate::orbit::Eccentricity;
use crate::time::UniverseTime;
use crate::units::{
    Dex, EarthMasses, Kelvin, LightYears, Metres, MetresPerSecondSquared, Pascals, SolarMasses,
    Watts, Years,
};

/// An air's partial pressures stated by hand, each gas's `pressure(gas)` in [`Gas::ALL`]'s order,
/// unvalidated, as the crate's own tests state one: for other crates' tests of what a derivation
/// reads of an air, as the server's conversion of P14.T51.j's oxidised surface does (a caller's
/// values are its own to check, as `SurfaceModifiers::derive` checks its inputs').
#[must_use]
pub fn partial_pressures(pressure: impl Fn(Gas) -> Pascals) -> PartialPressures {
    PartialPressures::from_fn(pressure)
}

/// A gas envelope's inputs stated by hand ([`envelope_by_hand`]): what a record's envelope reads of
/// its body, its host and its time, each a plain value its caller states (P14.T54.b.i).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopeByHand {
    /// The body's mass.
    pub mass: EarthMasses,
    /// Its mass fractions, whose envelope share is at least
    /// [`THIN_ENVELOPE_FRACTION`](crate::planetary::params::THIN_ENVELOPE_FRACTION).
    pub fractions: MassFractions,
    /// Its radius, its bulk section's.
    pub radius: Metres,
    /// Its surface gravity, its bulk section's.
    pub gravity: MetresPerSecondSquared,
    /// Its equilibrium temperature, its bulk section's: from its hosts' light alone, with its Bond
    /// albedo (ruling 112.7).
    pub equilibrium_temperature: Kelvin,
    /// Its internal luminosity, the record's (0 below 0.3 Jupiter masses, where P14.T24.d's
    /// cooling law gives its heat).
    pub internal_luminosity: Watts,
    /// Its age at the time.
    pub age: Years,
    /// Where it formed from its host.
    pub formation_distance: Metres,
    /// Its host's disc's snow line.
    pub snow_line: Metres,
    /// Its host's \[Fe/H\], dex.
    pub fe_h: Dex,
    /// Its host's \[α/Fe\], dex.
    pub alpha_fe: Dex,
    /// Its host's C/O and Mg/Si (P14.T50.a).
    pub host: HostAbundances,
    /// Its envelope's He/H by number before any helium rain: its host star's own, less what escape
    /// fractionated (a solar host's unfractionated, the protosolar
    /// [`Element::He`](crate::substance::Element::He)'s).
    pub primordial_helium_per_hydrogen: f64,
    /// Whether a star's ultraviolet lights it.
    pub starlit: bool,
}

/// The record's envelope of the body `inputs` states, by the record's own chain (P14.T24.d's
/// internal temperature, P14.T50.b's abundances at the envelope's (`T_eq`⁴ + `T_int`⁴)^¼ and
/// P14.T52.d's atmosphere on the adopted grey profile), on a network built afresh: for other
/// crates' tests of what the record holds, as the server's conversion and its fixtures' envelopes
/// read one (a caller's values are its own to check). Under P14.T50.d's formation draws it reads
/// the median ranks of a body in situ ([`EnvelopeDraws::MEDIAN`]), which are T50.b's envelope,
/// bit for bit.
///
/// # Errors
///
/// [`DeriveEnvelopeError`], the refusing step's:
///
/// - [`DeriveEnvelopeError::FormationZone`] for a formation distance or snow line that is not
///   positive;
/// - [`DeriveEnvelopeError::Network`] where P14.T52.a's network cannot be built from the registry,
///   which the registry as built never gives;
/// - [`DeriveEnvelopeError::InternalTemperature`], [`DeriveEnvelopeError::Abundances`] or
///   [`DeriveEnvelopeError::Atmosphere`] for inputs P14.T24.d's internal temperature, P14.T50.b or
///   P14.T52.d refuses, which leave a record's envelope not modelled.
///
/// [`DeriveEnvelopeError::NoAlphaFe`] and [`DeriveEnvelopeError::NoDisc`] are a record's alone:
/// the inputs by hand always state an \[α/Fe\] and a disc's snow line.
pub fn envelope_by_hand(inputs: &EnvelopeByHand) -> Result<Envelope, DeriveEnvelopeError> {
    let formation = FormationZone::new(inputs.formation_distance, inputs.snow_line)
        .map_err(DeriveEnvelopeError::FormationZone)?;
    let network = Network::registry().map_err(DeriveEnvelopeError::Network)?;
    let of = EnvelopeOf {
        mass: inputs.mass,
        fractions: inputs.fractions,
        radius: inputs.radius,
        gravity: inputs.gravity,
        equilibrium_temperature: inputs.equilibrium_temperature,
        internal_luminosity: inputs.internal_luminosity,
        age: inputs.age,
        formation: Ok(formation),
        fe_h: inputs.fe_h,
        alpha_fe: Some(inputs.alpha_fe),
        host: inputs.host,
        primordial_helium_per_hydrogen: inputs.primordial_helium_per_hydrogen,
        starlit: inputs.starlit,
        draws: EnvelopeDraws::MEDIAN,
        migration: Migration::InSitu,
    };
    envelope_of(&of, &network)
}

/// A single star of initial mass `mass` (0.08–150 M☉), \[Fe/H\] `fe_h` and age at the epoch
/// `age`, as the system `id`: the builder's defaults otherwise (the median star's draws, the
/// solar neighbourhood's sphere of influence).
///
/// # Errors
///
/// As [`SystemContextBuilder::build`](super::context::SystemContextBuilder::build): a mass, age or
/// \[Fe/H\] out of range.
pub fn synthetic_star(
    id: SystemId,
    mass: SolarMasses,
    fe_h: Dex,
    age: Years,
) -> Result<SystemContext, BuildSystemContextError> {
    SystemContext::builder()
        .system(id)
        .star(mass)
        .fe_h(fe_h)
        .age_at_epoch(age)
        .build()
}

/// A binary as the system `id`: a primary of `m1` M☉ and a companion of `m2` M☉ (a brown dwarf
/// below 0.08 M☉) on a relative orbit of semi-major axis `a` and eccentricity `e`, in the
/// reference plane with the companion at periapsis at the epoch, with \[Fe/H\] `fe_h` and age at
/// the epoch `age`; the builder's defaults otherwise.
///
/// # Errors
///
/// As [`SystemContextBuilder::build`](super::context::SystemContextBuilder::build): masses, age or
/// \[Fe/H\] out of range, a companion heavier than the primary, an orbit that cannot be built or
/// whose period or eccentricity plan 11's draw would not give, or one beyond the tidal cut.
pub fn synthetic_binary(
    id: SystemId,
    m1: SolarMasses,
    m2: SolarMasses,
    a: Metres,
    e: Eccentricity,
    fe_h: Dex,
    age: Years,
) -> Result<SystemContext, BuildSystemContextError> {
    SystemContext::builder()
        .system(id)
        .binary(m1, m2, a, e)
        .fe_h(fe_h)
        .age_at_epoch(age)
        .build()
}

/// The Sun-like point about which [`sample_contexts`] draws, light-years in the galactic frame:
/// in the plane, 26,000 ly out along +y, clear of the bar, where the other plans' tests take the
/// solar neighbourhood.
pub const SAMPLE_CENTRE_LY: [f64; 3] = [0.0, 26_000.0, 0.0];

/// The radius of the first sphere [`sample_contexts`] searches, light-years; it doubles until
/// the sphere holds enough systems.
const FIRST_RADIUS_LY: f64 = 16.0;

/// The most generation cells one sphere of [`sample_contexts`] may cover, summed over the layers
/// it walks: 2¹⁸, a sphere of about 315 ly in layer A, which holds some 10⁵ systems at the Sun's
/// density.
pub const MAX_SAMPLE_CELLS: u64 = 1 << 18;

/// Which systems [`sample_contexts`] keeps: a band of primary initial mass, and a test of the
/// record.
///
/// The band also decides which layers are walked at all, so a filter for M dwarfs generates no
/// cell of the massive stars' layers. A filter on anything a record does not carry (\[Fe/H\],
/// the stars' states) is a test of the contexts afterwards.
#[derive(Clone, Copy)]
pub struct SampleFilter {
    lo: SolarMasses,
    hi: SolarMasses,
    keep: fn(&SystemRecord) -> bool,
}

impl SampleFilter {
    /// Every system.
    pub const ALL: Self = Self {
        lo: SolarMasses::ZERO,
        hi: SolarMasses::new(f64::INFINITY),
        keep: keep_every,
    };

    /// The systems whose primary's initial mass is at least `lo` and below `hi`, M☉.
    #[must_use]
    pub const fn primary_masses(lo: SolarMasses, hi: SolarMasses) -> Self {
        Self {
            lo,
            hi,
            keep: keep_every,
        }
    }

    /// This filter, keeping only the records for which `keep` is also true.
    #[must_use]
    pub const fn keeping(self, keep: fn(&SystemRecord) -> bool) -> Self {
        Self { keep, ..self }
    }

    /// Whether `record` passes.
    #[must_use]
    fn passes(&self, record: &SystemRecord) -> bool {
        let m = record.primary_initial_mass().value();
        self.lo.value() <= m && m < self.hi.value() && (self.keep)(record)
    }

    /// Whether any primary of `band`, whose masses the placement draws from the closed band, can
    /// pass.
    #[must_use]
    fn may_pass(&self, band: MassBand) -> bool {
        band.lo() < self.hi.value() && self.lo.value() <= band.hi()
    }
}

impl fmt::Debug for SampleFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SampleFilter")
            .field("lo", &self.lo)
            .field("hi", &self.hi)
            .finish_non_exhaustive()
    }
}

/// The record test of a filter that keeps every record.
#[must_use]
fn keep_every(_: &SystemRecord) -> bool {
    true
}

/// The contexts of `n` real systems of the galaxy of `seed` with the Milky Way's parameters
/// ([`GalaxyParams::milky_way_like`]), for the statistical tests: [`sample_records`] of that
/// galaxy, each through [`SystemContext::from_record`], which is what
/// [`SystemContext::for_system`] gives its ID.
///
/// Pass the same `seed` to the planetary generators to generate the systems of that universe. A
/// context costs a stellar track, about a millisecond.
///
/// # Errors
///
/// - [`SampleContextsError::Galaxy`] if the galaxy cannot be built.
/// - [`SampleContextsError::TooFewSystems`] as [`sample_records`].
pub fn sample_contexts(
    n: usize,
    seed: Seed,
    filter: SampleFilter,
) -> Result<Vec<SystemContext>, SampleContextsError> {
    let galaxy = Galaxy::from_params(seed, GalaxyParams::milky_way_like())
        .map_err(SampleContextsError::Galaxy)?;
    Ok(sample_records(&galaxy, n, filter)?
        .iter()
        .map(|record| SystemContext::from_record(&galaxy, record))
        .collect())
}

/// The `n` systems of `galaxy` nearest [`SAMPLE_CENTRE_LY`] at the epoch that pass `filter`,
/// nearest first, ties broken by ID.
///
/// A volume-limited sample, so over every mass it is the galaxy's own mixture of masses,
/// populations and ages at the solar circle. It searches spheres from 16 ly, doubling the radius
/// until one holds `n` systems that pass, in the layers the filter's mass band reaches. The
/// answer is a pure function of the galaxy, `n` and the filter, and the first `k` of it are the
/// sample of `k`.
///
/// # Errors
///
/// [`SampleContextsError::TooFewSystems`] if no layer holds the filter's masses, or if the sphere
/// that would hold `n` of them covers more than [`MAX_SAMPLE_CELLS`] cells.
///
/// # Panics
///
/// Never: the Sun-like point lies inside the root cube, and every sphere searched has a positive,
/// finite radius.
pub fn sample_records(
    galaxy: &Galaxy,
    n: usize,
    filter: SampleFilter,
) -> Result<Vec<SystemRecord>, SampleContextsError> {
    let centre = GalacticPosition::from_light_years(SAMPLE_CENTRE_LY)
        .expect("the Sun-like point lies inside the root cube");
    let layers: Vec<_> = STELLAR_LAYERS
        .iter()
        .filter(|spec| filter.may_pass(spec.band()))
        .map(LayerSpec::layer)
        .collect();
    let mut radius = FIRST_RADIUS_LY;
    let mut found = Vec::new();
    let mut cell = Vec::new();
    loop {
        let sphere = QuerySphere::new(
            centre,
            LightYears::new(radius),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a positive, finite radius at the epoch, where no pad is needed");
        let cells: u64 = layers
            .iter()
            .map(|&layer| count_cells_in_sphere(layer, &sphere))
            .sum();
        if layers.is_empty() || cells > MAX_SAMPLE_CELLS {
            return Err(SampleContextsError::TooFewSystems {
                wanted: n,
                found: found.len(),
            });
        }
        let limit = Metres::from(LightYears::new(radius));
        found.clear();
        for &layer in &layers {
            for key in cells_in_sphere(layer, &sphere) {
                generate_cell(galaxy, key, &mut cell);
                for record in &cell {
                    let distance = centre.distance_to(record.epoch_position());
                    if distance <= limit && filter.passes(record) {
                        found.push((distance, *record));
                    }
                }
            }
        }
        if found.len() >= n {
            found
                .sort_by(|(d1, r1), (d2, r2)| d1.total_cmp(d2).then_with(|| r1.id().cmp(&r2.id())));
            return Ok(found
                .into_iter()
                .take(n)
                .map(|(_, record)| record)
                .collect());
        }
        radius *= 2.0;
    }
}

/// [`sample_contexts`] or [`sample_records`] could not draw its sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleContextsError {
    /// The galaxy could not be built.
    Galaxy(BuildGalaxyError),
    /// Fewer systems pass the filter than were asked for, inside the largest sphere searched.
    TooFewSystems {
        /// The number asked for.
        wanted: usize,
        /// The number found in the last sphere searched.
        found: usize,
    },
}

impl fmt::Display for SampleContextsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Galaxy(_) => f.write_str("the sample's galaxy cannot be built"),
            Self::TooFewSystems { wanted, found } => write!(
                f,
                "only {found} of the {wanted} systems asked for pass the filter near the sun-like \
                 point"
            ),
        }
    }
}

impl Error for SampleContextsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Galaxy(error) => Some(error),
            Self::TooFewSystems { .. } => None,
        }
    }
}

/// Gives the free-floating hosts their systems for every thread of this process, ahead of their
/// holds at generator version 26 (P14.T27.a's `SUBSTELLAR_HOSTS_VERSION` and P14.T27.b's
/// `ROGUE_PLANET_HOSTS_VERSION`), for another crate's test binary: the server answers on its pool's
/// threads, which a switch on the test's thread would not reach. A brown dwarf's and a rogue
/// planet's systems are generated, and the frame search walks their layers
/// ([`frame_at`](crate::galaxy::frame::frame_at)); every other 26 hold stays held, so every other
/// system reads version 25's records. It is one-way and process-wide, so a binary that calls it
/// calls it at the start of every test, and holds no test that asks for the free-floating hosts at
/// 25. Only a test binary calls this. 26's bump deletes it with the holds.
#[cfg(feature = "testing")]
pub fn admit_free_floating_hosts_in_this_process() {
    super::hosts::substellar::admit_free_floating_in_this_process();
}

/// `run`'s result with every landed hold at generator version 25 in force on this thread, for a
/// test that asserts a 25 figure: a hold that missed 24's membership, which P14.T54.b.i closed
/// (decision-p14-t53b-part3 §8), rides 25 behind its own version and switch, so that every 24 test
/// reads none of it. Each switch sets its rules back as it returns, a panic in `run` included, and
/// a test may set one hold back inside it to compare 25 with and without it.
///
/// The holds, each with its switch:
///
/// - the passes' superlinear root, `SECANT_ROOT_VERSION` (P14.T51.l), at `PassRoot::Secant`
///   through `with_pass_root`: a bracketed body's secant steps, a detected 2-cycle its own bracket.
///
/// A system or a record is read under the rules in force when it is generated: one generated
/// outside the helper (the tests' caches, `whole` and `golden_systems`, are version 24's) is not
/// 25's, so a test generates afresh inside it. A later hold at 25 adds its switch here; 25's bump
/// deletes this helper with its holds.
#[cfg(test)]
pub(crate) fn with_every_25_hold<R>(run: impl FnOnce() -> R) -> R {
    use crate::planetary::derive::passes::{PassRoot, with_pass_root};
    with_pass_root(PassRoot::Secant, run)
}

/// `run`'s result with every landed hold at generator version 26 in force on this thread, for a
/// test that asserts a 26 figure: every 25 hold ([`with_every_25_hold`]) and those at 26, each with
/// its switch, which sets its rules back as it returns, a panic in `run` included. A test may set
/// one hold back inside it to compare 26 with and without it.
///
/// The holds at 26:
///
/// - the substellar deck of a world locked 1:1, `LOCKED_CLIMATE_VERSION` (P14.T24.j), at
///   `LockedClimateRules::Deck` through `with_locked_climate`: its column's deck albedo and locked
///   greenhouse over open water, which its record and its passes' steps read;
/// - a molten body's surface on its published air, `MELT_OXYGEN_VERSION` (P14.T53.g), at
///   `MeltOxygenRules::Published` through `with_melt_oxygen`.
/// - a free-floating brown dwarf's planets, `SUBSTELLAR_HOSTS_VERSION` (P14.T27.a), at
///   `SubstellarHostRules::BrownDwarfs` through `with_substellar_hosts`.
/// - a rogue planet's bodies, `ROGUE_PLANET_HOSTS_VERSION` (P14.T27.b), at
///   `RoguePlanetRules::Bodies` through `with_rogue_planets`.
/// - the eyeball, `LOCKED_STATE_VERSION` (P14.T53.e's rule 5 on a world locked 1:1), at
///   `LockedStateRules::Eyeball` through `with_locked_state`: rule 5 also needs no open water
///   there;
/// - the dry deck, `SEA_DECK_VERSION` (P14.T53.b part 1 (b)–(c)), at `SeaDeckRules::Dry` through
///   `with_sea_deck`: (c) reads the published label, and a decked runaway over liquid water is
///   solved without the deck;
/// - the planets' lag, `PLANET_LAG_VERSION` (P14.T56.g's rule for an airless planet), at
///   `PlanetLagRules::Airless` through `with_planet_lag`: an airless or tenuous planet over a hot
///   ice bulk derived once more under the rock lag.
/// - the decks' fall radius, `FALL_RADIUS_VERSION` (P14.T54.b.l's cost lever), at
///   `FallRadiusRoot::FalsePosition` through `with_fall_radius`: each level's `r_w` by the Illinois
///   false position on ln `v_f` in ln r, to the bisections' width.
/// - the settled compression, `SETTLED_COMPRESSION_VERSION` (P14.T54.b.l's cost lever), at
///   `CompressionRules::Settled` through `with_compression`: a liquid's compression ended by the
///   first step that moves its density by at most 2⁻⁴⁰ of itself.
/// - the CO₂ cap's energy balance, `CAP_BALANCE_VERSION` (P14.T53.h), at
///   `CapBalanceRules::Balanced` through `with_cap_balance`.
/// - the tidal and encounter stripping, `STRIP_VERSION` (P14.T29), at `StripRules::Stripped`
///   through `with_stripping`: plan 09's sphere of influence and a feature member's encounter cut.
/// - the passes' own residual, `PASS_STOP_VERSION` (P14.T54.b.l's cost lever), at
///   `PassStop::OwnResidual` through `with_pass_stop`: a body's plain passes ended at the first
///   pass that settles alone.
/// - the 100 Pa line read once, `LINE_SWITCH_VERSION` (P14.T53.i), at `LineSwitchRules::OneSwitch`
///   through `with_line_switch`: a sub-line solve whose last step read the bulk climate held once
///   more over the airless span its record publishes.
/// - the passes' Aitken step, `AITKEN_STEP_VERSION` (P14.T54.b.l's cost lever), at
///   `PassStep::Aitken` through `with_pass_step`: a pass reading Aitken's Δ² extrapolation once
///   three plain passes in a row show linear convergence, under the own residual's stop.
/// - a solid body's supercritical envelope's, `SUPERCRITICAL_ENVELOPE_VERSION` (P14.T53.f), at
///   `SupercriticalEnvelopeRules::Modelled` through `with_supercritical_envelope`: its envelope
///   section's model, which nothing else reads;
/// - a loop deck's mode drawn on its own column, `DECK_PROFILE_VERSION` (P14.T52.g), at
///   `DeckProfileRules::Saturation` through `with_deck_profile`: its profile's exponent the
///   smaller of its deck's `f_sed` and its saturation's log-slope at the base;
/// - a hot envelope's dayside above 1 bar, `DAYSIDE_DECKS_VERSION` (P14.T52.h), at
///   `DaysideDecksRules::Cleared` through `with_dayside_decks`: each refractory, sulphide or salt
///   mode of an envelope at `T_eq` ≥ 950 K cut at 1 bar, or its photosphere where deeper.
/// - the protoplanetary disc body, `PROTOPLANETARY_DISC_VERSION` (P14.T28.a), at
///   `ProtoplanetaryDiscRules::Disc` through `with_protoplanetary_disc`.
/// - a white dwarf's pollution and debris disc, `WHITE_DWARF_DEBRIS_VERSION` (P14.T28.d), at
///   `WhiteDwarfDebrisRules::Debris` through `with_white_dwarf_debris`.
/// - a neutron star's second-generation planets, `SECOND_GENERATION_VERSION` (P14.T28.e), at
///   `SecondGenerationRules::Planets` through `with_second_generation`.
/// - an envelope's formation draws, `FORMATION_DRAWS_VERSION` (P14.T50.d), at
///   `FormationDrawRules::Drawn` through `with_formation_draws`: its enrichment's scatter and its
///   solids' share, on its own stream.
/// - a regolith's pore gas, `PORE_GAS_VERSION` (P14.T49.l), at `RegolithInertiaRules::PoreGas`
///   through `with_regolith_inertia`: the gas in a ground's pores at its column's pressure, and the
///   outer system's fluffy amorphous ice as the water-ice form.
/// - the zonal search's Z₀, `Z0_BRANCH_VERSION` (R09.T13.i), at `Z0BranchRules::Kept` through
///   `with_z0_branch`: Z₀'s drift stop, and the held run in its place where it drifted.
///
/// As under [`with_every_25_hold`], a system or a record is read under the rules in force when
/// it is generated, so a test generates afresh inside it. A later hold at 26 adds its switch
/// here; 26's bump deletes this helper with its holds.
#[cfg(test)]
pub(crate) fn with_every_26_hold<R>(run: impl FnOnce() -> R) -> R {
    use crate::planetary::chem::condense::{
        DeckProfileRules, FallRadiusRoot, with_deck_profile, with_fall_radius,
    };
    use crate::planetary::chem::envelope_abundances::{FormationDrawRules, with_formation_draws};
    use crate::planetary::derive::atmosphere::{
        LineSwitchRules, LockedStateRules, MeltOxygenRules, SeaDeckRules, with_line_switch,
        with_locked_state, with_melt_oxygen, with_sea_deck,
    };
    use crate::planetary::derive::passes::{PassStep, PassStop, with_pass_step, with_pass_stop};
    use crate::planetary::hooks::envelope_atmosphere::{DaysideDecksRules, with_dayside_decks};
    use crate::planetary::hooks::surface::deck::{LockedClimateRules, with_locked_climate};
    use crate::planetary::hooks::surface::{CapBalanceRules, with_cap_balance};
    use crate::planetary::hosts::second_generation::{
        SecondGenerationRules, with_second_generation,
    };
    use crate::planetary::hosts::strip::{StripRules, with_stripping};
    use crate::planetary::hosts::substellar::{
        RoguePlanetRules, SubstellarHostRules, with_rogue_planets, with_substellar_hosts,
    };
    use crate::planetary::hosts::white_dwarfs::{WhiteDwarfDebrisRules, with_white_dwarf_debris};
    use crate::planetary::hosts::young::{ProtoplanetaryDiscRules, with_protoplanetary_disc};
    use crate::planetary::surface::steps::climate::{Z0BranchRules, with_z0_branch};
    use crate::planetary::system::envelope::{
        SupercriticalEnvelopeRules, with_supercritical_envelope,
    };
    use crate::planetary::system::planet_lag::{PlanetLagRules, with_planet_lag};
    use crate::substance::helmholtz::{CompressionRules, with_compression};
    let run = move || with_z0_branch(Z0BranchRules::Kept, run);
    let run = move || with_melt_oxygen(MeltOxygenRules::Published, run);
    let run = move || with_substellar_hosts(SubstellarHostRules::BrownDwarfs, run);
    let run = move || with_rogue_planets(RoguePlanetRules::Bodies, run);
    let run = move || with_locked_state(LockedStateRules::Eyeball, run);
    let run = move || with_sea_deck(SeaDeckRules::Dry, run);
    let run = move || with_planet_lag(PlanetLagRules::Airless, run);
    let run = move || with_fall_radius(FallRadiusRoot::FalsePosition, run);
    let run = move || with_compression(CompressionRules::Settled, run);
    let run = move || with_cap_balance(CapBalanceRules::Balanced, run);
    let run = move || with_stripping(StripRules::Stripped, run);
    let run = move || with_pass_stop(PassStop::OwnResidual, run);
    let run = move || with_line_switch(LineSwitchRules::OneSwitch, run);
    let run = move || with_pass_step(PassStep::Aitken, run);
    let run = move || with_supercritical_envelope(SupercriticalEnvelopeRules::Modelled, run);
    let run = move || with_deck_profile(DeckProfileRules::Saturation, run);
    let run = move || with_dayside_decks(DaysideDecksRules::Cleared, run);
    let run = move || with_protoplanetary_disc(ProtoplanetaryDiscRules::Disc, run);
    let run = move || with_white_dwarf_debris(WhiteDwarfDebrisRules::Debris, run);
    let run = move || with_second_generation(SecondGenerationRules::Planets, run);
    let run = move || with_formation_draws(FormationDrawRules::Drawn, run);
    let run = move || {
        use crate::substance::thermal::{RegolithInertiaRules, with_regolith_inertia};
        with_regolith_inertia(RegolithInertiaRules::PoreGas, run)
    };
    with_every_25_hold(|| with_locked_climate(LockedClimateRules::Deck, run))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::placement::Existence;
    use crate::id::Layer;
    use crate::time::UniverseTime;
    use crate::units::AstronomicalUnits;

    const SEED: u64 = 0x0e14_0001_d000_0005;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    fn centre() -> GalacticPosition {
        GalacticPosition::from_light_years(SAMPLE_CENTRE_LY).unwrap()
    }

    #[test]
    fn a_sample_is_the_nearest_systems_that_pass_nearest_first() {
        let galaxy = galaxy();
        let sample = sample_records(&galaxy, 400, SampleFilter::ALL).unwrap();
        assert_eq!(sample.len(), 400);
        let distances: Vec<Metres> = sample
            .iter()
            .map(|r| centre().distance_to(r.epoch_position()))
            .collect();
        assert!(distances.windows(2).all(|d| d[0] <= d[1]));
        let mut ids: Vec<SystemId> = sample.iter().map(SystemRecord::id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 400, "no system twice");
        // Every system of the five layers inside the farthest sampled distance is in the sample.
        let farthest = LightYears::from(distances[399]);
        let sphere =
            QuerySphere::new(centre(), farthest, UniverseTime::EPOCH, LightYears::ZERO).unwrap();
        let mut cell = Vec::new();
        let mut inside = 0;
        for spec in STELLAR_LAYERS {
            for key in cells_in_sphere(spec.layer(), &sphere) {
                generate_cell(&galaxy, key, &mut cell);
                inside += cell
                    .iter()
                    .filter(|r| centre().distance_to(r.epoch_position()) <= distances[399])
                    .count();
            }
        }
        assert_eq!(inside, 400);
        // A volume-limited sample is mostly M dwarfs.
        let layer_a = sample.iter().filter(|r| r.layer() == Layer::A).count();
        assert!(layer_a > 200, "{layer_a} of 400 in layer A");
        // The sample of k is the first k of a larger one.
        assert_eq!(
            sample_records(&galaxy, 150, SampleFilter::ALL).unwrap(),
            sample[..150]
        );
    }

    #[test]
    fn a_filter_keeps_its_band_and_its_test() {
        let galaxy = galaxy();
        let (lo, hi) = (SolarMasses::new(0.8), SolarMasses::new(1.2));
        let old = |r: &SystemRecord| r.age_at_epoch().value() > 1e9;
        let filter = SampleFilter::primary_masses(lo, hi).keeping(old);
        let sample = sample_records(&galaxy, 60, filter).unwrap();
        assert_eq!(sample.len(), 60);
        for record in &sample {
            let m = record.primary_initial_mass();
            assert!(lo <= m && m < hi, "{m:?}");
            assert!(old(record));
            assert_eq!(record.layer(), Layer::C);
        }
        // Only layer C can hold these masses, so only it is walked.
        assert!(!filter.may_pass(MassBand::B) && filter.may_pass(MassBand::C));
        assert!(!filter.may_pass(MassBand::D));
        // A layer-B primary may be drawn at exactly 0.75 M☉, so a band from there walks B too.
        let edge = SampleFilter::primary_masses(SolarMasses::new(0.75), hi);
        assert!(edge.may_pass(MassBand::B) && edge.may_pass(MassBand::C));
    }

    #[test]
    fn a_filter_no_layer_can_meet_is_refused() {
        let galaxy = galaxy();
        let heavy = SampleFilter::primary_masses(SolarMasses::new(200.0), SolarMasses::new(300.0));
        assert_eq!(
            sample_records(&galaxy, 1, heavy),
            Err(SampleContextsError::TooFewSystems {
                wanted: 1,
                found: 0
            })
        );
    }

    #[test]
    fn a_sample_the_largest_sphere_cannot_hold_is_refused() {
        let galaxy = galaxy();
        let none = SampleFilter::primary_masses(SolarMasses::new(0.08), SolarMasses::new(0.5))
            .keeping(|_| false);
        assert_eq!(
            sample_records(&galaxy, 1, none),
            Err(SampleContextsError::TooFewSystems {
                wanted: 1,
                found: 0
            })
        );
    }

    #[test]
    fn sampled_contexts_are_their_systems_contexts() {
        let galaxy = galaxy();
        let seed = Seed::new(SEED);
        let contexts = sample_contexts(24, seed, SampleFilter::ALL).unwrap();
        assert_eq!(contexts.len(), 24);
        let records = sample_records(&galaxy, 24, SampleFilter::ALL).unwrap();
        for (context, record) in contexts.iter().zip(&records) {
            assert_eq!(context.id(), record.id());
            assert_eq!(
                Ok(context),
                SystemContext::for_system(&galaxy, record.id()).as_ref()
            );
        }
        // The same seed, the same sample.
        assert_eq!(
            contexts,
            sample_contexts(24, seed, SampleFilter::ALL).unwrap()
        );
        // Every sampled system exists at the epoch in the solar neighbourhood's mixture.
        assert!(
            contexts
                .iter()
                .all(|c| c.existence_at(UniverseTime::EPOCH) == Existence::Exists)
        );
    }

    #[test]
    fn the_synthetic_hosts_are_the_builder_s() {
        let key = crate::galaxy::placement::CellKey::new(Layer::C, [0, 812, 0]).unwrap();
        let id = key.candidate_id(5).unwrap();
        let (m, fe_h, age) = (SolarMasses::new(0.8), Dex::new(0.1), Years::new(3e9));
        assert_eq!(
            synthetic_star(id, m, fe_h, age),
            SystemContext::builder()
                .system(id)
                .star(m)
                .fe_h(fe_h)
                .age_at_epoch(age)
                .build()
        );
        let (a, e) = (
            Metres::from(AstronomicalUnits::new(40.0)),
            Eccentricity::new(0.4).unwrap(),
        );
        let pair = synthetic_binary(id, m, SolarMasses::new(0.3), a, e, fe_h, age).unwrap();
        assert_eq!(pair.stars().len(), 2);
        assert_eq!(
            Ok(pair),
            SystemContext::builder()
                .system(id)
                .binary(m, SolarMasses::new(0.3), a, e)
                .fe_h(fe_h)
                .age_at_epoch(age)
                .build()
        );
        assert!(matches!(
            synthetic_star(id, SolarMasses::new(-1.0), fe_h, age),
            Err(BuildSystemContextError::MassOutsideRange { star: 0, .. })
        ));
    }

    /// The passes' superlinear root's switch (P14.T51.l): in force inside 25's helper, the halvings
    /// outside it and where a test sets it back.
    #[test]
    fn the_pass_root_s_hold_is_in_force_inside_25_s_helper_alone() {
        use crate::planetary::derive::passes::{PassRoot, with_pass_root};
        assert_eq!(PassRoot::in_force(), PassRoot::Halvings);
        assert_eq!(with_every_25_hold(PassRoot::in_force), PassRoot::Secant);
        assert_eq!(
            with_every_25_hold(|| with_pass_root(PassRoot::Halvings, PassRoot::in_force)),
            PassRoot::Halvings
        );
        assert_eq!(PassRoot::in_force(), PassRoot::Halvings);
    }

    /// 26's deck profile's switch (P14.T52.g): in force inside 26's helper, held outside it, inside
    /// 25's and where a test sets it back.
    #[test]
    fn the_deck_profile_hold_is_in_force_inside_26_s_helper_alone() {
        use crate::planetary::chem::condense::{DeckProfileRules, with_deck_profile};
        assert_eq!(DeckProfileRules::in_force(), DeckProfileRules::Held);
        assert_eq!(
            with_every_26_hold(DeckProfileRules::in_force),
            DeckProfileRules::Saturation
        );
        assert_eq!(
            with_every_25_hold(DeckProfileRules::in_force),
            DeckProfileRules::Held
        );
        assert_eq!(
            with_every_26_hold(|| {
                with_deck_profile(DeckProfileRules::Held, DeckProfileRules::in_force)
            }),
            DeckProfileRules::Held
        );
        assert_eq!(DeckProfileRules::in_force(), DeckProfileRules::Held);
    }

    /// 26's dayside decks' switch (P14.T52.h): in force inside 26's helper, held outside it, inside
    /// 25's and where a test sets it back.
    #[test]
    fn the_dayside_decks_hold_is_in_force_inside_26_s_helper_alone() {
        use crate::planetary::hooks::envelope_atmosphere::{DaysideDecksRules, with_dayside_decks};
        assert_eq!(DaysideDecksRules::in_force(), DaysideDecksRules::Held);
        assert_eq!(
            with_every_26_hold(DaysideDecksRules::in_force),
            DaysideDecksRules::Cleared
        );
        assert_eq!(
            with_every_25_hold(DaysideDecksRules::in_force),
            DaysideDecksRules::Held
        );
        assert_eq!(
            with_every_26_hold(|| {
                with_dayside_decks(DaysideDecksRules::Held, DaysideDecksRules::in_force)
            }),
            DaysideDecksRules::Held
        );
        assert_eq!(DaysideDecksRules::in_force(), DaysideDecksRules::Held);
    }

    /// 26's switch, the zonal search's Z₀ (R09.T13.i): in force inside 26's helper, not inside
    /// 25's, and held outside them and where a test sets it back.
    #[test]
    fn the_z0_branch_is_in_force_inside_26_s_helper_alone() {
        use crate::planetary::surface::steps::climate::{Z0BranchRules, with_z0_branch};
        assert_eq!(Z0BranchRules::in_force(), Z0BranchRules::Held);
        assert_eq!(
            with_every_26_hold(Z0BranchRules::in_force),
            Z0BranchRules::Kept
        );
        assert_eq!(
            with_every_25_hold(Z0BranchRules::in_force),
            Z0BranchRules::Held
        );
        assert_eq!(
            with_every_26_hold(|| with_z0_branch(Z0BranchRules::Held, Z0BranchRules::in_force)),
            Z0BranchRules::Held
        );
        assert_eq!(Z0BranchRules::in_force(), Z0BranchRules::Held);
    }

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        let messages = [SampleContextsError::TooFewSystems {
            wanted: 10,
            found: 3,
        }
        .to_string()];
        for message in messages {
            let first = message.chars().next().unwrap();
            assert!(!first.is_uppercase(), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
        assert!(format!("{:?}", SampleFilter::ALL).starts_with("SampleFilter"));
    }
}
