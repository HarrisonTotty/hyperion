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

use crate::galaxy::Galaxy;
use crate::galaxy::Population;
use crate::galaxy::catalogue_classes::ClassId;
use crate::galaxy::catalogue_classes::binary::feature::{
    FeatureBinariesRules, LISTED_CLASSES, ListedBinary, hosts_per_formed_mass, listed_entry,
};
use crate::galaxy::catalogue_classes::binary::{BinaryHost, ListedHost};
use crate::galaxy::features::interior::{ClassProfile, ProfileShape};
use crate::galaxy::features::members::ListClass;
use crate::galaxy::placement::SystemOrigin;
use crate::id::{CentreMemberId, MemberSlot, SystemId};
use crate::rng::{ObjectKey, Stream, tags};
use crate::units::Years;
use crate::units::consts::METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND;

use super::CentreModel;
use super::classes::age_components;
use super::members::{CentreMemberRecord, centre_composition};
use super::recent::{centre_class, galactic, position};

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
    /// member it is, if its episode's draws and its class's keep it (module documentation).
    #[must_use]
    pub(crate) fn entry(
        &self,
        galaxy: &Galaxy,
        model: &CentreModel,
        class: ClassId,
        index: u16,
    ) -> Option<(ListedBinary, CentreMemberRecord)> {
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
        let at = position(model, &component, key, seed);
        let mut velocity = Stream::open(seed, tags::CENTRE_VELOCITY, key);
        let velocity_km_s = model.distribution(class_of.tracer).draw_velocity(
            model.profile(),
            at,
            &mut velocity,
        )?;
        let position = galactic(at);
        let composition = centre_composition();
        let velocity = crate::coords::GalacticVelocity::new(
            velocity_km_s.map(|c| c * METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND),
        );
        let host = ListedHost::new(
            Population::NuclearDisc,
            (
                Years::new(component.youngest.value().max(0.0)),
                component.oldest,
            ),
            composition,
            velocity,
            SystemOrigin::CentreMember { attempt: 0 },
        );
        let entry = listed_entry(galaxy, class, BinaryHost::listed(system, position, host))?;
        let member =
            CentreMemberRecord::from_parts(*entry.record(), class_of, composition, velocity_km_s);
        Some((entry, member))
    }
}

#[cfg(test)]
mod tests;
