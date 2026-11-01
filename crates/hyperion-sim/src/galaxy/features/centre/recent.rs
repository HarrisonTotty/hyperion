//! The galactic centre's recently dead and its luminous blue variables (plan 09, P09.T36.b), on
//! its feature-level list after the tidal disruptions' victims, from index N + 1, under P09.T36's
//! hold (`FEATURE_CLASSES_VERSION`), and plan 11's binary classes after them, under P11.T8.f's
//! (`centre::binaries`).
//!
//! # The classes
//!
//! In Design note 16's order after the victims (research-leans-10, item 4):
//!
//! - **`CORE_COLLAPSE`**, from the centre's young classes, the constant component and the burst's
//!   two tracers: band-E stars whose primary dies in the supernova test's interval and makes a
//!   shell that the test keeps, in the gas at the site (the smooth gas about the origin; the
//!   field's surroundings), as a catalogue feature's are drawn
//!   ([`recent`](crate::galaxy::features::members::recent)): each component's band-E primaries
//!   times the share of band E in the envelope of masses and ages ([`Candidates`]) whose lifetime
//!   bracket can put a death in the interval.
//! - **`TYPE_IA`**, from its old classes: each component's formed mass times its mean Type Ia rate
//!   over its ages, drawn delay first on a delay-time distribution of its ages.
//! - **`LUMINOUS_BLUE_VARIABLE`**, from its young classes: band-E stars whose window overlaps the
//!   source horizon and that the core collapses do not claim. Both or neither: the list holds them
//!   exactly where the centre's band-E cells carve them, by the one switch of the hold and the one
//!   claim, [`claimed_at_centre`].
//!
//! An entry picks its component by the components' expected candidates, draws its radius from its
//! tracer's profile, its direction isotropically and its velocity from its tracer's distribution
//! function, as a cell's member does, without the marks (lean, 2026-10-31); its site is its epoch
//! position moved straight along that velocity to its death.
//!
//! # The cells' half
//!
//! A band-E member of a young class that [`claimed_at_centre`] claims is no member of the cells
//! ([`list_claims`], which `CentrePlacement::candidate` asks), so a recent death or a variable is
//! the list's or the cells', and never both. The old classes' Type Ia carve nothing: the centre
//! draws no binary that a Type Ia could come from (lean, 2026-10-31).

use std::num::NonZeroU16;

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::galaxy::catalogue_classes::lbv::{horizon_window, may_host};
use crate::galaxy::catalogue_classes::supernova::{
    DeathMarks, SupernovaInterval, Surroundings, claimed_window,
};
use crate::galaxy::catalogue_classes::type_ia::{DelayTimeDistribution, IaLeftover, IaProgenitor};
use crate::galaxy::displaced::{ExplosionSite, death_in_interval_under};
use crate::galaxy::features::interior::{ClassProfile, ProfileShape};
use crate::galaxy::features::members::recent::{
    Candidates, FeatureClassesRules, RecentClass, moved, uniform_ages,
};
use crate::galaxy::features::members::{FeatureLevelList, ListClass, ListIndex, MemberZero};
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::snr::SHELL_WINDOW_CAP;
use crate::galaxy::snr::{ShellEnvironment, ShellWindow};
use crate::galaxy::{Galaxy, Population};
use crate::id::{CentreMemberId, MemberSlot, SystemId};
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::{SystemStars, primary_draws, primary_eta};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{SolarMasses, Years};

use super::CentreModel;
use super::binaries::CentreBinaries;
use super::classes::{AgeComponent, CentreClass, CentreClassKind, age_components};
use super::events::victims::victim_count;
use super::members::{CentreMemberRecord, centre_composition};
use crate::galaxy::catalogue_classes::binary::feature::FeatureBinariesRules;

/// The oldest age of a young class: a component whose oldest star is at most this old is young,
/// 150 Myr, the constant component's (`age_components`); the rest are old.
const YOUNG_OLDEST: Years = Years::new(1.5e8);

/// One component's part of a class: its expected candidates and, for a core collapse or a
/// variable, what its candidates draw their masses and ages from.
#[derive(Debug, Clone, PartialEq)]
struct Part {
    component: u8,
    expected: f64,
    candidates: Option<Candidates>,
}

/// The centre's recent classes (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CentreRecent {
    /// Each class's parts, in list order, those of a class expecting none left out.
    classes: Vec<(RecentClass, Vec<Part>)>,
    /// Plan 11's binary classes after them (P11.T8.f): none under P11.T8.f's hold.
    binaries: CentreBinaries,
    /// The index of the classes' first entry, past the victims'.
    first: NonZeroU16,
}

