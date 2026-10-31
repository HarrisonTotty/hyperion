//! The global list: the few large features that no catalogue cell can hold (plan 10).
//!
//! The brainstorm's "Large features" keeps one list per galaxy for what is too big or too sparse
//! for the feature catalogue's cells: the galactic centre (plan 09's, entry 0), the stellar
//! streams of the halo and the cores of accreted dwarf galaxies ("Streams and accreted
//! structure"). Their members carry IDs under the `10` prefix, plan 01's [`StreamMemberId`] and
//! [`DwarfCoreMemberId`], whose number fields this module gives a meaning.
//!
//! # Numbering
//!
//! A stream's number is its place in a fixed order (plan 10, Design note 12): the living globular
//! clusters with a tube, in the order of the catalogue's walk over the globulars, then the orphan
//! streams, then the streams of the recent dwarf progenitors by accretion time. A dwarf core's
//! number is its progenitor's place among the progenitors that keep a core, in the same order.
//! Both are pure functions of the seed, so an ID read back from a save names the same stream or
//! core in every run. Beside the list ([`GlobalList`], P10.T4) this module holds the numbers, the
//! kinds, the cache trait, the orbit integrator ([`orbit`]), the debris the list is built from
//! ([`Debris`]: which progenitors get a tube or a core, P10.T3, with [`spec`], [`orphans`] and
//! [`dwarfs`]), the halo's discrete share it makes up ([`discrete_share`], P10.T3.d.a, in
//! [`share`]), the dwarf cores' model ([`dwarf_core`], P10.T7.a), the particle spray that
//! measures a stream's track ([`spray`], P10.T5) and the tube table it is reduced to ([`tube`],
//! P10.T6.a).
//!
//! # The list
//!
//! [`GlobalList::build`] gives one [`GlobalEntry`] per feature: entry 0 the centre
//! ([`CentreModel::as_global_entry`]), then every stream in number order, then every dwarf core,
//! each with its mass and a [`BoundingShell`], a spherical shell about the galactic centre,
//! clipped to the root cube, that holds every member it can place. A query tests the shells first
//! (brainstorm, "Streams and accreted structure"), so a stream whose shell misses the query costs
//! no tube table. The list is a plain value the caller keeps with the galaxy, as it keeps the tube
//! tables; it holds the [`Debris`] it was built from, which the tube tables and the cores' models
//! are made from.
//!
//! # Bounding shells
//!
//! A shell is estimated before any tracer is sprayed, from one integrated orbit per entry. A
//! stream's progenitor is integrated from its epoch state backwards over its stripping time `T_s`,
//! the steps of the spray's own backward pass in the spray's table of the potential
//! ([`spray::SprayForce`], at [`SphericalOrbit::fixed_step`]), and, if `T_s` is shorter than a
//! radial period, forwards over `T_s` as well, since the leading arm lies ahead of the progenitor
//! by less than the progenitor's own advance over `T_s`. An orbit that spans a radial period has
//! passed both its turning points. The least and greatest distances from the centre over it are
//! then widened, each at its own radius `r`, by
//!
//! - four times the expected width, [`CUT_WIDTHS`] × `r r_t ÷ r_p`, since the tube is cut at four
//!   of its local widths: the debris spreads about its orbit by an angle of about the tidal radius
//!   over the pericentre, Johnston, Sackett and Bullock's `s = (m ÷ M)^⅓` (2001, ApJ 557, 137;
//!   some 1.26 `r_t ÷ r_p` with the Jacobi `r_t` used here, [`spec`]), so its width grows with the
//!   distance from the centre;
//! - and the spray's spread in its turning points, [`SHELL_WIDENING`] × `η r`, with `η = C r_t ÷
//!   r_p` the fractional spread in energy across the debris of Design note 5
//!   ([`spec::FREQUENCY_SPREAD`] is `C`). In a flat rotation curve a turning point moves by
//!   `d ln r = (ΔE ÷ v_c² − x² ΔL ÷ L) ÷ (1 − x²)`, `x = v_φ ÷ v_c` there: by `η` if `E` and `L`
//!   change together by `η`, but a release at the tidal radius near pericentre changes `E` by
//!   more, so the pericentres spread by about `η r ÷ 2` and the apocentres by `η r` to `3 η r` as
//!   `r_a ÷ r_p` runs from 1 to 10, the tails of the release offsets further.
//!
//! `r_t` is the progenitor's tidal radius at its pericentre, the larger of the specification's and
//! the one at its mass at the epoch plus the tube's, the greatest bound mass a living cluster held
//! over `T_s`, which released its first debris at a mass larger than today's. A dwarf core's
//! orbit is integrated one radial period either way and widened by its extent
//! ([`DwarfCoreModel::extent`](dwarf_core::DwarfCoreModel::extent)), within which its members
//! lie about its centre; the centre's shell is its reach. Members drift
//! from their epoch positions, which the query's padded radius covers, as it does for the grid's
//! cells.
//!
//! **A stream's shell bounds its members from version 26** (`STREAM_SHELL_VERSION`, P10.T9).
//! The shell holds the tracers, but not every point of the tube's cut. At a tip, where its few
//! oldest tracers lie sparse, a knot's kernel spans a long arc of the orbit, and fitted in x, y
//! and z its centre line was a chord inside the arc, with widths some ten times the median:
//! `pal5_like`'s trailing tip's knot 3 lay at 28,648 ly, below every tracer (the least at 30,316)
//! and the shell's inner radius (28,689), and its members up to 11,000 ly inside it. The tips are
//! now fitted in the orbital plane ([`tube`]'s "The reduction", P10.T6.a). That leaves 3 of
//! `pal5_like`'s members outside its shell over eight seeds, and 265 of 4.9 million in a drawn
//! Milky Way (seed `0x0a10_3000_0000_0000`, a probe of P10.T6.a's lane); research-leans-10 found
//! every tracer in the root cube inside its shell, so those are the cut's tail past the tracers.
//! So from 26 a stream's member outside its shell is no member, as one outside the root cube is
//! not (main's ruling of 2026-10-31, reversible behind its hold, kept as the guard of the tips'
//! fix): the range query and
//! [`resolve_with`](crate::galaxy::placement::resolve_with) keep only those inside, and a query
//! whose sphere misses a shell can pass its stream by. Below 26 nothing reads a stream's shell
//! for its members, and a query visits every stream.
//!
//! [`CentreModel::as_global_entry`]: crate::galaxy::features::centre::CentreModel::as_global_entry
//!
//! # Caches
//!
//! A stream's tube table costs about 0.2 s to build (brainstorm, "Runtime and code shape"), so
//! the caller keeps it: [`TubeLookup`] is what a caller's cache implements, and
//! [`resolve_with`](crate::galaxy::placement::resolve_with) reads a stream's or a core's member
//! through it. The sim keeps nothing.

mod debris;
pub mod dwarf_core;
pub mod dwarfs;
pub mod members;
pub mod orbit;
pub mod orphans;
pub mod share;
pub mod source;
pub mod spec;
pub mod spray;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod tube;

use std::error::Error;
use std::fmt;

pub use debris::{Debris, DebrisPlan};
pub use share::discrete_share;
pub use source::GlobalListSource;
pub use spec::{ClusterStars, DwarfCoreSpec, MassLoss, SphericalOrbit, StreamSpec};
pub use tube::{TubeLookup, TubeTable};

use crate::GeneratorVersion;
use crate::coords::{GalacticDisplacement, GalacticPosition, ROOT_HALF_WIDTH_LY};
use crate::galaxy::features::{FeatureId, FeatureKind};
use crate::galaxy::potential::PotentialTables;
use crate::galaxy::query::QuerySphere;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{DwarfCoreMemberId, StreamMemberId};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Seconds, SolarMasses};

use spray::{Leg, SprayForce};

/// How many of its local widths a tube reaches across, on each of its two axes: the brainstorm's
/// "the tube is cut at four widths" (plan 10, Design note 7), a parameter of the generator version
/// that P10.T8's members are placed within and P10.T4's bounding shells are widened by.
pub const CUT_WIDTHS: f64 = 4.0;

