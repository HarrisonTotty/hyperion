//! P10.T12's slow statistical tests and cost figures (plan 10): the test streams' members over a
//! millennium, the discrete share's budgets over the sweep, a drawn galaxy's tubes against the
//! brainstorm's figures and its streams' contrast, and the share of members option A drops from
//! version 26 (research-leans-10's tripwire).
//!
//! Every figure is measured and printed. One outside the brainstorm's range is a finding, pinned as
//! it stands and not tuned (the lane rules). The budget test's second half, the sampled halo field
//! plus the discrete members in eight volumes, waits for P10.T3.d.b's call, P10.T10.b's
//! registration and P09.T2.c's `field_factor` in the fields (plan 10, P10.T12), so it is not here.
//! The benches of P10.T4, T6.c and T9 are recorded where they run (`benches/global_list.rs` and
//! `GlobalList::build`'s documentation), with the machine.

use crate::Seed;
use crate::coords::{GalacticDisplacement, GalacticPosition};
use crate::galaxy::global_list::debris::in_list_order;
use crate::galaxy::global_list::dwarfs::dwarfs;
use crate::galaxy::global_list::members::{
    GlobalMemberRecord, StreamBandShares, member_coordinates, stream_cell_members,
};
use crate::galaxy::global_list::orbit::dot;
use crate::galaxy::global_list::share::{INSIDE_LY, ShareFigures};
use crate::galaxy::global_list::spray::{Spray, SprayForce, SprayParams, Tracer};
use crate::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
use crate::galaxy::global_list::tube::{ACROSS_CELLS, TubeCell, TubeKnot};
use crate::galaxy::global_list::{
    BoundingShell, Debris, GlobalList, StreamOrigin, StreamSpec, TubeTable,
};
use crate::galaxy::imf::{MassBand, MassFunctionKind};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::potential::{MassModel, PotentialTables};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{Layer, StreamMemberId};
use crate::planetary::testing::in_shares;
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};

/// The test streams' seed: their galaxy's, their sprays' and their gap noise's.
const SEED: Seed = Seed::new(0x0a10_c000_0000_0000);

/// The sweep's first galaxy's seed, P10.T3.d.a's (`share.rs`): galaxy `n` is this `| n`.
const SWEEP: u64 = 0x0a10_3d00_0000_0000;

/// The slots each threaded test takes (`.config/nextest.toml`'s slow profile).
const THREADS: usize = 8;

/// The slots the millennium test takes, one for each test stream's spray (`.config/nextest.toml`'s
/// slow profile).
const SPRAY_THREADS: usize = 3;

/// Every stellar band, the coarsest first.
const BANDS: [MassBand; 5] = [
    MassBand::E,
    MassBand::D,
    MassBand::C,
    MassBand::B,
    MassBand::A,
];

/// The disc's reference density, systems per cubic light-year, the solar neighbourhood's (brainstorm,
/// "Sizing the layers"; the substellar stage's `REFERENCE_SYSTEMS_PER_LY3`), against which a
/// stream's axis is "a thousandth" (brainstorm, "Streams and accreted structure").
const REFERENCE_DENSITY: f64 = 0.003;

/// 15 and 18 kpc in light-years (1 kpc = 3,261.56 ly, IAU 2015 B2's parsec), where the brainstorm
/// reads a stream's contrast.
const CONTRAST_RANGE_LY: [f64; 2] = [48_923.4, 58_708.1];

/// The distance of `x` from the galactic centre, ly.
fn radius_ly(x: &GalacticDisplacement) -> f64 {
    let [a, b, c] = x.metres();
    (a * a + b * b + c * c).sqrt() / METRES_PER_LIGHT_YEAR
}