impl CentreRecent {
    /// The centre's recent classes in `galaxy`, whose centre is `model` and whose victims are
    /// `victims`: none under the hold's held arm.
    ///
    /// # Panics
    ///
    /// If `victims` is 65,535, which leaves no index past them: the centre's headroom check
    /// counts the victims against its 65,536, and a Milky Way's are some thirty.
    #[must_use]
    pub(crate) fn new(galaxy: &Galaxy, model: &CentreModel, victims: u16) -> Self {
        let classes = match galaxy.rules().feature_classes {
            FeatureClassesRules::Unlisted => Vec::new(),
            FeatureClassesRules::Listed => Self::classes(galaxy, model),
        };
        let first = victims
            .checked_add(1)
            .and_then(NonZeroU16::new)
            .expect("the victims leave the centre's list an index past them");
        Self {
            classes,
            binaries: CentreBinaries::new(galaxy, model),
            first,
        }
    }

    /// The parts of each class in `galaxy`, at its window caps.
    #[must_use]
    fn classes(galaxy: &Galaxy, model: &CentreModel) -> Vec<(RecentClass, Vec<Part>)> {
        let caps = galaxy.window_caps();
        parts(
            model.classes().primaries_formed(),
            galaxy.mass_function(),
            galaxy.mean_formed_mass(),
            &SupernovaInterval::of_cap(caps, ShellEnvironment::Field),
            &SupernovaInterval::of_cap(caps, ShellEnvironment::TypeIa),
        )
    }

    /// Each class's expected candidates, in list order.
    #[must_use]
    pub(crate) fn expected(&self) -> Vec<(RecentClass, f64)> {
        self.classes
            .iter()
            .map(|(class, parts)| (*class, parts.iter().map(|p| p.expected).sum()))
            .collect()
    }

    /// The centre's hosts of plan 11's binary classes (P11.T8.f).
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn binaries(&self) -> &CentreBinaries {
        &self.binaries
    }

