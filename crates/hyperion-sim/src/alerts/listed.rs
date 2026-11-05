//! The recently dead of the catalogue features' and the galactic centre's feature-level lists in
//! the alert walks (plan 09's P09.T36 in plan 12's walks), from generator version 29
//! (`LISTED_ALERTS_VERSION`).
//!
//! From version 27 the young field gives most of its young stars to the features that bore them
//! (P09.T2.c), and their deaths are their lists' (P09.T36): the core collapses of the nurseries and
//! the young open clusters, the Type Ia of every cluster, and the centre's own after its victims.
//! The walks of [`ClassId::CORE_COLLAPSE`] and [`ClassId::TYPE_IA`] read them beside the class's
//! cells, under the class's one census, and in the same order: their detections are merged with
//! the cells' by arrival and then by event.
//!
//! # What is read
//!
//! A list is read for each kind the class's walk sees: a core collapse's supernova
//! ([`TransientKind::CoreCollapse`]) and its magnetar ([`TransientKind::MagnetarBurst`]), a Type
//! Ia's supernova ([`TransientKind::TypeIa`]), each within its [`kind_reach`]. An entry's epoch
//! place lies within its feature's reach of its centre, and its site on its candidate's line at the
//! feature's bulk velocity, so a death that can light the window at T lies within the reach and
//! the bulk speed times |T| of the centre; a magnetar's remnant lies within [`LIFETIME_REACH`] of
//! its site, by `DeathMarks::of`'s cap. So:
//!
//! - **The features.** Those whose reach touches the sphere of the kind's reach about the observer
//!   padded by that travel at [`UNBOUND_PAD_SPEED`], above every feature's bulk speed
//!   ([`FeatureCatalogue::near`], through the context's feature cells,
//!   [`AlertContext::with_feature_cells`]), and then, from the feature's own bulk speed, those
//!   whose deaths' ball can come within the reach. Only these build their lists.
//! - **The centre.** Its list (`CentreRecent`), whose entries move at their own velocities, its
//!   ball the centre's reach padded by [`LIFETIME_REACH`], by the same cap.
//! - **The deaths.** Each list's entries of the class whose death's time, read first, can light the
//!   window from the ball (the alert walks' cut of a class cell, P12.T8.b.2, about the ball in place of a
//!   cell's box): only these build their stars (`FeatureLevelList::recent_entry_when`).
//!
//! # The walks
//!
//! A death's supernova is a [`OneShotEvent`] of its explosion on the list's member, its event
//! named as its [`SupernovaEntry`] would name it, and is detected as the class's own walk detects
//! its entries' ([`one_shot_detections`]); a core collapse's neutron star born above the critical
//! field is a [`MagnetarHost`], walked as the class's magnetars are ([`magnetar_detections`]).
//!
//! The census adds the lists' expected candidates of the class to its cells' when the cells alone
//! leave the class complete ([`class_alerts`](super::class_alerts)'s "the candidates its walk
//! draws"); a background walk counts the candidates it drew of them. A background scan walks the
//! lists of the features whose centres lie in its x-slabs of the class's sphere, the first and the
//! last slab taking those beyond, and the centre in the slab of the origin ([`listed_scan_slab`]):
//! over every slab, merged, the detections are the interactive walk's, each to the bit.
//!
//! Below the version no walk reads a list, and every walk is the cells' alone, bit for bit. The
//! lists' luminous blue variables (P09.T43) and plan 11's binary classes on them (P11.T8.f) are not
//! walked yet ([`AlertSource::FeatureLists`](super::AlertSource::FeatureLists)).
//!
//! [`one_shot_detections`]: super::one_shot_detections
//! [`magnetar_detections`]: super::magnetar_detections

use std::ops::Range;

