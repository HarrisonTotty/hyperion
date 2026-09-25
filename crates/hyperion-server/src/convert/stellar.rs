//! The `system_summary` request and its answer (plan 06, P06.T34, with plan 11's P11.T13 slice):
//! the checks of the request's fields, and the wire form of a system's stars and of the hierarchy
//! of orbits that holds them.
//!
//! The answer follows the three states of `hyperion_protocol`'s stellar module (ruling 54 of
//! 2026-09-22): a field this generator version does not compute is absent, one computed that the
//! object lacks is `null`, and the rest carry their values. So rotation, activity, variability, a
//! planetary nebula and the active events are absent for every star (plan 06's T16 and T25–T28),
//! as are a pulsar's detail (T21) and a black hole's spin (T22). A remnant's natal kick is
//! P06.T19's ([`StarModel::natal_kick`]), `null` where the model holds none.

use hyperion_protocol::{
    ErrorCode, HierarchyDto, HierarchyNodeDto, KickModeDto, Modelled, NatalKickDto, ObjectKindDto,
    OrbitDto, PhaseDto, RemnantDto, RequestError, StarSummaryDto, StellarBriefDto,
    SystemExistenceDto, SystemIdHex, SystemSummaryDto, SystemSummaryRequest,
};
use hyperion_sim::id::SystemId;
use hyperion_sim::orbit::KeplerElements;
use hyperion_sim::stellar::multiplicity::{HierarchyNode, SystemHierarchy};
use hyperion_sim::stellar::remnant::{CompactRemnant, KickMode, NatalKick, RemnantKind};
use hyperion_sim::stellar::system::{
    StarModel, StarSummary, StellarBrief, SystemExistence, SystemStars,
};
use hyperion_sim::stellar::{ObjectKind, Phase, StarState};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{Dex, KilometresPerSecond, Magnitudes, Megayears, Years};

use super::query_time;

/// A `system_summary` request, checked: the system it names, decoded, and the time inside the
/// clock window.
///
/// The universe is looked up before this, as every handler that names one does. Then `time` is
/// checked as the range query checks it (plan 04, design note 24): a time outside the clock window
/// is a `bad_request` naming `time`. Then the ID is decoded: a well-formed `SystemIdHex` whose bits
/// are not a system ID names no system, so it is `unknown_system` naming `system`, as an ID that
/// plan 03's `resolve` refuses is on the pool job ([`unknown_system`]). A `SystemIdHex` that is not
/// sixteen hexadecimal digits never reaches here: the request does not parse, and plan 04 answers
/// `bad_request`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SummaryRequest {
    system: SystemId,
    time: UniverseTime,
}

impl SummaryRequest {
    /// The system asked about, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn system(self) -> SystemId {
        self.system
    }

    /// The instant asked about, inside the clock window.
    #[must_use]
    pub(crate) fn time(self) -> UniverseTime {
        self.time
    }
}

impl TryFrom<&SystemSummaryRequest> for SummaryRequest {
    type Error = RequestError;

    /// Checks `time`, then decodes `system`.
    fn try_from(request: &SystemSummaryRequest) -> Result<Self, Self::Error> {
        let time = query_time(&request.time)?;
        let system = SystemId::from_raw(request.system.to_u64())
            .map_err(|error| unknown_system(&request.system, error))?;
        Ok(Self { system, time })
    }
}

/// The refusal of a well-formed system ID that names no system of the universe:
/// `unknown_system`, naming the request's `system` field (plan 06, P06.T33).
#[must_use]
pub(crate) fn unknown_system(system: &SystemIdHex, reason: impl std::fmt::Display) -> RequestError {
    RequestError {
        code: ErrorCode::UnknownSystem,
        message: format!(
            "system {} names no system of this universe: {reason}",
            system.as_str()
        ),
        field: Some("system".to_owned()),
    }
}

