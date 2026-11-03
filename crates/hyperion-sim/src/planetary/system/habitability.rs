//! The habitability assessment's wiring (P14.T25): what
//! [`PlanetarySystem::hooks_at`] reads of a body for [`HabitabilityAssessment::assess`], from its
//! record, its seasonal orbit, its zone's habitable zone and its zone's stars.
//!
//! - **The record**: its state, mass and bulk class, and its surface section's state, pressure,
//!   mean temperature, liquid-water flag, published water (liquid or ice), the state its water
//!   follows and subsurface ocean, with its conditions' radiation class. A body whose
//!   section is not applicable while it is present has an envelope over it: its derivation's
//!   published state and X-ray and ultraviolet flux stand in.
//! - **The seasonal orbit**: a planet's or a member's own record's orbit, about its zone's host,
//!   and a moon's planet's. A planet or member locked 1:1 at the time
//!   ([`SpinState::Locked`] in the synchronous resonance) is tidally locked; a moon never is.
//! - **The zone**: Kopparapu et al.'s, of the zone's stars at the time with their companions'
//!   light, as [`PlanetarySystem::habitable_zone_at`] gives it.
//! - **The zone's stars**: their phases at the time and, from each one's track, its main
//!   sequence's end ([`HostClock`]).
//!
//! Nothing here draws, and nothing here is on the record or the wire.

use crate::planetary::chem::partition::CondensatePhase;
use crate::planetary::derive::atmosphere::{Gas, SurfaceState};
use crate::planetary::derive::habitable_zone_of;
use crate::planetary::derive::rotation::{SpinOrbitResonance, SpinState};
use crate::planetary::fate::BodyState;
use crate::planetary::hooks::Subsurface;
use crate::planetary::hooks::habitability::{
    HabitabilityAssessment, HabitabilityInputs, HostClock, SurfaceReading,
};
use crate::planetary::hooks::surface::{RadiationClass, SeasonsFrom};
use crate::planetary::placement::OrbitZone;
use crate::planetary::record::{BodyOrbit, BodyRecord, Section};
use crate::stellar::Phase;
use crate::units::{WattsPerSquareMetre, Years};

use super::{Body, Epoch, PlanetarySystem};

/// What a body's derivation gives its assessment where an envelope hides its surface: its
/// published state and the X-ray and ultraviolet flux at it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Unseen {
    /// Its atmosphere's published state
    /// ([`Atmosphere::published_state`](crate::planetary::derive::atmosphere::Atmosphere::published_state)).
    pub(super) state: SurfaceState,
    /// The X-ray and ultraviolet flux at it.
    pub(super) xuv_flux: WattsPerSquareMetre,
}

