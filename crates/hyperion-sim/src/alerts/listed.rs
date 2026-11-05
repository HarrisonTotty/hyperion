//! The hosts of the catalogue features' and the galactic centre's feature-level lists in the alert
//! walks (plan 09's P09.T36, P09.T43 and plan 11's P11.T8.f in plan 12's walks), from generator
//! version 29.
//!
//! From version 27 the young field gives most of its young stars to the features that bore them
//! (P09.T2.c), and their deaths are their lists' (P09.T36): the core collapses of the nurseries and
//! the young open clusters, the Type Ia of every cluster, and the centre's own after its victims.
//! The lists hold the luminous blue variables of the young features too (P09.T43), and plan 11's
//! binary classes of every feature with members (P11.T8.f), which the catalogue gives up there.
//! Every class's walk reads them beside the class's cells, under the class's one census, and in the
//! same order: their detections are merged with the cells' by arrival and then by event.
//!
//! # What is read
//!
//! A list is read for each kind the class's walk sees, each within its [`kind_reach`]:
//!
//! - [`ClassId::CORE_COLLAPSE`]: its supernovae ([`TransientKind::CoreCollapse`]) and its
//!   magnetars ([`TransientKind::MagnetarBurst`]); [`ClassId::TYPE_IA`]: its supernovae;
//! - [`ClassId::LUMINOUS_BLUE_VARIABLE`]: its giant eruptions;
//! - [`ClassId::ACCRETING_WHITE_DWARF`] and [`ClassId::ACCRETING_WHITE_DWARF_SLOW`]: their novae;
//!   [`ClassId::XRAY_BINARY`]: its transients' outbursts, its bursters' Type I bursts and its Be/X
//!   outbursts;
//! - [`ClassId::STELLAR_MERGER`] and [`ClassId::NEUTRON_STAR_MERGER`]: their luminous red novae
//!   and kilonovae.
//!
//! An entry's epoch place lies within its feature's reach of its centre, and it moves on a line at
//! the feature's bulk velocity, so an event at T whose light can reach the window lies within the
//! reach and the bulk speed times |T| of the centre; a magnetar's remnant lies within
//! [`LIFETIME_REACH`] of its site, by `DeathMarks::of`'s cap, which bounds a death's travel too.
//! So:
//!
//! - **The features.** Those whose reach touches the sphere of the kind's reach about the observer
//!   padded by that travel at [`UNBOUND_PAD_SPEED`], above every feature's bulk speed
//!   ([`FeatureCatalogue::near`], through the context's feature cells,
//!   [`AlertContext::with_feature_cells`]), and then, from the feature's own bulk speed, those
//!   whose hosts' ball can come within the reach.
//! - **The centre.** Its list, whose entries move at their own velocities: for its deaths, its
//!   ball the centre's reach padded by [`LIFETIME_REACH`], by the same cap; for its living hosts,
//!   padded by their travel at the fastest of them.
//! - **The deaths.** Each list's deaths of the class whose time, read first, can light the window
//!   from the ball (the alert walks' cut of a class cell, P12.T8.b.2, about the ball in place of a
//!   cell's box): only these build their entries. A living host reads no time first, and its own
//!   emission interval decides, as a cell's host's does.
//!
//! What a walk reads of a list that no window changes, each class's counts and its hosts, is kept
//! between walks ([`ListedHostsCache`]; [`AlertContext::with_listed_hosts`]), so a walk of the whole cube
//! builds no list another walk has built (fix-p09-alert-walk-features-2).
//!
//! # The walks
//!
//! Each host is walked as its class's own walk walks a cell's: a death's supernova is a
//! [`OneShotEvent`] of its explosion on the list's member, its event named as its
//! [`SupernovaEntry`] would name it ([`one_shot_detections`]), and a core collapse's neutron star
//! born above the critical field a [`MagnetarHost`] ([`magnetar_detections`]); a variable an
//! [`EruptionHost`], a white dwarf or an X-ray binary its scan marks, and a merger its one event
//! ([`recurrent_detections`]' and [`one_shot_detections`]' per-host steps).
//!
//! The census adds the lists' expected candidates of the class to its cells' when the cells alone
//! leave the class complete ([`class_alerts`](super::class_alerts)'s "the candidates its walk
//! draws"); a background walk counts the candidates it drew of them. A background scan walks the
//! lists of the features whose centres lie in its x-slabs of the class's sphere, the first and the
//! last slab taking those beyond, and the centre in the slab of the origin ([`listed_scan_slab`]):
//! over every slab, merged, the detections are the interactive walk's, each to the bit.
//!
//! Below the version no walk reads a list, and every walk is the cells' alone, bit for bit.
//!
//! [`one_shot_detections`]: super::one_shot_detections
//! [`magnetar_detections`]: super::magnetar_detections
//! [`recurrent_detections`]: super::recurrent_detections

