//! Tests of the track integrator (plan 06, P06.T10.c–e).

use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

use super::super::continuity::find_jump;
use super::*;
use crate::math;
use crate::stellar::draws::{StandardNormal, StarDrawsParts};
use crate::stellar::remnant::{DeathKind, RemnantKind};
use crate::units::{Dex, HeliumExcess};

/// The initial masses of the SSE comparison (P06.T12).
const GRID_MASSES: [f64; 16] = [
    0.1, 0.3, 0.5, 0.8, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 10.0, 15.0, 20.0, 40.0, 60.0, 100.0,
];

fn composition(z: f64) -> Composition {
    Composition::from_fe_h(Dex::new(math::log10(z / 0.02)), HeliumExcess::ZERO)
}

/// The draws of a star whose Reimers η has the standard normal `z` (design note 7), every other
/// draw at its median.
fn draws(z: f64) -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        eta: StandardNormal::new(z).expect("a finite draw"),
        ..StarDrawsParts::MEDIAN
    })
}

/// A random star: initial mass log-uniform in 0.1–100 M☉, Z log-uniform in 10⁻⁴–0.03, and η from
/// a standard normal, as design note 7 draws it.
fn random_star(rng: &mut Lcg) -> (SolarMasses, Composition, StarDraws) {
    let log_m = -1.0 + 3.0 * rng.next_f64();
    let log_z = -4.0 + (math::log10(0.03) + 4.0) * rng.next_f64();
    // Box–Muller from two uniforms of (0, 1].
    let (u1, u2) = (1.0 - rng.next_f64(), rng.next_f64());
    let z = (-2.0 * math::ln(u1)).sqrt() * math::cos(core::f64::consts::TAU * u2);
    let m = math::exp10(log_m).clamp(0.1, 100.0);
    (
        SolarMasses::new(m),
        composition(math::exp10(log_z).clamp(1e-4, 0.03)),
        draws(z),
    )
}

/// Ages spread over a track's whole life: evenly in log age over its living span, and through its
/// first gigayear as a remnant.
fn ages_of(track: &Track, n: u32) -> Vec<f64> {
    let life = track.lifetime().map_or(1e10, Years::value);
    let end = life + 1e9;
    (0..n)
        .map(|i| {
            let x = f64::from(i) / f64::from(n - 1);
            // log-spaced from 10⁻⁶ of the life to the end, with age zero first.
            if i == 0 {
                0.0
            } else {
                math::exp10(math::log10(end) - 6.0 * (1.0 - x))
            }
        })
        .collect()
}

fn state_bits(s: &StarState) -> [u64; 9] {
    [
        bits(s.age().value()),
        bits(s.mass().value()),
        bits(s.core_mass().value()),
        bits(s.luminosity().value()),
        bits(s.radius().value()),
        bits(s.effective_temperature().value()),
        bits(s.mass_loss_rate().value()),
        bits(s.phase_fraction()),
        u64::from(s.phase() as u8),
    ]
}

/// Design note 2: the segments of `to_age(a)` are the first segments of `full`, bit for bit, and
/// answer the same at every age they cover.
#[test]
fn a_track_built_to_an_age_is_the_prefix_of_the_full_track() {
    for (m, z) in [
        (1.0, 0.02),
        (5.0, 0.004),
        (0.3, 0.0001),
        (40.0, 0.03),
        (2.0, 0.001),
    ] {
        let (comp, d) = (composition(z), draws(0.3));
        let full = Track::full(SolarMasses::new(m), &comp, &d);
        let life = full.lifetime().expect("a full track reaches death").value();
        for k in 0..10 {
            let age = life * f64::from(k) / 8.0;
            let part = Track::to_age(SolarMasses::new(m), &comp, &d, Years::new(age));
            let n = part.segments().len();
            assert!(n <= full.segments().len(), "M = {m}, Z = {z}, age {age}");
            assert_eq!(
                &full.segments()[..n],
                part.segments(),
                "M = {m}, Z = {z}, {age}"
            );
            let covered = part.built_until().value().min(age);
            for j in 0..=20 {
                let a = covered * f64::from(j) / 20.0;
                assert_eq!(
                    state_bits(&part.state_at(Years::new(a))),
                    state_bits(&full.state_at(Years::new(a))),
                    "M = {m}, Z = {z}, built to {age}, asked {a}"
                );
            }
        }
    }
}

