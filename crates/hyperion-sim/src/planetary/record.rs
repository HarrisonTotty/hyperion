//! What a body query returns, and how it degrades for the Knowledge overlay: [`BodyRecord`],
//! [`SystemSnapshot`], [`DetailLevel`] and [`Section`] (plan 14, design note 16, P14.T34).
//!
//! A record is nested so that each detail level is a prefix of the next:
//!
//! | Level | Adds |
//! | ----- | ---- |
//! | [`Contact`](DetailLevel::Contact) | the identity with the kind unknown, and the position (the brainstorm's "unresolved contact") |
//! | [`MassAndOrbit`](DetailLevel::MassAndOrbit) | the kind and label, the mass, the orbit, and the lists of moons and rings; on a system, its belts and halo |
//! | [`Bulk`](DetailLevel::Bulk) | radius, density, surface gravity, class, composition and equilibrium temperature; on a system, the belts' members |
//! | [`Surface`](DetailLevel::Surface) | atmosphere, surface conditions, rotation and global figures (P14.T13, T14, T24) |
//! | [`Full`](DetailLevel::Full) | the hooks: surface seed, bulk composition, habitability, resources (P14.T23–T26) |
//!
//! Every optional section carries one of four states (ruling 34 of 2026-09-22, item 3):
//! [`Section::Ok`] with its value, [`Section::NotResolved`] where the granted level withholds it,
//! [`Section::NotModelled`] where this generator version does not compute it, and
//! [`Section::NotApplicable`] where it means nothing for the body's kind. [`BodyRecord::degrade`]
//! and [`SystemSnapshot::degrade`] are the only producers of `NotResolved`: they turn every section
//! above the level into it and never blur a number, and a record cannot be built with one. The
//! generator is the authority on what it computes, so it sets every other state; a reader never
//! infers one. "None" is data, not a state: a planet with no moons has an `Ok` empty list.
//!
//! In the vertical slice (ruling 33) a planet's surface and hooks sections, its moons and rings,
//! and a system's belts and halo are [`Section::NotModelled`], and a giant's surface is
//! [`Section::NotApplicable`]. The contents of the surface and hooks sections, [`Surface`] and
//! [`Hooks`], have no value until their tasks define them.

use std::error::Error;
use std::fmt;

use crate::coords::SystemPosition;
use crate::id::{BodyId, SystemId};
use crate::orbit::KeplerElements;
use crate::planetary::derive::{DerivedBody, MassFractions, PlanetClass};
use crate::planetary::fate::BodyState;
use crate::planetary::index::{BodyIndex, BodySub};
use crate::planetary::placement::OrbitHost;
use crate::time::UniverseTime;
use crate::units::{
    EarthMasses, EarthRadii, Kelvin, KilogramsPerCubicMetre, MetresPerSecondSquared,
};

/// How much of a body a record holds, from least to most; each level holds everything the levels
/// below it do (design note 16).
///
/// Until the Knowledge overlay exists the server grants whatever level a request asks for, and
/// says which it granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DetailLevel {
    /// An unresolved contact: the ID and the position, and nothing of what the body is.
    Contact,
    /// Mass and orbit only, with what the body is.
    MassAndOrbit,
    /// Its bulk: radius, density, class and equilibrium temperature.
    Bulk,
    /// Its surface: atmosphere, surface conditions, rotation and global figures.
    Surface,
    /// Everything, the hooks for the layers above included.
    Full,
}

impl DetailLevel {
    /// Every level, from least to most.
    pub const ALL: [Self; 5] = [
        Self::Contact,
        Self::MassAndOrbit,
        Self::Bulk,
        Self::Surface,
        Self::Full,
    ];
}

/// One optional section of a record, tagged with its state (ruling 34 of 2026-09-22, item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section<T> {
    /// The section, with its value.
    Ok(T),
    /// The granted detail level withholds the section.
    ///
    /// Only [`BodyRecord::degrade`] and [`SystemSnapshot::degrade`] produce it.
    NotResolved,
    /// This generator version does not compute the section. It must never be read as "none".
    NotModelled,
    /// The section has no meaning for the body's kind, as a gas giant's surface has none.
    NotApplicable,
}

impl<T> Section<T> {
    /// The section's state, without its value.
    #[must_use]
    pub const fn state(&self) -> SectionState {
        match self {
            Self::Ok(_) => SectionState::Ok,
            Self::NotResolved => SectionState::NotResolved,
            Self::NotModelled => SectionState::NotModelled,
            Self::NotApplicable => SectionState::NotApplicable,
        }
    }

    /// The value, if the section is [`Ok`](Self::Ok).
    #[must_use]
    pub const fn ok(&self) -> Option<&T> {
        match self {
            Self::Ok(value) => Some(value),
            Self::NotResolved | Self::NotModelled | Self::NotApplicable => None,
        }
    }

    /// The section with `f` applied to its value, and its state otherwise kept.
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Section<U> {
        match self {
            Self::Ok(value) => Section::Ok(f(value)),
            Self::NotResolved => Section::NotResolved,
            Self::NotModelled => Section::NotModelled,
            Self::NotApplicable => Section::NotApplicable,
        }
    }
}

impl<T: Clone> Section<T> {
    /// The section as a record granted `granted` holds it, where the section belongs to `level`:
    /// unchanged at or below the grant, [`NotResolved`](Self::NotResolved) above it.
    #[must_use]
    fn granted(&self, level: DetailLevel, granted: DetailLevel) -> Self {
        if level <= granted {
            self.clone()
        } else {
            Self::NotResolved
        }
    }
}

/// The state of a [`Section`], without its value: what the wire's `state` tag says (P14.T35).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SectionState {
    /// [`Section::Ok`].
    Ok,
    /// [`Section::NotResolved`].
    NotResolved,
    /// [`Section::NotModelled`].
    NotModelled,
    /// [`Section::NotApplicable`].
    NotApplicable,
}

/// The sections of a [`BodyRecord`], each with the detail level that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecordSection {
    /// The label for people (design note 22), withheld from a contact.
    Label,
    /// The mass.
    Mass,
    /// The orbit about the body's primary.
    Orbit,
    /// The body's moons.
    Moons,
    /// The body's rings.
    Rings,
    /// The bulk properties.
    Bulk,
    /// The surface.
    Surface,
    /// The hooks.
    Hooks,
}