use std::ops::Range;
use std::sync::Arc;

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::supernova::{LIFETIME_REACH, SupernovaEntry, SupernovaKind};
use crate::galaxy::catalogue_classes::{CatalogueMotion, ClassId, class_slabs};
use crate::galaxy::features::catalogue::{
    EDGE, FeatureCatalogue, FeatureCellCache, FeatureRecord, MAX_FEATURE_REACH, bulk_velocity,
};
use crate::galaxy::features::centre::profile::REACH;
use crate::galaxy::features::members::recent::RecentEntry;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::{QuerySphere, UNBOUND_PAD_SPEED};
use crate::id::SystemId;
use crate::stellar::Composition;
use crate::stellar::binary::scan::{AwdScanMarks, XrbScanMarks};
use crate::time::UniverseTime;
use crate::units::consts::SPEED_OF_LIGHT;
use crate::units::{LightYears, MetresPerSecond};

use super::arriving::SUPERNOVA_LIGHT_YEARS;
use super::cut;
use super::oneshot::{OneShotEvent, candidate, detect};
use super::reach::kind_reach;
use super::recurrent::{
    BinaryListing, EruptionHost, EruptionListing, EruptionWalk, MAGNETAR_ACTIVE_SPAN, MagnetarHost,
    NovaWalk, StarRecipe, emission_of, hosts_detections, longest_years, sort_detections,
    xray_binary_host_detections,
};
use super::registry::TransientKind;
use super::{AlertContext, AlertQuery, Detection, SourceStatus};

mod hosts;

use hosts::ClassHosts;
pub use hosts::{ListedCell, ListedHosts, ListedHostsCache, NoListedHostsCache, ToldEntries};

/// The relative margin on a pad, over the rounding of its products.
const PAD_MARGIN: f64 = 1e-9;

/// The absolute margin on a pad or a reach, ly.
const NEAR_MARGIN_LY: f64 = 1e-6;

/// The most kinds one class's lists are walked for: the X-ray binaries' three.
const MAX_KINDS: usize = 3;

/// How a class's hosts on the lists are walked (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Walked {
    /// Deaths: their supernovae, and a core collapse's magnetar.
    Deaths,
    /// Luminous blue variables' giant eruptions.
    Eruptions,
    /// Accreting white dwarfs' novae.
    Novae,
    /// X-ray binaries' outbursts and bursts.
    XrayBinaries,
    /// Mergers' one events.
    Mergers,
}

impl Walked {
    /// How `class`'s hosts are walked, and the kinds its lists are walked for, in the order its
    /// walk reads them; `None` for a class no catalogue feature's list holds.
    #[must_use]
    fn of(class: ClassId) -> Option<(Self, &'static [TransientKind])> {
        use TransientKind as K;
        Some(match class {
            ClassId::CORE_COLLAPSE => (Self::Deaths, &[K::CoreCollapse, K::MagnetarBurst]),
            ClassId::TYPE_IA => (Self::Deaths, &[K::TypeIa]),
            ClassId::LUMINOUS_BLUE_VARIABLE => (Self::Eruptions, &[K::GiantEruption]),
            ClassId::ACCRETING_WHITE_DWARF | ClassId::ACCRETING_WHITE_DWARF_SLOW => {
                (Self::Novae, &[K::Nova])
            }
            ClassId::XRAY_BINARY => (
                Self::XrayBinaries,
                &[K::XrayTransient, K::XrayBurst, K::BeXOutburst],
            ),
            ClassId::STELLAR_MERGER => (Self::Mergers, &[K::LuminousRedNova]),
            ClassId::NEUTRON_STAR_MERGER => (Self::Mergers, &[K::Kilonova]),
            // The victims are the centre's alone, walked through the global list.
            _ => return None,
        })
    }

    /// Whether its hosts die, so that their travel is capped at [`LIFETIME_REACH`] and their times
    /// read first.
    #[must_use]
    const fn dies(self) -> bool {
        matches!(self, Self::Deaths)
    }
}

/// How long after its onset an event of `kind` can give light that reaches a window, Julian
/// years, in [`site_pad`]'s terms: a supernova's [`SUPERNOVA_LIGHT_YEARS`], a magnetar's active
/// span and its remnant's [`LIFETIME_REACH`] of light ([`magnetar_span_years`]), and any other
/// event's longest duration, its registry row's.
#[must_use]
fn span_years(kind: TransientKind) -> f64 {
    match kind {
        TransientKind::CoreCollapse | TransientKind::TypeIa => SUPERNOVA_LIGHT_YEARS,
        TransientKind::MagnetarBurst => magnetar_span_years(),
        other => longest_years(other),
    }
}

/// How far beyond a list's ball of sites an event of `kind` can go off, ly: a magnetar's remnant
/// within [`LIFETIME_REACH`] of its site; nothing for any other.
#[must_use]
fn beyond_ly(kind: TransientKind) -> f64 {
    if kind == TransientKind::MagnetarBurst {
        LIFETIME_REACH.value()
    } else {
        0.0
    }
}

