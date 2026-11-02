//! A cluster's members: conditional draws of a member of a class, its velocity and its sphere of
//! influence (plan 09, P09.T10; Design note 20).
//!
//! A member is a plan 03 [`SystemRecord`] with extras ([`MemberRecord`]): its origin is
//! `FeatureMember` and its population the budget it is drawn from; it carries no density component,
//! so no consumer can read a component's laws for it by mistake. Its stellar stage reads the
//! cluster's composition through [`SystemStars::generate_with`], never plan 06's
//! `draw_metallicity`.
//!
//! # The draw ([`draw_member`])
//!
//! - **Initial mass** on `member.mass`: attempt `k` reads words `6k` and `6k + 1`, a mass from the
//!   galaxy's mass function in the class's range and a uniform. A living star is kept with the
//!   depletion factor's odds (P09.T9.a), a tail's with the odds of having been lost, `1 − d(m)`
//!   where the cluster is depleted.
//! - **Remnant classes** redraw, attempt after attempt, until the star's remnant is of the class's
//!   kind and its kick is below the cluster's effective escape speed, a low-mode neutron star
//!   judged instead on its pair's systemic speed, a Maxwellian of σ = 12 km/s
//!   ([`PAIR_SYSTEMIC_SIGMA`], ruling 126.3) drawn on words 2–5 of the attempt's six. Attempt
//!   `k`'s star reads its draws at
//!   [`StarDraws::for_attempt`] `k`, and the record's origin keeps `k` so that the star generated
//!   later is the same. The loop is bounded at [`MAX_ATTEMPTS`] with a debug assertion; its
//!   expected length is 1 ÷ retention. From version 26 the cluster's tabulated remnants decide
//!   the attempts they can, and only the rest build their star (the crate's `members::remnants`).
//! - **Age and metallicity** are the cluster's, a nursery's members younger by up to its age
//!   spread (`member.abundance`'s word 2); a second-population member draws its enrichment
//!   (word 0) and a spread cluster's member its iron offset (words 3–4).
//! - **Multiplicity** (P11.T8.f, from generator version 27): a binary class's members draw their
//!   companions inside the cluster's hard–soft boundary at their radius
//!   ([`hard_soft_separation`]), single classes none, and the members plan 09 marks as recycled
//!   objects (P09.T9.e) on `member.recycled` draw theirs conditionally on the mark
//!   (`draw_member_marked`, [`RecycledMark`]): a millisecond pulsar's or a blue straggler's.
//!   The X-ray binaries' mark is borne by the X-ray binary class on the cluster's list, whose
//!   count in a globular is the same encounter-rate law.
//! - **Velocity**: the cluster's bulk motion plus an isotropic normal of `σ(r) g(m) ÷ g(m_TO)`,
//!   Bianchini et al. 2016's partial equipartition with `m_eq` = 1.5 M☉ (ruling 126.7), on
//!   `member.velocity`, cut at the local escape speed of the cluster's Plummer sphere as plan 08
//!   cuts a field star's (16 attempts, then 0.99 of the cut); a tail member's is at the tidal
//!   radius's σ, uncut.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::catalogue_classes::ClassId;
use crate::galaxy::catalogue_classes::binary::feature::FeatureBinariesRules;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{BodyId, FeatureMemberId, SystemId};
use crate::math;
use crate::rng::{ObjectKey, Stream, Threshold, tags};
use crate::stellar::Composition;
use crate::stellar::Phase;
use crate::stellar::binary::{Component, is_millisecond_pulsar, pulsar_at};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{
    MARKED_ATTEMPTS, MultiplicityContext, RedrawAttempt, StarIndex, SystemHierarchy,
    draw_hierarchy_of_primary,
};
use crate::stellar::remnant::{KickMode, RemnantKind};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::turn_off_mass;
use crate::stellar::system::{RemnantFate, SystemStars, primary_model};
use crate::units::consts::{METRES_PER_AU, METRES_PER_LIGHT_YEAR};
use crate::units::{
    Dex, GravitationalParameter, KilometresPerSecond, LightYears, Metres, SolarMasses, Years,
};

use super::catalogue::FeatureRecord;
use super::cluster::ClusterModel;
use super::ids::PackedFeature;
use super::interior::abundances::{IRON_SPREAD_SHARE, IRON_SPREAD_SIGMA};
use super::interior::counts::{BINARY_MASS_FACTOR, depleted_slope, depletion};
use super::interior::retention::PAIR_SYSTEMIC_SIGMA;
use super::interior::{ClassKind, Generation, MemberAbundances, MemberClass, Multiplicity};
use remnants::remnant_attempt_fits;

pub mod headroom;
pub mod level_list;
pub mod placement;
pub mod recent;
pub(crate) mod remnants;
pub mod source;

pub use headroom::{
    CLASS_ACCEPTANCE_FLOOR, CentreListForecast, ExceedFeatureListError, centre_list_forecast,
    check_feature_list_headroom,
};
pub use level_list::{
    FeatureLevelList, ListClass, ListEntry, ListIndex, MemberZero, listed_death_motion,
};
pub use placement::{FeatureInterior, resolve_member};
pub use source::{FeatureInteriorCache, FeatureMemberSource, KeepInteriors, NoInteriorCache};

/// The most attempts a member's conditional draw makes (P09.T10).
pub const MAX_ATTEMPTS: u32 = 4_096;

/// The attempts of the velocity's escape cut, as plan 08's.
const VELOCITY_ATTEMPTS: u32 = 16;

/// The largest internal speed [`member_velocity`] can draw, in units of its σ, rounded up:
/// 8.58 √2 = 12.134 (P09.T23.b).
///
/// The draw is one Box–Muller pair and the first normal of a second pair, each times σ. A pair's
/// norm is its radius `√(−2 ln u)`, at most `√(106 ln 2)` = 8.5717 at the smallest uniform, 2⁻⁵³
/// ([`Stream::standard_normal_pair`]), and the third normal is at most that too, so the norm is at
/// most `√(212 ln 2)` = 12.1222 σ, which the draw can reach. Rounded up to 8.58 √2 as
/// `galaxy::params::draws` rounds its cap, which covers the rounding of the logarithm and the norm.
/// An uncut tail member's internal speed is bounded by it; a core member's is cut at the escape
/// speed first.
pub(crate) const INTERNAL_SPEED_SIGMAS: f64 = 8.58 * std::f64::consts::SQRT_2;