impl RecordSection {
    /// Every section, in the record's order.
    pub const ALL: [Self; 8] = [
        Self::Label,
        Self::Mass,
        Self::Orbit,
        Self::Moons,
        Self::Rings,
        Self::Bulk,
        Self::Surface,
        Self::Hooks,
    ];

    /// The least detail level that holds the section (design note 16).
    #[must_use]
    pub const fn level(self) -> DetailLevel {
        match self {
            Self::Label | Self::Mass | Self::Orbit | Self::Moons | Self::Rings => {
                DetailLevel::MassAndOrbit
            }
            Self::Bulk => DetailLevel::Bulk,
            Self::Surface => DetailLevel::Surface,
            Self::Hooks => DetailLevel::Full,
        }
    }
}

impl fmt::Display for RecordSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Label => "label",
            Self::Mass => "mass",
            Self::Orbit => "orbit",
            Self::Moons => "moons",
            Self::Rings => "rings",
            Self::Bulk => "bulk",
            Self::Surface => "surface",
            Self::Hooks => "hooks",
        })
    }
}

/// What a body is (plan 14, Provides; every variant from the start, so that the wire's
/// `BodyKindDto` never changes shape).
///
/// A planet's class by composition is its bulk section's ([`BulkProperties::class`]), since design
/// note 16 puts the class at the `Bulk` level and the kind is shown from `MassAndOrbit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BodyKind {
    /// A planet, bound to a host or free-floating.
    Planet,
    /// A belt's largest member, derived as a dwarf planet (P14.T21.c).
    DwarfPlanet,
    /// A moon, by how it came to orbit its planet.
    Moon(MoonOrigin),
    /// A ring system (P14.T20).
    Ring,
    /// A belt's population (P14.T21).
    Belt(BeltKind),
    /// A cometary halo, a statistical population (P14.T21.d).
    CometaryHalo,
    /// A protoplanetary disc, before its lifetime ends (P14.T28.a).
    ProtoplanetaryDisc,
    /// A white dwarf's dusty debris disc (P14.T28.d).
    DebrisDisc,
    /// An unresolved contact, whose kind the granted level withholds. Only
    /// [`BodyRecord::degrade`] to [`DetailLevel::Contact`] produces it.
    Unresolved,
}

/// How a moon came to orbit its planet (P14.T17–T19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MoonOrigin {
    /// Formed in the planet's circumplanetary disc (P14.T17).
    Regular,
    /// Formed from a giant impact, as Earth's Moon and Charon (P14.T18).
    GiantImpact,
    /// Captured, on a distant, eccentric and often retrograde orbit (P14.T19).
    Captured,
}

/// Which kind of belt a population is (P14.T21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BeltKind {
    /// An asteroid belt, inside the innermost giant or in a wide gap between planets (P14.T21.a).
    Asteroid,
    /// A Kuiper-like belt outside the outermost planet, with its scattered component (P14.T21.b).
    Kuiper,
}

/// A body's label for people: host letter, planets lettered from `b` by semi-major axis, moons in
/// Roman numerals, belts numbered (design note 22), such as `A b` or `A d II`.
///
/// Labels are derived for a whole system (P14.T30.c, not built); the designation of record stays
/// plan 01's, which parses back to the ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyLabel(String);

impl BodyLabel {
    /// The label `text`, or `None` if it is empty.
    #[must_use]
    pub fn new(text: String) -> Option<Self> {
        (!text.is_empty()).then_some(Self(text))
    }

    /// The label's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BodyLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Who a body is: its ID, kind, label, parent and state. Every record has one; the kind and the
/// label are what a contact withholds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BodyIdentity {
    system: SystemId,
    index: BodyIndex,
    kind: BodyKind,
    label: Section<BodyLabel>,
    parent: Option<OrbitHost>,
    state: BodyState,
}

impl BodyIdentity {
    /// Body `index` of `system`, of kind `kind`, orbiting `parent`, in state `state`, with its label
    /// [`Section::NotModelled`] until P14.T30.c labels systems.
    ///
    /// `parent` is what the record's body orbits (ruling 53): [`OrbitHost::Star`] for a planet of
    /// one star, [`OrbitHost::Pair`] for a circumbinary one, [`OrbitHost::Barycentre`] for what is
    /// bound to the whole system, and [`OrbitHost::Body`] for a moon's or a ring's planet or a
    /// member's belt. It is `None` only for a system's root host, a free-floating object.
    #[must_use]
    pub const fn new(
        system: SystemId,
        index: BodyIndex,
        kind: BodyKind,
        parent: Option<OrbitHost>,
        state: BodyState,
    ) -> Self {
        Self {
            system,
            index,
            kind,
            label: Section::NotModelled,
            parent,
            state,
        }
    }

    /// The same identity with its label.
    #[must_use]
    pub fn with_label(self, label: BodyLabel) -> Self {
        Self {
            label: Section::Ok(label),
            ..self
        }
    }

    /// The body's system.
    #[must_use]
    pub const fn system(&self) -> SystemId {
        self.system
    }

    /// The body's index in its system.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// The body's ID, plan 01's.
    #[must_use]
    pub const fn id(&self) -> BodyId {
        self.index.body_id(self.system)
    }

    /// What the body is; [`BodyKind::Unresolved`] in a contact.
    #[must_use]
    pub const fn kind(&self) -> BodyKind {
        self.kind
    }

    /// The label for people.
    #[must_use]
    pub const fn label(&self) -> &Section<BodyLabel> {
        &self.label
    }

    /// What the record's body orbits; `None` for a system's root host.
    #[must_use]
    pub const fn parent(&self) -> Option<OrbitHost> {
        self.parent
    }

    /// The body's state at the record's time.
    #[must_use]
    pub const fn state(&self) -> BodyState {
        self.state
    }
}

/// A body's orbit about its primary, as a record holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyOrbit {
    elements: KeplerElements,
    valid_until: Option<UniverseTime>,
}

