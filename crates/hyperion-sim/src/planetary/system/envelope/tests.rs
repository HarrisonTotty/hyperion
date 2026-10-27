//! The envelope section on the record (P14.T54.b.i): held at 23, so that nothing of it is computed;
//! under every 24 hold with the record's batch rules, every sub-Neptune's and giant's envelope is
//! its builder's on its record's own members, or not modelled for a stated reason, over the golden
//! systems and the plan-14 sample; the table's Jupiter against the probe; the ice giants' methane
//! decks (G3); the census of the bless note's lines (ignored).

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::Seed;
use crate::math;
use crate::planetary::chem::condense::DeckBound;
use crate::planetary::chem::escape::{EnvelopeHelium, host_helium_per_hydrogen};
use crate::planetary::chem::{AbundanceDraws, HostAbundances};
use crate::planetary::derive::solar::{PLANETS, SOLAR_AGE, solar_disc, solar_system};
use crate::planetary::derive::{DerivedBody, PlanetClass};
use crate::planetary::hooks::cold_traps::ColdTrapRules;
use crate::planetary::hooks::envelope_atmosphere::{DECK_COLUMN_BOTTOM, DeckKind};
use crate::planetary::hooks::structure::envelope::{
    ADIABAT_BOTTOM, MEAN_REDISTRIBUTION, OPACITY_RATIO, OPTICAL_DEPTH_EXPONENT,
    THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG,
};
use crate::planetary::hooks::surface::HourCosines;
use crate::planetary::label;
use crate::planetary::record::{BodyRecord, RecordSection, SectionState};
use crate::planetary::system::tests::{SEED, golden_systems_afresh, sample, thin_air_sets};
use crate::planetary::system::{Body, Epoch, Part, PlanetarySystem, RecordRules, fate_host};
use crate::planetary::testing::with_every_24_hold;
use crate::planetary::{SystemContext, generate};
use crate::stellar::Composition;
use crate::substance::{Element, SubstanceId, by_key};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::METRES_PER_AU;
use crate::units::{JupiterMasses, Pascals};

/// The row whose key is `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

/// The row's key.
fn key_of(id: SubstanceId) -> String {
    id.substance().key().as_str().to_owned()
}

/// The registry's network, built once for the tests (P14.T52.a: build it once).
fn network() -> &'static Network {
    static NETWORK: OnceLock<Network> = OnceLock::new();
    NETWORK.get_or_init(|| Network::registry().expect("the registry's network builds"))
}

/// The record's rules at their batch (the air's sections, the cold traps), as every 24 test reads
/// them; read inside [`with_every_24_hold`], whose small bodies' rules they take.
fn batch_rules() -> RecordRules {
    RecordRules {
        cold_traps: ColdTrapRules::Batch,
        air: AirRules::Batch,
        ..RecordRules::in_force()
    }
}

/// One record of a walk ([`walk`]): the body, its record, and for a present planet or member its
/// derivation and its next change.
struct Visit<'a> {
    body: &'a Body,
    record: BodyRecord,
    derived: Option<DerivedBody>,
    changes_at: Option<UniverseTime>,
}

/// Every planet's, member's and moon's record of `system` (of `ctx`) at `t`, as
/// `PlanetarySystem::snapshot_at` builds it (one epoch, its shades and small bodies, one table of
/// cosines), each given to `visit` with the epoch: a system not born at `t` gives none.
fn walk(
    ctx: &SystemContext,
    system: &PlanetarySystem,
    t: UniverseTime,
    visit: &mut dyn FnMut(&Epoch<'_>, Visit<'_>),
) {
    if ctx.age_at(t).value() <= 0.0 {
        return;
    }
    let epoch = Epoch::new(system, ctx, t)
        .with_shades()
        .with_small_bodies(system.record.small_bodies);
    let cosines = HourCosines::new();
    for body in system.bodies() {
        if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
            continue;
        }
        let host = fate_host(ctx, system.zone_of(body));
        let fate = system.fate_of(body, &host);
        let label = label::label(system, body.index).expect("every body has a label");
        let (record, now) = system.primary_record(&epoch, &cosines, body, label, &fate);
        let (derived, changes_at) = (now.derived, now.changes_at);
        for child in system.children(body.index) {
            if !matches!(child.part, Part::Moon(_)) {
                continue;
            }
            let label = label::label(system, child.index).expect("every body has a label");
            let moon = system.satellite_record(&epoch, &cosines, child, label, &now, &fate);
            visit(
                &epoch,
                Visit {
                    body: child,
                    record: moon,
                    derived: None,
                    changes_at: None,
                },
            );
        }
        visit(
            &epoch,
            Visit {
                body,
                record,
                derived,
                changes_at,
            },
        );
    }
}

/// What the envelope of the present planet or member `body` reads, rebuilt from its record
/// `record`'s own sections where it publishes them (its mass, and its bulk section's fractions,
/// radius, gravity and equilibrium temperature) and from its derivation `derived` (its internal
/// luminosity and its carried helium), its system's host and the epoch's time and light: the
/// record's builder's, by another reading.
fn rebuilt(
    epoch: &Epoch<'_>,
    system: &PlanetarySystem,
    body: &Body,
    (record, derived): (&BodyRecord, &DerivedBody),
) -> EnvelopeOf {
    let ctx = epoch.ctx;
    let bulk = record.bulk().ok().expect("a present primary's bulk");
    EnvelopeOf {
        mass: *record.mass().ok().expect("a present primary's mass"),
        fractions: bulk.fractions(),
        radius: Metres::from(bulk.radius()),
        gravity: bulk.surface_gravity(),
        equilibrium_temperature: bulk.equilibrium_temperature(),
        internal_luminosity: derived.internal_luminosity(),
        age: ctx.age_at(epoch.t),
        formation: system.formation_zone(body),
        fe_h: ctx.fe_h(),
        alpha_fe: ctx.alpha_fe(),
        host: HostAbundances::of(
            ctx.fe_h(),
            &AbundanceDraws::for_system(system.seed, system.system),
        ),
        primordial_helium_per_hydrogen: derived.envelope_helium().map_or_else(
            || host_helium_per_hydrogen(ctx.composition()),
            EnvelopeHelium::helium_to_hydrogen,
        ),
        starlit: epoch
            .photochemistry_temperature(epoch.brightest(system.zone_of(body)))
            .is_some(),
    }
}

