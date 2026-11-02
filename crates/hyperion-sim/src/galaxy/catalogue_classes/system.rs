//! What a member of a catalogue class's entry is to the rest of the sim: a catalogue system, with
//! its record, its motion and its stars (plan 09, P09.T33.a).
//!
//! A class's entry type is its own ([`ClassProcess::Entry`]), but every member of an entry is a
//! system the sim can name, place and evaluate. [`ClassProcess::system`] turns a member into a
//! [`CatalogueSystem`], and [`system_of`] resolves a `111` ID to one through the production lookup:
//! plan 03's [`resolve`](crate::galaxy::placement::resolve) returns its record, and the range
//! query's positions, the stellar stage and the planetary context read its motion and stars from
//! it. Its record's origin is [`SystemOrigin::CatalogueSystem`]: no density component's laws apply
//! to it.
//!
//! A supernova entry's member 0 moves on a straight line broken at its explosion, where its compact
//! remnant takes its natal kick (P09.T33.a: "at the site + velocity × (t − T) before the explosion
//! and at site + (velocity + kick) × (t − T) after it"), so a catalogue system's motion is
//! [`CatalogueMotion`], two lines that meet at one instant.

use super::grid::{
    ClassLookup, ClassProcess, ClassVisitor, MemberVisitor, ProductionClasses, resolve,
};
use super::{CatalogueCellKey, CatalogueMember, ClassId};
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
#[cfg(doc)]
use crate::galaxy::placement::SystemOrigin;
use crate::galaxy::placement::{Existence, ResolveSystemError, SystemRecord};
use crate::id::{CatalogueSystemId, SystemIdKind};
use crate::stellar::Composition;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::SystemStars;
use crate::time::UniverseTime;
use crate::units::{MetresPerSecond, Seconds};

/// Where a catalogue system is at any time: a straight line through a point, broken there at one
/// instant (plan 09, P09.T33.a).
///
/// A supernova entry's member 0 is at the explosion site at T, moves at the progenitor's velocity
/// before T and at the remnant's, the progenitor's plus the natal kick, from T. A system that moves
/// on one line has the same velocity on both sides ([`CatalogueMotion::straight`]). Velocities are
/// galactic-frame.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::galaxy::catalogue_classes::CatalogueMotion;
/// use hyperion_sim::time::UniverseTime;
///
/// let site = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in range")?;
/// let exploded = UniverseTime::from_julian_years(-1_000).ok_or("on the clock")?;
/// // A progenitor moving at 300 km/s along +x, whose remnant is kicked to rest.
/// let moving = GalacticVelocity::new([3e5, 0.0, 0.0]);
/// let motion = CatalogueMotion::broken(site, exploded, moving, GalacticVelocity::default());
/// assert_eq!(motion.position_at(exploded), site);
/// // At the epoch the remnant is still at the site; two thousand years before it, the progenitor
/// // was a thousand years' travel short of it, about 1.0 ly.
/// assert_eq!(motion.position_at(UniverseTime::EPOCH), site);
/// let before = UniverseTime::from_julian_years(-2_000).ok_or("on the clock")?;
/// let gap = site.distance_to(&motion.position_at(before)).value() / 9.460_730_472_580_8e15;
/// assert!((gap - 1.0007).abs() < 1e-3);
/// # Ok::<(), &str>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogueMotion {
    point: GalacticPosition,
    at: UniverseTime,
    before: GalacticVelocity,
    after: GalacticVelocity,
}

impl CatalogueMotion {
    /// The motion through `point` at `at`, at `before` until then and `after` from then.
    #[must_use]
    pub const fn broken(
        point: GalacticPosition,
        at: UniverseTime,
        before: GalacticVelocity,
        after: GalacticVelocity,
    ) -> Self {
        Self {
            point,
            at,
            before,
            after,
        }
    }

    /// One straight line, through `epoch_position` at the epoch at `velocity`.
    #[must_use]
    pub const fn straight(epoch_position: GalacticPosition, velocity: GalacticVelocity) -> Self {
        Self::broken(epoch_position, UniverseTime::EPOCH, velocity, velocity)
    }

    /// The point the two lines meet at: a supernova's explosion site.
    #[must_use]
    pub const fn point(&self) -> &GalacticPosition {
        &self.point
    }

    /// When the lines meet: a supernova's T.
    #[must_use]
    pub const fn at(&self) -> UniverseTime {
        self.at
    }

    /// The velocity at `t`: the first line's before the break, the second's from it.
    #[must_use]
    pub fn velocity_at(&self, t: UniverseTime) -> GalacticVelocity {
        if t < self.at { self.before } else { self.after }
    }

    /// The faster of its two lines' speeds.
    #[must_use]
    pub(crate) fn fastest(&self) -> MetresPerSecond {
        MetresPerSecond::new(self.before.speed().value().max(self.after.speed().value()))
    }

