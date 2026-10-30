//! The global list's members: a stream's band shares (P10.T8.a).

use std::sync::OnceLock;

use super::*;
use crate::galaxy::Population;
use crate::galaxy::global_list::testing::{gd1_like, pal5_like, sagittarius_like};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::id::StreamMemberId;
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