/// A member: its record and what a member carries beyond one (Design note 20).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemberRecord {
    record: SystemRecord,
    class: MemberClass,
    composition: Composition,
    abundances: MemberAbundances,
    velocity: GalacticVelocity,
    /// For an entry of one of plan 11's binary classes on its feature's list (P11.T8.f), its
    /// class, whose entry holds its stars.
    listed: Option<ClassId>,
    /// For a member whose mark its companions bear out, the redraw attempt its conditional draw
    /// kept (P11.T8.f).
    marked_attempt: Option<RedrawAttempt>,
    /// The widest semi-major axis its orbits may have, its cluster's hard–soft boundary, for a
    /// binary member from generator version 27 (P11.T8.f).
    max_separation: Option<Metres>,
    /// Its recycled-object mark (P09.T9.e's marks, drawn from version 27; P11.T8.f).
    recycled: RecycledMark,
}

/// A feature member's recycled-object mark (plan 09, P09.T9.e; drawn by plan 11, P11.T8.f): the
/// recycled objects are marks inside their members' classes, with expected counts from the
/// encounter rate, and the binary stage draws a marked member's companions conditionally on it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecycledMark {
    /// No mark: the member's stars are its class's draw.
    #[default]
    Unmarked,
    /// A millisecond pulsar among a cluster's neutron stars: its stars are the first of its
    /// companions' marked redraw attempts whose pair has recycled its neutron star, a radio pulsar
    /// at the epoch, and holds no carved class.
    MillisecondPulsar,
    /// A blue straggler among the binaries at a cluster's turn-off: its stars are the first of its
    /// companions' marked redraw attempts, inside [`BLUE_STRAGGLER_SEPARATION`], whose pair has
    /// left a main-sequence star above the turn-off at the epoch, a merger's product or a star its
    /// companion fed, and holds no carved class.
    BlueStraggler,
}

impl MemberRecord {
    /// Its plan 03 record, of origin `FeatureMember`.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// Its class.
    #[must_use]
    pub const fn class(&self) -> &MemberClass {
        &self.class
    }

    /// Its composition: the cluster's [Fe/H], its iron offset and its helium excess.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Its light-element offsets.
    #[must_use]
    pub const fn abundances(&self) -> &MemberAbundances {
        &self.abundances
    }

    /// Its velocity at the epoch, galactic.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The member that an entry of plan 11's binary class `listed` on its feature's list is
    /// (P11.T8.f): its record, its class, its stars' composition and its velocity, the feature's.
    #[must_use]
    pub(crate) const fn of_listed_binary(
        record: SystemRecord,
        class: MemberClass,
        composition: Composition,
        velocity: GalacticVelocity,
        listed: ClassId,
    ) -> Self {
        Self {
            record,
            class,
            composition,
            abundances: MemberAbundances::NONE,
            velocity,
            listed: Some(listed),
            marked_attempt: None,
            max_separation: None,
            recycled: RecycledMark::Unmarked,
        }
    }

    /// A single star of `class` with `record`, `composition` and `velocity`, no light-element
    /// offsets and no mark: an entry of plan 09's recent classes on a feature's list (P09.T36).
    #[must_use]
    pub(crate) const fn of_single(
        record: SystemRecord,
        class: MemberClass,
        composition: Composition,
        velocity: GalacticVelocity,
    ) -> Self {
        Self {
            record,
            class,
            composition,
            abundances: MemberAbundances::NONE,
            velocity,
            listed: None,
            marked_attempt: None,
            max_separation: None,
            recycled: RecycledMark::Unmarked,
        }
    }

    /// The same member moving at `velocity`: a listed core collapse's at the epoch, its remnant's
    /// once it has died (fix-p09-recent-kicks).
    #[must_use]
    pub(crate) const fn moving_at(self, velocity: GalacticVelocity) -> Self {
        Self { velocity, ..self }
    }

    /// Its recycled-object mark.
    #[must_use]
    pub const fn recycled(&self) -> RecycledMark {
        self.recycled
    }

    /// The redraw attempt of its companions its conditional draw kept, for a member whose
    /// recycled-object mark they bear out (P11.T8.f); `None` otherwise.
    #[must_use]
    pub const fn marked_attempt(&self) -> Option<RedrawAttempt> {
        self.marked_attempt
    }

    /// The widest semi-major axis its orbits may have: its cluster's hard–soft boundary for a
    /// binary or marked member from generator version 27, `None` otherwise.
    #[must_use]
    pub const fn max_separation(&self) -> Option<Metres> {
        self.max_separation
    }

    /// The binary class of an entry of its feature's list that holds plan 11's carved binaries
    /// (P11.T8.f), whose stars are its class's marked pair; `None` for every other member.
    #[must_use]
    pub const fn listed_class(&self) -> Option<ClassId> {
        self.listed
    }

    /// The attempt its primary's draws are read at.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        match self.record.origin() {
            SystemOrigin::FeatureMember { attempt, .. }
            | SystemOrigin::CentreMember { attempt }
            | SystemOrigin::GlobalListMember { attempt } => u32::from(attempt),
            SystemOrigin::Grid(_) | SystemOrigin::CatalogueSystem(_) => 0,
        }
    }

    /// The multiplicity context its stars are drawn under: forced multiple for a binary class or
    /// a member marked a millisecond pulsar, its orbits inside its cluster's hard–soft boundary
    /// from generator version 27, and single otherwise.
    #[must_use]
    pub const fn multiplicity_context(&self) -> MultiplicityContext {
        match (self.class.multiplicity, self.recycled) {
            (Multiplicity::Single, RecycledMark::Unmarked) => MultiplicityContext::ForcedSingle,
            (Multiplicity::Binary, _)
            | (
                Multiplicity::Single,
                RecycledMark::MillisecondPulsar | RecycledMark::BlueStraggler,
            ) => MultiplicityContext::ForcedMultiple {
                max_separation: self.max_separation,
            },
        }
    }

    /// Its stars: [`SystemStars::generate_with`] at its composition, under its
    /// [`multiplicity_context`](Self::multiplicity_context); for an entry of one of plan 11's
    /// binary classes on its feature's list, its class's marked pair (P11.T8.f).
    ///
    /// # Panics
    ///
    /// For a listed binary member that its feature's list does not hold in `galaxy`: a member
    /// resolved in another galaxy.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        if self.listed.is_some() {
            return level_list::listed_binary_stars(galaxy, self.record.id())
                .expect("a listed binary member's entry is on its feature's list in its galaxy");
        }
        if let Some(attempt) = self.marked_attempt {
            return SystemStars::generate_at(
                galaxy,
                &self.record,
                &self.composition,
                self.multiplicity_context(),
                attempt,
            );
        }
        SystemStars::generate_with(
            galaxy,
            &self.record,
            &self.composition,
            self.multiplicity_context(),
        )
    }
}