    /// The centre's list of its recent classes, and plan 11's binary classes after them (P11.T8.f),
    /// from index `victims + 1`: its counts, and its entries' indices (the positions are each
    /// entry's own, [`entry`](Self::entry), [`CentreBinaries::entry`]).
    ///
    /// # Panics
    ///
    /// If the victims and the classes' candidates together cannot keep eight standard deviations
    /// under the centre's 65,536 indices, which the centre's headroom check, counting both, rules
    /// out for a galaxy that passes it ([`FeatureLevelList::with_classes`]).
    #[must_use]
    pub(crate) fn list(&self, galaxy: &Galaxy) -> FeatureLevelList {
        // The positions are each entry's own draw, so the list's profile is never read.
        let placeholder = ClassProfile::new(ProfileShape::Plummer { scale: 1.0 }, 128.0, 1.0);
        let mut classes: Vec<ListClass> = self
            .expected()
            .into_iter()
            .map(|(class, expected)| ListClass::new(class.class(), expected, placeholder.clone()))
            .collect();
        classes.extend(self.binaries.list_classes());
        FeatureLevelList::with_classes_from(
            galaxy,
            SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).raw(),
            &GalacticPosition::ORIGIN,
            MemberZero::Present,
            ListIndex::Centre,
            self.first,
            classes,
        )
    }

    /// The entry at list index `index`, of class `class`, if its class's test keeps it (module
    /// documentation).
    #[must_use]
    pub(crate) fn entry(
        &self,
        galaxy: &Galaxy,
        model: &CentreModel,
        class: RecentClass,
        index: u16,
    ) -> Option<CentreRecentEntry> {
        let parts = &self.classes.iter().find(|(c, _)| *c == class)?.1;
        // `IndexTooLarge` is the one error, and an index past the centre's names no entry.
        let id = CentreMemberId::new(MemberSlot::FeatureLevel { index }).ok()?;
        let system = SystemId::from(id);
        let key = ObjectKey::from(system);
        let seed = galaxy.seed();
        let mut stream = Stream::open(seed, tags::MEMBER_MASS, key);
        let weights: Vec<f64> = parts.iter().map(|p| p.expected).collect();
        let total = weights.iter().fold(0.0, |s, w| s + w);
        let part = &parts[stream.mark().pick_weighted(&weights, total)?];
        let component = age_components()[usize::from(part.component)];
        let drawn = match &part.candidates {
            Some(candidates) => Some(candidates.draw(galaxy.mass_function(), &mut stream)?),
            None => None,
        };
        let class_of = centre_class(model, part.component)?;
        let at = position(model, &component, key, seed);
        let mut velocity = Stream::open(seed, tags::CENTRE_VELOCITY, key);
        let velocity_km_s = model.distribution(class_of.tracer).draw_velocity(
            model.profile(),
            at,
            &mut velocity,
        )?;
        let position = galactic(at);
        match class {
            RecentClass::CoreCollapse | RecentClass::LuminousBlueVariable => {
                let (mass, age, _) = drawn?;
                let record = record(system, position, mass, age);
                let member = CentreMemberRecord::from_parts(
                    record,
                    class_of,
                    centre_composition(),
                    velocity_km_s,
                );
                let (claim, death) =
                    claim_at_centre(galaxy, &record, &centre_composition(), member.velocity())?;
                (claim == class).then_some(CentreRecentEntry {
                    class,
                    member,
                    death,
                })
            }
            RecentClass::TypeIa => {
                let delays = DelayTimeDistribution::of_feature(
                    galaxy,
                    Population::NuclearDisc,
                    uniform_ages(component.youngest, component.oldest),
                );
                let first = galaxy.fields().component_id(0)?;
                let interval =
                    SupernovaInterval::of_cap(galaxy.window_caps(), ShellEnvironment::TypeIa);
                let mut stream = Stream::open(seed, tags::CLASS_TYPE_IA_PROGENITOR, key);
                let progenitor = IaProgenitor::draw_in(
                    &delays,
                    &mut stream,
                    first,
                    interval.start(),
                    interval.end(),
                )?;
                let mut stream = Stream::open(seed, tags::CLASS_TYPE_IA_LEFTOVER, key);
                let leftover = IaLeftover::draw(progenitor.channel(), &mut stream);
                let record = record(
                    system,
                    position,
                    progenitor.primary_mass(),
                    component.oldest,
                );
                let member = CentreMemberRecord::from_parts(
                    record,
                    class_of,
                    centre_composition(),
                    velocity_km_s,
                );
                let site = ExplosionSite::new(
                    moved(&position, member.velocity(), progenitor.explosion()),
                    progenitor.explosion(),
                );
                let marks = DeathMarks::of_type_ia(
                    galaxy,
                    system,
                    &progenitor,
                    &leftover,
                    &site,
                    member.velocity(),
                );
                let window = claimed_window(galaxy, &site, &marks, Surroundings::TypeIa)?;
                Some(CentreRecentEntry {
                    class,
                    member,
                    death: Some((site, window)),
                })
            }
        }
    }
}

/// The parts of each class of a centre whose cluster formed `primaries` primaries on
/// `mass_function`, `formed` per primary, its core collapses and variables drawn over `interval`
/// and its Type Ia over `ia`: the galaxy's, at its window caps, or the forecast's bound
/// ([`forecast_candidates`]).
#[must_use]
fn parts(
    primaries: f64,
    imf: &dyn MassFunction,
    formed: SolarMasses,
    interval: &SupernovaInterval,
    ia: &SupernovaInterval,
) -> Vec<(RecentClass, Vec<Part>)> {
    let components = age_components();
    let shares: f64 = components.iter().map(|c| c.share).sum();
    let all = imf.integral(MASS_BAND_EDGES[0], MASS_BAND_EDGES[5]);
    let band = imf.integral(MassBand::E.lo(), MassBand::E.hi());
    let band_e = |c: &AgeComponent| primaries * c.share / shares * band / all;
    let ia_years = ia.length().as_julian_years_f64();
    let (mut core, mut type_ia, mut lbv) = (Vec::new(), Vec::new(), Vec::new());
    for (component, c) in (0_u8..).zip(&components) {
        let ages = (c.youngest, c.oldest);
        if is_young(c) {
            let part = |candidates: Candidates| Part {
                component,
                expected: band_e(c) * candidates.share(),
                candidates: Some(candidates),
            };
            core.extend(Candidates::core_collapse(imf, ages, interval).map(part));
            lbv.extend(Candidates::luminous_blue_variable(imf, ages).map(part));
        } else {
            let mass = primaries * c.share / shares * formed.value();
            let rate = DelayTimeDistribution::mean_rate_over(&uniform_ages(ages.0, ages.1));
            type_ia.push(Part {
                component,
                expected: mass * rate * ia_years,
                candidates: None,
            });
        }
    }
    [
        (RecentClass::CoreCollapse, core),
        (RecentClass::TypeIa, type_ia),
        (RecentClass::LuminousBlueVariable, lbv),
    ]
    .into_iter()
    .map(|(class, parts)| {
        let parts: Vec<Part> = parts.into_iter().filter(|p| p.expected > 0.0).collect();
        (class, parts)
    })
    .filter(|(_, parts)| !parts.is_empty())
    .collect()
}