/// The kinds a class's lists are walked for, each with its reach for a query's sensor (module
/// documentation), in the class's order, those the sensor does not see left out.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Reaches {
    walked: Walked,
    kinds: [Option<(TransientKind, LightYears)>; MAX_KINDS],
}

impl Reaches {
    /// The kinds `class`'s lists are walked for, for `query`; `None` for a class the query does not
    /// ask about, one no list holds, or one none of whose kinds the sensor sees.
    #[must_use]
    fn of(class: ClassId, query: &AlertQuery) -> Option<Self> {
        if !query.classes().contains(class) {
            return None;
        }
        let (walked, all) = Walked::of(class)?;
        let mut kinds = [None; MAX_KINDS];
        for (slot, &kind) in kinds.iter_mut().zip(all) {
            let reach = kind_reach(kind, query.sensor());
            *slot = (reach > LightYears::ZERO).then_some((kind, reach));
        }
        kinds
            .iter()
            .any(Option::is_some)
            .then_some(Self { walked, kinds })
    }

    /// The kinds seen, with their reaches.
    fn seen(&self) -> impl Iterator<Item = (TransientKind, LightYears)> + '_ {
        self.kinds.iter().flatten().copied()
    }

    /// The reach of `kind`, if it is walked and seen.
    #[must_use]
    fn reach_of(&self, kind: TransientKind) -> Option<LightYears> {
        self.seen()
            .find(|&(k, _)| k == kind)
            .map(|(_, reach)| reach)
    }

    /// The pad a list of reach `reach_ly` about a centre `distance_ly` from the observer, moving at
    /// `beta` of light's speed, takes for an event lighting within `span_years` ([`site_pad`]),
    /// capped at [`LIFETIME_REACH`] for a class whose hosts die.
    #[must_use]
    fn pad(
        &self,
        query: &AlertQuery,
        (span_years, distance_ly): (f64, f64),
        (reach_ly, beta): (f64, f64),
    ) -> f64 {
        site_pad(
            query,
            span_years,
            distance_ly,
            (reach_ly, beta),
            self.walked,
        )
    }

    /// The radius about the observer within which every catalogue feature whose list can light the
    /// window lies within its own reach ([`FeatureCatalogue::near`]'s test), ly: each kind's reach
    /// with [`Balls::of`]'s margins and what lies beyond its sites ([`beyond_ly`]), and the pad of a
    /// feature at [`UNBOUND_PAD_SPEED`] as far off as the reach allows ([`lookup_pad`]).
    #[must_use]
    fn lookup_radius(&self, query: &AlertQuery) -> LightYears {
        let beta = MetresPerSecond::from(UNBOUND_PAD_SPEED).value() / SPEED_OF_LIGHT;
        let reach = MAX_FEATURE_REACH.value();
        let seen = |r: LightYears| r.value() * (1.0 + PAD_MARGIN) + NEAR_MARGIN_LY;
        let radius = self.seen().fold(0.0, |far: f64, (kind, r)| {
            let r = seen(r) + beyond_ly(kind);
            far.max(r + lookup_pad(query, span_years(kind), (r, reach), beta, self.walked))
        });
        LightYears::new(radius)
    }
}

/// The span after its death over which a magnetar's light can reach a window, Julian years, in
/// [`site_pad`]'s terms: its active span and its remnant's [`LIFETIME_REACH`] of light.
#[must_use]
fn magnetar_span_years() -> f64 {
    MAGNETAR_ACTIVE_SPAN.value() + LIFETIME_REACH.value()
}

/// The latest of `query`'s window's ends from the epoch, |t|, Julian years.
#[must_use]
fn latest_years(query: &AlertQuery) -> f64 {
    let window = query.window();
    let years = |t: UniverseTime| t.since_epoch().as_julian_years_f64().abs();
    years(window.start()).max(years(window.end()))
}

/// How far beyond its reach `reach_ly` from its centre a host of a list `distance_ly` from the
/// observer may lie when the light of its event can reach `query`'s window, ly, its feature moving
/// at `beta` of light's speed: its feature's travel from the epoch to the event. Light covers a
/// light-year a Julian year, so years and light-years add.
///
/// An event at T whose light can reach the window within `span_years` of it lies at most
/// distance + reach + β|T| from the observer, so |T| ≤ (|t|max + span + distance + reach + 1)
/// ÷ (1 − β), and its host within reach + β|T| of the centre; a year more covers the walks' day
/// margins. For hosts that die, walked as `walked`, never more than [`LIFETIME_REACH`], the cap's
/// bound on any line's travel from its site over its entry's lifetime.
#[must_use]
fn site_pad(
    query: &AlertQuery,
    span_years: f64,
    distance_ly: f64,
    (reach_ly, beta): (f64, f64),
    walked: Walked,
) -> f64 {
    let k = pad_rate(beta);
    let pad =
        k * (latest_years(query) + span_years + distance_ly + reach_ly + 1.0) + NEAR_MARGIN_LY;
    capped(pad, walked)
}

