//! The neutron-star merger class (plan 11, P11.T8.d; design notes 10 and 13): the mergers of two
//! compact stars, one at least a neutron star, inside the source horizon, each a one-shot entry
//! placed on the 4,096 ly cells of `NEUTRON_STAR_MERGER` (3).
//!
//! The grid redraws every binary that falls into a carved class (P11.T7), and a pair of compact
//! stars with a neutron star that coalesces inside the source horizon is a neutron-star merger, so
//! these entries are nowhere in the cells; the share matrix gives up what the class holds, its row
//! of [`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES), ten mergers in the horizon at Milky
//! Way parameters, the brainstorm's "about 10": some 4 × 10⁻⁵ a year, inside the 9 × 10⁻⁷–1.5 × 10⁻⁴
//! a year a Milky Way-equivalent galaxy has at GWTC-3's 10–1,700 Gpc⁻³ yr⁻¹ (Abbott et al. 2023,
//! PRX 13, 011048) and 0.0116 such galaxies per Mpc³ (Kopparapu et al. 2008, ApJ 675, 1459), both
//! recalled, not read at source. The class is a Poisson process of that intensity over the
//! density components ([`ClassHosts`]), placed by thinning on the class grid of plan 09
//! ([`ClassProcess`]).
//!
//! # A candidate
//!
//! A candidate draws its scan marks first (`stellar::binary::scan`, [`nsm_scan_marks`]): its site,
//! its merger time T uniform over the source horizon, its age, its composition, its pair and its
//! velocity. Then the orbit's orientation and phase (`class.nsm.binary`), on which it builds:
//!
//! - **the pair before T**, detached and inspiralling by its gravitational waves alone (Peters
//!   1964): the circular orbit that coalesces at T from the start of the source horizon,
//!   [`peters_separation_for`] of that time, each compact star held at its state at T (a neutron
//!   star at plan 06's radius and Hurley, Pols and Tout's cooling luminosity at its age since its
//!   progenitor's death, a black hole at its Schwarzschild radius);
//! - **the remnant from T**, by the total mass: plan 11's rule for two compact stars (P11.T4.d),
//!   a black hole of the pair's mass above plan 06's heaviest neutron star.
//!
//! # The test
//!
//! The candidate's [`BinaryTimeline::from_marks`] is the inspiral from zero age to T and the
//! remnant from T, without the engine. It is an entry if and only if [`carved_class`] of that
//! timeline is [`CarvedClass::NeutronStarMerger`], the one test the grid applies to an evolved
//! pair (design note 10).
//!
//! # The entry
//!
//! An entry has one member, its system, at its site at the epoch, moving at its velocity on a
//! straight line. Before T it is the inspiralling pair, from T the remnant, under the same ID
//! ([`NsmEntry::stars`]); its kilonova at T is P11.T9's one-shot event. Its record is a catalogue
//! system's, counted in its primary progenitor's layer, E.

use super::super::grid::{CatalogueMember, ClassProcess};
use super::super::ids::CatalogueCellKey;
use super::super::registry::ClassId;
use super::super::system::{CatalogueMotion, CatalogueSystem};
use super::hosts::ClassHosts;
use crate::galaxy::Galaxy;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::id::{BodyId, CatalogueSystemId, SystemId};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation, peters_separation_for};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::binary::scan::{NsmScanMarks, nsm_scan_marks};
use crate::stellar::binary::{
    BinaryMarks, BinaryTimeline, CarvedClass, MarkedMerger, SegmentKind, carved_class,
};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{SlotKind, SystemHierarchy};
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::neutron_star::hpt_luminosity;
use crate::stellar::remnant::structure::{black_hole_radius, neutron_star_radius};
use crate::stellar::system::{StarModel, SystemStars};
use crate::stellar::{Phase, StarState, StarStateParts};
use crate::time::SourceHorizon;
use crate::units::{
    GravitationalParameter, PerCubicLightYear, Radians, SolarLuminosities, SolarMasses,
    SolarMassesPerYear, Years,
};

/// The least time from the inspiral's marked orbit to the merger, Julian years: a day, so that an
/// entry whose T falls at the very start of the source horizon is marked on an orbit of some
/// 2,100 km (two 1.33 M☉ neutron stars), outside both stars, rather than of none.
const LEAST_INSPIRAL_YEARS: f64 = 1.0 / 365.25;

/// The neutron-star merger class (plan 11's Provides; P11.T8.d): [`ClassId::NEUTRON_STAR_MERGER`].
///
/// # Examples
///
/// The class expects about ten mergers in the source horizon of a Milky Way-like galaxy, and each
/// leaves a black hole:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::NeutronStarMergers;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let galaxy = Galaxy::from_params(Seed::new(9), GalaxyParams::milky_way_like())?;
/// let expected = NeutronStarMergers::new(&galaxy).hosts().expected_hosts(&galaxy);
/// assert!((8.0..=12.0).contains(&expected));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct NeutronStarMergers {
    hosts: ClassHosts,
}