/// The centre's recent classes' expected candidates at their bound, for the forecast of its list
/// ([`centre_list_forecast`](crate::galaxy::features::members::centre_list_forecast)) that no galaxy
/// is built for: every window at its ceiling, [`SHELL_WINDOW_CAP`], so that the interval is the
/// longest any environment's cap gives.
#[must_use]
pub(crate) fn forecast_candidates(
    primaries: f64,
    mass_function: &dyn MassFunction,
    formed: SolarMasses,
) -> f64 {
    let bound = SupernovaInterval::of_window(SHELL_WINDOW_CAP);
    parts(primaries, mass_function, formed, &bound, &bound)
        .iter()
        .flat_map(|(_, parts)| parts.iter().map(|p| p.expected))
        .sum()
}

/// The member at the centre's list index `index` past its victims, if a recent class's test keeps
/// it, or plan 11's binary class's draws and test (P11.T8.f): none under both holds' held arms,
/// for an index before the classes' or past them.
#[must_use]
pub(crate) fn recent_member(
    galaxy: &Galaxy,
    model: &CentreModel,
    index: u16,
) -> Option<CentreMemberRecord> {
    if galaxy.rules().feature_classes == FeatureClassesRules::Unlisted
        && galaxy.rules().feature_binaries == FeatureBinariesRules::Unlisted
    {
        return None;
    }
    // The disruptions' model refuses only a black hole no galaxy the builder allows has, whose
    // list holds no entry.
    let victims = victim_count(galaxy).ok()?;
    let recent = CentreRecent::new(galaxy, model, victims);
    let entry = recent.list(galaxy).entry(index)?;
    match RecentClass::of(entry.class()) {
        Some(class) => recent
            .entry(galaxy, model, class, index)
            .map(|entry| entry.member),
        None => recent
            .binaries
            .entry(galaxy, model, entry.class(), index)
            .map(|(_, member)| member),
    }
}

/// One recent entry of the centre's list: its class, its member, and for a death its site and the
/// window of its shell.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CentreRecentEntry {
    class: RecentClass,
    member: CentreMemberRecord,
    death: Option<(ExplosionSite, ShellWindow)>,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P09.T37's merge reads the centre's entries; until then its tests alone do"
    )
)]
impl CentreRecentEntry {
    /// Its class.
    #[must_use]
    pub(crate) const fn class(&self) -> RecentClass {
        self.class
    }

    /// Its member.
    #[must_use]
    pub(crate) const fn member(&self) -> &CentreMemberRecord {
        &self.member
    }

    /// Where and when it explodes or exploded, for a death.
    #[must_use]
    pub(crate) fn site(&self) -> Option<&ExplosionSite> {
        self.death.as_ref().map(|(site, _)| site)
    }

    /// The window of its shell, for a death: none for a luminous blue variable.
    #[must_use]
    pub(crate) fn window(&self) -> Option<&ShellWindow> {
        self.death.as_ref().map(|(_, window)| window)
    }
}

/// The centre's young classes' claim on the member `record`, of composition `composition` moving
/// at `velocity` (module documentation): the core collapses if the one shared test keeps its death's
/// shell in the gas at its site, the luminous blue variables if its window overlaps the source
/// horizon and the core collapses do not claim it.
///
/// The cells ask it of each band-E member of a young class, the list of each candidate, so the two
/// halves agree on every mark set.
#[must_use]
pub(crate) fn claimed_at_centre(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
    velocity: GalacticVelocity,
) -> Option<RecentClass> {
    claim_at_centre(galaxy, record, composition, velocity).map(|(class, _)| class)
}

/// [`claimed_at_centre`] with the core collapse's site and window.
#[must_use]
fn claim_at_centre(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
    velocity: GalacticVelocity,
) -> Option<(RecentClass, Option<(ExplosionSite, ShellWindow)>)> {
    let stars = || {
        SystemStars::generate_with(
            galaxy,
            record,
            composition,
            MultiplicityContext::ForcedSingle,
        )
    };
    if let Some(death) = death_in_interval_under(
        record.primary_initial_mass(),
        composition,
        primary_eta(galaxy, record),
        record.age_at_epoch(),
        || primary_draws(galaxy, record),
        galaxy.rules().stellar,
    ) {
        let site = ExplosionSite::new(moved(record.epoch_position(), velocity, death), death);
        if let Some(window) = DeathMarks::of(galaxy, &stars(), &site, velocity)
            .and_then(|marks| claimed_window(galaxy, &site, &marks, Surroundings::Field))
        {
            return Some((RecentClass::CoreCollapse, Some((site, window))));
        }
    }
    (may_host(record.primary_initial_mass(), record.age_at_epoch())
        && horizon_window(&stars()).is_some())
    .then_some((RecentClass::LuminousBlueVariable, None))
}