impl BodyOrbit {
    /// Elements `elements`, which hold until `valid_until`: the next change of the body's state or
    /// orbit that the fate transform (P14.T28) knows of, or `None` if none falls inside the clock
    /// window. The client propagates the elements for drawing up to that time (design note 18).
    #[must_use]
    pub const fn new(elements: KeplerElements, valid_until: Option<UniverseTime>) -> Self {
        Self {
            elements,
            valid_until,
        }
    }

    /// The Kepler elements, in the system frame for a planet and in the parent's for a moon.
    #[must_use]
    pub const fn elements(&self) -> &KeplerElements {
        &self.elements
    }

    /// When the elements stop holding, if inside the clock window.
    #[must_use]
    pub const fn valid_until(&self) -> Option<UniverseTime> {
        self.valid_until
    }
}

/// A body's bulk properties: what [`derive_body`](crate::planetary::derive::derive_body) computes
/// of it, less its mass, which the `MassAndOrbit` level shows in its own section.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BulkProperties {
    radius: EarthRadii,
    density: KilogramsPerCubicMetre,
    surface_gravity: MetresPerSecondSquared,
    class: PlanetClass,
    fractions: MassFractions,
    equilibrium_temperature: Kelvin,
}

impl From<&DerivedBody> for BulkProperties {
    fn from(derived: &DerivedBody) -> Self {
        Self {
            radius: derived.radius(),
            density: derived.density(),
            surface_gravity: derived.surface_gravity(),
            class: derived.class(),
            fractions: derived.fractions(),
            equilibrium_temperature: derived.equilibrium_temperature(),
        }
    }
}

impl BulkProperties {
    /// The radius at the record's time.
    #[must_use]
    pub const fn radius(&self) -> EarthRadii {
        self.radius
    }

    /// The mean density.
    #[must_use]
    pub const fn density(&self) -> KilogramsPerCubicMetre {
        self.density
    }

    /// The gravity at the radius.
    #[must_use]
    pub const fn surface_gravity(&self) -> MetresPerSecondSquared {
        self.surface_gravity
    }

    /// The class by composition.
    #[must_use]
    pub const fn class(&self) -> PlanetClass {
        self.class
    }

    /// The mass fractions of iron, rock, water and envelope.
    #[must_use]
    pub const fn fractions(&self) -> MassFractions {
        self.fractions
    }

    /// The equilibrium temperature at the record's time.
    #[must_use]
    pub const fn equilibrium_temperature(&self) -> Kelvin {
        self.equilibrium_temperature
    }
}

/// The contents of a surface section: atmosphere, surface conditions, rotation and global figures
/// (design note 16), which P14.T13, T14 and T24 compute.
///
/// None of them is built, so the type has no value, and no record can hold a surface section that
/// is [`Section::Ok`]: every surface is [`Section::NotModelled`], or [`Section::NotApplicable`] for
/// a giant. The tasks that compute it give it its contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Surface {}

/// The contents of a hooks section: surface seed, bulk composition, habitability and resources
/// (design note 16), which P14.T23–T26 compute.
///
/// None of them is built, so the type has no value, and every hooks section is
/// [`Section::NotModelled`]. The tasks that compute them give it its contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Hooks {}

/// What a body query returns: a body at a time, at a detail level (design note 16).
///
/// Built at [`DetailLevel::Full`] by [`BodyRecord::builder`], and reduced by
/// [`degrade`](Self::degrade).
///
/// # Examples
///
/// A planet's record degraded to mass and orbit withholds its bulk, and to a contact its kind:
///
/// ```
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::fate::BodyState;
/// use hyperion_sim::planetary::placement::OrbitHost;
/// use hyperion_sim::planetary::record::{
///     BodyIdentity, BodyKind, BodyRecord, DetailLevel, RecordSection, Section, SectionState,
/// };
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::units::EarthMasses;
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let index = BodyIndex::new(BodySlot::Planet(3), BodySub::Primary)?;
/// let identity =
///     BodyIdentity::new(system, index, BodyKind::Planet, Some(OrbitHost::Star(0)), BodyState::Present);
/// let record = BodyRecord::builder(identity)
///     .mass(Section::Ok(EarthMasses::new(1.0)))
///     .rings(Section::NotApplicable)
///     .build()?;
/// let orbit_only = record.degrade(DetailLevel::MassAndOrbit);
/// assert_eq!(orbit_only.mass(), &Section::Ok(EarthMasses::new(1.0)));
/// assert_eq!(orbit_only.section_state(RecordSection::Bulk), SectionState::NotResolved);
/// let contact = record.degrade(DetailLevel::Contact);
/// assert_eq!(contact.identity().kind(), BodyKind::Unresolved);
/// assert_eq!(contact.section_state(RecordSection::Rings), SectionState::NotResolved);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BodyRecord {
    level: DetailLevel,
    identity: BodyIdentity,
    position: Option<SystemPosition>,
    mass: Section<EarthMasses>,
    orbit: Section<BodyOrbit>,
    moons: Section<Vec<BodyIndex>>,
    rings: Section<Vec<BodyIndex>>,
    bulk: Section<BulkProperties>,
    surface: Section<Surface>,
    hooks: Section<Hooks>,
}

impl BodyRecord {
    /// A builder of the record of `identity` at [`DetailLevel::Full`], with every section
    /// [`Section::NotModelled`] and no position until set.
    #[must_use]
    pub const fn builder(identity: BodyIdentity) -> BodyRecordBuilder {
        BodyRecordBuilder {
            identity,
            position: None,
            mass: Section::NotModelled,
            orbit: Section::NotModelled,
            moons: Section::NotModelled,
            rings: Section::NotModelled,
            bulk: Section::NotModelled,
            surface: Section::NotModelled,
            hooks: Section::NotModelled,
        }
    }

    /// The detail level the record holds.
    #[must_use]
    pub const fn level(&self) -> DetailLevel {
        self.level
    }

    /// Who the body is.
    #[must_use]
    pub const fn identity(&self) -> &BodyIdentity {
        &self.identity
    }