/// The spray's spread in its turning points in units of `η r` (module documentation, "Bounding
/// shells"), by which a stream's bounding shell is widened beyond four expected widths: a parameter
/// of the generator version (plan 10, P10.T4), checked against built tubes in P10.T9.
///
/// Measured at v23 on the sprays of 330 streams that reach the root cube (the Milky Way
/// fixture's, every dwarf's and every eighth stream of four drawn galaxies, and the three test
/// streams `pal5_like`, `gd1_like` and `sagittarius_like`): every tracer lies within 3.2 `η r`
/// of its progenitor's orbit's radii (the median stream's within 1), so 4 holds them with a
/// quarter to spare, for those streams' eccentricities.
pub const SHELL_WIDENING: f64 = 4.0;

/// The generator version from which a stream's bounding shell bounds its members (module
/// documentation, "Bounding shells"; the hold of [`StreamShellRules`]).
///
/// Below it every galaxy built by `Galaxy::from_params` takes the held arm,
/// [`StreamShellRules::Unbounded`], which reads no shell for a stream's members, so every generated
/// value keeps its bits; the tests reach the new arm through a galaxy built under
/// `GalaxyRules::every_26_hold()`. The bump to 26 deletes this hold, [`StreamShellRules`] with its
/// `Unbounded` arm, the galaxy's `stream_shell` field and its tripwire
/// (`the_stream_shell_is_held_only_until_the_26_bump`).
pub(crate) const STREAM_SHELL_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// Whether a stream's bounding shell bounds its members (the hold of [`STREAM_SHELL_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum StreamShellRules {
    /// Version 25's and before: a member is every point of the tube's cut in the root cube.
    Unbounded,
    /// Version 26's: a member outside its stream's shell is none.
    Shell,
}

/// Whether a stream member at `position` is one in `galaxy`, whose stream's shell is `shell`
/// (module documentation, "Bounding shells"): every point under [`StreamShellRules::Unbounded`],
/// and those inside the shell under [`StreamShellRules::Shell`].
#[must_use]
pub(crate) fn in_stream_shell(
    galaxy: &Galaxy,
    shell: &BoundingShell,
    position: &GalacticPosition,
) -> bool {
    match galaxy.rules().stream_shell {
        StreamShellRules::Unbounded => true,
        StreamShellRules::Shell => {
            shell.contains(&GalacticPosition::ORIGIN.displacement_to(position))
        }
    }
}

/// A stream's number on the global list, below 2¹² = 4,096 (plan 01's 12-bit field of a
/// [`StreamMemberId`]).
///
/// The number is the stream's place in the order of the module's "Numbering". Numbers at or
/// above the list's stream count are reserved (plan 10, "Generator version"): they are
/// well-formed and name nothing.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::global_list::StreamNumber;
///
/// let gd1 = StreamNumber::new(17)?;
/// assert_eq!(gd1.get(), 17);
/// assert!(StreamNumber::new(4_096).is_err());
/// # Ok::<(), hyperion_sim::galaxy::global_list::BuildEntryNumberError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamNumber(u16);

impl StreamNumber {
    /// The exclusive bound of a stream's number: 4,096.
    pub const LIMIT: u16 = StreamMemberId::NUMBER_LIMIT;

    /// Stream `number`.
    ///
    /// # Errors
    ///
    /// [`BuildEntryNumberError::StreamNumberTooLarge`] for a number of 4,096 or more.
    pub const fn new(number: u16) -> Result<Self, BuildEntryNumberError> {
        if number < Self::LIMIT {
            Ok(Self(number))
        } else {
            Err(BuildEntryNumberError::StreamNumberTooLarge(number))
        }
    }

    /// The number, below [`LIMIT`](Self::LIMIT).
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for StreamNumber {
    type Error = BuildEntryNumberError;

    fn try_from(number: u16) -> Result<Self, Self::Error> {
        Self::new(number)
    }
}

impl From<StreamMemberId> for StreamNumber {
    /// The stream a member belongs to. A member ID's field holds 12 bits, so every one is a valid
    /// number.
    fn from(id: StreamMemberId) -> Self {
        Self(id.number())
    }
}

/// A dwarf core's number on the global list, below 4 (plan 01's 2-bit field of a
/// [`DwarfCoreMemberId`]).
///
/// A galaxy has 0–3 cores (brainstorm, "Streams and accreted structure"), numbered as the
/// module's "Numbering" says; the numbers above the list's core count are reserved.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::global_list::DwarfCoreNumber;
///
/// assert_eq!(DwarfCoreNumber::new(3)?.get(), 3);
/// assert!(DwarfCoreNumber::new(4).is_err());
/// # Ok::<(), hyperion_sim::galaxy::global_list::BuildEntryNumberError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DwarfCoreNumber(u8);

impl DwarfCoreNumber {
    /// The exclusive bound of a dwarf core's number: 4.
    pub const LIMIT: u8 = DwarfCoreMemberId::NUMBER_LIMIT;

    /// Dwarf core `number`.
    ///
    /// # Errors
    ///
    /// [`BuildEntryNumberError::DwarfCoreNumberTooLarge`] for a number of 4 or more.
    pub const fn new(number: u8) -> Result<Self, BuildEntryNumberError> {
        if number < Self::LIMIT {
            Ok(Self(number))
        } else {
            Err(BuildEntryNumberError::DwarfCoreNumberTooLarge(number))
        }
    }

    /// The number, below [`LIMIT`](Self::LIMIT).
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for DwarfCoreNumber {
    type Error = BuildEntryNumberError;

    fn try_from(number: u8) -> Result<Self, Self::Error> {
        Self::new(number)
    }
}

impl From<DwarfCoreMemberId> for DwarfCoreNumber {
    /// The core a member belongs to. A member ID's field holds 2 bits, so every one is a valid
    /// number.
    fn from(id: DwarfCoreMemberId) -> Self {
        Self(id.number())
    }
}

/// A number is beyond what its ID field can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildEntryNumberError {
    /// A stream number of 4,096 or more.
    StreamNumberTooLarge(u16),
    /// A dwarf core number of 4 or more.
    DwarfCoreNumberTooLarge(u8),
}

impl fmt::Display for BuildEntryNumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StreamNumberTooLarge(n) => {
                write!(f, "stream number {n} is not below {}", StreamNumber::LIMIT)
            }
            Self::DwarfCoreNumberTooLarge(n) => write!(
                f,
                "dwarf core number {n} is not below {}",
                DwarfCoreNumber::LIMIT
            ),
        }
    }
}

impl Error for BuildEntryNumberError {}

/// What an entry of the global list is, with its number.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::features::FeatureKind;
/// use hyperion_sim::galaxy::global_list::{GlobalEntryKind, StreamNumber};
///
/// let kind = GlobalEntryKind::Stream(StreamNumber::new(0)?);
/// assert_eq!(kind.feature_kind(), FeatureKind::Stream);
/// # Ok::<(), hyperion_sim::galaxy::global_list::BuildEntryNumberError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GlobalEntryKind {
    /// The galactic centre, plan 09's model, always entry 0.
    Centre,
    /// A stellar stream: a tube of debris stripped from a globular cluster or a dwarf galaxy.
    Stream(StreamNumber),
    /// What is left, still bound, of a recently accreted dwarf galaxy.
    DwarfCore(DwarfCoreNumber),
}

impl GlobalEntryKind {
    /// The feature kind the entry is, as plan 09 names it.
    #[must_use]
    pub const fn feature_kind(self) -> FeatureKind {
        match self {
            Self::Centre => FeatureKind::GalacticCentre,
            Self::Stream(_) => FeatureKind::Stream,
            Self::DwarfCore(_) => FeatureKind::DwarfCore,
        }
    }
}

