//! P14.T55.b over the record, under its hold at 35 ([`with_every_35_hold`]): every surface holding
//! melt holds its melt's vapour, every other none, and nothing but the vapour and a volcanic lid's
//! melt area moves; below 35 (the version's rules) no record holds a vapour. The census of the
//! ordinary systems is `census_tests`'s.

use super::tests::{GOLDEN_SYSTEMS_SEED, SEED, golden_contexts, sample};
use super::*;
use crate::planetary::chem::silicate_vapour::{
    ATOMIC_CRITICAL_JEANS_PARAMETER, DIATOMIC_CRITICAL_JEANS_PARAMETER, VapourEscape,
};
use crate::planetary::record::BulkProperties;
use crate::planetary::testing::with_every_35_hold;
use crate::units::consts::BOLTZMANN_CONSTANT;
use hyperion_testkit::float::bits;

/// How many of the sample's systems the fast test walks, the first in its order.
const SAMPLE_SYSTEMS: usize = 60;

/// The systems the fast test walks: the first [`SAMPLE_SYSTEMS`] of the sample and the 15 golden
/// ones, each with its set's name and its universe's seed.
fn fast_sets() -> Vec<(&'static str, Seed, SystemContext)> {
    sample()[..SAMPLE_SYSTEMS]
        .iter()
        .map(|ctx| ("sample", SEED, ctx.clone()))
        .chain(
            golden_contexts()
                .into_iter()
                .map(|(name, ctx)| (name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx)),
        )
        .collect()
}

/// The records of the systems `sets` at the epoch, each generated afresh under the rules in force
/// on this thread, in body order, each with its set and ID.
fn records(sets: &[(&'static str, Seed, SystemContext)]) -> Vec<(String, BodyRecord)> {
    let mut records = Vec::new();
    for (set, seed, ctx) in sets {
        let system = generate(*seed, ctx);
        for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
            records.push((format!("{set} {}", record.identity().id()), record.clone()));
        }
    }
    records
}

/// Whether `surface` holds melt, a fact on the wire (decision-backlog-2 §2.3 item 4): the state
/// `MagmaOcean`, or a crust whose melt area is above 0.
pub(super) fn holds_melt(surface: &Surface) -> bool {
    surface.state() == SurfaceState::MagmaOcean
        || surface
            .crust()
            .ok()
            .is_some_and(|crust| crust.melt_area_fraction() > 0.0)
}

/// What is wrong with the vapour `vapour` over the surface `surface` of a body of bulk section
/// `bulk`, `None` if nothing: every figure finite and positive, the species summing to 1 and
/// largest first, its λ its bulk section's g R m ÷ (k T) within 10⁻¹², its class of escape its
/// λ's against its critical value, and its temperature its melt's warmest to the bit: a starlit
/// melt's peak (`SurfaceConditions::peak_temperature`), or a volcanic lid's eruption temperature
/// where that is warmer or no starlight melts it.
pub(super) fn vapour_fault(
    vapour: &SilicateVapour,
    surface: &Surface,
    bulk: &BulkProperties,
) -> Option<String> {
    let t = vapour.melt_temperature().value();
    let figures = [
        t,
        vapour.pressure().value(),
        vapour.mean_molar_mass_g_per_mol(),
        vapour.jeans_parameter(),
    ];
    if !figures.iter().all(|x| x.is_finite() && *x > 0.0) {
        return Some(format!("a figure not finite and positive: {figures:?}"));
    }
    let species = vapour.species();
    let sum = species.iter().fold(0.0, |sum, &(_, x)| sum + x);
    if (sum - 1.0).abs() > 1e-12 || !species.windows(2).all(|pair| pair[0].1 >= pair[1].1) {
        return Some(format!("species not a mixture: {species:?}"));
    }
    let m = vapour.mean_molar_mass_g_per_mol()
        * crate::planetary::derive::atmosphere::ATOMIC_MASS_CONSTANT_KG;
    let lambda = bulk.surface_gravity().value() * Metres::from(bulk.radius()).value() * m
        / (BOLTZMANN_CONSTANT * t);
    if (vapour.jeans_parameter() / lambda - 1.0).abs() > 1e-12 {
        return Some(format!("λ {} against {lambda}", vapour.jeans_parameter()));
    }
    let critical = vapour.critical_jeans_parameter();
    if critical.total_cmp(&ATOMIC_CRITICAL_JEANS_PARAMETER).is_ne()
        && critical
            .total_cmp(&DIATOMIC_CRITICAL_JEANS_PARAMETER)
            .is_ne()
    {
        return Some(format!("critical λ {critical}"));
    }
    let escape = if vapour.jeans_parameter() < critical {
        VapourEscape::Outflow
    } else {
        VapourEscape::Bound
    };
    if vapour.escape() != escape {
        return Some(format!("{:?} at λ {lambda}", vapour.escape()));
    }
    let lava = surface
        .crust()
        .ok()
        .and_then(|crust| crust.active_lava())
        .map(|lava| lava.eruption_temperature().value());
    let starlit = (surface.state() == SurfaceState::MagmaOcean
        || surface
            .crust()
            .ok()
            .is_some_and(|crust| crust.starlit_melt_fraction() > 0.0))
    .then(|| surface.conditions().peak_temperature().value());
    let expected = match (starlit, lava) {
        (Some(peak), Some(lava)) if lava > peak => lava,
        (Some(t), _) | (None, Some(t)) => t,
        (None, None) => return Some("a vapour over neither a starlit melt nor lava".to_owned()),
    };
    if bits(t) != bits(expected) {
        return Some(format!("{t} K against its melt's warmest, {expected} K"));
    }
    None
}

/// The record `record`'s members but its surface's, and its surface's but its crust and its
/// vapour, as text: what the hold must leave to the bit.
fn unheld(record: &BodyRecord) -> String {
    let surface = match record.surface() {
        Section::Ok(surface) => format!(
            "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
            surface.state(),
            surface.published_state(),
            surface.material(),
            surface.climate_regime(),
            surface.conditions(),
            surface.figures(),
            surface.vertical_structure(),
            surface.aerosols(),
            surface.condensates(),
            surface.modifiers(),
            surface.taxonomic_class(),
        ),
        other => format!("{other:?}"),
    };
    format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} | {surface}",
        record.identity(),
        record.position(),
        record.mass(),
        record.orbit(),
        record.moons(),
        record.rings(),
        record.population(),
        record.bulk(),
        record.hooks(),
        record.photometry(),
        record.figure(),
        record.rotation(),
        record.envelope(),
    )
}