impl NeutronStarMergers {
    /// The class in `galaxy`.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        Self {
            hosts: ClassHosts::new(
                galaxy,
                ClassId::NEUTRON_STAR_MERGER,
                CarvedClass::NeutronStarMerger,
            ),
        }
    }

    /// Its hosts over the density components: the intensity, bound and site it thins on.
    #[must_use]
    pub const fn hosts(&self) -> &ClassHosts {
        &self.hosts
    }

    /// Candidate `id` of `cell`: its scan marks, then its orbit, pair and remnant.
    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<NsmCandidate> {
        let scan = nsm_scan_marks(galaxy, self, cell, id, bound)?;
        let seed = galaxy.seed();
        let system = SystemId::from(id);
        let mut stream = Stream::open(seed, tags::CLASS_NSM_BINARY, ObjectKey::from(system));
        let orientation = Orientation::new(
            Radians::new(math::acos(2.0 * stream.uniform() - 1.0)),
            Radians::new(std::f64::consts::TAU * stream.uniform()),
            Radians::new(std::f64::consts::TAU * stream.uniform()),
        )
        .expect("an inclination in [0, π] and angles in [0, 2π) are an orientation");
        let mean_anomaly = Radians::new(std::f64::consts::TAU * stream.uniform());
        let pair = Inspiral::marked(&scan, (orientation, mean_anomaly));
        let product = compact_state(
            scan.remnant_phase(),
            scan.remnant_mass(),
            scan.merger_age(),
            Years::ZERO,
        );
        let draws = [0, 1].map(|body| StarDraws::for_star(seed, BodyId::new(system, body)));
        let marks = BinaryMarks::merger(
            *scan.composition(),
            draws,
            scan.age_at_epoch(),
            MarkedMerger {
                age: scan.merger_age(),
                before: SegmentKind::Detached,
                stars: pair.stars,
                orbit: pair.orbit,
                transfer_rate: None,
                product,
            },
        )
        .expect("a merger age past the progenitors' deaths is a marked merger");
        Some(NsmCandidate {
            scan,
            pair,
            product,
            marks,
        })
    }

    /// The class's test on `candidate`: its timeline from its marks, kept if [`carved_class`]
    /// gives the class.
    fn test(candidate: NsmCandidate) -> Option<NsmEntry> {
        let NsmCandidate {
            scan,
            pair,
            product,
            marks,
        } = candidate;
        let timeline = BinaryTimeline::from_marks(marks);
        if carved_class(&timeline, scan.age_at_epoch()) != Some(CarvedClass::NeutronStarMerger) {
            return None;
        }
        let record = SystemRecord::from_parts(
            SystemId::from(scan.id()),
            *scan.position(),
            SystemOrigin::CatalogueSystem(ClassId::NEUTRON_STAR_MERGER),
            scan.population(),
            scan.progenitor_masses()[0],
            scan.age_at_epoch(),
        );
        Some(NsmEntry {
            record,
            motion: CatalogueMotion::straight(*scan.position(), scan.velocity()),
            scan,
            pair,
            product,
            timeline,
        })
    }
}

/// A compact star of `phase` and `mass` at the system's `age`, `since` its formation: a neutron
/// star at plan 06's radius ([`neutron_star_radius`]) and Hurley, Pols and Tout's (2000, equation
/// 93) luminosity at that age, or a black hole at its Schwarzschild radius and no light.
fn compact_state(phase: Phase, mass: SolarMasses, age: Years, since: Years) -> StarState {
    let recipe = RemnantRecipe::default();
    let (luminosity, radius) = if phase == Phase::NeutronStar {
        (hpt_luminosity(mass, since), neutron_star_radius(recipe))
    } else {
        (SolarLuminosities::ZERO, black_hole_radius(recipe, mass))
    };
    StarState::new(StarStateParts {
        phase,
        age,
        mass,
        core_mass: mass,
        luminosity,
        radius,
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.0,
    })
}

/// A neutron-star merger candidate the scan kept, with every mark its test reads.
#[derive(Debug, Clone, PartialEq)]
pub struct NsmCandidate {
    scan: NsmScanMarks,
    pair: Inspiral,
    product: StarState,
    marks: BinaryMarks,
}

/// A neutron-star merger's pair before T (P11.T8.d): two compact stars on the circular orbit that
/// gravitational waves bring to coalescence at T from the start of the source horizon (Peters
/// 1964).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inspiral {
    stars: [StarState; 2],
    orbit: KeplerElements,
}

