//! P14.T24.j's census over the record: a world locked 1:1's substellar deck, under every 26 hold
//! against every 25 hold, over the 400 sample systems.

use std::time::Instant;

use super::audit_tests::{AuditCase, AuditSet, audit_walk};
use super::tests::{GOLDEN_SYSTEMS_SEED, SEED, golden_contexts, sample};
use crate::planetary::chem::condense::{FallRadiusRoot, with_fall_radius};
use crate::planetary::derive::atmosphere::{MeltOxygenRules, with_melt_oxygen};
use crate::planetary::derive::passes::{PassStop, with_pass_stop};
use crate::planetary::hooks::surface::regime::Forcing;
use crate::planetary::hooks::surface::{CapBalanceRules, deck, with_cap_balance};
use crate::planetary::testing::{with_every_25_hold, with_every_26_hold};
use crate::rng::Seed;
use crate::substance::helmholtz::{CompressionRules, with_compression};
use crate::substance::phase::ice_ih_melting_temperature;
use crate::units::{Metres, Pascals};

/// The sample systems the research's pilot found in the deck's domain, by their index in the
/// sample: s243 wholly, s363 and s187 in part (at their flux and pressure tapers).
const DOMAIN_SYSTEMS: [usize; 3] = [243, 363, 187];

/// The sample systems whose bodies the deck moves (the census, 2026-10-30): the pilot's three, and
/// s59 and s102, whose published passes list no liquid water but whose earlier passes did.
const MOVED_SYSTEMS: [usize; 5] = [59, 102, 187, 243, 363];

/// `run`'s result under every 26 hold but P14.T53.g's, P14.T53.h's and P14.T54.b.l's three cost
/// levers (the fall radius, the settled compression and the own-residual stop), which this census
/// leaves held: the deck alone against every 25 hold. The levers move a figure by at most
/// 1.5 × 10⁻⁴ of itself, but they move the digests of bodies outside the deck's domain.
fn deck_alone<R>(run: impl FnOnce() -> R) -> R {
    with_every_26_hold(|| {
        with_melt_oxygen(MeltOxygenRules::Held, || {
            with_cap_balance(CapBalanceRules::Held, || {
                with_fall_radius(FallRadiusRoot::Bisections, || {
                    with_compression(CompressionRules::Fixed, || {
                        with_pass_stop(PassStop::Consecutive, run)
                    })
                })
            })
        })
    })
}