/// The grid does not depend on the query: one track asked at 1,000 ages in two orders, and a
/// fresh track for each age, give the same bits.
#[test]
fn a_tracks_answers_do_not_depend_on_what_was_asked_before() {
    let (m, comp, d) = (SolarMasses::new(3.0), composition(0.008), draws(-0.7));
    let track = Track::full(m, &comp, &d);
    let ages: Vec<f64> = ages_of(&track, 1_000);
    let forward: Vec<[u64; 9]> = ages
        .iter()
        .map(|&a| state_bits(&track.state_at(Years::new(a))))
        .collect();
    let backward: Vec<[u64; 9]> = ages
        .iter()
        .rev()
        .map(|&a| state_bits(&track.state_at(Years::new(a))))
        .collect();
    for (i, f) in forward.iter().enumerate() {
        assert_eq!(*f, backward[ages.len() - 1 - i], "age {}", ages[i]);
    }
    for (i, &a) in ages.iter().enumerate().step_by(7) {
        let fresh = Track::to_age(m, &comp, &d, Years::new(a));
        assert_eq!(
            state_bits(&fresh.state_at(Years::new(a))),
            forward[i],
            "{a}"
        );
    }
}

/// The same star built twice is the same track.
#[test]
fn the_same_star_gives_the_same_track() {
    let (m, comp, d) = (SolarMasses::new(12.0), composition(0.002), draws(1.1));
    assert_eq!(Track::full(m, &comp, &d), Track::full(m, &comp, &d));
}

/// Mass never increases (there is no protostar until P06.T15), over random tracks under both
/// recipes.
#[test]
fn mass_never_increases() {
    let mut rng = Lcg::new(0x6d61_7373);
    for _ in 0..60 {
        let (m, comp, d) = random_star(&mut rng);
        for options in [TrackOptions::default(), TrackOptions::hurley2000()] {
            let track = Track::full_with(m, &comp, &d, options);
            let mut last = f64::INFINITY;
            for a in ages_of(&track, 800) {
                let mass = track.state_at(Years::new(a)).mass().value();
                assert!(
                    mass <= last,
                    "{m:?}, {comp:?}: mass rises at {a}: {mass} > {last}"
                );
                last = mass;
            }
        }
    }
}

/// Ruling 30: a release build lets a non-finite value through `StarState::new`, so the
/// integrator must never produce one. Random tracks under both recipes, every state at 400
/// ages each and at every segment boundary.
#[test]
fn no_state_of_a_track_is_non_finite() {
    let mut rng = Lcg::new(0x6e61_6e73);
    for _ in 0..120 {
        let (m, comp, d) = random_star(&mut rng);
        for options in [TrackOptions::default(), TrackOptions::hurley2000()] {
            let track = Track::full_with(m, &comp, &d, options);
            let mut ages = ages_of(&track, 400);
            ages.extend(track.segments().iter().map(|s| s.start));
            for a in ages {
                let s = track.state_at(Years::new(a));
                let values = [
                    s.mass().value(),
                    s.core_mass().value(),
                    s.envelope_mass().value(),
                    s.luminosity().value(),
                    s.radius().value(),
                    s.effective_temperature().value(),
                    s.mass_loss_rate().value(),
                    s.phase_fraction(),
                ];
                assert!(
                    values.iter().all(|v| v.is_finite() && *v >= 0.0),
                    "{m:?}, {comp:?}, {options:?} at {a}: {s:?}"
                );
                assert!(
                    s.phase().is_remnant()
                        || (s.luminosity().value() > 0.0 && s.radius().value() > 0.0),
                    "a living star shines and has a size: {s:?}"
                );
            }
        }
    }
}