/// `position` about the galactic centre.
fn about_centre(position: &GalacticPosition) -> GalacticDisplacement {
    GalacticPosition::ORIGIN.displacement_to(position)
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// The least, the median and the greatest of `values`.
fn spread(mut values: Vec<f64>) -> [f64; 3] {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    [values[0], values[n / 2], values[n - 1]]
}

/// The share of `tracers`' weight inside [`INSIDE_LY`] of the galactic centre: how the spray puts a
/// tube's mass inside the share's sphere.
fn weight_inside(tracers: &[Tracer]) -> f64 {
    let (mut inside, mut all) = (0.0, 0.0);
    for t in tracers {
        let w = t.weight().value();
        all += w;
        if radius_ly(&t.position()) < INSIDE_LY {
            inside += w;
        }
    }
    inside / all
}

/// Sweep galaxy `n`, with its kinematic tables.
fn sweep_galaxy(n: u64) -> Galaxy {
    let seed = Seed::new(SWEEP | n);
    Galaxy::from_params(
        seed,
        GalaxyParams::from_seed(seed, MassFunctionKind::default()),
    )
    .expect("a drawn galaxy builds")
    .with_full_potential()
}

/// About `count` members of `tube`'s stream spread along it, each with its cell: from each of up to
/// 1,000 along cells spaced evenly, ten times their share of `count` with their across cells taken
/// in a fixed scatter (ten, for a tube most of whose cells lie outside the root cube, where no
/// member is placed), then every so many kept so that `count` remain, where that many lie there.
fn spread_members(
    galaxy: &Galaxy,
    tube: &TubeTable,
    shares: &StreamBandShares,
    count: usize,
) -> Vec<(GlobalMemberRecord, TubeCell)> {
    let cells = tube.along_cells();
    let positions = cells.min(1_000);
    let per_along = (10 * count).div_ceil(usize::try_from(positions).expect("small"));
    let across = u32::from(ACROSS_CELLS) * u32::from(ACROSS_CELLS);
    let mut out = Vec::new();
    let mut members = Vec::new();
    for k in 0..positions {
        let along = u16::try_from(k * cells / positions).expect("under 16,384");
        let mut taken = 0;
        // 2,731 is prime to 4,096, so its multiples visit every across cell once, scattered.
        for j in (0..across).map(|i| i * 2_731 % across) {
            let [a, b] = [j / u32::from(ACROSS_CELLS), j % u32::from(ACROSS_CELLS)]
                .map(|c| u8::try_from(c).expect("under 64"));
            let cell: TubeCell =
                StreamMemberId::new(tube.stream().get(), Layer::A, along, [a, b], 0)
                    .expect("a tube's cell")
                    .into();
            for band in BANDS {
                stream_cell_members(galaxy, tube, shares, cell, band, &mut members);
                out.extend(members.iter().map(|m| (*m, cell)));
                taken += members.len();
            }
            if taken >= per_along {
                break;
            }
        }
    }
    if out.len() > count {
        let all = out.len();
        out = (0..count).map(|k| out[k * all / count]).collect();
    }
    out
}

/// Each test stream's median distance moved along its tube in 1,000 years, ly, as measured
/// (2026-10-31, x86-64; least and most for the record): `pal5_like` 0.535 (0.46–0.92, 90% in
/// 0.3–0.8), `gd1_like` 0.828 (0.66–0.86, 25%), `sagittarius_like` 0.353 (0.00–1.04, 34%).
const ALONG_MEDIANS_LY: [f64; 3] = [0.535, 0.828, 0.353];

/// P10.T12, tubes over time: of each test stream's tube at the generator's 2,000 tracers, 10⁴
/// members spread along it, at t = ±1,000 yr, drifting on their own straight lines, lie within 4
/// widths plus 0.01 ly of the centre line, have moved along it about [`ALONG_MEDIANS_LY`] (against
/// the brainstorm's 0.3–0.8 ly, a finding, pinned as measured to 5%), and the curvature their
/// straight lines neglect, ½ |g| t² in the potential's pull, is under 10⁻⁴ ly.
#[test]
#[ignore = "slow: sprays the three test streams of 2,000 tracers and follows 10⁴ members of each"]
fn the_test_streams_members_keep_to_their_tubes_over_a_millennium() {
    let params = GalaxyParams::milky_way_like();
    let galaxy = Galaxy::from_params(SEED, params.clone()).expect("the Milky Way fixture builds");
    let tables = PotentialTables::full(&MassModel::new(&params));
    let force = SprayForce::new(&tables).expect("the full tables");
    let specs = [
        ("pal5", pal5_like(&tables)),
        ("gd1", gd1_like(&tables)),
        ("sagittarius", sagittarius_like(&tables)),
    ];
    let mean = galaxy.mean_system_mass(Population::Halo);
    let tubes = in_shares(specs.len(), SPRAY_THREADS, |k| {
        let tracers = Spray::new(SEED, &force, &tables, &specs[k].1, &SprayParams::default())
            .expect("a finite stripping time")
            .tracers();
        TubeTable::from_tracers(SEED, mean, &specs[k].1, &tracers).expect("enough tracers")
    });
    let seconds = SECONDS_PER_JULIAN_YEAR * 1_000.0;
    for (((name, spec), tube), pinned) in specs.iter().zip(&tubes).zip(ALONG_MEDIANS_LY) {
        let shares = StreamBandShares::of(&galaxy, spec);
        let members = spread_members(&galaxy, tube, &shares, 10_000);
        assert_eq!(members.len(), 10_000, "{name}");
        let (mut along, mut past_cut, mut bent) = (Vec::new(), 0.0_f64, 0.0_f64);
        for sign in [1.0, -1.0] {
            for (member, cell) in &members {
                let record = member.record();
                let [u, _, _] = member_coordinates(&galaxy, record.id(), *cell, tube.along_cells());
                let centre = tube.locate(u, 0.0, 0.0);
                let step = |a: f64, b: f64| centre.displacement_to(&tube.locate(u, a, b)).metres();
                let (normal, binormal) = (step(1.0, 0.0), step(0.0, 1.0));
                let tangent = tube.frame_at(u)[0];
                let drift = member
                    .velocity()
                    .metres_per_second()
                    .map(|v| sign * v * seconds);
                let offset = centre.displacement_to(record.epoch_position()).metres();
                let moved: [f64; 3] = core::array::from_fn(|i| offset[i] + drift[i]);
                for width in [normal, binormal] {
                    let w = norm(width);
                    let across = dot(moved, width).abs() / w;
                    past_cut = past_cut.max((across - 4.0 * w) / METRES_PER_LIGHT_YEAR);
                }
                along.push(dot(drift, tangent).abs() / METRES_PER_LIGHT_YEAR);
                let pull = norm(force.acceleration_m_s2(&about_centre(record.epoch_position())));
                bent = bent.max(0.5 * pull * seconds * seconds / METRES_PER_LIGHT_YEAR);
            }
        }
        let inside = along.iter().filter(|&&s| (0.3..=0.8).contains(&s)).count();
        let share_inside = f64::from(u32::try_from(inside).expect("small"))
            / f64::from(u32::try_from(along.len()).expect("small"));
        let [least, median, most] = spread(along);
        eprintln!(
            "{name}: {} members at ±1,000 yr; along {least:.3} / {median:.3} / {most:.3} ly \
             (least / median / most), {share_inside:.3} within 0.3–0.8 ly; past the cut by \
             {past_cut:.2e} ly at most; curvature {bent:.2e} ly at most",
            members.len()
        );
        assert!(past_cut <= 0.01, "{name}: {past_cut} ly past four widths");
        assert!(bent < 1e-4, "{name}: {bent} ly of curvature neglected");
        assert!(
            (median / pinned - 1.0).abs() < 0.05,
            "{name}: {median} ly against {pinned}"
        );
    }
}

/// One sweep galaxy's budgets, all M☉ but the shares.
#[derive(Debug, Clone, Copy)]
struct Budget {
    /// The share's globular term, inside [`INSIDE_LY`].
    globular_term: f64,
    /// The globular tubes' mass inside [`INSIDE_LY`] by their sprays' tracers.
    globular_sprayed: f64,
    /// The share's dwarf part, scaled as the debris scales it.
    dwarf_term: f64,
    /// The dwarfs' tubes' mass inside [`INSIDE_LY`] by their sprays, and their cores' whole.
    dwarf_sprayed: f64,
    /// The share applied.
    applied: f64,
    /// The globular tubes' share of the halo.
    globular_share: f64,
}

/// The globular tubes inside [`INSIDE_LY`] by their sprays against the share's term, galaxy by
/// galaxy over the sweep, as measured (2026-10-31, x86-64: 0.6775–1.2673, mean 0.9622), with a
/// little room.
const GLOBULAR_BAND: [f64; 2] = [0.65, 1.30];

/// The dwarfs inside [`INSIDE_LY`] by their sprays, their cores whole, against the share's dwarf
/// part, galaxy by galaxy over the sweep's galaxies with any, as measured (2026-10-31, x86-64:
/// 0.1811–1.6321, median 1.0000, 25 of 64 within 5%), with a little room: a finding, since the
/// share's period estimate puts a young tube's mass where its orbit spends its time, not where a
/// few wraps have put its tracers.
const DWARF_BAND: [f64; 2] = [0.15, 1.70];

/// Tracers an arm for a globular tube's share inside [`INSIDE_LY`] in the budget sweep: enough for
/// a share to a few per cent a tube, a per cent over a galaxy's few hundred, at a thirtieth of the
/// generator's cost; a dwarf's tube, whose few count one by one, takes the generator's.
const SWEEP_TRACERS_PER_ARM: u32 = 32;

/// Sweep galaxy `n`'s budgets (P10.T12, "Budgets").
fn budget_of(n: u64) -> Budget {
    let galaxy = sweep_galaxy(n);
    let tables = galaxy.potential();
    let mut debris = dwarfs(galaxy.seed(), galaxy.params(), tables);
    in_list_order(&mut debris);
    let figures = ShareFigures::of(
        galaxy.params(),
        tables,
        &debris,
        galaxy.rules().orphan_streams,
    );
    let [globular_term, dwarfs_cold, _] = figures.parts_solar_masses();
    let force = SprayForce::new(tables).expect("the full potential");
    let few = SprayParams::new(std::num::NonZeroU32::new(SWEEP_TRACERS_PER_ARM).expect("not zero"));
    let (mut globular_sprayed, mut dwarf_sprayed) = (0.0, 0.0);
    let drawn = Debris::generate(&galaxy);
    for spec in drawn.streams() {
        let dwarf = matches!(spec.origin(), StreamOrigin::Dwarf(_));
        let params = if dwarf { SprayParams::default() } else { few };
        let tracers = Spray::new(galaxy.seed(), &force, tables, spec, &params)
            .expect("a finite stripping time")
            .tracers();
        let inside = spec.mass().value() * weight_inside(&tracers);
        if dwarf {
            dwarf_sprayed += inside;
        } else {
            globular_sprayed += inside;
        }
    }
    dwarf_sprayed += drawn.cores().iter().map(|c| c.mass().value()).sum::<f64>();
    Budget {
        globular_term,
        globular_sprayed,
        dwarf_term: dwarfs_cold * figures.dwarf_scale(),
        dwarf_sprayed,
        applied: figures.applied(),
        globular_share: figures.globular_share(),
    }
}

/// P10.T12, the budgets, counted as the share counts them (plan 10's Risks, "Research-p10's rulings
/// for P10.T12") over the 64 sweep galaxies: (i) each galaxy's dwarfs, their tubes inside
/// 65,000 ly by their sprays and their cores whole, against the share's dwarf part, which the plan
/// holds to 5% and most galaxies miss; (ii) the
/// globular tubes inside 65,000 ly by their sprays within 5% of the share's globular term in the
/// mean (from generator version 26 within 6%: the mean is 0.949, a finding pinned as it stands),
/// and each galaxy within the band measured again here by the sprays, [`GLOBULAR_BAND`] (by
/// their orbits' periods it was 0.71–1.33); (iii) the share applied never below its globular term.
/// The dwarfs miss in most galaxies, a finding pinned as measured ([`DWARF_BAND`]).
#[test]
#[ignore = "slow: builds 64 galaxies' kinematic tables and sprays every stream of each"]
fn the_discrete_share_s_budgets_hold_over_the_sweep() {
    let budgets = in_shares(64, THREADS, |n| budget_of(u64::try_from(n).expect("small")));
    let mut against_term = Vec::new();
    let mut dwarf_ratios = Vec::new();
    for (n, b) in budgets.iter().enumerate() {
        let globular = b.globular_sprayed / b.globular_term;
        against_term.push(globular);
        let dwarf = (b.dwarf_term > 0.0).then(|| b.dwarf_sprayed / b.dwarf_term);
        if let Some(r) = dwarf {
            dwarf_ratios.push(r);
        }
        eprintln!(
            "sweep galaxy {n}: globular tubes {globular:.4} of the term; dwarfs {} of their \
             part ({:.4e} of {:.4e} M☉); applied {:.5}, globular share {:.5}",
            dwarf.map_or_else(|| "none".to_owned(), |r| format!("{r:.4}")),
            b.dwarf_sprayed,
            b.dwarf_term,
            b.applied,
            b.globular_share
        );
        assert!(b.applied >= b.globular_share, "sweep galaxy {n}: {b:?}");
    }
    assert!(!dwarf_ratios.is_empty(), "no sweep galaxy has dwarfs");
    let mean = against_term.iter().sum::<f64>()
        / f64::from(u32::try_from(against_term.len()).expect("64"));
    let [g_least, g_median, g_most] = spread(against_term);
    let [d_least, d_median, d_most] = spread(dwarf_ratios.clone());
    let dwarfs_within = dwarf_ratios
        .iter()
        .filter(|&&r| (r - 1.0).abs() <= 0.05)
        .count();
    eprintln!(
        "globular tubes against the term: {g_least:.4} / {g_median:.4} / {g_most:.4}, mean \
         {mean:.4}; dwarfs against their part: {d_least:.4} / {d_median:.4} / {d_most:.4}, {} of \
         {} within 5%",
        dwarfs_within,
        dwarf_ratios.len()
    );
    assert!(
        (mean - 1.0).abs() <= 0.06,
        "the globular tubes' mean {mean}"
    );
    assert!(
        GLOBULAR_BAND[0] <= g_least && g_most <= GLOBULAR_BAND[1],
        "the globular tubes {g_least}–{g_most} of the term"
    );
    assert!(
        DWARF_BAND[0] <= d_least && d_most <= DWARF_BAND[1] && (d_median - 1.0).abs() <= 0.05,
        "the dwarfs {d_least}–{d_most}, median {d_median}, of their part"
    );
}

/// A tube's figures as the brainstorm states them: its length, ly; its median rms width, ly, the
/// mean of the normal's and the binormal's; its median rms velocity dispersion along one axis,
/// km/s; and its wraps, its span of ψ over a turn.
fn figures_of(tube: &TubeTable) -> [f64; 4] {
    let knots: Vec<&TubeKnot> = tube.knots().collect();
    let widths = knots
        .iter()
        .map(|k| f64::midpoint(k.widths()[0].value(), k.widths()[1].value()))
        .collect();
    let sigmas = knots
        .iter()
        .map(|k| {
            let s = k.dispersions().map(crate::units::MetresPerSecond::value);
            (dot(s, s) / 3.0).sqrt() / 1e3
        })
        .collect();
    let psi = spread(knots.iter().map(|k| k.psi().value()).collect());
    [
        tube.length().value(),
        spread(widths)[1],
        spread(sigmas)[1],
        (psi[2] - psi[0]) / std::f64::consts::TAU,
    ]
}

/// The contrast of `tube`'s axis where it lies 15–18 kpc from the galactic centre, the median over
/// its segments there: its expected systems per cubic light-year on the axis against the
/// reference density, and against the smooth halo's there; `None` if it never lies there.
fn contrast_of(galaxy: &Galaxy, tube: &TubeTable) -> Option<[f64; 2]> {
    let mean = galaxy.mean_system_mass(Population::Halo).value();
    let knots: Vec<&TubeKnot> = tube.knots().collect();
    let (mut reference, mut halo) = (Vec::new(), Vec::new());
    for pair in knots.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let r = radius_ly(&about_centre(&from.centre()));
        let length = radius_ly(&from.centre().displacement_to(&to.centre()));
        let mass = to.cumulative_mass().value() - from.cumulative_mass().value();
        if !(CONTRAST_RANGE_LY[0]..CONTRAST_RANGE_LY[1]).contains(&r)
            || length <= 0.0
            || mass <= 0.0
        {
            continue;
        }
        let [wn, wb] =
            [0, 1].map(|i| f64::midpoint(from.widths()[i].value(), to.widths()[i].value()));
        // A bivariate normal's peak over the cross-section: the line density over 2π σ_n σ_b.
        let axis = mass / length / mean / (std::f64::consts::TAU * wn * wb);
        let [x, y, z] = from.centre().to_light_years_f64();
        let smooth = galaxy
            .fields()
            .population_density(Population::Halo, &PointLy::new(x, y, z));
        reference.push(axis / REFERENCE_DENSITY);
        halo.push(axis / smooth);
    }
    (!reference.is_empty()).then(|| [spread(reference)[1], spread(halo)[1]])
}