/// `pad` for hosts walked as `walked`: at most [`LIFETIME_REACH`] for hosts that die.
#[must_use]
fn capped(pad: f64, walked: Walked) -> f64 {
    if walked.dies() {
        pad.min(LIFETIME_REACH.value())
    } else {
        pad
    }
}

/// The pad a list's site takes a light-year of its bound on |T|, ly: β ÷ (1 − β) with a relative
/// margin over the rounding.
#[must_use]
fn pad_rate(beta: f64) -> f64 {
    beta / (1.0 - beta) * (1.0 + PAD_MARGIN)
}

/// A bound on [`site_pad`] of any list a lookup of `radius_ly` keeps whose ball comes within it,
/// ly, for lists of reach at most `reach_ly` whose hosts are walked as `walked`.
///
/// Such a list lies at most radius + reach + its pad P from the observer, and P = k (|t|max +
/// span + its distance + reach + 1) + ε for k = [`pad_rate`], so P ≤ k (|t|max + span + radius +
/// 2 reach + 1 + P) + ε, which the fixed point (k (…) + ε) ÷ (1 − k) bounds; a margin more covers
/// its own rounding.
#[must_use]
fn lookup_pad(
    query: &AlertQuery,
    span_years: f64,
    (radius_ly, reach_ly): (f64, f64),
    beta: f64,
    walked: Walked,
) -> f64 {
    let k = pad_rate(beta);
    let years = latest_years(query) + span_years + radius_ly + 2.0 * reach_ly + 1.0;
    let fixed = (k * years + NEAR_MARGIN_LY) / (1.0 - k);
    capped(fixed * (1.0 + PAD_MARGIN) + NEAR_MARGIN_LY, walked)
}

/// One list's balls for a walk: about its centre, the radius that holds every place of its hosts
/// from which an event of each kind walked can light the window, ly (module documentation), where
/// it comes within its kind's reach of the observer.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Balls {
    centre: GalacticPosition,
    /// Each kind's ball, in [`Reaches`]' order, where it comes within its reach.
    kinds: [Option<(TransientKind, LightYears)>; MAX_KINDS],
}

impl Balls {
    /// The balls of a list about `centre` whose entries lie within `reach_ly` of it at the epoch,
    /// its hosts within `pad(span_years, distance_ly)` more, ly, for `query` and `reaches`.
    #[must_use]
    fn of(
        query: &AlertQuery,
        reaches: &Reaches,
        (centre, reach_ly): (GalacticPosition, f64),
        pad: impl Fn(f64, f64) -> f64,
    ) -> Self {
        let distance = LightYears::from(query.observer_position().distance_to(&centre)).value();
        let within = |radius: f64, extra: f64, seen: LightYears| {
            (distance - radius - extra <= seen.value() * (1.0 + PAD_MARGIN) + NEAR_MARGIN_LY)
                .then_some(LightYears::new(radius))
        };
        let mut kinds = [None; MAX_KINDS];
        for (slot, reach) in kinds.iter_mut().zip(&reaches.kinds) {
            *slot = reach.and_then(|(kind, seen)| {
                let radius = reach_ly + pad(span_years(kind), distance);
                within(radius, beyond_ly(kind), seen).map(|radius| (kind, radius))
            });
        }
        Self { centre, kinds }
    }

    /// The balls of `feature`'s list, or `None` for a feature with no bulk motion, which lists
    /// nothing (a cloud, or a galaxy without its kinematic tables), or whose hosts reach no kind's
    /// sphere.
    #[must_use]
    fn of_feature(
        galaxy: &Galaxy,
        query: &AlertQuery,
        reaches: &Reaches,
        feature: &FeatureRecord,
    ) -> Option<Self> {
        let speed = bulk_velocity(galaxy, feature)?.speed();
        debug_assert!(
            speed < MetresPerSecond::from(UNBOUND_PAD_SPEED),
            "a feature's bulk speed of {} m/s passes the lists' lookup's pad speed",
            speed.value()
        );
        let beta = speed.value() / SPEED_OF_LIGHT;
        let reach = feature.reach().value();
        Self::of(query, reaches, (*feature.position(), reach), |span, d| {
            reaches.pad(query, (span, d), (reach, beta))
        })
        .reaching()
    }

    /// The balls of the galactic centre's list, whose entries move at their own velocities: its
    /// [`REACH`], and for its deaths the cap's [`LIFETIME_REACH`], for its living hosts their travel
    /// at `beta`, the fastest's speed over light's ([`site_pad`]).
    #[must_use]
    fn of_centre(query: &AlertQuery, reaches: &Reaches, beta: f64) -> Option<Self> {
        let reach = REACH.value();
        Self::of(
            query,
            reaches,
            (GalacticPosition::ORIGIN, reach),
            |span, d| {
                if reaches.walked.dies() {
                    LIFETIME_REACH.value()
                } else {
                    reaches.pad(query, (span, d), (reach, beta))
                }
            },
        )
        .reaching()
    }

    /// The balls, if any comes within its kind's reach.
    #[must_use]
    fn reaching(self) -> Option<Self> {
        self.kinds.iter().any(Option::is_some).then_some(self)
    }

