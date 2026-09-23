//! A placed system: its ID, where it is at the epoch, and the marks placement draws for it (plan
//! 03, P03.T5.a).
//!
//! A [`SystemRecord`] is a system as placed and nothing more: the state at the epoch, never a
//! position at some other time (brainstorm, "Runtime and code shape": caches "hold only accepted
//! systems, as state at the epoch and never as positions at some time"). The range query moves a
//! record to the time asked about; the record itself does not move.
//!
//! Its marks are the two the first milestone needs, each on a stream of its own so that
//! metallicity, velocity and multiplicity can be added later without moving either (plan 03, Design
//! note 4): the primary's initial mass, drawn from the galaxy's mass function inside the layer's
//! band, and the age at the epoch, drawn from the age distribution of the density component the
//! candidate was picked for. An age is signed and runs down to −H where its population still forms
//! stars, so "no system yet" is a question asked with a time ([`SystemRecord::existence_at`];
//! brainstorm, "Events in time": "Star formation continues").

use super::CellKey;
use crate::coords::GalacticPosition;
use crate::galaxy::fields::{Component, ComponentId};
use crate::galaxy::imf::MassBand;
use crate::galaxy::{Galaxy, Population};
use crate::id::{Layer, SystemId};
use crate::rng::{ObjectKey, Stream, tags};
use crate::time::UniverseTime;
use crate::units::{SolarMasses, Years};

/// A system as placed: its state at the epoch.
///
/// Placement produces one for every candidate the thinning accepts, and resolving an ID reproduces
/// the same record without generating the rest of its cell. It is small and [`Copy`], because a
/// cache holds a whole cell's worth and a query returns a sphere's worth.
///
/// Its position is the position at the epoch. Where a system is at some other time is
/// `query`'s question, which adds the drift of the velocity plan 08 will draw.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// // A 32 ly cell at the solar circle: layer C holds a dozen or so systems.
/// let key = CellKey::new(Layer::C, [0, 812, 0])?;
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, key, &mut cell);
/// let system = cell.first().expect("layer C is not empty at the solar circle");
/// // A layer-C primary formed between 0.75 and 2.5 M☉, and its layer is its ID's.
/// assert!((0.75..=2.5).contains(&system.primary_initial_mass().value()));
/// assert_eq!(system.layer(), Layer::C);
/// // The record is the state at the epoch: its age at the epoch is the age it stores.
/// let at_epoch = system.age_at(UniverseTime::EPOCH);
/// assert!(at_epoch.total_cmp(&system.age_at_epoch()).is_eq());
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemRecord {
    id: SystemId,
    epoch_position: GalacticPosition,
    origin: SystemOrigin,
    population: Population,
    primary_initial_mass: SolarMasses,
    age_at_epoch: Years,
}

/// What made a record (plan 03, Design note 18).
///
/// The members of the galactic centre, of streams and of dwarf cores belong to no density
/// component, so a record carries where it came from instead of a component that some sources
/// could not supply. [`Grid`](Self::Grid) is the only variant of the first milestone; plan 09 adds
/// `FeatureMember(FeatureId)`, `CentreMember` and `CatalogueSystem(ClassId)` and plan 10
/// `GlobalListMember`, none of which carries a component, and no grid record changes when they
/// arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SystemOrigin {
    /// Placed by the layer's grid, in the density component the thinning picked.
    Grid(ComponentId),
}

/// Whether a system exists at a time (plan 03, Design note 7).
///
/// Every population that still forms stars draws ages from −H, so some systems are not yet born at
/// the epoch (brainstorm, "Events in time"). Their IDs resolve at every time; only their existence
/// depends on one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Existence {
    /// The system is not born yet: its age at this time is zero or negative.
    NoSystemYet,
    /// The system exists.
    Exists,
}

impl SystemRecord {
    /// A record from its parts, for non-grid sources (plans 09 and 10) and for tests.
    ///
    /// Placement itself never calls this: an accepted candidate's record comes from the candidate's
    /// own draws.
    ///
    /// # Panics
    ///
    /// If `origin` is [`SystemOrigin::Grid`] and `id` is not a grid ID, which would leave
    /// [`layer`](Self::layer) with no answer.
    #[must_use]
    pub fn from_parts(
        id: SystemId,
        epoch_position: GalacticPosition,
        origin: SystemOrigin,
        population: Population,
        primary_initial_mass: SolarMasses,
        age_at_epoch: Years,
    ) -> Self {
        match origin {
            SystemOrigin::Grid(_) => assert!(
                id.layer().is_some(),
                "a record of grid origin needs a grid ID, got {id:?}"
            ),
        }
        Self {
            id,
            epoch_position,
            origin,
            population,
            primary_initial_mass,
            age_at_epoch,
        }
    }