/// The drawn Milky Way's medians, as measured (2026-10-31, x86-64), each with its name: its globular
/// tubes' length, width and dispersion; its one dwarf tube's width, dispersion and wraps; and its
/// globular streams' axes at 15–18 kpc against the reference density and the smooth halo.
const DRAWN_MEDIANS: [(&str, f64); 8] = [
    ("the globular tubes' length, ly", 136_515.0),
    ("the globular tubes' width, ly", 229.0),
    ("the globular tubes' dispersion, km/s", 0.77),
    ("the dwarf tube's width, ly", 717.0),
    ("the dwarf tube's dispersion, km/s", 3.2),
    ("the dwarf tube's wraps", 1.88),
    ("the axis against the reference density", 1.88e-4),
    ("the axis against the smooth halo", 3.32),
];

/// Prints the drawn galaxy's figures, each column's least, median and most and how many lie in the
/// brainstorm's range, and returns what [`DRAWN_MEDIANS`] pins of the globular tubes' and the
/// contrast's: the length, width and dispersion medians, and the contrast's against the reference
/// density and the smooth halo.
fn report(
    globular: &[Vec<f64>; 4],
    dwarf: &[Vec<f64>; 4],
    to_reference: &[f64],
    to_halo: &[f64],
) -> [f64; 5] {
    let within = |values: &[f64], range: [f64; 2]| {
        let n = values
            .iter()
            .filter(|v| (range[0]..=range[1]).contains(v))
            .count();
        format!("{n} of {}", values.len())
    };
    let [length, width, sigma, wraps] = globular.clone().map(spread);
    eprintln!(
        "globular tubes ({}): length {:.0} / {:.0} / {:.0} ly ({} in 30,000–160,000); width {:.0} \
         / {:.0} / {:.0} ly ({} in 100–400); dispersion {:.2} / {:.2} / {:.2} km/s ({} in 0.5–3); \
         wraps {:.2} / {:.2} / {:.2}",
        globular[0].len(),
        length[0],
        length[1],
        length[2],
        within(&globular[0], [30_000.0, 160_000.0]),
        width[0],
        width[1],
        width[2],
        within(&globular[1], [100.0, 400.0]),
        sigma[0],
        sigma[1],
        sigma[2],
        within(&globular[2], [0.5, 3.0]),
        wraps[0],
        wraps[1],
        wraps[2],
    );
    if !dwarf[0].is_empty() {
        let [_, width, sigma, wraps] = dwarf.clone().map(spread);
        eprintln!(
            "dwarf tubes ({}): width {:.0} / {:.0} / {:.0} ly ({} in 1,000–6,000); dispersion \
             {:.1} / {:.1} / {:.1} km/s ({} in 10–25); wraps {:.2} / {:.2} / {:.2}",
            dwarf[0].len(),
            width[0],
            width[1],
            width[2],
            within(&dwarf[1], [1_000.0, 6_000.0]),
            sigma[0],
            sigma[1],
            sigma[2],
            within(&dwarf[2], [10.0, 25.0]),
            wraps[0],
            wraps[1],
            wraps[2],
        );
    }
    let [r_least, r_median, r_most] = spread(to_reference.to_vec());
    let [h_least, h_median, h_most] = spread(to_halo.to_vec());
    eprintln!(
        "contrast at 15–18 kpc ({} globular streams): the reference density's {r_least:.2e} / \
         {r_median:.2e} / {r_most:.2e}; the smooth halo's {h_least:.2} / {h_median:.2} / \
         {h_most:.2} times ({} in 7–15)",
        to_reference.len(),
        within(to_halo, [7.0, 15.0])
    );
    [length[1], width[1], sigma[1], r_median, h_median]
}

