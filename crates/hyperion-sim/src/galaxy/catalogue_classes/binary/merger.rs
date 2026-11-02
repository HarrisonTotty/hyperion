//! The stellar merger class (plan 11, P11.T8.c; design notes 10 and 13): the mergers of two
//! stars inside the source horizon, each a one-shot entry placed on the 4,096 ly cells of
//! `STELLAR_MERGER` (2).
//!
//! The grid redraws every binary that falls into a carved class (P11.T7), and a pair with a living
//! star that coalesces inside the source horizon is a stellar merger, so these entries are nowhere
//! in the cells; the share matrix gives up what the class holds, its row of
//! [`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES), 9 × 10⁴ mergers in the horizon at Milky
//! Way parameters, 0.34 a year (Kochanek, Adams and Belczynski 2014, MNRAS 443, 1319, the
//! brainstorm's 0.2–0.5). The class is a Poisson process of that intensity over the density
//! components ([`ClassHosts`]), placed by thinning on the class grid of plan 09 ([`ClassProcess`]).
//!
//! # A candidate
//!
//! A candidate draws its scan marks first (`stellar::binary::scan`, [`merger_scan_marks`]): its
//! site, its merger time T uniform over the source horizon, its age, its composition, its pair of
//! main-sequence stars and its velocity. Then the orbit's orientation and phase
//! (`class.merger.binary`), on which it builds:
//!
//! - **the pair before T**, in contact as W Ursae Majoris pairs and V1309 Scorpii's progenitor are
//!   (Tylenda et al. 2011, A&A 528, A114, saw that pair merge): each star at its own track's state
//!   at T, its radius its
//!   Roche lobe's (Eggleton 1983) on the circular orbit on which the primary fills its lobe, so
//!   that the period follows from the primary's radius and the mass ratio;
//! - **the merged star from T**: plan 11's rule for two main-sequence stars (P11.T4.d; Hurley,
//!   Tout and Pols 2002, section 2.7.3 and equation 80), a main-sequence star of the pair's mass at
//!   the fractional age 0.1 Σ Mᵢ τᵢ ÷ M on its own track, rejuvenated as the engine's product is,
//!   so that a pair the engine merges and an entry agree.
//!
//! # The engine's channels, from 26
//!
//! From [`MERGER_CLASS_VERSION`] (research-leans-13 §2(c): the engine merges only some contact
//! pairs on the main sequence, and the progenitors seen are mostly evolved, as M31-LRN-2015's and
//! M101 OT2015-1's were, MacLeod et al. 2017 and Blagorodnova et al. 2017, `MERGER_SAMPLER`'s
//! doc) the scan draws a channel first, by `MERGER_SAMPLER`'s weights: a contact pair as above,
//! its mass ratio at the tidal limit, or an **evolved donor**, past its main sequence and living
//! at T, beside a main-sequence companion. The scan asks the engine itself, built at T and not
//! stepped, what the onset of transfer from the donor at its lobe does
//! (`stellar::binary::class_merger`): its stability test, then a common envelope; the candidate is
//! kept only if that merges the pair into one star. Before T the evolved donor fills its lobe on
//! a circular orbit, detached, as the engine's segment before that onset is
//! ([`MergerPair::before`]); from T it is the star the engine's envelope leaves, bit for bit.
//!
//! # The dynamical main sequence, from 27
//!
//! From [`MERGER_CHANNELS_VERSION`] (P11.T8.c's missing channel, research-leans-16 §1.1: the
//! engine's mergers of two main-sequence stars, 0.30 of its mergers, are nearly all a low-mass
//! donor's dynamical transfer, detached just before, whose mass ratio no contact pair at the tidal
//! limit has) the scan draws one of three channels, by `MERGER_SAMPLER`'s `EngineChannel` weights:
//! the two above, or a **dynamical main-sequence** merger, a low-mass (BSE's k = 0) main-sequence
//! donor at its lobe beside a lighter main-sequence companion, built at its lobe as an evolved
//! donor is. The engine's onset of transfer finds the donor past its critical ratio and merges the
//! pair dynamically (`Engine::merge_dynamically`); before T the pair is detached at the donor's
//! lobe, from T the engine's merged star, bit for bit. An evolved donor's companion is drawn by
//! its donor's kind's mark.
//!
//! # The test
//!
//! The candidate's [`BinaryTimeline::from_marks`] is the pair before T (the contact pair, or from
//! 26 a donor's detached pair at its lobe) from zero age to T and the merged star from T, built from
//! its marks. It is an entry if and only if [`carved_class`] of that timeline is
//! [`CarvedClass::StellarMerger`], the one test the grid applies to an evolved pair (design note
//! 10); a merger in the horizon of a pair with a living star always is.
//!
//! # The entry
//!
//! An entry has one member, its system, at its site at the epoch, moving at its velocity on a
//! straight line: over the source horizon a merger below 4,600 km/s stays within plan 09's 4,096 ly
//! reach of its cell. Before T it is its pair, from T the merged star, under the same ID
//! ([`MergerEntry::stars`]); its luminous red nova at T is P11.T9's one-shot event. Its record is a
//! catalogue system's, counted in its primary's layer.