use crate::GeneratorVersion;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::supernova::{LIFETIME_REACH, SupernovaEntry, SupernovaKind};
use crate::galaxy::catalogue_classes::{CatalogueMotion, ClassId, class_slabs};
use crate::galaxy::features::catalogue::{
    EDGE, FeatureCatalogue, FeatureCellCache, FeatureRecord, MAX_FEATURE_REACH, bulk_velocity,
};
use crate::galaxy::features::centre::events::victims::victim_count;
use crate::galaxy::features::centre::profile::REACH;
use crate::galaxy::features::centre::recent::{CentreRecent, TracerProfiles};
use crate::galaxy::features::members::FeatureLevelList;
use crate::galaxy::features::members::recent::{RecentClass, RecentEntry};
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::{QuerySphere, UNBOUND_PAD_SPEED};
use crate::id::SystemId;
use crate::stellar::Composition;
use crate::time::UniverseTime;
use crate::units::consts::SPEED_OF_LIGHT;
use crate::units::{LightYears, MetresPerSecond};

use super::arriving::SUPERNOVA_LIGHT_YEARS;
use super::cut;
use super::oneshot::{OneShotEvent, candidate, detect};
use super::reach::kind_reach;
use super::recurrent::{
    MAGNETAR_ACTIVE_SPAN, MagnetarHost, StarRecipe, emission_of, hosts_detections, sort_detections,
};
use super::registry::TransientKind;
use super::{AlertContext, AlertQuery, Detection};

/// The generator version from which the alert walks read the recently dead of the features' and
/// the galactic centre's lists beside the classes' cells (module documentation): 29, train 27's
/// finding that from 27 the walks see the field's deaths alone (fix-p09-alert-walk-features).
///
/// Below it `ListedAlertsRules::of_version` is `ListedAlertsRules::FieldAlone`, and every walk is
/// version 27's bit for bit. 29's bump deletes this hold, `ListedAlertsRules` with its `FieldAlone`
/// arm, `GalaxyRules::listed_alerts`, the `match` in `ListedWalk::rules_read`, the held arm of
/// `ListedWalk::unwalked_by` and its tripwire (`the_listed_alerts_are_held_only_until_the_29_bump`).
pub(crate) const LISTED_ALERTS_VERSION: GeneratorVersion = GeneratorVersion::new(29);

/// Whether the alert walks read the lists' recently dead (its hold, [`LISTED_ALERTS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ListedAlertsRules {
    /// Version 28's and before: the classes' cells alone.
    FieldAlone,
    /// Version 29's: the lists' recently dead beside them.
    Lists,
}

impl ListedAlertsRules {
    /// The arm at `version`: [`Lists`](Self::Lists) from [`LISTED_ALERTS_VERSION`], the held arm
    /// below it.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        if version.get() >= LISTED_ALERTS_VERSION.get() {
            Self::Lists
        } else {
            Self::FieldAlone
        }
    }
}

/// The relative margin on a pad, over the rounding of its products.
const PAD_MARGIN: f64 = 1e-9;

/// The absolute margin on a pad or a reach, ly.
const NEAR_MARGIN_LY: f64 = 1e-6;

/// The kinds a class's lists are walked for, each with its reach for a query's sensor (module
/// documentation): none for a class with no recent deaths, or one the query does not ask about.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Reaches {
    /// The supernovae's kind and reach, if the sensor sees them.
    supernovae: Option<(TransientKind, LightYears)>,
    /// The magnetars' reach, for the core collapses, if the sensor sees them.
    magnetars: Option<LightYears>,
}

impl Reaches {
    /// The kinds `class`'s lists are walked for, for `query`.
    #[must_use]
    fn of(class: ClassId, query: &AlertQuery) -> Self {
        let seen = |kind| {
            let reach = kind_reach(kind, query.sensor());
            (reach > LightYears::ZERO).then_some(reach)
        };
        let none = Self {
            supernovae: None,
            magnetars: None,
        };
        if !query.classes().contains(class) {
            return none;
        }
        match RecentClass::of(class) {
            Some(RecentClass::CoreCollapse) => Self {
                supernovae: seen(TransientKind::CoreCollapse)
                    .map(|reach| (TransientKind::CoreCollapse, reach)),
                magnetars: seen(TransientKind::MagnetarBurst),
            },
            Some(RecentClass::TypeIa) => Self {
                supernovae: seen(TransientKind::TypeIa).map(|reach| (TransientKind::TypeIa, reach)),
                magnetars: None,
            },
            // The lists' variables' eruptions are not walked yet (module documentation).
            Some(RecentClass::LuminousBlueVariable) | None => none,
        }
    }