/// P10.T12, the figures and the contrast, over a drawn Milky Way's tubes at the generator's
/// tracers: the globular tubes against the brainstorm's 30,000–160,000 ly long, 100–400 ly wide
/// and 0.5–3 km/s; the dwarfs' against 1,000–6,000 ly wide, 10–25 km/s and several wraps; and a
/// globular stream's axis at 15–18 kpc against "about a thousandth" of the reference density and
/// "roughly 7–15 times" the smooth halo. Each figure is recorded and its median pinned to 5%
/// ([`DRAWN_MEDIANS`]). The dwarf tube and the contrast miss their ranges, and some two fifths of
/// the globular tubes each of theirs: findings, pinned as measured.
#[test]
#[ignore = "slow: builds a Milky Way galaxy's kinematic tables and every tube of its list"]
fn a_drawn_galaxy_s_tubes_and_contrast_keep_their_measured_figures() {
    let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())
        .expect("the fixture builds")
        .with_full_potential();
    let debris = Debris::generate(&galaxy);
    let streams: Vec<&StreamSpec> = debris.streams().iter().collect();
    let rows = in_shares(streams.len(), THREADS, |k| {
        let tube = TubeTable::build(&galaxy, streams[k]).expect("the full potential");
        (figures_of(&tube), contrast_of(&galaxy, &tube))
    });
    let mut globular: [Vec<f64>; 4] = Default::default();
    let mut dwarf: [Vec<f64>; 4] = Default::default();
    let (mut to_reference, mut to_halo) = (Vec::new(), Vec::new());
    for (spec, (figures, contrast)) in streams.iter().zip(&rows) {
        let into = if matches!(spec.origin(), StreamOrigin::Dwarf(_)) {
            &mut dwarf
        } else {
            if let Some([r, h]) = contrast {
                to_reference.push(*r);
                to_halo.push(*h);
            }
            &mut globular
        };
        for (column, value) in into.iter_mut().zip(figures) {
            column.push(*value);
        }
    }
    // Counted before any median is taken, so that an empty column fails here, by name.
    assert!(
        to_reference.len() > 10,
        "{} streams cross 15–18 kpc",
        to_reference.len()
    );
    assert!(!dwarf[0].is_empty(), "the drawn galaxy has no dwarf tube");
    assert!(globular.iter().flatten().all(|v| v.is_finite()));
    let [length, width, sigma, r_median, h_median] =
        report(&globular, &dwarf, &to_reference, &to_halo);
    let [dwarf_width, dwarf_sigma, dwarf_wraps] = [1, 2, 3].map(|c| spread(dwarf[c].clone())[1]);
    let medians = [
        length,
        width,
        sigma,
        dwarf_width,
        dwarf_sigma,
        dwarf_wraps,
        r_median,
        h_median,
    ];
    for (median, (name, pinned)) in medians.iter().zip(DRAWN_MEDIANS) {
        assert!(
            (median / pinned - 1.0).abs() < 0.05,
            "{name}: {median} against {pinned}"
        );
    }
}

