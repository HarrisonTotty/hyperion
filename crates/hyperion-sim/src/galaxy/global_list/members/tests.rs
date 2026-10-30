//! The global list's members: a stream's band shares (P10.T8.a) and its members in its tube's
//! cells (P10.T8.b).

use std::sync::OnceLock;

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{
    ALPHA, assert_p_value, assert_poisson_count, chi_square_gof, ks_one_sample,
};

use super::*;
use crate::galaxy::Population;
use crate::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::id::{Layer, StreamMemberId};
use crate::rng::Seed;
use crate::stellar::ObjectKind;
use crate::time::UniverseTime;

fn milky_way() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(0x0a10_8a00), GalaxyParams::milky_way_like()).unwrap()
    })
}

/// The three test streams' specifications and band shares, by name.
fn test_streams() -> [(&'static str, StreamSpec, StreamBandShares); 3] {
    let galaxy = milky_way();
    let tables = galaxy.potential();
    [
        ("pal5", pal5_like(tables)),
        ("gd1", gd1_like(tables)),
        ("sagittarius", sagittarius_like(tables)),
    ]
    .map(|(name, spec)| {
        let shares = StreamBandShares::of(galaxy, &spec);
        (name, spec, shares)
    })
}

/// The ID of member `index` of `band` in a cell of stream `number`.
fn member(number: u16, band: MassBand, index: u16) -> SystemId {
    SystemId::from(StreamMemberId::new(number, band.layer(), 100, [31, 32], index).unwrap())
}

/// The canonical evolved population's share in `band` at `turn_off` (M☉) on `f`: its living
/// stars below the turn-off and its white dwarfs' progenitors to 8 M☉, by the mass function's
/// own integral.
fn canonical_share(f: &dyn MassFunction, band: MassBand, turn_off: f64) -> f64 {
    let in_band = |lo: f64, hi: f64| {
        let (a, b) = band_edges(band);
        let (lo, hi) = (lo.max(a), hi.min(b));
        if hi > lo { f.integral(lo, hi) } else { 0.0 }
    };
    let all = f.integral(0.08, turn_off) + f.integral(turn_off, WHITE_DWARF_PROGENITOR_MAX);
    (in_band(0.08, turn_off) + in_band(turn_off, WHITE_DWARF_PROGENITOR_MAX)) / all
}

/// P10.T8.a: every stream's shares sum to 1; a living globular's stream is bottom-heavy, band A
/// above the canonical evolved population's share; an orphan's, which kept nothing, is that
/// population; a globular's holds white dwarfs; and no stream holds band E but a young dwarf's.
#[test]
fn the_shares_sum_to_one_and_a_living_globular_s_stream_is_bottom_heavy() {
    let galaxy = milky_way();
    let f = galaxy.mass_function();
    for (name, spec, shares) in test_streams() {
        let total: f64 = MassBand::ALL.iter().map(|&b| shares.share(b)).sum();
        assert!((total - 1.0).abs() < 1e-12, "{name}: {total}");
        for band in MassBand::ALL {
            assert!(shares.living(band) >= 0.0 && shares.white_dwarfs(band) >= 0.0);
        }
        let mean = shares.mean_member_mass().value();
        assert!((0.08..1.0).contains(&mean), "{name}: {mean}");
        eprintln!(
            "{name}: shares {:?}, white dwarfs {:.3}, mean mass {mean:.3} M☉",
            MassBand::ALL.map(|b| (shares.share(b) * 1e3).round() / 1e3),
            MassBand::ALL
                .iter()
                .map(|&b| shares.white_dwarfs(b))
                .sum::<f64>()
        );
        let Some(stars) = spec.stars() else { continue };
        let composition = Composition::from_fe_h(stars.fe_h(), HeliumExcess::ZERO);
        let turn_off = turn_off_mass(stars.age(), &composition).value();
        let canonical = canonical_share(f, MassBand::A, turn_off);
        let a = shares.share(MassBand::A);
        if stars.kept() > 0.0 {
            assert!(a > canonical, "{name}: band A {a} against {canonical}");
        } else {
            assert!(
                (a / canonical - 1.0).abs() < 1e-6,
                "{name}: {a} {canonical}"
            );
        }
        let white_dwarfs: f64 = MassBand::ALL.iter().map(|&b| shares.white_dwarfs(b)).sum();
        assert!(white_dwarfs > 0.05, "{name}: {white_dwarfs}");
        assert!(shares.share(MassBand::E) < 1e-15, "{name}");
    }
}

/// P10.T8.a: no member of a globular's stream is a neutron star or a black hole, in any band; a
/// living member is no remnant and a white dwarf's has left the main sequence at the epoch; the
/// living stars' and white dwarfs' split in band C is the shares'.
#[test]
fn no_member_of_a_globular_stream_is_a_neutron_star_or_a_black_hole() {
    let galaxy = milky_way();
    for (name, spec, shares) in test_streams().into_iter().take(2) {
        let number = spec.number().get();
        for band in MassBand::ALL {
            if shares.share(band) <= 0.0 {
                let system = member(number, band, 0);
                let composition = shares.composition(galaxy, system);
                assert_eq!(shares.draw_star(galaxy, band, system, &composition), None);
                continue;
            }
            let (mut living, mut drawn) = (0_u32, 0_u32);
            for index in 0..400 {
                let system = member(number, band, index);
                let composition = shares.composition(galaxy, system);
                let (class, mass, attempt, age) = shares
                    .draw_star(galaxy, band, system, &composition)
                    .expect("a band with members draws a star");
                assert!(
                    matches!(class.kind, ClassKind::Living | ClassKind::WhiteDwarf),
                    "{name} {band:?}: {class:?}"
                );
                let (lo, hi) = band_edges(band);
                assert!((lo..=hi).contains(&mass.value()));
                living += u32::from(class.kind == ClassKind::Living);
                drawn += 1;
                if index < 6 {
                    let record = SystemRecord::from_parts(
                        system,
                        crate::coords::GalacticPosition::ORIGIN,
                        SystemOrigin::GlobalListMember {
                            attempt: u16::try_from(attempt).unwrap(),
                        },
                        Population::Halo,
                        mass,
                        age,
                    );
                    let member = GlobalMemberRecord::new(
                        record,
                        class,
                        composition,
                        GalacticVelocity::new([0.0; 3]),
                    );
                    let kind = member
                        .stars(galaxy)
                        .brief_at(UniverseTime::EPOCH)
                        .expect("a member exists")
                        .kind();
                    assert!(
                        !matches!(kind, ObjectKind::NeutronStar | ObjectKind::BlackHole),
                        "{name} {band:?}: {kind:?}"
                    );
                    // Plan 09's split is at the main sequence's turn-off, so a star past it that
                    // is not yet a remnant counts with the white dwarfs, as in its clusters.
                    if class.kind == ClassKind::Living {
                        assert!(
                            !matches!(kind, ObjectKind::WhiteDwarf),
                            "{name} {band:?}: {kind:?} of {class:?}"
                        );
                    } else {
                        assert_ne!(kind, ObjectKind::Dwarf, "{name} {band:?}: {class:?}");
                    }
                }
            }
            if band == MassBand::C {
                let want = shares.living(band) / shares.share(band);
                let got = f64::from(living) / f64::from(drawn);
                let sigma = (want * (1.0 - want) / f64::from(drawn)).sqrt();
                assert!(
                    (got - want).abs() < 4.0 * sigma,
                    "{name}: {got} against {want}"
                );
            }
        }
    }
}

/// P10.T8.a: a dwarf's tube holds no star younger than its last stripping, and its members spread
/// in [Fe/H] about its core's mean by the core's spread.
#[test]
fn a_dwarf_s_tube_holds_no_star_younger_than_its_last_stripping() {
    let galaxy = milky_way();
    let [_, _, (_, spec, shares)] = test_streams();
    let MassLoss::Pulsed {
        last_pericentre, ..
    } = spec.mass_loss()
    else {
        panic!("a dwarf's tube is pulsed")
    };
    let mut fe_h = Vec::new();
    for index in 0..2_000 {
        let system = member(spec.number().get(), MassBand::A, index);
        let composition = shares.composition(galaxy, system);
        let (_, _, _, age) = shares
            .draw_star(galaxy, MassBand::A, system, &composition)
            .expect("band A holds members");
        assert!(
            age.value() >= last_pericentre.value() - 1.0,
            "{}",
            age.value()
        );
        fe_h.push(composition.fe_h().value());
    }
    let n = f64::from(u32::try_from(fe_h.len()).unwrap());
    let mean = fe_h.iter().sum::<f64>() / n;
    let spread = (fe_h.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n).sqrt();
    let StreamStars::Dwarf {
        fe_h: want,
        spread: want_spread,
        ..
    } = shares.stars
    else {
        panic!("a dwarf's stream's stars")
    };
    assert!((mean - want.value()).abs() < 4.0 * want_spread.value() / n.sqrt());
    assert!(
        (spread / want_spread.value() - 1.0).abs() < 0.05,
        "{spread}"
    );
}

/// The synthetic tube's radius, ly, and the tube's mass, M☉: a stream on a circle in the plane
/// with 2,000 M☉ over 64 along cells.
const RADIUS: f64 = 50_000.0;
const MASS: f64 = 2_000.0;

/// `per_arm` tracers from each Lagrange point of a stream on a circle of `radius` ly in the
/// galactic plane, one radian an arm, 100 ly wide, at 220 km/s with dispersions of 3, 1 and 0.5
/// km/s along, outward and up, weighing [`MASS`] in all (as the tube's own tests' stream).
fn circle(per_arm: u32, radius: f64) -> Vec<crate::galaxy::global_list::spray::Tracer> {
    use crate::coords::GalacticDisplacement;
    use crate::galaxy::global_list::spray::{Arm, Tracer};
    use crate::math;
    use crate::units::Radians;
    use crate::units::consts::METRES_PER_LIGHT_YEAR;
    let mut draws = Stream::open(
        Seed::new(0x0a10_8b00),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy(),
    );
    let weight = SolarMasses::new(MASS / f64::from(2 * per_arm));
    (0..2 * per_arm)
        .map(|i| {
            let k = i / 2;
            let arm = if i.is_multiple_of(2) {
                Arm::Leading
            } else {
                Arm::Trailing
            };
            let gained = (f64::from(k) + draws.uniform()) / f64::from(per_arm);
            let psi = if arm == Arm::Trailing {
                -gained
            } else {
                gained
            };
            let (sin, cos) = math::sin_cos(psi);
            let out = radius + 100.0 * draws.standard_normal();
            let up = 100.0 * draws.standard_normal();
            let position = [out * cos, out * sin, up].map(|c| c * METRES_PER_LIGHT_YEAR);
            let along = 2.2e5 + 3_000.0 * draws.standard_normal();
            let outward = 1_000.0 * draws.standard_normal();
            let vertical = 500.0 * draws.standard_normal();
            let velocity = [
                -along * sin + outward * cos,
                along * cos + outward * sin,
                vertical,
            ];
            Tracer::from_parts(
                arm,
                0,
                Years::new(1e9),
                weight,
                GalacticDisplacement::new(position),
                GalacticVelocity::new(velocity),
                Radians::new(psi),
            )
        })
        .collect()
}

/// The synthetic tube of `gd1_like`'s stream (number 1) on a circle of `radius` ly, and its band
/// shares, an orphan's.
fn synthetic(radius: f64) -> (TubeTable, StreamBandShares) {
    let galaxy = milky_way();
    let spec = gd1_like(galaxy.potential());
    let tube = TubeTable::from_tracers(
        galaxy.seed(),
        galaxy.mean_system_mass(Population::Halo),
        &spec,
        &circle(300, radius),
    )
    .expect("enough tracers an arm");
    (tube, StreamBandShares::of(galaxy, &spec))
}

/// Every cell of a tube with its along and across indices, in [`TubeCell`]'s order.
fn every_cell(tube: &TubeTable) -> impl Iterator<Item = TubeCell> + '_ {
    (0..tube.along_cells()).flat_map(|along| {
        (0..64_u8).flat_map(move |a| {
            (0..64_u8).map(move |b| {
                TubeCell::from(
                    StreamMemberId::new(1, Layer::A, u16::try_from(along).unwrap(), [a, b], 0)
                        .unwrap(),
                )
            })
        })
    })
}

