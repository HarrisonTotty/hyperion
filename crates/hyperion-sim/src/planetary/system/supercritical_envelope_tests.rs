//! P14.T53.f's census over the record: a solid body's supercritical envelope under every 26 hold,
//! against its own hold set back, over the sample systems that hold decision-p14-t53a-thick-co2
//! §11.1's three systems water worlds, and (run by name) over the 400 sample systems and the
//! golden systems.

use std::time::Instant;

use hyperion_testkit::order::assert_order_independent;

use super::audit_tests::{AuditCase, AuditSet, audit_walk};
use super::envelope::{SupercriticalEnvelopeRules, with_supercritical_envelope};
use super::tests::{GOLDEN_SYSTEMS_SEED, SEED, golden_contexts, sample};
use crate::planetary::derive::atmosphere::{Gas, SurfaceState};
use crate::planetary::generate;
use crate::planetary::hooks::envelope_atmosphere::supercritical::STEAM_FIT_EARTH_INSOLATION;
use crate::planetary::hooks::envelope_atmosphere::supercritical::tests::{
    assert_held_at_saturation, assert_monotone, assert_no_deck_at_or_above_the_critical_point,
};
use crate::planetary::hooks::envelope_atmosphere::{ABSORBER_LEVEL, COMPOSITION_LEVEL};
use crate::planetary::hooks::surface::STANDARD_GRAVITY;
use crate::planetary::hooks::surface::atmosphere_mole_fractions;
use crate::planetary::record::{Modelled, Section};
use crate::planetary::testing::with_every_26_hold;
use crate::rng::Seed;
use crate::substance::{SubstanceId, by_key};
use crate::time::UniverseTime;
use crate::units::WattsPerSquareMetre;

/// The systems of decision-p14-t53a-thick-co2 §11.1's three systems water worlds, each with the
/// body's index, all of the 400 sample systems: icy water worlds published `Snowball` at 647–811 K
/// under 2.9 kbar to 2.2 Mbar at 24, which P14.T53.b part 3 labels steam envelopes.
const WATER_WORLDS: [(&str, &str); 3] = [
    ("02001b2ca0020000", "0200"),
    ("02000b2ce0040000", "0600"),
    ("02000b2d9ffe0000", "0700"),
];

