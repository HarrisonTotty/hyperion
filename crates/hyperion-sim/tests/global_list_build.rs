//! The global list over many galaxies (plan 10, P10.T4): how many tubes it holds, and that the
//! specifications and shells it holds do not depend on the order lists are built in.
//!
//! Each galaxy needs the full potential and the kinematic tables, and its list walks every
//! globular and integrates one orbit per stream, so both tests are slow.

use std::sync::OnceLock;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::global_list::{GlobalList, MassLoss, StreamOrigin};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::order::assert_order_independent;

/// Milky Way galaxies, differing only in their seed: P10.T3's seed family, whose debris plan 10's
/// records count.
const MILKY_WAY_SEED: u64 = 0x0a10_3000_0000_0000;

/// Drawn galaxies of P10.T3.d.a's sweep (`share::tests`), whose first 2,000 its summary holds.
const SWEEP_SEED: u64 = 0x0a10_3d00_0000_0000;

fn milky_way(n: u64) -> Galaxy {
    Galaxy::from_params(
        Seed::new(MILKY_WAY_SEED | n),
        GalaxyParams::milky_way_like(),
    )
    .expect("the fixture's parameters are valid")
    .with_full_potential()
}

fn drawn(n: u64) -> Galaxy {
    let seed = Seed::new(SWEEP_SEED | n);
    Galaxy::from_params(
        seed,
        GalaxyParams::from_seed(seed, MassFunctionKind::default()),
    )
    .expect("a drawn galaxy builds")
    .with_full_potential()
}

/// A digest of the bits of every stream's specification as the list holds it (its epoch state,
/// turning points, stripping time, masses, tidal radius, mass loss and stars, the orphans' among
/// them), and of every entry's mass and shell, which the list's own equality would not tell from a
/// signed zero's.
fn digest(list: &GlobalList) -> (usize, u64, u64) {
    let mut specs = Vec::new();
    for s in list.debris().streams() {
        specs.extend(s.position().metres());
        specs.extend(s.velocity().metres_per_second());
        specs.push(s.orbit().pericentre().value());
        specs.push(s.orbit().apocentre().value());
        specs.push(s.stripping_time().value());
        specs.push(s.mass().value());
        specs.push(s.tidal_radius().value());
        specs.push(s.progenitor_mass().value());
        match s.mass_loss() {
            MassLoss::Steady(rate) => specs.push(rate.value()),
            MassLoss::Dissolved { ago } => specs.push(ago.value()),
            MassLoss::Pulsed {
                fraction,
                last_pericentre,
                ..
            } => specs.extend([fraction, last_pericentre.value()]),
        }
        if let Some(stars) = s.stars() {
            specs.extend([stars.fe_h().value(), stars.age().value()]);
        }
    }
    let entries: Vec<f64> = list
        .entries()
        .iter()
        .flat_map(|e| {
            [
                e.mass().value(),
                e.shell().inner().value(),
                e.shell().outer().value(),
            ]
        })
        .collect();
    (
        list.entries().len(),
        f64_digest(&specs),
        f64_digest(&entries),
    )
}

/// Plan 10's "150–1,500 tubes over seeds and about 240 at Milky Way parameters" (P10.T4).
///
/// Every list of eight Milky Way galaxies and sixteen drawn galaxies holds 150–1,500 tubes (the
/// drawn sixteen 154–893 at v23), and "about 240" is held as the Milky Way galaxies' mean within
/// 200–300: they measure 269.8, 1.5 tubes for each of the 179 globulars the fixture's parameters
/// expect, where 240 is 1.5 for each of some 160. Over more seeds the floor does not hold: plan
/// 02's globular count is at least 80, so a galaxy expects at least 120 tubes, and 228 of the
/// 2,000 summary galaxies of P10.T3.d.a expect fewer than 150 (a finding for the owner).
#[test]
#[ignore = "slow: builds the full potential, the kinematic tables and the list of 24 galaxies"]
fn lists_hold_150_to_1500_tubes_and_about_240_at_milky_way_parameters() {
    let tubes = |galaxy: &Galaxy, label: &str| {
        let list = GlobalList::build(galaxy);
        let debris = list.debris();
        let streams = debris.streams();
        let orphans = streams
            .iter()
            .filter(|s| s.origin() == StreamOrigin::Orphan)
            .count();
        let dwarfs = streams
            .iter()
            .filter(|s| matches!(s.origin(), StreamOrigin::Dwarf(_)))
            .count();
        let empty = list
            .entries()
            .iter()
            .filter(|e| e.shell().is_empty())
            .count();
        eprintln!(
            "{label}: {} tubes ({orphans} orphans, {dwarfs} dwarfs), {} cores, {} expected \
             globulars; {empty} shells wholly outside the root cube",
            streams.len(),
            debris.cores().len(),
            galaxy.params().accretion().globular_count()
        );
        assert!(
            (150..=1_500).contains(&streams.len()),
            "{label}: {} tubes",
            streams.len()
        );
        streams.len()
    };
    let mut milky_way_tubes = 0;
    for n in 0..8 {
        milky_way_tubes += tubes(&milky_way(n), &format!("Milky Way {n}"));
    }
    let mean = f64::from(u32::try_from(milky_way_tubes).expect("a few thousand")) / 8.0;
    eprintln!("Milky Way mean {mean:.1} tubes against plan 10's about 240");
    assert!((200.0..=300.0).contains(&mean), "{mean} tubes");
    for n in 0..16 {
        tubes(&drawn(n), &format!("drawn {n}"));
    }
}

/// Plan 10's order independence of the specifications (P10.T4), over the lists of two Milky Way
/// galaxies, each built once and read by every pass: forwards, backwards, permuted and alone, each
/// list is the same, whole and in the bits of every stream's specification (the orphans'
/// positions, velocities and turning points among them) and every shell, whichever list was built
/// before it and whether or not its galaxy's centre had been built.
#[test]
#[ignore = "slow: builds two galaxies' full potentials and kinematic tables, and eight lists"]
fn lists_do_not_depend_on_the_order_they_are_built_in() {
    static GALAXIES: OnceLock<[Galaxy; 2]> = OnceLock::new();
    let galaxies = GALAXIES.get_or_init(|| [milky_way(0), milky_way(1)]);
    assert_order_independent(&[0_usize, 1], |&n| {
        let list = GlobalList::build(&galaxies[n]);
        (digest(&list), list)
    });
}