use super::super::grid::{CatalogueMember, ClassProcess, site_stream};
use super::super::ids::CatalogueCellKey;
use super::super::registry::ClassId;
use super::super::system::{CatalogueMotion, CatalogueSystem};
use super::hosts::ClassHosts;
use crate::GeneratorVersion;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::placement::SystemRecord;
use crate::id::{BodyId, CatalogueSystemId};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation, roche_lobe_radius};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::binary::scan::{MergerScanMarks, donor_product, merger_scan_marks};
use crate::stellar::binary::{
    BinaryMarks, BinaryTimeline, CarvedClass, MarkedMerger, SegmentKind, carved_class,
};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{SlotKind, SystemHierarchy};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{self, Track, TrackOptions, ZCoeffs};
use crate::stellar::system::{StarModel, SystemStars, track_mass_of};
use crate::stellar::{Composition, Phase, StarState, StarStateParts};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{
    GravitationalParameter, Metres, PerCubicLightYear, Radians, SolarMasses, SolarRadii, Years,
};

/// The generator version whose stellar merger class draws the engine's channels (P11.T8.c's fix,
/// research-leans-13 §2(c)): a contact pair of main-sequence stars at the tidal limit, or an
/// evolved donor past its main sequence whose common envelope merges it with its companion, by
/// the weights of `MERGER_SAMPLER`'s channel; and whose neutron-star mergers' remnants keep the
/// pair's mass less the published ejecta (P11.T8.d's rider, research-leans-13 §2(b)).
///
/// Below it the class takes the held arm, [`MergerClassRules::ContactOnly`], the code of version
/// 24 verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `GalaxyRules::every_27_hold()`. The bump to 27 deletes this hold: [`MergerClassRules`] with its
/// `ContactOnly` arm, the galaxy rules' `merger_class` field, the scan's and the neutron-star
/// remnant's matches on it, `MergerMark::MassRatio`, and the tripwire
/// (`the_merger_class_is_held_only_until_the_27_bump`).
pub(crate) const MERGER_CLASS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// The generator version whose stellar merger class adds the engine's dynamical main-sequence
/// mergers (P11.T8.c's missing channel, research-leans-16 §1.1): a third channel, a low-mass
/// main-sequence donor at its lobe that the engine merges dynamically, by `MERGER_SAMPLER`'s
/// `EngineChannel` weights; and an evolved donor's companion by its donor's kind's mark.
///
/// Below it the class's channels take the held arm, [`MergerChannelRules::ContactAndEvolved`],
/// version 26's two verbatim (and under [`MergerClassRules::ContactOnly`], below 26, the channels
/// are never read), so every generated value keeps its bits; the tests reach the new arm through
/// `GalaxyRules::every_27_hold()`. The bump to 27 deletes this hold: [`MergerChannelRules`] with
/// its `ContactAndEvolved` arm, the galaxy rules' `merger_channels` field, the scan's match on it
/// with `MergerPick::Channel` and `MergerChannel::HELD`, and the tripwire
/// (`the_merger_channels_are_held_only_until_the_27_bump`).
pub(crate) const MERGER_CHANNELS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Which channels the stellar merger class draws under [`MergerClassRules::Channels`] (the galaxy
/// rules' `merger_channels` field, the hold of [`MERGER_CHANNELS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MergerChannelRules {
    /// Version 26's: a contact pair or an evolved donor, by `MergerPick::Channel`, an evolved
    /// donor's companion by `MergerMark::EvolvedMassRatio` whatever its kind.
    ContactAndEvolved,
    /// Version 27's: a contact pair, an evolved donor or a dynamical main-sequence donor, by
    /// `MergerPick::EngineChannel`, an evolved donor's companion by its donor's kind's mark.
    WithDynamical,
}