/// The answer to `system_summary`: the system's stars and their hierarchy at the request's time.
///
/// `universe`, `system` and `time` are echoed from the request, which is why it is taken by value.
/// Before the system is born it has no stars and no nodes, and its age is negative.
#[must_use]
pub(crate) fn system_summary(
    request: SystemSummaryRequest,
    stars: &SystemStars,
    time: UniverseTime,
) -> SystemSummaryDto {
    let summary = stars.summary_at(time);
    SystemSummaryDto {
        universe: request.universe,
        system: request.system,
        time: request.time,
        existence: existence(summary.existence()),
        age_myr: Megayears::from(stars.record().age_at(time)).value(),
        fe_h_dex: summary.composition().fe_h().value(),
        stars: summary
            .stars()
            .iter()
            .map(|star| {
                let model = &stars.stars()[usize::from(star.body().body_index())];
                star_summary(model, star)
            })
            .collect(),
        hierarchy: summary
            .hierarchy()
            .map_or_else(|| HierarchyDto { nodes: Vec::new() }, hierarchy),
    }
}

/// One star's summary as the wire carries it; `model` is the star the summary is of.
#[must_use]
fn star_summary(model: &StarModel, star: &StarSummary) -> StarSummaryDto {
    let state = star.state();
    // An object with no light has no photosphere, and its 0 K is not a temperature (ruling 54).
    let lit = state.luminosity().value() > 0.0;
    StarSummaryDto {
        body_index: star.body().body_index(),
        kind: object_kind(star.kind()),
        phase: phase(state.phase(), star.kind()),
        class: star.classification().to_string(),
        initial_mass_msun: model.initial_mass().value(),
        mass_msun: state.mass().value(),
        core_mass_msun: state.core_mass().value(),
        luminosity_lsun: state.luminosity().value(),
        radius_rsun: state.radius().value(),
        teff_k: lit.then(|| state.effective_temperature().value()),
        absolute_v_mag: star.absolute_magnitude_v().map(Magnitudes::value),
        colour_b_v_mag: star.colour_b_v().map(Magnitudes::value),
        mass_loss_rate_msun_per_yr: state.mass_loss_rate().value(),
        remnant: star
            .remnant()
            .map(|remnant| remnant_dto(model, state, remnant)),
        death_time: star.death_in_window().map(|(when, _)| wire_time(when)),
        rotation_period_d: Modelled::NotModelled,
        activity_log_lx_lbol: Modelled::NotModelled,
        variability: Modelled::NotModelled,
        planetary_nebula: Modelled::NotModelled,
        active_events: None,
    }
}

/// A range row's brief of its system's primary, as the wire carries it (plan 06, P06.T34).
///
/// As in a summary, an object with no light has no photosphere: its log L and `T_eff` are `null`
/// (ruling 54), where the sim's brief has no logarithm and a temperature of 0 K. The wire's `f32`s
/// round the sim's values, which is finer than any display of them.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "log L lies within ±20 and T_eff under 10⁷ K, well inside f32's range, whose seven \
              digits are finer than the chart and the HR diagram draw them"
)]
pub(crate) fn brief_dto(brief: &StellarBrief) -> StellarBriefDto {
    let log_luminosity = brief.log_luminosity().map(Dex::value);
    StellarBriefDto {
        kind: object_kind(brief.kind()),
        class: brief.class().to_string(),
        log_luminosity_lsun: log_luminosity.map(|l| l as f32),
        teff_k: log_luminosity.map(|_| brief.effective_temperature().value() as f32),
        star_count: brief.star_count(),
    }
}

/// What a dead star left, with what this generator version knows of it.
///
/// A white dwarf's cooling age is the time since the star died, its age now less its age at death.
#[must_use]
fn remnant_dto(model: &StarModel, state: &StarState, remnant: CompactRemnant) -> RemnantDto {
    let natal_kick = model.natal_kick().map(natal_kick);
    match remnant.kind() {
        RemnantKind::WhiteDwarf => {
            let death = model
                .death()
                .expect("a star that has left a white dwarf has died on its track");
            RemnantDto::WhiteDwarf {
                cooling_age_myr: Megayears::from(Years::new(
                    state.age().value() - death.age().value(),
                ))
                .value(),
                natal_kick,
            }
        }
        RemnantKind::NeutronStar => RemnantDto::NeutronStar {
            pulsar: None,
            natal_kick,
        },
        RemnantKind::BlackHole => RemnantDto::BlackHole {
            dimensionless_spin: None,
            natal_kick,
        },
        RemnantKind::None => RemnantDto::NoRemnant,
    }
}