/// A refusal's name, for the counts.
fn reason(error: &DeriveEnvelopeError) -> &'static str {
    use DeriveEnvelopeAtmosphereError as E;
    match error {
        DeriveEnvelopeError::NoAlphaFe => "no [α/Fe]",
        DeriveEnvelopeError::NoDisc => "no disc",
        DeriveEnvelopeError::FormationZone(_) => "formation zone",
        DeriveEnvelopeError::InternalTemperature(_) => "internal temperature",
        DeriveEnvelopeError::Abundances(_) => "abundances",
        DeriveEnvelopeError::Network(_) => "network",
        DeriveEnvelopeError::Atmosphere(E::NotHydrogenDominated { .. }) => "not hydrogen-dominated",
        DeriveEnvelopeError::Atmosphere(E::LevelAboveRecords { .. }) => "level above records",
        DeriveEnvelopeError::Atmosphere(
            E::InternalTemperature(_)
            | E::EquilibriumTemperature(_)
            | E::Gravity(_)
            | E::FeH(_)
            | E::Metallicity(_)
            | E::Speciation { .. }
            | E::Profile(_)
            | E::Quench(_)
            | E::Gas { .. }
            | E::Condensation(_)
            | E::Sedimentation(_)
            | E::Rule(_)
            | E::Mode(_)
            | E::Absorber(_)
            | E::Mixture,
        ) => "a step's refusal",
    }
}

/// Asserts what every `Ok` envelope holds (the design's §4 item 1), the envelope `envelope` of the
/// body `at` whose inputs were `of`: no supercritical condensable; the adopted grey profile (κ₁
/// 2.87 × 10⁻³ m² kg⁻¹, n 2, γ 0.4, f ¼); `T_int` finite and positive; `T_irr`⁴ = 4 `T_eq`⁴ (f ¼,
/// so that the wire's `T_irr`⁴ = `T_eq`⁴ ÷ f) to 10⁻¹²; a finite join, the adiabat's nodes
/// ascending from it to at least the larger of 100 bar and e^0.3 times its pressure; the 1-bar
/// composition summing to 1 to 10⁻⁹, largest first, each at least 10⁻¹²; every mode covering the
/// whole sky; the deck modes first, each naming a listed deck, then the hazes, each of its own
/// material and naming none; each deck's condensate once; the hazes' τ their modes' sum to the
/// bit; and the abundances' He/H its input's times its rain's depletion, to the bit.
fn check_envelope(at: &str, envelope: &Envelope, of: &EnvelopeOf) {
    assert_eq!(envelope.supercritical(), None, "{at}");
    let atmosphere = envelope.atmosphere();
    let profile = atmosphere.profile();
    let grey = profile.grey();
    assert_same_bits(
        grey.thermal_opacity_m2_per_kg(),
        THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG,
    );
    assert!(
        (grey.thermal_opacity_m2_per_kg() - 2.87e-3).abs() < 5e-6,
        "{at}"
    );
    assert_same_bits(grey.optical_depth_exponent(), OPTICAL_DEPTH_EXPONENT);
    assert_same_bits(grey.optical_depth_exponent(), 2.0);
    assert_same_bits(grey.opacity_ratio(), OPACITY_RATIO);
    assert_same_bits(grey.opacity_ratio(), 0.4);
    assert_same_bits(grey.redistribution(), MEAN_REDISTRIBUTION);
    assert_same_bits(grey.redistribution(), 0.25);
    let t_int = atmosphere.internal_temperature().value();
    assert!(t_int.is_finite() && t_int > 0.0, "{at}: T_int {t_int}");
    let irradiation = math::powi(atmosphere.irradiation_temperature().value(), 4);
    let expected = 4.0 * math::powi(of.equilibrium_temperature.value(), 4);
    assert!(
        (irradiation - expected).abs() <= 1e-12 * expected,
        "{at}: T_irr⁴ {irradiation} against 4 T_eq⁴ {expected}"
    );
    let join = profile.join_pressure().value();
    assert!(join.is_finite() && join > 0.0, "{at}: join {join}");
    let nodes = profile.nodes();
    assert_same_bits(nodes[0].pressure().value(), join);
    for pair in nodes.windows(2) {
        assert!(pair[1].pressure() > pair[0].pressure(), "{at}: {pair:?}");
    }
    let deepest = nodes.last().expect("an adiabat's nodes").pressure().value();
    let floor = ADIABAT_BOTTOM.value().max(math::exp(0.3) * join);
    assert!(
        deepest >= floor * (1.0 - 1e-12),
        "{at}: {deepest} Pa against {floor}"
    );
    let composition = atmosphere.composition();
    let total: f64 = composition.iter().map(|&(_, x)| x).sum();
    assert!(
        (total - 1.0).abs() < 1e-9,
        "{at}: the composition sums to {total}"
    );
    for pair in composition.windows(2) {
        assert!(pair[0].1 >= pair[1].1, "{at}: largest first");
    }
    assert!(composition.iter().all(|&(_, x)| x >= 1e-12), "{at}");
    // By a loop: the deck's type is the inventory scan's to name (`hooks::aerosols`).
    let mut decks: Vec<SubstanceId> = Vec::with_capacity(atmosphere.decks().len());
    for deck in atmosphere.decks() {
        decks.push(deck.condensate());
    }
    let mut once = decks.clone();
    once.sort_unstable();
    once.dedup();
    assert_eq!(once.len(), decks.len(), "{at}: a condensate's deck once");
    let modes = atmosphere.modes();
    let split = modes
        .iter()
        .position(|mode| !decks.contains(&mode.deck()))
        .unwrap_or(modes.len());
    let mut haze_tau = 0.0;
    for (k, mode) in modes.iter().enumerate() {
        assert_same_bits(mode.coverage(), 1.0);
        if k < split {
            assert!(decks.contains(&mode.deck()), "{at}: deck mode {k}");
        } else {
            assert!(!decks.contains(&mode.deck()), "{at}: haze {k} names a deck");
            assert_eq!(mode.deck(), mode.material(), "{at}: haze {k}");
            if let Ok(depth) = mode.optical_depth() {
                haze_tau += depth.value();
            }
        }
    }
    assert!(split >= decks.len(), "{at}: every deck has a mode");
    assert_same_bits(atmosphere.haze_optical_depth(), haze_tau);
    let abundances = envelope.abundances();
    assert_same_bits(
        abundances.per_hydrogen(Element::He),
        of.primordial_helium_per_hydrogen * abundances.helium_depletion(),
    );
}