    /// The body's index in its system.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.identity.index
    }

    /// Where the body is at the record's time, in the system frame (plan 01's `coords`); `None`
    /// for a body that is not present, or a population with no single position.
    #[must_use]
    pub const fn position(&self) -> Option<SystemPosition> {
        self.position
    }

    /// The mass.
    #[must_use]
    pub const fn mass(&self) -> &Section<EarthMasses> {
        &self.mass
    }

    /// The orbit about the body's primary.
    #[must_use]
    pub const fn orbit(&self) -> &Section<BodyOrbit> {
        &self.orbit
    }

    /// The body's moons, by index; an empty list is a body with none.
    #[must_use]
    pub const fn moons(&self) -> &Section<Vec<BodyIndex>> {
        &self.moons
    }

    /// The body's rings, by index; an empty list is a body with none.
    #[must_use]
    pub const fn rings(&self) -> &Section<Vec<BodyIndex>> {
        &self.rings
    }

    /// The bulk properties.
    #[must_use]
    pub const fn bulk(&self) -> &Section<BulkProperties> {
        &self.bulk
    }

    /// The surface.
    #[must_use]
    pub const fn surface(&self) -> &Section<Surface> {
        &self.surface
    }

    /// The hooks.
    #[must_use]
    pub const fn hooks(&self) -> &Section<Hooks> {
        &self.hooks
    }

    /// The state of the section `section`.
    #[must_use]
    pub const fn section_state(&self, section: RecordSection) -> SectionState {
        match section {
            RecordSection::Label => self.identity.label.state(),
            RecordSection::Mass => self.mass.state(),
            RecordSection::Orbit => self.orbit.state(),
            RecordSection::Moons => self.moons.state(),
            RecordSection::Rings => self.rings.state(),
            RecordSection::Bulk => self.bulk.state(),
            RecordSection::Surface => self.surface.state(),
            RecordSection::Hooks => self.hooks.state(),
        }
    }

    /// The record as a reader granted `level` holds it (design note 16).
    ///
    /// Every section above the level becomes [`Section::NotResolved`], whatever its state, and
    /// every section at or below it is untouched; at [`DetailLevel::Contact`] the kind also becomes
    /// [`BodyKind::Unresolved`]. The ID, the parent, the state and the position are always kept.
    ///
    /// Degrading never adds detail: the result holds the lower of `level` and the record's own
    /// level, so `degrade(a).degrade(b)` is `degrade(min(a, b))`, and `degrade` is idempotent.
    #[must_use]
    pub fn degrade(&self, level: DetailLevel) -> Self {
        let granted = self.level.min(level);
        let at = |section: RecordSection| section.level();
        let kind = if granted < DetailLevel::MassAndOrbit {
            BodyKind::Unresolved
        } else {
            self.identity.kind
        };
        Self {
            level: granted,
            identity: BodyIdentity {
                system: self.identity.system,
                index: self.identity.index,
                kind,
                label: self
                    .identity
                    .label
                    .granted(at(RecordSection::Label), granted),
                parent: self.identity.parent,
                state: self.identity.state,
            },
            position: self.position,
            mass: self.mass.granted(at(RecordSection::Mass), granted),
            orbit: self.orbit.granted(at(RecordSection::Orbit), granted),
            moons: self.moons.granted(at(RecordSection::Moons), granted),
            rings: self.rings.granted(at(RecordSection::Rings), granted),
            bulk: self.bulk.granted(at(RecordSection::Bulk), granted),
            surface: self.surface.granted(at(RecordSection::Surface), granted),
            hooks: self.hooks.granted(at(RecordSection::Hooks), granted),
        }
    }
}

/// Builds a [`BodyRecord`] at [`DetailLevel::Full`]; see [`BodyRecord::builder`].
#[derive(Debug, Clone, PartialEq)]
pub struct BodyRecordBuilder {
    identity: BodyIdentity,
    position: Option<SystemPosition>,
    mass: Section<EarthMasses>,
    orbit: Section<BodyOrbit>,
    moons: Section<Vec<BodyIndex>>,
    rings: Section<Vec<BodyIndex>>,
    bulk: Section<BulkProperties>,
    surface: Section<Surface>,
    hooks: Section<Hooks>,
}

impl BodyRecordBuilder {
    /// The body's position at the record's time.
    #[must_use]
    pub fn position(self, position: SystemPosition) -> Self {
        Self {
            position: Some(position),
            ..self
        }
    }

    /// The mass section.
    #[must_use]
    pub fn mass(self, mass: Section<EarthMasses>) -> Self {
        Self { mass, ..self }
    }

    /// The orbit section.
    #[must_use]
    pub fn orbit(self, orbit: Section<BodyOrbit>) -> Self {
        Self { orbit, ..self }
    }

    /// The moons section.
    #[must_use]
    pub fn moons(self, moons: Section<Vec<BodyIndex>>) -> Self {
        Self { moons, ..self }
    }

    /// The rings section.
    #[must_use]
    pub fn rings(self, rings: Section<Vec<BodyIndex>>) -> Self {
        Self { rings, ..self }
    }

    /// The bulk section.
    #[must_use]
    pub fn bulk(self, bulk: Section<BulkProperties>) -> Self {
        Self { bulk, ..self }
    }

    /// The surface section.
    #[must_use]
    pub fn surface(self, surface: Section<Surface>) -> Self {
        Self { surface, ..self }
    }

    /// The hooks section.
    #[must_use]
    pub fn hooks(self, hooks: Section<Hooks>) -> Self {
        Self { hooks, ..self }
    }

    /// The sections that `derived` fills, as the vertical slice tags them: the mass and bulk
    /// [`Section::Ok`], and the surface [`Section::NotApplicable`] for a class with none (a giant)
    /// and [`Section::NotModelled`] otherwise, until P14.T13 computes it.
    #[must_use]
    pub fn derived(self, derived: &DerivedBody) -> Self {
        let surface = if derived.class().has_surface() {
            Section::NotModelled
        } else {
            Section::NotApplicable
        };
        Self {
            mass: Section::Ok(derived.mass()),
            bulk: Section::Ok(BulkProperties::from(derived)),
            surface,
            ..self
        }
    }