/// How the merger classes draw their entries (the galaxy rules' `merger_class` field, the hold of
/// [`MERGER_CLASS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MergerClassRules {
    /// Version 25's and before: every stellar merger a contact pair of main-sequence stars, its
    /// mass ratio `MergerMark::MassRatio`'s; a neutron-star merger's remnant the pair's mass.
    ContactOnly,
    /// Version 26's: the engine's channels, contact or evolved donor; a neutron-star merger's
    /// remnant the pair's mass less its ejecta.
    Channels,
}

/// The share of the merged star's own main-sequence lifetime its track is built past the end of
/// that main sequence: 5%, so that the main sequence on which the product is placed ends inside
/// the track.
const PRODUCT_REACH_MARGIN: f64 = 1.05;

/// The stellar merger class (plan 11's Provides; P11.T8.c): [`ClassId::STELLAR_MERGER`].
///
/// # Examples
///
/// A cell about the Sun-like point holds mergers, each resolving by its ID to itself, a contact
/// pair before its merger time and the merged star from it:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::StellarMergers;
/// use hyperion_sim::galaxy::catalogue_classes::{
///     CatalogueCellKey, CatalogueClassCell, ClassId, resolve_entry,
/// };
/// use hyperion_sim::stellar::binary::SegmentKind;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = StellarMergers::new(&galaxy);
/// let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [0, 6, 0])?;
/// let cell = CatalogueClassCell::generate(&galaxy, &process, key);
/// let entry = cell.entries().first().ok_or("the solar circle has mergers")?;
/// assert_eq!(resolve_entry(&galaxy, &process, entry.id())?.entry(), entry);
/// let merger = entry.value();
/// let at_t = merger.scan().merger_age();
/// assert_eq!(merger.timeline().merger_age(), Some(at_t));
/// let before = merger.timeline().state_at(at_t - hyperion_sim::units::Years::new(10.0));
/// assert_eq!(before.kind(), SegmentKind::Contact);
/// assert_eq!(merger.timeline().state_at(at_t).kind(), SegmentKind::Merged);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StellarMergers {
    hosts: ClassHosts,
}

