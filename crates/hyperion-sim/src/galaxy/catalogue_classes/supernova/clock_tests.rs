//! The supernova test with a clock (plan 09, P09.T17): the marks, the shared test and its
//! interval.
//!
//! The fixtures are one drawn galaxy, built per test (nextest runs each test in a process of its
//! own), a core collapse re-aged out of one of its layer-E cells, and marks with every field given
//! for the sweeps and the boundaries.

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::Population;
use crate::galaxy::catalogue_classes::type_ia::DelayTimeDistribution;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::id::Layer;
use crate::stellar::remnant::{ProgenitorAtDeath, Stripping};
use crate::units::{HydrogenPerCm3, Kelvin};

/// The galaxy of every test here: the seed whose layer-E cell (0, 203, 0) the doctests read.
fn galaxy() -> Galaxy {
    Galaxy::new(Seed::new(19))
}

/// The record `record` with its age at the epoch set to `age`.
fn re_aged(record: &SystemRecord, age: Years) -> SystemRecord {
    SystemRecord::from_parts(
        record.id(),
        *record.epoch_position(),
        record.origin(),
        record.population(),
        record.primary_initial_mass(),
        age,
    )
}

/// The first record of `galaxy`'s layer-E cell (0, 203, 0), at the solar circle, whose primary
/// makes a supernova, re-aged so that it explodes about `since` Julian years before the epoch
/// (after it, for a negative `since`), with its stars and its site at its epoch position.
///
/// The metallicity draw reads the age, so the re-aged death moves a little from the target.
fn exploding(galaxy: &Galaxy, since: f64) -> (SystemStars, ExplosionSite) {
    let mut cell = Vec::new();
    generate_cell(
        galaxy,
        CellKey::new(Layer::E, [0, 203, 0]).unwrap(),
        &mut cell,
    );
    for record in &cell {
        let ClockDeath::At(death, _) = SystemStars::generate(galaxy, record).death_time() else {
            continue;
        };
        let lifetime = death.since_epoch().as_julian_years_f64() + record.age_at_epoch().value();
        let dying = re_aged(record, Years::new(lifetime + since));
        let stars = SystemStars::generate(galaxy, &dying);
        let ClockDeath::At(t, _) = stars.death_time() else {
            continue;
        };
        let site = ExplosionSite::new(*dying.epoch_position(), t);
        if DeathMarks::of(galaxy, &stars, &site, GalacticVelocity::default()).is_some() {
            return (stars, site);
        }
    }
    panic!("the cell holds a layer-E primary that makes a supernova");
}

/// A system ID for marks no system has.
fn some_system(k: u64) -> SystemId {
    SystemId::from_raw(0xF000_0007_0000_0000 | k).unwrap()
}

/// The clock time `years` Julian years from the epoch, floored to the nanosecond.
fn at_years(years: f64) -> UniverseTime {
    let span = Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR).unwrap();
    UniverseTime::EPOCH.checked_add(span).unwrap()
}

/// One nanosecond, the clock's tick.
fn tick() -> Span {
    Span::new(0, 1).unwrap()
}

/// A site in the plane at the solar circle, where the gas gives a window of some 10⁵ years.
fn solar_site(death: UniverseTime) -> ExplosionSite {
    let p = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap();
    ExplosionSite::new(p, death)
}

/// A superbubble interior inside the gas the bubble's cap was scanned over: a density in the
/// interior's truncated range, 1.26 × 10⁻³ to 0.020 cm⁻³ (ruling 136.3), at 1.6 × 10⁶ K, a little
/// hotter than a nursery's 10^6.2 K, so its window is a little shorter.
fn interior(density: f64, wall: f64) -> BubbleInterior {
    let gas = SiteGas::hot_interior(HydrogenPerCm3::new(density), Kelvin::new(1.6e6)).unwrap();
    BubbleInterior::new(gas, LightYears::new(wall))
}

/// The marks of a core collapse of median energy at `death`, its remnant moving at `speed` km/s.
fn plain_marks(death: UniverseTime, speed: f64) -> DeathMarks {
    DeathMarks::from_parts(
        some_system(1),
        death,
        SupernovaKind::CoreCollapse(SupernovaType::IIP),
        ExplosionEnergy::MEDIAN,
        MetresPerSecond::from(KilometresPerSecond::new(speed)),
    )
}