/// Where a stream's debris came from (plan 10, Design notes 10–12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StreamOrigin {
    /// A globular cluster of the catalogue that is still alive and losing stars.
    LivingGlobular(FeatureId),
    /// A globular cluster that has dissolved, leaving a stream with a gap where it was.
    Orphan,
    /// A recently accreted dwarf galaxy: recent progenitor `j` of plan 02's accretion history,
    /// [`ProgenitorKind::Recent(j)`](crate::galaxy::params::ProgenitorKind::Recent).
    Dwarf(u32),
}

/// A spherical shell about the galactic centre, clipped to the root cube, that holds every member
/// of one entry of the global list (module documentation, "Bounding shells").
///
/// Its radii are distances from the galactic centre, ly: the inner at least zero and the outer at
/// most the root cube's corner, 65,536 √3 ly. An entry whose shell lies wholly beyond the corner
/// has an empty shell, which nothing touches, since no member outside the cube is placed.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::global_list::BoundingShell;
/// use hyperion_sim::galaxy::query::QuerySphere;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::LightYears;
///
/// let shell = BoundingShell::new(LightYears::new(30_000.0), LightYears::new(250_000.0));
/// // The outer radius stops at the root cube's corner.
/// assert!(shell.outer().value() < 113_512.0);
/// let sphere = |x: f64| {
///     let centre = GalacticPosition::from_light_years([x, 0.0, 0.0]).expect("in range");
///     QuerySphere::new(centre, LightYears::new(50.0), UniverseTime::EPOCH, LightYears::ZERO)
/// };
/// // The Sun's neighbourhood lies inside the shell's hole; a query near 40,000 ly meets it.
/// assert!(!shell.touches(&sphere(26_000.0)?));
/// assert!(shell.touches(&sphere(40_000.0)?));
/// # Ok::<(), hyperion_sim::galaxy::query::BuildQuerySphereError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingShell {
    inner_ly: f64,
    outer_ly: f64,
}

impl BoundingShell {
    /// The shell between `inner` and `outer`, clipped to the root cube.
    ///
    /// The inner radius is clipped to at least zero and the outer to at most the cube's corner;
    /// the shell is empty if, so clipped, the inner radius lies beyond the outer.
    #[must_use]
    pub fn new(inner: LightYears, outer: LightYears) -> Self {
        let corner = root_corner_ly();
        // Branches, not `max` and `min`, so that a signed zero cannot reach the bits.
        let inner_ly = if inner.value() > 0.0 {
            inner.value()
        } else {
            0.0
        };
        let outer_ly = if outer.value() < corner {
            outer.value()
        } else {
            corner
        };
        Self { inner_ly, outer_ly }
    }

    /// The inner radius, the least distance from the centre at which a member may lie.
    #[must_use]
    pub fn inner(&self) -> LightYears {
        LightYears::new(self.inner_ly)
    }

    /// The outer radius, the greatest distance from the centre at which a member may lie.
    #[must_use]
    pub fn outer(&self) -> LightYears {
        LightYears::new(self.outer_ly)
    }

    /// Whether the shell holds no point of the root cube.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner_ly > self.outer_ly
    }

    /// Whether `position`, about the galactic centre, lies in the shell and in the root cube.
    #[must_use]
    pub fn contains(&self, position: &GalacticDisplacement) -> bool {
        let p = position.metres().map(|c| c / METRES_PER_LIGHT_YEAR);
        let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        let half = f64::from(ROOT_HALF_WIDTH_LY);
        self.inner_ly <= r && r <= self.outer_ly && p.iter().all(|c| c.abs() <= half)
    }

    /// Whether `sphere`, out to its padded radius, meets the shell and the root cube: a test that
    /// may pass for a sphere that meets the two only apart, never fail for one that meets their
    /// intersection.
    #[must_use]
    pub fn touches(&self, sphere: &QuerySphere) -> bool {
        let [x, y, z] = sphere.centre().to_light_years_f64();
        let reach = sphere.padded_radius().value();
        let d = (x * x + y * y + z * z).sqrt();
        if self.is_empty() || d - reach > self.outer_ly || d + reach < self.inner_ly {
            return false;
        }
        let half = f64::from(ROOT_HALF_WIDTH_LY);
        let beyond = |c: f64| {
            let excess = c.abs() - half;
            if excess > 0.0 { excess } else { 0.0 }
        };
        let (bx, by, bz) = (beyond(x), beyond(y), beyond(z));
        bx * bx + by * by + bz * bz <= reach * reach
    }
}

/// The distance from the galactic centre to a corner of the root cube, 65,536 √3 ly.
#[must_use]
fn root_corner_ly() -> f64 {
    f64::from(ROOT_HALF_WIDTH_LY) * 3.0_f64.sqrt()
}

/// One entry of the global list: what it is, its mass and the shell that holds its members
/// (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalEntry {
    kind: GlobalEntryKind,
    mass: SolarMasses,
    shell: BoundingShell,
}

impl GlobalEntry {
    /// The entry of `kind`, of `mass`, whose members lie in `shell`.
    #[must_use]
    pub(crate) const fn new(
        kind: GlobalEntryKind,
        mass: SolarMasses,
        shell: BoundingShell,
    ) -> Self {
        Self { kind, mass, shell }
    }

    /// What it is, with its number.
    #[must_use]
    pub const fn kind(&self) -> GlobalEntryKind {
        self.kind
    }

    /// Its mass: a stream's tube's stellar mass, a dwarf core's bound stellar mass, and the
    /// centre's mass inside its reach, the black hole's and the cluster's stars there.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The shell its members lie in.
    #[must_use]
    pub const fn shell(&self) -> BoundingShell {
        self.shell
    }

    /// Whether it holds catalogue-class members, which plan 12's alerts walk.
    ///
    /// They are the centre's feature-level list (the black hole's flares and tidal disruptions,
    /// plan 09) and a dwarf core's (plan 10, P10.T7.b). A stream holds none (Design note 9).
    #[must_use]
    pub const fn has_class_members(&self) -> bool {
        match self.kind {
            GlobalEntryKind::Centre | GlobalEntryKind::DwarfCore(_) => true,
            GlobalEntryKind::Stream(_) => false,
        }
    }
}

/// One galaxy's global list (plan 10, P10.T4): the centre, every stream and every dwarf core, each
/// with its bounding shell (module documentation, "The list").
///
/// [`GlobalList::default`] is the empty list, with no centre, for a caller with no galaxy to
/// build one from, such as a [`TubeLookup`] that hands out no tables.
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::global_list::{GlobalEntryKind, GlobalList};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::query::QuerySphere;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// let list = GlobalList::build(&galaxy);
/// assert_eq!(list.entries()[0].kind(), GlobalEntryKind::Centre);
/// // A query in the Sun's neighbourhood tests every shell and builds tables only for the streams
/// // whose shells it meets.
/// let centre = GalacticPosition::from_light_years([26_000.0, 0.0, 20.0]).ok_or("out of range")?;
/// let (radius, time) = (LightYears::new(50.0), UniverseTime::EPOCH);
/// let sphere = QuerySphere::new(centre, radius, time, LightYears::ZERO)?;
/// let touched = list.entries_touching(&sphere).count();
/// assert!(touched < list.entries().len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlobalList {
    entries: Vec<GlobalEntry>,
    debris: Debris,
}

impl GlobalList {
    /// The global list of `galaxy` (module documentation): the centre's entry, then one per
    /// stream and one per dwarf core of its [`Debris`], in number order.
    ///
    /// It generates the debris (about 0.11 s at the Milky Way's parameters on a pod of 16 vCPU,
    /// [`Debris::generate`]), reads the galaxy's
    /// centre ([`Galaxy::centre`], built on its first read) and integrates every stream's and
    /// core's orbits together, side by side, in the spray's table of the potential, which it
    /// builds once.
    ///
    /// # Panics
    ///
    /// - If `galaxy` holds no kinematic tables, as [`Debris::generate`]: build it with
    ///   [`Galaxy::with_full_potential`].
    /// - If its centre has no model ([`Galaxy::centre`]'s [`BuildCentreError`]), which no galaxy
    ///   the builder allows reaches ([`CentreModel::new`]'s documentation).
    /// - As [`Galaxy::centre`], which it reads first: if the mass about the black hole does not
    ///   bracket its own, which no seed of plan 09's sweep reaches.
    /// - If a shell's orbit or widening is not finite, which no specification of [`Debris`]
    ///   gives.
    ///
    /// [`BuildCentreError`]: crate::galaxy::features::centre::BuildCentreError
    /// [`CentreModel::new`]: crate::galaxy::features::centre::CentreModel::new
    #[must_use]
    pub fn build(galaxy: &Galaxy) -> Self {
        // The centre first, as the list always read it; it is kept in the galaxy either way.
        centre_of(galaxy);
        let plan = Self::plan(galaxy);
        let force = SprayForce::new(galaxy.potential())
            .expect("the kinematic tables Debris::generate needs come with the (R, z) grid");
        let radii = spray::radii_over_ly(&force, plan.legs());
        plan.finish(galaxy, &radii)
    }