impl StellarMergers {
    /// The class in `galaxy`.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        Self {
            hosts: ClassHosts::new(galaxy, ClassId::STELLAR_MERGER, CarvedClass::StellarMerger),
        }
    }

    /// Its hosts over the density components: the intensity, bound and site it thins on.
    #[must_use]
    pub const fn hosts(&self) -> &ClassHosts {
        &self.hosts
    }

    /// The class's expected mergers a year in the whole galaxy, before its test: its expected
    /// entries over the source horizon's L + 2H, in Julian years.
    #[must_use]
    pub fn mergers_per_year(&self, galaxy: &Galaxy) -> f64 {
        self.hosts.expected_hosts(galaxy) / horizon_years()
    }

    /// Candidate `id` of `cell`: its scan marks, then its orbit, pair and merged star.
    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<MergerCandidate> {
        let scan = merger_scan_marks(galaxy, self, cell, id, bound)?;
        Some(Self::candidate_of(galaxy, &scan))
    }

    /// The candidate whose scan marks are `scan`, a catalogue candidate's or a feature's list
    /// entry's (P11.T8.f): its orbit, its pair before T and the star it leaves.
    #[must_use]
    pub(crate) fn candidate_of(galaxy: &Galaxy, scan: &MergerScanMarks) -> MergerCandidate {
        let seed = galaxy.seed();
        let system = scan.id();
        let mut stream = Stream::open(seed, tags::CLASS_MERGER_BINARY, ObjectKey::from(system));
        let orientation = Orientation::new(
            Radians::new(math::acos(2.0 * stream.uniform() - 1.0)),
            Radians::new(std::f64::consts::TAU * stream.uniform()),
            Radians::new(std::f64::consts::TAU * stream.uniform()),
        )
        .expect("an inclination in [0, π] and angles in [0, 2π) are an orientation");
        let mean_anomaly = Radians::new(std::f64::consts::TAU * stream.uniform());
        let angles = (orientation, mean_anomaly);
        let draws = [0, 1].map(|body| StarDraws::for_star(seed, BodyId::new(system, body)));
        let rules = galaxy.rules().stellar;
        let (pair, product) = match scan.donor_separation() {
            None => (
                MergerPair::contact(scan.stars(), angles),
                main_sequence_product(scan.stars(), scan.composition(), &draws[0], rules),
            ),
            Some(separation) => (
                MergerPair::at_lobe(scan.stars(), separation, angles),
                donor_product(scan, &draws, rules).expect(
                    "the scan kept this pair because the engine's onset merges this same input",
                ),
            ),
        };
        let marks = BinaryMarks::merger(
            *scan.composition(),
            draws,
            scan.age_at_epoch(),
            MarkedMerger {
                age: scan.merger_age(),
                before: pair.before,
                stars: pair.stars,
                orbit: pair.orbit,
                transfer_rate: None,
                product,
            },
        )
        .expect("a merger age past the stars' arrival is a marked merger");
        MergerCandidate {
            scan: *scan,
            pair,
            product,
            marks,
        }
    }

    /// The class's test on `candidate`: its timeline from its marks, kept if [`carved_class`]
    /// gives the class.
    #[must_use]
    pub(crate) fn test(candidate: MergerCandidate) -> Option<MergerEntry> {
        let MergerCandidate {
            scan,
            pair,
            product,
            marks,
        } = candidate;
        let timeline = BinaryTimeline::from_marks(marks);
        if carved_class(&timeline, scan.age_at_epoch()) != Some(CarvedClass::StellarMerger) {
            return None;
        }
        let record = SystemRecord::from_parts(
            scan.id(),
            *scan.position(),
            scan.host().origin(ClassId::STELLAR_MERGER),
            scan.population(),
            scan.primary_mass(),
            scan.age_at_epoch(),
        );
        Some(MergerEntry {
            record,
            motion: CatalogueMotion::straight(*scan.position(), scan.velocity()),
            scan,
            pair,
            product,
            timeline,
        })
    }
}

/// The source horizon's length, L + 2H, Julian years.
fn horizon_years() -> f64 {
    crate::time::SourceHorizon::END
        .checked_since(crate::time::SourceHorizon::START)
        .expect("the source horizon runs forward")
        .as_julian_years_f64()
}

/// A stellar merger candidate the scan kept, with every mark its test reads.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerCandidate {
    scan: MergerScanMarks,
    pair: MergerPair,
    product: StarState,
    marks: BinaryMarks,
}

/// A stellar merger's pair before T (P11.T8.c): two main-sequence stars in contact on a circular
/// orbit, each filling its Roche lobe; or, from 26, a donor filling its lobe on a circular orbit
/// about its main-sequence companion, detached, as the engine's segment before the onset leaves
/// it: an evolved donor, or from 27 a low-mass main-sequence donor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MergerPair {
    before: SegmentKind,
    stars: [StarState; 2],
    orbit: KeplerElements,
}

impl MergerPair {
    /// What the pair is before T: [`SegmentKind::Contact`] for a contact pair,
    /// [`SegmentKind::Detached`] for a donor at its lobe.
    #[must_use]
    pub const fn before(&self) -> SegmentKind {
        self.before
    }