/// A placed member with its band, cell and tube coordinates.
type Placed = (MassBand, TubeCell, GlobalMemberRecord, [f64; 3]);

/// The synthetic tube, its shares, its members, its counts summed by along cell, and its count.
type Synthetic = (TubeTable, StreamBandShares, Vec<Placed>, Vec<u64>, u64);

/// Every member of the synthetic tube on a circle of [`RADIUS`], with its band, cell and tube
/// coordinates, and every cell's count summed by along cell.
fn synthetic_members() -> &'static Synthetic {
    static MEMBERS: OnceLock<Synthetic> = OnceLock::new();
    MEMBERS.get_or_init(|| {
        let galaxy = milky_way();
        let (tube, shares) = synthetic(RADIUS);
        let mut members = Vec::new();
        let mut by_along = vec![0_u64; usize::try_from(tube.along_cells()).unwrap()];
        let mut counted = 0;
        let mut cell_members = Vec::new();
        for cell in every_cell(&tube) {
            for band in MassBand::ALL {
                let count = stream_member_count(galaxy, &tube, &shares, cell, band);
                by_along[usize::try_from(cell.along()).unwrap()] += u64::from(count);
                counted += u64::from(count);
                if count == 0 {
                    continue;
                }
                stream_cell_members(galaxy, &tube, &shares, cell, band, &mut cell_members);
                members.extend(cell_members.iter().map(|m| {
                    let at = member_coordinates(galaxy, m.record().id(), cell, tube.along_cells());
                    (band, cell, *m, at)
                }));
            }
        }
        (tube, shares, members, by_along, counted)
    })
}

