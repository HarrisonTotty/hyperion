//! Plan 11's P11.T9.b: the galaxy's nova rate at Milky Way parameters against the brainstorm's
//! 30–50 a year (slow), from a 1% sample of the catalogue: the accreting white dwarfs, both
//! classes, of one in a hundred of the class cells within 51,200 ly of the centre along the plane
//! and 4,096 ly off it, their novae over the last millennium counted through
//! `stellar::binary::events`. A miss is a finding against the scratch sampler's rates.

use hyperion_sim::Seed;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::CatalogueCellKey;
use hyperion_sim::galaxy::catalogue_classes::binary::{AwdProcess, AwdSide};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::stellar::binary::events::{BinaryEventKind, BinarySystem, events_in};
use hyperion_sim::stellar::binary::scan::scan_cell;
use hyperion_sim::time::UniverseTime;

/// The Milky Way fixture's seed.
const SEED: u64 = 0x0b17_09b0_0000_0002;

/// The share of the class cells sampled: one in [`SAMPLE`].
const SAMPLE: i64 = 100;

/// Whether cell `cell` is one of the sample's: a hash of its coordinates, one cell in [`SAMPLE`].
fn sampled(cell: [i32; 3]) -> bool {
    let [x, y, z] = cell.map(i64::from);
    let h = (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663) ^ z.wrapping_mul(83_492_791))
        .wrapping_mul(0x5851_f42d_4c95_7f2d)
        .rotate_left(29);
    h.rem_euclid(SAMPLE) == 0
}

/// The galaxy's novae a year at Milky Way parameters, from the sample: each class's novae a host
/// over the last thousand years, times its expected hosts (a ratio over the hosts its sampled
/// cells hold, so that the cells' own spread cancels), against the brainstorm's 30–50 a year, the
/// Galactic rate's estimates of Shafter (2017, ApJ 834, 196: 50 (+31, −23)) and De et al. (2021,
/// ApJ 912, 19: 43.7 (+19.5, −8.7)) (recalled, not read at source).
#[test]
#[ignore = "slow: a 1% sample of the galaxy's accreting white dwarfs"]
fn the_galaxy_s_nova_rate_at_milky_way_parameters() {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let since = UniverseTime::from_julian_years(-1_000).expect("on the clock");
    let window = TimeWindow::new(since, UniverseTime::EPOCH).expect("ordered");
    let mut hosts = Vec::new();
    let mut events = Vec::new();
    let mut rate = 0.0;
    let mut by_cells = 0.0;
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let process = AwdProcess::new(&galaxy, side);
        let (mut novae, mut sampled_hosts) = (0_usize, 0_usize);
        for y in -100..100 {
            for x in -100..100 {
                for z in -8..8 {
                    if !sampled([x, y, z]) {
                        continue;
                    }
                    let key = CatalogueCellKey::new(side.class(), [x, y, z]).expect("a cell");
                    hosts.clear();
                    scan_cell(&galaxy, &process, key, &mut hosts);
                    sampled_hosts += hosts.len();
                    for host in &hosts {
                        events.clear();
                        let system = BinarySystem::accreting_white_dwarf_scan(galaxy.seed(), host);
                        events_in(&system, window, &mut events);
                        novae += events
                            .iter()
                            .filter(|e| e.kind() == BinaryEventKind::Nova)
                            .count();
                    }
                }
            }
        }
        #[expect(clippy::cast_precision_loss, reason = "counts of some thousands")]
        let (novae_f, hosts_f) = (novae as f64, sampled_hosts as f64);
        let class_rate = novae_f / hosts_f * process.expected_hosts(&galaxy) / 1_000.0;
        #[expect(clippy::cast_precision_loss, reason = "a share of a hundred")]
        let share = SAMPLE as f64;
        by_cells += novae_f * share / 1_000.0;
        println!(
            "{side:?}: {novae} novae of {sampled_hosts} sampled hosts over 1,000 years, \
             {class_rate:.1} a year"
        );
        rate += class_rate;
    }
    println!("{rate:.1} novae a year in the galaxy ({by_cells:.1} by the cells' share)");
    assert!((30.0..=50.0).contains(&rate), "{rate}");
}
