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
//! - [`sample_rogue_records`] takes a galaxy's rogue planets about the same point, in a fixed walk
//!   of layer G's cells.
//! - For this crate's tests alone, `in_shares` runs a test's shares on threads of its own with the
//!   test's holds in force on each (`held`).
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
use crate::galaxy::placement::{CellKey, LayerSpec, STELLAR_LAYERS, SystemRecord, generate_cell};
use crate::galaxy::query::{QuerySphere, cells_in_sphere, count_cells_in_sphere};
use crate::galaxy::{BuildGalaxyError, Galaxy};
use crate::id::{Layer, SystemId};
use crate::orbit::Eccentricity;
use crate::time::UniverseTime;
use crate::units::{
    Dex, EarthMasses, Kelvin, LightYears, Metres, MetresPerSecondSquared, Pascals, SolarMasses,
    Watts, Years,
};

#[cfg(test)]
mod held;
#[cfg(test)]
pub(crate) use held::{HeldRules, in_shares};

/// `run`'s result with every planetary hold of version 30 in force on this thread (train 30): the
/// cold giants' chromophore mixed in its haze's gas
/// ([`CHROMOPHORE_MIXING_VERSION`](super::chem::photochem::CHROMOPHORE_MIXING_VERSION)); train 29
/// holds no planetary rule. A test that needs the galaxy's holds as well builds its galaxy inside it
/// with `GalaxyRules::every_29_hold()`, whose holds ride the galaxy (none is at 30).
#[cfg(test)]
pub(crate) fn with_every_30_hold<R>(run: impl FnOnce() -> R) -> R {
    use super::chem::photochem::{ChromophoreRules, with_chromophore_mixing};
    with_chromophore_mixing(ChromophoreRules::HazeMixing, run)
}

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
/// density. It is also the most cells [`sample_rogue_records`] walks.
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

/// The cell of layer G's 4 ly cells that holds [`SAMPLE_CENTRE_LY`]'s 26,000 ly along +y.
const ROGUE_CENTRE_CELL_Y: i32 = 6_500;

/// The first `n` rogue planets of `galaxy`'s layer G about the solar circle, in a fixed walk of
/// its cells: a slab two cells thick about the plane, ring by ring outwards from the cell at the
/// Sun-like point ([`SAMPLE_CENTRE_LY`]), each cell's records in their order.
///
/// They are the galaxy's own rogue planets, their masses drawn by Sumi et al.'s (2023) law, so few
/// are giants. The answer is a pure function of the galaxy and `n`, and the first `k` of it are the
/// sample of `k`. A record's context is [`SystemContext::from_record`]'s, which is what
/// [`SystemContext::for_system`] gives its ID.
///
/// # Errors
///
/// [`SampleContextsError::TooFewSystems`] if the walk's first [`MAX_SAMPLE_CELLS`] cells hold
/// fewer than `n` rogue planets.
///
/// # Panics
///
/// Never: the walk's first [`MAX_SAMPLE_CELLS`] cells lie within 181 cells (724 ly) of the
/// Sun-like point's, inside the root cube.
pub fn sample_rogue_records(
    galaxy: &Galaxy,
    n: usize,
) -> Result<Vec<SystemRecord>, SampleContextsError> {
    let mut found = Vec::with_capacity(n);
    let mut cell = Vec::new();
    let mut walked = 0_u64;
    let mut ring = 0_i32;
    loop {
        for x in -ring..=ring {
            for y in -ring..=ring {
                if x.abs().max(y.abs()) != ring {
                    continue;
                }
                for z in [-1, 0] {
                    if found.len() >= n {
                        found.truncate(n);
                        return Ok(found);
                    }
                    if walked == MAX_SAMPLE_CELLS {
                        return Err(SampleContextsError::TooFewSystems {
                            wanted: n,
                            found: found.len(),
                        });
                    }
                    walked += 1;
                    let key = CellKey::new(Layer::RoguePlanet, [x, ROGUE_CENTRE_CELL_Y + y, z])
                        .expect("the walk's cells lie within 181 cells of the solar circle's");
                    generate_cell(galaxy, key, &mut cell);
                    found.extend(cell.iter().copied());
                }
            }
        }
        ring += 1;
    }
}

/// [`sample_contexts`], [`sample_records`] or [`sample_rogue_records`] could not draw its sample.
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