/// The temperature at `bar` bar of `envelope`'s profile, K.
fn at_bar(envelope: &Envelope, bar: f64) -> f64 {
    envelope
        .atmosphere()
        .profile()
        .temperature_at(Pascals::new(bar * 1e5))
        .value()
}

/// The figures of an envelope that continuity compares, and its inputs: its state, its `T_int`,
/// `T_irr`, join and μ with its three largest shares, and its record's mass, radius, gravity,
/// `T_eq`, internal luminosity, age and He/H.
#[derive(Debug, Clone, PartialEq)]
struct Figures {
    state: SectionState,
    outputs: Vec<(String, f64)>,
    inputs: [f64; 7],
}

impl Figures {
    fn of(record: &BodyRecord, of: &EnvelopeOf) -> Self {
        let outputs = match record.envelope() {
            Section::Ok(envelope) => {
                let atmosphere = envelope.atmosphere();
                let mut outputs = vec![
                    (
                        "T_int".to_owned(),
                        atmosphere.internal_temperature().value(),
                    ),
                    (
                        "T_irr".to_owned(),
                        atmosphere.irradiation_temperature().value(),
                    ),
                    (
                        "join".to_owned(),
                        atmosphere.profile().join_pressure().value(),
                    ),
                    ("mu".to_owned(), atmosphere.mean_molar_mass_g_per_mol()),
                ];
                outputs.extend(
                    atmosphere
                        .composition()
                        .iter()
                        .take(3)
                        .map(|&(id, x)| (key_of(id), x)),
                );
                outputs.extend(atmosphere.decks().iter().map(|deck| {
                    (
                        format!("deck {}", key_of(deck.condensate())),
                        deck.base().value(),
                    )
                }));
                outputs
            }
            Section::NotResolved | Section::NotModelled | Section::NotApplicable => Vec::new(),
        };
        Self {
            state: record.section_state(RecordSection::Envelope),
            outputs,
            inputs: [
                of.mass.value(),
                of.radius.value(),
                of.gravity.value(),
                of.equilibrium_temperature.value(),
                of.internal_luminosity.value(),
                of.age.value(),
                of.primordial_helium_per_hydrogen,
            ],
        }
    }
}

/// The relative move from `a` to `b`, 0 where both are 0 (each below the smallest normal float).
fn relative(a: f64, b: f64) -> f64 {
    if a.abs() < f64::MIN_POSITIVE && b.abs() < f64::MIN_POSITIVE {
        0.0
    } else {
        ((b - a) / a).abs()
    }
}

/// Under the air's sections' hold the record's envelope is not modelled, and neither its inputs
/// nor its network are read (`record_envelope`'s `Held` arm).
#[test]
fn the_held_envelope_computes_nothing() {
    let held = record_envelope(
        AirRules::Held,
        || panic!("nothing of the envelope is read under the hold"),
        || panic!("no network is built under the hold"),
    );
    assert_eq!(held, Section::NotModelled);
}