/// The crust `crust`'s members but its melt area, as text.
fn crust_but_its_melt(crust: &CrustComposition) -> String {
    format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        crust.primary(),
        crust.secondary(),
        crust.tertiary(),
        crust.bulk(),
        crust.lid(),
        crust.provinces(),
        crust.redox(),
        crust.mantle_delta_iw(),
        crust.thermal_history(),
        crust.peak_central_temperature(),
    )
}

/// Over the first 60 sample systems and the golden ones: at the version's rules no surface holds a
/// vapour; under every 35 hold each surface holding melt holds one with no fault
/// ([`vapour_fault`]) and every other none, a crust's starlit share is the version's melt area to
/// the bit and moves only by a volcanic lid's active lava, and every other member of every record
/// keeps its bits; the walk reaches a magma ocean, a pool, a volcanic lid and an outflow, and the
/// golden systems' records under the hold are the same twice. The counts by kind are printed.
#[test]
fn under_every_35_hold_a_surface_holds_its_melt_s_vapour_and_none_elsewhere() {
    let sets = fast_sets();
    let held = records(&sets);
    let modelled = with_every_35_hold(|| records(&sets));
    assert_eq!(held.len(), modelled.len());
    let (mut surfaces, mut molten, mut pools, mut volcanic, mut outflows) = (0, 0, 0, 0, 0);
    for ((label, held), (other, modelled)) in held.iter().zip(&modelled) {
        assert_eq!(label, other);
        assert_eq!(unheld(held), unheld(modelled), "{label}");
        let (Section::Ok(was), Section::Ok(surface)) = (held.surface(), modelled.surface()) else {
            continue;
        };
        surfaces += 1;
        assert_eq!(was.silicate_vapour(), Modelled::NotModelled, "{label}");
        match (was.crust(), surface.crust()) {
            (Modelled::Ok(was), Modelled::Ok(crust)) => {
                assert_eq!(
                    crust_but_its_melt(was),
                    crust_but_its_melt(crust),
                    "{label}"
                );
                assert_eq!(
                    bits(crust.starlit_melt_fraction()),
                    bits(was.melt_area_fraction()),
                    "{label}"
                );
                if crust.active_lava().is_none() {
                    assert_eq!(
                        bits(crust.melt_area_fraction()),
                        bits(was.melt_area_fraction()),
                        "{label}"
                    );
                } else {
                    volcanic += 1;
                }
            }
            (Modelled::NotModelled, Modelled::NotModelled) => {}
            (was, crust) => panic!("{label}: the crust {was:?} became {crust:?}"),
        }
        let Section::Ok(bulk) = modelled.bulk() else {
            panic!("{label}: a surface with no bulk section");
        };
        match surface.silicate_vapour() {
            Modelled::Ok(Some(vapour)) => {
                assert!(holds_melt(surface), "{label}: a vapour with no melt");
                if let Some(fault) = vapour_fault(vapour, surface, bulk) {
                    panic!("{label}: {fault}");
                }
                if surface.state() == SurfaceState::MagmaOcean {
                    molten += 1;
                } else {
                    pools += 1;
                }
                if vapour.escape() == VapourEscape::Outflow {
                    outflows += 1;
                }
                eprintln!(
                    "{label} {:?}: {:.1} K, {:.3e} Pa, {} {:.3}, λ {:.3} {:?}",
                    surface.state(),
                    vapour.melt_temperature().value(),
                    vapour.pressure().value(),
                    vapour.species()[0].0.substance().key().as_str(),
                    vapour.species()[0].1,
                    vapour.jeans_parameter(),
                    vapour.escape(),
                );
            }
            Modelled::Ok(None) => assert!(!holds_melt(surface), "{label}: melt with no vapour"),
            Modelled::NotModelled => panic!("{label}: the vapour is not modelled under its hold"),
        }
    }
    eprintln!(
        "{surfaces} surfaces: {molten} magma oceans and {pools} other melts with a vapour, \
         {volcanic} volcanic lids with active lava, {outflows} outflows"
    );
    assert!(molten > 0, "the walk holds a magma ocean");
    assert!(
        pools > 0 && volcanic > 0,
        "the walk holds a pool and a volcanic lid"
    );
    assert!(outflows > 0, "the walk holds an outflow");
    // The same systems again under the hold give the same records, to the text of their bits.
    let again = with_every_35_hold(|| records(&sets[SAMPLE_SYSTEMS..]));
    for ((label, first), (_, second)) in modelled[modelled.len() - again.len()..].iter().zip(&again)
    {
        assert_eq!(format!("{first:?}"), format!("{second:?}"), "{label}");
    }
}