/// The member of `class` at `position` with the ID `id` of `feature`, whose model is `model` and
/// bulk velocity `bulk`, or `None` if its conditional draw exhausts [`MAX_ATTEMPTS`] (module
/// documentation), with no recycled-object mark (a cluster's interior marks its members through
/// the crate's `draw_member_marked`, from generator version 27).
///
/// # Panics
///
/// In debug builds, if the draw exhausts its attempts.
#[must_use]
pub fn draw_member(
    galaxy: &Galaxy,
    feature: &FeatureRecord,
    model: &ClusterModel,
    class: &MemberClass,
    position: &GalacticPosition,
    id: FeatureMemberId,
    bulk: GalacticVelocity,
) -> Option<MemberRecord> {
    draw_member_marked(
        galaxy,
        feature,
        model,
        (class, RecycledShares::NONE),
        position,
        id,
        bulk,
    )
}

/// The shares of a cluster's members that bear each recycled-object mark (P09.T9.e; P11.T8.f):
/// its millisecond pulsars over its expected neutron stars, and its blue stragglers over its
/// expected binaries at the turn-off, the living binaries of the band the turn-off cuts.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct RecycledShares {
    /// The share of a neutron-star class marked millisecond pulsars, in [0, 1].
    pulsars: f64,
    /// The share of a binary class at the turn-off marked blue stragglers, in [0, 1].
    blue_stragglers: f64,
}

impl RecycledShares {
    /// No mark.
    pub(crate) const NONE: Self = Self {
        pulsars: 0.0,
        blue_stragglers: 0.0,
    };

    /// The shares `pulsars` of a cluster's neutron stars and `blue_stragglers` of its binaries at
    /// the turn-off.
    ///
    /// # Panics
    ///
    /// If either is outside [0, 1].
    #[must_use]
    pub(crate) fn new(pulsars: f64, blue_stragglers: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&pulsars) && (0.0..=1.0).contains(&blue_stragglers),
            "recycled shares of {pulsars} and {blue_stragglers}"
        );
        Self {
            pulsars,
            blue_stragglers,
        }
    }

    /// The share of a neutron-star class marked millisecond pulsars.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn pulsars(&self) -> f64 {
        self.pulsars
    }

    /// The share of a binary class at the turn-off marked blue stragglers.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn blue_stragglers(&self) -> f64 {
        self.blue_stragglers
    }

    /// The mark a member of `class` may bear and the share of its class that bears it.
    #[must_use]
    fn of(&self, class: &MemberClass) -> (RecycledMark, f64) {
        match class.kind {
            ClassKind::NeutronStar => (RecycledMark::MillisecondPulsar, self.pulsars),
            ClassKind::Living if is_at_the_turn_off(class) => {
                (RecycledMark::BlueStraggler, self.blue_stragglers)
            }
            ClassKind::Living | ClassKind::WhiteDwarf | ClassKind::BlackHole | ClassKind::Tail => {
                (RecycledMark::Unmarked, 0.0)
            }
        }
    }
}

/// Whether `class` is of the living binaries at the turn-off: binary, living, its primaries' range
/// cut by the turn-off below its band's top.
#[must_use]
pub(crate) fn is_at_the_turn_off(class: &MemberClass) -> bool {
    class.kind == ClassKind::Living
        && class.multiplicity == Multiplicity::Binary
        && class.initial_mass_range.1.value() < class.band.hi()
}

/// [`draw_member`] with a member of `class` marked by `shares` (P11.T8.f), from generator version
/// 27: one mark on `member.recycled` against them ([`RecycledShares`]), and for a marked member a
/// conditional draw of its companions, its primary drawn as an unmarked member's: the first of its
/// marked redraw attempts ([`RedrawAttempt`] 8–255, on words no other draw reads) whose stars bear
/// its mark out ([`RecycledMark`]); a member none of whose attempts does is unmarked. A binary or
/// marked member takes its cluster's hard–soft boundary at its radius as its widest separation
/// ([`hard_soft_separation`]), a blue straggler at most [`BLUE_STRAGGLER_SEPARATION`].
///
/// # Panics
///
/// In debug builds, if the draw exhausts its attempts.
#[must_use]
pub(crate) fn draw_member_marked(
    galaxy: &Galaxy,
    feature: &FeatureRecord,
    model: &ClusterModel,
    (class, shares): (&MemberClass, RecycledShares),
    position: &GalacticPosition,
    id: FeatureMemberId,
    bulk: GalacticVelocity,
) -> Option<MemberRecord> {
    let system = SystemId::from(id);
    let key = ObjectKey::from(system);
    let seed = galaxy.seed();
    // Age, metallicity and the second population's enrichment.
    let (abundances, composition, age) = member_abundances(galaxy, model, class.generation, key);
    // The initial mass, conditional on the class.
    let (lo, hi) = (
        class.initial_mass_range.0.value(),
        class.initial_mass_range.1.value(),
    );
    let alpha = depleted_slope(model);
    let v_eff = model.escape_speed_effective().value();
    let imf = galaxy.mass_function();
    // The recycled-object mark and the hard–soft boundary, from version 27.
    let (marked, max_separation) =
        mark_and_boundary(galaxy, (feature, model), (class, shares), position, key);
    let mut mass_stream = Stream::open(seed, tags::MEMBER_MASS, key);
    let mut chosen = None;
    for attempt in 0..MAX_ATTEMPTS {
        mass_stream.seek(ATTEMPT_WORDS * u64::from(attempt));
        let mass = imf.quantile_in(lo, hi, mass_stream.uniform());
        let mark = mass_stream.mark();
        let keep = match class.kind {
            ClassKind::Living => mark.is_below(Threshold::from_probability(depletion(mass, alpha))),
            ClassKind::Tail => {
                let kept = depletion(mass, alpha);
                kept >= 1.0 || mark.is_below(Threshold::from_probability(1.0 - kept))
            }
            ClassKind::WhiteDwarf | ClassKind::NeutronStar | ClassKind::BlackHole => {
                // Only a low-mode kick is judged on the pair's speed, and a white dwarf's never
                // takes the low mode, so its attempt leaves the speed undrawn, as a living star's
                // does: each attempt seeks its own words.
                let systemic = if class.kind == ClassKind::WhiteDwarf {
                    0.0
                } else {
                    systemic_speed(&mut mass_stream)
                };
                remnant_attempt_fits(
                    galaxy,
                    model,
                    (class.kind, mass),
                    &composition,
                    (seed, system, attempt),
                    age,
                    (v_eff, systemic),
                )
            }
        };
        if keep {
            chosen = Some((mass, attempt));
            break;
        }
    }
    debug_assert!(
        chosen.is_some(),
        "a member of {class:?} exhausted its {MAX_ATTEMPTS} attempts"
    );
    let (primary_mass, attempt) = chosen?;
    let record = SystemRecord::from_parts(
        system,
        *position,
        SystemOrigin::FeatureMember {
            feature: PackedFeature::from(feature.id()),
            attempt: u16::try_from(attempt).expect("an attempt below 4,096 fits in u16"),
        },
        model.population(),
        SolarMasses::new(primary_mass),
        age,
    );
    let velocity = member_velocity(galaxy, feature, model, class, position, key, bulk);
    let ctx = MultiplicityContext::ForcedMultiple { max_separation };
    let marked_attempt = bearing_attempt(galaxy, &record, &composition, ctx, marked);
    // A mark that none of its attempts bears out is not borne: the member is its class's, its
    // separation its class's boundary or none.
    let (recycled, max_separation) = if marked_attempt.is_some() {
        (marked, max_separation)
    } else {
        (
            RecycledMark::Unmarked,
            mark_and_boundary(
                galaxy,
                (feature, model),
                (class, RecycledShares::NONE),
                position,
                key,
            )
            .1,
        )
    };
    Some(MemberRecord {
        record,
        class: *class,
        composition,
        abundances,
        velocity,
        listed: None,
        marked_attempt,
        max_separation,
        recycled,
    })
}