impl PlanetarySystem {
    /// The habitability section of the body whose record at the epoch's time is `record`
    /// (P14.T25; the [module](self) documentation), whose seasonal orbit is `primary`'s orbit
    /// section `seasonal`, which is `whose`: the body's own, or its planet's for a moon. `unseen`
    /// is its derivation's, read only where it is present with no surface section.
    ///
    /// - [`Section::NotApplicable`] for a body not present at the time;
    /// - the surface section's state where that is not modelled or not resolved, and not modelled
    ///   where the record or the derivation lacks what the assessment reads;
    /// - otherwise [`Section::Ok`].
    #[must_use]
    pub(super) fn habitability_section(
        &self,
        epoch: &Epoch<'_>,
        (primary, seasonal, whose): (&Body, Option<&BodyOrbit>, SeasonsFrom),
        record: &BodyRecord,
        unseen: Option<Unseen>,
    ) -> Section<HabitabilityAssessment> {
        match record.identity().state() {
            BodyState::Present => {}
            BodyState::NotYetFormed | BodyState::Destroyed { .. } | BodyState::Unbound { .. } => {
                return Section::NotApplicable;
            }
        }
        let (state, surface, radiation) = match record.surface() {
            Section::Ok(surface) => {
                let conditions = surface.conditions();
                let water = Gas::Water.substance();
                let condensates = surface.condensates();
                let reading = SurfaceReading {
                    pressure: conditions.surface_pressure(),
                    liquid_water: conditions.liquid_water(),
                    subsurface_ocean: match surface.figures().subsurface_ocean() {
                        Subsurface::Ocean(_) => true,
                        Subsurface::NoOcean | Subsurface::NotComputed => false,
                    },
                    mean_temperature: conditions.mean_surface_temperature(),
                    holds_water: [CondensatePhase::Liquid, CondensatePhase::Solid]
                        .into_iter()
                        .any(|phase| condensates.entry(water, phase).is_some()),
                    water_state: surface.figures().water_state(),
                };
                (surface.state(), Some(reading), conditions.radiation())
            }
            Section::NotApplicable => match unseen {
                Some(unseen) => (unseen.state, None, RadiationClass::of(unseen.xuv_flux)),
                None => return Section::NotModelled,
            },
            Section::NotModelled => return Section::NotModelled,
            Section::NotResolved => return Section::NotResolved,
        };
        let (Section::Ok(bulk), Section::Ok(mass), Some(seasonal)) =
            (record.bulk(), record.mass(), seasonal)
        else {
            return Section::NotModelled;
        };
        let zone = self.zone_of(primary);
        let habitable = epoch
            .lights(zone)
            .zip(epoch.companions(zone))
            .map(|(orbited, companions)| habitable_zone_of(&orbited, &companions));
        let locked = match whose {
            SeasonsFrom::Own => record.rotation().ok().is_some_and(|frame| {
                matches!(
                    frame.state_at(epoch.t),
                    SpinState::Locked(SpinOrbitResonance::Synchronous)
                )
            }),
            SeasonsFrom::Planet => false,
        };
        let elements = seasonal.elements();
        Section::Ok(HabitabilityAssessment::assess(&HabitabilityInputs {
            class: bulk.class(),
            mass: *mass,
            state,
            surface,
            zone: habitable,
            semi_major_axis: elements.semi_major_axis(),
            eccentricity: elements.eccentricity(),
            locked,
            radiation,
            host: host_clock(epoch, zone),
        }))
    }
}

/// What the assessment of a body of `zone` reads of the zone's stars at the epoch's
/// time ([`HostClock`]): whether one is a protostar or before its main sequence, and the least
/// time one has left on it, from its track's main-sequence end less its age then, zero for one
/// past it that still burns. A star not formed, a remnant, a brown dwarf and a star with no
/// track (below 0.1 M☉) leave nothing.
///
/// From generator version 27 (P14.T1.g) a star of a pair the binary engine ran reads its clock
/// from its pair's timeline ([`SystemContext::history_pair_at`](crate::planetary::context::SystemContext::history_pair_at)),
/// a merger's product's its own.
#[must_use]
fn host_clock(epoch: &Epoch<'_>, zone: &OrbitZone) -> HostClock {
    let (ctx, t) = (epoch.ctx, epoch.t);
    let mut clock = HostClock {
        before_main_sequence: false,
        main_sequence_left: None,
    };
    for member in zone.members() {
        let Some(state) = epoch.states[usize::from(member)].as_ref() else {
            continue;
        };
        let star = &ctx.stars()[usize::from(member)];
        // A paired star's clock is its own, its pair's history's (P14.T1.g): a merger's product's
        // is the product's.
        let ahead = || {
            if let Some((pair, component, age)) = ctx.history_pair_at(usize::from(member), t) {
                return pair.timeline().main_sequence_left(component, age);
            }
            let end = star.shared_track()?.main_sequence_ends()?;
            Some(Years::new((end.value() - star.age_at(t).value()).max(0.0)))
        };
        let left = match state.phase() {
            Phase::Protostar | Phase::PreMainSequence => {
                clock.before_main_sequence = true;
                ahead()
            }
            Phase::MainSequence => ahead(),
            Phase::HertzsprungGap
            | Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::HeliumMainSequence
            | Phase::HeliumHertzsprungGap
            | Phase::HeliumGiantBranch
            | Phase::PostAgb => Some(Years::new(0.0)),
            Phase::Substellar
            | Phase::HeliumWhiteDwarf
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf
            | Phase::NeutronStar
            | Phase::BlackHole
            | Phase::NoRemnant => None,
        };
        clock.main_sequence_left = match (clock.main_sequence_left, left) {
            (Some(least), Some(left)) => Some(Years::new(least.value().min(left.value()))),
            (least, left) => least.or(left),
        };
    }
    clock
}
