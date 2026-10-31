//! The envelope section on the record (P14.T54.b.i), from generator version 24: every sub-Neptune's
//! and giant's envelope is its builder's on its record's own members, or not modelled for a stated
//! reason, over the golden systems and the plan-14 sample; the table's Jupiter against the probe;
//! the ice giants' methane decks (G3); the census of the bless note's lines (ignored).

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::Seed;
use crate::math;
use crate::planetary::chem::condense::DeckBound;
use crate::planetary::chem::condense::{ResolveOpticalDepthError, SizeModeError};
use crate::planetary::chem::escape::{EnvelopeHelium, host_helium_per_hydrogen};
use crate::planetary::chem::{AbundanceDraws, HostAbundances};
use crate::planetary::derive::solar::{PLANETS, SOLAR_AGE, solar_disc, solar_system};
use crate::planetary::derive::{DerivedBody, PlanetClass};
use crate::planetary::hooks::aerosols::AerosolMode;
use crate::planetary::hooks::envelope_atmosphere::{DECK_COLUMN_BOTTOM, DeckKind};
use crate::planetary::hooks::structure::envelope::{
    ADIABAT_BOTTOM, MEAN_REDISTRIBUTION, OPACITY_RATIO, OPTICAL_DEPTH_EXPONENT,
    THERMAL_OPACITY_AT_ONE_BAR_M2_PER_KG,
};
use crate::planetary::hooks::surface::HourCosines;
use crate::planetary::label;
use crate::planetary::record::{BodyRecord, RecordSection, SectionState};
use crate::planetary::system::tests::{SEED, golden_systems_afresh, sample, thin_air_sets};
use crate::planetary::system::{Body, Epoch, Part, PlanetarySystem, fate_host};
use crate::planetary::testing::{EnvelopeByHand, envelope_by_hand, with_every_25_hold};
use crate::planetary::{SystemContext, generate};
use crate::stellar::Composition;
use crate::substance::CondensedPhase;
use crate::substance::aerosol::{
    LIQUID_STAND_INS, MoltenDecksRules, ResolveAerosolError, with_molten_decks,
};
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
    let epoch = Epoch::new(system, ctx, t).with_shades().with_small_bodies();
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
        DeriveEnvelopeError::NoSupercriticalReservoir => "no supercritical reservoir",
        DeriveEnvelopeError::Supercritical(_) => "the supercritical model's refusal",
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
    let abundances = envelope
        .abundances()
        .expect("an H₂/He envelope's abundances");
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