/// P10.T8.b: the cells' means sum to the tube's expected members, and the counts along the tube
/// follow its line density times its gap noise, by a chi-square test over its along cells.
#[test]
fn the_counts_along_follow_the_line_density_and_the_gap_noise() {
    let (tube, shares, _, by_along, counted) = synthetic_members();
    let cells = tube.along_cells();
    let per_cell = tube.mass().value() / (shares.mean_member_mass().value() * f64::from(cells));
    let expected: Vec<f64> = (0..cells).map(|i| per_cell * tube.noise(i)).collect();
    let total: f64 = expected.iter().sum();
    let want = tube.mass().value() / shares.mean_member_mass().value();
    assert!((total / want - 1.0).abs() < 1e-9, "{total} {want}");
    // The across factors of one along cell and band sum to 1.
    let one: f64 = every_cell(tube)
        .take(64 * 64)
        .map(|c| expected_members(tube, shares, c, MassBand::A))
        .sum();
    assert!((one / (per_cell * tube.noise(0) * shares.share(MassBand::A)) - 1.0).abs() < 1e-9);
    // The total by its Poisson law, the distribution along by the chi-square test of the counts
    // against the means scaled to it.
    assert_poisson_count("the tube's members", *counted, total, ALPHA);
    let n = f64::from(u32::try_from(*counted).unwrap());
    let scaled: Vec<f64> = expected.iter().map(|e| e * n / total).collect();
    let chi = chi_square_gof(by_along, &scaled);
    assert_p_value("the counts along", chi.p_value, ALPHA);
}