    /// Each star's marked state, primary first: its own track's at T, a contact pair's radii its
    /// Roche lobes'.
    #[must_use]
    pub const fn stars(&self) -> &[StarState; 2] {
        &self.stars
    }

    /// The relative orbit, circular, on which the primary fills its Roche lobe.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// A donor's pair at its lobe of `stars` (their own states at T, the donor first) on its
    /// circular orbit of `separation`, the scan's, on which the donor fills its lobe, oriented and
    /// phased by `angles`; each star at its own radius.
    #[must_use]
    fn at_lobe(
        stars: &[StarState; 2],
        separation: Metres,
        (orientation, mean_anomaly): (Orientation, Radians),
    ) -> Self {
        let [m1, m2] = stars.map(|s| s.mass().value());
        let orbit = KeplerElements::from_semi_major_axis(
            separation,
            GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
            Eccentricity::CIRCULAR,
            orientation,
            mean_anomaly,
        )
        .expect("a star's radius about a star or two is an orbit");
        Self {
            before: SegmentKind::Detached,
            stars: *stars,
            orbit,
        }
    }

    /// The contact pair of `stars` (their own states at T), oriented and phased by `angles`: the
    /// separation a = R₁ ÷ f(M₁ ÷ M₂), with f Eggleton's lobe over the separation, and each
    /// star's radius its lobe on it.
    fn contact(
        stars: &[StarState; 2],
        (orientation, mean_anomaly): (Orientation, Radians),
    ) -> Self {
        let [m1, m2] = stars.map(|s| s.mass().value());
        let unit = Metres::new(1.0);
        let primary_share = roche_lobe_radius(m1 / m2, unit).value();
        let separation = Metres::new(stars[0].radius().value() * SOLAR_RADIUS_M / primary_share);
        let orbit = KeplerElements::from_semi_major_axis(
            separation,
            GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
            Eccentricity::CIRCULAR,
            orientation,
            mean_anomaly,
        )
        .expect("a separation of stellar radii about a star or two is an orbit");
        let lobe =
            |q: f64| SolarRadii::new(roche_lobe_radius(q, separation).value() / SOLAR_RADIUS_M);
        Self {
            before: SegmentKind::Contact,
            stars: [
                with_radius(&stars[0], lobe(m1 / m2)),
                with_radius(&stars[1], lobe(m2 / m1)),
            ],
            orbit,
        }
    }
}

/// `state` with its radius `radius`.
fn with_radius(state: &StarState, radius: SolarRadii) -> StarState {
    StarState::new(StarStateParts {
        phase: state.phase(),
        age: state.age(),
        mass: state.mass(),
        core_mass: state.core_mass(),
        luminosity: state.luminosity(),
        radius,
        mass_loss_rate: state.mass_loss_rate(),
        phase_fraction: state.phase_fraction(),
    })
}

/// The merged star of two main-sequence stars in `stars` (P11.T4.d's rule; Hurley, Tout and Pols
/// 2002, section 2.7.4 and equation 80): a main-sequence star of their mass M at the fractional age
/// τ = 0.1 Σ Mᵢ τᵢ ÷ M, on its own track at `composition` with the primary's `draws` under
/// `rules`, as the engine's `main_sequence_merger` places it.
///
/// # Panics
///
/// Never: a main-sequence track of 0.2–150 M☉ holds its main sequence.
#[must_use]
pub(crate) fn main_sequence_product(
    stars: &[StarState; 2],
    composition: &Composition,
    draws: &StarDraws,
    rules: StellarRules,
) -> StarState {
    let total = stars[0].mass().value() + stars[1].mass().value();
    let tau = 0.1
        * stars
            .iter()
            .fold(0.0, |sum, s| sum + s.mass().value() * s.phase_fraction())
        / total;
    let mass = track_mass_of(SolarMasses::new(total));
    let coeffs = ZCoeffs::new(composition.z_fit());
    let reach = sse::main_sequence_start(mass, composition)
        + PRODUCT_REACH_MARGIN * sse::main_sequence_lifetime(&coeffs, false, mass.value());
    let options = TrackOptions::default().with_rules(rules);
    let track = Track::to_age_with(mass, composition, draws, options, Years::new(reach));
    let at = track
        .age_in_phase(Phase::MainSequence, tau)
        .expect("a track built past its main sequence holds it");
    track.state_at(Years::new(at))
}