/// A member's light-element offsets, composition and age at the epoch, of `generation`, in the
/// cluster of `model`, drawn on `member.abundance` keyed by `key` (module documentation).
#[must_use]
fn member_abundances(
    galaxy: &Galaxy,
    model: &ClusterModel,
    generation: Generation,
    key: ObjectKey,
) -> (MemberAbundances, Composition, Years) {
    let mut abundance = Stream::open(galaxy.seed(), tags::MEMBER_ABUNDANCE, key);
    let enrichment = abundance.uniform_open_low();
    let _reserved = abundance.next_u64();
    let spread_u = abundance.uniform();
    let iron_normal = abundance.standard_normal();
    let spread = model
        .marks()
        .iron_spread()
        .is_below(Threshold::from_probability(IRON_SPREAD_SHARE));
    let iron = Dex::new(if spread {
        IRON_SPREAD_SIGMA * iron_normal
    } else {
        0.0
    });
    let enriched = match generation {
        Generation::First => 0.0,
        Generation::Second => enrichment,
    };
    let abundances = MemberAbundances::of(
        enriched,
        model.initial_mass(),
        model.parameters().fe_h,
        iron,
    );
    let composition = Composition::from_fe_h(
        Dex::new(model.parameters().fe_h.value() + iron.value()),
        abundances.helium_excess,
    );
    let age = Years::new(model.age().value() - model.parameters().age_spread.value() * spread_u);
    (abundances, composition, age)
}

/// A member's recycled-object mark, drawn on `member.recycled` keyed by `key` against its class's
/// share in `shares`, and its widest separation: its cluster's hard–soft boundary at `position`
/// for a pair of its class's system mass, held to [`BLUE_STRAGGLER_SEPARATION`] for a straggler,
/// for a binary or marked member. Under the held arm, no mark and no separation.
#[must_use]
fn mark_and_boundary(
    galaxy: &Galaxy,
    (feature, model): (&FeatureRecord, &ClusterModel),
    (class, shares): (&MemberClass, RecycledShares),
    position: &GalacticPosition,
    key: ObjectKey,
) -> (RecycledMark, Option<Metres>) {
    if galaxy.rules().feature_binaries == FeatureBinariesRules::Unlisted {
        return (RecycledMark::Unmarked, None);
    }
    let (mark, share) = shares.of(class);
    let marked = if share > 0.0
        && Stream::open(galaxy.seed(), tags::MEMBER_RECYCLED, key)
            .mark()
            .is_below(Threshold::from_probability(share))
    {
        mark
    } else {
        RecycledMark::Unmarked
    };
    let separation = (class.multiplicity == Multiplicity::Binary
        || marked != RecycledMark::Unmarked)
        .then(|| {
            let local = local_offset(feature.position(), position);
            let radius = (local.x * local.x + local.y * local.y + local.z * local.z).sqrt();
            let system_mass = match class.multiplicity {
                Multiplicity::Binary => class.mean_mass.value(),
                Multiplicity::Single => class.mean_mass.value() * BINARY_MASS_FACTOR,
            };
            let boundary = hard_soft_separation(
                model,
                LightYears::new(radius),
                SolarMasses::new(system_mass),
            );
            match marked {
                RecycledMark::BlueStraggler => {
                    Metres::new(boundary.value().min(BLUE_STRAGGLER_SEPARATION.value()))
                }
                RecycledMark::Unmarked | RecycledMark::MillisecondPulsar => boundary,
            }
        });
    (marked, separation)
}