    /// The system's ID, which is also its candidate's address: layer, cell and candidate index.
    #[must_use]
    pub fn id(&self) -> SystemId {
        self.id
    }

    /// The layer that placed the system, which is always its ID's and never
    /// [`layer_for_initial_mass`](super::layer_for_initial_mass) of its primary's mass: plan 02's
    /// mass function draws from the closed band, so a primary can sit on its band's upper edge.
    ///
    /// # Panics
    ///
    /// Never: a grid origin is built with a grid ID, which names a layer.
    #[must_use]
    pub fn layer(&self) -> Layer {
        match self.origin {
            SystemOrigin::Grid(_) => self
                .id
                .layer()
                .expect("a record of grid origin was built with a grid ID"),
        }
    }

    /// Where the system is at the epoch, in the galactic frame.
    #[must_use]
    pub fn epoch_position(&self) -> &GalacticPosition {
        &self.epoch_position
    }

    /// What made the record (Design note 18).
    #[must_use]
    pub fn origin(&self) -> SystemOrigin {
        self.origin
    }

    /// The density component the system was drawn from, for a record the grid placed; `None` for
    /// every other origin.
    ///
    /// The ID is plan 02's flat index into [`Fields::components`](crate::galaxy::fields::Fields),
    /// valid for this galaxy's fields and no other's. Code that reads a component's laws — plan
    /// 06's metallicity, plan 08's velocity — takes it from here.
    #[must_use]
    pub fn component(&self) -> Option<ComponentId> {
        match self.origin {
            SystemOrigin::Grid(component) => Some(component),
        }
    }

    /// The population the system belongs to.
    ///
    /// It is stored, not looked up, so that a record answers without its [`Galaxy`] and whatever
    /// its origin is.
    #[must_use]
    pub fn population(&self) -> Population {
        self.population
    }

    /// The initial mass of the primary star, M☉: inside the band of the system's layer, and never
    /// above the upper mass limit of 150 M☉.
    ///
    /// It is the mass the star formed with, not the mass it has now: a layer-E system whose primary
    /// has died is a neutron star or a black hole (brainstorm, "Sizing the layers"). The stellar
    /// stage (plan 06) turns this, the age and the metallicity into what the star is today.
    #[must_use]
    pub fn primary_initial_mass(&self) -> SolarMasses {
        self.primary_initial_mass
    }

    /// The system's age at the epoch, Julian years: positive for a system already born, and down to
    /// −H for one a still-forming population will form later (Design note 7).
    #[must_use]
    pub fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// The system's age at `t`, Julian years: the age at the epoch plus the time since the epoch.
    #[must_use]
    pub fn age_at(&self, t: UniverseTime) -> Years {
        Years::new(self.age_at_epoch.value() + t.since_epoch().as_julian_years_f64())
    }

    /// Whether the system exists at `t`: it does once [`age_at`](Self::age_at) is positive.
    #[must_use]
    pub fn existence_at(&self, t: UniverseTime) -> Existence {
        if self.age_at(t).value() > 0.0 {
            Existence::Exists
        } else {
            Existence::NoSystemYet
        }
    }

    /// The record of an accepted candidate: its marks, drawn on their own streams, around the
    /// position and component the thinning settled (plan 03, Design note 4).
    #[must_use]
    pub(super) fn of_candidate(
        galaxy: &Galaxy,
        key: CellKey,
        id: SystemId,
        epoch_position: GalacticPosition,
        component: ComponentId,
    ) -> Self {
        let primary_initial_mass = primary_initial_mass(galaxy, id, key.band());
        let component_laws = galaxy.fields().component(component);
        let age_at_epoch = age_at_epoch(galaxy, id, component_laws);
        Self {
            id,
            epoch_position,
            origin: SystemOrigin::Grid(component),
            population: component_laws.population(),
            primary_initial_mass,
            age_at_epoch,
        }
    }
}

/// The primary's initial mass: one word on `system.primary_mass`, keyed by the candidate's ID,
/// through the galaxy's mass function restricted to `band` (brainstorm, "Systems and stars":
/// "Primary mass from an initial mass function, within the layer's band, up to a limit of 150 M☉").
#[must_use]
fn primary_initial_mass(galaxy: &Galaxy, id: SystemId, band: MassBand) -> SolarMasses {
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::SYSTEM_PRIMARY_MASS,
        ObjectKey::from(id),
    );
    SolarMasses::new(galaxy.mass_function().sample_in_band(band, &mut stream))
}