    /// Whether a death at `t` can light `query`'s window from a ball (the cut): a supernova's, or a
    /// magnetar's.
    fn wanted(&self, query: &AlertQuery) -> impl Fn(UniverseTime) -> bool {
        let ball = |wanted: TransientKind| {
            self.kinds
                .iter()
                .flatten()
                .find(|&&(kind, _)| kind == wanted)
                .map(|&(_, radius)| radius)
        };
        let supernovae = ball(TransientKind::CoreCollapse)
            .or_else(|| ball(TransientKind::TypeIa))
            .map(|radius| cut::one_shot_ball(query, &self.centre, radius));
        let magnetars = ball(TransientKind::MagnetarBurst)
            .map(|radius| cut::magnetars_ball(query, &self.centre, radius));
        move |t| {
            supernovae.as_ref().is_some_and(|cut| cut(t))
                || magnetars.as_ref().is_some_and(|cut| cut(t))
        }
    }
}

/// A death on a list, as the walks read it: its member, what exploded and when, how it moves, and
/// a core collapse's magnetar.
#[derive(Debug, Clone, PartialEq)]
struct Death {
    host: SystemId,
    kind: SupernovaKind,
    explosion: UniverseTime,
    motion: CatalogueMotion,
    magnetar: Option<MagnetarHost>,
}

impl Death {
    /// The death `entry` of a feature's list, or `None` for an entry that is none.
    #[must_use]
    fn of_feature(galaxy: &Galaxy, entry: &RecentEntry) -> Option<Self> {
        let record = entry.member().record();
        let composition = *entry.member().composition();
        Some(Self::of_parts(
            galaxy,
            (record, composition),
            entry.explosion_kind()?,
            entry.site()?.death(),
            (*entry.motion(), entry.supernova()),
        ))
    }

    /// The death of `record`'s primary at `composition`, of `kind` at `explosion`, moving by
    /// `motion`, its shell's entry `supernova` for a core collapse: its magnetar where its neutron
    /// star is born above the critical field, as the class's [`MagnetarClass`] finds one.
    ///
    /// [`MagnetarClass`]: super::MagnetarClass
    #[must_use]
    fn of_parts(
        galaxy: &Galaxy,
        (record, composition): (&SystemRecord, Composition),
        kind: SupernovaKind,
        explosion: UniverseTime,
        (motion, supernova): (CatalogueMotion, Option<&SupernovaEntry>),
    ) -> Self {
        // As the class's magnetars: a field at or below the critical field is no magnetar's, and a
        // list's motions are below light's speed, so nothing else is refused.
        let magnetar = supernova
            .and_then(|supernova| supernova.pulsar().map(|pulsar| (supernova, pulsar)))
            .and_then(|(supernova, pulsar)| {
                MagnetarHost::new(
                    record.id(),
                    supernova.explosion(),
                    pulsar.birth_field(),
                    motion,
                    StarRecipe::of_record(galaxy, record, composition),
                )
                .ok()
            });
        Self {
            host: record.id(),
            kind,
            explosion,
            motion,
            magnetar,
        }
    }
}

/// What a walk read of a class's lists (module documentation): the lists' expected candidates of
/// the class and the candidates they drew, and their hosts that can light the window, in the lists'
/// order (the catalogue's cell and index order, the centre's last) and each list's index order.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ListedWalk {
    expected: f64,
    drawn: u64,
    /// The deaths whose time can light the window.
    deaths: Vec<Death>,
    /// The luminous blue variables.
    eruptions: Vec<EruptionHost>,
    /// The accreting white dwarfs.
    novae: Vec<AwdScanMarks>,
    /// The X-ray binaries.
    xray_binaries: Vec<XrbScanMarks>,
    /// The mergers' events.
    mergers: Vec<OneShotEvent>,
}

impl ListedWalk {
    /// The lists of `class` that `query`'s walk reads in `galaxy`, their features through `cells`
    /// and the lists as the walks read them through `lists` (module documentation), or `None` for a
    /// class no list holds or whose kinds the sensor does not see.
    ///
    /// # Panics
    ///
    /// If the galaxy's centre has no model or no disruptions' model, which no galaxy the builder
    /// allows reaches, and as the lists' entries do.
    #[must_use]
    pub(crate) fn find(
        galaxy: &Galaxy,
        (cells, lists): (&dyn FeatureCellCache, &dyn ListedHostsCache),
        query: &AlertQuery,
        class: ClassId,
    ) -> Option<Self> {
        let reaches = Reaches::of(class, query)?;
        let mut walk = Self::default();
        let observer = query.observer_position();
        let radius = reaches.lookup_radius(query);
        let mut found = Vec::new();
        for (cell, last) in FeatureCatalogue::cells_near(galaxy, observer, radius) {
            found.clear();
            FeatureCatalogue::found_in(galaxy, cells, (cell, last), (observer, radius), &mut found);
            walk.read_cell(galaxy, (cells, lists), (query, &reaches, class), &found);
        }
        walk.read_centre(galaxy, lists, (query, &reaches, class));
        Some(walk)
    }