/// The largest radius and luminosity so far never fall and never lie below the present ones.
#[test]
fn the_largest_radius_and_luminosity_so_far_never_fall() {
    let mut rng = Lcg::new(0x6d61_7869);
    for _ in 0..40 {
        let (m, comp, d) = random_star(&mut rng);
        let track = Track::full(m, &comp, &d);
        let mut ages = ages_of(&track, 1_500);
        for s in track.segments() {
            if s.end.is_finite() {
                ages.extend((0..=40).map(|i| s.start + (s.end - s.start) * f64::from(i) / 40.0));
            }
        }
        ages.sort_by(f64::total_cmp);
        let mut last = [0.0_f64; 2];
        for a in ages {
            let now = track.state_at(Years::new(a));
            let r = track.max_radius_until(Years::new(a)).value();
            let l = track.max_luminosity_until(Years::new(a)).value();
            assert!(
                r >= now.radius().value() && l >= now.luminosity().value(),
                "{m:?} at {a}"
            );
            assert!(
                r >= last[0] * (1.0 - 1e-12) && l >= last[1] * (1.0 - 1e-12),
                "{m:?}, {comp:?}: a maximum falls at {a}: {r} after {}, {l} after {}",
                last[0],
                last[1]
            );
            last = [r.max(last[0]), l.max(last[1])];
        }
    }
}

/// The plan's end point for the backbone: with HPT's winds and η = 0.5 a 1 M☉ star of solar
/// metallicity ends as a carbon–oxygen white dwarf of 0.50–0.56 M☉ (0.5197 M☉ here; the published
/// SSE code gives 0.5196 at converged steps).
#[test]
fn a_solar_mass_star_ends_as_a_carbon_oxygen_white_dwarf_of_half_a_solar_mass() {
    let track = Track::full_with(
        SolarMasses::new(1.0),
        &composition(0.02),
        &StarDraws::median(),
        TrackOptions::hurley2000(),
    );
    let death = track.death().expect("a full track reaches death");
    assert_eq!(death.kind(), DeathKind::EnvelopeLoss);
    let remnant = track.remnant().expect("and leaves a remnant");
    assert_eq!(remnant.kind(), RemnantKind::WhiteDwarf);
    let m = remnant.mass().value();
    assert!((0.50..=0.56).contains(&m), "{m}");
    let after = track.state_at(Years::new(death.age().value() + 1e6));
    assert_eq!(after.phase(), Phase::CarbonOxygenWhiteDwarf);
    assert!((after.mass().value() - m).abs() < 1e-15);
}

/// Ruling 40: a helium star whose core stops at the shell limit 1.45 M − 0.31, below 0.689 M☉,
/// leaves a carbon–oxygen white dwarf of its whole mass, as the published SSE code does, not of
/// its core.
#[test]
fn a_light_helium_star_leaves_a_white_dwarf_of_its_whole_mass() {
    let comp = composition(0.02);
    let coeffs = ZCoeffs::new(comp.z_fit());
    let builder = build::Builder::new(
        &coeffs,
        &comp,
        TrackOptions::hurley2000(),
        ReimersEta::new(0.5),
        Resolution::GENERATOR,
        build::Keep::Track,
    );
    let mut step = builder.helium_main_sequence(0.0, 0.5, 0.0, None);
    let fate = loop {
        match step.next {
            build::Entry::HeliumShellBurning { star, clock0, mass } => {
                step = builder.helium_shell_burning(step.end, &star, clock0, mass, step.end_state);
            }
            build::Entry::Dead(fate) => break fate,
            other => panic!("a helium star of 0.5 M☉ does not enter {other:?}"),
        }
    };
    assert_eq!(fate.phase, Phase::CarbonOxygenWhiteDwarf);
    let progenitor = fate.death.progenitor();
    let (whole, core) = (
        progenitor.helium_core_mass().value(),
        progenitor.co_core_mass().value(),
    );
    assert!(
        whole < 0.5 && whole > core + 0.05,
        "{whole} against a core of {core}"
    );
    assert!((fate.remnant.mass().value() - whole).abs() < 1e-15);
    assert!((core - (1.45 * whole - 0.31)).abs() < 1e-6, "{core}");
}