/// What option A drops of one tube's members from version 26: the members it places, inside the
/// root cube, that lie outside its stream's shell, and its expected members placed, the mass of its
/// knot segments whose ends both lie in the cube over the halo's mean system mass (at least the
/// members found outside).
///
/// Only the along cells of a segment whose cut can reach past the shell are enumerated. Between two
/// knots the centre line is their chord, the widths at most the greater of theirs, and the frame's
/// normal and binormal unit vectors, so every point of the cut lies within `4 (w_n + w_b)` of the
/// chord (the triangle inequality): no nearer the galactic centre than the chord's nearest point
/// less that, and no farther than its farther end plus that. A segment whose bound stays inside
/// the shell holds no member outside it.
fn dropped_of(
    galaxy: &Galaxy,
    tube: &TubeTable,
    spec: &StreamSpec,
    shell: BoundingShell,
) -> (u64, f64) {
    let (inner, outer) = (shell.inner().value(), shell.outer().value());
    let knots: Vec<&TubeKnot> = tube.knots().collect();
    let total = tube.mass().value();
    let cells = tube.along_cells();
    let ly = |k: &TubeKnot| {
        about_centre(&k.centre())
            .metres()
            .map(|c| c / METRES_PER_LIGHT_YEAR)
    };
    let mut placed = 0.0;
    let mut along_cells = std::collections::BTreeSet::new();
    for pair in knots.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if from.centre().in_root_cube() && to.centre().in_root_cube() {
            placed += to.cumulative_mass().value() - from.cumulative_mass().value();
        }
        let widest = |i: usize| from.widths()[i].value().max(to.widths()[i].value());
        let reach = 4.0 * (widest(0) + widest(1));
        let (a, b) = (ly(from), ly(to));
        let chord: [f64; 3] = core::array::from_fn(|i| b[i] - a[i]);
        let along = dot(chord, chord);
        let t = if along > 0.0 {
            (-dot(a, chord) / along).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let nearest = norm(core::array::from_fn(|i| a[i] + t * chord[i]));
        let farthest = norm(a).max(norm(b));
        if shell.is_empty() || nearest - reach < inner || farthest + reach > outer {
            let [lo, hi] = [from, to].map(|k| k.cumulative_mass().value() / total);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a share of the mass in [0, 1] times at most 16,384 cells"
            )]
            let (first, last) = (
                (lo * f64::from(cells)).floor() as u32,
                ((hi * f64::from(cells)).ceil() as u32).min(cells),
            );
            along_cells.extend(first..last.max(first + 1).min(cells));
        }
    }
    let shares = StreamBandShares::of(galaxy, spec);
    let mut members = Vec::new();
    let mut outside = 0;
    for along in along_cells {
        let along = u16::try_from(along).expect("under 16,384");
        for a in 0..ACROSS_CELLS {
            for b in 0..ACROSS_CELLS {
                let cell = StreamMemberId::new(tube.stream().get(), Layer::A, along, [a, b], 0)
                    .expect("a tube's cell")
                    .into();
                for band in BANDS {
                    stream_cell_members(galaxy, tube, &shares, cell, band, &mut members);
                    outside += members
                        .iter()
                        .filter(|m| !shell.contains(&about_centre(m.record().epoch_position())))
                        .count();
                }
            }
        }
    }
    let outside = u64::try_from(outside).expect("small");
    let expected = (placed / galaxy.mean_system_mass(Population::Halo).value())
        .max(f64::from(u32::try_from(outside).expect("small")));
    (outside, expected)
}