/// A natal kick as the wire carries it: its speed and the branch of the law that gave it.
#[must_use]
fn natal_kick(kick: NatalKick) -> NatalKickDto {
    NatalKickDto {
        speed_km_s: KilometresPerSecond::from(kick.speed()).value(),
        mode: match kick.mode() {
            KickMode::Ordinary => KickModeDto::Ordinary,
            KickMode::Low => KickModeDto::Low,
            KickMode::FallbackNone => KickModeDto::FallbackNone,
            KickMode::WhiteDwarf => KickModeDto::WhiteDwarf,
        },
    }
}

/// A system's hierarchy as the wire carries it: its nodes in the same depth-first order, each
/// star with its body index and the initial mass its orbits are bound to, each pair with its
/// members' indices and its orbit (plan 11, P11.T13).
#[must_use]
fn hierarchy(hierarchy: &SystemHierarchy) -> HierarchyDto {
    HierarchyDto {
        nodes: hierarchy
            .nodes()
            .iter()
            .map(|node| match node {
                HierarchyNode::Star(index) => {
                    let slot = hierarchy.star(*index);
                    HierarchyNodeDto::Star {
                        body_index: slot.body().body_index(),
                        mass_msun: slot.initial_mass().value(),
                    }
                }
                HierarchyNode::Pair {
                    inner,
                    outer,
                    orbit,
                } => HierarchyNodeDto::Pair {
                    inner: inner.get(),
                    outer: outer.get(),
                    orbit: orbit_dto(orbit),
                },
            })
            .collect(),
    }
}

/// An orbit as the wire carries it: every element at the epoch, and the gravitational parameter
/// (ruling 33 of 2026-09-22). Plan 14's body orbits take the same form.
#[must_use]
pub(crate) fn orbit_dto(orbit: &KeplerElements) -> OrbitDto {
    OrbitDto {
        period_s: orbit.period().value(),
        semi_major_axis_m: orbit.semi_major_axis().value(),
        eccentricity: orbit.eccentricity().value(),
        inclination_rad: orbit.inclination().value(),
        ascending_node_rad: orbit.ascending_node().value(),
        argument_of_periapsis_rad: orbit.argument_of_periapsis().value(),
        mean_anomaly_at_epoch_rad: orbit.mean_anomaly_at_epoch().value(),
        mu_m3_s2: orbit.gravitational_parameter().value(),
    }
}

/// A clock time as the wire carries it.
#[must_use]
pub(super) fn wire_time(t: UniverseTime) -> hyperion_protocol::UniverseTime {
    hyperion_protocol::UniverseTime {
        seconds: t.seconds(),
        nanos: t.subsec_nanos(),
    }
}

/// Whether the system exists, as the wire says it.
#[must_use]
fn existence(existence: SystemExistence) -> SystemExistenceDto {
    match existence {
        SystemExistence::NotYetBorn => SystemExistenceDto::NotYetBorn,
        SystemExistence::Exists => SystemExistenceDto::Exists,
    }
}