/// Every 24 hold in force and the record's rules at their batch, over the golden systems and the
/// sample's first 10 systems at both ends of the clock window: every present sub-Neptune's and
/// giant's envelope is `Ok` and equal to `envelope_of` rebuilt from its record's own members, to
/// the bit, with every invariant of [`check_envelope`], or `NotModelled` for a reason
/// `envelope_of` gives, not hydrogen-dominated or a level above the records (counted), every other
/// reason none; every solid class's is not applicable, or not modelled under a supercritical
/// envelope (P14.T53.b part 2, counted), its surface then not applicable. A sub-Neptune's envelope
/// is `Ok` with its surface not applicable, and a rocky body's envelope not applicable (P14.T24.d's
/// moved record tests). Every body present at both ends keeps its envelope's state, and its
/// `T_int`, `T_irr`, join, μ and three largest shares move by at most 10 times its inputs' largest
/// relative move (and 10⁻¹²), its decks' bases stated.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one walk with the section's every invariant and its continuity across the window"
)]
fn under_every_24_hold_every_gas_envelope_record_is_its_builder_s_or_not_modelled() {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut figures: std::collections::BTreeMap<(String, u16), Vec<Figures>> =
        std::collections::BTreeMap::new();
    let (mut sub_neptunes, mut rocky) = (0_usize, 0_usize);
    with_every_24_hold(|| {
        let rules = batch_rules();
        let sets: Vec<(String, SystemContext, PlanetarySystem)> = golden_systems_afresh()
            .into_iter()
            .map(|(name, ctx, system)| (name.to_owned(), ctx, system))
            .chain(
                sample()
                    .iter()
                    .take(10)
                    .map(|ctx| (format!("{:?}", ctx.id()), ctx.clone(), generate(SEED, ctx))),
            )
            .collect();
        for (name, ctx, system) in &sets {
            let system = system.clone().with_record_rules(rules);
            for t in [ClockWindow::START, ClockWindow::END] {
                walk(ctx, &system, t, &mut |epoch, visit| {
                    let record = &visit.record;
                    let Section::Ok(bulk) = record.bulk() else {
                        assert_ne!(
                            record.section_state(RecordSection::Envelope),
                            SectionState::Ok
                        );
                        return;
                    };
                    let at = format!("{name} {:04x} at {t:?}", visit.body.index.get());
                    let envelope = record.envelope();
                    let solid = bulk.class().has_solid_surface();
                    if solid {
                        match envelope {
                            Section::NotApplicable => {
                                if bulk.class() == PlanetClass::Rocky {
                                    rocky += 1;
                                }
                            }
                            Section::NotModelled => {
                                assert_eq!(
                                    record.section_state(RecordSection::Surface),
                                    SectionState::NotApplicable,
                                    "{at}"
                                );
                                *counts.entry("solid, supercritical".to_owned()).or_default() += 1;
                            }
                            Section::Ok(_) | Section::NotResolved => {
                                panic!("{at}: a solid class's envelope {envelope:?}")
                            }
                        }
                        return;
                    }
                    assert_eq!(
                        record.section_state(RecordSection::Surface),
                        SectionState::NotApplicable,
                        "{at}"
                    );
                    let derived = visit
                        .derived
                        .as_ref()
                        .expect("a present primary's derivation");
                    let of = rebuilt(epoch, &system, visit.body, (record, derived));
                    if let Some(helium) = derived.envelope_helium() {
                        assert_same_bits(
                            of.primordial_helium_per_hydrogen,
                            helium.helium_to_hydrogen(),
                        );
                    }
                    let again = envelope_of(&of, network());
                    match (envelope, &again) {
                        (Section::Ok(envelope), Ok(again)) => {
                            assert_eq!(format!("{envelope:?}"), format!("{again:?}"), "{at}");
                            check_envelope(&at, envelope, &of);
                            *counts.entry(format!("{:?} ok", bulk.class())).or_default() += 1;
                            if bulk.class() == PlanetClass::SubNeptune {
                                sub_neptunes += 1;
                            }
                        }
                        (Section::NotModelled, Err(error)) => {
                            let why = reason(error);
                            assert!(
                                matches!(why, "not hydrogen-dominated" | "level above records"),
                                "{at}: {error:?}"
                            );
                            *counts
                                .entry(format!("{:?} {why}", bulk.class()))
                                .or_default() += 1;
                        }
                        (section, again) => {
                            panic!("{at}: the record's {section:?} against {again:?}")
                        }
                    }
                    figures
                        .entry((name.clone(), visit.body.index.get()))
                        .or_default()
                        .push(Figures::of(record, &of));
                });
            }
        }
    });
    let mut deck_moves = 0.0_f64;
    let mut compared = 0_usize;
    for ((name, index), both) in &figures {
        let [start, end] = both.as_slice() else {
            continue;
        };
        let at = format!("{name} {index:04x}");
        assert_eq!(start.state, end.state, "{at}: the envelope's section flips");
        let largest = start
            .inputs
            .iter()
            .zip(&end.inputs)
            .map(|(&a, &b)| relative(a, b))
            .fold(0.0_f64, f64::max);
        for (label, a) in &start.outputs {
            let Some(&(_, b)) = end.outputs.iter().find(|(other, _)| other == label) else {
                assert!(
                    !label.starts_with("deck"),
                    "{at}: {label} lost across the window"
                );
                continue;
            };
            let moved = relative(*a, b);
            if label.starts_with("deck") {
                deck_moves = deck_moves.max(moved);
            } else {
                assert!(
                    moved <= 10.0 * largest + 1e-12,
                    "{at}: {label} moves {moved:e} against its inputs' {largest:e}"
                );
            }
        }
        compared += 1;
    }
    eprintln!(
        "the envelopes by class and reason: {counts:?}; {compared} compared across the window, \
         the largest deck base move {deck_moves:e}"
    );
    assert!(sub_neptunes > 0 && rocky > 0, "{sub_neptunes} {rocky}");
    assert!(compared > 10, "{compared}");
    for (label, n) in &counts {
        assert!(
            label.ends_with(" ok")
                || label.ends_with("not hydrogen-dominated")
                || label.ends_with("level above records")
                || label == "solid, supercritical",
            "{label}: {n}"
        );
    }
}