    /// The record.
    ///
    /// # Errors
    ///
    /// [`BuildBodyRecordError::NotResolved`] naming the first section that is
    /// [`Section::NotResolved`], and [`BuildBodyRecordError::UnresolvedKind`] for a kind of
    /// [`BodyKind::Unresolved`]: both are what degrading produces, never what a generator states.
    pub fn build(self) -> Result<BodyRecord, BuildBodyRecordError> {
        if self.identity.kind == BodyKind::Unresolved {
            return Err(BuildBodyRecordError::UnresolvedKind);
        }
        let record = BodyRecord {
            level: DetailLevel::Full,
            identity: self.identity,
            position: self.position,
            mass: self.mass,
            orbit: self.orbit,
            moons: self.moons,
            rings: self.rings,
            bulk: self.bulk,
            surface: self.surface,
            hooks: self.hooks,
        };
        match RecordSection::ALL
            .into_iter()
            .find(|&section| record.section_state(section) == SectionState::NotResolved)
        {
            Some(section) => Err(BuildBodyRecordError::NotResolved(section)),
            None => Ok(record),
        }
    }
}

/// A [`BodyRecord`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildBodyRecordError {
    /// A section was [`Section::NotResolved`], which only degrading produces.
    NotResolved(RecordSection),
    /// The kind was [`BodyKind::Unresolved`], which only degrading to a contact produces.
    UnresolvedKind,
}

impl fmt::Display for BuildBodyRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotResolved(section) => write!(
                f,
                "a new record's {section} section cannot be not resolved, which only degrading makes"
            ),
            Self::UnresolvedKind => f.write_str(
                "a new record's kind cannot be unresolved, which only degrading to a contact makes",
            ),
        }
    }
}

impl Error for BuildBodyRecordError {}

/// The sections of a [`SystemSnapshot`] beyond its bodies' records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemSection {
    /// The system's belts.
    Belts,
    /// The system's cometary halo.
    Halo,
}

impl SystemSection {
    /// The least detail level that holds the section: both say what bodies are, which a contact
    /// withholds.
    #[must_use]
    pub const fn level(self) -> DetailLevel {
        match self {
            Self::Belts | Self::Halo => DetailLevel::MassAndOrbit,
        }
    }
}

impl fmt::Display for SystemSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Belts => "belts",
            Self::Halo => "halo",
        })
    }
}

/// Every body of a system at a time, at one detail level: what a system query returns (P14.T30.b,
/// `snapshot_at`).
///
/// The bodies' records are in index order. A belt, the halo and a belt's members are bodies too
/// (design note 3, slots `0xE0`–`0xEF`); the `belts` and `halo` sections say which bodies they
/// are, so that a reader can tell "no belts" (`Ok` and empty, `Ok(None)` for the halo) from "belts
/// not yet modelled".
#[derive(Debug, Clone, PartialEq)]
pub struct SystemSnapshot {
    system: SystemId,
    time: UniverseTime,
    level: DetailLevel,
    bodies: Vec<BodyRecord>,
    belts: Section<Vec<BodyIndex>>,
    halo: Section<Option<BodyIndex>>,
}

impl SystemSnapshot {
    /// The snapshot of `system` at `time`, at [`DetailLevel::Full`], of `bodies`, each at
    /// [`DetailLevel::Full`], with its belts and halo [`Section::NotModelled`] until
    /// [`with_populations`](Self::with_populations) sets them (P14.T21).
    ///
    /// # Errors
    ///
    /// - [`BuildSystemSnapshotError::ForeignBody`] for a body of another system.
    /// - [`BuildSystemSnapshotError::NotInIndexOrder`] for a body whose index is not above the
    ///   one before it.
    /// - [`BuildSystemSnapshotError::DegradedBody`] for a body below [`DetailLevel::Full`]: a
    ///   snapshot is degraded as a whole.
    pub fn new(
        system: SystemId,
        time: UniverseTime,
        bodies: Vec<BodyRecord>,
    ) -> Result<Self, BuildSystemSnapshotError> {
        let mut previous: Option<BodyIndex> = None;
        for body in &bodies {
            let index = body.index();
            if body.identity.system != system {
                return Err(BuildSystemSnapshotError::ForeignBody { index });
            }
            if previous.is_some_and(|before| before >= index) {
                return Err(BuildSystemSnapshotError::NotInIndexOrder { index });
            }
            if body.level != DetailLevel::Full {
                return Err(BuildSystemSnapshotError::DegradedBody { index });
            }
            previous = Some(index);
        }
        Ok(Self {
            system,
            time,
            level: DetailLevel::Full,
            bodies,
            belts: Section::NotModelled,
            halo: Section::NotModelled,
        })
    }

    /// The same snapshot with its belts `belts` and its halo `halo`, which phase D computes
    /// (P14.T21): each index that an `Ok` section names must be a body of the snapshot of that kind.
    ///
    /// # Errors
    ///
    /// - [`BuildSystemSnapshotError::NotResolved`] for a section that is
    ///   [`Section::NotResolved`], which only degrading produces.
    /// - [`BuildSystemSnapshotError::NotAPopulation`] for an index that is not a body of the
    ///   snapshot, or whose kind is not a belt (for `belts`) or a cometary halo (for `halo`).
    pub fn with_populations(
        self,
        belts: Section<Vec<BodyIndex>>,
        halo: Section<Option<BodyIndex>>,
    ) -> Result<Self, BuildSystemSnapshotError> {
        if belts.state() == SectionState::NotResolved {
            return Err(BuildSystemSnapshotError::NotResolved(SystemSection::Belts));
        }
        if halo.state() == SectionState::NotResolved {
            return Err(BuildSystemSnapshotError::NotResolved(SystemSection::Halo));
        }
        let is = |index: BodyIndex, wanted: fn(BodyKind) -> bool| {
            self.body(index)
                .is_some_and(|body| wanted(body.identity.kind))
        };
        let listed_belts = belts.ok().map_or(&[][..], Vec::as_slice);
        if let Some(&index) = listed_belts
            .iter()
            .find(|&&index| !is(index, |kind| matches!(kind, BodyKind::Belt(_))))
        {
            return Err(BuildSystemSnapshotError::NotAPopulation {
                section: SystemSection::Belts,
                index,
            });
        }
        if let Some(&Some(index)) = halo.ok()
            && !is(index, |kind| kind == BodyKind::CometaryHalo)
        {
            return Err(BuildSystemSnapshotError::NotAPopulation {
                section: SystemSection::Halo,
                index,
            });
        }
        Ok(Self {
            belts,
            halo,
            ..self
        })
    }

