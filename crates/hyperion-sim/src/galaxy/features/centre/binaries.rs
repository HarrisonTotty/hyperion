//! Plan 11's binary classes on the galactic centre's feature-level list (P11.T8.f), after its
//! victims and its recent classes, under P11.T8.f's hold (`FEATURE_BINARIES_VERSION`).
//!
//! The nuclear cluster is a budget of its own (plan 09, Design note 5), so the catalogue gives up
//! nothing for these hosts. Each class's expected hosts are the cluster's formed mass, its
//! primaries formed times the galaxy's mass formed per system (the share matrix's `r m̄_f` of
//! every system born, P11.T7, which counts the companions the centre's members are drawn without:
//! ruling 144.4's question, answered as the share matrix answers it, lean 2026-11-01), times the
//! class's hosts per solar mass formed in the nuclear disc's population
//! ([`hosts_per_formed_mass`]). That is at most P11.T8.e's forecast, which takes each row's largest
//! rate and allows for the thinning, so a centre that check passes builds its list.
//!
//! An entry picks one of the centre's star-formation episodes by its share of the mass formed
//! (`member.mass`, word 0), draws its position from the episode's tracer and its velocity from its
//! distribution function, as the recent classes' entries do, and is drawn present first at that
//! host through its class's own sampler and scan marks, its age in the episode's ages and its
//! composition the centre's ([`listed_entry`]). A candidate the episode's ages cannot hold (an
//! accreting white dwarf among the young stars) is thinned.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::Population;
use crate::galaxy::catalogue_classes::ClassId;
use crate::galaxy::catalogue_classes::binary::feature::{
    FeatureBinariesRules, LISTED_CLASSES, ListedBinary, hosts_per_formed_mass, is_listed_binary,
    listed_entry,
};
use crate::galaxy::catalogue_classes::binary::{BinaryHost, ListedHost};
use crate::galaxy::catalogue_classes::source::member_layers;
use crate::galaxy::features::interior::{ClassProfile, ProfileShape};
use crate::galaxy::features::members::{FeatureLevelList, ListClass};
use crate::galaxy::placement::SystemOrigin;
use crate::galaxy::query::{LayerCounts, LayerSet};
use crate::id::{CentreMemberId, MemberSlot, SystemId};
use crate::rng::{ObjectKey, Stream, tags};
use crate::units::consts::METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND;
use crate::units::{LightYears, Years};

use super::CentreModel;
use super::classes::{AgeComponent, CentreClass, age_components};
use super::members::{CentreMemberRecord, centre_composition};
use super::recent::{TracerProfiles, centre_class, galactic, position_on};

/// The centre's hosts of plan 11's binary classes (module documentation): each class's expected
/// hosts, in list order, those expecting none left out.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CentreBinaries {
    classes: Vec<(ClassId, f64)>,
}

impl CentreBinaries {
    /// The binary classes of the centre of `galaxy`, whose model is `model`; none under the hold's
    /// held arm.
    #[must_use]
    pub(crate) fn new(galaxy: &Galaxy, model: &CentreModel) -> Self {
        if galaxy.rules().feature_binaries == FeatureBinariesRules::Unlisted {
            return Self {
                classes: Vec::new(),
            };
        }
        let formed = model.classes().primaries_formed() * galaxy.mean_formed_mass().value();
        let classes = LISTED_CLASSES
            .iter()
            .map(|&class| {
                (
                    class,
                    formed * hosts_per_formed_mass(galaxy, class, Population::NuclearDisc),
                )
            })
            .filter(|&(_, expected)| expected > 0.0)
            .collect();
        Self { classes }
    }