/// The record's envelope is the same for the same seed twice, and a body's own record
/// (`PlanetarySystem::body_at`) holds the snapshot's envelope (`snapshot_at`), each to the bit
/// (signed zeros included, by `Debug`), every 24 hold in force and the record's rules at their
/// batch, over three golden systems with giants; and `body_at` gives each body the same envelope
/// whatever order its system's bodies are asked in, so that the escape batch's carried helium, read
/// by each envelope, keeps nothing between bodies (the sim-determinism skill's call-order rule).
#[test]
fn under_every_24_hold_the_record_s_envelope_is_the_same_twice_and_in_both_queries() {
    let mut envelopes = 0;
    with_every_24_hold(|| {
        let rules = batch_rules();
        let systems: Vec<_> = golden_systems_afresh()
            .into_iter()
            .filter(|(name, ..)| matches!(*name, "hot_jupiter" | "solar_like" | "eccentric_giant"))
            .collect();
        assert_eq!(systems.len(), 3);
        for (name, ctx, system) in systems {
            let system = system.with_record_rules(rules);
            let again = generate(system.seed, &ctx).with_record_rules(rules);
            let t = UniverseTime::EPOCH;
            let snapshot = system.snapshot_at(&ctx, t);
            let twice = again.snapshot_at(&ctx, t);
            assert_eq!(snapshot, twice, "{name}");
            for (record, other) in snapshot.bodies().iter().zip(twice.bodies()) {
                assert_eq!(
                    format!("{:?}", record.envelope()),
                    format!("{:?}", other.envelope()),
                    "{name} {:?}",
                    record.index()
                );
                if record.section_state(RecordSection::Envelope) != SectionState::Ok {
                    continue;
                }
                let alone = system.body_at(&ctx, record.index(), t).unwrap();
                assert_eq!(&alone, record, "{name} {:?}", record.index());
                assert_eq!(
                    format!("{:?}", alone.envelope()),
                    format!("{:?}", record.envelope()),
                    "{name} {:?}",
                    record.index()
                );
                envelopes += 1;
            }
            let indices = system.indices();
            assert_order_independent(&indices, |&index| {
                format!(
                    "{:?}",
                    system
                        .body_at(&ctx, index, t)
                        .expect("a body of its system")
                        .envelope()
                )
            });
        }
    });
    assert!(envelopes > 2, "{envelopes} envelopes");
}

/// The table's giant `name` as its record's envelope reads it (the Solar System through
/// `derive_body` about the present Sun, every 24 hold in force, so that its derivation carries its
/// escape's helium from the protosolar start, its host's): its derivation's mass, fractions,
/// radius, gravity, `T_eq` and internal luminosity, the Sun's age, its formation at its own
/// semi-major axis in the zero-age Sun's disc, a protosolar host (\[Fe/H\] and \[α/Fe\] 0,
/// P14.T50.a's protosolar C/O and Mg/Si, as P14.T52.d's table cases take it), starlit.
fn table_envelope(name: &str) -> (EnvelopeOf, Envelope) {
    let &(_, _, _, a, _) = PLANETS
        .iter()
        .find(|(n, ..)| *n == name)
        .expect("the giant is in the table");
    let (_, body) = solar_system()
        .into_iter()
        .find(|(n, _)| *n == name)
        .expect("the giant is in the table");
    let of = EnvelopeOf {
        mass: body.mass(),
        fractions: body.fractions(),
        radius: Metres::from(body.radius()),
        gravity: body.surface_gravity(),
        equilibrium_temperature: body.equilibrium_temperature(),
        internal_luminosity: body.internal_luminosity(),
        age: SOLAR_AGE,
        formation: FormationZone::new(Metres::new(a * METRES_PER_AU), solar_disc().snow_line())
            .ok(),
        fe_h: Dex::ZERO,
        alpha_fe: Some(Dex::ZERO),
        host: HostAbundances::protosolar(),
        primordial_helium_per_hydrogen: body.envelope_helium().map_or_else(
            || host_helium_per_hydrogen(&Composition::SOLAR),
            EnvelopeHelium::helium_to_hydrogen,
        ),
        starlit: true,
    };
    let envelope = envelope_of(&of, network()).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    (of, envelope)
}

/// The share of the row `key` in `envelope`'s 1-bar gas.
fn share(envelope: &Envelope, key: &str) -> f64 {
    let id = row(key);
    envelope
        .atmosphere()
        .composition()
        .iter()
        .filter(|(s, _)| *s == id)
        .map(|&(_, x)| x)
        .sum()
}

/// The table's Jupiter, through the record's envelope (`envelope_of`) on its own derivation about
/// a solar host, every 24 hold in force, follows the Galileo probe as P14.T52.d's table case does:
/// T(1 bar) within 10% of the probe's 166.1 K (Seiff et al. 1998), its ammonia deck's base at
/// 0.5–1 bar and CH₄ ÷ H₂ at 1.5–3 × 10⁻³ (Wong et al. 2004's 2.4 × 10⁻³, as Guillot et al. 2022's
/// Table 2 lists it); its join and photosphere stated. Its He/H is its carry's, fractionated from
/// a solar host's start, the protosolar Y: 0.096818, 1.0 × 10⁻⁴ below the registry's n☉ (0.096828),
/// since the carry's He/H (`EnvelopeHelium::helium_to_hydrogen`) reads hydrogen and helium at the
/// gas rows' molecular masses and the start's Y at the standard atomic weights (stated, to the lane:
/// the two scales of one figure).
#[test]
fn the_table_s_jupiter_on_the_record_follows_the_probe() {
    let (of, jupiter) = with_every_24_hold(|| table_envelope("Jupiter"));
    check_envelope("Jupiter", &jupiter, &of);
    let one_bar = at_bar(&jupiter, 1.0);
    let ammonia = jupiter
        .atmosphere()
        .decks()
        .iter()
        .find(|deck| deck.condensate() == row("NH3"))
        .expect("an ammonia deck")
        .base()
        .value()
        / 1e5;
    let methane = share(&jupiter, "CH4") / share(&jupiter, "H2");
    let profile = jupiter.atmosphere().profile();
    eprintln!(
        "state: the table's Jupiter on the record: T(1 bar) {one_bar:.2} K, T_int {:.2} K, T_irr \
         {:.2} K, join {:.4} bar, photosphere {:.4} bar, NH3 base {ammonia:.3} bar, CH4/H2 \
         {methane:.4e}, He/H {:.6} (depletion {:.4}), {} decks, {} modes, haze tau {:.4}",
        jupiter.atmosphere().internal_temperature().value(),
        jupiter.atmosphere().irradiation_temperature().value(),
        profile.join_pressure().value() / 1e5,
        profile.photosphere_pressure().value() / 1e5,
        of.primordial_helium_per_hydrogen,
        jupiter.abundances().helium_depletion(),
        jupiter.atmosphere().decks().len(),
        jupiter.atmosphere().modes().len(),
        jupiter.atmosphere().haze_optical_depth(),
    );
    assert!((one_bar - 166.1).abs() < 16.61, "T(1 bar) {one_bar} K");
    assert!((0.5..1.0).contains(&ammonia), "NH3's base at {ammonia} bar");
    assert!((1.5e-3..3e-3).contains(&methane), "CH4/H2 {methane}");
    assert!(
        (of.primordial_helium_per_hydrogen / Element::He.protosolar_abundance_per_hydrogen() - 1.0)
            .abs()
            < 2e-4,
        "{}",
        of.primordial_helium_per_hydrogen
    );
}