/// FNV-1a 64 of `text`'s bytes: a record's digest, so that a census of some 6,000 bodies keeps no
/// record.
fn digest(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// One body of the census at the epoch: its record's digest, and for a world locked 1:1 with an
/// `Ok` surface what the deck reads and moves.
#[derive(Debug, Clone)]
struct CensusBody {
    /// "s{index} {id} {kind}" for a sample system's, "{golden} {id} {kind}" for a golden one's.
    label: String,
    /// Whether it is a golden system's.
    golden: bool,
    /// The digest of its record and its derivation's passes.
    digest: u64,
    /// The locked world's figures, `None` for every other body.
    locked: Option<LockedFigures>,
}

/// What the census reads of a world locked 1:1 with an `Ok` surface.
#[derive(Debug, Clone, PartialEq)]
struct LockedFigures {
    /// Whether it lies in the deck's domain: locked, r `f_S` `f_p` > 0 at its published figures,
    /// under the deck's rules ([`deck::reads_open_water`]).
    domain: bool,
    /// Whether its published pass lists liquid water.
    liquid_water: bool,
    mean_k: f64,
    air_k: f64,
    equator_pole_k: f64,
    day_night_k: f64,
    range_k: (f64, f64),
    pressure_pa: f64,
    flux_w_per_m2: f64,
    state: String,
    regime: String,
    passes: u32,
    jump: bool,
}

impl LockedFigures {
    /// One line of its figures.
    fn line(&self) -> String {
        format!(
            "domain {} liquid {} mean {:.2} K (air {:.2}), ΔT_ep {:.2}, ΔT_dn {:.2}, range \
             [{:.1}, {:.1}] K, p {:.3e} Pa, S {:.0} W m⁻², {} {}, {} passes{}",
            self.domain,
            self.liquid_water,
            self.mean_k,
            self.air_k,
            self.equator_pole_k,
            self.day_night_k,
            self.range_k.0,
            self.range_k.1,
            self.pressure_pa,
            self.flux_w_per_m2,
            self.state,
            self.regime,
            self.passes,
            if self.jump { " (a jump)" } else { "" }
        )
    }
}

/// The case's census body under the rules in force, in its system `system` ("s{index}" or a golden
/// system's name).
fn body_of(system: &str, case: &AuditCase<'_>) -> CensusBody {
    let record = case.record;
    let derived = case.derived;
    let air = derived.atmosphere();
    let text = format!(
        "{record:?} {air:?} {} {:?}",
        derived.atmosphere_passes(),
        derived.convergence()
    );
    let locked = case.surface().and_then(|surface| {
        let conditions = surface.conditions();
        let regime = conditions.climate_regime()?;
        if regime.forcing() != Forcing::Locked {
            return None;
        }
        let radius = Metres::from(record.bulk().ok()?.radius());
        let (coldest, warmest) = conditions.climate_range();
        Some(LockedFigures {
            domain: deck::reads_open_water(
                &conditions.spin(),
                radius,
                conditions.stellar_flux(),
                conditions.surface_pressure(),
            ),
            liquid_water: air.lists_liquid_water(),
            mean_k: conditions.mean_surface_temperature().value(),
            air_k: air.surface_temperature().value(),
            equator_pole_k: conditions.equator_pole_contrast_k(),
            day_night_k: conditions.day_night_contrast_k(),
            range_k: (coldest.value(), warmest.value()),
            pressure_pa: conditions.surface_pressure().value(),
            flux_w_per_m2: conditions.stellar_flux().value(),
            state: format!("{:?}", surface.state()),
            regime: format!("{regime:?}"),
            passes: derived.atmosphere_passes(),
            jump: derived.convergence().is_jump(),
        })
    });
    CensusBody {
        label: format!("{system} {} {}", record.identity().id(), case.kind),
        golden: case.set != "sample",
        digest: digest(&text),
        locked,
    }
}

/// The census of the sample's systems of indices `systems` and, where `goldens` says, the golden
/// systems, under the rules in force: every planet, member and moon with a derivation at the
/// epoch, in walk order.
fn census_of(systems: impl IntoIterator<Item = usize>, goldens: Goldens) -> Vec<CensusBody> {
    let sample = sample();
    let golden = match goldens {
        Goldens::Walked => golden_contexts(),
        Goldens::Skipped => Vec::new(),
    };
    let sets = systems
        .into_iter()
        .map(|index| {
            let ctx = sample[index].clone();
            (format!("s{index}"), ("sample", SEED, ctx))
        })
        .chain(
            golden
                .into_iter()
                .map(|(name, ctx)| (name.to_owned(), (name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx))),
        );
    let mut bodies = Vec::new();
    for (system, set) in sets {
        let set: [AuditSet; 1] = [set];
        audit_walk(&set, &mut |case| bodies.push(body_of(&system, case)));
    }
    bodies
}

/// Whether a census walks the golden systems too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goldens {
    Walked,
    Skipped,
}

/// The census of the whole sample and the golden systems ([`census_of`]).
fn census() -> Vec<CensusBody> {
    census_of(0..sample().len(), Goldens::Walked)
}

/// The labels of the bodies whose digests differ between `a` and `b`, one walk's bodies in one
/// order each.
fn moved_between(a: &[CensusBody], b: &[CensusBody]) -> Vec<String> {
    assert_eq!(a.len(), b.len(), "one walk's bodies");
    a.iter()
        .zip(b)
        .filter(|(x, y)| {
            assert_eq!(x.label, y.label, "one walk's order");
            x.digest != y.digest
        })
        .map(|(_, y)| y.label.clone())
        .collect()
}

/// The systems the deck moves, generated twice under every 26 hold, give the same bits twice, and
/// against every 25 hold only their locked worlds in the deck's domain move: the pilot's three
/// (s187, s243, s363) among them.
#[test]
fn under_every_26_hold_the_moved_systems_repeat_and_only_their_decks_move() {
    let held = with_every_25_hold(|| census_of(MOVED_SYSTEMS, Goldens::Skipped));
    let deck = deck_alone(|| census_of(MOVED_SYSTEMS, Goldens::Skipped));
    let again = deck_alone(|| census_of(MOVED_SYSTEMS, Goldens::Skipped));
    assert_eq!(
        moved_between(&deck, &again),
        Vec::<String>::new(),
        "the same bits twice"
    );
    let moved = moved_between(&held, &deck);
    eprintln!("moved: {moved:?}");
    for label in &moved {
        let body = deck.iter().find(|body| &body.label == label).unwrap();
        assert!(
            body.locked.as_ref().is_some_and(|f| f.domain),
            "{label} moved outside the deck's domain"
        );
    }
    for index in DOMAIN_SYSTEMS {
        let prefix = format!("s{index} ");
        assert!(
            moved.iter().any(|label| label.starts_with(&prefix)),
            "s{index}'s locked world moves: {moved:?}"
        );
    }
}