/// A stellar merger entry (plan 11, P11.T8.c): a merger in the source horizon.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerEntry {
    record: SystemRecord,
    motion: CatalogueMotion,
    scan: MergerScanMarks,
    pair: MergerPair,
    product: StarState,
    timeline: BinaryTimeline,
}

impl MergerEntry {
    /// The record of the entry's system, of origin
    /// [`SystemOrigin::CatalogueSystem`](crate::galaxy::placement::SystemOrigin::CatalogueSystem)
    /// of the class, at its site at the epoch; for a feature's list entry (P11.T8.f), of its
    /// feature's or the galactic centre's member origin, at its list position.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// How the system moves: at its velocity, on one straight line.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// Its scan marks, the first it drew: T, the age, the composition and the pair's masses.
    #[must_use]
    pub const fn scan(&self) -> &MergerScanMarks {
        &self.scan
    }

    /// The pair before T.
    #[must_use]
    pub const fn pair(&self) -> &MergerPair {
        &self.pair
    }

    /// The merged star from T, held at its state then.
    #[must_use]
    pub const fn product(&self) -> &StarState {
        &self.product
    }

    /// The pair's timeline, from its marks: the contact pair to T, the merged star from it.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        &self.timeline
    }

    /// The system's stars: the primary and the secondary, each on its own model at its initial
    /// mass and the system's age, on the contact orbit, and the pair's timeline, which gives both
    /// stars' states, and from T the merged star in the primary's place.
    ///
    /// # Panics
    ///
    /// Never: the scan's masses are stars' and its age is finite.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        let system = self.record.id();
        let age = self.scan.age_at_epoch();
        let rules = galaxy.rules().stellar;
        let masses = [self.scan.primary_mass(), self.scan.secondary_mass()];
        let stars = [0_u16, 1].into_iter().zip(masses).map(|(body, mass)| {
            let draws = StarDraws::for_star(galaxy.seed(), BodyId::new(system, body));
            StarModel::new_under(mass, *self.scan.composition(), draws, age, rules)
                .expect("a merger's stars are of 0.1–150 M☉ at a finite age")
        });
        let hierarchy = SystemHierarchy::binary_on(
            system,
            masses[0],
            (masses[1], SlotKind::Star),
            self.pair.orbit,
        );
        SystemStars::of_marked_pair(
            galaxy.seed(),
            self.record,
            hierarchy,
            stars.collect(),
            self.timeline.clone(),
        )
    }
}

impl ClassProcess for StellarMergers {
    type Candidate = MergerCandidate;
    type Entry = MergerEntry;

    fn class(&self) -> ClassId {
        ClassId::STELLAR_MERGER
    }

    fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        self.hosts.bound(galaxy, cell)
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<MergerCandidate> {
        self.candidate(galaxy, cell, id, bound)
    }

    fn test(&self, _: &Galaxy, candidate: MergerCandidate) -> Option<MergerEntry> {
        Self::test(candidate)
    }

    fn members(&self, _: &MergerEntry) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<MergerEntry>) -> CatalogueSystem {
        let entry = member.entry().value();
        CatalogueSystem::new(entry.record, entry.motion).with_class_stars()
    }

    fn stars(&self, galaxy: &Galaxy, member: &CatalogueMember<MergerEntry>) -> Option<SystemStars> {
        Some(member.entry().value().stars(galaxy))
    }

    /// The site, where the entry sits at the epoch, moving at its component's draw.
    fn epoch_place(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
    ) -> Option<GalacticPosition> {
        Some(site_stream(galaxy, cell, id, tags::CLASS_MERGER_SITE).0)
    }

    fn reads_place_first(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod channels_hold;
#[cfg(test)]
mod engine_channels_hold;
#[cfg(test)]
mod reference;
#[cfg(test)]
mod tests;