/// The conditional draw of a member marked `marked` with `record` and `composition`, under `ctx`:
/// the first of its marked redraw attempts whose stars bear its mark out, or `None` for an
/// unmarked member or a mark none bears out.
///
/// Each attempt is [`SystemStars::generate_at`]'s system, bit for bit, from the one primary its
/// attempts share, built once (rendering plan R06, R06.T16.a's cost at 27: the stragglers' search
/// was most of the members' census near the Sun). A straggler's attempt is not generated where its
/// stars, all of them together, weigh no more than the turn-off (`may_straggle`), since then none
/// of them can be a main-sequence star above it: the attempts found are the same.
#[must_use]
fn bearing_attempt(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
    ctx: MultiplicityContext,
    marked: RecycledMark,
) -> Option<RedrawAttempt> {
    if marked == RecycledMark::Unmarked {
        return None;
    }
    let (primary, _) = primary_model(galaxy, record, *composition);
    let turn_off = turn_off_mass(record.age_at_epoch(), composition);
    (0..MARKED_ATTEMPTS)
        .filter_map(RedrawAttempt::marked)
        .find(|&n| match marked {
            RecycledMark::Unmarked => false,
            RecycledMark::MillisecondPulsar => recycles(
                galaxy,
                record,
                &SystemStars::at_attempt(galaxy, record, &primary, ctx, n),
            ),
            RecycledMark::BlueStraggler => {
                let hierarchy = draw_hierarchy_of_primary(galaxy, record, &primary, ctx, n);
                may_straggle(&hierarchy, turn_off)
                    && straggles(
                        &SystemStars::at_attempt(galaxy, record, &primary, ctx, n),
                        turn_off,
                    )
            }
        })
}

/// The share of the turn-off by which a system's stars must weigh more than it before
/// [`may_straggle`] lets its attempt be generated: far above the rounding of the binary engine's
/// sums of the masses it moves (each step's sum rounds by some 10⁻¹⁶ of the mass).
const STRAGGLER_MASS_MARGIN: f64 = 1e-9;

/// The mass, M☉, by which [`may_straggle`] allows a system's stars more than they weigh: ten times
/// the floor of 10⁻⁹ M☉ under which the binary engine holds a star it drains (`transfer_trial`),
/// the one place the engine can leave a star more than it is given.
const STRAGGLER_FLOOR_ALLOWANCE_MSUN: f64 = 1e-8;

/// Whether the stars of `hierarchy` could hold a main-sequence star more massive than `turn_off`.
///
/// That is whether they weigh more than it together, less [`STRAGGLER_MASS_MARGIN`] of it and
/// [`STRAGGLER_FLOOR_ALLOWANCE_MSUN`]. No star of a system is ever heavier than all of its stars at
/// birth: a star's wind takes mass away, and the binary engine moves mass only between the two
/// stars of a pair, a merger's product holding at most both, so a system that weighs no more
/// cannot bear a straggler out ([`straggles`]).
#[must_use]
fn may_straggle(hierarchy: &SystemHierarchy, turn_off: SolarMasses) -> bool {
    let total = hierarchy
        .stars()
        .iter()
        .fold(0.0, |sum, star| sum + star.initial_mass().value());
    total > turn_off.value() * (1.0 - STRAGGLER_MASS_MARGIN) - STRAGGLER_FLOOR_ALLOWANCE_MSUN
}

/// [`bearing_attempt`] as it was first written, each attempt generated whole from its own primary:
/// what the tests hold it to.
#[cfg(test)]
#[must_use]
fn bearing_attempt_by_generation(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
    ctx: MultiplicityContext,
    marked: RecycledMark,
) -> Option<RedrawAttempt> {
    let bears = |stars: &SystemStars| match marked {
        RecycledMark::Unmarked => false,
        RecycledMark::MillisecondPulsar => recycles(galaxy, record, stars),
        RecycledMark::BlueStraggler => {
            straggles(stars, turn_off_mass(record.age_at_epoch(), composition))
        }
    };
    if marked == RecycledMark::Unmarked {
        return None;
    }
    (0..MARKED_ATTEMPTS)
        .filter_map(RedrawAttempt::marked)
        .find(|&n| {
            bears(&SystemStars::generate_at(
                galaxy,
                record,
                composition,
                ctx,
                n,
            ))
        })
}

/// The widest separation a member marked a blue straggler draws its companions at, m: 0.03 au,
/// ours, about six solar radii, where a pair of stars near a globular's turn-off is close enough to
/// come into contact and merge by the epoch. Inside it the binary engine leaves a star above the
/// turn-off in about one draw in sixty, and inside 1 au in one in 150 (measured for P11.T8.f, 600
/// pairs near a 12 Gyr turn-off at \[Fe/H\] = −0.72).
pub const BLUE_STRAGGLER_SEPARATION: Metres = Metres::new(0.03 * METRES_PER_AU);

/// Whether `stars` hold a main-sequence star more massive than `turn_off` at the epoch, a blue
/// straggler, and no carved pair: what a member marked one keeps an attempt by (P11.T8.f).
#[must_use]
fn straggles(stars: &SystemStars, turn_off: SolarMasses) -> bool {
    stars.carved_pair().is_none()
        && stars
            .state_at(crate::time::UniverseTime::EPOCH)
            .is_some_and(|state| {
                state
                    .stars()
                    .iter()
                    .any(|s| s.phase() == Phase::MainSequence && s.mass() > turn_off)
            })
}

/// The separation of `model`'s hard–soft boundary for a pair of system mass `mass` at `radius`
/// from its centre (plan 11, P11.T2.b; supplied by P11.T8.f): where the pair's orbital speed
/// `√(G M ÷ a)` equals the cluster's dispersion there, `a = G M ÷ σ(r)²`, with plan 09's
/// [`ClusterModel::sigma`]. A wider pair is soft: encounters with the cluster's stars, as fast as
/// its own orbit, unbind it rather than harden it (Heggie 1975, MNRAS 173, 729, whose boundary
/// sets the pair's binding energy against the field stars' mean kinetic energy; this is that
/// boundary for a pair of the field stars' mass, as the plan words it).
///
/// # Panics
///
/// Never: a cluster's dispersion is positive and finite at every radius.
#[must_use]
pub fn hard_soft_separation(model: &ClusterModel, radius: LightYears, mass: SolarMasses) -> Metres {
    let sigma = model.sigma(radius).value() * 1.0e3;
    Metres::new(GravitationalParameter::from_solar_masses(mass).value() / (sigma * sigma))
}

/// Whether `stars`, those of `record`, have recycled its primary, a neutron star, into a millisecond
/// pulsar at the epoch ([`is_millisecond_pulsar`]: under 30 ms and above the death line) in a pair
/// the binary engine ran, and hold no carved pair: what a member marked a millisecond pulsar keeps
/// an attempt by (P11.T8.f).
#[must_use]
fn recycles(galaxy: &Galaxy, record: &SystemRecord, stars: &SystemStars) -> bool {
    if stars.carved_pair().is_some() {
        return false;
    }
    let draws = crate::stellar::system::primary_draws(galaxy, record);
    stars.pairs().iter().any(|pair| {
        pair.stars()[0] == StarIndex::PRIMARY
            && pulsar_at(
                pair.timeline(),
                Component::Primary,
                &draws,
                record.age_at_epoch(),
            )
            .is_some_and(|pulsar| pulsar.recycled() && is_millisecond_pulsar(pulsar.state()))
    })
}