    /// The position at `t`: the break's point moved along [`velocity_at`](Self::velocity_at) over
    /// `t` − the break.
    ///
    /// # Panics
    ///
    /// If the line would leave the addressable range, which no speed below light's does within a
    /// hundred million years of the break: 10⁸ ly against the range's 2³¹.
    #[must_use]
    pub fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        if t == self.at {
            return self.point;
        }
        // The span is exact on the clock, so it rounds once, to seconds as a float.
        let elapsed = Seconds::new(
            t.checked_since(self.at)
                .expect("two clock times are a span apart")
                .as_seconds_f64(),
        );
        self.point
            .translated(self.velocity_at(t).displacement_over(elapsed))
            .expect(
                "a catalogue system moves below light's speed and is asked about within the clock's \
                 reach of its break, which leaves the addressable range nowhere",
            )
    }
}

/// A catalogue system as a source of light (plan 12): its broken line, read on whichever side of
/// the break each time falls (plan 10, P10.T10.b's registration member). Its stated curvature
/// error is a straight line's, the line through the light's emission at its velocity then.
impl crate::observe::Trajectory for CatalogueMotion {
    fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        Self::position_at(self, t)
    }

    fn velocity_at(&self, t: UniverseTime) -> GalacticVelocity {
        Self::velocity_at(self, t)
    }
}

/// A member of a catalogue class's entry, as the rest of the sim reads it: its record, its motion,
/// and how its stars are generated where its class models them by the stellar stage (plan 09,
/// P09.T33.a).
///
/// Built by its class's [`ClassProcess::system`]; [`system_of`] resolves one from its ID.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogueSystem {
    record: SystemRecord,
    motion: CatalogueMotion,
    stars: Option<StarSource>,
}

/// Where a catalogue system's stars come from.
#[derive(Debug, Clone, Copy, PartialEq)]
enum StarSource {
    /// The stellar stage, at a composition and under a multiplicity context.
    Stage(Composition, MultiplicityContext),
    /// Its class, which builds them from its own marks ([`ClassProcess::stars`]).
    Class,
}

impl CatalogueSystem {
    /// The system of `record` moving by `motion`, with no stars of the stellar stage's.
    ///
    /// # Panics
    ///
    /// If `record`'s origin is not a catalogue system's: a class builds its own members only.
    #[must_use]
    pub fn new(record: SystemRecord, motion: CatalogueMotion) -> Self {
        assert!(
            matches!(record.id().kind(), SystemIdKind::Catalogue(_)),
            "a catalogue system's record carries its 111 ID, got {:?}",
            record.id()
        );
        Self {
            record,
            motion,
            stars: None,
        }
    }

    /// The same system, whose stars are the stellar stage's at `composition` under `context`
    /// ([`SystemStars::generate_with`]).
    #[must_use]
    pub const fn with_stars(self, composition: Composition, context: MultiplicityContext) -> Self {
        Self {
            stars: Some(StarSource::Stage(composition, context)),
            ..self
        }
    }

    /// The same system, whose stars its class builds from its own marks
    /// ([`ClassProcess::stars`]), as plan 11's binary classes do (P11.T8.a): a white dwarf and its
    /// donor that the stellar stage, which evolves single stars, would not give.
    #[must_use]
    pub const fn with_class_stars(self) -> Self {
        Self {
            stars: Some(StarSource::Class),
            ..self
        }
    }

    /// The record: what plan 03's resolve returns for the system's ID.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The motion.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// Where the system is at `t`, as [`CatalogueMotion::position_at`].
    ///
    /// # Panics
    ///
    /// As [`CatalogueMotion::position_at`].
    #[must_use]
    pub fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        self.motion.position_at(t)
    }

    /// Whether it is seen at `t`, as the range query lists it and the sky shows it in light emitted
    /// then (P09.T37.a; rendering plan R06, R06.T16.a): where its record exists then, and a further
    /// member of its entry only from its entry's break, before which a Type Ia's survivor is its
    /// progenitor's companion, at member 0's place (P09.T34.a's lean).
    #[must_use]
    pub fn shown_at(&self, t: UniverseTime) -> bool {
        let SystemIdKind::Catalogue(id) = self.record.id().kind() else {
            unreachable!("a catalogue system's record carries its 111 ID");
        };
        if id.member() > 0 && t < self.motion.at() {
            return false;
        }
        match self.record.existence_at(t) {
            Existence::NoSystemYet => false,
            Existence::Exists => true,
        }
    }

    /// The composition its stars are generated at and the multiplicity context they are drawn
    /// under, where its class models them by the stellar stage.
    #[must_use]
    pub fn star_model(&self) -> Option<(&Composition, MultiplicityContext)> {
        match &self.stars {
            Some(StarSource::Stage(composition, context)) => Some((composition, *context)),
            Some(StarSource::Class) | None => None,
        }
    }

    /// Its stars: [`SystemStars::generate_with`] of its record at its composition and context,
    /// those its class builds ([`ClassProcess::stars`] of the member its ID resolves to), or `None`
    /// for a member its class does not model.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> Option<SystemStars> {
        match self.stars? {
            StarSource::Stage(composition, context) => Some(SystemStars::generate_with(
                galaxy,
                &self.record,
                &composition,
                context,
            )),
            StarSource::Class => {
                let SystemIdKind::Catalogue(id) = self.record.id().kind() else {
                    unreachable!("a catalogue system's record carries its 111 ID");
                };
                // A system its class built resolves to the same member; one that does not resolve
                // in `galaxy`, a system of another galaxy's, has no stars here.
                resolve(galaxy, &ProductionClasses, id, StarsOf { galaxy })
                    .ok()
                    .flatten()
            }
        }
    }
}