/// Whether the centre's list claims `member`, a band-E member of its cells, which the cells then
/// give up: a member of a young class ([`centre_cells_claim`]) that [`claimed_at_centre`] claims.
#[must_use]
pub(crate) fn list_claims(galaxy: &Galaxy, member: &CentreMemberRecord) -> bool {
    member
        .class()
        .is_some_and(|class| centre_cells_claim(galaxy, class.component))
        && claimed_at_centre(
            galaxy,
            member.record(),
            member.composition(),
            member.velocity(),
        )
        .is_some()
}

/// Whether the centre's band-E cells give the list the members [`claimed_at_centre`] claims: a
/// member of a young class (`component`) under the hold's new arm, the one switch the list's
/// classes read too, so that the list holds a class exactly where the cells carve it.
#[must_use]
pub(crate) fn centre_cells_claim(galaxy: &Galaxy, component: u8) -> bool {
    match galaxy.rules().feature_classes {
        FeatureClassesRules::Unlisted => false,
        FeatureClassesRules::Listed => is_young(&age_components()[usize::from(component)]),
    }
}

/// Whether `component` is one of the young classes': its oldest star at most [`YOUNG_OLDEST`].
#[must_use]
fn is_young(component: &AgeComponent) -> bool {
    component.oldest.value() <= YOUNG_OLDEST.value()
}

/// The band-E class of the centre's component `component`, a living star's where it has one, or
/// `None` if it has no band-E class.
#[must_use]
pub(super) fn centre_class(model: &CentreModel, component: u8) -> Option<CentreClass> {
    let classes = model.classes().classes();
    let band_e = classes
        .iter()
        .filter(|c| c.band == MassBand::E && c.component == component);
    band_e
        .clone()
        .find(|c| c.kind == CentreClassKind::Living)
        .or_else(|| band_e.clone().next())
        .copied()
}

/// An entry's local position, ly about the black hole: its radius at a uniform's quantile of its
/// component's tracer's profile, by bisection, and an isotropic direction, on `member.position`.
#[must_use]
pub(super) fn position(
    model: &CentreModel,
    component: &AgeComponent,
    key: ObjectKey,
    seed: crate::Seed,
) -> [f64; 3] {
    let profile = component.tracer.profile(model.profile().stars());
    let mut stream = Stream::open(seed, tags::MEMBER_POSITION, key);
    let u = stream.uniform();
    let cut = profile.cut_radius().value();
    let whole = profile.fraction_within(cut);
    let (mut lo, mut hi) = (0.0_f64, cut);
    for _ in 0..64 {
        let mid = f64::midpoint(lo, hi);
        if profile.fraction_within(mid) < u * whole {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let r = f64::midpoint(lo, hi);
    let cos_theta = 2.0 * stream.uniform() - 1.0;
    let phi = 2.0 * core::f64::consts::PI * stream.uniform();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let (s, c) = math::sin_cos(phi);
    [r * sin_theta * c, r * sin_theta * s, r * cos_theta]
}

/// The galactic position of the local point `p`, ly about the black hole at the origin.
///
/// # Panics
///
/// Never for a point within the centre's reach, some hundreds of light-years.
#[must_use]
pub(super) fn galactic(p: [f64; 3]) -> GalacticPosition {
    GalacticPosition::ORIGIN
        .translated(GalacticDisplacement::new(
            p.map(|c| c * METRES_PER_LIGHT_YEAR),
        ))
        .expect("an entry within the centre's reach is addressable")
}

/// The centre's record of `system` at `position`, of initial mass `mass` and age `age`.
#[must_use]
fn record(
    system: SystemId,
    position: GalacticPosition,
    mass: SolarMasses,
    age: Years,
) -> SystemRecord {
    SystemRecord::from_parts(
        system,
        position,
        SystemOrigin::CentreMember { attempt: 0 },
        Population::NuclearDisc,
        mass,
        age,
    )
}

#[cfg(test)]
mod tests;