    /// Whether no kind is walked.
    #[must_use]
    const fn is_empty(&self) -> bool {
        self.supernovae.is_none() && self.magnetars.is_none()
    }

    /// The radius about the observer within which every catalogue feature whose list can light the
    /// window lies within its own reach ([`FeatureCatalogue::near`]'s test), ly: each kind's reach
    /// with [`Balls::of`]'s margins, a magnetar's remnant's [`LIFETIME_REACH`], and the pad of a
    /// feature at [`UNBOUND_PAD_SPEED`] as far off as the reach allows ([`lookup_pad`]).
    #[must_use]
    fn lookup_radius(&self, query: &AlertQuery) -> LightYears {
        let beta = MetresPerSecond::from(UNBOUND_PAD_SPEED).value() / SPEED_OF_LIGHT;
        let reach = MAX_FEATURE_REACH.value();
        let seen = |r: LightYears| r.value() * (1.0 + PAD_MARGIN) + NEAR_MARGIN_LY;
        let supernovae = self.supernovae.map_or(0.0, |(_, r)| {
            seen(r) + lookup_pad(query, SUPERNOVA_LIGHT_YEARS, seen(r), reach, beta)
        });
        let magnetars = self.magnetars.map_or(0.0, |r| {
            let extra = LIFETIME_REACH.value();
            let r = seen(r) + extra;
            r + lookup_pad(query, magnetar_span_years(), r, reach, beta)
        });
        LightYears::new(supernovae.max(magnetars))
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

/// How far beyond its reach `reach_ly` from its centre a death of a list `distance_ly` from the
/// observer may lie when its light can reach `query`'s window, ly, its feature moving at `beta` of
/// light's speed: its feature's travel from the epoch to the death. Light covers a light-year a
/// Julian year, so years and light-years add.
///
/// A death at T whose light can reach the window within `span_years` of it lies at most
/// distance + reach + β|T| from the observer, so |T| ≤ (|t|max + span + distance + reach)
/// ÷ (1 − β), and its site within reach + β|T| of the centre; a year more covers the walks' day
/// margins. Never more than [`LIFETIME_REACH`], the cap's bound on any line's travel from its site
/// over its entry's lifetime.
#[must_use]
fn site_pad(
    query: &AlertQuery,
    span_years: f64,
    distance_ly: f64,
    reach_ly: f64,
    beta: f64,
) -> f64 {
    let k = pad_rate(beta);
    (k * (latest_years(query) + span_years + distance_ly + reach_ly + 1.0) + NEAR_MARGIN_LY)
        .min(LIFETIME_REACH.value())
}

/// The pad a list's site takes a light-year of its bound on |T|, ly: β ÷ (1 − β) with a relative
/// margin over the rounding.
#[must_use]
fn pad_rate(beta: f64) -> f64 {
    beta / (1.0 - beta) * (1.0 + PAD_MARGIN)
}

/// A bound on [`site_pad`] of any list a lookup of `radius_ly` keeps whose ball comes within it,
/// ly, for lists of reach at most `reach_ly`.
///
/// Such a list lies at most radius + reach + its pad P from the observer, and P = k (|t|max +
/// span + its distance + reach + 1) + ε for k = [`pad_rate`], so P ≤ k (|t|max + span + radius +
/// 2 reach + 1 + P) + ε, which the fixed point (k (…) + ε) ÷ (1 − k) bounds; a margin more covers
/// its own rounding.
#[must_use]
fn lookup_pad(
    query: &AlertQuery,
    span_years: f64,
    radius_ly: f64,
    reach_ly: f64,
    beta: f64,
) -> f64 {
    let k = pad_rate(beta);
    let years = latest_years(query) + span_years + radius_ly + 2.0 * reach_ly + 1.0;
    let fixed = (k * years + NEAR_MARGIN_LY) / (1.0 - k);
    (fixed * (1.0 + PAD_MARGIN) + NEAR_MARGIN_LY).min(LIFETIME_REACH.value())
}

/// One list's balls for a walk: about its centre, the radius that holds every site of its deaths
/// that can light the window for each kind walked, ly (module documentation), and whether each
/// comes within its kind's reach of the observer.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Balls {
    centre: GalacticPosition,
    /// The supernovae's site ball, if it comes within their reach.
    supernovae: Option<LightYears>,
    /// The magnetars' site ball, if it comes within their reach with the remnant's
    /// [`LIFETIME_REACH`].
    magnetars: Option<LightYears>,
}

impl Balls {
    /// The balls of a list about `centre` whose entries lie within `reach_ly` of it at the epoch,
    /// its sites within `pad(span_years, distance_ly)` more, ly, for `query` and `reaches`.
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
        let supernovae = reaches.supernovae.and_then(|(_, seen)| {
            let radius = reach_ly + pad(SUPERNOVA_LIGHT_YEARS, distance);
            within(radius, 0.0, seen)
        });
        let magnetars = reaches.magnetars.and_then(|seen| {
            let radius = reach_ly + pad(magnetar_span_years(), distance);
            within(radius, LIFETIME_REACH.value(), seen)
        });
        Self {
            centre,
            supernovae,
            magnetars,
        }
    }