/// P09.T17: membership is independent of any query time. `claims` takes no time, and the entry
/// built from the marks keeps its ID through the progenitor, the supernova and the bare remnant.
#[test]
fn membership_does_not_depend_on_the_query_time() {
    let galaxy = galaxy();
    let (stars, site) = exploding(&galaxy, 1e4);
    let velocity = GalacticVelocity::new([12e3, -230e3, 4e3]);
    let marks = DeathMarks::of(&galaxy, &stars, &site, velocity).unwrap();
    assert_eq!(marks.system(), stars.record().id());
    assert_eq!(marks.death_time(), site.death());
    let claimed = claims(&galaxy, &site, &marks, Surroundings::Field);
    assert!(claimed, "dead some 10⁴ years: {marks:?}");
    let window = effective_window(&galaxy, &site, &marks, Surroundings::Field);
    let w = window.duration().value();
    assert!(w > 1e4, "{w}");
    let kick = stars.natal_kick().unwrap();
    let entry = SupernovaEntry::new(marks.system(), marks.death_time(), marks.kind(), window)
        .with_remnant(KilometresPerSecond::from(kick.speed()), None);
    let t = marks.death_time();
    let mut seen = Vec::new();
    for offset in [-2e4, -1.0, 0.0, 1.0, 0.5 * w, w + 1.0, 3e6] {
        let at = if offset < 0.0 {
            t.checked_sub(Span::from_seconds_f64(-offset * SECONDS_PER_JULIAN_YEAR).unwrap())
        } else {
            t.checked_add(Span::from_seconds_f64(offset * SECONDS_PER_JULIAN_YEAR).unwrap())
        }
        .unwrap();
        let state = entry.state_at(at);
        seen.push(match state {
            SupernovaState::Progenitor { .. } => 0,
            SupernovaState::Supernova { .. } => 1,
            SupernovaState::BareRemnant { .. } => 2,
        });
        // The same member 0 at every time, and the same answer.
        assert_eq!(entry.system(), marks.system());
        assert_eq!(entry.explosion_event().subject().system(), marks.system());
        assert_eq!(claims(&galaxy, &site, &marks, Surroundings::Field), claimed);
    }
    assert_eq!(seen, vec![0, 0, 1, 1, 1, 2, 2]);
    // A star due to explode five centuries from now is an entry at the epoch already, a
    // progenitor then.
    let (stars, site) = exploding(&galaxy, -500.0);
    let marks = DeathMarks::of(&galaxy, &stars, &site, velocity).unwrap();
    assert!(marks.death_time() > UniverseTime::EPOCH);
    assert!(claims(&galaxy, &site, &marks, Surroundings::Field));
    let window = effective_window(&galaxy, &site, &marks, Surroundings::Field);
    let entry = SupernovaEntry::new(marks.system(), marks.death_time(), marks.kind(), window);
    assert!(matches!(
        entry.state_at(UniverseTime::EPOCH),
        SupernovaState::Progenitor { .. }
    ));
}

/// The marks of a core collapse: T and the kind from the stellar stage, the remnant at the
/// system's velocity plus its kick, and an energy of the core-collapse law on its own stream.
#[test]
fn the_marks_read_the_stars_and_their_own_stream() {
    let galaxy = galaxy();
    let (stars, site) = exploding(&galaxy, 2e5);
    let velocity = GalacticVelocity::new([30e3, -220e3, -9e3]);
    let marks = DeathMarks::of(&galaxy, &stars, &site, velocity).unwrap();
    let ClockDeath::At(t, death) = stars.death_time() else {
        panic!("it died");
    };
    assert_eq!(marks.death_time(), t);
    assert_eq!(
        Some(marks.kind()),
        SupernovaKind::of_death(&stars.primary().death().unwrap())
    );
    assert!(matches!(
        death,
        DeathKind::CoreCollapse { .. } | DeathKind::ElectronCapture
    ));
    let kick = stars.natal_kick().unwrap();
    let after = velocity + GalacticVelocity::new(kick.velocity());
    assert_same_bits(marks.fastest_member_speed().value(), after.speed().value());
    // The energy: one uniform of `snr.energy`, word 0, keyed by the system's ID, through the law
    // of the death, bit for bit.
    let key = ObjectKey::from(stars.record().id());
    let u = Stream::open(galaxy.seed(), tags::SNR_ENERGY, key).uniform();
    let law = if death == DeathKind::ElectronCapture {
        ExplosionEnergy::electron_capture_from_uniform(u)
    } else {
        ExplosionEnergy::from_uniform(u)
    };
    assert_same_bits(marks.energy().foes(), law.foes());
    // The same system gives the same marks twice, and the velocity moves only v_fast.
    assert_eq!(
        DeathMarks::of(&galaxy, &stars, &site, velocity),
        Some(marks)
    );
    let still = DeathMarks::of(&galaxy, &stars, &site, GalacticVelocity::default()).unwrap();
    assert_eq!(still.energy(), marks.energy());
    assert_same_bits(still.fastest_member_speed().value(), kick.speed().value());
    // Two systems draw on two streams.
    let other = drawn_energy(&galaxy, some_system(2), EnergyLaw::Standard);
    let another = drawn_energy(&galaxy, some_system(3), EnergyLaw::Standard);
    assert_ne!(other, another);
    assert_ne!(other, marks.energy());
}