/// The sweep galaxies the tripwire spends its full sprays on.
const TRIPWIRE_GALAXIES: u64 = 8;

/// The most of a galaxy's placed stream members option A may drop, and of one tube's: as measured
/// over [`TRIPWIRE_GALAXIES`] (2026-10-31, x86-64: 6.89 × 10⁻⁴, sweep galaxy 1, and 0.167, sweep
/// galaxy 1's stream 133; at generator version 25 5.33 × 10⁻⁴, sweep galaxy 5, and 0.186, the same
/// stream), with a little room. The tube's is a finding: a sixth of one stream's members inside the
/// cube lie past its shell.
const DROPPED_BOUNDS: [f64; 2] = [8e-4, 0.2];

/// The fewest members a tube must expect inside the cube for its own share to be bounded
/// ([`DROPPED_BOUNDS`]' second): below it one member is a whole share, so a smaller tube's count
/// is bounded instead ([`SMALL_TUBE_MOST_OUTSIDE`]). At generator version 25 sweep galaxy 1's
/// stream 97, an orphan with no knot in the cube, expects 1.000 there, and its one, of its tail,
/// lies 0.56 ly inside its shell's inner edge (lean, 2026-11-01, the galaxy's share still counting
/// it; research's review).
const TUBE_MEMBERS_JUDGED: f64 = 10.0;