/// FNV-1a 64 of `text`'s bytes: a record's digest, so that a census keeps no record.
fn digest(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// One body of the census at the epoch: its record's digests, whole and but its envelope, and its
/// supercritical envelope's figures.
#[derive(Debug, Clone)]
struct CensusBody {
    /// "{set or system} {id} {kind}".
    label: String,
    /// The digest of its record.
    digest: u64,
    /// The digest of its record but its envelope section.
    outside_the_envelope: u64,
    /// Whether its published state is `SteamEnvelope`.
    steam: bool,
    /// Whether its surface is not applicable and its class solid: under a supercritical envelope.
    under_a_supercritical_envelope: bool,
    /// Its supercritical envelope's figures where its envelope section is `Ok` with one.
    figures: Option<Figures>,
}

/// What the census states of a modelled supercritical envelope.
#[derive(Debug, Clone)]
struct Figures {
    /// Its condensable's key.
    condensable: String,
    /// The published surface's temperature and pressure, K and Pa.
    surface: (f64, f64),
    /// Its profile's internal temperature, K, and join, Pa.
    internal_k: f64,
    join_pa: f64,
    /// Its transit radius's inflation, m, where modelled.
    inflation_m: Option<f64>,
    /// Its decks, and its condensables not formed.
    decks: usize,
    not_formed: String,
    /// What Turbet et al. 2020's grid reads of a water envelope: its insolation in their Earth
    /// insolations, its gravity in Earth gravities and its water's column, Pa.
    grid: Option<(f64, f64, f64)>,
    /// Its water's mole fraction in the published air (its deep gas, which its section's gas kept
    /// before its condensables were held at saturation), at 1 bar and at 10 mbar.
    water: (f64, f64, f64),
    /// The profile's temperatures at 1 bar and at 10 mbar, K.
    levels_k: (f64, f64),
}

impl Figures {
    /// One line of its figures.
    fn line(&self) -> String {
        format!(
            "{}: T_s {:.1} K, p_s {:.4e} Pa, T_int {:.2} K, join {:.3e} Pa, inflation {}, {} decks, \
             not formed {}; H2O {:.4e} deep, {:.4e} at 1 bar ({:.1} K), {:.4e} at 10 mbar ({:.1} K){}",
            self.condensable,
            self.surface.0,
            self.surface.1,
            self.internal_k,
            self.join_pa,
            self.inflation_m.map_or_else(
                || "not modelled".to_owned(),
                |z| format!("{:.1} km", z / 1e3)
            ),
            self.decks,
            self.not_formed,
            self.water.0,
            self.water.1,
            self.levels_k.0,
            self.water.2,
            self.levels_k.1,
            self.grid.map_or_else(String::new, |(s, g, p)| format!(
                "; its grid's inputs: {s:.4} S⊕ (1–30), {g:.3} g⊕ (0.2–6), water {p:.4e} Pa \
                 (2.7e5–2.7e9)"
            ))
        )
    }
}

/// The mole fraction of `id` in `gas`, 0 where it holds none.
fn share_of(gas: &[(SubstanceId, f64)], id: SubstanceId) -> f64 {
    gas.iter().find(|&&(s, _)| s == id).map_or(0.0, |&(_, x)| x)
}

/// The case's census body under the rules in force, its supercritical envelope checked: every
/// figure finite, its profile monotone in pressure and meeting the published surface, no deck
/// based at or above its condensable's critical point, and each condensable at most its saturation
/// at 1 bar and 10 mbar.
fn body_of(system: &str, case: &AuditCase<'_>) -> CensusBody {
    let record = case.record;
    let air = case.derived.atmosphere();
    let text = format!("{record:?}");
    let envelope = format!("{:?}", record.envelope());
    let outside = text.replacen(&envelope, "", 1);
    let label = format!("{system} {} {}", record.identity().id(), case.kind);
    let solid = record
        .bulk()
        .ok()
        .is_some_and(|bulk| bulk.class().has_solid_surface());
    let water = Gas::Water.substance();
    let figures = match record.envelope() {
        Section::Ok(envelope) => envelope.supercritical().map(|condensable| {
            let atmosphere = envelope.atmosphere();
            let profile = atmosphere.profile();
            let surface = air
                .surface_pressure()
                .expect("a supercritical envelope's air");
            let t_s = air.surface_temperature().value();
            assert_monotone(profile, surface);
            assert_no_deck_at_or_above_the_critical_point(atmosphere);
            assert_held_at_saturation(atmosphere);
            let at_surface = profile.temperature_at(surface).value();
            assert!(
                (at_surface / t_s - 1.0).abs() < 1e-9,
                "{label}: {at_surface} K at its surface against {t_s} K"
            );
            let inflation = match envelope.transit_radius_inflation() {
                Some(Modelled::Ok(z)) => {
                    assert!(z.value().is_finite() && z.value() > 0.0, "{label}: {z:?}");
                    Some(z.value())
                }
                Some(Modelled::NotModelled) => None,
                None => panic!("{label}: a supercritical envelope states its inflation"),
            };
            let total: f64 = atmosphere.composition().iter().map(|&(_, x)| x).sum();
            assert!((total - 1.0).abs() < 1e-9, "{label}: {total}");
            for value in [
                atmosphere.internal_temperature().value(),
                atmosphere.irradiation_temperature().value(),
                profile.join_pressure().value(),
                atmosphere.mean_molar_mass_g_per_mol(),
                atmosphere.heat_capacity_over_r(),
            ] {
                assert!(value.is_finite(), "{label}: {value}");
            }
            let bulk = record
                .bulk()
                .ok()
                .expect("a body under an envelope has its bulk");
            let grid = air
                .supercritical_reservoir(bulk.surface_gravity())
                .filter(|reservoir| reservoir.gas() == Gas::Water)
                .map(|reservoir| {
                    (
                        WattsPerSquareMetre::from(case.derived.flux()).value()
                            / STEAM_FIT_EARTH_INSOLATION.value(),
                        bulk.surface_gravity().value() / STANDARD_GRAVITY.value(),
                        (air.partial_pressures().of(Gas::Water) + reservoir.beneath_the_air())
                            .value(),
                    )
                });
            Figures {
                condensable: condensable.substance().key().as_str().to_owned(),
                surface: (t_s, surface.value()),
                internal_k: atmosphere.internal_temperature().value(),
                join_pa: profile.join_pressure().value(),
                inflation_m: inflation,
                decks: atmosphere.decks().len(),
                not_formed: format!("{:?}", atmosphere.not_formed()),
                grid,
                water: (
                    share_of(&atmosphere_mole_fractions(air.partial_pressures()), water),
                    share_of(atmosphere.composition(), water),
                    share_of(atmosphere.upper_gas(), water),
                ),
                levels_k: (
                    profile.temperature_at(COMPOSITION_LEVEL).value(),
                    profile.temperature_at(ABSORBER_LEVEL).value(),
                ),
            }
        }),
        Section::NotResolved | Section::NotModelled | Section::NotApplicable => None,
    };
    CensusBody {
        label,
        digest: digest(&text),
        outside_the_envelope: digest(&outside),
        steam: air.published_state() == SurfaceState::SteamEnvelope,
        under_a_supercritical_envelope: solid && matches!(record.surface(), Section::NotApplicable),
        figures,
    }
}

/// The census of `sets` under the rules in force: every planet, member and moon with a derivation
/// at the epoch, in walk order.
fn census_of(sets: &[(String, AuditSet)]) -> Vec<CensusBody> {
    let mut bodies = Vec::new();
    for (system, set) in sets {
        let set: [AuditSet; 1] = [set.clone()];
        audit_walk(&set, &mut |case| bodies.push(body_of(system, case)));
    }
    bodies
}

/// The sample's systems whose IDs `ids` names, in the sample's order.
fn sample_sets(ids: &[&str]) -> Vec<(String, AuditSet)> {
    sample()
        .iter()
        .enumerate()
        .filter(|(_, ctx)| ids.contains(&ctx.id().to_string().as_str()))
        .map(|(index, ctx)| (format!("s{index}"), ("sample", SEED, ctx.clone())))
        .collect()
}

/// The whole sample and the golden systems.
fn every_set() -> Vec<(String, AuditSet)> {
    sample()
        .iter()
        .enumerate()
        .map(|(index, ctx)| (format!("s{index}"), ("sample", SEED, ctx.clone())))
        .chain(
            golden_contexts()
                .into_iter()
                .map(|(name, ctx)| (name.to_owned(), (name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx))),
        )
        .collect()
}

/// The census under every 26 hold, with the supercritical envelope's own hold at `rules`.
fn census_under(rules: SupercriticalEnvelopeRules, sets: &[(String, AuditSet)]) -> Vec<CensusBody> {
    with_every_26_hold(|| with_supercritical_envelope(rules, || census_of(sets)))
}

/// Between `held` and `modelled`, one walk's bodies in one order each: the labels of the bodies
/// whose records differ, each asserted to differ in its envelope section alone, from not modelled
/// to its supercritical envelope's model.
fn movers(held: &[CensusBody], modelled: &[CensusBody]) -> Vec<String> {
    assert_eq!(held.len(), modelled.len(), "one walk's bodies, both rules");
    let mut moved = Vec::new();
    for (was, now) in held.iter().zip(modelled) {
        assert_eq!(was.label, now.label, "one walk's order");
        if was.digest == now.digest {
            continue;
        }
        assert_eq!(
            was.outside_the_envelope, now.outside_the_envelope,
            "{}: only the envelope section moves",
            now.label
        );
        assert!(
            was.under_a_supercritical_envelope && was.figures.is_none() && now.figures.is_some(),
            "{}: a supercritical envelope's section, not modelled under its hold, is modelled",
            now.label
        );
        moved.push(now.label.clone());
    }
    moved
}

/// decision-p14-t53a-thick-co2 §11.1's three systems water worlds, under every 26 hold: each is a
/// steam envelope whose envelope section is its model, H₂O, every figure finite, its profile
/// monotone in pressure and meeting its surface, no deck based at or above water's critical point;
/// the same bits twice (D4); and against its own hold set back, only their envelope sections move.
#[test]
fn under_every_26_hold_the_three_systems_water_worlds_take_their_steam_envelope() {
    let ids: Vec<&str> = WATER_WORLDS.iter().map(|&(system, _)| system).collect();
    let sets = sample_sets(&ids);
    assert_eq!(
        sets.len(),
        WATER_WORLDS.len(),
        "each system is in the sample"
    );
    let held = census_under(SupercriticalEnvelopeRules::Held, &sets);
    let modelled = census_under(SupercriticalEnvelopeRules::Modelled, &sets);
    let again = census_under(SupercriticalEnvelopeRules::Modelled, &sets);
    assert!(
        modelled
            .iter()
            .zip(&again)
            .all(|(a, b)| a.label == b.label && a.digest == b.digest),
        "the same bits twice"
    );
    let water = by_key("H2O").unwrap().substance().key();
    for (system, index) in WATER_WORLDS {
        let id = format!("{system}.{index}");
        let body = modelled
            .iter()
            .find(|body| body.label.contains(&id))
            .unwrap_or_else(|| panic!("{id} is walked"));
        eprintln!(
            "{}: steam {}, {}",
            body.label,
            body.steam,
            body.figures
                .as_ref()
                .map_or_else(|| "not modelled".to_owned(), Figures::line)
        );
        assert!(body.steam, "{id} is labelled a steam envelope");
        let figures = body.figures.as_ref().expect("its envelope is modelled");
        assert_eq!(figures.condensable, water.as_str(), "{id}");
        // Held at ice's vapour pressure aloft, the deep gas's 9–32% of water is gone.
        assert!(figures.water.2 < 1e-6, "{id}: {:?}", figures.water);
    }
    let moved = movers(&held, &modelled);
    eprintln!("moved: {moved:?}");
    for (system, index) in WATER_WORLDS {
        let id = format!("{system}.{index}");
        assert!(moved.iter().any(|label| label.contains(&id)), "{id} moves");
    }
}

/// Under every 26 hold, over the three systems water worlds' systems: a body's own record
/// (`PlanetarySystem::body_at`) holds the snapshot's envelope (`snapshot_at`) to the bit, by
/// `Debug`, and `body_at` gives each body the same envelope whatever order its system's bodies are
/// asked in, the supercritical envelopes among them (the sim-determinism skill's call-order rule).
#[test]
fn under_every_26_hold_a_supercritical_envelope_is_the_same_alone_and_in_any_order() {
    let ids: Vec<&str> = WATER_WORLDS.iter().map(|&(system, _)| system).collect();
    let mut supercritical = 0;
    with_every_26_hold(|| {
        for (name, (_, seed, ctx)) in sample_sets(&ids) {
            let system = generate(seed, &ctx);
            let t = UniverseTime::EPOCH;
            let snapshot = system.snapshot_at(&ctx, t);
            for record in snapshot.bodies() {
                let Section::Ok(envelope) = record.envelope() else {
                    continue;
                };
                supercritical += u32::from(envelope.supercritical().is_some());
                let alone = system.body_at(&ctx, record.index(), t).unwrap();
                assert_eq!(&alone, record, "{name} {:?}", record.index());
                assert_eq!(
                    format!("{:?}", alone.envelope()),
                    format!("{:?}", record.envelope()),
                    "{name} {:?}",
                    record.index()
                );
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
    assert_eq!(supercritical, 3, "the three water worlds' envelopes");
}

/// P14.T53.f's census over the 400 sample systems and the golden systems, run by name: every
/// published body under every 26 hold, with the supercritical envelope's own hold set back and in
/// force.
///
/// - **Only the supercritical envelopes move**: a body's record is the held one's to the bit but
///   its envelope section, and that only where it is under a supercritical envelope, from not
///   modelled to its model, asserted ([`movers`]).
/// - **Every modelled envelope's figures** are finite, its profile monotone in pressure and
///   meeting its surface, no deck based at or above its condensable's critical point, and each
///   condensable at most its saturation at 1 bar and 10 mbar, asserted ([`body_of`]); the walk
///   under its rules gives the same bits twice (D4).
/// - Printed: the steam envelopes (part 3's census at 25 found none in 1,706 bodies), the bodies
///   under a supercritical envelope, the modelled ones with their figures, the largest inflation,
///   the golden systems' movers (26's bump re-blesses their goldens), and each walk's time.
#[test]
#[ignore = "a measurement for the 26 bless, run by name: the 400 sample systems, walked three times"]
fn under_every_26_hold_only_the_supercritical_envelopes_move_over_the_400() {
    let sets = every_set();
    let start = Instant::now();
    let held = census_under(SupercriticalEnvelopeRules::Held, &sets);
    let walk_held = start.elapsed();
    let start = Instant::now();
    let modelled = census_under(SupercriticalEnvelopeRules::Modelled, &sets);
    let walk_modelled = start.elapsed();
    let again = census_under(SupercriticalEnvelopeRules::Modelled, &sets);
    assert!(
        modelled
            .iter()
            .zip(&again)
            .all(|(a, b)| a.label == b.label && a.digest == b.digest),
        "the same bits twice"
    );
    eprintln!(
        "the walks (pod): held {walk_held:?}, modelled {walk_modelled:?}, {} bodies",
        modelled.len()
    );
    let moved = movers(&held, &modelled);
    let steam: Vec<&str> = modelled
        .iter()
        .filter(|body| body.steam)
        .map(|body| body.label.as_str())
        .collect();
    let under: Vec<&str> = modelled
        .iter()
        .filter(|body| body.under_a_supercritical_envelope)
        .map(|body| body.label.as_str())
        .collect();
    eprintln!(
        "{} steam envelopes: {steam:?}; {} bodies under a supercritical envelope: {under:?}",
        steam.len(),
        under.len()
    );
    let mut largest: Option<(f64, &str)> = None;
    for body in &modelled {
        if let Some(figures) = &body.figures {
            eprintln!("modelled {}: {}", body.label, figures.line());
            if let Some(z) = figures.inflation_m
                && largest.is_none_or(|(most, _)| z > most)
            {
                largest = Some((z, body.label.as_str()));
            }
        } else if body.under_a_supercritical_envelope {
            eprintln!(
                "not modelled under a supercritical envelope: {}",
                body.label
            );
        }
    }
    eprintln!(
        "{} movers: {moved:?}; the largest inflation {}",
        moved.len(),
        largest.map_or_else(
            || "none modelled".to_owned(),
            |(z, label)| format!("{:.1} km ({label})", z / 1e3)
        )
    );
    let golden: Vec<&String> = moved
        .iter()
        .filter(|label| !label.starts_with('s'))
        .collect();
    eprintln!("the golden systems' movers, for 26's bless: {golden:?}");
}