/// P09.T17, design point (e): what each death makes.
#[test]
fn each_death_makes_its_supernova_or_none() {
    let m = SolarMasses::new;
    let death = |kind: DeathKind, envelope: f64, helium_core: f64, co_core: f64| {
        Death::new(
            Years::new(1e7),
            kind,
            ProgenitorAtDeath::new(m(co_core), m(helium_core), m(envelope), Stripping::None),
        )
    };
    for supernova in [
        SupernovaType::IIP,
        SupernovaType::IIL,
        SupernovaType::IIb,
        SupernovaType::Ib,
        SupernovaType::Ic,
    ] {
        let collapse = DeathKind::CoreCollapse { supernova };
        assert_eq!(
            SupernovaKind::of_death(&death(collapse, 8.0, 5.0, 3.0)),
            Some(SupernovaKind::CoreCollapse(supernova))
        );
    }
    let failed = DeathKind::CoreCollapse {
        supernova: SupernovaType::None,
    };
    assert_eq!(SupernovaKind::of_death(&death(failed, 8.0, 5.0, 3.0)), None);
    // A super-AGB star keeps its envelope: a Type IIP. Stripped of hydrogen it would be a Ib with
    // 0.5 M☉ of helium outside its core, a Ic with 0.1.
    let capture = DeathKind::ElectronCapture;
    assert_eq!(
        SupernovaKind::of_death(&death(capture, 7.0, 1.9, 1.37)),
        Some(SupernovaKind::CoreCollapse(SupernovaType::IIP))
    );
    assert_eq!(
        SupernovaKind::of_death(&death(capture, 0.0, 1.9, 1.4)),
        Some(SupernovaKind::CoreCollapse(SupernovaType::Ib))
    );
    assert_eq!(
        SupernovaKind::of_death(&death(capture, 0.0, 1.5, 1.4)),
        Some(SupernovaKind::CoreCollapse(SupernovaType::Ic))
    );
    for none in [
        DeathKind::EnvelopeLoss,
        DeathKind::DirectCollapse,
        DeathKind::PairInstability,
        DeathKind::ThermonuclearDisruption,
    ] {
        assert_eq!(
            SupernovaKind::of_death(&death(none, 8.0, 5.0, 3.0)),
            None,
            "{none:?}"
        );
    }
}

/// P09.T17, design point (b): a Type Ia candidate's marks, from its progenitor and leftover, with
/// the Iax law for a Type Iax and the bound |v| + the survivor's speed.
#[test]
fn a_type_ia_candidate_has_marks_of_its_own() {
    let galaxy = galaxy();
    let delays = DelayTimeDistribution::from_galaxy(&galaxy);
    let after = at_years(-1e5);
    let until = ClockWindow::END;
    let progenitors: Vec<IaProgenitor> = (0..400_u64)
        .filter_map(|k| {
            let mut s = Stream::open(
                Seed::new(0x0917_1a00),
                tags::SELFTEST_STREAM,
                ObjectKey::galaxy_item(k),
            );
            IaProgenitor::draw(&delays, &mut s, Population::OldThinDisc, after, until)
        })
        .collect();
    let velocity = GalacticVelocity::new([0.0, -200e3, 0.0]);
    let mut seen = [false; 4];
    for (k, progenitor) in (0_u64..).zip(&progenitors) {
        let system = some_system(0x100 + k);
        let site = solar_site(progenitor.explosion());
        let leftover = match progenitor.channel() {
            IaChannel::Merger => IaLeftover::Nothing,
            IaChannel::DoubleDetonation => IaLeftover::SurvivingDonor {
                speed: KilometresPerSecond::new(2_400.0),
            },
            IaChannel::HydrogenDonor => IaLeftover::HydrogenDonor {
                speed: KilometresPerSecond::new(150.0),
            },
            IaChannel::Iax => IaLeftover::PartlyBurntDwarf,
        };
        let marks = DeathMarks::of_type_ia(&galaxy, system, progenitor, &leftover, &site, velocity);
        assert_eq!(marks.death_time(), progenitor.explosion());
        assert_eq!(marks.kind(), SupernovaKind::TypeIa(progenitor.channel()));
        let survivor = leftover
            .speed()
            .map_or(0.0, |s| MetresPerSecond::from(s).value());
        assert_same_bits(marks.fastest_member_speed().value(), 200e3 + survivor);
        let u = Stream::open(galaxy.seed(), tags::SNR_ENERGY, ObjectKey::from(system)).uniform();
        let law = if progenitor.channel() == IaChannel::Iax {
            ExplosionEnergy::iax_from_uniform(u)
        } else {
            ExplosionEnergy::from_uniform(u)
        };
        assert_same_bits(marks.energy().foes(), law.foes());
        seen[IaChannel::ALL
            .iter()
            .position(|&c| c == progenitor.channel())
            .unwrap()] = true;
        // A fast survivor's lifetime cap holds its window, and the window stays positive.
        let window = effective_window(&galaxy, &site, &marks, Surroundings::TypeIa);
        let cap = lifetime_cap(marks.fastest_member_speed()).value();
        assert!(window.duration().value() <= cap.max(0.0));
        assert!(window.natural_duration().value() > 0.0);
    }
    assert_eq!(seen, [true; 4], "every channel among the progenitors");
}