/// Whether the attempt's star of initial mass `mass` leaves a remnant of `kind` kicked below `v_eff`
/// (km/s), a low-mode neutron star judged on its pair's `systemic` speed instead (km/s), with the
/// star built under the galaxy's stellar `rules`.
///
/// The star's model is not built: [`RemnantFate`] gives its remnant and judges its kick bit for
/// bit from builds that keep no segment, and reads the companion-stripped mark only where it can
/// change the answer (13.9 s to 6.0 s for a globular core's 50 ly query, a pod of 16 vCPU).
pub(crate) fn remnant_fits(
    kind: ClassKind,
    mass: f64,
    composition: &Composition,
    (universe, system, attempt): (crate::Seed, SystemId, u32),
    age: Years,
    (v_eff, systemic): (f64, f64),
    rules: StellarRules,
) -> bool {
    let want = match kind {
        ClassKind::WhiteDwarf => RemnantKind::WhiteDwarf,
        ClassKind::NeutronStar => RemnantKind::NeutronStar,
        ClassKind::BlackHole => RemnantKind::BlackHole,
        ClassKind::Living | ClassKind::Tail => return true,
    };
    let draws = StarDraws::for_attempt(universe, BodyId::new(system, 0), attempt);
    let Ok(Some(fate)) = RemnantFate::of(SolarMasses::new(mass), *composition, draws, age, rules)
    else {
        return false;
    };
    if fate.remnant().kind() != want {
        return false;
    }
    fate.kick_is_none_or(|kick| {
        let judged = if kick.mode() == KickMode::Low {
            systemic
        } else {
            KilometresPerSecond::from(kick.speed()).value()
        };
        judged < v_eff
    })
}

/// The words each attempt of a member's conditional draw reads on `member.mass`: the mass, the
/// mark, and a pair's systemic velocity (two normals as a pair, then one).
pub(crate) const ATTEMPT_WORDS: u64 = 6;

/// A pair's systemic speed on `stream` from its current word, km/s: an isotropic Maxwellian of σ
/// [`PAIR_SYSTEMIC_SIGMA`].
pub(crate) fn systemic_speed(stream: &mut Stream) -> f64 {
    let (a, b) = stream.standard_normal_pair();
    let c = stream.standard_normal();
    PAIR_SYSTEMIC_SIGMA * (a * a + b * b + c * c).sqrt()
}

/// The systemic speed attempt `attempt` of member `id`'s conditional draw read, km/s: what judges a
/// low-mode neutron star (for tests and the retained-kick check).
#[must_use]
pub fn attempt_systemic_speed(galaxy: &Galaxy, id: FeatureMemberId, attempt: u32) -> f64 {
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::MEMBER_MASS,
        ObjectKey::from(SystemId::from(id)),
    );
    stream.seek(ATTEMPT_WORDS * u64::from(attempt) + 2);
    systemic_speed(&mut stream)
}