/// Over the golden systems and the
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
    {
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
            for t in [ClockWindow::START, ClockWindow::END] {
                walk(ctx, system, t, &mut |epoch, visit| {
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
                    let of = rebuilt(epoch, system, visit.body, (record, derived));
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
    };
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
/// (signed zeros included, by `Debug`) and the record's rules at their
/// batch, over three golden systems with giants; and `body_at` gives each body the same envelope
/// whatever order its system's bodies are asked in, so that the escape batch's carried helium, read
/// by each envelope, keeps nothing between bodies (the sim-determinism skill's call-order rule).
#[test]
fn under_every_24_hold_the_record_s_envelope_is_the_same_twice_and_in_both_queries() {
    let mut envelopes = 0;
    {
        let systems: Vec<_> = golden_systems_afresh()
            .into_iter()
            .filter(|(name, ..)| matches!(*name, "hot_jupiter" | "solar_like" | "eccentric_giant"))
            .collect();
        assert_eq!(systems.len(), 3);
        for (name, ctx, system) in systems {
            let again = generate(system.seed, &ctx);
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
    };
    assert!(envelopes > 2, "{envelopes} envelopes");
}

/// The table's giant `name` as its record's envelope reads it (the Solar System through
/// `derive_body` about the present Sun, so that its derivation carries its
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
            .map_err(DeriveEnvelopeError::FormationZone),
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
/// a solar host, follows the Galileo probe as P14.T52.d's table case does:
/// T(1 bar) within 10% of the probe's 166.1 K (Seiff et al. 1998), its ammonia deck's base at
/// 0.5–1 bar and CH₄ ÷ H₂ at 1.5–3 × 10⁻³ (Wong et al. 2004's 2.4 × 10⁻³, as Guillot et al. 2022's
/// Table 2 lists it); its join and photosphere stated. Its He/H is its carry's, fractionated from
/// a solar host's start, the protosolar Y: 0.096818, 1.0 × 10⁻⁴ below the registry's n☉ (0.096828),
/// since the carry's He/H (`EnvelopeHelium::helium_to_hydrogen`) reads hydrogen and helium at the
/// gas rows' molecular masses and the start's Y at the standard atomic weights (stated, to the lane:
/// the two scales of one figure).
#[test]
fn the_table_s_jupiter_on_the_record_follows_the_probe() {
    let (of, jupiter) = table_envelope("Jupiter");
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
        jupiter
            .abundances()
            .expect("an H₂/He envelope's abundances")
            .helium_depletion(),
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
/// record's envelope, each below R08's cloud-deck split of 10 and within 3
/// times its range's top (G2's 0.5–4 for Neptune and 1–5 for Uranus, Irwin et al. 2022), stated:
/// P14.T52.d's table cases at the protosolar He/H gave 5.77 and 3.76, which the carry's He/H, the
/// escape's from the protosolar start, moves slightly.
#[test]
fn the_ice_giants_methane_decks_meet_g3() {
    for (name, top) in [("Neptune", 4.0), ("Uranus", 5.0)] {
        let (of, envelope) = table_envelope(name);
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
/// lines and P14.T54.b.k's count (x) inputs) and the record's rules at their
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
    for (set, seed, ctx) in &sets {
        let system = generate(*seed, ctx);
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
                let abundances = envelope
                    .abundances()
                    .expect("a giant's H₂/He envelope's abundances");
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

/// The molten decks' liquids (P14.T49.p), the rows [`LIQUID_STAND_INS`] serves.
fn molten_liquids() -> Vec<SubstanceId> {
    LIQUID_STAND_INS.iter().map(|s| row(s.key())).collect()
}

/// Whether a mode's τ is refused as the molten decks' hold refuses it (P14.T49.p): its size for
/// want of a density (a molten salt's), or its optics for want of a file (a molten refractory's).
fn refused_as_held(mode: &AerosolMode) -> bool {
    matches!(
        mode.optical_depth(),
        Err(
            ResolveOpticalDepthError::Size(SizeModeError::NoDensity { .. })
                | ResolveOpticalDepthError::Aerosol(ResolveAerosolError::NoFile { .. })
        )
    )
}

/// The records of the golden systems, generated afresh, at the epoch under every 25 hold with the
/// molten decks' rules `rules`, by system.
fn golden_records_at_25(rules: MoltenDecksRules) -> Vec<(&'static str, Vec<BodyRecord>)> {
    with_every_25_hold(|| {
        with_molten_decks(rules, || {
            golden_systems_afresh()
                .into_iter()
                .map(|(name, ctx, system)| {
                    let snapshot = system.snapshot_at(&ctx, UniverseTime::EPOCH);
                    (name, snapshot.bodies().to_vec())
                })
                .collect()
        })
    })
}

/// P14.T49.p over the golden systems at the epoch, every other 25 hold in force: under the molten
/// decks' rules each record is its held twin's to the bit (by `Debug`, signed zeros included) but
/// for its molten modes. Each of those is a liquid of [`LIQUID_STAND_INS`]' rows whose twin's τ
/// was refused for want of a file or a density, and it takes a size and a τ, its material, phase,
/// deck, column mass, profile, coverage and temperature its twin's.
#[test]
fn under_every_25_hold_the_molten_decks_take_a_tau_and_move_nothing_else() {
    let molten_rows = molten_liquids();
    let held = golden_records_at_25(MoltenDecksRules::Held);
    let molten = golden_records_at_25(MoltenDecksRules::Molten);
    assert_eq!(held.len(), molten.len());
    let mut gained: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for ((name, held), (_, molten)) in held.iter().zip(&molten) {
        assert_eq!(held.len(), molten.len(), "{name}");
        for (a, b) in held.iter().zip(molten) {
            let at = format!("{name} {:04x}", a.index().get());
            let (mut text_a, mut text_b) = (format!("{a:?}"), format!("{b:?}"));
            if let (Section::Ok(was), Section::Ok(now)) = (a.envelope(), b.envelope()) {
                let (was, now) = (was.atmosphere().modes(), now.atmosphere().modes());
                assert_eq!(was.len(), now.len(), "{at}");
                for (x, y) in was.iter().zip(now) {
                    let (dx, dy) = (format!("{x:?}"), format!("{y:?}"));
                    if dx == dy {
                        continue;
                    }
                    let key = key_of(x.material());
                    assert!(
                        x.phase() == CondensedPhase::Liquid
                            && molten_rows.contains(&x.material())
                            && refused_as_held(x),
                        "{at}: a mode that is not a molten deck's moves: {dx} against {dy}"
                    );
                    assert!(
                        y.size().is_ok() && y.optical_depth().is_ok(),
                        "{at} {key}: {dy}"
                    );
                    assert_eq!(
                        (x.material(), x.phase(), x.deck(), x.shape()),
                        (y.material(), y.phase(), y.deck(), y.shape()),
                        "{at}"
                    );
                    assert_eq!(
                        format!(
                            "{:?}",
                            (x.column_mass(), x.profile(), x.coverage(), x.temperature())
                        ),
                        format!(
                            "{:?}",
                            (y.column_mass(), y.profile(), y.coverage(), y.temperature())
                        ),
                        "{at} {key}"
                    );
                    text_a = text_a.replacen(&dx, "a molten mode", 1);
                    text_b = text_b.replacen(&dy, "a molten mode", 1);
                    *gained.entry(key).or_default() += 1;
                }
            }
            assert_eq!(
                text_a, text_b,
                "{at}: the record moves beyond its molten modes"
            );
        }
    }
    eprintln!("the golden systems' molten modes that take a τ, by row: {gained:?}");
    for key in ["MgSiO3", "Mg2SiO4", "Fe"] {
        assert!(gained.contains_key(key), "{key}: {gained:?}");
    }
}

/// The shared fixture's hot Jupiter (`packages/protocol/fixtures/planetary.json`'s
/// `body_detail_hot_jupiter`, an HD 209458 b), as `hyperion-server`'s `fixture_giant` builds its
/// inputs from the record's bulk section: its mass, radius, gravity, `T_eq` and `T_eff` copied
/// from the fixture, the generator's fractions for its mass, formed at the slice zone's snow line,
/// 3.1 Gyr old (Torres, Winn and Holman 2008), a solar host, starlit.
fn fixture_hot_jupiter() -> EnvelopeByHand {
    use crate::planetary::derive::{SnowLineSide, giant_composition};
    use crate::units::consts::STEFAN_BOLTZMANN;
    use crate::units::{Dex, EarthMasses, Kilograms, MetresPerSecondSquared, Watts, Years};
    let mass = EarthMasses::from(Kilograms::new(1.300_215_349_175_194_7e27));
    let (radius, t_eq, t_eff) = (97_157_628.0, 1_449.0, 1_456.9);
    let snow_line = Metres::new(337_938_907_867.118_65);
    let area = 4.0 * core::f64::consts::PI * radius * radius;
    EnvelopeByHand {
        mass,
        fractions: giant_composition(mass, SnowLineSide::Beyond)
            .expect("a giant's mass")
            .fractions(),
        radius: Metres::new(radius),
        gravity: MetresPerSecondSquared::new(9.193_210_521_502_017),
        equilibrium_temperature: Kelvin::new(t_eq),
        internal_luminosity: Watts::new(
            area * STEFAN_BOLTZMANN * (math::powi(t_eff, 4) - math::powi(t_eq, 4)),
        ),
        age: Years::new(3.1e9),
        formation_distance: snow_line,
        snow_line,
        fe_h: Dex::ZERO,
        alpha_fe: Dex::ZERO,
        host: HostAbundances::of(Dex::ZERO, &AbundanceDraws::MEDIAN),
        primordial_helium_per_hydrogen: Element::He.protosolar_abundance_per_hydrogen(),
        starlit: true,
    }
}

/// One deep deck's mode in count (x) (P14.T49.p): where, its row, its top, bar, its τ or why
/// not, its basis, and whether its τ is refused as the molten decks' hold refuses it.
#[derive(Debug, Clone)]
struct DeepMode {
    at: String,
    key: String,
    top_bar: f64,
    tau: Result<f64, String>,
    basis: Option<String>,
    refused: bool,
}

/// Count (x)'s deep decks over `sets` at the epoch, every 25 hold in force with the molten decks'
/// rules `rules`: each deck of a refractory, a sulphide or a salt based below its envelope's join,
/// by `set id body deck`, with its τ above 1 bar, and its modes.
fn deep_decks_at_25(
    sets: &[(&'static str, Seed, SystemContext)],
    rules: MoltenDecksRules,
) -> std::collections::BTreeMap<String, (Option<f64>, Vec<DeepMode>)> {
    let molten_rows = molten_liquids();
    let mut decks = std::collections::BTreeMap::new();
    with_every_25_hold(|| {
        with_molten_decks(rules, || {
            for (set, seed, ctx) in sets {
                let system = generate(*seed, ctx);
                walk(ctx, &system, UniverseTime::EPOCH, &mut |_, visit| {
                    let Section::Ok(envelope) = visit.record.envelope() else {
                        return;
                    };
                    let at = format!("{set} {} {:04x}", ctx.id(), visit.body.index.get());
                    let atmosphere = envelope.atmosphere();
                    let join = atmosphere.profile().join_pressure().value();
                    for deck in atmosphere.decks() {
                        let condensate = deck.condensate();
                        let deep = matches!(
                            DeckKind::of(condensate),
                            Some(DeckKind::Refractory | DeckKind::SulphideOrSalt)
                        ) && deck.base().value() > join;
                        if !deep {
                            continue;
                        }
                        let key = key_of(condensate);
                        let modes = atmosphere
                            .modes()
                            .iter()
                            .filter(|m| m.deck() == condensate)
                            .map(|mode| DeepMode {
                                at: at.clone(),
                                key: key.clone(),
                                top_bar: mode.profile().top().value() / 1e5,
                                tau: mode
                                    .optical_depth()
                                    .map(|d| d.value())
                                    .map_err(|e| format!("{e:?}")),
                                basis: mode
                                    .optical_depth()
                                    .ok()
                                    .map(|d| format!("{:?}", d.basis())),
                                refused: mode.phase() == CondensedPhase::Liquid
                                    && molten_rows.contains(&mode.material())
                                    && refused_as_held(mode),
                            })
                            .collect();
                        decks.insert(
                            format!("{at} {key}"),
                            (deck_tau_above(envelope, condensate, 1e5), modes),
                        );
                    }
                });
            }
        });
    });
    decks
}

/// Count (x) re-run for P14.T49.p (decision-galaxy-25-train §6), every 25 hold in force, over
/// [`census_sets`] at the epoch, printed (`--nocapture`): the deep decks' modes with no τ under
/// the molten decks' hold that are topped above 10 bar, by row, with their τ, tops and bases under
/// 25's rules and any still without one; every deep deck's τ above 1 bar under 25's ("TO MAIN" at
/// 1 or more); and the fixture's hot Jupiter's τ above its photosphere, by deck, both ways. The
/// galaxy-side hold at 25 (`KEPLER_REGIME_VERSION`) moves only the central 10 ly, which no census
/// set reaches.
#[test]
#[ignore = "slow: count (x) over 102 systems, held and molten"]
#[expect(
    clippy::too_many_lines,
    reason = "one census, each of the entry's lines printed in turn"
)]
fn under_every_25_hold_count_x_states_the_molten_decks() {
    use std::fmt::Write as _;
    let sets = census_sets();
    let held = deep_decks_at_25(&sets, MoltenDecksRules::Held);
    let molten = deep_decks_at_25(&sets, MoltenDecksRules::Molten);
    assert_eq!(
        held.keys().collect::<Vec<_>>(),
        molten.keys().collect::<Vec<_>>(),
        "the same deep decks"
    );
    let mut gap: Vec<(&DeepMode, &DeepMode)> = Vec::new();
    let mut deeper: Vec<&DeepMode> = Vec::new();
    let mut unrefused_without_tau = Vec::new();
    let mut envelopes = std::collections::BTreeSet::new();
    let mut to_main = Vec::new();
    for (deck, (_, was)) in &held {
        let (above_one, now) = &molten[deck];
        assert_eq!(was.len(), now.len(), "{deck}");
        for (x, y) in was.iter().zip(now) {
            if !x.refused {
                assert_eq!(
                    format!("{x:?}"),
                    format!("{y:?}"),
                    "{deck}: a mode the hold did not refuse moves"
                );
                if x.tau.is_err() && x.top_bar < 10.0 {
                    unrefused_without_tau.push(format!("{deck}: {:?}", x.tau));
                }
                continue;
            }
            if let Err(why) = &y.tau {
                to_main.push(format!(
                    "{deck}: no τ at 25, topped at {:.3} bar ({why})",
                    y.top_bar
                ));
            }
            if x.top_bar < 10.0 {
                gap.push((x, y));
                envelopes.insert(x.at.clone());
            } else {
                deeper.push(y);
            }
        }
        if above_one.is_some_and(|tau| tau >= 1.0) {
            to_main.push(format!("{deck}: τ above 1 bar {above_one:?} at 25"));
        }
    }
    let mut by_row: std::collections::BTreeMap<&str, Vec<&DeepMode>> =
        std::collections::BTreeMap::new();
    for (_, y) in &gap {
        by_row.entry(&y.key).or_default().push(y);
    }
    let mut table = String::new();
    writeln!(
        table,
        "| Row | Modes | Envelopes | τ min | τ median | τ max | Highest top, bar | Basis |"
    )
    .expect("a string takes a line");
    writeln!(table, "| --- | --- | --- | --- | --- | --- | --- | --- |")
        .expect("a string takes a line");
    for (key, modes) in &by_row {
        let mut taus: Vec<f64> = modes.iter().filter_map(|m| m.tau.clone().ok()).collect();
        taus.sort_by(f64::total_cmp);
        let on: std::collections::BTreeSet<&str> = modes.iter().map(|m| m.at.as_str()).collect();
        let highest = modes
            .iter()
            .map(|m| m.top_bar)
            .fold(f64::INFINITY, f64::min);
        let bases: std::collections::BTreeSet<&str> =
            modes.iter().filter_map(|m| m.basis.as_deref()).collect();
        writeln!(
            table,
            "| {key} | {} | {} | {:.3e} | {:.3e} | {:.3e} | {highest:.4} | {bases:?} |",
            modes.len(),
            on.len(),
            taus.first().copied().unwrap_or(f64::NAN),
            taus.get(taus.len() / 2).copied().unwrap_or(f64::NAN),
            taus.last().copied().unwrap_or(f64::NAN),
        )
        .expect("a string takes a line");
    }
    let mut above_one: Vec<(f64, &str)> = molten
        .iter()
        .filter_map(|(deck, (tau, _))| tau.map(|tau| (tau, deck.as_str())))
        .collect();
    above_one.sort_by(|a, b| b.0.total_cmp(&a.0));
    let hot = fixture_hot_jupiter();
    let fixture = |rules| {
        with_every_25_hold(|| {
            with_molten_decks(rules, || {
                let envelope = envelope_by_hand(&hot).expect("the fixture's hot Jupiter");
                let photosphere = envelope
                    .atmosphere()
                    .profile()
                    .photosphere_pressure()
                    .value();
                let decks: Vec<(String, Option<f64>)> = envelope
                    .atmosphere()
                    .decks()
                    .iter()
                    .map(|deck| {
                        let condensate = deck.condensate();
                        (
                            key_of(condensate),
                            deck_tau_above(&envelope, condensate, photosphere),
                        )
                    })
                    .collect();
                (photosphere, decks)
            })
        })
    };
    let (photosphere, fixture_held) = fixture(MoltenDecksRules::Held);
    let (_, fixture_molten) = fixture(MoltenDecksRules::Molten);
    let total = |decks: &[(String, Option<f64>)]| decks.iter().filter_map(|d| d.1).sum::<f64>();
    let mut deeper_taus: Vec<f64> = deeper.iter().filter_map(|m| m.tau.clone().ok()).collect();
    deeper_taus.sort_by(f64::total_cmp);
    let mut modes = String::new();
    for (_, y) in &gap {
        writeln!(
            modes,
            "  {} {}: top {:.4} bar, τ {:?}, {:?}",
            y.at, y.key, y.top_bar, y.tau, y.basis
        )
        .expect("a string takes a line");
    }
    eprintln!("the modes, each with its top, at 25:\n{modes}");
    eprintln!(
        "count (x) at 25 over {} systems: {} deep decks; {} modes with no τ under the hold topped \
         above 10 bar, on {} envelopes\n{table}and {} more the hold refused topped below 10 bar, τ \
         {:.3e} to {:.3e} at 25; deep modes with no τ the hold did not refuse, topped above 10 \
         bar: {unrefused_without_tau:?}\nthe deep decks' τ above 1 bar at 25, the largest ten: \
         {:?}\nthe fixture's hot Jupiter, its photosphere at {:.4} bar, τ above it by deck, held \
         {:.3} in all: {fixture_held:?}\nmolten {:.3} in all: {fixture_molten:?}\nTO MAIN: \
         {to_main:#?}",
        sets.len(),
        molten.len(),
        gap.len(),
        envelopes.len(),
        deeper.len(),
        deeper_taus.first().copied().unwrap_or(f64::NAN),
        deeper_taus.last().copied().unwrap_or(f64::NAN),
        above_one.iter().take(10).collect::<Vec<_>>(),
        photosphere / 1e5,
        total(&fixture_held),
        total(&fixture_molten),
    );
    assert!(!gap.is_empty());
    let untaken: Vec<&String> = to_main
        .iter()
        .filter(|line| line.contains("no τ"))
        .collect();
    assert_eq!(
        untaken,
        Vec::<&String>::new(),
        "every refused mode takes a τ at 25"
    );
}

/// The molten decks' arm gives the same envelope twice (the sim-determinism skill's rule), over
/// the fixture's hot Jupiter, whose Fe deck it moves.
#[test]
fn under_every_25_hold_the_molten_decks_envelope_is_the_same_twice() {
    let hot = fixture_hot_jupiter();
    let twice: Vec<Envelope> = (0..2)
        .map(|_| with_every_25_hold(|| envelope_by_hand(&hot).expect("the fixture's hot Jupiter")))
        .collect();
    assert_eq!(format!("{:?}", twice[0]), format!("{:?}", twice[1]));
    assert!(
        twice[0]
            .atmosphere()
            .modes()
            .iter()
            .any(|m| m.material() == row("Fe") && m.optical_depth().is_ok()),
        "its Fe deck takes a τ"
    );
}

/// P14.T53.f is held until the 25 → 26 bump: the rules in force are version 25's, a solid body's
/// envelope under a supercritical envelope not modelled, and the tests' switch reaches the model.
#[test]
fn the_supercritical_envelope_is_held_only_until_the_26_bump() {
    assert!(
        crate::GENERATOR_VERSION < SUPERCRITICAL_ENVELOPE_VERSION,
        "the 25 → 26 bump puts P14.T53.f's supercritical envelope on the record (decision-p14-t53b-\
         part3 §7; main, 2026-10-30): delete `SUPERCRITICAL_ENVELOPE_VERSION`, \
         `SupercriticalEnvelopeRules` with its `Held` arm and `supercritical_envelope_in_force`'s \
         guard in `record_supercritical_envelope`, the tests' `with_supercritical_envelope`, its \
         switch in `with_every_26_hold` (and the helper, if no other 26 hold is left), and this \
         test"
    );
    assert_eq!(
        SupercriticalEnvelopeRules::in_force(),
        SupercriticalEnvelopeRules::Held
    );
    assert!(!supercritical_envelope_in_force());
    assert!(with_supercritical_envelope(
        SupercriticalEnvelopeRules::Modelled,
        supercritical_envelope_in_force
    ));
    assert!(!supercritical_envelope_in_force());
    assert!(crate::planetary::testing::with_every_26_hold(
        supercritical_envelope_in_force
    ));
}
