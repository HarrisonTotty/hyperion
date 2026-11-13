//! The tests of the X-ray binaries' high-mass side at its intrinsic count (P11.T8.b's fix,
//! research-leans-51; on the record from generator version 32): the galaxy's high-mass hosts, their
//! kinds, a Be/X host's active mark and the giant outbursts it gives.

use std::sync::OnceLock;

use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};

use super::{XrayBinaries, XrbSide};
use crate::Seed;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
use crate::galaxy::params::GalaxyParams;
use crate::rng::ObjectKey;
use crate::stellar::binary::events::{BEX_ACTIVE_SHARE, BEX_GIANT_OUTBURST_RATE_PER_YEAR};
use crate::stellar::binary::scan::{XrbScanMarks, bex_active_mark, scan_cell};
use crate::tables::binary::{BLACK_HOLE_TRANSIENT_SHARE, XRB_SAMPLER, XrbHostKind, XrbPick};

/// The galaxy's expected hosts on `side` under `process`, before the class's test and the young
/// lists' thinning by their members' ages: the class's and the young features' lists' together,
/// each component's catalogue hosts over the share of them the catalogue keeps. The tests' one
/// reading of the galaxy's high-mass hosts (here and `fitted_tables`).
pub(super) fn galaxy_hosts_on(galaxy: &Galaxy, process: &XrayBinaries, side: XrbSide) -> f64 {
    let row =
        crate::tables::binary::CLASS_SHARES.row(crate::stellar::binary::CarvedClass::XrayBinary);
    let fields = galaxy.fields();
    process
        .components()
        .iter()
        .filter(|c| c.side() == side)
        .map(|c| {
            let component = fields.component(c.id());
            c.hosts_per_system() * component.count_with_unborn()
                / super::feature::catalogue_share(galaxy, row, component)
        })
        .sum()
}

/// The Galaxy's intrinsic high-mass X-ray binaries that task `binary_xrb`'s manifest pins the
/// high-mass side to: 3,000, about the log-centre of the 10³–10⁴ Be/X-ray binaries of Rappaport and
/// van den Heuvel (1982, IAU Symp. 98, 327; recalled, not read at source; research-leans-51).
const PINNED_SIDE_HOSTS: f64 = 3_000.0;

/// The high-mass kinds the manifest pins inside the side, with their hosts: the supergiant hosts'
/// 40 and the black-hole high-mass hosts' 2, about the catalogued systems (Liu et al. 2006, A&A
/// 455, 1165, about 36 supergiant ones; Cyg X-1 and a second allowed; recalled).
const PINNED_PERSISTENT_KINDS: [(XrbHostKind, f64); 2] = [
    (XrbHostKind::Supergiant, 40.0),
    (XrbHostKind::BlackHoleHighMass, 2.0),
];

/// The Galaxy's giant outbursts of Be/X-ray binaries a year that P11.T9.b's and P12.T5.c's rate is
/// held inside: 1–10, the few a year MAXI and Swift/BAT see (recalled, rough; research-leans-51).
const GIANT_OUTBURSTS_PER_YEAR: (f64, f64) = (1.0, 10.0);

/// The Milky Way fixture, built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(
            Seed::new(0x0b17_08b0_0000_0032),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture builds")
    })
}

/// The galaxy's expected high-mass hosts of each high-mass kind under `process`, in
/// [`XrbHostKind::HIGH_MASS`]' order: the side's hosts in its sampler's weights.
fn galaxy_high_mass_kinds(galaxy: &Galaxy, process: &XrayBinaries) -> [f64; 3] {
    let high = galaxy_hosts_on(galaxy, process, XrbSide::HighMass);
    let weights = process
        .sampler()
        .table()
        .pick(XrbPick::HighMassKind)
        .weights();
    let sum: f64 = weights.iter().sum();
    std::array::from_fn(|k| high * weights[k] / sum)
}

/// The class draws from [`XRB_SAMPLER`]: the galaxy's expected high-mass hosts, the class's and the
/// young features' lists' together, are the intrinsic 3,000, the supergiant hosts 40 and the
/// black-hole high-mass hosts 2 of them, the Be/X hosts the rest; its hosts are still the row's 10⁴
/// and its black-hole transients 1,300.
#[test]
fn the_high_mass_side_holds_the_intrinsic_hosts() {
    let galaxy = galaxy();
    let process = XrayBinaries::new(galaxy);
    assert_eq!(process.sampler().table(), &XRB_SAMPLER);
    let low = galaxy_hosts_on(galaxy, &process, XrbSide::LowMass);
    let high = galaxy_hosts_on(galaxy, &process, XrbSide::HighMass);
    let catalogued = process.expected_hosts_on(galaxy, XrbSide::HighMass);
    let kinds = galaxy_high_mass_kinds(galaxy, &process);
    println!(
        "{:.1} hosts, {high:.2} high-mass (the catalogue's {catalogued:.2}, a share of {:.4}; by \
         kind {kinds:.2?}), against the intrinsic {PINNED_SIDE_HOSTS}",
        low + high,
        catalogued / high
    );
    assert!(((low + high) / 1.0e4 - 1.0).abs() < 1e-9, "{}", low + high);
    assert!((high / PINNED_SIDE_HOSTS - 1.0).abs() < 1e-9, "{high}");
    assert!(catalogued > 0.0 && catalogued < high, "{catalogued}");
    let pinned: f64 = PINNED_PERSISTENT_KINDS.iter().map(|&(_, h)| h).sum();
    for (k, kind) in XrbHostKind::HIGH_MASS.into_iter().enumerate() {
        let wanted = PINNED_PERSISTENT_KINDS
            .iter()
            .find(|&&(pinned_kind, _)| pinned_kind == kind)
            .map_or(PINNED_SIDE_HOSTS - pinned, |&(_, h)| h);
        assert!(
            (kinds[k] / wanted - 1.0).abs() < 1e-9,
            "{kind:?}: {} against {wanted}",
            kinds[k]
        );
    }
    let low_weights = XRB_SAMPLER.pick(XrbPick::LowMassKind).weights();
    let transients = XrbHostKind::LOW_MASS
        .iter()
        .position(|&k| k == XrbHostKind::BlackHoleTransient)
        .expect("a low-mass kind");
    let low_sum: f64 = low_weights.iter().sum();
    let black_hole_transients = low * low_weights[transients] / low_sum;
    assert!(
        (black_hole_transients / 1_300.0 - 1.0).abs() < 1e-9,
        "{black_hole_transients}"
    );
    assert!((low_weights[transients] - BLACK_HOLE_TRANSIENT_SHARE).abs() < 1e-15);
    let share: f64 = XRB_SAMPLER
        .pick(XrbPick::HighMassKind)
        .weights()
        .iter()
        .sum();
    assert!((share - PINNED_SIDE_HOSTS / 1.0e4).abs() < 1e-15, "{share}");
}