    /// What [`build`](Self::build) decides before it integrates the shells' orbits: the
    /// [`Debris`] and every shell's orbits, its [`Leg`]s, in list order (P10.T5.b).
    ///
    /// A caller's pool steps the legs ([`spray::radii_over_ly`], in any grouping, each leg's
    /// radii a pure function of the leg) and gathers their radii by index into
    /// [`ListPlan::finish`], for `build`'s list, bit for bit; and it may generate the debris
    /// itself ([`Debris::plan`]) and plan from it ([`ListPlan::of_debris`]).
    ///
    /// # Panics
    ///
    /// As [`Debris::generate`]: if `galaxy` holds no kinematic tables.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::global_list::GlobalList;
    /// use hyperion_sim::galaxy::global_list::spray::{SprayForce, radii_over_ly};
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    ///
    /// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
    ///     .with_full_potential();
    /// let plan = GlobalList::plan(&galaxy);
    /// let force = SprayForce::new(galaxy.potential())?;
    /// // Each leg alone, as a pool's workers could step them, gathered by index.
    /// let radii: Vec<_> = plan.legs().iter().map(|leg| radii_over_ly(&force, &[*leg])[0]).collect();
    /// assert_eq!(plan.finish(&galaxy, &radii), GlobalList::build(&galaxy));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn plan(galaxy: &Galaxy) -> ListPlan {
        ListPlan::of_debris(Debris::generate(galaxy))
    }

    /// The entries, in list order.
    #[must_use]
    pub fn entries(&self) -> &[GlobalEntry] {
        &self.entries
    }

    /// The kinds of the entries, in list order.
    pub fn kinds(&self) -> impl Iterator<Item = GlobalEntryKind> + '_ {
        self.entries.iter().map(GlobalEntry::kind)
    }

    /// The debris the list was built from.
    #[must_use]
    pub fn debris(&self) -> &Debris {
        &self.debris
    }

    /// Stream `number`'s specification, if the list has it.
    #[must_use]
    pub fn stream(&self, number: StreamNumber) -> Option<&StreamSpec> {
        self.debris.streams().get(usize::from(number.get()))
    }

    /// Dwarf core `number`'s specification, if the list has it.
    #[must_use]
    pub fn core(&self, number: DwarfCoreNumber) -> Option<&DwarfCoreSpec> {
        self.debris.cores().get(usize::from(number.get()))
    }

    /// Stream `number`'s entry, if the list has it.
    #[must_use]
    pub fn stream_entry(&self, number: StreamNumber) -> Option<&GlobalEntry> {
        self.entries
            .iter()
            .find(|e| e.kind == GlobalEntryKind::Stream(number))
    }

    /// A list of `streams` and `cores` of `galaxy`, in the potential `tables`, the galaxy's full
    /// tables, without the centre, for tests of the query path on streams and cores no drawn
    /// galaxy has, such as [`testing`]'s.
    ///
    /// Each entry's shell is as [`build`](Self::build) gives it. A galaxy without the full
    /// potential serves, since its tables are given.
    ///
    /// # Panics
    ///
    /// If stream `k` or core `k` is not numbered `k`, or `tables` have no (R, z) grid.
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn of_parts(
        galaxy: &Galaxy,
        tables: &PotentialTables,
        streams: Vec<StreamSpec>,
        cores: Vec<DwarfCoreSpec>,
    ) -> Self {
        for (k, s) in streams.iter().enumerate() {
            assert_eq!(usize::from(s.number().get()), k, "stream {k}'s number");
        }
        for (k, c) in cores.iter().enumerate() {
            assert_eq!(usize::from(c.number().get()), k, "core {k}'s number");
        }
        let plan = ListPlan::of_debris(Debris::of_parts(streams, cores));
        let force = SprayForce::new(tables)
            .expect("the kinematic tables Debris::generate needs come with the (R, z) grid");
        let radii = spray::radii_over_ly(&force, plan.legs());
        let entries = plan.debris_entries(galaxy, tables, &radii);
        Self {
            entries,
            debris: plan.debris,
        }
    }

    /// The entries whose shells `sphere`, out to its padded radius, may meet
    /// ([`BoundingShell::touches`]), in list order.
    pub fn entries_touching<'a>(
        &'a self,
        sphere: &QuerySphere,
    ) -> impl Iterator<Item = &'a GlobalEntry> + use<'a> {
        let sphere = *sphere;
        self.entries
            .iter()
            .filter(move |e| e.shell.touches(&sphere))
    }

    /// The entries that hold catalogue-class members ([`GlobalEntry::has_class_members`]), for
    /// plan 12's alerts, in list order: the centre and the dwarf cores.
    pub fn entries_with_class_members(&self) -> impl Iterator<Item = &GlobalEntry> + '_ {
        self.entries.iter().filter(|e| e.has_class_members())
    }
}

/// The centre's model of `galaxy`, built on its first read and kept in the galaxy.
///
/// # Panics
///
/// As [`GlobalList::build`]: if its centre has no model.
fn centre_of(galaxy: &Galaxy) -> &crate::galaxy::features::centre::CentreModel {
    galaxy.centre().expect(
        "no galaxy the builder allows fails its centre's inversions (CentreModel::new's \
         documentation and tests)",
    )
}

/// What [`GlobalList::build`] decides before it integrates the shells' orbits: the debris and
/// every shell's legs in list order ([`GlobalList::plan`]; P10.T5.b).
#[derive(Debug, Clone, PartialEq)]
pub struct ListPlan {
    debris: Debris,
    /// Each stream's and then each core's orbits, in number order.
    shells: Vec<ShellLegs>,
    /// The shells' legs in turn: each one's backward leg, then its forward one if it has one.
    legs: Vec<Leg>,
}

impl ListPlan {
    /// The plan of the list of `debris`: its streams' and cores' shells' legs.
    #[must_use]
    pub fn of_debris(debris: Debris) -> Self {
        let shells: Vec<ShellLegs> = (debris.streams().iter().map(stream_legs))
            .chain(debris.cores().iter().map(core_legs))
            .collect();
        let legs = shells
            .iter()
            .flat_map(|s| core::iter::once(s.back).chain(s.ahead))
            .collect();
        Self {
            debris,
            shells,
            legs,
        }
    }

    /// The debris the list is built from.
    #[must_use]
    pub fn debris(&self) -> &Debris {
        &self.debris
    }

    /// Every shell's legs, in list order: what [`spray::radii_over_ly`] steps for
    /// [`finish`](Self::finish).
    #[must_use]
    pub fn legs(&self) -> &[Leg] {
        &self.legs
    }

    /// The list of `galaxy`, the galaxy the plan's debris is of, from `radii`, each of
    /// [`legs`](Self::legs)' least and greatest distances from the centre in turn, as
    /// [`spray::radii_over_ly`] gives them: [`GlobalList::build`]'s, bit for bit. It reads the
    /// galaxy's centre, which is built on its first read.
    ///
    /// # Panics
    ///
    /// - If `radii` do not number the legs.
    /// - As [`GlobalList::build`]: if the centre has no model, or a shell's orbit or widening is
    ///   not finite.
    #[must_use]
    pub fn finish(self, galaxy: &Galaxy, radii: &[[f64; 2]]) -> GlobalList {
        let mut entries = vec![centre_of(galaxy).as_global_entry()];
        entries.extend(self.debris_entries(galaxy, galaxy.potential(), radii));
        GlobalList {
            entries,
            debris: self.debris,
        }
    }