    /// The lists of `class` whose features' centres lie in the x-slab `x` of `sphere` that
    /// `query`'s background walk reads in `galaxy` ([`listed_scan_slab`]).
    #[must_use]
    fn find_in_slab(
        galaxy: &Galaxy,
        (cells, lists): (&dyn FeatureCellCache, &dyn ListedHostsCache),
        query: &AlertQuery,
        (class, sphere, x): (ClassId, &QuerySphere, i32),
    ) -> Self {
        let mut walk = Self::default();
        let slabs = class_slabs(class, sphere);
        if !slabs.contains(&x) {
            return walk;
        }
        let Some(reaches) = Reaches::of(class, query) else {
            return walk;
        };
        let log2 = class
            .cell_log2_ly()
            .expect("a class whose lists are walked has a grid");
        let edge = f64::from(1_u32 << log2);
        let (lo, hi) = slab_bounds(&slabs, x, edge);
        let observer = query.observer_position();
        let radius = reaches.lookup_radius(query);
        let mut cells_near = FeatureCatalogue::cells_near(galaxy, observer, radius);
        // Closed at both faces: a centre's x in light-years may round onto its cell's face.
        cells_near.retain(|(cell, _)| {
            let low = f64::from(cell.to_array()[0]) * EDGE;
            low <= hi && low + EDGE >= lo
        });
        // The slabs of one column of feature cells, which a background scan's jobs walk side by
        // side, each start on its own cells, so that each builds its own lists first rather than
        // waiting on another's (`ListedHostsCache`); the detections are sorted whatever the order.
        // A feature cell, `EDGE` = 4,096 ly, spans 2^(12 − log2) slabs of the class's cells.
        let per_column = 1_i32 << 12_u32.saturating_sub(log2);
        debug_assert_eq!(f64::from(per_column) * edge, EDGE.max(edge));
        let place = usize::try_from(x.rem_euclid(per_column)).expect("a remainder is not negative");
        let per_column = usize::try_from(per_column).expect("a column's slabs are few");
        let start = cells_near.len() * place / per_column;
        cells_near.rotate_left(start);
        let mut found = Vec::new();
        for (cell, last) in cells_near {
            found.clear();
            FeatureCatalogue::found_in(galaxy, cells, (cell, last), (observer, radius), &mut found);
            found.retain(|feature| (lo..hi).contains(&feature.position().to_light_years_f64()[0]));
            walk.read_cell(galaxy, (cells, lists), (query, &reaches, class), &found);
        }
        if (lo..hi).contains(&0.0) {
            walk.read_centre(galaxy, lists, (query, &reaches, class));
        }
        walk
    }

    /// Reads the lists of `found`, features of one cell in index order, whose hosts can reach a
    /// kind's sphere: the cell's lists found through `lists` when the first is needed.
    fn read_cell(
        &mut self,
        galaxy: &Galaxy,
        (cells, lists): (&dyn FeatureCellCache, &dyn ListedHostsCache),
        (query, reaches, class): (&AlertQuery, &Reaches, ClassId),
        found: &[FeatureRecord],
    ) {
        let mut listed: Option<Arc<ListedCell>> = None;
        for feature in found {
            let Some(balls) = Balls::of_feature(galaxy, query, reaches, feature) else {
                continue;
            };
            let cell = listed.get_or_insert_with(|| lists.cell(galaxy, feature.id().cell(), cells));
            let Some(hosts) = cell.hosts(feature) else {
                continue;
            };
            let read = (feature, &balls);
            self.read_feature(galaxy, lists, (query, reaches, class), read, hosts);
        }
    }

    /// Reads `feature`'s list, `hosts`, whose hosts can reach a kind's sphere from `balls`, its
    /// told deaths' entries through `lists`.
    fn read_feature(
        &mut self,
        galaxy: &Galaxy,
        lists: &dyn ListedHostsCache,
        (query, reaches, class): (&AlertQuery, &Reaches, ClassId),
        (feature, balls): (&FeatureRecord, &Balls),
        hosts: &ListedHosts,
    ) {
        let (expected, count) = hosts.class(class);
        self.expected += expected;
        self.drawn += u64::from(count);
        let Some(found) = hosts.feature_hosts(galaxy, feature, class) else {
            return;
        };
        if let ClassHosts::Told(told) = found {
            let wanted = balls.wanted(query);
            if !told.iter().any(|told| wanted(told.death)) {
                return;
            }
            // Every told death's entry, built at once and kept apart from the list, taken by its
            // told time as the cut takes its told death.
            let built = lists.told_entries(galaxy, feature.id(), &mut || {
                ToldEntries::of(galaxy, feature, told)
            });
            self.deaths.extend(
                built
                    .entries()
                    .iter()
                    .filter(|&&(death, _)| wanted(death))
                    .map(|(_, entry)| entry.clone()),
            );
            return;
        }
        self.take(query, balls, reaches, found);
    }