/// The catalogue system a `111` ID names in `galaxy`, through the production lookup (plan 09,
/// P09.T33.a): [`resolve`] with a visitor that asks the class's process for its member's system.
///
/// What a class generates may read the galaxy's kinematic tables: a core collapse's progenitor
/// moves at its drawn velocity, which decides its window's lifetime cap and its position at the
/// epoch, and rests in a galaxy built without them, as every grid record does
/// ([`Galaxy::with_full_potential`]). The server builds its galaxies with them.
///
/// # Errors
///
/// - [`ResolveSystemError::NoSuchSystem`] for an ID that names no class cell
///   ([`CatalogueCellKey::of`]: a free class value, a class never placed, a misaligned cell), and,
///   as [`resolve`], for an index beyond its cell's count, a candidate thinned or failing its
///   class's test, and a member the entry lacks.
/// - [`ResolveSystemError::KindNotGenerated`] for a placed class whose process is not built yet:
///   its IDs will name systems once its task registers it, so they are not "no system" now.
///
/// # Panics
///
/// As [`resolve`].
///
/// # Examples
///
/// The first core-collapse entry of a cell, resolved from its ID alone:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::{
///     CatalogueCellKey, CatalogueClassCell, ClassId, system_of,
/// };
/// use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
/// use hyperion_sim::galaxy::placement::resolve;
/// use hyperion_sim::id::SystemId;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = CoreCollapseProcess::new(&galaxy);
/// // Cells along a line through the inner disc, in the plane, until one holds an entry.
/// let mut entries = Vec::new();
/// for y in 8..28 {
///     let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, y, 0])?;
///     entries.extend(CatalogueClassCell::generate(&galaxy, &process, key).into_entries());
///     if !entries.is_empty() {
///         break;
///     }
/// }
/// let entry = entries.first().ok_or("the inner disc has recent deaths")?;
/// let system = system_of(&galaxy, entry.id())?;
/// assert_eq!(system.record(), &resolve(&galaxy, SystemId::from(entry.id()))?);
/// assert!(system.stars(&galaxy).is_some());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn system_of(
    galaxy: &Galaxy,
    id: CatalogueSystemId,
) -> Result<CatalogueSystem, ResolveSystemError> {
    let key = CatalogueCellKey::of(id).map_err(|_| ResolveSystemError::NoSuchSystem)?;
    ProductionClasses
        .visit_class(galaxy, key.class(), Registered)
        .ok_or(ResolveSystemError::KindNotGenerated)?;
    resolve(galaxy, &ProductionClasses, id, SystemOf { galaxy })
}

/// Whether the production lookup has a class's process: a visit that does nothing.
struct Registered;

impl ClassVisitor for Registered {
    type Output = ();

    fn visit<P: ClassProcess>(self, _: &P) {}
}

/// The catalogue system of `record`, a record of [`SystemOrigin::CatalogueSystem`] resolved in
/// `galaxy`: [`system_of`] of its ID.
///
/// # Panics
///
/// If `record` is not a catalogue system's, or does not resolve in `galaxy`, which only a record of
/// another galaxy can fail to.
#[must_use]
pub fn system_of_record(galaxy: &Galaxy, record: &SystemRecord) -> CatalogueSystem {
    let SystemIdKind::Catalogue(id) = record.id().kind() else {
        panic!(
            "a catalogue system's record carries its 111 ID, got {:?}",
            record.id()
        );
    };
    system_of(galaxy, id)
        .expect("a catalogue system's record resolves in the galaxy that placed it")
}

/// [`CatalogueSystem::stars`]'s visit of the member of class stars [`resolve`] found.
struct StarsOf<'g> {
    galaxy: &'g Galaxy,
}

impl MemberVisitor for StarsOf<'_> {
    type Output = Option<SystemStars>;

    fn visit<P: ClassProcess>(
        self,
        process: &P,
        member: CatalogueMember<P::Entry>,
    ) -> Option<SystemStars> {
        process.stars(self.galaxy, &member)
    }
}

/// [`system_of`]'s visit of the member [`resolve`] found.
struct SystemOf<'g> {
    galaxy: &'g Galaxy,
}

impl MemberVisitor for SystemOf<'_> {
    type Output = CatalogueSystem;

    fn visit<P: ClassProcess>(
        self,
        process: &P,
        member: CatalogueMember<P::Entry>,
    ) -> CatalogueSystem {
        let system = process.system(self.galaxy, &member);
        debug_assert_eq!(
            system.record().id(),
            member.id().into(),
            "the {} class gives member {:?} another's record",
            ClassId::from_value(member.id().class()).map_or("free", ClassId::name),
            member.id()
        );
        system
    }
}