/// The most members a tube that expects fewer than [`TUBE_MEMBERS_JUDGED`] inside the cube may
/// have past its shell: one, the most measured at generator version 25 over 1,224 such tubes of
/// the 3,592, the first of them stream 97 (research's review of round 7's lean, 2026-11-01).
const SMALL_TUBE_MOST_OUTSIDE: u64 = 1;

/// P10.T12, research-leans-10's tripwire on option A: over the first sweep galaxies, each tube at
/// the generator's tracers, the share of a galaxy's stream members that lie outside their shells,
/// which A drops from 26, and the most of one tube's, both bounded ([`DROPPED_BOUNDS`]; a tube
/// expecting few members by its count, [`SMALL_TUBE_MOST_OUTSIDE`]), so that A guards the tips'
/// remaining tail and does not hide a worse tube than it does now. It stands in for P10.T9's
/// containment over a sample of seeds, the shells' widening not raised (option C, rejected). Only
/// the along cells whose cut can reach past the shell are enumerated ([`dropped_of`]); it covers
/// the first 8 of the sweep's 64, since spraying every tube costs some 340 CPU-seconds a galaxy.
#[test]
#[ignore = "slow: builds eight galaxies' kinematic tables and lists, and every tube of each"]
fn option_a_drops_a_bounded_share_over_the_sweep() {
    let mut most_galaxy = 0.0_f64;
    let mut most_tube = (0.0_f64, 0, 0);
    let (mut small_tubes, mut tubes) = (0, 0);
    let mut most_small = (0_u64, 0, 0);
    for n in 0..TRIPWIRE_GALAXIES {
        let galaxy = sweep_galaxy(n);
        let list = GlobalList::build(&galaxy);
        let streams = list.debris().streams();
        let rows = in_shares(streams.len(), THREADS, |k| {
            let spec = &streams[k];
            let tube = TubeTable::build(&galaxy, spec).expect("the full potential");
            let shell = list.stream_entry(spec.number()).expect("listed").shell();
            dropped_of(&galaxy, &tube, spec, shell)
        });
        let (mut outside, mut expected) = (0, 0.0);
        for (k, &(o, e)) in rows.iter().enumerate() {
            outside += o;
            expected += e;
            let share = f64::from(u32::try_from(o).expect("small")) / e;
            tubes += 1;
            if e < TUBE_MEMBERS_JUDGED {
                small_tubes += 1;
                if o > most_small.0 {
                    most_small = (o, n, k);
                }
            } else if share > most_tube.0 {
                most_tube = (share, n, k);
            }
        }
        let share = f64::from(u32::try_from(outside).expect("small")) / expected;
        most_galaxy = most_galaxy.max(share);
        eprintln!(
            "sweep galaxy {n}: {outside} of some {expected:.3e} stream members outside their \
             shells, {share:.2e}"
        );
    }
    eprintln!(
        "the most a galaxy drops {most_galaxy:.2e}; the most a tube drops {:.2e} (sweep galaxy {}, \
         stream {})",
        most_tube.0, most_tube.1, most_tube.2
    );
    eprintln!(
        "{small_tubes} of {tubes} tubes expect under {TUBE_MEMBERS_JUDGED} members; the most of \
         theirs outside {} (sweep galaxy {}, stream {})",
        most_small.0, most_small.1, most_small.2
    );
    assert!(
        most_galaxy <= DROPPED_BOUNDS[0] && most_tube.0 <= DROPPED_BOUNDS[1],
        "a galaxy drops {most_galaxy}, a tube {most_tube:?}"
    );
    assert!(
        most_small.0 <= SMALL_TUBE_MOST_OUTSIDE,
        "a tube expecting under {TUBE_MEMBERS_JUDGED} members drops {most_small:?}"
    );
}
