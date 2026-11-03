//! Plan 09's feature members in the census (rendering plan R06, R06.T16.a; Design note 10).
//!
//! From plan 09's P09.T2.c the field gives up to the features the share φ of each population that
//! sits in them, so the clusters', associations' and nurseries' stars are the features' members,
//! no longer field stars. The census takes them exactly where the galaxy's own field gives them up
//! ([`takes_feature_members`]): the sky and the field flip together, whatever generator version
//! holds φ, and a member is never counted twice, once as a member and once spread through the
//! field.
//!
//! A census's jobs each carry the same sources ([`SkyContext::sources`]): a server's are plan 09's
//! [`FeatureMemberSource`] over its shared feature caches (P09.T40.a). [`census_members`] takes
//! one shell's members of the shell's layer:
//!
//! - **Which members.** Those of the layer's mass band that may lie in the shell at some time from
//!   its outer edge's light time before the observer's time to it ([`SkySource`]). The source pads
//!   each feature by its own members' speed bound over that window (P09.T23.b), never by the cell
//!   rule, and draws only the candidates within the shell so padded.
//! - **Each member.** Its line is its epoch position at its own velocity, the cluster's bulk
//!   motion plus its own (P09.T10), from which it is found at its retarded time. It is the shell's
//!   if its apparent position, brought nearer by the farthest its layer's stars lie from their
//!   barycentres, lies between the shell's edges, so that no star of it is nearer than a shell
//!   done before it. It is always generated, from its own composition and multiplicity
//!   ([`MemberRecord::stars`]): it has no density component, so no envelope bound skips it, and a
//!   helium-rich member, which the brightness envelope does not bound, is never skipped.
//! - **Its stars.** As a grid system's, each measured at its retarded time
//!   ([`census_record`](super::census_record)'s path, a star past the cut with no extinction
//!   taking no sightline), and kept only nearer than the radius its layer is complete to towards
//!   its band texel once the census is final (`Completeness::radius_at`): the band's boundary,
//!   beyond which the band holds the layer's light.
//!
//! From generator version 27 the global list's members join them, exactly where the range query
//! registers the list's source (`GlobalListSource::is_registered_in`, plan 10's P10.T10.b, with
//! plan 09's P09.T37.b), the hold under which the halo's field gives up their share: each
//! stream's and dwarf core's member on its own line ([`Drift::of_global_member`]), and the
//! galactic centre's on its orbit (P09.T28, the path a range row is placed on). Of the centre's
//! members the census generates only the living stars, each single, since no multiplicity is
//! ruled for its cells and its remnants are dark (lean, 2026-11-02); its black hole, its tidal
//! disruptions' victims and its recent entries are not taken.
//!
//! The field gives the catalogue classes their hosts, the core collapses' and the luminous blue
//! variables' under plan 09's carve-out (P09.T35) and plan 11's binary classes' with φ, and the
//! census takes each class's exactly there ([`takes_hosts_of`]; lean, 2026-11-02): every host of
//! those classes the production lookup has, through its [`CachedClassSource`], each on its broken
//! line ([`CatalogueMotion`](crate::galaxy::catalogue_classes::CatalogueMotion)) with its class's
//! own stars and multiplicity ([`CatalogueSystem::stars`]; the Type Ia's members have none), and
//! seen only where it exists at its emitted time, a further member of an entry from its entry's
//! break, as the range query lists it ([`CatalogueSystem::shown_at`]).
//!
//! # The band and the caps
//!
//! They read the budget, each population's whole density, the members' included (decided at
//! R06.T16.a's re-validation as a lean, 2026-11-01): the members' light fainter than the cut and
//! beyond each cap reaches the band, and their counts brighter than the cut each cap's expected
//! count beyond, as the field's do, in expectation, spread as the field is rather than gathered at
//! the features. The listed members lie within the band's boundary and above the cut, where the
//! band holds no light, so no star's light is counted twice. A globular's unresolved light is
//! therefore in the band where the halo's is, not at the globular.

use core::num::NonZeroU32;

use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::{
    ALL_CLASSES, CachedClassSource, CatalogueSystem, ClassId, ClassLookup,
};
use crate::galaxy::displaced::site::CarveOutRules;
use crate::galaxy::features::centre::path::CellPath;
use crate::galaxy::features::centre::{CentreClassKind, CentreMemberLight};
use crate::galaxy::features::members::MemberRecord;
use crate::galaxy::features::members::source::FeatureMemberSource;
use crate::galaxy::global_list::GlobalListSource;
use crate::galaxy::global_list::members::GlobalMemberRecord;
use crate::galaxy::imf::MassBand;
use crate::galaxy::motion::Trajectory;
use crate::math;
use crate::observe::{Drift, Observer, Retardation, retarded};
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::SystemStars;
use crate::time::UniverseTime;
use crate::units::LightYears;
use crate::units::consts::METRES_PER_LIGHT_YEAR;

use super::cell::{Bound, CensusTallies, Counts, SkyStar, layer_offset_bound, system_stars};
use super::query::{CensusPlan, Completeness, Shell, SkyContext, SkyQuery, light_time};

/// A source of systems beyond the grid as the sky's census reads them (R06.T16.a): each a
/// [`SkyMember`], from which its stars and its path are drawn.
///
/// The census retards each member a source gives and keeps it only in its shell, so a source may
/// give more than the shell holds, never fewer. Its order is its own, but the census's merge is
/// total, so no answer depends on it. Plan 09's feature members' source is one
/// ([`FeatureMemberSource`]); the global list's, the streams', the dwarf cores' and the galactic
/// centre's together, is another ([`GlobalListSource`], from generator version 27); and the
/// catalogue classes' hosts are the third ([`CachedClassSource`], from the carve-out).
pub trait SkySource {
    /// Gives `each` every member of `seen`'s part of its shell whose light can reach `observer`.
    ///
    /// The shell is a band and an inner and an outer distance; the members are each of the band
    /// that lies between them at some time from the outer distance's light time before the
    /// observer's time to it, born by the observer's time. The parts of one number hold each
    /// member once between them.
    fn members_seen(
        &self,
        galaxy: &Galaxy,
        observer: &Observer,
        seen: ((MassBand, LightYears, LightYears), MemberPart),
        each: &mut dyn FnMut(SkyMember<'_>),
    );

    /// Whether it gives plan 09's feature members, so that a census whose jobs carry it has them
    /// ([`CensusTallies::feature_members_absent`]): no, unless it says so.
    fn holds_feature_members(&self) -> bool {
        false
    }
}

/// One member a [`SkySource`] gives the census: what its stars and its path are drawn from.
///
/// A source gives a centre member beside its own cell's trajectory, which the census retards it
/// on and draws its stars beside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkyMember<'a> {
    /// A catalogue feature's member, on its line at its cluster's motion and its own (P09.T10).
    Feature(&'a MemberRecord),
    /// A stream's or a dwarf core's member, on its line at its own velocity (plan 10, P10.T10.b).
    Listed(&'a GlobalMemberRecord),
    /// A member of the galactic centre, on its orbit (P09.T28): what its stars are drawn from and
    /// its cell's trajectory, whose start is its record.
    Centre(&'a CentreMemberLight, &'a Trajectory),
    /// A catalogue class's host, on its broken line, with its class's stars (plan 09, P09.T37.a).
    Class(&'a CatalogueSystem),
}

impl SkyMember<'_> {
    /// Its plan 03 record.
    #[must_use]
    fn record(&self) -> &crate::galaxy::placement::SystemRecord {
        match self {
            Self::Feature(member) => member.record(),
            Self::Listed(member) => member.record(),
            Self::Centre(_, trajectory) => trajectory.start().record(),
            Self::Class(system) => system.record(),
        }
    }

    /// Whether the census generates its stars: every member but the galactic centre's not of a
    /// living class, which are dark (the module's documentation; lean, 2026-11-02).
    #[must_use]
    fn shines(&self) -> bool {
        match self {
            Self::Feature(_) | Self::Listed(_) | Self::Class(_) => true,
            Self::Centre(light, _) => light.kind() == Some(CentreClassKind::Living),
        }
    }

    /// Whether it is seen in light emitted at `emitted`: a class host as the range query lists it
    /// ([`CatalogueSystem::shown_at`]: where it exists then, and a further member of its entry from
    /// its entry's break); every other member, whose stars say when they exist.
    #[must_use]
    fn shown_at(&self, emitted: UniverseTime) -> bool {
        match self {
            Self::Feature(_) | Self::Listed(_) | Self::Centre(..) => true,
            Self::Class(system) => system.shown_at(emitted),
        }
    }

    /// Where `observer` sees it: its retardation on its own path.
    #[must_use]
    fn retarded(&self, observer: &Observer) -> Retardation {
        match self {
            Self::Feature(member) => retarded(
                observer,
                &Drift::new(*member.record().epoch_position(), member.velocity()),
            ),
            Self::Listed(member) => retarded(observer, &Drift::of_global_member(member)),
            Self::Centre(_, trajectory) => retarded(observer, &CellPath(trajectory)),
            Self::Class(system) => retarded(observer, system.motion()),
        }
    }

    /// Its stars: a feature's or a list's member's own ([`MemberRecord::stars`],
    /// [`GlobalMemberRecord::stars`]), a centre member's single star at its composition (lean,
    /// 2026-11-02), and a class host's as its class models them ([`CatalogueSystem::stars`]), or
    /// `None` for a host its class gives no stars.
    #[must_use]
    fn stars(&self, galaxy: &Galaxy) -> Option<SystemStars> {
        match self {
            Self::Feature(member) => Some(member.stars(galaxy)),
            Self::Listed(member) => Some(member.stars(galaxy)),
            Self::Centre(light, trajectory) => Some(SystemStars::generate_with(
                galaxy,
                trajectory.start().record(),
                light.composition(),
                MultiplicityContext::ForcedSingle,
            )),
            Self::Class(system) => system.stars(galaxy),
        }
    }
}

/// One of the parts a shell's members are drawn in, so that they can be several jobs.
///
/// It is the `index`-th of `of` (R06.T16.a's cost). Every member is in one part of each number of
/// parts, so the census of a shell is the same however many parts it is taken in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MemberPart {
    index: u32,
    of: NonZeroU32,
}

impl MemberPart {
    /// The one part that holds every member.
    pub const WHOLE: Self = Self {
        index: 0,
        of: NonZeroU32::MIN,
    };