    /// The balls of `feature`'s list, or `None` for a feature with no bulk motion, which lists
    /// nothing (a cloud, or a galaxy without its kinematic tables), or whose deaths reach no
    /// kind's sphere.
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
            site_pad(query, span, d, reach, beta)
        })
        .reaching()
    }

    /// The balls of the galactic centre's list, whose entries move at their own velocities: the
    /// centre's [`REACH`] and the cap's [`LIFETIME_REACH`].
    #[must_use]
    fn of_centre(query: &AlertQuery, reaches: &Reaches) -> Option<Self> {
        Self::of(
            query,
            reaches,
            (GalacticPosition::ORIGIN, REACH.value()),
            |_, _| LIFETIME_REACH.value(),
        )
        .reaching()
    }

    /// The balls, if any comes within its kind's reach.
    #[must_use]
    const fn reaching(self) -> Option<Self> {
        if self.supernovae.is_some() || self.magnetars.is_some() {
            Some(self)
        } else {
            None
        }
    }

    /// Whether a death at `t` can light `query`'s window from a ball (the cut).
    fn wanted(&self, query: &AlertQuery) -> impl Fn(UniverseTime) -> bool {
        let supernovae = self
            .supernovae
            .map(|radius| cut::one_shot_ball(query, &self.centre, radius));
        let magnetars = self
            .magnetars
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
/// the class and the candidates they drew, and their deaths whose time can light the window, in
/// the lists' order (the catalogue's cell and index order, the centre's last) and each list's
/// index order.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ListedWalk {
    expected: f64,
    drawn: u64,
    deaths: Vec<Death>,
}

impl ListedWalk {
    /// Whether `galaxy`'s walks read the lists (the hold, [`LISTED_ALERTS_VERSION`]).
    #[must_use]
    fn rules_read(galaxy: &Galaxy) -> bool {
        match galaxy.rules().listed_alerts {
            ListedAlertsRules::FieldAlone => false,
            ListedAlertsRules::Lists => true,
        }
    }

    /// The lists of `class` that `query`'s walk reads in `galaxy`, their features through `cells`
    /// (module documentation), or `None` below [`LISTED_ALERTS_VERSION`] and for a class no list
    /// holds or whose kinds the sensor does not see.
    ///
    /// # Panics
    ///
    /// If the galaxy's centre has no model or no disruptions' model, which no galaxy the builder
    /// allows reaches, and as the lists' entries do.
    #[must_use]
    pub(crate) fn find(
        galaxy: &Galaxy,
        cells: &dyn FeatureCellCache,
        query: &AlertQuery,
        class: ClassId,
    ) -> Option<Self> {
        let reaches = Reaches::of(class, query);
        if !Self::rules_read(galaxy) || reaches.is_empty() {
            return None;
        }
        let mut walk = Self::default();
        let radius = reaches.lookup_radius(query);
        for feature in FeatureCatalogue::near(galaxy, query.observer_position(), radius, cells) {
            walk.read_feature(galaxy, query, &reaches, class, &feature);
        }
        walk.read_centre(galaxy, query, &reaches, class);
        Some(walk)
    }