/// R08's cloud-deck split, `CLOUD_DECK_SPLIT_OPTICAL_DEPTH`, 10 (decision-bump-plan-after-22 G3):
/// a deck of an envelope, which covers the whole sky, above it would classify its planet
/// `cloudDeck`.
const CLOUD_DECK_SPLIT_OPTICAL_DEPTH: f64 = 10.0;

/// G3 (decision-bump-plan-after-22): the table's Neptune's and Uranus's CH₄ decks through the
/// record's envelope, every 24 hold in force, each below R08's cloud-deck split of 10 and within 3
/// times its range's top (G2's 0.5–4 for Neptune and 1–5 for Uranus, Irwin et al. 2022), stated:
/// P14.T52.d's table cases at the protosolar He/H gave 5.77 and 3.76, which the carry's He/H, the
/// escape's from the protosolar start, moves slightly.
#[test]
fn the_ice_giants_methane_decks_meet_g3() {
    for (name, top) in [("Neptune", 4.0), ("Uranus", 5.0)] {
        let (of, envelope) = with_every_24_hold(|| table_envelope(name));
        check_envelope(name, &envelope, &of);
        let tau = envelope
            .atmosphere()
            .deck_optical_depth(row("CH4"))
            .unwrap_or_else(|| panic!("{name}: a CH4 deck with a tau"));
        let base = envelope
            .atmosphere()
            .decks()
            .iter()
            .find(|deck| deck.condensate() == row("CH4"))
            .expect("a CH4 deck")
            .base()
            .value()
            / 1e5;
        eprintln!(
            "state: G3 {name}: CH4 tau {tau:.4} (range top {top}, split 10), base {base:.3} bar, \
             He/H {:.6}, T(1 bar) {:.2} K, T_int {:.2} K",
            of.primordial_helium_per_hydrogen,
            at_bar(&envelope, 1.0),
            envelope.atmosphere().internal_temperature().value()
        );
        assert!(
            tau < CLOUD_DECK_SPLIT_OPTICAL_DEPTH,
            "TO MAIN: {name}'s CH4 tau {tau}"
        );
        assert!(
            tau < 3.0 * top,
            "TO MAIN: {name}'s CH4 tau {tau} against 3 x {top}"
        );
    }
}

/// The census's sets (§4 item 2): `thin_air_sets`' first 60 sample systems (the helper holds the
/// sample's 400), its 15 golden systems and its 27 LOOK72 systems, as the other 24 censuses take
/// them (P14.T54.b.g's, h's).
fn census_sets() -> Vec<(&'static str, Seed, SystemContext)> {
    let mut samples = 0;
    thin_air_sets()
        .into_iter()
        .filter(|(set, ..)| {
            if *set == "sample" {
                samples += 1;
                samples <= 60
            } else {
                true
            }
        })
        .collect()
}

/// τ(550) of the modes of `envelope` naming the deck `condensate` that lies above the level
/// `level` (Pa): each mode's mixing ratio as (p ÷ `p_b`)^`f_sed`, so that its τ above p is τ ((p ÷
/// `p_b`)^(f+1) − (`p_t` ÷ `p_b`)^(f+1)) ÷ (1 − (`p_t` ÷ `p_b`)^(f+1)), 0 above its top and all of
/// it below its base (decision-deep-decks-split §8.1); `None` where none carries a τ.
fn deck_tau_above(envelope: &Envelope, condensate: SubstanceId, level: f64) -> Option<f64> {
    let mut total = None;
    for mode in envelope
        .atmosphere()
        .modes()
        .iter()
        .filter(|mode| mode.deck() == condensate)
    {
        let Ok(depth) = mode.optical_depth() else {
            continue;
        };
        let profile = mode.profile();
        let (base, top, f) = (
            profile.base().value(),
            profile.top().value(),
            profile.exponent(),
        );
        let tau = depth.value();
        let above = if level <= top {
            0.0
        } else if level >= base {
            tau
        } else {
            let edge = math::powf(top / base, f + 1.0);
            tau * (math::powf(level / base, f + 1.0) - edge) / (1.0 - edge)
        };
        total = Some(total.unwrap_or(0.0) + above);
    }
    total
}