    /// The system.
    #[must_use]
    pub const fn system(&self) -> SystemId {
        self.system
    }

    /// The time the snapshot is of.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// The detail level the snapshot holds, which every body's record holds too.
    #[must_use]
    pub const fn level(&self) -> DetailLevel {
        self.level
    }

    /// The bodies' records, in index order.
    #[must_use]
    pub fn bodies(&self) -> &[BodyRecord] {
        &self.bodies
    }

    /// The record of body `index`, if the snapshot holds it.
    #[must_use]
    pub fn body(&self, index: BodyIndex) -> Option<&BodyRecord> {
        self.bodies
            .binary_search_by(|body| body.index().cmp(&index))
            .ok()
            .map(|at| &self.bodies[at])
    }

    /// The system's belts, by the index of each belt's population.
    #[must_use]
    pub const fn belts(&self) -> &Section<Vec<BodyIndex>> {
        &self.belts
    }

    /// The system's cometary halo, by its index; `Ok(None)` for a system with none.
    #[must_use]
    pub const fn halo(&self) -> &Section<Option<BodyIndex>> {
        &self.halo
    }

    /// The state of the section `section`.
    #[must_use]
    pub const fn section_state(&self, section: SystemSection) -> SectionState {
        match section {
            SystemSection::Belts => self.belts.state(),
            SystemSection::Halo => self.halo.state(),
        }
    }

    /// The snapshot as a reader granted `level` holds it: every body's record degraded to the
    /// level ([`BodyRecord::degrade`]), the belts and halo [`Section::NotResolved`] at a contact,
    /// and below [`DetailLevel::Bulk`] no record of a belt's members, which a population seen as a
    /// whole does not resolve (P14.T34).
    ///
    /// As for a record, `degrade(a).degrade(b)` is `degrade(min(a, b))`.
    #[must_use]
    pub fn degrade(&self, level: DetailLevel) -> Self {
        let granted = self.level.min(level);
        let bodies = self
            .bodies
            .iter()
            .filter(|body| {
                granted >= DetailLevel::Bulk || !matches!(body.index().sub(), BodySub::Member(_))
            })
            .map(|body| body.degrade(granted))
            .collect();
        Self {
            system: self.system,
            time: self.time,
            level: granted,
            bodies,
            belts: self.belts.granted(SystemSection::Belts.level(), granted),
            halo: self.halo.granted(SystemSection::Halo.level(), granted),
        }
    }
}

/// A [`SystemSnapshot`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSystemSnapshotError {
    /// A body's record is of another system.
    ForeignBody {
        /// The body's index.
        index: BodyIndex,
    },
    /// A body's index is not above the one before it.
    NotInIndexOrder {
        /// The body's index.
        index: BodyIndex,
    },
    /// A body's record is below [`DetailLevel::Full`].
    DegradedBody {
        /// The body's index.
        index: BodyIndex,
    },
    /// A section was [`Section::NotResolved`], which only degrading produces.
    NotResolved(SystemSection),
    /// A section names an index that is not a body of the snapshot of the section's kind.
    NotAPopulation {
        /// The section.
        section: SystemSection,
        /// The index it names.
        index: BodyIndex,
    },
}

impl fmt::Display for BuildSystemSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignBody { index } => {
                write!(f, "body {:#06x} belongs to another system", index.get())
            }
            Self::NotInIndexOrder { index } => {
                write!(f, "body {:#06x} is out of index order", index.get())
            }
            Self::DegradedBody { index } => write!(
                f,
                "body {:#06x} is degraded, and a snapshot is degraded as a whole",
                index.get()
            ),
            Self::NotResolved(section) => write!(
                f,
                "a new snapshot's {section} section cannot be not resolved, which only degrading makes"
            ),
            Self::NotAPopulation { section, index } => write!(
                f,
                "the {section} section names body {:#06x}, which is not one of its bodies",
                index.get()
            ),
        }
    }
}