/// P09.T17: `claims` is a pure function of its arguments. Over a seeded sweep of sites, marks and
/// surroundings the same arguments give the same answer and the same window, bit for bit, in any
/// order of evaluation.
#[test]
fn claims_is_a_pure_function_of_its_arguments() {
    let galaxy = galaxy();
    let mut lcg = Lcg::new(0x0917_0002);
    let cases: Vec<(ExplosionSite, DeathMarks, Surroundings)> = (0..240_u64)
        .map(|k| {
            let phi = lcg.next_f64() * core::f64::consts::TAU;
            let radius = 12_000.0 + 30_000.0 * lcg.next_f64();
            let height = 1_200.0 * lcg.next_f64() - 600.0;
            let position = GalacticPosition::from_light_years([
                radius * math::cos(phi),
                radius * math::sin(phi),
                height,
            ])
            .unwrap();
            // T over the widest interval and a little beyond either end.
            let death = at_years(-4.4e6 + 4.402e6 * lcg.next_f64());
            let kind = [
                SupernovaKind::CoreCollapse(SupernovaType::IIP),
                SupernovaKind::CoreCollapse(SupernovaType::Ib),
                SupernovaKind::TypeIa(IaChannel::Merger),
                SupernovaKind::TypeIa(IaChannel::Iax),
            ][usize::try_from(lcg.next_below(4)).unwrap()];
            let rank = lcg.next_f64();
            let energy = if kind == SupernovaKind::TypeIa(IaChannel::Iax) {
                ExplosionEnergy::iax_from_uniform(rank)
            } else {
                ExplosionEnergy::from_uniform(rank)
            };
            let speed = MetresPerSecond::from(KilometresPerSecond::new(3_000.0 * lcg.next_f64()));
            let marks = DeathMarks::from_parts(some_system(k), death, kind, energy, speed);
            let surroundings = match lcg.next_below(3) {
                0 => Surroundings::Field,
                1 => Surroundings::TypeIa,
                _ => Surroundings::Bubble(interior(
                    1.3e-3 * math::exp10(1.16 * lcg.next_f64()),
                    300.0 * lcg.next_f64(),
                )),
            };
            (ExplosionSite::new(position, death), marks, surroundings)
        })
        .collect();
    let answer = |(site, marks, surroundings): &(ExplosionSite, DeathMarks, Surroundings)| {
        let window = effective_window(&galaxy, site, marks, *surroundings);
        (
            claims(&galaxy, site, marks, *surroundings),
            bits(window.duration().value()),
        )
    };
    // `claims` is `claimed_window`'s answer, and the window it gives is the effective one.
    for (site, marks, surroundings) in &cases {
        let claimed = claimed_window(&galaxy, site, marks, *surroundings);
        assert_eq!(
            claimed.is_some(),
            claims(&galaxy, site, marks, *surroundings)
        );
        if let Some(window) = claimed {
            assert_eq!(
                window,
                effective_window(&galaxy, site, marks, *surroundings)
            );
        }
    }
    let first: Vec<_> = cases.iter().map(answer).collect();
    let second: Vec<_> = cases.iter().map(answer).collect();
    assert_eq!(first, second);
    assert_order_independent(&cases, answer);
    // The sweep reaches both answers in every environment.
    for environment in ShellEnvironment::ALL {
        let answers: Vec<bool> = cases
            .iter()
            .zip(&first)
            .filter(|((_, _, s), _)| s.environment() == environment)
            .map(|(_, &(claimed, _))| claimed)
            .collect();
        assert!(
            answers.contains(&true) && answers.contains(&false),
            "{environment:?}"
        );
    }
}