    /// The entries of the debris' streams and then its cores, in number order, each shell widened
    /// in the potential `tables` (module documentation, "Bounding shells") from `radii`, its legs'
    /// in turn: what [`GlobalList::build`] lists after the centre.
    ///
    /// # Panics
    ///
    /// If `radii` do not number the legs, or a shell's orbit or widening is not finite, which no
    /// specification of [`Debris`] gives.
    #[must_use]
    fn debris_entries(
        &self,
        galaxy: &Galaxy,
        tables: &PotentialTables,
        radii: &[[f64; 2]],
    ) -> Vec<GlobalEntry> {
        assert_eq!(radii.len(), self.legs.len(), "one radius pair per leg");
        let reached = reaches_ly(&self.shells, radii);
        let debris = &self.debris;
        let (streams, cores) = reached.split_at(debris.streams().len());
        let mut entries = Vec::with_capacity(self.shells.len());
        entries.extend(debris.streams().iter().zip(streams).map(|(s, &reach)| {
            GlobalEntry::new(
                GlobalEntryKind::Stream(s.number()),
                s.mass(),
                widened_stream_shell(tables, s, reach),
            )
        }));
        entries.extend(debris.cores().iter().zip(cores).map(|(c, &reach)| {
            GlobalEntry::new(
                GlobalEntryKind::DwarfCore(c.number()),
                c.mass(),
                widened_core_shell(galaxy, c, reach),
            )
        }));
        entries
    }
}

/// The orbits a bounding shell is widened from, each from its entry's epoch state (module
/// documentation, "Bounding shells"): one backwards, and one forwards for a stream whose
/// stripping time is shorter than a radial period and for every dwarf core.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ShellLegs {
    back: Leg,
    ahead: Option<Leg>,
}

/// The orbits of `spec`'s stream's bounding shell: its stripping time backwards, and forwards too
/// if that is shorter than a radial period.
#[must_use]
fn stream_legs(spec: &StreamSpec) -> ShellLegs {
    let orbit = spec.orbit();
    let leg = |span: Seconds| {
        let fixed = orbit.fixed_step(span).expect(
            "a specification's stripping time is finite, and its step rule's count fits a u32",
        );
        Leg::new(spec.position(), spec.velocity(), fixed)
    };
    let stripping = Seconds::from(spec.stripping_time());
    ShellLegs {
        back: leg(-stripping),
        ahead: (spec.stripping_time().value() < orbit.radial_period().value())
            .then(|| leg(stripping)),
    }
}

/// The orbits of dwarf core `spec`'s bounding shell: a radial period either way.
#[must_use]
fn core_legs(spec: &DwarfCoreSpec) -> ShellLegs {
    let orbit = spec.orbit();
    let period = Seconds::from(orbit.radial_period());
    let leg = |span: Seconds| {
        let fixed = orbit
            .fixed_step(span)
            .expect("a core's radial period is finite, and its step rule's count fits a u32");
        Leg::new(spec.position(), spec.velocity(), fixed)
    };
    ShellLegs {
        back: leg(-period),
        ahead: Some(leg(period)),
    }
}

/// The least and greatest distances from the galactic centre, ly, over each of `shells`' orbits,
/// in order, from `radii`, their legs' in turn: each shell's backward leg, then its forward one
/// if it has one.
#[must_use]
fn reaches_ly(shells: &[ShellLegs], radii: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut radii = radii.iter().copied();
    let mut next = || radii.next().expect("one radius pair per leg");
    shells
        .iter()
        .map(|s| {
            let [back_least, back_greatest] = next();
            let Some([ahead_least, ahead_greatest]) = s.ahead.map(|_| next()) else {
                return [back_least, back_greatest];
            };
            [
                if ahead_least < back_least {
                    ahead_least
                } else {
                    back_least
                },
                if ahead_greatest > back_greatest {
                    ahead_greatest
                } else {
                    back_greatest
                },
            ]
        })
        .collect()
}

/// The bounding shell of `spec`'s stream in a galaxy of `tables` and their spray table `force`
/// (module documentation, "Bounding shells"), for tests of streams on no list.
#[cfg(test)]
#[must_use]
fn stream_shell(tables: &PotentialTables, force: &SprayForce, spec: &StreamSpec) -> BoundingShell {
    let shell = stream_legs(spec);
    let legs: Vec<Leg> = core::iter::once(shell.back).chain(shell.ahead).collect();
    let radii = spray::radii_over_ly(force, &legs);
    widened_stream_shell(tables, spec, reaches_ly(&[shell], &radii)[0])
}

/// The bounding shell of `spec`'s stream in a galaxy of `tables`, from the least and greatest
/// distances of its orbits from the centre, `reach_ly`, widened (module documentation, "Bounding
/// shells").
#[must_use]
fn widened_stream_shell(
    tables: &PotentialTables,
    spec: &StreamSpec,
    reach_ly: [f64; 2],
) -> BoundingShell {
    let [least, greatest] = reach_ly;
    let orbit = spec.orbit();
    let pericentre = orbit.pericentre().value();
    let first = SolarMasses::new(spec.progenitor_mass().value() + spec.mass().value());
    let at_first =
        LightYears::from(tables.tidal_radius(first, &PointLy::new(pericentre, 0.0, 0.0))).value();
    let tidal = if at_first > spec.tidal_radius().value() {
        at_first
    } else {
        spec.tidal_radius().value()
    };
    let angle = tidal / pericentre;
    let widening = CUT_WIDTHS * angle + SHELL_WIDENING * spec::FREQUENCY_SPREAD * angle;
    assert!(
        least.is_finite() && greatest.is_finite() && widening.is_finite(),
        "stream {}'s shell from {least}–{greatest} ly widened by {widening}",
        spec.number().get()
    );
    BoundingShell::new(
        LightYears::new(least * (1.0 - widening)),
        LightYears::new(greatest * (1.0 + widening)),
    )
}