/// A pair's history before an age, by its worst event: the classes the binary hosts' census
/// counts (P14.T1.e), worst first.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PairHistory {
    /// A common envelope that ended in the pair's coalescence.
    CommonEnvelopeMerger,
    /// A common envelope the pair survived, and a merger after it.
    CommonEnvelopeThenMerger,
    /// A common envelope the pair survived, and no merger.
    PostCommonEnvelope,
    /// A merger with no common envelope before it.
    Merger,
    /// Stable transfer, with no common envelope and no merger.
    StableTransfer,
    /// Contact, and nothing worse.
    Contact,
    /// None of these: a detached pair, or one whose only events are supernovae.
    Detached,
}

#[cfg(test)]
impl PairHistory {
    /// What `timeline` holds up to `age`, Julian years since the stars' onset of collapse.
    #[must_use]
    pub(crate) fn of(timeline: &crate::stellar::binary::BinaryTimeline, age: Years) -> Self {
        use crate::stellar::binary::SegmentKind;
        let segments: Vec<SegmentKind> = timeline
            .segments()
            .iter()
            .take_while(|s| s.start() <= age)
            .map(crate::stellar::binary::Segment::kind)
            .collect();
        let has = |kind: SegmentKind| segments.contains(&kind);
        let merged = segments.iter().position(|&k| k == SegmentKind::Merged);
        let envelope = has(SegmentKind::CommonEnvelope);
        match merged {
            Some(m) if m > 0 && segments[m - 1] == SegmentKind::CommonEnvelope => {
                Self::CommonEnvelopeMerger
            }
            Some(_) if envelope => Self::CommonEnvelopeThenMerger,
            Some(_) => Self::Merger,
            None if envelope => Self::PostCommonEnvelope,
            None if segments
                .iter()
                .any(|k| matches!(k, SegmentKind::StableTransfer { .. })) =>
            {
                Self::StableTransfer
            }
            None if has(SegmentKind::Contact) => Self::Contact,
            None => Self::Detached,
        }
    }
}

/// The first of the `n` systems of `galaxy` nearest [`SAMPLE_CENTRE_LY`] with a pair the binary
/// engine ran whose history before the system's age at the epoch is `history` and for which
/// `also` holds, with the pair's index among the system's pairs; `None` if no such system is
/// among them.
#[cfg(test)]
pub(crate) fn first_paired_system(
    galaxy: &Galaxy,
    n: usize,
    history: PairHistory,
    also: impl Fn(&crate::stellar::system::SystemStars, usize) -> bool,
) -> Option<(crate::stellar::system::SystemStars, usize)> {
    use crate::stellar::system::SystemStars;
    sample_records(galaxy, n, SampleFilter::ALL)
        .expect("the Sun-like point's sphere holds the sample")
        .iter()
        .filter(|record| record.kind() != crate::galaxy::placement::SystemKind::RoguePlanet)
        .map(|record| SystemStars::generate(galaxy, record))
        .find_map(|stars| {
            let age = stars.record().age_at_epoch();
            let pair = stars
                .pairs()
                .iter()
                .position(|pair| PairHistory::of(pair.timeline(), age) == history)?;
            also(&stars, pair).then_some((stars, pair))
        })
}

/// The registry's network ([`Network::registry`]), built once a process, for the tests that solve
/// many envelopes (P14.T52.a: build it once).
#[cfg(test)]
#[must_use]
pub(crate) fn network() -> &'static Network {
    use std::sync::OnceLock;

    static NETWORK: OnceLock<Network> = OnceLock::new();
    NETWORK.get_or_init(|| Network::registry().expect("the registry's network builds"))
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

    /// The rogue planets' walk starts at the cell that holds the Sun-like point, takes layer G's
    /// records alone, and its first `k` are the sample of `k`.
    #[test]
    fn a_rogue_sample_walks_out_from_the_sun_like_point() {
        let galaxy = galaxy();
        assert_eq!(
            CellKey::containing(Layer::RoguePlanet, &centre()).unwrap(),
            CellKey::new(Layer::RoguePlanet, [0, ROGUE_CENTRE_CELL_Y, 0]).unwrap()
        );
        let sample = sample_rogue_records(&galaxy, 200).unwrap();
        assert_eq!(sample.len(), 200);
        assert!(
            sample
                .iter()
                .all(|r| r.kind() == crate::galaxy::placement::SystemKind::RoguePlanet)
        );
        assert_eq!(sample_rogue_records(&galaxy, 80).unwrap(), sample[..80]);
        assert_eq!(sample_rogue_records(&galaxy, 0).unwrap(), Vec::new());
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