/// The endings by initial mass that P06.T11 left to the tracks: under HPT's recipes a 20 M☉ star
/// of solar metallicity leaves a neutron star and 40 M☉ a black hole, as SSE does.
#[test]
fn twenty_solar_masses_leave_a_neutron_star_and_forty_a_black_hole() {
    let end = |m: f64| {
        Track::full_with(
            SolarMasses::new(m),
            &composition(0.02),
            &StarDraws::median(),
            TrackOptions::hurley2000(),
        )
        .remnant()
        .expect("a full track reaches death")
        .kind()
    };
    assert_eq!(end(20.0), RemnantKind::NeutronStar);
    assert_eq!(end(40.0), RemnantKind::BlackHole);
}

/// Doubling the knots and the midpoint steps moves the lifetime and the final mass (the remnant's)
/// by under 0.5% for the 16 masses of P06.T12 at two metallicities, under both recipes, which is
/// what shows the grid converged (design note 1).
#[test]
fn doubling_the_knots_and_steps_moves_the_lifetime_and_final_mass_by_under_half_a_percent() {
    for z in [0.001, 0.02] {
        for m in GRID_MASSES {
            for options in [TrackOptions::default(), TrackOptions::hurley2000()] {
                let build = |r: Resolution| {
                    Track::build(
                        SolarMasses::new(m),
                        &composition(z),
                        &StarDraws::median(),
                        options,
                        r,
                        None,
                    )
                };
                let (coarse, fine) = (
                    build(Resolution::GENERATOR),
                    build(Resolution::GENERATOR.doubled()),
                );
                let what = format!("M = {m}, Z = {z}, {options:?}");
                let (a, b) = (coarse.lifetime().unwrap(), fine.lifetime().unwrap());
                assert!(
                    (a / b - 1.0).abs() < 5e-3,
                    "lifetime at {what}: {a:?} against {b:?}"
                );
                let (ra, rb) = (coarse.remnant().unwrap(), fine.remnant().unwrap());
                assert_eq!(ra.kind(), rb.kind(), "remnant at {what}");
                if rb.mass().value() > 0.0 {
                    let (ma, mb) = (ra.mass().value(), rb.mass().value());
                    assert!(
                        (ma / mb - 1.0).abs() < 5e-3,
                        "final mass at {what}: {ma} against {mb}"
                    );
                }
            }
        }
    }
}

/// Ages concentrated near a track's junctions: around each segment boundary at spacings from 10⁻⁸
/// to a fifth of the neighbouring segments, and through each segment.
fn junction_sweep(track: &Track, n: usize) -> Vec<f64> {
    let mut ages = Vec::with_capacity(n);
    let segments = track.segments();
    let per = (n / segments.len().max(1)).max(8);
    for (i, s) in segments.iter().enumerate() {
        let end = if s.end.is_finite() {
            s.end
        } else {
            s.start + 1e8
        };
        let len = end - s.start;
        for k in 0..per {
            let x = f64::from(u32::try_from(k).unwrap()) / f64::from(u32::try_from(per).unwrap());
            // Cluster towards both ends of the segment.
            let y = 0.5 - 0.5 * math::cos(core::f64::consts::PI * x);
            ages.push(s.start + len * y);
        }
        if i > 0 {
            for e in 1..=8 {
                let d = len * math::exp10(-f64::from(e));
                ages.push(s.start + d);
                ages.push((s.start - d).max(0.0));
            }
        }
    }
    ages.sort_by(f64::total_cmp);
    ages.dedup_by(|a, b| a.total_cmp(b).is_eq());
    ages
}

/// Where the track's state steps at `a`: a declared step, a sudden death or the white dwarf's
/// hand-over, lies within `b − a`.
fn declared_between(track: &Track, a: f64, b: f64) -> bool {
    track.declared_steps().iter().any(|&s| s >= a && s <= b)
}