/// The bounding shell of dwarf core `spec` of `galaxy`, from the least and greatest distances of
/// its orbits from the centre, `reach_ly`, widened by its extent (module documentation, "Bounding
/// shells").
#[must_use]
fn widened_core_shell(galaxy: &Galaxy, spec: &DwarfCoreSpec, reach_ly: [f64; 2]) -> BoundingShell {
    let [least, greatest] = reach_ly;
    let extent = dwarf_core::extent_of(galaxy, spec).value();
    assert!(
        least.is_finite() && greatest.is_finite() && extent.is_finite(),
        "dwarf core {}'s shell from {least}–{greatest} ly widened by {extent} ly",
        spec.number().get()
    );
    BoundingShell::new(
        LightYears::new(least - extent),
        LightYears::new(greatest + extent),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::GalacticPosition;
    use crate::id::{Layer, MemberSlot, SystemId, SystemIdKind};
    use crate::time::UniverseTime;

    #[test]
    fn numbers_out_of_range_are_rejected() {
        assert_eq!(StreamNumber::new(4_095).map(StreamNumber::get), Ok(4_095));
        assert_eq!(
            StreamNumber::new(4_096),
            Err(BuildEntryNumberError::StreamNumberTooLarge(4_096))
        );
        assert_eq!(
            StreamNumber::try_from(u16::MAX),
            Err(BuildEntryNumberError::StreamNumberTooLarge(u16::MAX))
        );
        assert_eq!(DwarfCoreNumber::new(3).map(DwarfCoreNumber::get), Ok(3));
        assert_eq!(
            DwarfCoreNumber::new(4),
            Err(BuildEntryNumberError::DwarfCoreNumberTooLarge(4))
        );
        assert_eq!(
            DwarfCoreNumber::try_from(u8::MAX),
            Err(BuildEntryNumberError::DwarfCoreNumberTooLarge(u8::MAX))
        );
    }

    #[test]
    fn every_member_id_names_a_valid_number() {
        for number in [0, 1, 2_047, 4_095] {
            let id = StreamMemberId::new(number, Layer::E, 16_383, [63, 0], 8_191).unwrap();
            assert_eq!(StreamNumber::from(id).get(), number);
            // Through the canonical decode, as an ID from a save arrives.
            let SystemIdKind::Stream(decoded) =
                SystemId::from_raw(SystemId::from(id).raw()).unwrap().kind()
            else {
                panic!("a stream member decodes as one");
            };
            assert_eq!(StreamNumber::from(decoded), StreamNumber::from(id));
        }
        for number in 0..DwarfCoreNumber::LIMIT {
            let id = DwarfCoreMemberId::new(number, MemberSlot::FeatureLevel { index: 5 }).unwrap();
            assert_eq!(DwarfCoreNumber::from(id).get(), number);
        }
    }

    #[test]
    fn entry_kinds_are_the_reserved_feature_kinds() {
        assert_eq!(
            GlobalEntryKind::Centre.feature_kind(),
            FeatureKind::GalacticCentre
        );
        let core = GlobalEntryKind::DwarfCore(DwarfCoreNumber::new(1).unwrap());
        assert_eq!(core.feature_kind(), FeatureKind::DwarfCore);
        assert_eq!(GlobalList::default().kinds().count(), 0);
    }

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        for message in [
            BuildEntryNumberError::StreamNumberTooLarge(5_000).to_string(),
            BuildEntryNumberError::DwarfCoreNumberTooLarge(9).to_string(),
        ] {
            assert!(!message.starts_with(char::is_uppercase), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
        assert_eq!(
            BuildEntryNumberError::StreamNumberTooLarge(5_000).to_string(),
            "stream number 5000 is not below 4096"
        );
    }

    /// A query sphere of `radius` ly about `at` (ly), padded by `pad` ly.
    fn sphere(at: [f64; 3], radius: f64, pad: f64) -> QuerySphere {
        let centre = GalacticPosition::from_light_years(at).expect("inside the coordinates' range");
        QuerySphere::new(
            centre,
            LightYears::new(radius),
            UniverseTime::EPOCH,
            LightYears::new(pad),
        )
        .expect("a positive radius and pad")
    }

    /// A displacement of `at` ly from the galactic centre.
    fn displacement(at: [f64; 3]) -> GalacticDisplacement {
        GalacticDisplacement::new(at.map(|c| c * METRES_PER_LIGHT_YEAR))
    }

    #[test]
    fn a_shell_is_clipped_to_the_root_cube_and_may_hold_none_of_it() {
        let corner = 65_536.0 * 3.0_f64.sqrt();
        let wide = BoundingShell::new(LightYears::new(-10.0), LightYears::new(1e6));
        assert!(wide.inner().value().total_cmp(&0.0).is_eq());
        assert!(wide.outer().value().total_cmp(&corner).is_eq());
        assert!(!wide.is_empty());
        // A negative zero is cleared to zero, so its sign cannot reach a golden file.
        let zero = BoundingShell::new(LightYears::new(-0.0), LightYears::new(10.0));
        assert!(zero.inner().value().total_cmp(&0.0).is_eq());
        // Wholly beyond the corner: no member of the cube lies in it, and no sphere touches it.
        let beyond = BoundingShell::new(LightYears::new(200_000.0), LightYears::new(300_000.0));
        assert!(beyond.is_empty());
        assert!(!beyond.touches(&sphere([0.0, 0.0, 0.0], 1e6, 0.0)));
        assert!(!beyond.contains(&displacement([60_000.0, 60_000.0, 60_000.0])));
    }

    #[test]
    fn a_sphere_touches_a_shell_it_reaches_inside_the_root_cube() {
        let shell = BoundingShell::new(LightYears::new(30_000.0), LightYears::new(40_000.0));
        // From inside the hole, and from beyond the outer radius, by the padded radius.
        assert!(!shell.touches(&sphere([26_000.0, 0.0, 0.0], 3_999.0, 0.0)));
        assert!(shell.touches(&sphere([26_000.0, 0.0, 0.0], 4_001.0, 0.0)));
        assert!(shell.touches(&sphere([26_000.0, 0.0, 0.0], 50.0, 3_951.0)));
        assert!(!shell.touches(&sphere([0.0, 45_000.0, 0.0], 4_999.0, 0.0)));
        assert!(shell.touches(&sphere([0.0, 45_000.0, 0.0], 5_001.0, 0.0)));
        assert!(shell.touches(&sphere([0.0, 0.0, 35_000.0], 1.0, 0.0)));
        // A sphere that meets the spherical shell only outside the cube touches nothing.
        let outer = BoundingShell::new(LightYears::new(60_000.0), LightYears::new(80_000.0));
        assert!(!outer.touches(&sphere([70_000.0, 0.0, 0.0], 3_000.0, 0.0)));
        assert!(outer.touches(&sphere([70_000.0, 0.0, 0.0], 4_500.0, 0.0)));
        assert!(outer.touches(&sphere([45_000.0, 45_000.0, 0.0], 1.0, 0.0)));
    }

    #[test]
    fn a_shell_contains_what_lies_between_its_radii_inside_the_cube() {
        let shell = BoundingShell::new(LightYears::new(30_000.0), LightYears::new(90_000.0));
        assert!(shell.contains(&displacement([35_000.0, 0.0, 0.0])));
        assert!(!shell.contains(&displacement([0.0, 29_999.0, 0.0])));
        assert!(shell.contains(&displacement([60_000.0, -60_000.0, 0.0])));
        assert!(!shell.contains(&displacement([0.0, 0.0, 70_000.0])));
        assert!(!shell.contains(&displacement([60_000.0, 60_000.0, 35_000.0])));
    }

    #[test]
    fn the_centre_and_the_cores_hold_class_members_and_streams_none() {
        let shell = BoundingShell::new(LightYears::ZERO, LightYears::new(128.0));
        let mass = SolarMasses::new(1.0);
        let entry = |kind| GlobalEntry::new(kind, mass, shell);
        assert!(entry(GlobalEntryKind::Centre).has_class_members());
        assert!(
            entry(GlobalEntryKind::DwarfCore(DwarfCoreNumber::new(2).unwrap())).has_class_members()
        );
        assert!(!entry(GlobalEntryKind::Stream(StreamNumber::new(5).unwrap())).has_class_members());
        let empty = GlobalList::default();
        assert_eq!(
            empty.entries_touching(&sphere([0.0; 3], 1e6, 0.0)).count(),
            0
        );
        assert_eq!(empty.entries_with_class_members().count(), 0);
        assert_eq!(empty.stream(StreamNumber::new(0).unwrap()), None);
        assert_eq!(empty.core(DwarfCoreNumber::new(0).unwrap()), None);
    }

    /// The Milky Way fixture's list, built once for the checks below.
    mod milky_way {
        use std::sync::OnceLock;

        use hyperion_testkit::golden;
        use hyperion_testkit::golden::{GoldenWriter, f64_digest};

        use std::num::NonZeroU32;

        use super::*;
        use crate::GENERATOR_VERSION;
        use crate::galaxy::global_list::spray::{Spray, SprayParams};
        use crate::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
        use crate::galaxy::params::GalaxyParams;
        use crate::rng::Seed;

        struct Fixture {
            galaxy: Galaxy,
            /// Built on the galaxy before its centre was read.
            list: GlobalList,
        }

        /// The Milky Way fixture, seed 7, with its full potential, and its list.
        fn fixture() -> &'static Fixture {
            static FIXTURE: OnceLock<Fixture> = OnceLock::new();
            FIXTURE.get_or_init(|| {
                let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())
                    .expect("the fixture's parameters are valid")
                    .with_full_potential();
                let list = GlobalList::build(&galaxy);
                Fixture { galaxy, list }
            })
        }

        /// The list's checks on the Milky Way fixture, each below on one build of it, in turn:
        /// the golden file first, so that a failing check does not keep a bless from writing it.
        /// Here rather than in an integration test, so that the sim's fast suite compares the
        /// golden on `wasm32-wasip1` too; the galaxy's build takes some seconds, and nextest runs
        /// each test in a process of its own.
        #[test]
        fn the_milky_way_s_list_is_pinned_ordered_and_pure() {
            golden_list();
            the_lanes_give_every_shell_orbit_s_bits_alone();
            the_entries_are_the_centre_then_the_streams_then_the_cores();
            the_list_does_not_depend_on_whether_the_centre_was_built();
            the_list_planned_and_made_in_shares_is_the_build();
            the_shells_hold_their_progenitors_and_spheres_touch_them_by_their_radii();
            a_shell_orbit_is_the_spray_s_progenitor_and_its_shell_holds_the_tracers();
        }

        /// The drawn figures of orphan `s` beyond its orbit: its stripping time, tidal radius,
        /// progenitor mass (zero), dissolution, [Fe/H] and age, so that every word its marks and
        /// orbit draw reaches the golden file.
        fn drawn(s: &StreamSpec) -> [f64; 6] {
            let MassLoss::Dissolved { ago } = s.mass_loss() else {
                panic!(
                    "stream {} is an orphan, whose cluster dissolved",
                    s.number().get()
                );
            };
            let stars = s
                .stars()
                .expect("an orphan's stream has its cluster's stars");
            [
                s.stripping_time().value(),
                s.tidal_radius().value(),
                s.progenitor_mass().value(),
                ago.value(),
                stars.fe_h().value(),
                stars.age().value(),
            ]
        }

        /// The bits of the first ten entries (the centre, then the fixture's four living
        /// globulars' streams and five orphans'), and digests of every entry and of every
        /// orphan's position, velocity and turning points: a galaxy with ten living tubes or more
        /// has no orphan among its first ten entries, and the orphans are most of the list.
        fn golden_list() {
            let list = &fixture().list;
            let debris = list.debris();
            let mut writer = GoldenWriter::new();
            writer.header(GENERATOR_VERSION.get());
            let of = |origin: fn(&StreamOrigin) -> bool| {
                debris
                    .streams()
                    .iter()
                    .filter(|s| origin(&s.origin()))
                    .count()
            };
            writer.line(&format!(
                "streams {} (living globulars {}, orphans {}, dwarfs {}), cores {}, beyond the \
                 limit {}",
                debris.streams().len(),
                of(|o| matches!(o, StreamOrigin::LivingGlobular(_))),
                of(|o| matches!(o, StreamOrigin::Orphan)),
                of(|o| matches!(o, StreamOrigin::Dwarf(_))),
                debris.cores().len(),
                debris.cores_beyond_limit()
            ));
            for (i, entry) in list.entries().iter().take(10).enumerate() {
                writer.line(&format!("entry.{i}.kind {:?}", entry.kind()));
                writer.f64(&format!("entry.{i}.mass"), entry.mass().value());
                let shell = entry.shell();
                writer.f64(&format!("entry.{i}.shell.inner"), shell.inner().value());
                writer.f64(&format!("entry.{i}.shell.outer"), shell.outer().value());
            }
            let entries: Vec<f64> = list
                .entries()
                .iter()
                .flat_map(|e| {
                    [
                        e.mass().value(),
                        e.shell().inner().value(),
                        e.shell().outer().value(),
                    ]
                })
                .collect();
            writer.line(&format!("entries {}", list.entries().len()));
            writer.u64_hex("entries.digest", f64_digest(&entries));
            let mut orphans = Vec::new();
            for s in debris
                .streams()
                .iter()
                .filter(|s| s.origin() == StreamOrigin::Orphan)
            {
                orphans.extend(s.position().metres());
                orphans.extend(s.velocity().metres_per_second());
                orphans.push(s.orbit().pericentre().value());
                orphans.push(s.orbit().apocentre().value());
                orphans.extend(drawn(s));
            }
            writer.u64_hex("orphans.digest", f64_digest(&orphans));
            golden!("galaxy/global_list/list", writer.as_str());
        }

        fn the_entries_are_the_centre_then_the_streams_then_the_cores() {
            let Fixture { galaxy, list } = fixture();
            let debris = list.debris();
            let (streams, cores) = (debris.streams(), debris.cores());
            assert_eq!(list.entries().len(), 1 + streams.len() + cores.len());
            let centre = galaxy.centre().expect("the fixture's centre inverts");
            assert_eq!(list.entries()[0], centre.as_global_entry());
            let shell = list.entries()[0].shell();
            assert!(shell.inner().value().total_cmp(&0.0).is_eq());
            assert!(shell.outer().value().total_cmp(&128.0).is_eq());
            let same = |entry: &GlobalEntry, mass: SolarMasses| {
                let (a, b) = (entry.mass().value(), mass.value());
                assert!(
                    a.total_cmp(&b).is_eq(),
                    "{:?}: {a} against {b}",
                    entry.kind()
                );
            };
            for (entry, s) in list.entries()[1..].iter().zip(streams) {
                assert_eq!(entry.kind(), GlobalEntryKind::Stream(s.number()));
                same(entry, s.mass());
                assert_eq!(list.stream(s.number()), Some(s));
            }
            for (entry, c) in list.entries()[1 + streams.len()..].iter().zip(cores) {
                assert_eq!(entry.kind(), GlobalEntryKind::DwarfCore(c.number()));
                same(entry, c.mass());
                assert_eq!(list.core(c.number()), Some(c));
            }
            let past = u16::try_from(streams.len()).expect("under 4,096 streams");
            assert_eq!(list.stream(StreamNumber::new(past).unwrap()), None);
            let with: Vec<_> = list
                .entries_with_class_members()
                .map(GlobalEntry::kind)
                .collect();
            let want: Vec<_> = std::iter::once(GlobalEntryKind::Centre)
                .chain(cores.iter().map(|c| GlobalEntryKind::DwarfCore(c.number())))
                .collect();
            assert_eq!(with, want);
            // Plan 10's 150–1,500 tubes a galaxy (the slow `tests/global_list_build.rs` holds
            // the Milky Way's count to about 240 over eight seeds).
            assert!(
                (150..=1_500).contains(&streams.len()),
                "{} streams",
                streams.len()
            );
        }

        /// Stepped side by side in lanes, every orbit of the fixture's shells reaches the same
        /// radii, bit for bit, as when the spray's own loop steps it alone, and every shell the
        /// least and greatest of its legs'. Every third stream is given a forward leg too, which
        /// none of the fixture's has, so that shells with one and without alternate.
        fn the_lanes_give_every_shell_orbit_s_bits_alone() {
            let Fixture { galaxy, list } = fixture();
            let force = SprayForce::new(galaxy.potential()).expect("the fixture's full tables");
            let debris = list.debris();
            let ahead = |s: &StreamSpec| {
                let span = Seconds::from(s.stripping_time());
                let fixed = s.orbit().fixed_step(span).expect("a finite span");
                Leg::new(s.position(), s.velocity(), fixed)
            };
            let shells: Vec<ShellLegs> = (debris.streams().iter().enumerate())
                .map(|(n, s)| {
                    let legs = stream_legs(s);
                    ShellLegs {
                        ahead: legs.ahead.or_else(|| (n % 3 == 0).then(|| ahead(s))),
                        ..legs
                    }
                })
                .chain(debris.cores().iter().map(core_legs))
                .collect();
            let legs: Vec<Leg> = shells
                .iter()
                .flat_map(|s| core::iter::once(s.back).chain(s.ahead))
                .collect();
            assert_eq!(
                legs.len(),
                352,
                "the 262 streams' backward legs, 88 forward ones and the core's two"
            );
            let alone: Vec<[f64; 2]> = legs
                .iter()
                .map(|leg| spray::radii_over_ly_alone(&force, leg))
                .collect();
            let lanes = spray::radii_over_ly(&force, &legs);
            for (n, (radii, alone)) in lanes.iter().zip(&alone).enumerate() {
                assert_eq!(
                    f64_digest(radii),
                    f64_digest(alone),
                    "leg {n}: {radii:?} against {alone:?}"
                );
            }
            // The base's merge of a stream's two legs.
            let mut alone = alone.into_iter();
            for (n, (shell, reach)) in shells.iter().zip(reaches_ly(&shells, &lanes)).enumerate() {
                let [mut least, mut greatest] = alone.next().expect("a backward leg");
                if shell.ahead.is_some() {
                    let [ahead_least, ahead_greatest] = alone.next().expect("a forward leg");
                    if ahead_least < least {
                        least = ahead_least;
                    }
                    if ahead_greatest > greatest {
                        greatest = ahead_greatest;
                    }
                }
                assert_eq!(
                    f64_digest(&reach),
                    f64_digest(&[least, greatest]),
                    "shell {n}"
                );
            }
        }

        /// The fixture's list was built before the galaxy's centre was read; built again with the
        /// centre held, it is the same.
        fn the_list_does_not_depend_on_whether_the_centre_was_built() {
            let Fixture { galaxy, list } = fixture();
            assert_eq!(&GlobalList::build(galaxy), list);
        }

        /// The list as a caller's pool makes it (P10.T5.b): the debris planned, each orphan drawn
        /// apart by its own copy of the plan, the last first, and gathered by index; the legs
        /// stepped in three shares dealt out by their steps, the longest first, and gathered by
        /// index; and the plan finished. Each step is the build's, bit for bit.
        fn the_list_planned_and_made_in_shares_is_the_build() {
            let Fixture { galaxy, list } = fixture();
            let plan = Debris::plan(galaxy);
            assert!(plan.orphan_count() > 100 && plan.orphan_room() > 1_000);
            let mut drawn: Vec<_> = (0..plan.orphan_count())
                .rev()
                .map(|k| (k, plan.clone().orphan(galaxy, k)))
                .collect();
            drawn.sort_by_key(|&(k, _)| k);
            let debris = plan.finish(drawn.into_iter().map(|(_, orphan)| orphan));
            assert_eq!(&debris, list.debris());
            let planned = ListPlan::of_debris(debris);
            assert_eq!(planned, GlobalList::plan(galaxy));
            let legs = planned.legs();
            let force = SprayForce::new(galaxy.potential()).expect("the full potential");
            let mut by_steps: Vec<usize> = (0..legs.len()).collect();
            by_steps.sort_by_key(|&n| core::cmp::Reverse(legs[n].steps()));
            let mut shares: [(u64, Vec<usize>); 3] = Default::default();
            for n in by_steps {
                let lightest = shares
                    .iter_mut()
                    .min_by_key(|(steps, _)| *steps)
                    .expect("three shares");
                lightest.0 += u64::from(legs[n].steps());
                lightest.1.push(n);
            }
            let mut radii = vec![[f64::NAN; 2]; legs.len()];
            for (_, share) in &shares {
                let mine: Vec<Leg> = share.iter().map(|&n| legs[n]).collect();
                for (&n, r) in share.iter().zip(spray::radii_over_ly(&force, &mine)) {
                    radii[n] = r;
                }
            }
            let whole = spray::radii_over_ly(&force, legs);
            assert_eq!(
                f64_digest(radii.as_flattened()),
                f64_digest(whole.as_flattened())
            );
            assert_eq!(&planned.finish(galaxy, &radii), list);
        }

        fn the_shells_hold_their_progenitors_and_spheres_touch_them_by_their_radii() {
            let list = &fixture().list;
            let half = f64::from(ROOT_HALF_WIDTH_LY);
            let in_cube = |p: &GalacticDisplacement| {
                p.metres()
                    .iter()
                    .all(|c| (c / METRES_PER_LIGHT_YEAR).abs() <= half)
            };
            let mut held = 0;
            for entry in list.entries() {
                let at = match entry.kind() {
                    GlobalEntryKind::Centre => GalacticDisplacement::new([0.0; 3]),
                    GlobalEntryKind::Stream(n) => list.stream(n).unwrap().position(),
                    GlobalEntryKind::DwarfCore(n) => list.core(n).unwrap().position(),
                };
                if in_cube(&at) {
                    assert!(
                        entry.shell().contains(&at),
                        "{:?}: {:?}",
                        entry.kind(),
                        entry.shell()
                    );
                    held += 1;
                }
            }
            assert!(held > 100, "{held} progenitors inside the cube");
            for (at, radius) in [
                ([26_000.0, 0.0, 20.0], 50.0),
                ([0.0, 0.0, 0.0], 10.0),
                ([0.0, 0.0, 60_000.0], 1_000.0),
                ([-40_000.0, 30_000.0, 10_000.0], 5_000.0),
                ([60_000.0, 60_000.0, 60_000.0], 500.0),
                ([0.0, 0.0, 0.0], 200_000.0),
            ] {
                let s = sphere(at, radius, 0.0);
                let d = (at[0] * at[0] + at[1] * at[1] + at[2] * at[2]).sqrt();
                let want: Vec<_> = list
                    .entries()
                    .iter()
                    .filter(|e| {
                        let shell = e.shell();
                        !shell.is_empty()
                            && d - radius <= shell.outer().value()
                            && d + radius >= shell.inner().value()
                    })
                    .map(GlobalEntry::kind)
                    .collect();
                let got: Vec<_> = list.entries_touching(&s).map(GlobalEntry::kind).collect();
                assert_eq!(got, want, "a sphere of {radius} ly at {at:?}");
            }
            let all = sphere([0.0; 3], 200_000.0, 0.0);
            let shown = list
                .entries()
                .iter()
                .filter(|e| !e.shell().is_empty())
                .count();
            assert_eq!(list.entries_touching(&all).count(), shown);
        }

        /// A stream's shell orbit over its stripping time backwards is its spray's progenitor,
        /// whose forward pass retraces it to rounding: the radii match the track's to 10⁻⁹. And
        /// every tracer the cube holds of the three test streams' sprays, a hundred releases an
        /// arm, lies in its shell (P10.T9 checks the members of their tubes).
        fn a_shell_orbit_is_the_spray_s_progenitor_and_its_shell_holds_the_tracers() {
            let galaxy = &fixture().galaxy;
            let tables = galaxy.potential();
            let force = SprayForce::new(tables).expect("the fixture's full tables");
            let params = SprayParams::new(NonZeroU32::new(100).expect("not zero"));
            let half = f64::from(ROOT_HALF_WIDTH_LY);
            for spec in [
                pal5_like(tables),
                gd1_like(tables),
                sagittarius_like(tables),
            ] {
                let spray = Spray::new(galaxy.seed(), &force, tables, &spec, &params)
                    .expect("a test stream's stripping time is finite");
                let back = spec
                    .orbit()
                    .fixed_step(-Seconds::from(spec.stripping_time()))
                    .expect("a finite span");
                let leg = Leg::new(spec.position(), spec.velocity(), back);
                let [least, greatest] = spray::radii_over_ly(&force, &[leg])[0];
                let radii = (0..=back.count()).map(|step| {
                    let (at, _) = spray.progenitor(step).expect("a boundary of the track");
                    at.length().value() / METRES_PER_LIGHT_YEAR
                });
                let (lo, hi) = radii.fold((f64::INFINITY, 0.0_f64), |(lo, hi), r| {
                    (lo.min(r), hi.max(r))
                });
                assert!((least / lo - 1.0).abs() < 1e-9, "{least} against {lo}");
                assert!(
                    (greatest / hi - 1.0).abs() < 1e-9,
                    "{greatest} against {hi}"
                );
                let shell = stream_shell(tables, &force, &spec);
                for t in spray.tracers() {
                    let at = t.position();
                    let inside = at
                        .metres()
                        .iter()
                        .all(|c| (c / METRES_PER_LIGHT_YEAR).abs() <= half);
                    if inside {
                        assert!(shell.contains(&at), "{shell:?} misses {at:?}");
                    }
                }
            }
        }
    }
}