    /// Takes the hosts `found` of a list whose balls are `balls`: its deaths that the cut takes, or
    /// every living host.
    fn take(&mut self, query: &AlertQuery, balls: &Balls, reaches: &Reaches, found: &ClassHosts) {
        debug_assert!(
            matches!(
                (reaches.walked, found),
                (Walked::Deaths, ClassHosts::Deaths(_))
                    | (Walked::Eruptions, ClassHosts::Eruptions(_))
                    | (Walked::Novae, ClassHosts::Novae(_))
                    | (Walked::XrayBinaries, ClassHosts::XrayBinaries(_))
                    | (Walked::Mergers, ClassHosts::Mergers(_))
            ),
            "a class's hosts are walked as its class walks them"
        );
        match found {
            ClassHosts::Told(_) => {}
            ClassHosts::Deaths(deaths) => {
                let wanted = balls.wanted(query);
                self.deaths.extend(
                    deaths
                        .iter()
                        .filter(|death| wanted(death.explosion))
                        .cloned(),
                );
            }
            ClassHosts::Eruptions(hosts) => self.eruptions.extend(hosts.iter().cloned()),
            ClassHosts::Novae(marks) => self.novae.extend_from_slice(marks),
            ClassHosts::XrayBinaries(marks) => self.xray_binaries.extend_from_slice(marks),
            ClassHosts::Mergers(events) => self.mergers.extend_from_slice(events),
        }
    }

    /// Reads the galactic centre's list through `lists`, if its hosts can reach a kind's sphere.
    ///
    /// # Panics
    ///
    /// As [`find`](Self::find).
    fn read_centre(
        &mut self,
        galaxy: &Galaxy,
        lists: &dyn ListedHostsCache,
        (query, reaches, class): (&AlertQuery, &Reaches, ClassId),
    ) {
        // Its deaths' ball is the cap's, known before the list is read; its living hosts' needs the
        // fastest of them.
        let (centre, balls) = if reaches.walked.dies() {
            let Some(balls) = Balls::of_centre(query, reaches, 0.0) else {
                return;
            };
            (lists.centre(galaxy), balls)
        } else {
            let centre = lists.centre(galaxy);
            let beta = centre.centre_fastest(galaxy, class);
            let Some(balls) = Balls::of_centre(query, reaches, beta) else {
                return;
            };
            (centre, balls)
        };
        let (expected, count) = centre.class(class);
        self.expected += expected;
        self.drawn += u64::from(count);
        if let Some(found) = centre.centre_hosts(galaxy, class) {
            self.take(query, &balls, reaches, found);
        }
    }

    /// The status of the catalogue features' lists' source (the census's
    /// [`AlertSource::FeatureLists`](super::AlertSource::FeatureLists)): walked with their classes,
    /// every class they hold.
    #[must_use]
    pub(crate) const fn source_status() -> SourceStatus {
        SourceStatus::Complete
    }

    /// The expected candidates of the class on the lists read: what the census adds to its cells'.
    #[must_use]
    pub(crate) const fn expected(&self) -> f64 {
        self.expected
    }

    /// The candidates the lists read drew of the class: what a background walk adds to its cells'.
    #[must_use]
    pub(crate) const fn drawn(&self) -> u64 {
        self.drawn
    }

    /// Appends to `out` the detections of the hosts of `class` read that `query`'s sensor sees in
    /// its window, sorted by arrival and then by event (module documentation).
    pub(crate) fn detections(
        &self,
        galaxy: &Galaxy,
        ctx: &mut AlertContext<'_>,
        query: &AlertQuery,
        class: ClassId,
        out: &mut Vec<Detection>,
    ) {
        let start = out.len();
        let Some(reaches) = Reaches::of(class, query) else {
            return;
        };
        let observer = *query.observer_position();
        match reaches.walked {
            Walked::Deaths => {
                let supernova = [TransientKind::CoreCollapse, TransientKind::TypeIa]
                    .into_iter()
                    .find(|&kind| reaches.reach_of(kind).is_some());
                if let Some(kind) = supernova {
                    let candidates = self
                        .deaths
                        .iter()
                        .map(|death| {
                            OneShotEvent::of_explosion(
                                death.host,
                                death.explosion,
                                death.kind,
                                death.motion,
                            )
                        })
                        .filter_map(|event| candidate(&event, &observer, query));
                    for candidate in candidates {
                        if let Some(detection) = detect(galaxy, ctx, query, class, kind, &candidate)
                        {
                            out.push(detection);
                        }
                    }
                }
                if reaches.reach_of(TransientKind::MagnetarBurst).is_some() {
                    let active = self
                        .deaths
                        .iter()
                        .filter_map(|death| death.magnetar.as_ref())
                        .filter_map(|host| {
                            emission_of(host, &observer, query)
                                .map(|interval| (host.clone(), interval))
                        })
                        .collect();
                    hosts_detections(galaxy, ctx, query, class, active, out);
                }
            }
            Walked::Eruptions => {
                let walk = EruptionWalk::new(class, query);
                let mut listing = EruptionListing::default();
                for host in &self.eruptions {
                    walk.host(galaxy, ctx, query, host, &mut listing, out);
                }
            }
            Walked::Novae => {
                let walk = NovaWalk::new(class, query);
                let mut listing = BinaryListing::default();
                for marks in &self.novae {
                    walk.host(galaxy, ctx, query, marks, &mut listing, out);
                }
            }
            Walked::XrayBinaries => {
                let mut listing = BinaryListing::default();
                for marks in &self.xray_binaries {
                    xray_binary_host_detections(galaxy, ctx, query, marks, &mut listing, out);
                }
            }
            Walked::Mergers => {
                if let Some((kind, _)) = reaches.seen().next() {
                    for candidate in self
                        .mergers
                        .iter()
                        .filter_map(|event| candidate(event, &observer, query))
                    {
                        if let Some(detection) = detect(galaxy, ctx, query, class, kind, &candidate)
                        {
                            out.push(detection);
                        }
                    }
                }
            }
        }
        sort_detections(&mut out[start..]);
    }
}