    /// Each of `of` parts, in order.
    pub fn all(of: NonZeroU32) -> impl Iterator<Item = Self> {
        (0..of.get()).map(move |index| Self { index, of })
    }

    /// Its place among its parts, from 0.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// How many parts it is one of.
    #[must_use]
    pub const fn of(self) -> NonZeroU32 {
        self.of
    }
}

impl SkySource for FeatureMemberSource<'_> {
    /// The members the source would draw for a sphere padded by each feature's own speed bound
    /// over the light's window, less the cells wholly inside `inner` so padded.
    ///
    /// # Panics
    ///
    /// If `outer`'s light time before the observer's time is off the clock, which no cap's is: an
    /// observer is within ±1,000 years of the epoch and a cap within the root cube's diagonal.
    fn members_seen(
        &self,
        galaxy: &Galaxy,
        observer: &Observer,
        (shell, part): ((MassBand, LightYears, LightYears), MemberPart),
        each: &mut dyn FnMut(SkyMember<'_>),
    ) {
        self.members_in_shell_over(
            galaxy,
            observer.position(),
            window(observer, shell.2),
            (shell, (part.index, part.of)),
            &mut |member| each(SkyMember::Feature(member)),
        );
    }

    fn holds_feature_members(&self) -> bool {
        true
    }
}

impl SkySource for GlobalListSource<'_> {
    /// The streams' and the dwarf cores' members the source would draw for a sphere padded by
    /// each entry's own speed bound over the light's window, then the galactic centre's whose
    /// paths can bring them within the shell over it (its cells' plunge pads at the window's
    /// farthest time from the epoch).
    ///
    /// # Panics
    ///
    /// As [`FeatureMemberSource`]'s.
    fn members_seen(
        &self,
        galaxy: &Galaxy,
        observer: &Observer,
        ((band, _, outer), part): ((MassBand, LightYears, LightYears), MemberPart),
        each: &mut dyn FnMut(SkyMember<'_>),
    ) {
        let (at, window) = (observer.position(), window(observer, outer));
        let seen = ((band, outer), (part.index, part.of));
        self.members_in_shell_over(galaxy, at, window, seen, &mut |member| {
            each(SkyMember::Listed(member));
        });
        if let Some(centre) = self.centre() {
            centre.members_in_shell_over(galaxy, at, window, seen, &mut |member, trajectory| {
                each(SkyMember::Centre(member, trajectory));
            });
        }
    }
}

impl<L: ClassLookup> SkySource for CachedClassSource<'_, L> {
    /// The hosts of the band's layer, of the classes whose hosts the galaxy's field gives up
    /// ([`takes_hosts_of`]), whose broken lines can bring them between `inner` and `outer` over
    /// the light's window, from the class cells the range query's hits would read for a sphere of
    /// `outer` at the window's farthest time from the epoch.
    ///
    /// # Panics
    ///
    /// As [`FeatureMemberSource`]'s; and if `outer` is not finite and positive.
    fn members_seen(
        &self,
        galaxy: &Galaxy,
        observer: &Observer,
        ((band, inner, outer), part): ((MassBand, LightYears, LightYears), MemberPart),
        each: &mut dyn FnMut(SkyMember<'_>),
    ) {
        self.members_in_shell_over(
            galaxy,
            observer.position(),
            window(observer, outer),
            ((band.layer(), inner, outer), (part.index, part.of)),
            &|class| takes_hosts_of(galaxy, class),
            &mut |system| each(SkyMember::Class(system)),
        );
    }
}

/// The times from `outer`'s light time before `observer`'s time to it.
///
/// # Panics
///
/// If the earlier is off the clock, which no cap's is: an observer is within ±1,000 years of the
/// epoch and a cap within the root cube's diagonal.
#[must_use]
fn window(
    observer: &Observer,
    outer: LightYears,
) -> (crate::time::UniverseTime, crate::time::UniverseTime) {
    let t = observer.time();
    let earliest = t
        .checked_sub(light_time(outer.value()))
        .expect("an observer's time less a cap's light time is on the clock");
    (earliest, t)
}

/// Whether the census takes plan 09's feature members in `galaxy`: exactly where its field gives
/// them up, the share φ of each population that sits in the features (plan 09's P09.T2.c, the
/// galaxy rules' `field_share`).
///
/// One switch for the sky and the field, so they flip together whatever generator version holds
/// φ: a server gives a census its feature sources where this holds ([`SkyContext::sources`]), and
/// none elsewhere, where the field keeps every population's whole budget and a member would be
/// counted twice. No version of its own: φ's hold's tripwire covers both.
#[must_use]
pub fn takes_feature_members(galaxy: &Galaxy) -> bool {
    galaxy.field_share().is_some()
}

/// Whether the census takes `class`'s hosts in `galaxy`: exactly where its field gives them up,
/// whatever generator version holds each, so that a host is never counted twice nor lost from the
/// sky (rendering plan R06, R06.T16.a; lean, 2026-11-02). The core collapses', the luminous blue
/// variables' and the Type Ia's under plan 09's carve-out (P09.T35, the galaxy rules'
/// `carve_out`), whose claims take their hosts from the grid; plan 11's binary classes' where the
/// field gives up their share with φ (P11.T7's `class_share`, which only the field share applies:
/// the galaxy rules' `field_share`); and no other's, the tidal-disruption victims', which are the
/// centre's, among them.
///
/// The range query registers the classes only from its own hold
/// ([`registered_in`](crate::galaxy::catalogue_classes::CatalogueClassSource::registered_in),
/// P09.T37.a), so between the two a host is in the sky and not in a range reply. No version of
/// its own: the carve-out's and φ's tripwires cover it.
#[must_use]
pub fn takes_hosts_of(galaxy: &Galaxy, class: ClassId) -> bool {
    match class {
        ClassId::CORE_COLLAPSE | ClassId::LUMINOUS_BLUE_VARIABLE | ClassId::TYPE_IA => {
            match galaxy.rules().carve_out {
                CarveOutRules::Unclaimed => false,
                CarveOutRules::Claimed => true,
            }
        }
        ClassId::ACCRETING_WHITE_DWARF
        | ClassId::ACCRETING_WHITE_DWARF_SLOW
        | ClassId::XRAY_BINARY
        | ClassId::STELLAR_MERGER
        | ClassId::NEUTRON_STAR_MERGER => takes_feature_members(galaxy),
        // The victims, and any value no class holds.
        _ => false,
    }
}

/// Whether the census takes any catalogue class's hosts in `galaxy` ([`takes_hosts_of`]): where a
/// server gives a census the classes' source ([`CachedClassSource`]), which takes each class's
/// where that class's own switch gives them up.
#[must_use]
pub fn takes_class_hosts(galaxy: &Galaxy) -> bool {
    ALL_CLASSES
        .into_iter()
        .any(|class| takes_hosts_of(galaxy, class))
}

/// The streams whose tube tables the census of `plan` for `query` reads through a
/// [`GlobalListSource`] over `list`, `galaxy`'s global list, in list order: each layer's members
/// out to its cap's farthest radius and its stars' offset, as [`census_members`] takes them, so
/// that a server can build the tables before the census, once each (R06.T16.a, at 27). It is
/// [`streams_seen_in`] of every shell of `plan`.
///
/// # Panics
///
/// If the farthest cap's light time before the observer's time is off the clock, which no cap's
/// is (as [`SkySource::members_seen`]'s).
#[must_use]
pub fn streams_seen(
    galaxy: &Galaxy,
    list: &crate::galaxy::global_list::GlobalList,
    plan: &CensusPlan,
    query: &SkyQuery,
) -> Vec<crate::galaxy::global_list::StreamNumber> {
    streams_seen_in(galaxy, list, (plan, plan.shells()), query)
}

/// The streams whose tube tables the members' census of `shells`, some of `plan`'s, reads through
/// a [`GlobalListSource`] over `list`, `galaxy`'s global list, for `query`, in list order: each
/// shell's members out to its outer edge and its layer's stars' offset, as [`census_members`] takes
/// them. So a server can build the tables of a sky's first steps before their members' jobs and
/// the rest while those run (R06.T16.a's cost, at 27).
///
/// The streams a census reads grow with the distance it reads out to, so those of some shells are
/// those of the farthest of them, and the streams of a set of shells hold those of each of its
/// shells.
///
/// # Panics
///
/// If a shell is not one of `plan`'s; if the farthest shell's light time before the observer's
/// time is off the clock, which no cap's is (as [`SkySource::members_seen`]'s).
///
/// # Examples
///
/// A server builds a sky's tables shell by shell, nearest first, each stream once, so that its
/// first shells' members wait for their own tables alone:
///
/// ```no_run
/// use std::collections::BTreeSet;
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::global_list::GlobalList;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::census::{
///     SHELL_EDGES_LY, SkyQuery, census_plan_with_edges, streams_seen, streams_seen_in,
/// };
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{LightYears, Magnitudes};
///
/// let galaxy = Galaxy::new(Seed::new(7)).with_full_potential();
/// let list = GlobalList::build(&galaxy);
/// let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let query = SkyQuery::builder(Observer::new(at, UniverseTime::EPOCH)?, Magnitudes::new(8.0))
///     .build()?
///     .with_caps_forced(LightYears::new(2_000.0))?;
/// let caps = query.forced_caps().ok_or("forced caps")?.to_vec();
/// let plan = census_plan_with_edges(&query, caps, &SHELL_EDGES_LY);
/// let mut built = BTreeSet::new();
/// for shell in plan.shells() {
///     for number in streams_seen_in(&galaxy, &list, (&plan, [shell]), &query) {
///         if built.insert(number) {
///             // Build this stream's tube table before the shell's members' jobs run.
///         }
///     }
/// }
/// // Every shell's streams together are the census's.
/// let whole: BTreeSet<_> = streams_seen(&galaxy, &list, &plan, &query).into_iter().collect();
/// assert_eq!(built, whole);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn streams_seen_in(
    galaxy: &Galaxy,
    list: &crate::galaxy::global_list::GlobalList,
    (plan, shells): (&CensusPlan, impl IntoIterator<Item = Shell>),
    query: &SkyQuery,
) -> Vec<crate::galaxy::global_list::StreamNumber> {
    let observer = query.observer();
    let [x, y, z] = observer.position().to_light_years_f64();
    let from_centre = math::hypot(math::hypot(x, y), z);
    let outer = shells
        .into_iter()
        .filter(|shell| {
            MassBand::ALL
                .iter()
                .any(|band| band.layer() == shell.layer())
        })
        .map(|shell| {
            let layer = shell.layer();
            let cap = plan
                .caps()
                .iter()
                .find(|cap| cap.layer() == layer)
                .expect("a shell's layer has a cap");
            // The offset `census_members` moves the layer's shells by, at its cap's farthest
            // radius.
            let reach = LightYears::new(from_centre + cap.radius().value());
            plan.shell_span(shell).1.value() + layer_offset_bound(galaxy, layer, reach).value()
        })
        .fold(0.0, f64::max);
    if outer <= 0.0 {
        return Vec::new();
    }
    let outer = LightYears::new(outer);
    GlobalListSource::streams_seen(
        galaxy,
        list,
        observer.position(),
        window(observer, outer),
        outer,
    )
}

/// The stars of `shell`'s members beyond the grid, one of `plan`'s shells, of the members `part`
/// holds, kept for `query`, appended to `out` (which is not cleared); their counts are returned,
/// for the caller to add up with [`CensusTallies::add`] beside the shell's cells' (see the
/// [module](self) documentation).
///
/// Every member of the shell's layer that `ctx`'s sources give and the shell holds is generated
/// and tallied in it, but the centre's that do not shine and the class hosts not shown at their
/// emitted time or given no stars by their class: `generated` per member whose state exists at
/// its emitted time, `accepted` per star kept. For a query with a cone, a member whose
/// stars all lie outside the cone a plan opens its cells by is not generated. A context with no
/// sources gives nothing; its tallies, as its cells', leave the feature members out
/// ([`CensusTallies::feature_members_absent`]) unless a source holds them
/// ([`SkySource::holds_feature_members`]). The stars of every shell of a plan are its members'
/// stars, each once: a census of its shells done, merged ([`merge_shells`](super::merge_shells)),
/// lists each where its cells' stars are listed.
///
/// # Panics
///
/// If `shell` is not one of `plan`'s; as [`MemberRecord::stars`], for a member a source gives of
/// another galaxy.
///
/// # Examples
///
/// Each shell's members beside its cells, as a census's jobs take them, the members only where the
/// galaxy's field gives them up:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
/// use hyperion_sim::galaxy::features::members::source::{FeatureMemberSource, KeepInteriors};
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::census::{
///     CellOffsets, MemberPart, NoSkyCellCache, SkyContext, SkyQuery, SkySource, census_cell,
///     census_members, census_plan, merge_shells, takes_feature_members,
/// };
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::new(Seed::new(11)).with_full_potential();
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let query = SkyQuery::builder(Observer::new(at, UniverseTime::EPOCH)?, Magnitudes::new(6.5))
///     .build()?;
/// let mut noise = NoiseCache::with_capacity(1 << 16);
/// let plan = census_plan(&galaxy, &tables, &envelope, &query, &mut noise);
/// let interiors = KeepInteriors::new(&galaxy);
/// let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
/// let held: [&dyn SkySource; 1] = [&source];
/// let sources: &[&dyn SkySource] = if takes_feature_members(&galaxy) { &held } else { &[] };
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise,
///     cells: &NoSkyCellCache,
///     sources,
///     modifiers: &NoModifiers,
/// };
/// let mut parts = Vec::new();
/// for shell in plan.shells() {
///     let mut stars = Vec::new();
///     let whole = (shell, MemberPart::WHOLE);
///     let members = census_members(&galaxy, &mut ctx, &plan, whole, &query, &mut stars);
///     parts.push((stars, members));
///     for key in plan.shell_slabs(shell).flat_map(|slab| slab.cells()) {
///         let mut stars = Vec::new();
///         let cell = census_cell(&galaxy, &mut ctx, key, &query, &mut stars);
///         parts.push((stars, cell));
///     }
/// }
/// let census = merge_shells(parts, query.n_max(), plan.complete());
/// // The sky's gap is stated exactly where the census lacks the members.
/// assert_eq!(census.tallies().feature_members_absent(), !takes_feature_members(&galaxy));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn census_members(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    plan: &CensusPlan,
    (shell, part): (Shell, MemberPart),
    query: &SkyQuery,
    out: &mut Vec<SkyStar>,
) -> CensusTallies {
    let mut counts = Counts::default();
    let sources = ctx.sources;
    if sources.is_empty() {
        return counts.tallies;
    }
    if sources.iter().any(|source| source.holds_feature_members()) {
        counts.tallies.note_feature_members();
    }
    let layer = shell.layer();
    let Some(band) = MassBand::ALL.into_iter().find(|band| band.layer() == layer) else {
        // The brown dwarfs: no feature holds a member of theirs.
        return counts.tallies;
    };
    let (inner, outer) = plan.shell_span(shell);
    let observer = query.observer();
    let [x, y, z] = observer.position().to_light_years_f64();
    let from_centre = math::hypot(math::hypot(x, y), z);
    // One offset for every shell of the layer, its cap's, so that its shells' spans, moved by it
    // alike, still meet edge to edge and hold each member once.
    let cap = plan
        .caps()
        .iter()
        .find(|cap| cap.layer() == layer)
        .expect("a shell's layer has a cap");
    let offset = layer_offset_bound(
        galaxy,
        layer,
        LightYears::new(from_centre + cap.radius().value()),
    );
    let complete = plan.complete();
    let span = (inner, outer, offset);
    for source in sources {
        let seen = ((band, inner, outer + offset), part);
        source.members_seen(galaxy, observer, seen, &mut |member| {
            census_member(
                galaxy,
                ctx,
                member,
                query,
                (span, &complete),
                &mut counts,
                out,
            );
        });
    }
    counts.tallies
}

/// One member's stars for `query`, in the shell between `inner` and `outer` whose layer's stars
/// lie at most `offset` from their barycentres, the same for every shell of the layer, each kept
/// within the radius `complete` gives its layer towards it.
fn census_member(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    member: SkyMember<'_>,
    query: &SkyQuery,
    ((inner, outer, offset), complete): ((LightYears, LightYears, LightYears), &Completeness),
    counts: &mut Counts,
    out: &mut Vec<SkyStar>,
) {
    let record = member.record();
    if query.exclude() == Some(record.id()) || !member.shines() {
        return;
    }
    let observer = query.observer();
    let r = member.retarded(observer);
    // Its nearest star, held at nought so that the first shell, from nought, holds a member about
    // the observer.
    let nearest = (observer
        .position()
        .distance_to(r.apparent_position())
        .value()
        / METRES_PER_LIGHT_YEAR
        - offset.value())
    .max(0.0);
    if nearest < inner.value() || nearest >= outer.value() {
        return;
    }
    if let Some(region) = query.cone_region() {
        let [x, y, z] = observer.position().to_light_years_f64();
        let [ax, ay, az] = r.apparent_position().to_light_years_f64();
        if !region.meets_ball([ax - x, ay - y, az - z], offset.value()) {
            return;
        }
    }
    if !member.shown_at(r.emitted()) {
        return;
    }
    let layer = record.layer();
    let Some(stars) = member.stars(galaxy) else {
        return;
    };
    system_stars(
        galaxy,
        ctx,
        (record, &stars, &r),
        query,
        (Bound::Applied, &|apparent, d| {
            d < complete.radius_at(layer, apparent).value()
        }),
        counts,
        out,
    );
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};