/// P10.T8.b: the members' across coordinates are normal, truncated to the cut at ±4 widths, by
/// a one-sample Kolmogorov–Smirnov test of each, and every member is the point `locate` gives
/// for its coordinates, in a cell `cells_touching` finds about it.
#[test]
fn members_are_normal_across_the_tube_and_lie_where_their_coordinates_say() {
    let (tube, _, members, _, _) = synthetic_members();
    assert!(members.len() > 3_000, "{}", members.len());
    let cut = cut_mass();
    let truncated = |x: f64| (normal_cdf(x) - normal_cdf(-CUT_WIDTHS)) / cut;
    for axis in 0..2 {
        let mut across: Vec<f64> = members.iter().map(|(_, _, _, at)| at[1 + axis]).collect();
        let ks = ks_one_sample(&mut across, truncated);
        assert_p_value(&format!("across {axis}"), ks.p_value, ALPHA);
    }
    for (band, cell, member, [u, a, b]) in members.iter().step_by(97) {
        let at = tube.locate(*u, *a, *b);
        assert!(at.is_same_point(member.record().epoch_position()));
        assert!(*u * f64::from(tube.along_cells()) >= f64::from(cell.along()));
        let [[a0, a1], [b0, b1]] = cell.across_widths();
        assert!((a0..=a1).contains(a) && (b0..=b1).contains(b), "{band:?}");
        let touched: Vec<TubeCell> = tube
            .cells_touching(&at, crate::units::LightYears::new(1.0))
            .collect();
        assert!(touched.binary_search(cell).is_ok(), "{cell:?}");
    }
}