/// The census of the records' envelopes (P14.T54.b.i; decision-p14-t24d-profile §9's bless-note
/// lines and P14.T54.b.k's count (x) inputs), every 24 hold in force and the record's rules at their
/// batch, at the epoch, over [`census_sets`], printed (`--nocapture`):
///
/// - the envelopes by class and state, the not-modelled ones by reason;
/// - the table's four giants' T(1 bar) and `T_int` through the record's envelope, and every
///   generated giant's T(1 bar), `T_int` and join; the joins deeper than 100 bar; the envelopes not
///   hydrogen-dominated;
/// - below 0.414 Jupiter masses, `T_int` against the bulk section's (`T_eff`⁴ − `T_eq`⁴)^¼;
/// - the 0.15–0.414 Jupiter-mass giants' He/H (the bless note's 0.15–0.3, and the blend's
///   0.3–0.414), at the envelope's `T_eff` against the bulk's;
/// - count (x): every refractory, sulphide or salt deck based below its join, with its τ(550), its
///   base, its τ above 10 bar and 1 bar, and every mode with no τ topped above 10 bar ("TO MAIN"
///   where a τ above 1 bar reaches 1, or such a mode's top lies above 10 bar); the largest deck τ;
///   every ice giant's CH₄ τ ("TO MAIN" above 10); the decks bound at the column's 10-kbar bottom
///   (`DeckBound::Bottom`), each based at 10⁹ Pa to rounding (asserted);
/// - every gas envelope whose recorded change falls inside the clock window, its section either
///   side of it (a body that loses its envelope changes section there);
/// - the cost: each envelope's rebuild and the query's network, timed (this build's profile, the
///   machine's load).
#[test]
#[ignore = "slow: the census of the records' envelopes over 102 systems"]
#[expect(
    clippy::too_many_lines,
    reason = "one census, each of the bless note's lines printed in turn"
)]
fn the_envelopes_are_stated() {
    use std::fmt::Write as _;
    let sets = census_sets();
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut lines = String::new();
    let (mut deep_joins, mut not_hydrogen) = (Vec::new(), Vec::new());
    let (mut mismatches, mut light_giants) = (Vec::new(), Vec::new());
    let (mut deep_decks, mut to_main, mut ice_giants) = (Vec::new(), Vec::new(), Vec::new());
    let (mut bottoms, mut changes) = (0_usize, Vec::new());
    let mut largest_tau = (0.0_f64, String::new());
    let mut timings: Vec<f64> = Vec::new();
    let jupiter = EarthMasses::from(JupiterMasses::new(1.0)).value();
    // The network a query builds at its first envelope (`Epoch::network`), timed once.
    let start = std::time::Instant::now();
    let built = Network::registry().expect("the registry's network builds");
    let network_build = start.elapsed().as_secs_f64();
    assert_eq!(&built, network());
    with_every_24_hold(|| {
        for name in ["Jupiter", "Saturn", "Uranus", "Neptune"] {
            let (of, envelope) = table_envelope(name);
            writeln!(
                lines,
                "  the table's {name}: T(1 bar) {:.2} K, T_int {:.2} K, join {:.4} bar, He/H {:.6}",
                at_bar(&envelope, 1.0),
                envelope.atmosphere().internal_temperature().value(),
                envelope.atmosphere().profile().join_pressure().value() / 1e5,
                of.primordial_helium_per_hydrogen
            )
            .expect("a string takes a line");
        }
        let rules = batch_rules();
        for (set, seed, ctx) in &sets {
            let system = generate(*seed, ctx).with_record_rules(rules);
            walk(ctx, &system, UniverseTime::EPOCH, &mut |epoch, visit| {
                let record = &visit.record;
                let Section::Ok(bulk) = record.bulk() else {
                    return;
                };
                let class = bulk.class();
                if class.has_solid_surface() {
                    if record.section_state(RecordSection::Envelope) == SectionState::NotModelled {
                        *counts.entry("solid, supercritical".to_owned()).or_default() += 1;
                    }
                    return;
                }
                let at = format!("{set} {} {:04x}", ctx.id(), visit.body.index.get());
                let derived = visit
                    .derived
                    .as_ref()
                    .expect("a present primary's derivation");
                let of = rebuilt(epoch, &system, visit.body, (record, derived));
                let start = std::time::Instant::now();
                let again = envelope_of(&of, network());
                timings.push(start.elapsed().as_secs_f64());
                let m_j = of.mass.value() / jupiter;
                if let Some(change) = visit.changes_at
                    && change > ClockWindow::START
                    && change <= ClockWindow::END
                {
                    let before = change.checked_sub(Span::from_seconds(1)).expect("in range");
                    let state = |t| {
                        system
                            .body_at(ctx, visit.body.index, t)
                            .map(|r| r.section_state(RecordSection::Envelope))
                    };
                    changes.push(format!(
                        "{at} {class:?} changes at {change:?}: {:?} before, {:?} at it",
                        state(before),
                        state(change)
                    ));
                }
                let envelope = match (record.envelope(), &again) {
                    (Section::Ok(envelope), Ok(_)) => envelope,
                    (_, Err(error)) => {
                        let why = reason(error);
                        *counts.entry(format!("{class:?} {why}")).or_default() += 1;
                        if why == "not hydrogen-dominated" {
                            not_hydrogen.push(format!("{at} {class:?} {m_j:.4} M_J: {error:?}"));
                        }
                        return;
                    }
                    (section, Ok(_)) => panic!("{at}: {section:?} against its builder's Ok"),
                };
                *counts.entry(format!("{class:?} ok")).or_default() += 1;
                let atmosphere = envelope.atmosphere();
                let profile = atmosphere.profile();
                let (t_int, join) = (
                    atmosphere.internal_temperature().value(),
                    profile.join_pressure().value(),
                );
                if matches!(class, PlanetClass::GasGiant | PlanetClass::IceGiant) {
                    writeln!(
                        lines,
                        "  {at} {class:?} {m_j:.4} M_J T_eq {:.1} K: T(1 bar) {:.2} K, T_int \
                         {t_int:.2} K, join {:.4} bar",
                        of.equilibrium_temperature.value(),
                        at_bar(envelope, 1.0),
                        join / 1e5
                    )
                    .expect("a string takes a line");
                }
                if join > 1e7 {
                    deep_joins.push(format!(
                        "{at} {class:?} {m_j:.4} M_J: {:.1} bar",
                        join / 1e5
                    ));
                }
                if m_j < 0.414 {
                    let t_eq = of.equilibrium_temperature.value();
                    let bulk_t_int = bulk.effective_temperature().map_or(0.0, |t| {
                        math::powf(
                            (math::powi(t.value(), 4) - math::powi(t_eq, 4)).max(0.0),
                            0.25,
                        )
                    });
                    mismatches.push(format!(
                        "{at} {class:?} {m_j:.4} M_J: T_int {t_int:.2} K against the bulk's \
                         {bulk_t_int:.2} K"
                    ));
                }
                // T24.d's blend reaches the giants below 0.414 M_J (`giant_share`): the bless
                // note's 0.15–0.3 M_J and the blended 0.3–0.414 M_J alike.
                if (0.15..0.414).contains(&m_j) {
                    let abundances = envelope.abundances();
                    let bulk_t_eff = bulk
                        .effective_temperature()
                        .unwrap_or(of.equilibrium_temperature);
                    let formation = of.formation.expect("an Ok envelope's formation");
                    let at_bulk = EnvelopeAbundances::derive(&EnvelopeInputs {
                        mass: of.mass,
                        fractions: of.fractions,
                        formation_distance: formation.distance(),
                        snow_line: formation.snow_line(),
                        fe_h: of.fe_h,
                        alpha_fe: of.alpha_fe.expect("an Ok envelope's [α/Fe]"),
                        host: of.host,
                        effective_temperature: bulk_t_eff,
                        age: of.age,
                        surface_gravity: of.gravity,
                        primordial_helium_per_hydrogen: of.primordial_helium_per_hydrogen,
                    })
                    .expect("the bulk's T_eff is an input T50.b takes");
                    light_giants.push(format!(
                        "{at} {class:?} {m_j:.4} M_J: He/H {:.6} (depletion {:.4}) at the \
                         envelope's T_eff, {:.6} ({:.4}) at the bulk's {:.2} K",
                        abundances.per_hydrogen(Element::He),
                        abundances.helium_depletion(),
                        at_bulk.per_hydrogen(Element::He),
                        at_bulk.helium_depletion(),
                        bulk_t_eff.value()
                    ));
                }
                for (k, deck) in atmosphere.decks().iter().enumerate() {
                    let condensate = deck.condensate();
                    let base = deck.base().value();
                    let tau = atmosphere.deck_optical_depth(condensate);
                    if let Some(tau) = tau
                        && tau > largest_tau.0
                    {
                        largest_tau = (tau, format!("{at} {}", key_of(condensate)));
                    }
                    if atmosphere.deck_bounds()[k] == DeckBound::Bottom {
                        assert!(
                            (base / DECK_COLUMN_BOTTOM.value() - 1.0).abs() < 1e-9,
                            "{at}: {} bound at the bottom, based at {base} Pa",
                            key_of(condensate)
                        );
                        bottoms += 1;
                    }
                    if condensate == row("CH4") && class == PlanetClass::IceGiant {
                        let tau = tau.unwrap_or(f64::NAN);
                        ice_giants.push(format!("{at}: CH4 tau {tau:.4}"));
                        if tau.is_nan() || tau > CLOUD_DECK_SPLIT_OPTICAL_DEPTH {
                            to_main.push(format!("{at}: an ice giant's CH4 tau {tau}"));
                        }
                    }
                    let deep = matches!(
                        DeckKind::of(condensate),
                        Some(DeckKind::Refractory | DeckKind::SulphideOrSalt)
                    ) && base > join;
                    if !deep {
                        continue;
                    }
                    let above_ten = deck_tau_above(envelope, condensate, 1e6);
                    let above_one = deck_tau_above(envelope, condensate, 1e5);
                    deep_decks.push(format!(
                        "{at} {}: tau {tau:?}, base {:.1} bar, tau above 10 bar {above_ten:?}, \
                         above 1 bar {above_one:?}",
                        key_of(condensate),
                        base / 1e5
                    ));
                    if above_one.is_some_and(|tau| tau >= 1.0) {
                        to_main.push(format!(
                            "{at}: {} tau above 1 bar {above_one:?}",
                            key_of(condensate)
                        ));
                    }
                    for mode in atmosphere.modes().iter().filter(|m| m.deck() == condensate) {
                        if let Err(error) = mode.optical_depth()
                            && mode.profile().top().value() < 1e6
                        {
                            to_main.push(format!(
                                "{at}: a {} mode with no tau topped at {:.3} bar ({error:?})",
                                key_of(condensate),
                                mode.profile().top().value() / 1e5
                            ));
                        }
                    }
                }
            });
        }
    });
    timings.sort_by(f64::total_cmp);
    let (n, total) = (timings.len(), timings.iter().sum::<f64>());
    let largest = timings.last().copied().unwrap_or(0.0);
    eprintln!(
        "{} systems; the envelopes by class and state: {counts:?}\nthe giants:\n{lines}joins \
         deeper than 100 bar ({}): {deep_joins:#?}\nnot hydrogen-dominated ({}): \
         {not_hydrogen:#?}\nT_int below 0.414 M_J ({}): {mismatches:#?}\n0.15-0.414 M_J giants' \
         helium ({}): {light_giants:#?}\ncount (x), deep decks ({}): {deep_decks:#?}\nthe \
         largest deck tau {:.4e} ({})\nthe ice giants' CH4 ({}): {ice_giants:#?}\ndecks bound at \
         the bottom: {bottoms}\nchanges inside the window ({}): {changes:#?}\nTO MAIN: \
         {to_main:#?}\nthe rebuilds: {n}, {total:.2} s in all, mean {:.1} ms, median {:.1} ms, \
         the slowest {:.1} ms; a query's network built in {:.1} ms",
        sets.len(),
        deep_joins.len(),
        not_hydrogen.len(),
        mismatches.len(),
        light_giants.len(),
        deep_decks.len(),
        largest_tau.0,
        largest_tau.1,
        ice_giants.len(),
        changes.len(),
        1e3 * total / f64::from(u32::try_from(n.max(1)).expect("a count of envelopes")),
        1e3 * timings.get(n / 2).copied().unwrap_or(0.0),
        1e3 * largest,
        1e3 * network_build,
    );
    assert!(counts.keys().any(|label| label.ends_with(" ok")));
}