    /// The lists of `class` whose features' centres lie in the x-slab `x` of `sphere` that
    /// `query`'s background walk reads in `galaxy` ([`listed_scan_slab`]).
    #[must_use]
    fn find_in_slab(
        galaxy: &Galaxy,
        cells: &dyn FeatureCellCache,
        query: &AlertQuery,
        (class, sphere, x): (ClassId, &QuerySphere, i32),
    ) -> Self {
        let reaches = Reaches::of(class, query);
        let mut walk = Self::default();
        let slabs = class_slabs(class, sphere);
        if !Self::rules_read(galaxy) || reaches.is_empty() || !slabs.contains(&x) {
            return walk;
        }
        let edge = class
            .cell_log2_ly()
            .map(|log2| f64::from(1_u32 << log2))
            .expect("a class whose lists are walked has a grid");
        let (lo, hi) = slab_bounds(&slabs, x, edge);
        let observer = query.observer_position();
        let radius = reaches.lookup_radius(query);
        let mut found = Vec::new();
        for (cell, last) in FeatureCatalogue::cells_near(galaxy, observer, radius) {
            // Closed at both faces: a centre's x in light-years may round onto its cell's face.
            let low = f64::from(cell.to_array()[0]) * EDGE;
            if low > hi || low + EDGE < lo {
                continue;
            }
            FeatureCatalogue::found_in(galaxy, cells, (cell, last), (observer, radius), &mut found);
        }
        for feature in found {
            let at = feature.position().to_light_years_f64()[0];
            if (lo..hi).contains(&at) {
                walk.read_feature(galaxy, query, &reaches, class, &feature);
            }
        }
        if (lo..hi).contains(&0.0) {
            walk.read_centre(galaxy, query, &reaches, class);
        }
        walk
    }

    /// Reads `feature`'s list, if its deaths can reach a kind's sphere.
    fn read_feature(
        &mut self,
        galaxy: &Galaxy,
        query: &AlertQuery,
        reaches: &Reaches,
        class: ClassId,
        feature: &FeatureRecord,
    ) {
        let Some(balls) = Balls::of_feature(galaxy, query, reaches, feature) else {
            return;
        };
        let list = FeatureLevelList::of(galaxy, feature);
        let count = list.count(class);
        self.expected += list.expected(class);
        self.drawn += u64::from(count);
        if count == 0 {
            return;
        }
        let wanted = balls.wanted(query);
        let entries = list.entries().filter(|entry| entry.class() == class);
        for entry in entries {
            if let Some(death) = list
                .recent_entry_when(galaxy, entry.index(), &wanted)
                .and_then(|entry| Death::of_feature(galaxy, &entry))
            {
                self.deaths.push(death);
            }
        }
    }