    /// Each class's expected hosts, in list order.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn expected(&self) -> &[(ClassId, f64)] {
        &self.classes
    }

    /// Its classes as the centre's list registers them: their expected hosts, and a placeholder
    /// profile, since each entry draws its own position.
    #[must_use]
    pub(crate) fn list_classes(&self) -> Vec<ListClass> {
        let placeholder = ClassProfile::new(ProfileShape::Plummer { scale: 1.0 }, 128.0, 1.0);
        self.classes
            .iter()
            .map(|&(class, expected)| ListClass::new(class, expected, placeholder.clone()))
            .collect()
    }

    /// The entry of class `class` at the centre's list index `index`, its class's entry and the
    /// member it is, if its episode's draws and its class's keep it (module documentation): its
    /// [`place`](Self::place), then its class's draws and test ([`CentreBinaryPlace::kept`]).
    #[must_use]
    pub(crate) fn entry(
        &self,
        galaxy: &Galaxy,
        model: &CentreModel,
        class: ClassId,
        index: u16,
    ) -> Option<(ListedBinary, CentreMemberRecord)> {
        self.place(galaxy, model, &TracerProfiles::of(model), class, index)?
            .kept(galaxy)
    }

    /// The candidate of class `class` at the centre's list index `index`, placed, its tracers'
    /// profiles `profiles` (P09.T37): its episode, its position and its velocity, before its
    /// class's draws; `None` for a class it does not list, an index past the centre's, or an
    /// episode or a velocity its draws refuse.
    #[must_use]
    pub(crate) fn place(
        &self,
        galaxy: &Galaxy,
        model: &CentreModel,
        profiles: &TracerProfiles,
        class: ClassId,
        index: u16,
    ) -> Option<CentreBinaryPlace> {
        if !self.classes.iter().any(|&(c, _)| c == class) {
            return None;
        }
        // `IndexTooLarge` is the one error, and an index past the centre's names no entry.
        let id = CentreMemberId::new(MemberSlot::FeatureLevel { index }).ok()?;
        let system = SystemId::from(id);
        let key = ObjectKey::from(system);
        let seed = galaxy.seed();
        let components = age_components();
        let weights = components.map(|c| c.share);
        let total = weights.iter().fold(0.0, |s, w| s + w);
        let picked = Stream::open(seed, tags::MEMBER_MASS, key)
            .mark()
            .pick_weighted(&weights, total)?;
        let component = components[picked];
        let class_of = centre_class(
            model,
            u8::try_from(picked).expect("one of the centre's six episodes"),
        )?;
        let at = position_on(profiles.of_tracer(component.tracer), key, seed);
        let mut velocity = Stream::open(seed, tags::CENTRE_VELOCITY, key);
        let velocity_km_s = model.distribution(class_of.tracer).draw_velocity(
            model.profile(),
            at,
            &mut velocity,
        )?;
        Some(CentreBinaryPlace {
            class,
            system,
            component,
            class_of,
            position: galactic(at),
            velocity_km_s,
        })
    }

    /// The census's share of the centre's binary classes in the ball of `radius` about a point
    /// `distance` from the black hole, added to `counts` in each layer its class's members can be
    /// of (the class source's `member_layers`; P09.T37): each class's expected hosts times the
    /// share of the episodes' tracer profiles inside the reach, by the episodes' shares, between
    /// `distance − radius` and `distance + radius` from the black hole, a shell that holds the
    /// ball. Classes in list order.
    pub(crate) fn add_expected_within(
        &self,
        profiles: &TracerProfiles,
        (distance, radius): (LightYears, LightYears),
        counts: &mut LayerCounts,
    ) {
        let components = age_components();
        let total = components.iter().fold(0.0, |s, c| s + c.share);
        let share = components.iter().fold(0.0, |sum, c| {
            sum + c.share / total * profiles.shell_share(c.tracer, (distance, radius))
        });
        for &(class, expected) in &self.classes {
            let layers = member_layers(class)
                .and_then(|sets| sets.first().copied())
                .unwrap_or(LayerSet::EMPTY);
            for layer in layers.iter() {
                counts.set(layer, counts.get(layer) + expected * share);
            }
        }
    }

    /// The places of every candidate of its classes on the centre's list `list`, in index order.
    #[must_use]
    pub(crate) fn places(
        &self,
        galaxy: &Galaxy,
        model: &CentreModel,
        (list, profiles): (&FeatureLevelList, &TracerProfiles),
    ) -> Vec<CentreBinaryPlace> {
        list.entries()
            .filter(|entry| is_listed_binary(entry.class()))
            .filter_map(|entry| self.place(galaxy, model, profiles, entry.class(), entry.index()))
            .collect()
    }

    /// Whether it lists no class.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

/// A candidate of plan 11's binary classes on the centre's list, placed, before its class's draws
/// (P09.T37): its class, its episode, its position at the epoch and its velocity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CentreBinaryPlace {
    class: ClassId,
    system: SystemId,
    component: AgeComponent,
    class_of: CentreClass,
    position: GalacticPosition,
    velocity_km_s: [f64; 3],
}

impl CentreBinaryPlace {
    /// Its class.
    #[must_use]
    pub(crate) const fn class(&self) -> ClassId {
        self.class
    }

    /// Its position at the epoch.
    #[must_use]
    pub(crate) const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// Its velocity, galactic, as its member's ([`CentreMemberRecord::velocity`]).
    #[must_use]
    pub(crate) fn velocity(&self) -> GalacticVelocity {
        GalacticVelocity::new(
            self.velocity_km_s
                .map(|c| c * METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND),
        )
    }

    /// Its class's entry and the member it is, if its class's draws and test at its host keep it
    /// (module documentation).
    #[must_use]
    pub(crate) fn kept(&self, galaxy: &Galaxy) -> Option<(ListedBinary, CentreMemberRecord)> {
        let composition = centre_composition();
        let host = ListedHost::new(
            Population::NuclearDisc,
            (
                Years::new(self.component.youngest.value().max(0.0)),
                self.component.oldest,
            ),
            composition,
            self.velocity(),
            SystemOrigin::CentreMember { attempt: 0 },
        );
        let entry = listed_entry(
            galaxy,
            self.class,
            BinaryHost::listed(self.system, self.position, host),
        )?;
        let member = CentreMemberRecord::from_parts(
            *entry.record(),
            self.class_of,
            composition,
            self.velocity_km_s,
        );
        Some((entry, member))
    }
}

#[cfg(test)]
mod tests;