    use hyperion_testkit::float::bits;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::Seed;
    use crate::coords::{GalacticPosition, UnitVector};
    use crate::galaxy::PointLy;
    use crate::galaxy::ages::FieldShareRules;
    use crate::galaxy::catalogue_classes::{
        CatalogueCellKey, CatalogueClassCellCache, CatalogueClassSource, ClassCellSystems, ClassId,
        ClassVisitor, NoClassCellCache, ProductionClasses, system_of,
    };
    use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord, NoFeatureCache};
    use crate::galaxy::features::centre::members::CentrePlacement;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::features::centre::{
        CentreOrbitCache, CentrePath, KeepCentreOrbits, NoCentreOrbitCache,
    };
    use crate::galaxy::features::cluster::ClusterModel;
    use crate::galaxy::features::members::source::KeepInteriors;
    use crate::galaxy::features::members::{FeatureInterior, local_offset};
    use crate::galaxy::features::{FeatureId, FeatureKind, FeatureProcess};
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::global_list::{GlobalList, StreamNumber, TubeLookup, TubeTable};
    use crate::galaxy::motion::EpochState;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::CellKey;
    use crate::galaxy::query::{LayerSet, QuerySphere, SystemHit, SystemSource};
    use crate::galaxy::rules::GalaxyRules;
    use crate::id::{Layer, SystemId, SystemIdKind};
    use crate::sky::caps::{CAPPED_LAYERS, LayerCap};
    use crate::sky::census::cache::NoSkyCellCache;
    use crate::sky::census::cell::{CellOffsets, census_cell};
    use crate::sky::census::merge::{SkyCensus, merge_shells, sky_order};
    use crate::sky::census::query::{
        CellSlab, Cone, MAX_N_MAX, SHELL_EDGES_LY, census_plan, census_plan_with_edges,
    };
    use crate::sky::luminosity::LuminosityTables;
    use crate::sky::testing::{SUN_LY, milky_way_dark_context, milky_way_envelope};
    use crate::time::UniverseTime;
    use crate::units::{Degrees, Magnitudes, Years};

    /// The sky fixture's universe (`galaxy::features::centre::testing::milky_way_galaxy`'s).
    const SEED: u64 = 0x0926_0000;

    /// The parts each shell's members are taken in by [`census`].
    const PARTS: NonZeroU32 = NonZeroU32::new(16).expect("not zero");

    /// Every 26 hold, with the field giving up φ whichever version holds it (P09.T2.c).
    fn phi_rules() -> GalaxyRules {
        GalaxyRules {
            field_share: FieldShareRules::Phi,
            ..GalaxyRules::every_26_hold_with_its_retargets()
        }
    }

    /// The fixture under `rules`, with its kinematic tables, so that its members move.
    fn fixture(rules: GalaxyRules) -> Galaxy {
        Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
            .expect("the fixture builds")
            .with_full_potential()
    }

    /// The fixture under [`phi_rules`]: its census takes the features' members.
    fn phi_galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| fixture(phi_rules()))
    }

    /// The fixture under every 26 hold but φ's: its field keeps every population's whole budget.
    fn budget_galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            fixture(GalaxyRules {
                field_share: FieldShareRules::None,
                ..phi_rules()
            })
        })
    }

    /// One job of a census: a slab's cells, or a part of a shell's members.
    enum Job {
        Cells(Box<CellSlab>),
        Members(Shell, MemberPart),
    }

    /// The census of `query` (whose caps are forced) in `galaxy` over every shell of its plan, its
    /// cells and, if `members`, its feature members through a [`FeatureMemberSource`] in
    /// [`PARTS`] parts a shell, on threads that take the jobs in turn, each with its own context,
    /// merged to the plan's whole completeness.
    fn census(galaxy: &Galaxy, query: &SkyQuery, members: bool) -> SkyCensus {
        let tables = LuminosityTables::dark(galaxy);
        let envelope = milky_way_envelope();
        let offsets = CellOffsets::build(galaxy);
        // A forced plan reads neither the tables nor the noise.
        let plan = census_plan(
            galaxy,
            &tables,
            envelope,
            query,
            &mut NoiseCache::with_capacity(0),
        );
        let mut jobs: Vec<Job> = Vec::new();
        for shell in plan.shells() {
            jobs.extend(MemberPart::all(PARTS).map(|part| Job::Members(shell, part)));
            jobs.extend(
                plan.shell_slabs(shell)
                    .map(|slab| Job::Cells(Box::new(slab))),
            );
        }
        let interiors = KeepInteriors::new(galaxy);
        let next = AtomicUsize::new(0);
        let parts = Mutex::new(Vec::new());
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(16));
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
                    let sources: [&dyn SkySource; 1] = [&source];
                    let mut ctx = SkyContext {
                        tables: &tables,
                        envelope,
                        offsets: &offsets,
                        noise: NoiseCache::with_capacity(1 << 14),
                        cells: &NoSkyCellCache,
                        sources: if members { &sources } else { &[] },
                        modifiers: &NoModifiers,
                    };
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        let Some(job) = jobs.get(k) else { break };
                        let mut stars = Vec::new();
                        let tallies = match job {
                            Job::Cells(slab) => {
                                let mut tallies: Option<CensusTallies> = None;
                                for key in slab.cells() {
                                    let cell =
                                        census_cell(galaxy, &mut ctx, key, query, &mut stars);
                                    match &mut tallies {
                                        Some(sum) => sum.add(&cell),
                                        None => tallies = Some(cell),
                                    }
                                }
                                let Some(tallies) = tallies else { continue };
                                tallies
                            }
                            Job::Members(shell, part) => census_members(
                                galaxy,
                                &mut ctx,
                                &plan,
                                (*shell, *part),
                                query,
                                &mut stars,
                            ),
                        };
                        parts
                            .lock()
                            .expect("no job panics")
                            .push((k, stars, tallies));
                    }
                });
            }
        });
        let mut parts = parts.into_inner().expect("no job panics");
        parts.sort_by_key(|(k, _, _)| *k);
        merge_shells(
            parts
                .into_iter()
                .map(|(_, stars, tallies)| (stars, tallies)),
            query.n_max(),
            plan.complete(),
        )
    }

    /// The builder of a query from `at` (ly) to `cut` in `galaxy`, listing up to [`MAX_N_MAX`].
    fn query_at(galaxy: &Galaxy, at: [f64; 3], cut: f64) -> crate::sky::census::SkyQueryBuilder {
        query_at_time(galaxy, (at, UniverseTime::EPOCH), cut)
    }

    /// [`query_at`] at `time`.
    fn query_at_time(
        _galaxy: &Galaxy,
        (at, time): ([f64; 3], UniverseTime),
        cut: f64,
    ) -> crate::sky::census::SkyQueryBuilder {
        let position = GalacticPosition::from_light_years(at).expect("in the cube");
        let observer = Observer::new(position, time).expect("on the clock");
        SkyQuery::builder(observer, Magnitudes::new(cut))
            .n_max(NonZeroU32::new(MAX_N_MAX).expect("not zero"))
    }

    /// `query` with its caps forced to `radii`, ly, and every other layer's to nought.
    fn forced_to(query: SkyQuery, radii: &[(Layer, f64)]) -> SkyQuery {
        let radii: Vec<(Layer, LightYears)> = radii
            .iter()
            .map(|&(layer, r)| (layer, LightYears::new(r)))
            .collect();
        query
            .with_caps_forced_per_layer(&radii)
            .expect("valid caps")
    }

    /// A query from `at` (ly) to `cut` in `galaxy`, its caps forced to `radii`.
    fn forced(galaxy: &Galaxy, at: [f64; 3], cut: f64, radii: &[(Layer, f64)]) -> SkyQuery {
        forced_to(
            query_at(galaxy, at, cut).build().expect("a valid query"),
            radii,
        )
    }

    /// The Pleiades-like open cluster of the fixture: 2,135 M☉ at 164 Myr, 804 ly from the Sun,
    /// the nearest of its kind (surveyed 2026-11-01).
    fn pleiades_like(galaxy: &Galaxy) -> FeatureRecord {
        let sun = GalacticPosition::from_light_years(SUN_LY).expect("in the cube");
        FeatureCatalogue::near(galaxy, &sun, LightYears::new(1_000.0), &NoFeatureCache)
            .find(|feature| {
                feature.kind_at(Years::ZERO) == Some(FeatureKind::OpenCluster)
                    && ClusterModel::from_record(galaxy, feature).is_some_and(|model| {
                        (1e8..3e8).contains(&model.age().value())
                            && (1_000.0..5_000.0).contains(&model.mass().value())
                    })
            })
            .expect("the fixture's Pleiades-like cluster")
    }

    /// Plain caps of `radius` for every layer.
    fn caps_of(radius: f64) -> Vec<LayerCap> {
        CAPPED_LAYERS
            .iter()
            .map(|&layer| LayerCap::forced(layer, LightYears::new(radius)))
            .collect()
    }

    /// The members' stars of every shell of `plan`, each shell in `parts` parts, through
    /// `interiors`, in the census's order, with their tallies added.
    fn members_of(
        galaxy: &Galaxy,
        (plan, query): (&CensusPlan, &SkyQuery),
        parts: NonZeroU32,
        interiors: &KeepInteriors<'_>,
    ) -> (Vec<SkyStar>, CensusTallies) {
        let source = FeatureMemberSource::new(&NoFeatureCache, interiors);
        members_through(galaxy, (plan, query), parts, &[&source])
    }

    /// [`members_of`] through `sources`.
    fn members_through(
        galaxy: &Galaxy,
        (plan, query): (&CensusPlan, &SkyQuery),
        parts: NonZeroU32,
        sources: &[&dyn SkySource],
    ) -> (Vec<SkyStar>, CensusTallies) {
        let mut ctx = SkyContext {
            sources,
            ..milky_way_dark_context()
        };
        let (mut stars, mut tallies) = (Vec::new(), None::<CensusTallies>);
        for shell in plan.shells() {
            for part in MemberPart::all(parts) {
                let part = census_members(galaxy, &mut ctx, plan, (shell, part), query, &mut stars);
                match &mut tallies {
                    Some(sum) => sum.add(&part),
                    None => tallies = Some(part),
                }
            }
        }
        stars.sort_by(sky_order);
        (stars, tallies.expect("a plan of shells"))
    }

    /// The brute force of [`members_of`] for plain caps of `radius`: every member of every band
    /// placed within `radius` and a margin of the observer, each found at its retarded time and
    /// every star of it measured, kept within `radius`.
    fn every_member(galaxy: &Galaxy, query: &SkyQuery, radius: f64) -> Vec<SkyStar> {
        // Far above a member's drift over the light's travel and its stars' offsets.
        const MARGIN_LY: f64 = 10.0;
        let observer = query.observer();
        let centre = observer.position();
        let mut ctx = milky_way_dark_context();
        let mut counts = Counts::default();
        let (mut stars, mut members) = (Vec::new(), Vec::new());
        let reach = LightYears::new(radius + MARGIN_LY);
        for feature in FeatureCatalogue::near(galaxy, centre, reach, &NoFeatureCache) {
            let Some(interior) = FeatureInterior::of(galaxy, &feature) else {
                continue;
            };
            let local = local_offset(feature.position(), centre);
            for band in MassBand::ALL {
                for cell in interior.grid().cells_touching(&local, reach) {
                    interior.members_in_cell(galaxy, band, cell, &mut members);
                    for (member, _) in &members {
                        let record = member.record();
                        let drift = Drift::new(*record.epoch_position(), member.velocity());
                        let r = retarded(observer, &drift);
                        system_stars(
                            galaxy,
                            &mut ctx,
                            (record, &member.stars(galaxy), &r),
                            query,
                            (Bound::Ignored, &|_, d| d < radius),
                            &mut counts,
                            &mut stars,
                        );
                    }
                }
            }
        }
        stars.sort_by(sky_order);
        stars
    }

    /// Each star's system and index, and its V, extinction and distance by their bits.
    fn star_bits(stars: &[SkyStar]) -> Vec<(SystemId, u8, [u64; 3])> {
        stars
            .iter()
            .map(|s| {
                (
                    s.system(),
                    s.star().get(),
                    [s.v().value(), s.a_v().value(), s.distance().value()].map(bits),
                )
            })
            .collect()
    }

    /// Whether `star` is a feature member's.
    fn is_member(star: &SkyStar) -> bool {
        matches!(star.system().kind(), SystemIdKind::FeatureMember(_))
    }

    /// The census takes feature members exactly where the field gives them up, and then each
    /// member once (R06.T16.a), each property on one build of the fixture in turn:
    ///
    /// - the switch: no members under the held arm of φ, members under its arm;
    /// - a context's sources say whether a census's tallies leave the members out, its cells'
    ///   and its members' alike, and one without them takes no member;
    /// - inside the Pleiades-like cluster, to 12 ly at V 9, the members' stars of shells to 4, 8
    ///   and 12 ly, in one part or three, through interiors built for them or held warm by an
    ///   earlier census, are those of one shell to 12 ly, bit for bit and tally for tally, and
    ///   those of a brute force over every member placed near, each star measured.
    #[test]
    fn the_census_takes_each_member_once_where_the_field_gives_them_up() {
        let galaxy = phi_galaxy();
        assert!(takes_feature_members(galaxy));
        let held = Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules {
                field_share: FieldShareRules::None,
                ..phi_rules()
            },
        )
        .expect("the fixture builds");
        assert!(!takes_feature_members(&held));

        let cluster = pleiades_like(galaxy);
        let [cx, cy, cz] = cluster.position().to_light_years_f64();
        let query = forced(galaxy, [cx + 3.0, cy, cz], 9.0, &[]);
        let shelled = census_plan_with_edges(&query, caps_of(12.0), &[4, 8]);
        let whole = census_plan_with_edges(&query, caps_of(12.0), &[]);
        assert!(shelled.shells().count() > whole.shells().count());

        // A cell's tallies with the sources and without, and a member census without them.
        let interiors = KeepInteriors::new(galaxy);
        let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
        let sources: [&dyn SkySource; 1] = [&source];
        let key = CellKey::containing(Layer::C, query.observer().position()).expect("a cell");
        let mut out = Vec::new();
        let mut without = milky_way_dark_context();
        assert!(census_cell(galaxy, &mut without, key, &query, &mut out).feature_members_absent());
        let shell = whole.shells().next().expect("a shell");
        let none = census_members(
            galaxy,
            &mut without,
            &whole,
            (shell, MemberPart::WHOLE),
            &query,
            &mut out,
        );
        assert_eq!(none, CensusTallies::default());
        assert!(none.feature_members_absent());
        assert!(!out.iter().any(is_member), "no member without the sources");
        let mut with = SkyContext {
            sources: &sources,
            ..milky_way_dark_context()
        };
        assert!(!census_cell(galaxy, &mut with, key, &query, &mut out).feature_members_absent());

        let one = NonZeroU32::MIN;
        let three = NonZeroU32::new(3).expect("not zero");
        let warm = KeepInteriors::new(galaxy);
        let (stars, tallies) = members_of(galaxy, (&whole, &query), one, &warm);
        assert!(!tallies.feature_members_absent());
        assert!(
            stars.len() > 10,
            "{} members' stars in the cluster",
            stars.len()
        );
        assert!(stars.iter().all(is_member));
        for (plan, parts, kept) in [
            (&shelled, one, false),
            (&shelled, three, true),
            (&whole, three, false),
        ] {
            let cold = KeepInteriors::new(galaxy);
            let interiors = if kept { &warm } else { &cold };
            let (each, each_tallies) = members_of(galaxy, (plan, &query), parts, interiors);
            assert_eq!(star_bits(&each), star_bits(&stars), "{parts} parts");
            for layer in Layer::ALL {
                let (got, want) = (each_tallies.layer(layer), tallies.layer(layer));
                assert_eq!(
                    (got.generated(), got.accepted(), got.without_photometry()),
                    (want.generated(), want.accepted(), want.without_photometry()),
                    "{layer:?}, {parts} parts"
                );
            }
        }
        assert_eq!(
            star_bits(&every_member(galaxy, &query, 12.0)),
            star_bits(&stars)
        );
    }

    /// Each layer's shells, at the fixed edges, meet edge to edge from nought to its cap's farthest
    /// radius ([`CensusPlan::shell_span`]), so that, one offset a layer moving them all alike, a
    /// member's nearest possible star lies in exactly one (R06.T16.a): near the Sun, at caps as
    /// `CensusPlan::completeness`'s example puts them.
    #[test]
    fn a_layers_shells_meet_edge_to_edge_from_nought_to_its_cap() {
        let query = forced(milky_way_galaxy(), SUN_LY, 7.95, &[]);
        let caps = [11.0, 68.0, 8_193.0, 9_925.0, 21_369.0, 1.0]
            .iter()
            .zip(CAPPED_LAYERS)
            .map(|(&r, layer)| LayerCap::forced(layer, LightYears::new(r)))
            .collect();
        let plan = census_plan_with_edges(&query, caps, &SHELL_EDGES_LY);
        for cap in plan.caps() {
            let spans: Vec<(LightYears, LightYears)> = plan
                .shells()
                .filter(|shell| shell.layer() == cap.layer())
                .map(|shell| plan.shell_span(shell))
                .collect();
            let (first, last) = (
                spans.first().expect("a shell"),
                spans.last().expect("a shell"),
            );
            assert_eq!(bits(first.0.value()), bits(0.0), "{:?}", cap.layer());
            assert_eq!(
                bits(last.1.value()),
                bits(cap.radius().value()),
                "{:?}",
                cap.layer()
            );
            for pair in spans.windows(2) {
                assert_eq!(
                    bits(pair[0].1.value()),
                    bits(pair[1].0.value()),
                    "{:?}",
                    cap.layer()
                );
                assert!(pair[0].0.value() < pair[0].1.value(), "{:?}", cap.layer());
            }
        }
        assert!(
            plan.shells()
                .filter(|shell| shell.layer() == Layer::E)
                .count()
                > 5
        );
    }

    /// The fixture under every 27 hold: the range query registers the global list's source
    /// (P10.T10.b), so the census takes the streams', the dwarf cores' and the centre's members.
    fn list_galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| fixture(GalaxyRules::every_27_hold()))
    }

    /// A lookup of `galaxy`'s global list that builds each stream's tube table on first use and
    /// keeps it, and notes each stream it is asked for.
    struct Tubes<'g> {
        galaxy: &'g Galaxy,
        list: GlobalList,
        kept: Mutex<BTreeMap<StreamNumber, Arc<TubeTable>>>,
        asked: Mutex<std::collections::BTreeSet<StreamNumber>>,
    }

    impl TubeLookup for Tubes<'_> {
        fn global_list(&self) -> &GlobalList {
            &self.list
        }

        fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
            self.asked.lock().expect("no job panics").insert(stream);
            if let Some(table) = self.kept.lock().expect("no job panics").get(&stream) {
                return Arc::clone(table);
            }
            // Built outside the lock, so that two streams build at once; an equal table may be
            // built twice, and the first kept.
            let spec = self.list.stream(stream).expect("a stream of the list");
            let table = Arc::new(TubeTable::build(self.galaxy, spec).expect("the full potential"));
            let mut kept = self.kept.lock().expect("no job panics");
            Arc::clone(kept.entry(stream).or_insert(table))
        }
    }

    /// The brute force of the centre's members for plain caps of `radius`: every living member of
    /// every band placed within `radius` and a margin of the observer, each on its own orbit,
    /// found at its retarded time and every star of it measured, kept within `radius`.
    fn every_centre_member(galaxy: &Galaxy, query: &SkyQuery, radius: f64) -> Vec<SkyStar> {
        // Far above a member's path over the light's travel, away from the black hole.
        const MARGIN_LY: f64 = 1.0;
        let placement = CentrePlacement::new(galaxy.centre().expect("the fixture's centre"));
        let observer = query.observer();
        let local = PointLy::from(observer.position());
        let mut ctx = milky_way_dark_context();
        let mut counts = Counts::default();
        let mut stars = Vec::new();
        let reach = LightYears::new(radius + MARGIN_LY);
        for band in MassBand::ALL {
            for cell in placement.grid().cells_touching(&local, reach) {
                for member in placement.members_in_cell(galaxy, band, cell) {
                    let trajectory = Trajectory::new(galaxy, EpochState::of_centre_member(&member));
                    let light = CentreMemberLight::of(&member);
                    let seen = SkyMember::Centre(&light, &trajectory);
                    if !seen.shines() {
                        continue;
                    }
                    let r = seen.retarded(observer);
                    system_stars(
                        galaxy,
                        &mut ctx,
                        (
                            seen.record(),
                            &seen.stars(galaxy).expect("a centre member has stars"),
                            &r,
                        ),
                        query,
                        (Bound::Ignored, &|_, d| d < radius),
                        &mut counts,
                        &mut stars,
                    );
                }
            }
        }
        stars.sort_by(sky_order);
        stars
    }

    /// The fixture's global list under every 27 hold, its tube tables built as they are read.
    fn list_lookup() -> &'static Tubes<'static> {
        static LOOKUP: OnceLock<Tubes<'static>> = OnceLock::new();
        LOOKUP.get_or_init(|| Tubes {
            galaxy: list_galaxy(),
            list: GlobalList::build(list_galaxy()),
            kept: Mutex::default(),
            asked: Mutex::default(),
        })
    }

    /// The census takes the galactic centre's members, through the global list's source, where
    /// the range query registers it, and then each once (R06.T16.a, at 27), each property on one
    /// build of the fixture under every 27 hold in turn:
    ///
    /// - the switch: the list's source unregistered under the held arm, registered under its arm;
    /// - 60 ly from the black hole, to 2 ly at V 9, the centre's living members' stars of shells to
    ///   1 and 2 ly, in one part or three, through orbits built for them or kept by an earlier
    ///   census, are those of one shell, bit for bit and tally for tally, and those of a brute
    ///   force over every member of the cells near, each on its own orbit.
    #[test]
    fn the_census_takes_the_centres_members_once_where_the_list_is_registered() {
        assert!(!GlobalListSource::is_registered_in(milky_way_galaxy()));
        let galaxy = list_galaxy();
        assert!(GlobalListSource::is_registered_in(galaxy));
        let lookup = list_lookup();
        let kept = KeepCentreOrbits::new(galaxy);
        let cold = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("a centre");
        let warm = GlobalListSource::new(galaxy, lookup, &kept).expect("a centre");
        let (one, three) = (NonZeroU32::MIN, NonZeroU32::new(3).expect("not zero"));
        let query = forced(galaxy, [60.0, 0.0, 0.0], 9.0, &[]);
        let shelled = census_plan_with_edges(&query, caps_of(2.0), &[1]);
        let whole = census_plan_with_edges(&query, caps_of(2.0), &[]);
        let (stars, tallies) = members_through(galaxy, (&whole, &query), one, &[&warm]);
        assert!(stars.len() > 10, "{} of the centre's stars", stars.len());
        // The list's source holds no feature member, so its census alone states them absent.
        assert!(tallies.feature_members_absent());
        assert!(
            stars
                .iter()
                .all(|s| matches!(s.system().kind(), SystemIdKind::Centre(_)))
        );
        for (plan, parts, source) in [
            (&shelled, one, &cold),
            (&shelled, three, &warm),
            (&whole, three, &cold),
        ] {
            let (each, each_tallies) = members_through(galaxy, (plan, &query), parts, &[source]);
            assert_eq!(star_bits(&each), star_bits(&stars), "{parts} parts");
            for layer in Layer::ALL {
                let (got, want) = (each_tallies.layer(layer), tallies.layer(layer));
                assert_eq!(
                    (got.generated(), got.accepted()),
                    (want.generated(), want.accepted()),
                    "{layer:?}, {parts} parts"
                );
            }
        }
        assert_eq!(
            star_bits(&every_centre_member(galaxy, &query, 2.0)),
            star_bits(&stars)
        );

        // Each job's census is the same whatever ran before it through the kept orbits.
        let jobs: Vec<(Shell, MemberPart)> = shelled
            .shells()
            .flat_map(|shell| MemberPart::all(three).map(move |part| (shell, part)))
            .collect();
        let job = |&(shell, part): &(Shell, MemberPart)| {
            let sources: [&dyn SkySource; 1] = [&warm];
            let mut ctx = SkyContext {
                sources: &sources,
                ..milky_way_dark_context()
            };
            let mut out = Vec::new();
            let tallies =
                census_members(galaxy, &mut ctx, &shelled, (shell, part), &query, &mut out);
            let layer = shell.layer();
            let counts = (
                tallies.layer(layer).generated(),
                tallies.layer(layer).accepted(),
            );
            (star_bits(&out), counts)
        };
        assert_order_independent(&jobs, job);

        // A century on, the census of the members' moving paths is the brute force's still.
        let later = UniverseTime::from_julian_years(100).expect("on the clock");
        let query = forced_to(
            query_at_time(galaxy, ([60.0, 0.0, 0.0], later), 9.0)
                .build()
                .expect("a valid query"),
            &[],
        );
        let whole = census_plan_with_edges(&query, caps_of(2.0), &[]);
        let (later_stars, _) = members_through(galaxy, (&whole, &query), three, &[&cold]);
        assert_ne!(
            later_stars.len(),
            0,
            "a century on, no member of the centre listed"
        );
        assert_eq!(
            star_bits(&every_centre_member(galaxy, &query, 2.0)),
            star_bits(&later_stars)
        );
    }

    /// A centre member's borrowed path is its [`CentrePath`]'s, bit for bit (R06.T16.a): for the
    /// members of the cells about the black hole, in its Kepler regime, and 60 ly out, where they
    /// drift, each retarded for an observer 60 ly out at the epoch and a century on is retarded
    /// alike on both, and each path's motion is the same.
    #[test]
    fn a_centre_members_cell_path_is_its_centre_path() {
        let galaxy = list_galaxy();
        let placement = CentrePlacement::new(galaxy.centre().expect("the fixture's centre"));
        let observers = [
            UniverseTime::EPOCH,
            UniverseTime::from_julian_years(100).expect("on the clock"),
        ]
        .map(|t| {
            let at = GalacticPosition::from_light_years([60.0, 0.0, 0.0]).expect("in the cube");
            Observer::new(at, t).expect("on the clock")
        });
        let (mut kepler, mut drift) = (0, 0);
        for (near, radius) in [([0.0; 3], 0.05), ([60.0, 0.0, 0.0], 0.5)] {
            let local =
                PointLy::from(&GalacticPosition::from_light_years(near).expect("in the cube"));
            for cell in placement
                .grid()
                .cells_touching(&local, LightYears::new(radius))
            {
                let orbits = NoCentreOrbitCache.orbits(galaxy, &placement, MassBand::B, cell);
                for trajectory in orbits.trajectories().iter().take(20) {
                    let SystemIdKind::Centre(id) = trajectory.start().record().id().kind() else {
                        unreachable!("a centre member's ID")
                    };
                    let path = CentrePath::of(galaxy, &NoCentreOrbitCache, id).expect("resolves");
                    let cell_path = CellPath(trajectory);
                    let motion = crate::observe::Trajectory::motion(&cell_path);
                    assert_eq!(motion, crate::observe::Trajectory::motion(&path));
                    match motion {
                        crate::observe::Motion::Followed => kepler += 1,
                        crate::observe::Motion::Drift => drift += 1,
                    }
                    for observer in &observers {
                        assert_eq!(retarded(observer, &cell_path), retarded(observer, &path));
                    }
                }
            }
        }
        assert!(
            kepler > 0 && drift > 0,
            "{kepler} followed and {drift} drifting"
        );
    }

    /// The census takes a stream's members once (R06.T16.a, at 27): a light-year from a member of
    /// the list's first stream, to 300 ly at V 11, its members' stars of shells to 100, 200 and
    /// 300 ly, in one part or three, are those of one shell, bit for bit, and some are listed; and
    /// the census reads only the streams' tables [`streams_seen`] names, and each shell's members
    /// only those [`streams_seen_in`] names for it.
    #[test]
    #[ignore = "slow: builds a Milky Way galaxy's list and a stream's tube table, some two minutes"]
    fn the_census_takes_a_streams_members_once_where_the_list_is_registered() {
        let galaxy = list_galaxy();
        let lookup = list_lookup();
        let source = GlobalListSource::new(galaxy, lookup, &NoCentreOrbitCache).expect("a centre");
        let number = lookup.list.debris().streams()[0].number();
        let middle = lookup.tube(number).locate(0.5, 0.0, 0.0);
        let sphere = crate::galaxy::query::QuerySphere::new(
            middle,
            LightYears::new(500.0),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a sphere");
        let mut hits = Vec::new();
        crate::galaxy::query::SystemSource::systems_in_sphere(
            &source,
            galaxy,
            &sphere,
            Layer::ALL
                .into_iter()
                .fold(crate::galaxy::query::LayerSet::EMPTY, |set, layer| {
                    set.with(layer)
                }),
            &mut hits,
        );
        let member = hits
            .iter()
            .find(|hit| matches!(hit.id().kind(), SystemIdKind::Stream(_)))
            .expect("a member within 500 ly of the stream's middle");
        let [x, y, z] = member.position().to_light_years_f64();
        let query = forced(galaxy, [x + 1.0, y, z], 11.0, &[]);
        let shelled = census_plan_with_edges(&query, caps_of(300.0), &[100, 200]);
        let whole = census_plan_with_edges(&query, caps_of(300.0), &[]);
        let one = NonZeroU32::MIN;
        lookup.asked.lock().expect("no job panics").clear();
        let (stars, _) = members_through(galaxy, (&whole, &query), one, &[&source]);
        // The census reads only the streams `streams_seen` names, which a server builds first.
        let seen = streams_seen(galaxy, &lookup.list, &whole, &query);
        let asked = lookup.asked.lock().expect("no job panics").clone();
        assert!(
            asked.iter().all(|number| seen.contains(number)),
            "{asked:?} against {seen:?}"
        );
        // Each shell's members read only the streams `streams_seen_in` names for that shell, so
        // that a server can build them a step at a time.
        for shell in shelled.shells() {
            lookup.asked.lock().expect("no job panics").clear();
            let mut ctx = SkyContext {
                sources: &[&source],
                ..milky_way_dark_context()
            };
            let whole = (shell, MemberPart::WHOLE);
            let mut stars = Vec::new();
            let tallies = census_members(galaxy, &mut ctx, &shelled, whole, &query, &mut stars);
            assert_eq!(
                tallies.layer(shell.layer()).accepted(),
                u64::try_from(stars.len()).expect("a count")
            );
            let seen = streams_seen_in(galaxy, &lookup.list, (&shelled, [shell]), &query);
            let asked = lookup.asked.lock().expect("no job panics").clone();
            assert!(
                asked.iter().all(|number| seen.contains(number)),
                "{shell:?}: {asked:?} against {seen:?}"
            );
        }
        let streamed = stars
            .iter()
            .filter(|s| matches!(s.system().kind(), SystemIdKind::Stream(_)))
            .count();
        eprintln!(
            "stream {}: {streamed} of {} stars",
            number.get(),
            stars.len()
        );
        assert!(streamed > 0, "no stream member's star by {:?}", member.id());
        for parts in [one, NonZeroU32::new(3).expect("not zero")] {
            for plan in [&shelled, &whole] {
                let (each, _) = members_through(galaxy, (plan, &query), parts, &[&source]);
                assert_eq!(star_bits(&each), star_bits(&stars), "{parts} parts");
            }
        }
    }

    /// A class-cell cache that keeps every cell's systems it makes, for one galaxy, each made
    /// outside its lock, so that a census's jobs make theirs at once; an equal cell may be made
    /// twice, and the first kept.
    #[derive(Default)]
    struct KeepClassCells(Mutex<BTreeMap<CatalogueCellKey, Arc<ClassCellSystems>>>);

    impl CatalogueClassCellCache for KeepClassCells {
        fn systems(
            &self,
            _galaxy: &Galaxy,
            key: CatalogueCellKey,
            make: &mut dyn FnMut() -> ClassCellSystems,
        ) -> Arc<ClassCellSystems> {
            if let Some(systems) = self.0.lock().expect("no job panics").get(&key) {
                return Arc::clone(systems);
            }
            let fresh = Arc::new(make());
            let mut kept = self.0.lock().expect("no job panics");
            Arc::clone(kept.entry(key).or_insert(fresh))
        }
    }

    /// Every layer, as a range query asks for them.
    fn every_layer() -> LayerSet {
        Layer::ALL.into_iter().fold(LayerSet::EMPTY, LayerSet::with)
    }

    /// One class of the production lookup.
    #[derive(Debug, Clone, Copy)]
    struct Only(ClassId);

    impl ClassLookup for Only {
        fn visit_class<V: ClassVisitor>(
            &self,
            galaxy: &Galaxy,
            class: ClassId,
            visitor: V,
        ) -> Option<V::Output> {
            if class == self.0 {
                ProductionClasses.visit_class(galaxy, class, visitor)
            } else {
                None
            }
        }
    }

    /// The luminous blue variable the range query returns nearest the Sun in `galaxy`: 7,772 ly
    /// from it, under every 26 hold and every 27 (surveyed 2026-11-02).
    fn nearest_lbv(galaxy: &Galaxy) -> SystemHit {
        let sun = GalacticPosition::from_light_years(SUN_LY).expect("in the cube");
        let sphere = QuerySphere::new(
            sun,
            LightYears::new(12_000.0),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a sphere");
        let mut hits = Vec::new();
        CatalogueClassSource::with_lookup(Only(ClassId::LUMINOUS_BLUE_VARIABLE)).systems_in_sphere(
            galaxy,
            &sphere,
            every_layer(),
            &mut hits,
        );
        hits.into_iter()
            .min_by(|a, b| a.distance().value().total_cmp(&b.distance().value()))
            .expect("a luminous blue variable within 12,000 ly of the Sun")
    }

    /// The brute force of the classes' hosts for plain caps of `radius`: every host the range
    /// query returns within `radius`, its layers' farthest star offset and a margin of the
    /// observer at its time, of a class whose hosts the census takes ([`takes_hosts_of`]), each
    /// retarded on its own line and kept where it is shown then and its nearest possible star lies
    /// within `radius`, as the census's shells together hold it, every star of it measured and kept
    /// within `radius`; with their tallies.
    fn every_class_host(
        galaxy: &Galaxy,
        query: &SkyQuery,
        radius: f64,
    ) -> (Vec<SkyStar>, CensusTallies) {
        // Far above a host's drift over the light's travel.
        const MARGIN_LY: f64 = 10.0;
        let observer = query.observer();
        let [x, y, z] = observer.position().to_light_years_f64();
        let reach = LightYears::new(math::hypot(math::hypot(x, y), z) + radius);
        // A host whose star is within `radius` may lie as far beyond it as its layer's offset:
        // some 300 ly in layer E near the Sun.
        let farthest_offset = MassBand::ALL
            .iter()
            .map(|band| layer_offset_bound(galaxy, band.layer(), reach).value())
            .fold(0.0, f64::max);
        let sphere = QuerySphere::new(
            *observer.position(),
            LightYears::new(radius + farthest_offset + MARGIN_LY),
            observer.time(),
            LightYears::ZERO,
        )
        .expect("a sphere");
        let mut hits = Vec::new();
        CatalogueClassSource::new().systems_in_sphere(galaxy, &sphere, every_layer(), &mut hits);
        let mut ctx = milky_way_dark_context();
        let mut counts = Counts::default();
        let mut stars = Vec::new();
        for hit in &hits {
            let SystemIdKind::Catalogue(id) = hit.id().kind() else {
                unreachable!("the classes' source returns class hosts")
            };
            let class = CatalogueCellKey::of(id).expect("a host's cell").class();
            if !takes_hosts_of(galaxy, class) {
                continue;
            }
            let system = system_of(galaxy, id).expect("a host the query returns resolves");
            let host = SkyMember::Class(&system);
            let r = host.retarded(observer);
            let offset = layer_offset_bound(galaxy, system.record().layer(), reach).value();
            let nearest = (observer
                .position()
                .distance_to(r.apparent_position())
                .value()
                / METRES_PER_LIGHT_YEAR
                - offset)
                .max(0.0);
            if nearest >= radius || !host.shown_at(r.emitted()) {
                continue;
            }
            let Some(own) = host.stars(galaxy) else {
                continue;
            };
            system_stars(
                galaxy,
                &mut ctx,
                (system.record(), &own, &r),
                query,
                (Bound::Ignored, &|_, d| d < radius),
                &mut counts,
                &mut stars,
            );
        }
        stars.sort_by(sky_order);
        (stars, counts.tallies)
    }

    /// Each layer's members generated and stars accepted.
    fn generated_and_accepted(tallies: &CensusTallies) -> Vec<(u64, u64)> {
        Layer::ALL
            .iter()
            .map(|&layer| {
                let tally = tallies.layer(layer);
                (tally.generated(), tally.accepted())
            })
            .collect()
    }

    /// The classes' hosts' census of `query` in `galaxy` to caps of `radius`, in one shell and one
    /// part through `cells`, its stars and tallies, checked to be the brute force's
    /// ([`every_class_host`]) and the census through each of `others`, of the shells to its edges
    /// and `radius`, in its parts, through its cells, bit for bit and tally for tally.
    fn checked_class_census(
        galaxy: &Galaxy,
        (query, radius): (&SkyQuery, f64),
        cells: &dyn CatalogueClassCellCache,
        others: &[(&[u32], NonZeroU32, &dyn CatalogueClassCellCache)],
    ) -> (Vec<SkyStar>, CensusTallies) {
        let whole = census_plan_with_edges(query, caps_of(radius), &[]);
        let source = CatalogueClassSource::new().with_cells(cells);
        let (stars, tallies) =
            members_through(galaxy, (&whole, query), NonZeroU32::MIN, &[&source]);
        let (brute, brute_tallies) = every_class_host(galaxy, query, radius);
        assert_eq!(star_bits(&brute), star_bits(&stars));
        assert_eq!(
            generated_and_accepted(&brute_tallies),
            generated_and_accepted(&tallies)
        );
        for &(edges, parts, cells) in others {
            let plan = census_plan_with_edges(query, caps_of(radius), edges);
            let source = CatalogueClassSource::new().with_cells(cells);
            let (each, each_tallies) = members_through(galaxy, (&plan, query), parts, &[&source]);
            assert_eq!(
                star_bits(&each),
                star_bits(&stars),
                "{edges:?}, {parts} parts"
            );
            assert_eq!(
                generated_and_accepted(&each_tallies),
                generated_and_accepted(&tallies),
                "{edges:?}, {parts} parts"
            );
        }
        (stars, tallies)
    }

    /// The census takes the catalogue classes' hosts exactly where the field gives them up, and
    /// then each host with stars the range query returns within its shells once (R06.T16.a), each
    /// property on one build of the fixture at each version from the switches, 26 and 27:
    ///
    /// - the switches: none at 24; the carve-out's classes under its arm and not its held arm, and
    ///   the binary classes under φ's arm and not its held arm, each whatever the other gives;
    /// - from the Sun to 400 ly at V 11, the hosts' stars of one shell are those of a brute force
    ///   over every host the range query returns, each on its own line, every star measured (the
    ///   accreting white dwarfs' and a stellar merger's, some hundred), and those of shells to 200
    ///   and 400 ly, in one part or three, through class cells made for the census, kept by an
    ///   earlier one or not kept, bit for bit and tally for tally; with φ held, none of them;
    /// - 30 ly from the luminous blue variable nearest the Sun, to 100 ly, it is listed, and the
    ///   same holds of shells to 50 and 100 ly in three parts;
    /// - at 27, each job's census is the same whatever ran before it through the kept cells, and a
    ///   century on the census of the hosts' moving lines is the brute force's still.
    #[test]
    fn the_census_takes_each_class_host_once_where_the_field_gives_them_up() {
        assert!(!takes_class_hosts(milky_way_galaxy()));
        let carved_held = Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules {
                carve_out: CarveOutRules::Unclaimed,
                ..phi_rules()
            },
        )
        .expect("the fixture builds");
        for (galaxy, carved, binaries) in [
            (&carved_held, false, true),
            (budget_galaxy(), true, false),
            (phi_galaxy(), true, true),
        ] {
            for class in [ClassId::CORE_COLLAPSE, ClassId::LUMINOUS_BLUE_VARIABLE] {
                assert_eq!(takes_hosts_of(galaxy, class), carved, "{class}");
            }
            for class in [ClassId::ACCRETING_WHITE_DWARF, ClassId::STELLAR_MERGER] {
                assert_eq!(takes_hosts_of(galaxy, class), binaries, "{class}");
            }
        }
        assert!(!takes_hosts_of(
            phi_galaxy(),
            ClassId::TIDAL_DISRUPTION_VICTIM
        ));
        let (one, three) = (NonZeroU32::MIN, NonZeroU32::new(3).expect("not zero"));
        for galaxy in [phi_galaxy(), list_galaxy()] {
            assert!(takes_class_hosts(galaxy));
            let query = forced(galaxy, SUN_LY, 11.0, &[]);
            let warm = KeepClassCells::default();
            let (stars, tallies) = checked_class_census(
                galaxy,
                (&query, 400.0),
                &warm,
                &[
                    (&[200], one, &KeepClassCells::default()),
                    (&[200], three, &warm),
                    (&[], three, &NoClassCellCache),
                ],
            );
            let hosts: u64 = generated_and_accepted(&tallies).iter().map(|c| c.0).sum();
            assert!(hosts > 10, "{hosts} hosts within 400 ly of the Sun");
            assert!(!stars.is_empty(), "no host's star brighter than V 11");
            // The classes' source holds no feature member, so its census alone states them absent.
            assert!(tallies.feature_members_absent());
            assert!(
                stars
                    .iter()
                    .all(|s| matches!(s.system().kind(), SystemIdKind::Catalogue(_)))
            );

            // The brightest of the hosts the grid gives up is in the sky.
            let lbv = nearest_lbv(galaxy);
            let [x, y, z] = lbv.position().to_light_years_f64();
            let query = forced(galaxy, [x, y, z + 30.0], 11.0, &[]);
            let cells = KeepClassCells::default();
            let (stars, _) =
                checked_class_census(galaxy, (&query, 100.0), &cells, &[(&[50], three, &cells)]);
            assert!(
                stars.iter().any(|s| s.system() == lbv.id()),
                "the luminous blue variable {:?} is not listed from 30 ly",
                lbv.id()
            );
        }

        // Where the field keeps φ, it keeps the binary classes' share, and the census takes none
        // of their hosts by the Sun.
        let budget = budget_galaxy();
        let query = forced(budget, SUN_LY, 11.0, &[]);
        let (_, tallies) =
            checked_class_census(budget, (&query, 400.0), &KeepClassCells::default(), &[]);
        let hosts: u64 = generated_and_accepted(&tallies).iter().map(|c| c.0).sum();
        assert_eq!(hosts, 0, "hosts by the Sun with φ held");

        // Each job's census is the same whatever ran before it through the kept cells.
        let galaxy = list_galaxy();
        let query = forced(galaxy, SUN_LY, 11.0, &[]);
        let shelled = census_plan_with_edges(&query, caps_of(400.0), &[200]);
        let warm = KeepClassCells::default();
        let jobs: Vec<(Shell, MemberPart)> = shelled
            .shells()
            .flat_map(|shell| MemberPart::all(three).map(move |part| (shell, part)))
            .collect();
        let job = |&(shell, part): &(Shell, MemberPart)| {
            let source = CatalogueClassSource::new().with_cells(&warm);
            let sources: [&dyn SkySource; 1] = [&source];
            let mut ctx = SkyContext {
                sources: &sources,
                ..milky_way_dark_context()
            };
            let mut out = Vec::new();
            let tallies =
                census_members(galaxy, &mut ctx, &shelled, (shell, part), &query, &mut out);
            (star_bits(&out), generated_and_accepted(&tallies))
        };
        assert_order_independent(&jobs, job);

        // A century on, the census of the hosts' moving lines is the brute force's still.
        let later = UniverseTime::from_julian_years(100).expect("on the clock");
        let query = forced_to(
            query_at_time(galaxy, (SUN_LY, later), 11.0)
                .build()
                .expect("a valid query"),
            &[],
        );
        checked_class_census(galaxy, (&query, 400.0), &warm, &[]);
    }

    /// The listed stars of `census` within `angle` (rad) of `axis` from `observer`: its members',
    /// then the field's.
    fn in_cone(
        census: &SkyCensus,
        observer: &Observer,
        axis: [f64; 3],
        angle: f64,
    ) -> (usize, usize) {
        let from = observer.position().to_light_years_f64();
        let cos = math::cos(angle);
        let (mut members, mut field) = (0, 0);
        for star in census.listed() {
            let to = star.apparent().to_light_years_f64();
            let d: [f64; 3] = core::array::from_fn(|k| to[k] - from[k]);
            let length = math::hypot(math::hypot(d[0], d[1]), d[2]);
            if (d[0] * axis[0] + d[1] * axis[1] + d[2] * axis[2]) / length >= cos {
                if is_member(star) {
                    members += 1;
                } else {
                    field += 1;
                }
            }
        }
        (members, field)
    }

    /// The Pleiades-like cluster of the fixture seen from 400 ly, towards the Sun from it, is a
    /// clump of bright stars (R06.T16.a): within twice its half-mass radius of its centre, its
    /// members brighter than V 6.5 outnumber the field's there at least three times over, and
    /// number at least six, as the real Pleiades' naked-eye stars do from 444 ly. The census is a
    /// cone's about the cluster, the field to 3,000 ly. Measured 2026-11-01: 107 members and 9
    /// field stars within 4.5° of the 2,135 M☉ cluster of 164 Myr, half-mass radius 15.9 ly.
    #[test]
    #[ignore = "slow: a cone's census of the field to 3,000 ly, some thousands of members"]
    fn a_pleiades_like_cluster_is_a_clump_of_bright_stars_from_400_ly() {
        let galaxy = phi_galaxy();
        let cluster = pleiades_like(galaxy);
        let model = ClusterModel::from_record(galaxy, &cluster).expect("a cluster");
        let at = cluster.position().to_light_years_f64();
        let towards: [f64; 3] = core::array::from_fn(|k| SUN_LY[k] - at[k]);
        let norm = math::hypot(math::hypot(towards[0], towards[1]), towards[2]);
        let u = towards.map(|c| c / norm);
        let observer_at: [f64; 3] = core::array::from_fn(|k| at[k] + 400.0 * u[k]);
        let axis = u.map(|c| -c);
        let angle = math::atan(2.0 * model.half_mass_radius().value() / 400.0);
        let cone = Cone::new(
            UnitVector::from_components(axis).expect("a direction"),
            Degrees::new(3.0_f64.max(1.5 * angle.to_degrees())),
        )
        .expect("a cone");
        let query = forced_to(
            query_at(galaxy, observer_at, 6.5)
                .cone(cone)
                .build()
                .expect("a valid query"),
            &[
                (Layer::A, 11.0),
                (Layer::B, 68.0),
                (Layer::C, 1_000.0),
                (Layer::D, 2_000.0),
                (Layer::E, 3_000.0),
            ],
        );
        let census = census(galaxy, &query, true);
        let (members, field) = in_cone(&census, query.observer(), axis, angle);
        eprintln!(
            "{:.0} M_sun at {:.0} Myr, half-mass radius {:.1} ly: within {:.2} deg, {members} \
             members and {field} field stars to V 6.5",
            model.mass().value(),
            model.age().value() / 1e6,
            model.half_mass_radius().value(),
            angle.to_degrees()
        );
        assert!(members >= 6, "{members} members brighter than V 6.5");
        assert!(
            members >= 3 * field.max(1),
            "{members} members against {field} field stars"
        );
    }

    /// The globular core's sky from its centre lists stars to V 6.5 within a factor of two of the
    /// brainstorm's about 4 × 10⁵, its 47 Tuc row (R06.T16.a).
    ///
    /// The count is set by the stars within some tens of light-years of the centre. The core is
    /// the fixture's of 1.33 × 10⁶ M☉ and 3,222 M☉ ly⁻³, about twice 47 Tuc's central density:
    /// 47 Tuc has 8.53 × 10⁵ M☉ and 10^4.72 M☉ pc⁻³, or 1.51 × 10³ M☉ ly⁻³ at 34.70 ly³ a cubic
    /// parsec (Baumgardt and Hilker's catalogue, version 4; `galaxy::features::testing`'s
    /// `NGC_104`). Measured 2026-11-01: it lists 2.98 × 10⁵ stars, its census some 6,500 CPU-s on
    /// a pod of 16 vCPU. The fixture's analogue of 47 Tuc by mass and density, of 7.6 × 10⁵ M☉
    /// and 1,581 M☉ ly⁻³, lists 1.24 × 10⁵ (1.42 × 10⁵ with A to 16 ly and B to 45 ly), a
    /// finding (lean, 2026-11-01).
    #[test]
    #[ignore = "slow: generates some 10⁶ members of a globular's core, some 6,500 CPU-s"]
    fn a_globular_cores_sky_lists_some_four_hundred_thousand_stars_to_v_6_5() {
        /// The pinned core's mass, M☉, and central density, M☉ ly⁻³ (surveyed 2026-11-01).
        const CORE: (f64, f64) = (1.333e6, 3.222e3);
        let galaxy = phi_galaxy();
        let (globular, model) = FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
            .filter_map(|feature| {
                ClusterModel::from_record(galaxy, &feature).map(|model| (feature, model))
            })
            .min_by(|a, b| {
                let off = |m: &ClusterModel| {
                    math::ln(m.mass().value() / CORE.0).abs()
                        + math::ln(m.central_density() / CORE.1).abs()
                };
                off(&a.1).total_cmp(&off(&b.1))
            })
            .expect("a globular");
        // Each layer's cap holds its stars brighter than V 6.5 from the centre: M dwarfs within
        // 10 ly, K dwarfs within 40 ly, the turn-off's and the giants' within the tidal radius.
        let query = forced(
            galaxy,
            globular.position().to_light_years_f64(),
            6.5,
            &[
                (Layer::A, 10.0),
                (Layer::B, 40.0),
                (Layer::C, 500.0),
                (Layer::D, 500.0),
                (Layer::E, 500.0),
            ],
        );
        let census = census(galaxy, &query, true);
        let stars = census.listed().len() + census.overflow().len();
        let members = census
            .listed()
            .iter()
            .chain(census.overflow())
            .filter(|star| is_member(star))
            .count();
        eprintln!(
            "{:.3e} M_sun, central density {:.0} M_sun/ly^3: {stars} stars to V 6.5, {members} \
             members",
            model.mass().value(),
            model.central_density()
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count below 2⁵³ converts exactly"
        )]
        let stars_f = stars as f64;
        assert!(
            (2e5..=8e5).contains(&stars_f),
            "{stars} stars to V 6.5 from the core"
        );
    }

    /// Near the Sun the census's count to V 6.5 is the count of the field and the members together
    /// (R06.T16.a, the band's and the caps' lean), against the same seed's census with the field
    /// the whole budget, φ's held arm, over the same caps: never below it by more than its Poisson
    /// error, so that the members make up what φ takes from the field, and never above it by more
    /// than its compound error, the features' members coming in clumps, each feature one Poisson
    /// event of as many stars as it lists (decided as a lean, 2026-11-01; near the Sun one cluster
    /// of 2.2 × 10⁴ M☉ at 485 ly lists 345 of the 365 members' stars). Measured 2026-11-01: 3,430
    /// stars with the members, 3,065 without, 3,242 of the budget.
    #[test]
    #[ignore = "slow: three censuses near the Sun to 1,000 ly"]
    fn near_the_sun_the_count_to_v_6_5_is_the_fields_and_the_members_together() {
        let radii = [
            (Layer::A, 11.0),
            (Layer::B, 68.0),
            (Layer::C, 250.0),
            (Layer::D, 500.0),
            (Layer::E, 1_000.0),
        ];
        let phi = phi_galaxy();
        let query = forced(phi, SUN_LY, 6.5, &radii);
        let together = census(phi, &query, true);
        let field = census(phi, &query, false).listed().len();
        let budget = census(
            budget_galaxy(),
            &forced(budget_galaxy(), SUN_LY, 6.5, &radii),
            false,
        )
        .listed()
        .len();
        let mut clumps: BTreeMap<FeatureId, u64> = BTreeMap::new();
        for star in together.listed() {
            if let SystemIdKind::FeatureMember(member) = star.system().kind() {
                *clumps.entry(FeatureId::new(member.feature())).or_default() += 1;
            }
        }
        let squares: u64 = clumps.values().map(|n| n * n).sum();
        let listed = together.listed().len();
        eprintln!(
            "to V 6.5: {listed} with the members ({clumps:?} by feature), {field} without, \
             {budget} of the budget"
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts below 2⁵³ convert exactly"
        )]
        let (listed, budget, squares) = (listed as f64, budget as f64, squares as f64);
        assert!(
            listed >= budget - 3.0 * budget.sqrt(),
            "{listed} against the budget's {budget}"
        );
        assert!(
            listed <= budget + 3.0 * (budget + squares).sqrt(),
            "{listed} against the budget's {budget}, the clumps' squares {squares}"
        );
    }
}