/// The galaxy's Be/X giant outbursts a year, its Be/X hosts × [`BEX_ACTIVE_SHARE`] ×
/// [`BEX_GIANT_OUTBURST_RATE_PER_YEAR`], are inside the few a year seen
/// ([`GIANT_OUTBURSTS_PER_YEAR`]; P11.T9.b, P12.T5.c), where every host active would give some 300.
/// The hosts drawn lie between the catalogue's expected Be/X hosts, which the class's test does not
/// refuse (a Be/X host is marked detached, with no rate to floor), and those with the young lists'
/// added before their thinning by their members' ages, which can only lower it; both bounds are
/// inside (the outburst rate counted before the lists' age thinning: a lean, 2026-11-13).
#[test]
fn the_galaxy_s_be_x_giant_outbursts_are_a_few_a_year() {
    let galaxy = galaxy();
    let process = XrayBinaries::new(galaxy);
    let be_x = galaxy_high_mass_kinds(galaxy, &process)[0];
    let weights = process
        .sampler()
        .table()
        .pick(XrbPick::HighMassKind)
        .weights();
    let catalogue_be_x = process.expected_hosts_on(galaxy, XrbSide::HighMass) * weights[0]
        / weights.iter().sum::<f64>();
    let per_host = BEX_ACTIVE_SHARE * BEX_GIANT_OUTBURST_RATE_PER_YEAR;
    let (fewest, most_per_year) = (catalogue_be_x * per_host, be_x * per_host);
    let all_active = be_x * BEX_GIANT_OUTBURST_RATE_PER_YEAR;
    println!(
        "{catalogue_be_x:.1}–{be_x:.1} Be/X hosts, {fewest:.2}–{most_per_year:.2} giant outbursts a \
         year ({all_active:.0} with every host active)"
    );
    let (least, most) = GIANT_OUTBURSTS_PER_YEAR;
    for per_year in [fewest, most_per_year] {
        assert!((least..=most).contains(&per_year), "{per_year}");
    }
    assert!(all_active > most, "{all_active}");
}

/// The active mark accepts its share of hosts: over 2 × 10⁵ hosts' keys, [`BEX_ACTIVE_SHARE`] of
/// them by chi-square at the project's α, and the same mark for the same key.
#[test]
fn the_active_mark_accepts_one_host_in_a_hundred() {
    let seed = Seed::new(0x0b17_08b0_0000_0032);
    let n = 200_000_u32;
    let active = (0..n)
        .filter(|&k| bex_active_mark(seed, ObjectKey::system(0x1234_0000_0000 + u64::from(k))))
        .count();
    let active = u64::try_from(active).expect("a count under 2⁶⁴");
    let total = f64::from(n);
    let fit = chi_square_gof(
        &[active, u64::from(n) - active],
        &[BEX_ACTIVE_SHARE * total, (1.0 - BEX_ACTIVE_SHARE) * total],
    );
    println!(
        "{active} of {n} active against {BEX_ACTIVE_SHARE}, p {:.3}",
        fit.p_value
    );
    assert_p_value(
        "the active share against BEX_ACTIVE_SHARE",
        fit.p_value,
        ALPHA,
    );
    let key = ObjectKey::system(0x1234_0000_0007);
    assert_eq!(bex_active_mark(seed, key), bex_active_mark(seed, key));
}

/// The X-ray binaries the plane's cells within some 8,000 ly of the centre hold under `galaxy`'s
/// rules, drawn through `process`.
fn plane_hosts(galaxy: &Galaxy, process: &XrayBinaries) -> Vec<XrbScanMarks> {
    let mut hosts = Vec::new();
    for x in -16..16 {
        for y in -16..16 {
            let key =
                CatalogueCellKey::new(ClassId::XRAY_BINARY, [x, y, 0]).expect("a cell of the disc");
            scan_cell(galaxy, process, key, &mut hosts);
        }
    }
    hosts
}

/// A Be/X host is active exactly where its mark says, and no host of another kind is.
#[test]
fn a_be_x_host_is_active_by_its_mark() {
    let galaxy = galaxy();
    let hosts = plane_hosts(galaxy, &XrayBinaries::new(galaxy));
    let be_x = hosts
        .iter()
        .filter(|h| h.kind() == XrbHostKind::BeX)
        .count();
    println!("{} hosts, {be_x} Be/X", hosts.len());
    assert!(be_x >= 20, "{be_x} Be/X hosts");
    for host in &hosts {
        let wanted =
            host.kind() == XrbHostKind::BeX && bex_active_mark(galaxy.seed(), host.host().key());
        assert_eq!(host.is_bex_active(), wanted, "{host:?}");
    }
}
