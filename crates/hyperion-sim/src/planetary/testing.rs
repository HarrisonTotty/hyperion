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
/// system reads the version's records. It is one-way and process-wide, so a binary that calls it
/// calls it at the start of every test, and holds no test that asks for the free-floating hosts at
/// 25. Only a test binary calls this. 26's bump deletes it with the holds.
#[cfg(feature = "testing")]
pub fn admit_free_floating_hosts_in_this_process() {
    super::hosts::substellar::admit_free_floating_in_this_process();
}

/// `run`'s result with every landed hold at generator version 26 in force on this thread, for a
/// test that asserts a 26 figure: the holds at 26, each with its switch, which sets its rules back
/// as it returns, a panic in `run` included. A test may set one hold back inside it to compare 26
/// with and without it.
///
/// The holds at 26:
///
/// - a molten body's surface on its published air, `MELT_OXYGEN_VERSION` (P14.T53.g), at
///   `MeltOxygenRules::Published` through `with_melt_oxygen`.
/// - a free-floating brown dwarf's planets, `SUBSTELLAR_HOSTS_VERSION` (P14.T27.a), at
///   `SubstellarHostRules::BrownDwarfs` through `with_substellar_hosts`.
/// - a rogue planet's bodies, `ROGUE_PLANET_HOSTS_VERSION` (P14.T27.b), at
///   `RoguePlanetRules::Bodies` through `with_rogue_planets`.
/// - the CO₂ cap's energy balance, `CAP_BALANCE_VERSION` (P14.T53.h), at
///   `CapBalanceRules::Balanced` through `with_cap_balance`.
/// - the 100 Pa line read once, `LINE_SWITCH_VERSION` (P14.T53.i), at `LineSwitchRules::OneSwitch`
///   through `with_line_switch`: a sub-line solve whose last step read the bulk climate held once
///   more over the airless span its record publishes.
/// - a regolith's pore gas, `PORE_GAS_VERSION` (P14.T49.l), at `RegolithInertiaRules::PoreGas`
///   through `with_regolith_inertia`: the gas in a ground's pores at its column's pressure, and the
///   outer system's fluffy amorphous ice as an airless lid's water-ice form.
///
/// A system or a record is read under the rules in force when it is generated: one generated
/// outside the helper (the tests' caches, `whole` and `golden_systems`, are the version's) is not
/// 26's, so a test generates afresh inside it. A later hold at 26 adds its switch here; 26's bump
/// deletes this helper with its holds.
#[cfg(test)]
pub(crate) fn with_every_26_hold<R>(run: impl FnOnce() -> R) -> R {
    use crate::planetary::derive::atmosphere::{
        LineSwitchRules, MeltOxygenRules, with_line_switch, with_melt_oxygen,
    };
    use crate::planetary::hooks::surface::{CapBalanceRules, with_cap_balance};
    use crate::planetary::hosts::substellar::{
        RoguePlanetRules, SubstellarHostRules, with_rogue_planets, with_substellar_hosts,
    };
    let run = move || with_melt_oxygen(MeltOxygenRules::Published, run);
    let run = move || with_substellar_hosts(SubstellarHostRules::BrownDwarfs, run);
    let run = move || with_rogue_planets(RoguePlanetRules::Bodies, run);
    let run = move || with_cap_balance(CapBalanceRules::Balanced, run);
    let run = move || with_line_switch(LineSwitchRules::OneSwitch, run);
    let run = move || {
        use crate::substance::thermal::{RegolithInertiaRules, with_regolith_inertia};
        with_regolith_inertia(RegolithInertiaRules::PoreGas, run)
    };
    run()
}

/// `run`'s result with every 26 hold ([`with_every_26_hold`]) and those retargeted from 26 to 27
/// in force on this thread (train 26, 2026-11-01: P14.T49.m's icy regolith and P14.T24.h.b's minor
/// clouds, organic sediment, Rayleigh adding and haze layer): the rules their tests were measured
/// under while they were held at 26. The 27 bump deletes it with them.
#[cfg(test)]
pub(crate) fn with_every_26_hold_with_its_retargets<R>(run: impl FnOnce() -> R) -> R {
    use crate::planetary::derive::photometry::area_mix::{
        HazeLayerRules, IcyRegolithRules, MinorCloudsRules, OrganicSedimentRules,
        RayleighAddingRules, with_haze_layer, with_icy_regolith, with_minor_clouds,
        with_organic_sediment, with_rayleigh_adding,
    };
    let run = move || with_icy_regolith(IcyRegolithRules::Mixture, run);
    let run = move || with_minor_clouds(MinorCloudsRules::OwnShare, run);
    let run = move || with_organic_sediment(OrganicSedimentRules::Sediment, run);
    let run = move || with_rayleigh_adding(RayleighAddingRules::Added, run);
    let run = move || with_haze_layer(HazeLayerRules::Absorbing, run);
    with_every_26_hold(run)
}