/// P14.T24.j's census over the 400 sample systems and the 15 golden systems, run by name: every
/// published body under every 26 hold against every 25 hold, the deck's own hold the difference.
///
/// - **Only the deck's domain moves**: a body's record and passes are 25's to the bit unless it is
///   a world locked 1:1 in the deck's domain (r `f_S` `f_p` > 0, [`deck::reads_open_water`]),
///   asserted.
/// - **A moved body keeps its open water**: its warmest climate at or above water's melting point
///   at its surface pressure, and its passes not a jump, asserted.
/// - **No locked world's published mean is non-finite**, asserted, and the walk under every 26
///   hold gives the same bits twice.
/// - Printed: the locked worlds and those in the domain, counted; each domain body under both, with
///   its regime; the three of the research's pilot (s243, s363, s187); the golden systems' bodies
///   that move, whose goldens 26's bump re-blesses; each walk's time.
#[test]
#[ignore = "a measurement for the 26 bless, run by name: the 400 sample systems, walked twice"]
fn under_every_26_hold_only_the_locked_worlds_under_their_deck_move_over_the_400() {
    let start = Instant::now();
    let held = with_every_25_hold(census);
    let walk_held = start.elapsed();
    let start = Instant::now();
    let deck = deck_alone(census);
    let walk_deck = start.elapsed();
    let again = deck_alone(census);
    assert_eq!(
        moved_between(&deck, &again),
        Vec::<String>::new(),
        "the same bits twice"
    );
    assert_eq!(held.len(), deck.len(), "one walk's bodies, both rules");
    eprintln!(
        "the walks (pod): every 25 hold {walk_held:?}, every 26 hold {walk_deck:?}, {} bodies",
        deck.len()
    );
    let (mut locked, mut domain, mut moved) = (0_u32, 0_u32, 0_u32);
    let mut faults = Vec::new();
    for (was, now) in held.iter().zip(&deck) {
        assert_eq!(was.label, now.label, "one walk's order");
        if let Some(figures) = &now.locked {
            locked += 1;
            if !figures.mean_k.is_finite() {
                faults.push(format!("{} has a mean of {} K", now.label, figures.mean_k));
            }
        }
        let in_domain = now.locked.as_ref().is_some_and(|f| f.domain);
        domain += u32::from(in_domain);
        if in_domain {
            eprintln!("in the domain {}", now.label);
            for (rules, body) in [("25", was), ("26", now)] {
                if let Some(figures) = &body.locked {
                    eprintln!("  {rules}: {}", figures.line());
                }
            }
        }
        if was.digest == now.digest {
            continue;
        }
        moved += 1;
        eprintln!("moved {}", now.label);
        let Some(figures) = now.locked.as_ref().filter(|f| f.domain) else {
            faults.push(format!("{} moved outside the deck's domain", now.label));
            continue;
        };
        let melting = ice_ih_melting_temperature(Pascals::new(figures.pressure_pa)).value();
        if figures.range_k.1 < melting {
            faults.push(format!(
                "{}'s warmest {} K lies under water's melting point {melting} K",
                now.label, figures.range_k.1
            ));
        }
        if figures.jump {
            faults.push(format!("{}'s passes jump", now.label));
        }
    }
    eprintln!("locked worlds {locked}, in the deck's domain {domain}, moved {moved}");
    let golden: Vec<&str> = held
        .iter()
        .zip(&deck)
        .filter(|(was, now)| was.digest != now.digest && now.golden)
        .map(|(_, now)| now.label.as_str())
        .collect();
    eprintln!("golden systems' bodies moved: {golden:?}");
    for index in DOMAIN_SYSTEMS {
        let prefix = format!("s{index} ");
        for (was, now) in held.iter().zip(&deck) {
            let (Some(a), Some(b)) = (&was.locked, &now.locked) else {
                continue;
            };
            if now.label.starts_with(&prefix) && (a.liquid_water || b.liquid_water || b.domain) {
                eprintln!("{}\n  held: {}\n  deck: {}", now.label, a.line(), b.line());
            }
        }
    }
    assert!(
        faults.is_empty(),
        "{}:\n  {}",
        faults.len(),
        faults.join("\n  ")
    );
}