    /// Reads the galactic centre's list, if its deaths can reach a kind's sphere.
    ///
    /// # Panics
    ///
    /// As [`find`](Self::find).
    fn read_centre(
        &mut self,
        galaxy: &Galaxy,
        query: &AlertQuery,
        reaches: &Reaches,
        class: ClassId,
    ) {
        let Some(balls) = Balls::of_centre(query, reaches) else {
            return;
        };
        let Some(recent_class) = RecentClass::of(class) else {
            return;
        };
        let model = galaxy
            .centre()
            .expect("no galaxy the builder allows fails its centre (CentreModel::new's tests)");
        let victims = victim_count(galaxy)
            .expect("no galaxy the builder allows fails its disruptions' model");
        let recent = CentreRecent::new(galaxy, model, victims);
        let expected = recent
            .expected()
            .iter()
            .filter(|(c, _)| *c == recent_class)
            .map(|(_, expected)| expected)
            .sum::<f64>();
        let list = recent.list(galaxy);
        let count = list.count(class);
        self.expected += expected;
        self.drawn += u64::from(count);
        if count == 0 {
            return;
        }
        let wanted = balls.wanted(query);
        let profiles = TracerProfiles::of(model);
        for entry in list.entries().filter(|entry| entry.class() == class) {
            let Some(kept) = recent
                .place(galaxy, model, &profiles, recent_class, entry.index())
                .and_then(|place| place.kept(galaxy))
            else {
                continue;
            };
            let (Some(site), Some(kind)) = (kept.site(), kept.explosion_kind()) else {
                continue;
            };
            if !wanted(site.death()) {
                continue;
            }
            let member = kept.member();
            self.deaths.push(Death::of_parts(
                galaxy,
                (member.record(), *member.composition()),
                kind,
                site.death(),
                (*kept.motion(), kept.supernova()),
            ));
        }
    }

    /// The task that walks what the features' lists hold that no walk reads in `galaxy` (the
    /// census's [`AlertSource::FeatureLists`](super::AlertSource::FeatureLists)): every class below
    /// [`LISTED_ALERTS_VERSION`] (P09.T36), and from it the luminous blue variables (P09.T43) and
    /// plan 11's binary classes (P11.T8.f).
    #[must_use]
    pub(crate) fn unwalked_by(galaxy: &Galaxy) -> &'static str {
        if Self::rules_read(galaxy) {
            "P09.T43, P11.T8.f"
        } else {
            "P09.T36"
        }
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

    /// Appends to `out` the detections of the deaths' supernovae and magnetars of `class` that
    /// `query`'s sensor sees in its window, sorted by arrival and then by event (module
    /// documentation).
    pub(crate) fn detections(
        &self,
        galaxy: &Galaxy,
        ctx: &mut AlertContext<'_>,
        query: &AlertQuery,
        class: ClassId,
        out: &mut Vec<Detection>,
    ) {
        let start = out.len();
        let reaches = Reaches::of(class, query);
        let observer = *query.observer_position();
        if let Some((kind, _)) = reaches.supernovae {
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
                if let Some(detection) = detect(galaxy, ctx, query, class, kind, &candidate) {
                    out.push(detection);
                }
            }
        }
        if reaches.magnetars.is_some() {
            let active = self
                .deaths
                .iter()
                .filter_map(|death| death.magnetar.as_ref())
                .filter_map(|host| {
                    emission_of(host, &observer, query).map(|interval| (host.clone(), interval))
                })
                .collect();
            hosts_detections(galaxy, ctx, query, class, active, out);
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

/// Appends to `out` the detections of the recently dead of `class` on the lists of the features
/// whose centres lie in the x-slab `x` of `sphere`, sorted by arrival and then by event, and
/// returns the candidates of the class those lists drew (module documentation).
///
/// `sphere` is the class's sphere of a background walk for `query`, whose slabs are
/// [`class_slabs`]'s: the first slab takes the features below it, the last those above, and the
/// slab of the origin the galactic centre's list. A pure function of the slab, which a background
/// scan runs beside its cells: over every slab of `sphere`, merged by arrival and then by event,
/// the detections are those the interactive walk adds to the class's cells'
/// ([`alerts_in`](super::alerts_in)), each to the bit, in any order and with any cache. Nothing
/// below generator version 29 (`LISTED_ALERTS_VERSION`), for a class no list holds, a slab outside
/// the sphere's, or a class the query does not ask about. The features are read through the
/// context's feature cells ([`AlertContext::with_feature_cells`]).
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
    if RecentClass::of(class).is_none() {
        return 0;
    }
    let walk = ListedWalk::find_in_slab(galaxy, ctx.feature_cells(), query, (class, sphere, x));
    walk.detections(galaxy, ctx, query, class, out);
    walk.drawn()
}

#[cfg(test)]
mod tests;
