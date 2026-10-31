//! Tests of the scan marks' parts (P11.T8.a): the rate the recurrence asks for, the nova clock
//! and phase, and the engine counter. The class's own scan, against its cells, is tested beside
//! the class (`galaxy::catalogue_classes::binary`).

use super::*;
use crate::stellar::binary::classify::STEADY_BURNING_RATE;
use crate::stellar::binary::tests::pair;
use crate::stellar::binary::{CarvedClass, evolve};
use crate::time::LIGHT_CROSSING_L;

/// The rate inverts the fit: its recurrence is the asked one, to rounding, across the white dwarf
/// masses and recurrences the sampler draws, and the hold binds nowhere there.
#[test]
fn the_rate_gives_back_the_recurrence() {
    for m in (0..=40).map(|k| 0.55 + 0.81 * f64::from(k) / 40.0) {
        for p in (0..=70).map(|k| 4.45 + 3.55 * f64::from(k) / 70.0) {
            let mass = SolarMasses::new(m);
            let rate = transfer_rate_for(mass, p);
            assert!(rate.value() > 0.0 && rate < STEADY_BURNING_RATE, "{m} {p}");
            assert!(rate.value() < 1.0e-8, "{m} {p}: {rate:?}");
            assert!(rate < MAX_NOVA_TRANSFER_RATE);
            let back = math::log10(nova_recurrence_period(mass, rate).value());
            assert!((back - p).abs() < 1e-12, "{m} {p}: {back}");
        }
    }
}

/// The scan splits on the class test's own rule: a recurrence of L is fast, a nanosecond's worth
/// more is slow.
#[test]
fn the_scan_splits_as_the_class_test() {
    let l = LIGHT_CROSSING_L.as_julian_years_f64();
    assert_eq!(recurrence_class(l), CarvedClass::AccretingWdFast);
    assert_eq!(
        recurrence_class(l * (1.0 + 1e-15)),
        CarvedClass::AccretingWdSlow
    );
    assert_eq!(
        recurrence_class(f64::INFINITY),
        CarvedClass::AccretingWdSlow
    );
}

/// The nova clock's period is the recurrence to the nanosecond, its origin a share of one period
/// before the epoch, and its phase noise spans the horizon in few octaves.
#[test]
fn the_nova_clock_and_phase() {
    for (years, share) in [(28_000.0, 0.0), (262_144.0, 0.5), (1.0e8, 0.999)] {
        let clock = nova_clock(Years::new(years), share);
        let period = clock.period().as_seconds_f64();
        assert!((period / (years * SECONDS_PER_JULIAN_YEAR) - 1.0).abs() < 1e-15);
        let back = UniverseTime::EPOCH
            .checked_since(clock.origin())
            .expect("the origin is before the epoch")
            .as_seconds_f64();
        assert!((back - share * period).abs() <= 1.0e-9 * period.max(1.0));
        let phase = nova_phase(&clock);
        assert!(phase.octaves() <= 5, "{years}: {}", phase.octaves());
        assert!(phase.noise_bound_cycles() < 1.0, "{years}");
    }
}

/// The counter counts this thread's runs of the engine, and only those.
#[test]
fn the_engine_counter_counts_runs() {
    let before = engine_calls();
    let input = pair(1.0, 0.5, 10.0, 0.0, 0.02);
    let _ = evolve(&input, Years::new(1.0e9));
    assert_eq!(engine_calls(), before + 1);
    let _ = nova_phase(&nova_clock(Years::new(5.0e5), 0.25));
    assert_eq!(engine_calls(), before + 1);
}

/// The horizon's eruptions are the monotone phase's own listing there, event for event, over
/// slow recurrences from just beyond L to 10⁸ years and clocks of every origin; none has more
/// than two.
#[test]
fn the_horizon_eruptions_are_the_phase_s_listing() {
    use crate::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
    use hyperion_testkit::lcg::Lcg;
    let key = CatalogueCellKey::new(ClassId::ACCRETING_WHITE_DWARF_SLOW, [0, 50, 0]).unwrap();
    let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap();
    let l = math::log10(LIGHT_CROSSING_L.as_julian_years_f64());
    let mut lcg = Lcg::new(0x0b17_08a0_0e70);
    let mut seen = [0_u32; 3];
    for index in 0..20_000 {
        let id = SystemId::from(key.candidate_id(index, 0).unwrap());
        let series = nova_series(Seed::new(17), id);
        let log_years = l + 1e-9 + (8.0 - l) * lcg.next_f64();
        let clock = nova_clock(Years::new(math::exp10(log_years)), lcg.next_f64());
        let phase = nova_phase(&clock);
        let (times, count) = horizon_eruptions(&phase, &series, &clock);
        let mut listed = Vec::new();
        phase.events_in(&series, &clock, horizon, &mut listed);
        let listed: Vec<UniverseTime> = listed.iter().map(CycleEvent::time).collect();
        assert_eq!(&times[..usize::from(count)], listed.as_slice(), "{index}");
        seen[usize::from(count)] += 1;
    }
    // Most slow hosts' novae miss the horizon; some erupt in it once, a few twice.
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
}