/// The system's age at the epoch: one word on `system.age`, keyed by the candidate's ID, through
/// the age distribution of the component it was picked for, which is why the pick is over
/// components and not populations (plan 03, Design note 3).
///
/// The age is signed and never clamped at zero: a population that still forms stars draws from −H.
#[must_use]
fn age_at_epoch(galaxy: &Galaxy, id: SystemId, component: &Component) -> Years {
    let mut stream = Stream::open(galaxy.seed(), tags::SYSTEM_AGE, ObjectKey::from(id));
    component.ages().sample(&mut stream)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::LyCell;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::STELLAR_LAYERS;
    use crate::id::CentreMemberId;
    use crate::rng::Seed;
    use crate::time::{CLOCK_WINDOW_H, ClockWindow};

    /// The seed of the galaxy these tests draw marks in.
    const SEED: u64 = 0x0300_5eed_0000_0000;

    /// How many marks each statistical check draws per layer.
    const DRAWS: u32 = 100_000;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
    }

    /// The Sun-like point: in the plane, 26,000 ly out on the +y axis, clear of the bar.
    fn sunlike() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    /// `n` candidate IDs of `layer` at the Sun-like point, running on into the next cell along x
    /// once a cell's index field is full.
    fn candidate_ids(layer: Layer, n: u32) -> impl Iterator<Item = SystemId> {
        let first = CellKey::containing(layer, &sunlike()).unwrap();
        let [x, y, z] = first.gen_cell().to_array();
        let capacity = first.index_capacity();
        (0..n).map(move |i| {
            let along = i32::try_from(i / capacity).unwrap();
            let key = CellKey::new(layer, [x + along, y, z]).unwrap();
            key.candidate_id(i % capacity).unwrap()
        })
    }

    /// A record with the given age at the epoch and nothing else of interest.
    fn record_aged(galaxy: &Galaxy, age: f64) -> SystemRecord {
        let key = CellKey::new(Layer::C, [0, 812, 0]).unwrap();
        SystemRecord::from_parts(
            key.candidate_id(3).unwrap(),
            GalacticPosition::new(LyCell::new([0, 26_000, 0]), [0.0; 3]).unwrap(),
            SystemOrigin::Grid(galaxy.fields().component_id(0).unwrap()),
            Population::OldThinDisc,
            SolarMasses::new(1.0),
            Years::new(age),
        )
    }

    /// A record's fields with the origin swapped for `O`, so that the test can measure what a wider
    /// origin would cost: the compiler lays it out as it lays out [`SystemRecord`], which the test
    /// checks with `O = SystemOrigin`.
    #[expect(dead_code, reason = "only the layout is measured")]
    struct RecordWith<O> {
        id: SystemId,
        epoch_position: GalacticPosition,
        origin: O,
        population: Population,
        primary_initial_mass: SolarMasses,
        age_at_epoch: Years,
    }

    /// An origin whose widest variant carries eight bytes at four-byte alignment.
    #[expect(dead_code, reason = "only the layout is measured")]
    enum OriginOfEightBytes {
        Grid(ComponentId),
        Wide(u32),
    }

    #[test]
    fn record_is_copy_and_fits_eighty_bytes() {
        // Plan 03, P03.T5.a: at most 80 bytes, with room for the origin to grow.
        let bytes = size_of::<SystemRecord>();
        assert!(bytes <= 80, "a record is {bytes} bytes");
        assert_eq!(size_of::<SystemOrigin>(), 1);
        assert_eq!(size_of::<RecordWith<SystemOrigin>>(), bytes);
        // Room for an origin of eight bytes at four-byte alignment, such as a tag and a `u32`. An
        // eight-byte-aligned payload rounds the enum up to sixteen and the record to 88 bytes, and
        // so does plan 01's `FeatureRef`, which is sixteen bytes in memory (plan 03, Risks).
        let grown = size_of::<RecordWith<OriginOfEightBytes>>();
        assert!(
            grown <= 80,
            "an eight-byte origin takes a record to {grown} bytes"
        );
        let record = record_aged(&galaxy(), 1.0);
        let copy = record;
        assert_eq!(copy, record);
    }

    #[test]
    fn record_reports_the_parts_it_was_built_from() {
        let galaxy = galaxy();
        let component = galaxy.fields().component_id(2).unwrap();
        let key = CellKey::new(Layer::B, [1, 1_625, 0]).unwrap();
        let id = key.candidate_id(9).unwrap();
        let position = GalacticPosition::new(LyCell::new([16, 26_000, 3]), [0.0; 3]).unwrap();
        let record = SystemRecord::from_parts(
            id,
            position,
            SystemOrigin::Grid(component),
            Population::ThickDisc,
            SolarMasses::new(0.6),
            Years::new(1.1e10),
        );
        assert_eq!(record.id(), id);
        assert_eq!(record.layer(), Layer::B);
        assert_eq!(record.epoch_position(), &position);
        assert_eq!(record.origin(), SystemOrigin::Grid(component));
        assert_eq!(record.component(), Some(component));
        assert_eq!(record.population(), Population::ThickDisc);
        assert_same_bits(record.primary_initial_mass().value(), 0.6);
        assert_same_bits(record.age_at_epoch().value(), 1.1e10);
    }

    #[test]
    #[should_panic(expected = "a record of grid origin needs a grid ID")]
    fn record_of_grid_origin_refuses_a_reserved_id() {
        let galaxy = galaxy();
        let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
        let _ = SystemRecord::from_parts(
            black_hole,
            GalacticPosition::ORIGIN,
            SystemOrigin::Grid(galaxy.fields().component_id(0).unwrap()),
            Population::Bulge,
            SolarMasses::new(10.0),
            Years::new(1.0),
        );
    }

    #[test]
    fn record_masses_lie_in_the_layers_band() {
        let galaxy = galaxy();
        for spec in &STELLAR_LAYERS {
            let band = spec.band();
            for id in candidate_ids(spec.layer(), DRAWS) {
                let mass = primary_initial_mass(&galaxy, id, band).value();
                assert!(
                    (band.lo()..=band.hi()).contains(&mass),
                    "layer {}: {mass} M☉ outside {}–{} M☉",
                    spec.layer().letter(),
                    band.lo(),
                    band.hi()
                );
            }
        }
    }

    #[test]
    fn record_ages_lie_in_their_components_range() {
        let galaxy = galaxy();
        let ids: Vec<_> = candidate_ids(Layer::C, 10_000).collect();
        for component_id in galaxy.fields().component_ids() {
            let component = galaxy.fields().component(component_id);
            let (min, max) = (component.ages().min(), component.ages().max());
            for &id in &ids {
                let age = age_at_epoch(&galaxy, id, component);
                assert!(
                    (min.value()..=max.value()).contains(&age.value()),
                    "component {}: {} yr outside {}–{} yr",
                    component_id.index(),
                    age.value(),
                    min.value(),
                    max.value()
                );
            }
            // A still-forming component reaches back to −H, and no age is clamped at zero.
            if min.value() < 0.0 {
                assert!(min.value() >= -CLOCK_WINDOW_H.as_julian_years_f64());
            }
        }
    }

    #[test]
    fn record_born_after_the_epoch_has_no_system_yet_until_its_birth() {
        let galaxy = galaxy();
        let record = record_aged(&galaxy, -500.0);
        let at = |years: i64| UniverseTime::from_julian_years(years).unwrap();
        assert_eq!(
            record.existence_at(UniverseTime::EPOCH),
            Existence::NoSystemYet
        );
        assert_eq!(record.existence_at(at(499)), Existence::NoSystemYet);
        assert_eq!(record.existence_at(at(501)), Existence::Exists);
        // Exactly at its birth it does not exist yet: existence is `age > 0`.
        assert_eq!(record.existence_at(at(500)), Existence::NoSystemYet);
        assert_eq!(
            record_aged(&galaxy, 1.0).existence_at(UniverseTime::EPOCH),
            Existence::Exists
        );
    }

    #[test]
    fn record_age_moves_with_the_clock_window() {
        let galaxy = galaxy();
        let age = 9.5e9;
        let record = record_aged(&galaxy, age);
        assert_same_bits(record.age_at(UniverseTime::EPOCH).value(), age);
        let h = CLOCK_WINDOW_H.as_julian_years_f64();
        assert_same_bits(h, 1_000.0);
        for (end, sign) in [(ClockWindow::END, 1.0), (ClockWindow::START, -1.0)] {
            let moved = record.age_at(end).value() - age;
            // The ages at ±H differ from the epoch's by H, to within the spacing of f64 there: the
            // year conversion is exact and the addition rounds once, by half a spacing.
            let slack = f64::EPSILON * age;
            assert!(
                (moved - sign * h).abs() <= slack,
                "the age moved by {moved} yr over {sign} × {h} yr"
            );
        }
    }
}