/// The x-range, ly, of the features whose lists the slab `x` of `slabs` walks: its cells' `edge`
/// wide, the first slab's open below and the last's above.
#[must_use]
fn slab_bounds(slabs: &Range<i32>, x: i32, edge: f64) -> (f64, f64) {
    let lo = if x == slabs.start {
        f64::NEG_INFINITY
    } else {
        f64::from(x) * edge
    };
    let hi = if x + 1 == slabs.end {
        f64::INFINITY
    } else {
        f64::from(x + 1) * edge
    };
    (lo, hi)
}

/// Appends to `out` the detections of the hosts of `class` on the lists of the features whose
/// centres lie in the x-slab `x` of `sphere`, sorted by arrival and then by event, and returns the
/// candidates of the class those lists drew (module documentation).
///
/// `sphere` is the class's sphere of a background walk for `query`, whose slabs are
/// [`class_slabs`]'s: the first slab takes the features below it, the last those above, and the
/// slab of the origin the galactic centre's list. A pure function of the slab, which a background
/// scan runs beside its cells: over every slab of `sphere`, merged by arrival and then by event,
/// the detections are those the interactive walk adds to the class's cells'
/// ([`alerts_in`](super::alerts_in)), each to the bit, in any order and with any cache. Nothing
/// for a class no list holds, a slab outside the sphere's, or a class the query does not ask
/// about. The features are read through the
/// context's feature cells ([`AlertContext::with_feature_cells`]) and their lists through its
/// listed hosts ([`AlertContext::with_listed_hosts`]).
///
/// # Panics
///
/// As the interactive walk's lists: if the galaxy's centre has no model or no disruptions' model,
/// which no galaxy the builder allows reaches, and as the lists' entries do.
///
/// # Examples
///
/// A background scan of the core collapses that reach the whole galaxy from near the Sun walks
/// the lists slab by slab, as it walks the class's cells, and merges them:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::{
///     AlertBand, AlertContext, AlertQuery, BandLimits, ClassSet, SensorHorizon, class_reach,
///     listed_scan_slab,
/// };
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::{ClassId, class_slabs};
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::query::QuerySphere;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{LightYears, WattsPerSquareMetre};
///
/// let galaxy = Galaxy::new(Seed::new(9)).with_full_potential();
/// let w = WattsPerSquareMetre::new;
/// let v = BandLimits::new(w(1e-12), w(1e-10))?;
/// let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), v)])?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in the cube")?;
/// let year = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(1).ok_or("on the clock")?)?;
/// let class = ClassId::CORE_COLLAPSE;
/// let query = AlertQuery::builder(sun, year, sensor).classes(ClassSet::of(class)).build()?;
/// let sphere = QuerySphere::new(sun, class_reach(class, query.sensor()), year.start(), LightYears::ZERO)?;
/// let mut noise = NoiseCache::with_capacity(4_096);
/// let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
/// let mut seen = Vec::new();
/// let mut drawn = 0;
/// for x in class_slabs(class, &sphere) {
///     drawn += listed_scan_slab(&galaxy, &mut ctx, &query, (class, &sphere), x, &mut seen);
/// }
/// seen.sort_by(|a, b| a.arrival().cmp(&b.arrival()).then_with(|| a.event().cmp(&b.event())));
/// // At generator version 27 no walk reads a list yet.
/// assert_eq!((seen.len(), drawn), (0, 0));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn listed_scan_slab(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    (class, sphere): (ClassId, &QuerySphere),
    x: i32,
    out: &mut Vec<Detection>,
) -> u64 {
    if Walked::of(class).is_none() {
        return 0;
    }
    let walk = ListedWalk::find_in_slab(
        galaxy,
        (ctx.feature_cells(), ctx.listed_hosts()),
        query,
        (class, sphere, x),
    );
    walk.detections(galaxy, ctx, query, class, out);
    walk.drawn()
}

#[cfg(test)]
mod tests;