/// P10.T8.b: every member's ID round-trips and resolves to the same member; an index past its
/// cell's count, another stream's ID and an along cell past the tube's resolve to nothing.
#[test]
fn every_stream_member_s_id_round_trips_and_resolves_to_it() {
    let galaxy = milky_way();
    let (tube, shares, members, _, _) = synthetic_members();
    for (_, _, member, _) in members.iter().step_by(53) {
        let id = member.record().id();
        assert_eq!(SystemId::from_raw(id.raw()).unwrap(), id);
        let crate::id::SystemIdKind::Stream(stream_id) = id.kind() else {
            panic!("a stream member's ID decodes as one: {id:?}")
        };
        assert_eq!(
            resolve_stream_member(galaxy, tube, shares, stream_id),
            Ok(*member)
        );
    }
    let (band, cell, ..) = members[0];
    let count = stream_member_count(galaxy, tube, shares, cell, band);
    let past = stream_member_id(tube, cell, band, count);
    let other = StreamMemberId::new(2, band.layer(), 0, [32, 32], 0).unwrap();
    let beyond = StreamMemberId::new(
        1,
        band.layer(),
        u16::try_from(tube.along_cells()).unwrap(),
        [32, 32],
        0,
    )
    .unwrap();
    for id in [past, other, beyond] {
        assert_eq!(
            resolve_stream_member(galaxy, tube, shares, id),
            Err(ResolveSystemError::NoSuchSystem),
            "{id:?}"
        );
    }
}

/// P10.T8.b: a stream's member is a pure function of its ID and its tube, whatever was placed
/// before it.
#[test]
fn stream_members_are_order_independent() {
    let galaxy = milky_way();
    let (tube, shares, members, _, _) = synthetic_members();
    let keys: Vec<(MassBand, TubeCell, u16)> = members
        .iter()
        .step_by(173)
        .map(|(band, cell, m, _)| {
            let crate::id::SystemIdKind::Stream(id) = m.record().id().kind() else {
                unreachable!("a stream member's ID")
            };
            (*band, *cell, id.index())
        })
        .collect();
    assert!(keys.len() > 20);
    assert_order_independent(&keys, |&(band, cell, index)| {
        stream_member(galaxy, tube, shares, cell, band, index)
    });
}

/// P10.T8.b: a member outside the root cube is dropped and its index kept: on a circle of
/// 66,000 ly, whose middle lies past the cube's 65,536 ly and whose tips inside it, cells count
/// members that are not placed, every member placed is inside, and the indices of those placed
/// keep their order.
#[test]
fn members_outside_the_root_cube_are_dropped_and_their_indices_kept() {
    let galaxy = milky_way();
    let (tube, shares) = synthetic(66_000.0);
    let (mut counted, mut placed) = (0_u32, 0_u32);
    let mut out = Vec::new();
    for cell in every_cell(&tube).filter(|c| c.across() == [32, 32] || c.across() == [31, 33]) {
        let count = stream_member_count(galaxy, &tube, &shares, cell, MassBand::A);
        stream_cell_members(galaxy, &tube, &shares, cell, MassBand::A, &mut out);
        assert!(
            out.iter()
                .all(|m| m.record().epoch_position().in_root_cube())
        );
        counted += u32::from(count);
        placed += u32::try_from(out.len()).unwrap();
        for (k, m) in out.iter().enumerate().skip(1) {
            let crate::id::SystemIdKind::Stream(id) = m.record().id().kind() else {
                unreachable!("a stream member's ID")
            };
            let crate::id::SystemIdKind::Stream(before) = out[k - 1].record().id().kind() else {
                unreachable!("a stream member's ID")
            };
            assert!(id.index() > before.index());
        }
    }
    assert!(placed < counted && placed > 0, "{placed} of {counted}");
}