/// Checks a track's continuity (plan 06, P06.T10.d): log L and log R have no jump of more than 1%
/// (0.0043 dex, the plan's "continuous to 1%" at the loss of an envelope, ruling 29) on `ages`,
/// except at the steps it declares, with P06.T5's jump detector. Returns how many junctions it
/// crossed.
fn assert_track_continuous(track: &Track, ages: &[f64], what: &str) -> usize {
    const TOLERANCE: f64 = 0.004_3;
    let log_l = |a: f64| math::log10(track.state_at(Years::new(a)).luminosity().value());
    let log_r = |a: f64| math::log10(track.state_at(Years::new(a)).radius().value());
    let starts: Vec<f64> = track.segments().iter().map(|s| s.start).collect();
    let mut crossed = 0;
    for pair in ages.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b <= a || declared_between(track, a, b) {
            continue;
        }
        if track.state_at(Years::new(b)).phase().is_remnant() {
            continue;
        }
        crossed += starts.iter().filter(|&&s| s > a && s <= b).count();
        for (name, f) in [("log L", &log_l as &dyn Fn(f64) -> f64), ("log R", &log_r)] {
            if (f(b) - f(a)).abs() <= TOLERANCE {
                continue;
            }
            if let Some((lo, hi)) = find_jump(f, a, b, TOLERANCE) {
                panic!(
                    "{what}: {name} jumps by {} between {lo} and {hi}",
                    f(hi) - f(lo)
                );
            }
        }
    }
    crossed
}

/// The continuity sweep of P06.T10.d on a sample of random stars (the full 200, with 2,000 ages
/// each, is the slow test below).
#[test]
fn random_tracks_are_continuous_except_at_their_declared_steps() {
    let mut rng = Lcg::new(0x636f_6e74);
    let mut crossed = 0;
    for _ in 0..16 {
        let (m, comp, d) = random_star(&mut rng);
        let track = Track::full(m, &comp, &d);
        let ages = junction_sweep(&track, 400);
        crossed += assert_track_continuous(&track, &ages, &format!("{m:?}, {comp:?}"));
    }
    assert!(crossed > 20, "only {crossed} junctions crossed");
}

/// The continuity sweep of P06.T10.d in full: 200 random (m, Z, η), 2,000 ages each concentrated
/// near junctions.
#[test]
#[ignore = "slow: 200 tracks at 2,000 ages each, with the jump detector"]
fn two_hundred_random_tracks_are_continuous_except_at_their_declared_steps() {
    let mut rng = Lcg::new(0x7377_6565);
    let mut crossed = 0;
    for _ in 0..200 {
        let (m, comp, d) = random_star(&mut rng);
        for options in [TrackOptions::default(), TrackOptions::hurley2000()] {
            let track = Track::full_with(m, &comp, &d, options);
            let ages = junction_sweep(&track, 2_000);
            crossed += assert_track_continuous(&track, &ages, &format!("{m:?}, {comp:?}"));
        }
    }
    assert!(crossed > 500, "only {crossed} junctions crossed");
}

/// The loss of the envelope (the helium star or white dwarf a stripped core becomes) is continuous
/// in L and R to 1%, the plan's "continuous to 1%", which HPT section 6.3's perturbation provides
/// (ruling 29): stars that lose their envelope at each point of their evolution.
#[test]
fn the_loss_of_an_envelope_is_continuous_to_one_percent() {
    let mut checked = 0;
    for (m, z) in [
        (0.8, 0.02),
        (0.5, 0.02),
        (40.0, 0.02),
        (40.0, 0.004),
        (60.0, 0.0001),
        (60.0, 0.02),
        (100.0, 0.001),
        (100.0, 0.03),
        (20.0, 0.03),
        (1.0, 0.03),
    ] {
        let track = Track::full_with(
            SolarMasses::new(m),
            &composition(z),
            &StarDraws::median(),
            TrackOptions::hurley2000(),
        );
        for pair in track.segments().windows(2) {
            let (before, after) = (&pair[0], &pair[1]);
            let (pa, pb) = (
                track.state_at(Years::new(before.end * (1.0 - 1e-12))),
                track.state_at(Years::new(after.start)),
            );
            let helium = matches!(
                pb.phase(),
                Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
            );
            let hydrogen = !matches!(
                pa.phase(),
                Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
            );
            if helium && hydrogen {
                checked += 1;
                let dl = math::log10(pb.luminosity() / pa.luminosity());
                let dr = math::log10(pb.radius() / pa.radius());
                assert!(dl.abs() < 0.004_3, "L at M = {m}, Z = {z}: {dl} dex");
                assert!(dr.abs() < 0.004_3, "R at M = {m}, Z = {z}: {dr} dex");
            }
        }
    }
    assert!(checked >= 4, "only {checked} envelope losses");
}