impl Inspiral {
    /// Each compact star's marked state at T, primary first.
    #[must_use]
    pub const fn stars(&self) -> &[StarState; 2] {
        &self.stars
    }

    /// The relative orbit, circular, held as it is at the source horizon's start.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// The inspiral of `scan`'s pair, oriented and phased by `angles`: the separation from which
    /// Peters's circular merger time is the time from the source horizon's start to T, at least
    /// [`LEAST_INSPIRAL_YEARS`].
    fn marked(scan: &NsmScanMarks, (orientation, mean_anomaly): (Orientation, Radians)) -> Self {
        let [m1, m2] = scan.masses();
        let to_merger = scan
            .merger_time()
            .checked_since(SourceHorizon::START)
            .expect("T lies in the source horizon")
            .as_julian_years_f64()
            .max(LEAST_INSPIRAL_YEARS);
        let separation = peters_separation_for(m1, m2, Years::new(to_merger));
        let orbit = KeplerElements::from_semi_major_axis(
            separation,
            GravitationalParameter::from_solar_masses(m1 + m2),
            Eccentricity::CIRCULAR,
            orientation,
            mean_anomaly,
        )
        .expect(
            "a separation of a thousand kilometres or more about two compact stars is an orbit",
        );
        let t = scan.merger_age();
        let collapse = scan.collapse_ages();
        let phases = scan.phases();
        let star = |k: usize| {
            compact_state(
                phases[k],
                scan.masses()[k],
                t,
                Years::new(t.value() - collapse[k].value()),
            )
        };
        Self {
            stars: [star(0), star(1)],
            orbit,
        }
    }
}

/// A neutron-star merger entry (plan 11, P11.T8.d): a merger in the source horizon.
#[derive(Debug, Clone, PartialEq)]
pub struct NsmEntry {
    record: SystemRecord,
    motion: CatalogueMotion,
    scan: NsmScanMarks,
    pair: Inspiral,
    product: StarState,
    timeline: BinaryTimeline,
}

impl NsmEntry {
    /// The record of the entry's system, of origin [`SystemOrigin::CatalogueSystem`] of the class,
    /// at its site at the epoch.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// How the system moves: at its velocity, on one straight line.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// Its scan marks, the first it drew: T, the pair, the ejecta and the remnant.
    #[must_use]
    pub const fn scan(&self) -> &NsmScanMarks {
        &self.scan
    }

    /// The pair before T.
    #[must_use]
    pub const fn pair(&self) -> &Inspiral {
        &self.pair
    }

    /// The remnant from T, held at its state then.
    #[must_use]
    pub const fn product(&self) -> &StarState {
        &self.product
    }

    /// The pair's timeline, from its marks: the inspiral to T, the remnant from it.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        &self.timeline
    }

    /// The system's stars: the two progenitors, each on its own model at its initial mass and the
    /// system's age, on the inspiral's orbit, and the pair's timeline, which gives both compact
    /// stars' states, and from T the remnant in the primary's place.
    ///
    /// # Panics
    ///
    /// Never: the progenitors' masses are stars' and the age is finite.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        let system = self.record.id();
        let age = self.scan.age_at_epoch();
        let rules = galaxy.rules().stellar;
        let masses = self.scan.progenitor_masses();
        let stars = [0_u16, 1].into_iter().zip(masses).map(|(body, mass)| {
            let draws = StarDraws::for_star(galaxy.seed(), BodyId::new(system, body));
            StarModel::new_under(mass, *self.scan.composition(), draws, age, rules)
                .expect("a merger's progenitors are of 8–60 M☉ at a finite age")
        });
        let hierarchy = SystemHierarchy::binary_on(
            system,
            masses[0],
            (masses[1], SlotKind::Star),
            self.pair.orbit,
        );
        SystemStars::of_marked_pair(
            self.record,
            hierarchy,
            stars.collect(),
            self.timeline.clone(),
        )
    }
}

impl ClassProcess for NeutronStarMergers {
    type Candidate = NsmCandidate;
    type Entry = NsmEntry;

    fn class(&self) -> ClassId {
        ClassId::NEUTRON_STAR_MERGER
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
    ) -> Option<NsmCandidate> {
        self.candidate(galaxy, cell, id, bound)
    }

    fn test(&self, _: &Galaxy, candidate: NsmCandidate) -> Option<NsmEntry> {
        Self::test(candidate)
    }

    fn members(&self, _: &NsmEntry) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<NsmEntry>) -> CatalogueSystem {
        let entry = member.entry().value();
        CatalogueSystem::new(entry.record, entry.motion).with_class_stars()
    }

    fn stars(&self, galaxy: &Galaxy, member: &CatalogueMember<NsmEntry>) -> Option<SystemStars> {
        Some(member.entry().value().stars(galaxy))
    }
}

#[cfg(test)]
mod tests;