/// P10.T8.b's golden: two test streams' tubes on the Milky Way fixture at 64 tracers an arm, a
/// few cells' expected members and counts by band, and the first members of the fullest cells in
/// full. They move with the spray, the tube, the band shares and the members' draws.
#[test]
fn stream_members_golden() {
    use std::num::NonZeroU32;

    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    use crate::galaxy::global_list::spray::{Spray, SprayForce, SprayParams};
    use crate::galaxy::potential::{MassModel, PotentialTables};

    let galaxy = milky_way();
    let params = GalaxyParams::milky_way_like();
    let tables = PotentialTables::full(&MassModel::new(&params));
    let force = SprayForce::new(&tables).expect("the full tables");
    let spray_params = SprayParams::new(NonZeroU32::new(64).expect("not zero"));
    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    for (name, spec) in [
        ("gd1", gd1_like(&tables)),
        ("sagittarius", sagittarius_like(&tables)),
    ] {
        let tracers = Spray::new(galaxy.seed(), &force, &tables, &spec, &spray_params)
            .expect("a finite stripping time")
            .tracers();
        let tube = TubeTable::from_tracers(
            galaxy.seed(),
            galaxy.mean_system_mass(Population::Halo),
            &spec,
            &tracers,
        )
        .expect("enough tracers an arm");
        let shares = StreamBandShares::of(galaxy, &spec);
        for band in MassBand::ALL {
            w.f64(&format!("{name}.share.{band:?}"), shares.share(band));
        }
        w.f64(
            &format!("{name}.mean_member_mass"),
            shares.mean_member_mass().value(),
        );
        let along = tube.along_cells() / 2;
        let mut out = Vec::new();
        for (a, b) in [(31, 32), (32, 32), (36, 28)] {
            let cell = TubeCell::from(
                StreamMemberId::new(1, Layer::A, u16::try_from(along).unwrap(), [a, b], 0).unwrap(),
            );
            for band in [MassBand::A, MassBand::C] {
                let label = format!("{name}.{along}.{a}.{b}.{band:?}");
                w.f64(
                    &format!("{label}.expected"),
                    expected_members(&tube, &shares, cell, band),
                );
                stream_cell_members(galaxy, &tube, &shares, cell, band, &mut out);
                w.line(&format!(
                    "{label}.count {} members {}",
                    stream_member_count(galaxy, &tube, &shares, cell, band),
                    out.len()
                ));
                for (k, m) in out.iter().take(2).enumerate() {
                    let record = m.record();
                    let at = format!("{label}.{k}");
                    w.u64_hex(&format!("{at}.id"), record.id().raw());
                    w.line(&format!(
                        "{at}.class {:?} attempt {}",
                        m.class().kind,
                        m.attempt()
                    ));
                    for (axis, c) in ["x", "y", "z"]
                        .iter()
                        .zip(record.epoch_position().to_light_years_f64())
                    {
                        w.f64(&format!("{at}.position.{axis}"), c);
                    }
                    w.f64(&format!("{at}.mass"), record.primary_initial_mass().value());
                    w.f64(&format!("{at}.age"), record.age_at_epoch().value());
                    w.f64(&format!("{at}.fe_h"), m.composition().fe_h().value());
                    for (axis, v) in ["x", "y", "z"].iter().zip(m.velocity().metres_per_second()) {
                        w.f64(&format!("{at}.velocity.{axis}"), v);
                    }
                }
            }
        }
    }
    golden!("galaxy/global_list/stream_members", w.as_str());
}

/// P10.T8.c: a stream member's velocity is its tube's mean plus the tube's three dispersions
/// along its local frame, the same for every band: about the mean, each band's offsets along the
/// tangent, the normal and the binormal, in units of the dispersions there, have a mean of 0 and
/// an rms of 1.
#[test]
fn stream_members_move_with_their_tube_s_dispersions_in_every_band() {
    let (tube, _, members, _, _) = synthetic_members();
    let mut tested = 0;
    for band in MassBand::ALL {
        let of_band: Vec<&(MassBand, TubeCell, GlobalMemberRecord, [f64; 3])> =
            members.iter().filter(|(b, ..)| *b == band).collect();
        if of_band.len() < 300 {
            continue;
        }
        let n = f64::from(u32::try_from(of_band.len()).unwrap());
        for axis in 0..3 {
            let z: Vec<f64> = of_band
                .iter()
                .map(|(_, _, m, [u, _, _])| {
                    let mean = tube.mean_velocity(*u).metres_per_second();
                    let v = m.velocity().metres_per_second();
                    let e = tube.frame_at(*u)[axis];
                    let offset: f64 = (0..3).map(|i| (v[i] - mean[i]) * e[i]).sum();
                    offset / tube.dispersions_at(*u)[axis].value()
                })
                .collect();
            let mean = z.iter().sum::<f64>() / n;
            let rms = (z.iter().map(|x| x * x).sum::<f64>() / n).sqrt();
            assert!(mean.abs() < 4.0 / n.sqrt(), "{band:?} axis {axis}: {mean}");
            assert!(
                (rms - 1.0).abs() < 5.0 / (2.0 * n).sqrt(),
                "{band:?} axis {axis}: {rms}"
            );
        }
        tested += 1;
    }
    assert!(tested >= 3, "{tested} bands tested");
}