/// P09.T17: the boundaries are exact. T = +H is claimed and the next tick is not; T = −(`W_eff` +
/// L + H) is not claimed and the next tick is.
#[test]
fn the_interval_s_ends_are_exact() {
    let galaxy = galaxy();
    let probe = plain_marks(UniverseTime::EPOCH, 300.0);
    let site = solar_site(UniverseTime::EPOCH);
    let window = effective_window(&galaxy, &site, &probe, Surroundings::Field);
    let interval = SupernovaInterval::of_window(window.duration());
    let start = interval.start();
    eprintln!(
        "W_eff {:.6e} yr at the solar site; the interval opens at {start} (excluded) and \
         closes at {} (included)",
        window.duration().value(),
        interval.end()
    );
    let claimed_at = |t: UniverseTime| {
        let marks = plain_marks(t, 300.0);
        claims(&galaxy, &solar_site(t), &marks, Surroundings::Field)
    };
    assert!(claimed_at(ClockWindow::END));
    assert!(!claimed_at(ClockWindow::END.checked_add(tick()).unwrap()));
    assert!(!claimed_at(start));
    assert!(claimed_at(start.checked_add(tick()).unwrap()));
    // The start is the window floored to the nanosecond, plus L + H.
    let w = Span::from_seconds_f64(window.duration().value() * SECONDS_PER_JULIAN_YEAR).unwrap();
    assert_eq!(interval.window(), w);
    let opens = UniverseTime::EPOCH
        .checked_sub(
            w.checked_add(LIGHT_CROSSING_L)
                .unwrap()
                .checked_add(CLOCK_WINDOW_H)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(start, opens);
    assert_eq!(interval.end(), ClockWindow::END);
}

/// P09.T17: the lifetime cap. A survivor at 2,500 km/s holds `W_eff` under 0.23 Myr, and over a
/// sweep `W_eff` is never negative, never above its environment's cap and never above
/// `SHELL_WINDOW_CAP`.
#[test]
fn the_lifetime_cap_holds_the_fastest_member() {
    let galaxy = galaxy();
    let fast = MetresPerSecond::from(KilometresPerSecond::new(2_500.0));
    let cap = lifetime_cap(fast).value();
    assert_same_bits(cap, 4_096.0 * SPEED_OF_LIGHT / 2.5e6 - 264_144.0);
    assert!((cap - 227_035.963_187_2).abs() < 1e-6, "{cap}");
    assert!(cap < 2.3e5);
    eprintln!("lifetime cap at 2,500 km/s: {cap:.4} yr");
    let site = solar_site(UniverseTime::EPOCH);
    let marks = DeathMarks::from_parts(
        some_system(9),
        UniverseTime::EPOCH,
        SupernovaKind::TypeIa(IaChannel::DoubleDetonation),
        ExplosionEnergy::MAX,
        fast,
    );
    // A thin, low-pressure site, whose window would be longer than the cap.
    let thin = ExplosionSite::new(
        GalacticPosition::from_light_years([0.0, 30_000.0, 900.0]).unwrap(),
        UniverseTime::EPOCH,
    );
    for site in [site, thin] {
        let window = effective_window(&galaxy, &site, &marks, Surroundings::TypeIa);
        assert!(window.duration().value() <= cap);
        assert!(window.duration().value() < 2.3e5);
    }
    // There the shell would outlast the cap, so the cap is what ends it.
    let window = effective_window(&galaxy, &thin, &marks, Surroundings::TypeIa);
    assert!(window.natural_duration().value() > cap, "{window:?}");
    assert_same_bits(window.duration().value(), cap);
    // Faster than 4,096 ly ÷ (L + 2H) the window is nothing.
    assert!(lifetime_cap(MetresPerSecond::from(KilometresPerSecond::new(4_700.0))).value() < 0.0);
    let mut lcg = Lcg::new(0x0917_0004);
    for k in 0..400_u64 {
        let phi = lcg.next_f64() * core::f64::consts::TAU;
        let r = 5_000.0 + 45_000.0 * lcg.next_f64();
        let z = 4_000.0 * lcg.next_f64() - 2_000.0;
        let p = GalacticPosition::from_light_years([r * math::cos(phi), r * math::sin(phi), z])
            .unwrap();
        let speed = MetresPerSecond::from(KilometresPerSecond::new(6_000.0 * lcg.next_f64()));
        let marks = DeathMarks::from_parts(
            some_system(0x200 + k),
            UniverseTime::EPOCH,
            SupernovaKind::CoreCollapse(SupernovaType::IIL),
            ExplosionEnergy::from_uniform(lcg.next_f64()),
            speed,
        );
        let surroundings = match k % 3 {
            0 => Surroundings::Field,
            1 => Surroundings::TypeIa,
            _ => Surroundings::Bubble(interior(1.3e-3 + 0.018 * lcg.next_f64(), 1e4)),
        };
        let site = ExplosionSite::new(p, UniverseTime::EPOCH);
        let w = effective_window(&galaxy, &site, &marks, surroundings)
            .duration()
            .value();
        let environment_cap = galaxy.window_caps().cap(surroundings.environment()).value();
        assert!(w >= 0.0, "{w}");
        assert!(w <= environment_cap, "{w} over {environment_cap}");
        assert!(w <= SHELL_WINDOW_CAP.value());
        assert!(w <= lifetime_cap(speed).value().max(0.0));
    }
}

/// P09.T17: every interval lies inside plan 08's `death_in_interval`'s, and an environment's
/// interval and thinning cap are its cap grown by L + H and L + 2H.
#[test]
fn every_interval_lies_inside_death_in_interval_s() {
    let galaxy = galaxy();
    // Plan 08's interval opens 4,263,144 Julian years, 134,534,593,094,400 s, before the epoch.
    let widest = SupernovaInterval::of_window(SHELL_WINDOW_CAP);
    assert_eq!(
        widest.start(),
        UniverseTime::new(-134_534_593_094_400, 0).unwrap()
    );
    assert_eq!(widest, SupernovaInterval::of_window(Years::new(1e9)));
    assert_eq!(
        SupernovaInterval::of_window(Years::new(-5.0)).window(),
        Span::ZERO
    );
    assert_eq!(
        SupernovaInterval::of_window(Years::new(f64::NAN)).window(),
        Span::ZERO
    );
    let caps = galaxy.window_caps();
    let l_2h = Span::from_julian_years(264_144).unwrap();
    for environment in ShellEnvironment::ALL {
        let cap = caps.cap(environment);
        let interval = SupernovaInterval::of_cap(caps, environment);
        let cap_span = Span::from_seconds_f64(cap.value() * SECONDS_PER_JULIAN_YEAR).unwrap();
        assert_eq!(interval.window(), cap_span);
        assert!(interval.start() >= widest.start());
        assert_eq!(
            interval.start(),
            UniverseTime::EPOCH
                .checked_sub(
                    cap_span
                        .checked_add(Span::from_julian_years(263_144).unwrap())
                        .unwrap()
                )
                .unwrap()
        );
        let thinning = SupernovaInterval::thinning_cap(caps, environment);
        assert_eq!(thinning, cap_span.checked_add(l_2h).unwrap());
        assert_eq!(thinning, interval.length());
        assert_eq!(
            interval.end().checked_since(interval.start()),
            Some(thinning)
        );
        eprintln!(
            "{environment:?}: cap {:.6e} yr, thinning cap {thinning}, opens at {}",
            cap.value(),
            interval.start()
        );
    }
    // An entry's interval at its own W_eff is no wider than its environment's.
    let marks = plain_marks(UniverseTime::EPOCH, 50.0);
    let site = solar_site(UniverseTime::EPOCH);
    let w = effective_window(&galaxy, &site, &marks, Surroundings::Field).duration();
    let entry = SupernovaInterval::of_window(w);
    assert!(entry.start() >= SupernovaInterval::of_cap(caps, ShellEnvironment::Field).start());
}

/// P09.T17, ruling 136.9: the window reads the site gas's `[M/H]`, which `GasField` gives as the
/// logarithm of its dust per hydrogen, bit for bit; the star's metallicity is not read.
#[test]
fn the_window_reads_the_site_gas_s_metallicity() {
    let galaxy = galaxy();
    let marks = plain_marks(UniverseTime::EPOCH, 100.0);
    for x in [-40_000.0, -8_000.0, 0.0, 15_000.0, 30_000.0] {
        let p = GalacticPosition::from_light_years([x, 9_000.0, 40.0]).unwrap();
        let metallicity = galaxy.gas().metallicity_at(&p);
        assert_same_bits(
            galaxy.gas().dust_per_hydrogen(&p),
            math::exp10(metallicity.value()),
        );
        let site = ExplosionSite::new(p, UniverseTime::EPOCH);
        let window = effective_window(&galaxy, &site, &marks, Surroundings::Field);
        let held = metallicity.value().clamp(-1.0, 0.5);
        assert_same_bits(window.metallicity_ratio(), math::exp10(held));
    }
}

/// P09.T17, design point (g): inside a superbubble the window is the interior's, cut at the
/// bubble's cap and ended where the shell reaches the wall.
#[test]
fn a_bubble_s_window_ends_at_its_wall() {
    let galaxy = galaxy();
    let marks = plain_marks(UniverseTime::EPOCH, 400.0);
    let site = solar_site(UniverseTime::EPOCH);
    let far = effective_window(
        &galaxy,
        &site,
        &marks,
        Surroundings::Bubble(interior(5e-3, 1e5)),
    );
    let near = effective_window(
        &galaxy,
        &site,
        &marks,
        Surroundings::Bubble(interior(5e-3, 20.0)),
    );
    let bubble_cap = galaxy.window_caps().cap(ShellEnvironment::Bubble).value();
    assert!(far.duration().value() <= bubble_cap);
    assert_same_bits(far.duration().value(), far.natural_duration().value());
    assert!(near.duration() < far.duration());
    assert!(near.duration().value() > 0.0);
    // Outside the bubble the shell ends at once.
    let outside = effective_window(
        &galaxy,
        &site,
        &marks,
        Surroundings::Bubble(interior(5e-3, -1.0)),
    );
    assert_eq!(outside.duration(), Years::ZERO);
    // A shell that ends at once is still claimed over (−(L + H), +H].
    let at = |t: UniverseTime| plain_marks(t, 400.0);
    let bubble = Surroundings::Bubble(interior(5e-3, -1.0));
    assert!(claims(&galaxy, &site, &at(at_years(-1e5)), bubble));
    assert!(!claims(&galaxy, &site, &at(at_years(-3e5)), bubble));
}

/// P09.T17, ruling 136.10: the Iax law holds its range and median, and a shell at its least
/// energy has a finite, positive window.
#[test]
fn the_iax_law_holds_its_range_and_median() {
    let (lo, hi) = (ExplosionEnergy::IAX_MIN, ExplosionEnergy::IAX_MAX);
    let span = ExplosionEnergy::IAX_TRUNCATION * ExplosionEnergy::IAX_SIGMA_DEX;
    assert!((math::log10(lo.foes()) - (ExplosionEnergy::IAX_LOG_MEDIAN - span)).abs() < 1e-14);
    assert!((math::log10(hi.foes()) - (ExplosionEnergy::IAX_LOG_MEDIAN + span)).abs() < 1e-14);
    assert!(
        hi <= ExplosionEnergy::MAX,
        "the caps scan at the greatest energy"
    );
    let median = ExplosionEnergy::iax_from_uniform(0.5).foes();
    assert!(
        (median - math::exp10(ExplosionEnergy::IAX_LOG_MEDIAN)).abs() < 1e-15 * median.max(1.0)
    );
    assert!((ExplosionEnergy::iax_from_uniform(0.0).foes() / lo.foes() - 1.0).abs() < 1e-12);
    assert!((ExplosionEnergy::iax_from_uniform(1.0).foes() / hi.foes() - 1.0).abs() < 1e-12);
    let mut last = 0.0;
    for k in 0..=1_000 {
        let e = ExplosionEnergy::iax_from_uniform(f64::from(k) / 1_000.0).foes();
        assert!(e >= last && e >= lo.foes() && e <= hi.foes(), "{e}");
        last = e;
    }
    // About 6% of Iax lie above 0.4 × 10⁵¹ erg: (Φ(2) − Φ(1.388)) ÷ (Φ(2) − Φ(−2)) = 0.0627.
    let above = (0..100_000)
        .filter(|&k| ExplosionEnergy::iax_from_uniform((f64::from(k) + 0.5) / 1e5).foes() > 0.4)
        .count();
    assert!((5_900..=6_600).contains(&above), "{above} in 10⁵");
    // Every energy either law gives is one `from_foes` accepts, and nothing beyond.
    assert_eq!(ExplosionEnergy::from_foes(lo.foes()), Ok(lo));
    assert_eq!(ExplosionEnergy::from_foes(hi.foes()), Ok(hi));
    assert!(matches!(
        ExplosionEnergy::from_foes(lo.foes() * 0.999),
        Err(crate::galaxy::snr::BuildExplosionEnergyError::OutOfRange(_))
    ));
    // The window at the least energy, across the window table's densities at a floor's pressure
    // and in a bubble's interior.
    for n in [1e-3, 1e-2, 0.1, 1.0, 10.0, 100.0, 1e3, 1e4] {
        let site = SiteGas::uniform(
            HydrogenPerCm3::new(n),
            crate::units::KelvinPerCm3::new(300.0),
        )
        .unwrap();
        let w = shell_window(&site, lo, crate::units::Dex::new(0.0))
            .duration()
            .value();
        assert!(w.is_finite() && w > 0.0, "n = {n}: {w}");
    }
    let w = shell_window(interior(1.3e-3, 1e4).gas(), lo, crate::units::Dex::new(0.0))
        .duration()
        .value();
    assert!(w.is_finite() && w > 0.0, "{w}");
}

/// P09.T17, design point (h): `claims` and the marks reach no feature. Neither takes a feature
/// cache, nor anything that holds one, and nothing in this module, or in the window and gas code
/// they call, names the feature catalogue or a cache of its cells, so no feature can be looked up.
#[test]
fn the_test_reaches_no_feature() {
    let _: fn(&Galaxy, &ExplosionSite, &DeathMarks, Surroundings) -> bool = claims;
    let _: fn(&Galaxy, &ExplosionSite, &DeathMarks, Surroundings) -> ShellWindow = effective_window;
    let _: fn(&Galaxy, &ExplosionSite, &DeathMarks, Surroundings) -> Option<ShellWindow> =
        claimed_window;
    let _: fn(&Galaxy, &SystemStars, &ExplosionSite, GalacticVelocity) -> Option<DeathMarks> =
        DeathMarks::of;
    let _: fn(
        &Galaxy,
        SystemId,
        &IaProgenitor,
        &IaLeftover,
        &ExplosionSite,
        GalacticVelocity,
    ) -> DeathMarks = DeathMarks::of_type_ia;
    let sources = [
        (
            "catalogue_classes/supernova.rs",
            include_str!("../supernova.rs"),
        ),
        ("snr/window.rs", include_str!("../../snr/window.rs")),
        ("snr/mod.rs", include_str!("../../snr/mod.rs")),
        ("snr/shell.rs", include_str!("../../snr/shell.rs")),
        ("snr/caps.rs", include_str!("../../snr/caps.rs")),
        ("gas/field.rs", include_str!("../../gas/field.rs")),
        ("gas/noise.rs", include_str!("../../gas/noise.rs")),
        ("gas/phase.rs", include_str!("../../gas/phase.rs")),
        ("gas/pressure.rs", include_str!("../../gas/pressure.rs")),
        ("gas/smooth.rs", include_str!("../../gas/smooth.rs")),
    ];
    for (name, source) in sources {
        let code = source
            .split("\n#[cfg(test)]\nmod ")
            .next()
            .unwrap_or(source);
        for forbidden in [
            "FeatureCatalogue",
            "FeatureCellCache",
            "NoFeatureCache",
            "FeatureRef",
        ] {
            assert!(!code.contains(forbidden), "{name} names {forbidden}");
        }
    }
    // Nor do the test and the window it reads name a feature's path or plan 07's modifiers.
    for (name, source) in [sources[0], sources[1]] {
        let code = source.split("\n#[cfg(test)]\nmod ").next().unwrap();
        for forbidden in ["features::", "GasModifier", "modifiers_near"] {
            assert!(!code.contains(forbidden), "{name} names {forbidden}");
        }
    }
}

/// The amendment of 2026-10-27 to P09.T17: the electron-capture law holds its range and median,
/// sits below the core-collapse law, and a shell at its least energy has a positive window.
#[test]
fn the_electron_capture_law_holds_its_range_and_median() {
    let (lo, hi) = (
        ExplosionEnergy::ELECTRON_CAPTURE_MIN,
        ExplosionEnergy::ELECTRON_CAPTURE_MAX,
    );
    let span =
        ExplosionEnergy::ELECTRON_CAPTURE_TRUNCATION * ExplosionEnergy::ELECTRON_CAPTURE_SIGMA_DEX;
    let log_median = ExplosionEnergy::ELECTRON_CAPTURE_LOG_MEDIAN;
    assert!((math::log10(lo.foes()) - (log_median - span)).abs() < 1e-14);
    assert!((math::log10(hi.foes()) - (log_median + span)).abs() < 1e-14);
    // √2 × 10⁵⁰ erg, the geometric middle of the simulations' 1–2 × 10⁵⁰, to the law's −0.85.
    assert!((log_median - math::log10(core::f64::consts::SQRT_2 * 0.1)).abs() < 1e-3);
    // Every energy of the law lies inside the type's range and below the core-collapse law's.
    assert!(hi < ExplosionEnergy::MIN && lo > ExplosionEnergy::IAX_MIN);
    let at = |u: f64| ExplosionEnergy::electron_capture_from_uniform(u).foes();
    assert!((at(0.5) / math::exp10(log_median) - 1.0).abs() < 1e-15);
    assert!((at(0.0) / lo.foes() - 1.0).abs() < 1e-12);
    assert!((at(1.0) / hi.foes() - 1.0).abs() < 1e-12);
    let mut last = 0.0;
    for k in 0..=1_000 {
        let e = at(f64::from(k) / 1_000.0);
        assert!(e >= last && e >= lo.foes() && e <= hi.foes(), "{e}");
        last = e;
    }
    // The quartiles of a normal truncated at ±2: ±0.6397 σ.
    assert!(((math::log10(at(0.75)) - log_median) / 0.2 - 0.6397).abs() < 1e-3);
    assert_eq!(ExplosionEnergy::from_foes(lo.foes()), Ok(lo));
    for n in [1e-3, 1e-2, 0.1, 1.0, 10.0, 100.0, 1e3, 1e4] {
        let site = SiteGas::uniform(
            HydrogenPerCm3::new(n),
            crate::units::KelvinPerCm3::new(300.0),
        )
        .unwrap();
        let w = shell_window(&site, lo, crate::units::Dex::new(0.0))
            .duration()
            .value();
        assert!(w.is_finite() && w > 0.0, "n = {n}: {w}");
    }
}

/// Each death and channel draws on its law: electron capture's for an electron-capture death,
/// Iax's for a Type Iax, the standard law for the rest, all on the same word.
#[test]
fn each_death_draws_on_its_law() {
    let m = SolarMasses::new;
    let death = |kind: DeathKind| {
        Death::new(
            Years::new(2e7),
            kind,
            ProgenitorAtDeath::new(m(1.37), m(1.9), m(7.0), Stripping::None),
        )
    };
    assert_eq!(
        EnergyLaw::of_death(&death(DeathKind::ElectronCapture)),
        EnergyLaw::ElectronCapture
    );
    for supernova in [SupernovaType::IIP, SupernovaType::Ic] {
        let kind = DeathKind::CoreCollapse { supernova };
        assert_eq!(EnergyLaw::of_death(&death(kind)), EnergyLaw::Standard);
    }
    assert_eq!(EnergyLaw::of_channel(IaChannel::Iax), EnergyLaw::Iax);
    for channel in [
        IaChannel::Merger,
        IaChannel::DoubleDetonation,
        IaChannel::HydrogenDonor,
    ] {
        assert_eq!(EnergyLaw::of_channel(channel), EnergyLaw::Standard);
    }
    let galaxy = galaxy();
    let system = some_system(0x300);
    let u = Stream::open(galaxy.seed(), tags::SNR_ENERGY, ObjectKey::from(system)).uniform();
    for (law, energy) in [
        (EnergyLaw::Standard, ExplosionEnergy::from_uniform(u)),
        (
            EnergyLaw::ElectronCapture,
            ExplosionEnergy::electron_capture_from_uniform(u),
        ),
        (EnergyLaw::Iax, ExplosionEnergy::iax_from_uniform(u)),
    ] {
        assert_same_bits(drawn_energy(&galaxy, system, law).foes(), energy.foes());
    }
}

/// The laws, the draw and the window, pinned bit for bit at the version in force, so that a change
/// to any of them before P09.T35 fails here rather than riding T35's bump unseen.
#[test]
fn the_laws_the_draw_and_the_window_are_pinned() {
    // Each law at three ranks: standard, electron capture, Iax.
    for (u, pinned) in [
        (
            0.1,
            [
                0x3fe2_8ccb_9965_fc9a,
                0x3fb4_f648_0571_0940,
                0x3f81_724f_3af6_3061,
            ],
        ),
        (
            0.5,
            [
                0x3ff0_0000_0000_0000,
                0x3fc2_149a_5fed_24da,
                0x3fa9_a929_4b85_36ea,
            ],
        ),
        (
            0.9,
            [
                0x3ffb_99e2_0ba2_7e1d,
                0x3fcf_30a8_f63f_ce18,
                0x3fd2_df0b_67cf_4d03,
            ],
        ),
    ] {
        let laws = [
            ExplosionEnergy::from_uniform(u),
            ExplosionEnergy::electron_capture_from_uniform(u),
            ExplosionEnergy::iax_from_uniform(u),
        ];
        assert_eq!(laws.map(|e| bits(e.foes())), pinned, "u = {u}");
    }
    // Two systems' draws on seed 19's `snr.energy`, through each law.
    let galaxy = galaxy();
    for (k, pinned) in [
        (
            1_u64,
            [
                0x3ff0_83ce_8c1a_957a,
                0x3fc2_a98c_7774_6eb1,
                0x3fac_7163_cf37_e267,
            ],
        ),
        (
            2,
            [
                0x3fe5_0c3e_f549_ca0c,
                0x3fb7_c8e1_36b4_4421,
                0x3f8a_4dbe_c86a_0c34,
            ],
        ),
    ] {
        let laws = [
            EnergyLaw::Standard,
            EnergyLaw::ElectronCapture,
            EnergyLaw::Iax,
        ];
        let drawn = laws.map(|law| bits(drawn_energy(&galaxy, some_system(k), law).foes()));
        assert_eq!(drawn, pinned, "system {k}");
    }
    // The solar site's W_eff, 3.181 × 10⁵ yr, and the interval's start on the clock.
    let marks = plain_marks(UniverseTime::EPOCH, 300.0);
    let site = solar_site(UniverseTime::EPOCH);
    let w = effective_window(&galaxy, &site, &marks, Surroundings::Field);
    assert_eq!(bits(w.duration().value()), 0x4113_6acd_c366_1d17);
    let interval = SupernovaInterval::of_window(w.duration());
    assert_eq!(
        interval.start(),
        UniverseTime::new(-18_343_657_851_197, 380_859_375).unwrap()
    );
}