/// No living phase at an age beyond the lifetime, and no remnant before it (P06.T10.e), over
/// random stars and ages.
#[test]
fn a_star_lives_until_its_lifetime_and_is_a_remnant_after() {
    check_life_and_death(0x6c69_6665, 300);
}

/// The same over 10⁵ random inputs.
#[test]
#[ignore = "slow: 10⁵ random stars and ages"]
fn a_hundred_thousand_stars_live_until_their_lifetime_and_are_remnants_after() {
    check_life_and_death(0x6c69_6666, 100_000);
}

fn check_life_and_death(seed: u64, stars: u32) {
    let mut rng = Lcg::new(seed);
    for _ in 0..stars {
        let (mass, comp, star) = random_star(&mut rng);
        let life = lifetime_of(mass, &comp, &star, TrackOptions::default()).value();
        // An age within a factor of ten either side of the lifetime, or near it.
        let near = rng.next_f64() < 0.2;
        let age = if near {
            life * (1.0 + (rng.next_f64() - 0.5) * 1e-9)
        } else {
            life * math::exp10(2.0 * rng.next_f64() - 1.0)
        };
        let state = crate::stellar::evolve(mass, &comp, &star, Years::new(age));
        let what = format!(
            "{mass:?}, {comp:?}, age {age} of life {life}: {:?}",
            state.phase()
        );
        if age < life {
            assert!(state.phase().is_living(), "{what}");
        } else {
            assert!(state.phase().is_remnant(), "{what}");
        }
    }
}

/// `lifetime` keeps no track and returns [`Track::lifetime`] of the full build bit for bit (the
/// full 10⁴ inputs are the slow test below).
#[test]
fn the_fast_lifetime_is_the_full_tracks_bit_for_bit() {
    check_lifetimes(0x6661_7374, 150);
}

#[test]
#[ignore = "slow: 10⁴ random stars built twice"]
fn the_fast_lifetime_is_the_full_tracks_over_ten_thousand_stars() {
    check_lifetimes(0x6661_7375, 10_000);
}

fn check_lifetimes(seed: u64, n: u32) {
    let mut rng = Lcg::new(seed);
    for _ in 0..n {
        let (m, comp, d) = random_star(&mut rng);
        let full = Track::full(m, &comp, &d)
            .lifetime()
            .expect("a full track dies");
        let fast = crate::stellar::lifetime(m, &comp, &d);
        assert_eq!(bits(fast.value()), bits(full.value()), "{m:?}, {comp:?}");
    }
}

/// The phases a star of `m` passes through, in order.
fn route(m: f64, comp: &Composition) -> Vec<Phase> {
    let track = Track::full(SolarMasses::new(m), comp, &StarDraws::median());
    let physics = track.physics();
    let mut phases: Vec<Phase> = track
        .segments()
        .iter()
        .map(|s| s.evaluate(&physics, s.start).point.phase)
        .collect();
    phases.dedup();
    phases
}

/// The lifetime falls with initial mass and is continuous in it wherever stars of neighbouring
/// masses pass through the same phases: it steps where they do not, at `M_HeF` (the flash's reset
/// of the initial mass, HPT section 7.1, and the flash bridge) and where the giant branch's wind
/// strips the envelope before the flash, leaving a helium white dwarf where the next heavier star
/// goes on to burn helium for another 10⁸ years. Two metallicities at 60 intervals of log mass
/// (the five of P06.T12 at 400 are the slow test below).
#[test]
fn the_lifetime_falls_with_mass_and_is_continuous_between_changes_of_route() {
    check_lifetime_in_mass(&[1e-4, 0.02], 60);
}

#[test]
#[ignore = "slow: 2,000 lifetimes and their jump detection"]
fn the_lifetime_falls_with_mass_and_is_continuous_at_five_metallicities() {
    check_lifetime_in_mass(&[1e-4, 0.001, 0.004, 0.02, 0.03], 400);
}