/// `run`'s result with every landed hold at generator version 27 in force on this thread, for a
/// test that asserts a 27 figure: every 26 hold ([`with_every_26_hold`]) and those at 27, each with
/// its switch, which sets its rules back as it returns, a panic in `run` included. A test may set
/// one hold back inside it to compare 27 with and without it.
///
/// The holds at 27:
///
/// - a body's bulk iron with its solids' minor oxides and following its core,
///   `MINOR_OXIDES_VERSION` (P14.T51.k), at `MinorOxidesRules::Chondritic` through
///   `with_minor_oxides`: the mantle `FeO` and the secondary crust that read it.
/// - the equilibrium network's further carriers, `CARRIERS_VERSION` (P14.T52.e), at
///   `CarriersRules::Carried` through `with_carriers`: the registry's rows from 115 on, which the
///   network carries, P and Cl solved.
/// - a body's resources by their ratios to its rock's α elements, `RESOURCE_RATIOS_VERSION`
///   (P14.T26's ratios), at `ResourceRatiosRules::Ratios` through `with_resource_ratios`.
/// - an ocean-branch moon's porous rock core, `POROUS_CORE_VERSION` (P14.T56.e), at
///   `PorousCoreRules::Porous` through `with_porous_core`: a core under its compaction pressure
///   dissipates, and its moon's membrane floats on it.
/// - a rocky body's core share by its host's iron, `CORE_SHARE_HOST_VERSION` (P14.T51.k's fix),
///   at `CoreShareRules::Host` through `with_core_share`: its draw, ruling 58's floor and the
///   window's dry top shifted by its host's Fe/(Mg + Si), and P14.T51.k's factor reading its core
///   share about the Sun's ratios.
/// - a regular moon's eccentricity at its age, `ECCENTRICITY_DAMPING_VERSION` (P14.T55.j.a), at
///   `EccentricityDampingRules::Damped` through `with_eccentricity_damping`: a forced
///   eccentricity only in a resonance that holds, at its budget, and a free one damped by its
///   moon's own heat.
/// - a moon's surface that radiates its internal heat, `SURFACE_HEAT_VERSION` (P14.T55.j), at
///   `SurfaceHeatRules::Radiated` through `with_surface_heat`: its tides' shell read at
///   σ `T_s`⁴ = σ `T_eq`⁴ + F.
/// - a gas envelope's own Bond albedo, `ENVELOPE_BOND_VERSION` (P14.T24.h.b part 5), at
///   `EnvelopeBondRules::Own` through `with_envelope_bond`: its column's above its floor, in place
///   of T51.b's class table, with part 6's chromophore at the disc's column, the haze above a
///   Jupiter-like ammonia cloud and κ₁ refitted at Jupiter's own albedo, part 7's CH₄ deck drawn
///   as its methane layer, and part 8's κ₁ falling with a cold envelope's effective temperature.
/// - the coarse pass's seasonal runs extrapolated by RRE(2), `ORBIT_EXTRAPOLATION_VERSION`
///   (rendering plan R09, I13), at `OrbitExtrapolationRules::ReducedRank` through
///   `with_orbit_extrapolation`: the climate's orbits to their stop, and so every wet world's field.
/// - the icy lid's regolith, `ICY_REGOLITH_VERSION` (P14.T49.m), at `IcyRegolithRules::Mixture`
///   through `with_icy_regolith`: an airless or tenuous body's `H2O` regolith ice with its rock's
///   lag where its tidal heat does not resurface it.
/// - a minor condensable's own cloud, `MINOR_CLOUDS_VERSION` (P14.T24.h.b), at
///   `MinorCloudsRules::OwnShare` through `with_minor_clouds`: a cloud whose condensable is not
///   water covers Titan's measured share.
/// - the organic sediment, `ORGANIC_SEDIMENT_VERSION` (P14.T24.h.b), at
///   `OrganicSedimentRules::Sediment` through `with_organic_sediment`: a hazy air's fallout covers
///   its share of the regolith and the `H2O` ice.
/// - the Rayleigh layer added, `RAYLEIGH_ADDING_VERSION` (P14.T24.h.b), at
///   `RayleighAddingRules::Added` through `with_rayleigh_adding`: the air's layer over the area mix
///   by the adding rule, Earth's cloud re-split.
/// - the haze layer, `HAZE_LAYER_VERSION` (P14.T24.h.b), at `HazeLayerRules::Absorbing` through
///   `with_haze_layer`: a hazy air's haze an absorbing layer over the rest of its disc, its
///   absorption the anti-greenhouse.
///
/// As under [`with_every_26_hold`], a system or a record is read under the rules in force when
/// it is generated, so a test generates afresh inside it; the carriers' rules are read when a
/// system's equilibrium network is first built, lazily, so a test reads its envelopes inside the
/// switch too. A later hold at 27 adds its switch here; 26's bump, which deletes
/// [`with_every_26_hold`], leaves this helper its 27 switches alone, and 27's bump deletes it with
/// its holds.
///
/// The galaxy's own holds at 27 travel with its rules, not the thread: a test builds its galaxy
/// under `GalaxyRules::every_27_hold()` (P15.T11.b's `orphan_streams`, P11.T8.c's
/// `merger_channels`) and runs inside this helper for the planetary ones.
#[cfg(test)]
pub(crate) fn with_every_27_hold<R>(run: impl FnOnce() -> R) -> R {
    use crate::planetary::derive::photometry::area_mix::{
        HazeLayerRules, IcyRegolithRules, MinorCloudsRules, OrganicSedimentRules,
        RayleighAddingRules, with_haze_layer, with_icy_regolith, with_minor_clouds,
        with_organic_sediment, with_rayleigh_adding,
    };
    use crate::planetary::derive::photometry::envelope_bond::{
        EnvelopeBondRules, with_envelope_bond,
    };
    use crate::planetary::derive::rocky::{CoreShareRules, with_core_share};
    use crate::planetary::hooks::crust::{MinorOxidesRules, with_minor_oxides};
    use crate::planetary::hooks::resources::{ResourceRatiosRules, with_resource_ratios};
    use crate::planetary::moons::eccentricity::{
        EccentricityDampingRules, with_eccentricity_damping,
    };
    use crate::planetary::moons::tides::{
        PorousCoreRules, SurfaceHeatRules, with_porous_core, with_surface_heat,
    };
    use crate::planetary::surface::steps::climate::{
        OrbitExtrapolationRules, with_orbit_extrapolation,
    };
    use crate::substance::{CarriersRules, with_carriers};
    let run = move || with_orbit_extrapolation(OrbitExtrapolationRules::ReducedRank, run);
    let run = move || with_core_share(CoreShareRules::Host, run);
    let run = move || with_carriers(CarriersRules::Carried, run);
    let run = move || with_resource_ratios(ResourceRatiosRules::Ratios, run);
    let run = move || with_porous_core(PorousCoreRules::Porous, run);
    let run = move || with_eccentricity_damping(EccentricityDampingRules::Damped, run);
    let run = move || with_surface_heat(SurfaceHeatRules::Radiated, run);
    let run = move || with_envelope_bond(EnvelopeBondRules::Own, run);
    let run = move || with_icy_regolith(IcyRegolithRules::Mixture, run);
    let run = move || with_minor_clouds(MinorCloudsRules::OwnShare, run);
    let run = move || with_organic_sediment(OrganicSedimentRules::Sediment, run);
    let run = move || with_rayleigh_adding(RayleighAddingRules::Added, run);
    let run = move || with_haze_layer(HazeLayerRules::Absorbing, run);
    with_every_26_hold(|| with_minor_oxides(MinorOxidesRules::Chondritic, run))
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

    /// 27's carriers' switch (P14.T52.e): in force inside 27's helper, held outside it, inside
    /// 26's and where a test sets it back.
    #[test]
    fn the_carriers_hold_is_in_force_inside_27_s_helper_alone() {
        use crate::substance::{CarriersRules, with_carriers};
        assert_eq!(CarriersRules::in_force(), CarriersRules::Held);
        assert_eq!(
            with_every_27_hold(CarriersRules::in_force),
            CarriersRules::Carried
        );
        assert_eq!(
            with_every_26_hold(CarriersRules::in_force),
            CarriersRules::Held
        );
        assert_eq!(
            with_every_27_hold(|| with_carriers(CarriersRules::Held, CarriersRules::in_force)),
            CarriersRules::Held
        );
        assert_eq!(CarriersRules::in_force(), CarriersRules::Held);
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