/// The member's velocity (module documentation).
fn member_velocity(
    galaxy: &Galaxy,
    feature: &FeatureRecord,
    model: &ClusterModel,
    class: &MemberClass,
    position: &GalacticPosition,
    key: ObjectKey,
    bulk: GalacticVelocity,
) -> GalacticVelocity {
    let local = local_offset(feature.position(), position);
    let radius = (local.x * local.x + local.y * local.y + local.z * local.z).sqrt();
    let tail = class.kind == ClassKind::Tail;
    let read_at = if tail {
        model.tidal_radius().value()
    } else {
        radius
    };
    let sigma = model.sigma(LightYears::new(read_at)).value()
        * equipartition_factor(class.mean_mass, model.turn_off_mass());
    let cut = if tail {
        f64::INFINITY
    } else {
        model.escape_speed_at(LightYears::new(radius)).value()
    };
    let norm = |w: &[f64; 3]| (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    let mut stream = Stream::open(galaxy.seed(), tags::MEMBER_VELOCITY, key);
    let mut last = [0.0; 3];
    let mut kept = None;
    for attempt in 0..VELOCITY_ATTEMPTS {
        stream.seek(4 * u64::from(attempt));
        let (first, second) = stream.standard_normal_pair();
        let third = stream.standard_normal();
        let internal = [first * sigma, second * sigma, third * sigma];
        if norm(&internal) < cut {
            kept = Some(internal);
            break;
        }
        last = internal;
    }
    let internal = kept.unwrap_or_else(|| {
        let speed = norm(&last);
        last.map(|c| c * 0.99 * cut / speed)
    });
    let [bx, by, bz] = bulk.metres_per_second();
    GalacticVelocity::new([
        bx + internal[0] * 1e3,
        by + internal[1] * 1e3,
        bz + internal[2] * 1e3,
    ])
}

/// Bianchini et al.'s (2016, MNRAS 458, 3644) partial equipartition's mass for which velocities
/// turn from exponential to a power law, `m_eq` = 1.5 M☉, their maximum (ruling 126.7).
pub const EQUIPARTITION_MASS: f64 = 1.5;

/// Bianchini et al. 2016's `g(m)`: `e^(−m ÷ 2m_eq)` up to `m_eq`, `e^(−½) (m ÷ m_eq)^(−½)` above.
#[must_use]
pub fn equipartition_g(m: SolarMasses) -> f64 {
    let x = m.value() / EQUIPARTITION_MASS;
    if x <= 1.0 {
        math::exp(-0.5 * x)
    } else {
        math::exp(-0.5) / x.sqrt()
    }
}

/// A class's velocity dispersion over the stars' at the turn-off, `g(m) ÷ g(m_TO)` (ruling
/// 126.7): 1.18 for band A's 0.3 M☉ at a 0.8 M☉ turn-off, against full equipartition's 1.63.
#[must_use]
pub fn equipartition_factor(m: SolarMasses, turn_off: SolarMasses) -> f64 {
    equipartition_g(m) / equipartition_g(turn_off)
}

/// `position` relative to `centre`, ly.
#[must_use]
pub fn local_offset(centre: &GalacticPosition, position: &GalacticPosition) -> PointLy {
    let [x, y, z] = centre
        .displacement_to(position)
        .metres()
        .map(|m| m / METRES_PER_LIGHT_YEAR);
    PointLy::new(x, y, z)
}

/// A member's sphere of influence: the smaller of its galactic tidal radius and the same formula
/// about its cluster's centre, `(m (r² + a²)^(3/2) ÷ 3M)^(1/3)` in the cluster's Plummer sphere,
/// which keeps its floor `a (m ÷ 3M)^(1/3)` in the harmonic core (P09.T10). The member's mass is
/// its class's mean.
#[must_use]
pub fn sphere_of_influence(
    galaxy: &Galaxy,
    feature: &FeatureRecord,
    model: &ClusterModel,
    member: &MemberRecord,
) -> Metres {
    let position = member.record().epoch_position();
    let m = member.class().mean_mass;
    let galactic = galaxy
        .potential()
        .tidal_radius(m, &PointLy::from(position))
        .value();
    let local = local_offset(feature.position(), position);
    Metres::new(galactic.min(cluster_tidal_radius(model, &local, m).value()))
}

/// The Hill radius of a system of mass `m` at `local` (ly from its cluster's centre) about the
/// mass of the cluster's Plummer sphere inside it, `r (m ÷ 3M(<r))^(1/3)` =
/// `(m (r² + a²)^(3/2) ÷ 3M)^(1/3)`, m: P09.T10's rule, the cluster's term of
/// [`sphere_of_influence`], which plan 14's strip radius reads at the system's own mass
/// (P14.T29).
#[must_use]
pub fn cluster_tidal_radius(model: &ClusterModel, local: &PointLy, m: SolarMasses) -> Metres {
    let r2 = local.x * local.x + local.y * local.y + local.z * local.z;
    let a = model.plummer_scale();
    let cluster_ly =
        math::cbrt(m.value() * math::powf(r2 + a * a, 1.5) / (3.0 * model.mass().value()));
    Metres::new(cluster_ly * METRES_PER_LIGHT_YEAR)
}

#[cfg(test)]
mod binary_tests;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::features::FeatureProcess;
    use crate::galaxy::features::catalogue::FeatureCatalogue;
    use crate::galaxy::features::interior::MemberClassTable;
    use crate::galaxy::imf::MassBand;
    use crate::galaxy::params::GalaxyParams;
    use crate::id::MemberSlot;
    use hyperion_testkit::stats::regularised_gamma_p;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(0x0910_0000), GalaxyParams::milky_way_like()).unwrap()
    }

    /// The most massive globular of the fixture, its model and class table.
    fn cluster(galaxy: &Galaxy) -> (FeatureRecord, ClusterModel, MemberClassTable) {
        let feature = FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
            .max_by(|a, b| {
                let m = |f: &FeatureRecord| match f.marks() {
                    super::super::catalogue::FeatureMarks::Globular(g) => g.mass().value(),
                    _ => 0.0,
                };
                m(a).total_cmp(&m(b))
            })
            .expect("the fixture has globulars");
        let model = ClusterModel::from_record(galaxy, &feature).expect("a globular is a cluster");
        let table = MemberClassTable::new(galaxy, &model, feature.reach(), [1.0, 0.0, 0.0]);
        (feature, model, table)
    }

    fn member_id(feature: &FeatureRecord, band: MassBand, index: u16) -> FeatureMemberId {
        FeatureMemberId::new(
            feature.id().feature_ref(),
            MemberSlot::InCell {
                band: band.layer(),
                level: 0,
                cell: [8, 8, 8],
                index: u32::from(index),
            },
        )
        .unwrap()
    }

    fn class_of(table: &MemberClassTable, band: MassBand, kind: ClassKind) -> MemberClass {
        *table
            .classes(band)
            .find(|(c, _)| c.kind == kind && c.multiplicity == Multiplicity::Single)
            .expect("the class exists")
            .0
    }

    fn at(feature: &FeatureRecord, offset: [f64; 3]) -> GalacticPosition {
        let [x, y, z] = feature.position().to_light_years_f64();
        GalacticPosition::from_light_years([x + offset[0], y + offset[1], z + offset[2]]).unwrap()
    }

    #[test]
    fn retained_neutron_stars_are_kicked_below_the_escape_speed() {
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let class = class_of(&table, MassBand::E, ClassKind::NeutronStar);
        let position = at(&feature, [1.0, 0.0, 0.0]);
        let v_eff = model.escape_speed_effective().value();
        for i in 0..40 {
            let id = member_id(&feature, MassBand::E, i);
            let member = draw_member(
                &galaxy,
                &feature,
                &model,
                &class,
                &position,
                id,
                GalacticVelocity::new([0.0; 3]),
            )
            .expect("a neutron star is retained within the attempts");
            let stars = member.stars(&galaxy);
            let primary = &stars.stars()[0];
            assert_eq!(
                primary.remnant().map(|r| r.kind()),
                Some(RemnantKind::NeutronStar)
            );
            let kick = primary.natal_kick().expect("a neutron star is kicked");
            let judged = if kick.mode() == KickMode::Low {
                attempt_systemic_speed(&galaxy, id, member.attempt())
            } else {
                KilometresPerSecond::from(kick.speed()).value()
            };
            assert!(judged < v_eff, "{judged} km/s against {v_eff}");
        }
    }

    #[test]
    fn members_disperse_in_partial_equipartition() {
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let r = 0.5 * model.half_mass_radius().value();
        let position = at(&feature, [r, 0.0, 0.0]);
        for band in [MassBand::A, MassBand::C] {
            let class = class_of(&table, band, ClassKind::Living);
            let n = 3_000_u16;
            let mut sum2 = 0.0;
            for i in 0..n {
                let id = member_id(&feature, band, i);
                let m = draw_member(
                    &galaxy,
                    &feature,
                    &model,
                    &class,
                    &position,
                    id,
                    GalacticVelocity::new([0.0; 3]),
                )
                .unwrap();
                let [vx, vy, vz] = m.velocity().metres_per_second();
                sum2 += (vx * vx + vy * vy + vz * vz) * 1e-6;
            }
            let sigma = (sum2 / (3.0 * f64::from(n))).sqrt();
            let free = model.sigma(LightYears::new(r)).value()
                * equipartition_factor(class.mean_mass, model.turn_off_mass());
            // The escape cut trims the Maxwellian: E[v² | v < c] = 3σ² F₅(x) ÷ F₃(x), x = c² ÷ σ²,
            // F_k the χ² distribution of k degrees.
            let cut = model.escape_speed_at(LightYears::new(r)).value();
            let x = cut * cut / (free * free);
            let ratio = regularised_gamma_p(2.5, 0.5 * x) / regularised_gamma_p(1.5, 0.5 * x);
            let want = free * ratio.sqrt();
            assert!(
                (sigma / want - 1.0).abs() < 0.04,
                "{band:?}: {sigma} against {want}"
            );
        }
    }

    #[test]
    fn band_a_moves_1_18_times_as_fast_as_the_turn_off() {
        let f = equipartition_factor(SolarMasses::new(0.3), SolarMasses::new(0.8));
        assert!((f - 1.18).abs() < 0.005, "{f}");
        // Continuous at m_eq, and falling above it as m^(−½).
        let below = equipartition_g(SolarMasses::new(1.5 * (1.0 - 1e-12)));
        assert!((below - equipartition_g(SolarMasses::new(1.5))).abs() < 1e-9);
        let ratio = equipartition_g(SolarMasses::new(6.0)) / equipartition_g(SolarMasses::new(1.5));
        assert!((ratio - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_member_does_not_depend_on_the_order_members_are_drawn_in() {
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let class = class_of(&table, MassBand::B, ClassKind::Living);
        let position = at(&feature, [0.0, 2.0, -1.0]);
        let draw = |i: u16| {
            draw_member(
                &galaxy,
                &feature,
                &model,
                &class,
                &position,
                member_id(&feature, MassBand::B, i),
                GalacticVelocity::new([0.0; 3]),
            )
        };
        let forward: Vec<_> = (0..20).map(draw).collect();
        let backward: Vec<_> = (0..20).rev().map(draw).collect();
        assert!(forward.iter().eq(backward.iter().rev()));
        let m = forward[3].unwrap();
        assert_eq!(m.record().component(), None);
        assert_eq!(m.record().population(), model.population());
        assert!(
            (class.initial_mass_range.0.value()..=class.initial_mass_range.1.value())
                .contains(&m.record().primary_initial_mass().value())
        );
    }

    /// A binary neutron-star member, a primary of 8 M☉ or more forced multiple, judges its
    /// primary's stripped mark at its own composition and draws, not a grid system's metallicity
    /// draw (whose debug assertion a member, with no density component, trips), and its brief
    /// model routes it the same way (`BriefModel::of_member`): a member whose pair can have
    /// changed its primary on the paired route (P11.T11.a), whose brief is its stars' bit for bit.
    #[test]
    fn a_massive_binary_member_is_drawn_at_its_own_composition() {
        use crate::stellar::brief::{BriefModel, BriefRoute};
        use crate::stellar::multiplicity::draw_hierarchy_of_composition;
        use crate::stellar::system::GRID_ATTEMPT;
        use crate::time::UniverseTime;
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let class = *table
            .classes(MassBand::E)
            .find(|(c, _)| {
                c.kind == ClassKind::NeutronStar && c.multiplicity == Multiplicity::Binary
            })
            .expect("a globular has binary neutron stars")
            .0;
        let position = at(&feature, [1.0, 0.0, 0.0]);
        let mut multiple = 0;
        let mut paired = 0;
        for i in 0..8 {
            let member = draw_member(
                &galaxy,
                &feature,
                &model,
                &class,
                &position,
                member_id(&feature, MassBand::E, i),
                GalacticVelocity::new([0.0; 3]),
            )
            .expect("a neutron star is retained within the attempts");
            let stars = member.stars(&galaxy);
            assert_eq!(stars.primary().composition(), member.composition());
            assert_eq!(
                stars.hierarchy(),
                &draw_hierarchy_of_composition(
                    &galaxy,
                    member.record(),
                    member.composition(),
                    member.multiplicity_context(),
                    GRID_ATTEMPT,
                )
            );
            multiple += usize::from(stars.star_count() > 1);
            let brief = BriefModel::of_member(&galaxy, &member);
            assert_eq!(brief.star_count(), stars.star_count());
            paired += usize::from(brief.route() == BriefRoute::Paired);
            let (fitted, exact) = (
                brief.brief_at(UniverseTime::EPOCH).expect("formed"),
                stars.brief_at(UniverseTime::EPOCH).expect("formed"),
            );
            if brief.route() == BriefRoute::Table {
                assert_eq!(
                    (fitted.kind(), fitted.class().to_string()),
                    (exact.kind(), exact.class().to_string())
                );
            } else {
                assert_eq!(fitted, exact);
            }
        }
        assert!(multiple > 0, "no binary member kept a companion");
        // A neutron star's pair may have been kicked and interacted, so its brief reads the pair.
        assert!(paired > 0, "no binary member takes the paired route");
    }

    #[test]
    fn generate_with_is_generate_in_for_a_grid_record() {
        use crate::galaxy::placement::{CellKey, generate_cell};
        use crate::id::Layer;
        use crate::stellar::system::draw_metallicity;
        let galaxy = galaxy();
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::new(Layer::C, [0, 812, 0]).unwrap(),
            &mut cell,
        );
        for record in cell.iter().take(5) {
            let a = SystemStars::generate_in(&galaxy, record, MultiplicityContext::Free);
            let b = SystemStars::generate_with(
                &galaxy,
                record,
                &draw_metallicity(&galaxy, record),
                MultiplicityContext::Free,
            );
            assert_eq!(a, b);
        }
    }

    #[test]
    fn a_member_s_sphere_of_influence_is_the_smaller_radius() {
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let class = class_of(&table, MassBand::A, ClassKind::Living);
        let centre = draw_member(
            &galaxy,
            &feature,
            &model,
            &class,
            &at(&feature, [0.01, 0.0, 0.0]),
            member_id(&feature, MassBand::A, 1),
            GalacticVelocity::new([0.0; 3]),
        )
        .unwrap();
        let soi =
            sphere_of_influence(&galaxy, &feature, &model, &centre).value() / METRES_PER_LIGHT_YEAR;
        // In the harmonic core the floor: a (m ÷ 3M)^(1/3).
        let floor = model.plummer_scale()
            * math::cbrt(class.mean_mass.value() / (3.0 * model.mass().value()));
        assert!((soi / floor - 1.0).abs() < 1e-3, "{soi} against {floor}");
    }
}