/// The lifetime at `intervals` + 1 masses evenly spaced in log mass over 0.1–100 M☉, at each of
/// `metallicities`: falling, and without a jump of 10⁻³ dex between two masses of the same route.
fn check_lifetime_in_mass(metallicities: &[f64], intervals: u32) {
    for &z in metallicities {
        let comp = composition(z);
        let life = |log_m: f64| {
            math::log10(
                crate::stellar::lifetime(
                    SolarMasses::new(math::exp10(log_m)),
                    &comp,
                    &StarDraws::median(),
                )
                .value(),
            )
        };
        let log_masses: Vec<f64> = (0..=intervals)
            .map(|i| -1.0 + 3.0 * f64::from(i) / f64::from(intervals))
            .collect();
        let values: Vec<f64> = log_masses.iter().map(|&x| life(x)).collect();
        for (i, pair) in values.windows(2).enumerate() {
            assert!(
                pair[1] < pair[0],
                "the lifetime rises at Z = {z} from {} to {} M☉",
                math::exp10(log_masses[i]),
                math::exp10(log_masses[i + 1])
            );
        }
        let routes: Vec<Vec<Phase>> = log_masses
            .iter()
            .map(|&x| route(math::exp10(x), &comp))
            .collect();
        let m_hef = math::log10(ZCoeffs::new(comp.z_fit()).m_hef().value());
        let mut changes = 0;
        for (i, pair) in log_masses.windows(2).enumerate() {
            let (a, b) = (pair[0], pair[1]);
            if routes[i] != routes[i + 1] || (a < m_hef && b >= m_hef) {
                changes += 1;
                continue;
            }
            super::super::continuity::assert_no_jump(
                &format!("log lifetime at Z = {z}"),
                life,
                a,
                b,
                1e-3,
            );
        }
        assert!(
            4 * changes < intervals,
            "{changes} changes of route in {intervals} intervals at Z = {z}"
        );
    }
}

/// Reimers' η is 0.5 + 0.07 z (design note 7), and never negative.
#[test]
fn reimers_eta_follows_design_note_7() {
    assert!((reimers_eta(0.0).value() - 0.5).abs() < 1e-15);
    assert!((reimers_eta(1.0).value() - 0.57).abs() < 1e-15);
    assert!((reimers_eta(-2.0).value() - 0.36).abs() < 1e-15);
    assert!(reimers_eta(-8.5).value() >= 0.0);
}

/// A remnant's state: a white dwarf fades by HPT's cooling law at the radius of its mass, a
/// neutron star has P06.T11's radius, and a black hole is dark.
#[test]
fn remnants_fade_or_stay_dark() {
    let comp = composition(0.02);
    let wd = Track::full(SolarMasses::new(2.0), &comp, &StarDraws::median());
    let death = wd.lifetime().unwrap().value();
    let (young, old) = (
        wd.state_at(Years::new(death + 1e6)),
        wd.state_at(Years::new(death + 1e9)),
    );
    assert!(old.luminosity() < young.luminosity());
    assert!((old.radius().value() - young.radius().value()).abs() < 1e-15);
    let bh = Track::full_with(
        SolarMasses::new(40.0),
        &comp,
        &StarDraws::median(),
        TrackOptions::hurley2000(),
    );
    let after = bh.state_at(Years::new(bh.lifetime().unwrap().value() + 1e3));
    assert_eq!(after.phase(), Phase::BlackHole);
    assert!(after.luminosity().value() <= 0.0);
    assert!(after.radius().value() > 0.0);
}

/// The thermal pulses since the start of the thermally pulsing AGB rise through it, and there are
/// none outside it.
#[test]
fn the_thermal_pulses_count_up_through_the_pulsing_agb() {
    let track = Track::full(
        SolarMasses::new(2.0),
        &composition(0.02),
        &StarDraws::median(),
    );
    let segment = track
        .segments()
        .iter()
        .find(|s| matches!(s.model, Model::ThermallyPulsingAgb { .. }))
        .expect("a 2 M☉ star pulses");
    let mut last = -1.0;
    for i in 0..100 {
        let a = segment.start + (segment.end - segment.start) * f64::from(i) / 100.0;
        let p = track
            .pulse_phase_at(Years::new(a))
            .expect("inside the phase");
        assert!(p >= last, "{p} after {last}");
        last = p;
    }
    assert!(last > 3.0, "{last} pulses");
    assert_eq!(track.pulse_phase_at(Years::new(segment.start * 0.5)), None);
}