impl Error for BuildSystemSnapshotError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::derive::solar::{orbit, solar_system};
    use crate::planetary::index::BodySlot;
    use crate::units::consts::METRES_PER_AU;

    fn system() -> SystemId {
        SystemId::from_raw(0x0200_0800_2000_0000).unwrap()
    }

    fn index(slot: BodySlot, sub: BodySub) -> BodyIndex {
        BodyIndex::new(slot, sub).unwrap()
    }

    fn planet(n: u8) -> BodyIndex {
        index(BodySlot::Planet(n), BodySub::Primary)
    }

    fn identity(at: BodyIndex, kind: BodyKind, parent: Option<OrbitHost>) -> BodyIdentity {
        BodyIdentity::new(system(), at, kind, parent, BodyState::Present)
    }

    fn at_au(a: f64) -> SystemPosition {
        SystemPosition::new([a * METRES_PER_AU, 0.0, 0.0])
    }

    /// The slice's record of a planet: derived, on an orbit, at a position.
    fn planet_record(n: u8, derived: &DerivedBody, a_au: f64) -> BodyRecord {
        BodyRecord::builder(identity(
            planet(n),
            BodyKind::Planet,
            Some(OrbitHost::Star(0)),
        ))
        .position(at_au(a_au))
        .orbit(Section::Ok(BodyOrbit::new(orbit(a_au, 0.0), None)))
        .derived(derived)
        .build()
        .unwrap()
    }

    fn earth_and_saturn() -> (DerivedBody, DerivedBody) {
        let bodies = solar_system();
        let find = |name: &str| bodies.iter().find(|(n, _)| *n == name).unwrap().1;
        (find("Earth"), find("Saturn"))
    }

    /// A record with each state in some section, and a label.
    fn mixed_record() -> BodyRecord {
        let (earth, _) = earth_and_saturn();
        let moon = index(BodySlot::Planet(3), BodySub::Moon(1));
        let named = identity(planet(3), BodyKind::Planet, Some(OrbitHost::Star(0)))
            .with_label(BodyLabel::new("A d".to_owned()).unwrap());
        BodyRecord::builder(named)
            .position(at_au(1.0))
            .orbit(Section::Ok(BodyOrbit::new(orbit(1.0, 0.0167), None)))
            .derived(&earth)
            .moons(Section::Ok(vec![moon]))
            .rings(Section::NotApplicable)
            .build()
            .unwrap()
    }

    fn records() -> Vec<BodyRecord> {
        let (earth, saturn) = earth_and_saturn();
        let moon = BodyRecord::builder(identity(
            index(BodySlot::Planet(3), BodySub::Moon(1)),
            BodyKind::Moon(MoonOrigin::GiantImpact),
            Some(OrbitHost::Body(planet(3))),
        ))
        .moons(Section::NotApplicable)
        .rings(Section::NotApplicable)
        .build()
        .unwrap();
        vec![
            planet_record(3, &earth, 1.0),
            planet_record(6, &saturn, 9.58),
            mixed_record(),
            moon,
        ]
    }

    #[test]
    fn degrade_is_idempotent_and_monotone() {
        for record in records() {
            for a in DetailLevel::ALL {
                let once = record.degrade(a);
                assert_eq!(once.degrade(a), once, "idempotent at {a:?}");
                assert_eq!(once.level(), a);
                for b in DetailLevel::ALL {
                    assert_eq!(
                        once.degrade(b),
                        record.degrade(a.min(b)),
                        "{a:?} then {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn degrade_withholds_exactly_the_sections_above_the_level() {
        for record in records() {
            for level in DetailLevel::ALL {
                let degraded = record.degrade(level);
                for section in RecordSection::ALL {
                    let expected = if section.level() > level {
                        SectionState::NotResolved
                    } else {
                        record.section_state(section)
                    };
                    let got = degraded.section_state(section);
                    assert_eq!(got, expected, "{section} at {level:?}");
                }
                let identity = degraded.identity();
                assert_eq!(identity.id(), record.identity().id());
                assert_eq!(identity.parent(), record.identity().parent());
                assert_eq!(identity.state(), record.identity().state());
                assert_eq!(degraded.position(), record.position());
                let kind = if level == DetailLevel::Contact {
                    BodyKind::Unresolved
                } else {
                    record.identity().kind()
                };
                assert_eq!(identity.kind(), kind, "{level:?}");
            }
        }
    }

    #[test]
    fn a_mass_and_orbit_record_holds_no_radius_temperature_or_composition() {
        // Plan 14's test serialises such a record to JSON and finds no radius, temperature or
        // composition key; `serde` is not a dependency of the sim, so the JSON half belongs to
        // P14.T35's wire types, and here the sections that hold those values are withheld.
        let record = mixed_record();
        assert!(record.bulk().ok().is_some());
        let degraded = record.degrade(DetailLevel::MassAndOrbit);
        assert_eq!(degraded.bulk(), &Section::NotResolved);
        assert_eq!(degraded.surface(), &Section::NotResolved);
        assert_eq!(degraded.hooks(), &Section::NotResolved);
        assert_eq!(degraded.mass(), record.mass());
        assert_eq!(degraded.orbit(), record.orbit());
        assert_eq!(degraded.moons(), record.moons());
        assert_eq!(degraded.identity().label(), record.identity().label());
        assert_eq!(degraded.identity().kind(), BodyKind::Planet);
    }

    #[test]
    fn a_contact_keeps_the_id_and_the_position_and_withholds_the_label() {
        let record = mixed_record();
        let contact = record.degrade(DetailLevel::Contact);
        assert_eq!(contact.identity().kind(), BodyKind::Unresolved);
        assert_eq!(contact.identity().label(), &Section::NotResolved);
        assert_eq!(contact.identity().id(), record.identity().id());
        assert_eq!(contact.position(), Some(at_au(1.0)));
        for section in RecordSection::ALL {
            assert_eq!(contact.section_state(section), SectionState::NotResolved);
        }
    }

    #[test]
    fn in_the_slice_a_planet_s_surface_and_hooks_are_not_modelled_and_a_giant_has_no_surface() {
        let (earth, saturn) = earth_and_saturn();
        let earth = planet_record(3, &earth, 1.0);
        assert_eq!(earth.level(), DetailLevel::Full);
        for (section, state) in [
            (RecordSection::Label, SectionState::NotModelled),
            (RecordSection::Mass, SectionState::Ok),
            (RecordSection::Orbit, SectionState::Ok),
            (RecordSection::Moons, SectionState::NotModelled),
            (RecordSection::Rings, SectionState::NotModelled),
            (RecordSection::Bulk, SectionState::Ok),
            (RecordSection::Surface, SectionState::NotModelled),
            (RecordSection::Hooks, SectionState::NotModelled),
        ] {
            assert_eq!(earth.section_state(section), state, "Earth's {section}");
        }
        let bulk = earth.bulk().ok().unwrap();
        assert_eq!(bulk.class(), PlanetClass::Rocky);
        assert!((bulk.radius().value() - 1.0).abs() < 1e-9);
        let saturn = planet_record(6, &saturn, 9.58);
        assert_eq!(saturn.surface(), &Section::NotApplicable);
        assert_eq!(saturn.bulk().ok().unwrap().class(), PlanetClass::GasGiant);
        let snapshot =
            SystemSnapshot::new(system(), UniverseTime::EPOCH, vec![earth, saturn]).unwrap();
        assert_eq!(snapshot.level(), DetailLevel::Full);
        assert_eq!(
            snapshot.section_state(SystemSection::Belts),
            SectionState::NotModelled
        );
        assert_eq!(
            snapshot.section_state(SystemSection::Halo),
            SectionState::NotModelled
        );
        assert_eq!(
            snapshot.body(planet(6)).unwrap().surface(),
            &Section::NotApplicable
        );
    }

    #[test]
    fn only_degrading_withholds() {
        let named = identity(planet(1), BodyKind::Planet, Some(OrbitHost::Star(0)));
        assert_eq!(
            BodyRecord::builder(named.clone())
                .bulk(Section::NotResolved)
                .build(),
            Err(BuildBodyRecordError::NotResolved(RecordSection::Bulk))
        );
        let unresolved = identity(planet(1), BodyKind::Unresolved, None);
        assert_eq!(
            BodyRecord::builder(unresolved).build(),
            Err(BuildBodyRecordError::UnresolvedKind)
        );
        let record = BodyRecord::builder(named).build().unwrap();
        let snapshot = SystemSnapshot::new(system(), UniverseTime::EPOCH, vec![record]).unwrap();
        assert_eq!(
            snapshot
                .clone()
                .with_populations(Section::NotResolved, Section::Ok(None)),
            Err(BuildSystemSnapshotError::NotResolved(SystemSection::Belts))
        );
        assert_eq!(
            snapshot.with_populations(Section::Ok(vec![]), Section::NotResolved),
            Err(BuildSystemSnapshotError::NotResolved(SystemSection::Halo))
        );
    }

    #[test]
    fn the_populations_must_name_bodies_of_their_kind() {
        let snapshot = belted_snapshot();
        let belt = index(BodySlot::Belt(0), BodySub::Primary);
        assert_eq!(
            snapshot
                .clone()
                .with_populations(Section::Ok(vec![planet(3)]), Section::Ok(None)),
            Err(BuildSystemSnapshotError::NotAPopulation {
                section: SystemSection::Belts,
                index: planet(3)
            })
        );
        assert_eq!(
            snapshot
                .clone()
                .with_populations(Section::Ok(vec![belt]), Section::Ok(Some(belt))),
            Err(BuildSystemSnapshotError::NotAPopulation {
                section: SystemSection::Halo,
                index: belt
            })
        );
        let absent = index(BodySlot::Belt(1), BodySub::Primary);
        assert_eq!(
            snapshot.with_populations(Section::Ok(vec![absent]), Section::NotModelled),
            Err(BuildSystemSnapshotError::NotAPopulation {
                section: SystemSection::Belts,
                index: absent
            })
        );
    }

    /// A snapshot of a planet, an asteroid belt with one named member, and no halo.
    fn belted_snapshot() -> SystemSnapshot {
        let (earth, _) = earth_and_saturn();
        let belt = index(BodySlot::Belt(0), BodySub::Primary);
        let member = index(BodySlot::Belt(0), BodySub::Member(1));
        let population = BodyRecord::builder(identity(
            belt,
            BodyKind::Belt(BeltKind::Asteroid),
            Some(OrbitHost::Star(0)),
        ))
        .orbit(Section::NotApplicable)
        .build()
        .unwrap();
        let ceres = BodyRecord::builder(identity(
            member,
            BodyKind::DwarfPlanet,
            Some(OrbitHost::Body(belt)),
        ))
        .position(at_au(2.77))
        .build()
        .unwrap();
        let bodies = vec![planet_record(3, &earth, 1.0), population, ceres];
        SystemSnapshot::new(system(), UniverseTime::EPOCH, bodies)
            .unwrap()
            .with_populations(Section::Ok(vec![belt]), Section::Ok(None))
            .unwrap()
    }

    #[test]
    fn below_bulk_a_snapshot_drops_its_belts_members() {
        let snapshot = belted_snapshot();
        let member = index(BodySlot::Belt(0), BodySub::Member(1));
        for level in DetailLevel::ALL {
            let degraded = snapshot.degrade(level);
            assert_eq!(degraded.level(), level);
            assert_eq!(
                degraded.body(member).is_some(),
                level >= DetailLevel::Bulk,
                "{level:?}"
            );
            let withheld = level == DetailLevel::Contact;
            assert_eq!(
                degraded.belts().state() == SectionState::NotResolved,
                withheld
            );
            assert_eq!(
                degraded.halo().state() == SectionState::NotResolved,
                withheld
            );
            for body in degraded.bodies() {
                assert_eq!(body.level(), level);
            }
        }
    }

    #[test]
    fn a_snapshot_s_degrade_is_idempotent_and_monotone() {
        let snapshot = belted_snapshot();
        for a in DetailLevel::ALL {
            let once = snapshot.degrade(a);
            assert_eq!(once.degrade(a), once);
            for b in DetailLevel::ALL {
                assert_eq!(
                    once.degrade(b),
                    snapshot.degrade(a.min(b)),
                    "{a:?} then {b:?}"
                );
            }
        }
    }

    #[test]
    fn a_snapshot_holds_its_own_full_records_in_index_order() {
        let record = |n: u8| {
            BodyRecord::builder(identity(
                planet(n),
                BodyKind::Planet,
                Some(OrbitHost::Star(0)),
            ))
            .build()
            .unwrap()
        };
        let make = |bodies| SystemSnapshot::new(system(), UniverseTime::EPOCH, bodies);
        assert_eq!(
            make(vec![record(2), record(1)]),
            Err(BuildSystemSnapshotError::NotInIndexOrder { index: planet(1) })
        );
        assert_eq!(
            make(vec![record(1), record(1)]),
            Err(BuildSystemSnapshotError::NotInIndexOrder { index: planet(1) })
        );
        assert_eq!(
            make(vec![record(1).degrade(DetailLevel::Bulk)]),
            Err(BuildSystemSnapshotError::DegradedBody { index: planet(1) })
        );
        let other = SystemId::from_raw(0x0200_0800_2000_0001).unwrap();
        let foreign = BodyRecord::builder(BodyIdentity::new(
            other,
            planet(1),
            BodyKind::Planet,
            None,
            BodyState::Present,
        ))
        .build()
        .unwrap();
        assert_eq!(
            make(vec![foreign]),
            Err(BuildSystemSnapshotError::ForeignBody { index: planet(1) })
        );
        assert!(make(vec![record(1), record(2)]).is_ok());
    }

    #[test]
    fn a_section_maps_its_value_and_keeps_its_state() {
        assert_eq!(Section::Ok(2).map(|x| x * 3), Section::Ok(6));
        assert_eq!(
            Section::<i32>::NotApplicable.map(|x| x * 3),
            Section::NotApplicable
        );
        assert_eq!(Section::Ok(2).ok(), Some(&2));
        assert_eq!(Section::<i32>::NotModelled.ok(), None);
        assert_eq!(
            Section::<i32>::NotResolved.state(),
            SectionState::NotResolved
        );
        assert!(BodyLabel::new(String::new()).is_none());
        let label = BodyLabel::new("A b".to_owned()).unwrap();
        assert_eq!((label.to_string().as_str(), label.as_str()), ("A b", "A b"));
    }
}