/// What an object is, as the wire says it.
#[must_use]
fn object_kind(kind: ObjectKind) -> ObjectKindDto {
    match kind {
        ObjectKind::Protostar => ObjectKindDto::Protostar,
        ObjectKind::PreMainSequence => ObjectKindDto::PreMainSequence,
        ObjectKind::Dwarf => ObjectKindDto::Dwarf,
        ObjectKind::Subgiant => ObjectKindDto::Subgiant,
        ObjectKind::Giant => ObjectKindDto::Giant,
        ObjectKind::Supergiant => ObjectKindDto::Supergiant,
        ObjectKind::WolfRayet => ObjectKindDto::WolfRayet,
        ObjectKind::HotSubdwarf => ObjectKindDto::HotSubdwarf,
        ObjectKind::WhiteDwarf => ObjectKindDto::WhiteDwarf,
        ObjectKind::NeutronStar => ObjectKindDto::NeutronStar,
        ObjectKind::BlackHole => ObjectKindDto::BlackHole,
        ObjectKind::NoRemnant => ObjectKindDto::NoRemnant,
        ObjectKind::Substellar => ObjectKindDto::Substellar,
    }
}

/// A star's phase, as the wire says it, from its phase in the sim and what kind of object it is.
///
/// The sim's [`Phase::Substellar`] is the track of P06.T13's cooling fits, which hold every object
/// of initial mass below 0.1 M☉, where Hurley, Pols and Tout's formulae stop (ruling 33 of
/// 2026-09-22): the brown dwarfs, and above the hydrogen-burning limit
/// ([`hyperion_sim::stellar::substellar::hydrogen_burning_limit`], 0.065–0.083 M☉ with
/// metallicity) the latest M dwarfs, which burn hydrogen in their cores for longer than the
/// universe's age. The sim's [`ObjectKind`] already tells the two apart, so the wire follows it:
/// an object on the cooling fits that is substellar is sent as [`PhaseDto::Substellar`], and one
/// above the limit, a dwarf, as [`PhaseDto::MainSequence`], the phase its hydrogen burning is.
#[must_use]
fn phase(phase: Phase, kind: ObjectKind) -> PhaseDto {
    match phase {
        Phase::Substellar if kind != ObjectKind::Substellar => PhaseDto::MainSequence,
        Phase::Protostar => PhaseDto::Protostar,
        Phase::PreMainSequence => PhaseDto::PreMainSequence,
        Phase::MainSequence => PhaseDto::MainSequence,
        Phase::HertzsprungGap => PhaseDto::HertzsprungGap,
        Phase::FirstGiantBranch => PhaseDto::FirstGiantBranch,
        Phase::CoreHeliumBurning => PhaseDto::CoreHeliumBurning,
        Phase::EarlyAgb => PhaseDto::EarlyAgb,
        Phase::ThermallyPulsingAgb => PhaseDto::ThermallyPulsingAgb,
        Phase::HeliumMainSequence => PhaseDto::HeliumMainSequence,
        Phase::HeliumHertzsprungGap => PhaseDto::HeliumHertzsprungGap,
        Phase::HeliumGiantBranch => PhaseDto::HeliumGiantBranch,
        Phase::PostAgb => PhaseDto::PostAgb,
        Phase::HeliumWhiteDwarf => PhaseDto::HeliumWhiteDwarf,
        Phase::CarbonOxygenWhiteDwarf => PhaseDto::CarbonOxygenWhiteDwarf,
        Phase::OxygenNeonWhiteDwarf => PhaseDto::OxygenNeonWhiteDwarf,
        Phase::NeutronStar => PhaseDto::NeutronStar,
        Phase::BlackHole => PhaseDto::BlackHole,
        Phase::NoRemnant => PhaseDto::NoRemnant,
        Phase::Substellar => PhaseDto::Substellar,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_protocol::UniverseIdHex;
    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::Galaxy;
    use hyperion_sim::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
    use hyperion_sim::id::Layer;
    use hyperion_sim::units::SolarMasses;

    use super::*;

    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(0x4d2)))
    }

    /// The first system of a layer-C cell at the solar circle, as a template for records of other
    /// masses and ages at the same place.
    fn template() -> SystemRecord {
        let mut cell = Vec::new();
        generate_cell(
            galaxy(),
            CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid"),
            &mut cell,
        );
        cell[0]
    }

    /// The template's system with its primary of `mass` M☉ and `age` years old at the epoch.
    fn system(mass: f64, age: f64) -> SystemStars {
        let t = template();
        let record = SystemRecord::from_parts(
            t.id(),
            *t.epoch_position(),
            SystemOrigin::Grid(t.component().expect("a grid record")),
            t.population(),
            SolarMasses::new(mass),
            Years::new(age),
        );
        SystemStars::generate(galaxy(), &record)
    }

    fn years(y: i64) -> UniverseTime {
        UniverseTime::from_julian_years(y).expect("inside the clock")
    }

    fn answer(stars: &SystemStars, y: i64) -> SystemSummaryDto {
        let time = years(y);
        let request = SystemSummaryRequest {
            universe: UniverseIdHex::from_u64(7),
            system: SystemIdHex::from_u64(stars.record().id().raw()),
            time: wire_time(time),
        };
        system_summary(request, stars, time)
    }

    #[test]
    fn a_system_not_yet_born_has_no_stars_no_nodes_and_a_negative_age() {
        let stars = system(1.0, -300.0);
        let before = answer(&stars, 0);
        assert_eq!(before.existence, SystemExistenceDto::NotYetBorn);
        assert!(before.stars.is_empty() && before.hierarchy.nodes.is_empty());
        assert!((before.age_myr + 3e-4).abs() < 1e-15, "{}", before.age_myr);
        let after = answer(&stars, 400);
        assert_eq!(after.existence, SystemExistenceDto::Exists);
        assert_eq!(after.stars.len(), usize::from(stars.star_count()));
        // Every star is a node, and every other node a pair: n stars, n − 1 orbits.
        assert_eq!(after.hierarchy.nodes.len(), 2 * after.stars.len() - 1);
    }

    #[test]
    fn a_dark_remnant_has_no_temperature_and_a_white_dwarf_its_cooling_age() {
        let dead = system(2.0, 5.0e9);
        let primary = &answer(&dead, 0).stars[0];
        assert_eq!(primary.kind, ObjectKindDto::WhiteDwarf);
        let died = dead
            .primary()
            .lifetime()
            .expect("a star of 2 M☉ dies")
            .value();
        match primary.remnant {
            Some(RemnantDto::WhiteDwarf {
                cooling_age_myr,
                natal_kick:
                    Some(NatalKickDto {
                        speed_km_s,
                        mode: KickModeDto::WhiteDwarf,
                    }),
            }) => {
                assert!(
                    (cooling_age_myr - (5.0e9 - died) / 1e6).abs() < 1e-6,
                    "{cooling_age_myr} Myr"
                );
                // A Maxwellian of σ = 1 km/s (P06.T19.c).
                assert!((0.0..10.0).contains(&speed_km_s), "{speed_km_s} km/s");
            }
            ref other => panic!("expected a white dwarf, got {other:?}"),
        }
        assert!(primary.teff_k.is_some());
        // Among the heaviest stars some leave black holes, which have no light.
        let hole = [25.0, 30.0, 40.0, 60.0, 80.0, 100.0, 120.0, 150.0]
            .into_iter()
            .map(|m| answer(&system(m, 2.0e8), 0).stars[0].clone())
            .find(|star| star.kind == ObjectKindDto::BlackHole)
            .expect("one of eight massive stars leaves a black hole");
        assert_eq!(hole.teff_k, None);
        assert_eq!(hole.luminosity_lsun.to_bits(), 0);
        assert_eq!(hole.phase, PhaseDto::BlackHole);
        assert!(matches!(
            hole.remnant,
            Some(RemnantDto::BlackHole {
                dimensionless_spin: None,
                natal_kick: Some(NatalKickDto {
                    mode: KickModeDto::Ordinary | KickModeDto::FallbackNone,
                    ..
                })
            })
        ));
    }

    /// An object on the cooling fits reads as its hydrogen burning makes it: a star above the
    /// hydrogen-burning limit, a dwarf, is on the main sequence, and a brown dwarf below it is
    /// substellar.
    #[test]
    fn a_hydrogen_burning_star_on_the_cooling_fits_is_on_the_main_sequence() {
        for mass in [0.09, 0.0999] {
            let star = &answer(&system(mass, 5.0e9), 0).stars[0];
            assert_eq!(star.kind, ObjectKindDto::Dwarf, "{mass} M☉");
            assert_eq!(star.phase, PhaseDto::MainSequence, "{mass} M☉");
        }
        let brown = &answer(&system(0.05, 5.0e9), 0).stars[0];
        assert_eq!(brown.kind, ObjectKindDto::Substellar);
        assert_eq!(brown.phase, PhaseDto::Substellar);
        // The track's own ends: from 0.1 M☉ the backbone's main sequence, unchanged.
        let backbone = &answer(&system(0.1, 5.0e9), 0).stars[0];
        assert_eq!(backbone.phase, PhaseDto::MainSequence);
    }

    /// What this generator version does not compute is absent from the wire, not `null`; what it
    /// computes and the star lacks is `null`.
    #[test]
    fn what_is_not_computed_is_absent_and_what_is_lacking_is_null() {
        let living = answer(&system(1.0, 4.6e9), 0);
        let wire = serde_json::to_value(&living.stars[0]).expect("a summary serialises");
        let fields = wire.as_object().expect("a star is an object");
        for absent in [
            "rotation_period_d",
            "activity_log_lx_lbol",
            "variability",
            "planetary_nebula",
            "active_events",
        ] {
            assert!(!fields.contains_key(absent), "{absent} is sent: {wire}");
        }
        for null in ["remnant", "death_time"] {
            assert_eq!(fields.get(null), Some(&serde_json::Value::Null), "{null}");
        }
        assert_eq!(fields.get("body_index"), Some(&serde_json::json!(0)));
    }

    #[test]
    fn an_orbit_carries_every_element_and_mu_bit_for_bit() {
        let mut cell = Vec::new();
        generate_cell(
            galaxy(),
            CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid"),
            &mut cell,
        );
        let multiple = cell
            .iter()
            .map(|record| SystemStars::generate(galaxy(), record))
            .find(|stars| stars.star_count() > 1)
            .expect("a cell at the solar circle holds a multiple system");
        let wire = hierarchy(multiple.hierarchy());
        let mut orbits = 0;
        for (node, sim) in wire.nodes.iter().zip(multiple.hierarchy().nodes()) {
            match (node, sim) {
                (HierarchyNodeDto::Pair { orbit, .. }, HierarchyNode::Pair { orbit: sim, .. }) => {
                    orbits += 1;
                    let pairs = [
                        (orbit.period_s, sim.period().value()),
                        (orbit.semi_major_axis_m, sim.semi_major_axis().value()),
                        (orbit.eccentricity, sim.eccentricity().value()),
                        (orbit.inclination_rad, sim.inclination().value()),
                        (orbit.ascending_node_rad, sim.ascending_node().value()),
                        (
                            orbit.argument_of_periapsis_rad,
                            sim.argument_of_periapsis().value(),
                        ),
                        (
                            orbit.mean_anomaly_at_epoch_rad,
                            sim.mean_anomaly_at_epoch().value(),
                        ),
                        (orbit.mu_m3_s2, sim.gravitational_parameter().value()),
                    ];
                    for (wire, sim) in pairs {
                        assert_eq!(wire.to_bits(), sim.to_bits());
                    }
                }
                (
                    HierarchyNodeDto::Star {
                        body_index,
                        mass_msun,
                    },
                    HierarchyNode::Star(index),
                ) => {
                    let slot = multiple.hierarchy().star(*index);
                    assert_eq!(*body_index, slot.body().body_index());
                    assert_eq!(mass_msun.to_bits(), slot.initial_mass().value().to_bits());
                }
                (node, sim) => panic!("{node:?} is not {sim:?}"),
            }
        }
        assert_eq!(orbits, usize::from(multiple.star_count()) - 1);
    }
}
