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

/// A black hole five times its neutron star's mass, about the class's usual pair, swallows it
/// whole and ejects nothing, from 1.15 M☉ to the heaviest neutron star: Foucart et al.'s remnant
/// vanishes above a compactness of 0.137, 1.134 M☉ at 12.2 km.
#[test]
fn a_black_hole_at_q_5_swallows_its_neutron_star_whole() {
    for k in 0..=17 {
        let ns = 1.15 + 0.05 * f64::from(k);
        let outflow = NsbhOutflow::of(SolarMasses::new(5.0 * ns), SolarMasses::new(ns));
        assert_eq!(outflow.remnant().value(), 0.0, "{ns}");
        assert_eq!(outflow.ejecta().value(), 0.0, "{ns}");
    }
    let usual = NsbhOutflow::of(SolarMasses::new(7.0), SolarMasses::new(1.33));
    assert_eq!(usual.ejecta().value(), 0.0);
}

/// A light hole tears its neutron star apart: a 3 M☉ hole and a 1.2 M☉ neutron star (q 2.5, C
/// 0.145, χ 0, inside Foucart et al.'s fit's Q of 1–7 and C of 0.13–0.182) leave 0.0469 M☉ outside
/// the hole, of which a quarter is dynamical, and eject 1.40% of the neutron star, as measured.
#[test]
fn a_light_hole_ejects_a_stated_share_of_its_neutron_star() {
    let (bh, ns) = (3.0, 1.2);
    let compactness =
        crate::units::consts::GM_SUN * ns / (12_200.0 * SPEED_OF_LIGHT * SPEED_OF_LIGHT);
    assert!((0.13..=0.182).contains(&compactness), "{compactness}");
    assert!((1.0..=7.0).contains(&(bh / ns)));
    let outflow = NsbhOutflow::of(SolarMasses::new(bh), SolarMasses::new(ns));
    eprintln!(
        "{outflow:?}, ejecta share {}",
        outflow.ejecta().value() / ns
    );
    let near = |got: SolarMasses, want: f64| (got.value() / want - 1.0).abs() < 1e-3;
    assert!(near(outflow.remnant(), REMNANT_AT_Q_2_5), "{outflow:?}");
    assert!(near(outflow.dynamical(), DYNAMICAL_AT_Q_2_5), "{outflow:?}");
    assert!(near(outflow.ejecta(), EJECTA_AT_Q_2_5), "{outflow:?}");
    let sum = outflow.dynamical().value() + outflow.disc().value();
    assert!((sum - outflow.remnant().value()).abs() <= 1e-15);
    // At q 3 a little is still left; nothing is dynamical there.
    let three = NsbhOutflow::of(SolarMasses::new(3.6), SolarMasses::new(1.2));
    assert!(
        three.ejecta().value() > 0.0 && three.ejecta() < outflow.ejecta(),
        "{three:?}"
    );
}

/// The outflow at q 2.5 and χ 0 of a 1.2 M☉ neutron star, M☉, as measured: what is left outside
/// the hole, its dynamical part and the kilonova's ejecta.
const REMNANT_AT_Q_2_5: f64 = 0.046_88;
const DYNAMICAL_AT_Q_2_5: f64 = 0.011_49;
const EJECTA_AT_Q_2_5: f64 = 0.016_80;

/// No pair of the class's black holes (3–20 M☉) and neutron stars (1.13–2.0 M☉) ejects more than
/// two neutron stars do, GW170817's 0.05 M☉: the kilonovae's bound at the top of their ejecta
/// (P12.T5.b's row).
#[test]
fn no_black_hole_ejects_more_than_two_neutron_stars() {
    let mut most = 0.0_f64;
    for i in 0..=200 {
        let bh = 3.0 * math::powf(20.0 / 3.0, f64::from(i) / 200.0);
        for j in 0..=200 {
            let ns = 1.13 + 0.87 * f64::from(j) / 200.0;
            let outflow = NsbhOutflow::of(SolarMasses::new(bh), SolarMasses::new(ns));
            assert!(outflow.ejecta().value().is_finite() && outflow.ejecta().value() >= 0.0);
            assert!(outflow.dynamical() <= outflow.remnant());
            most = most.max(outflow.ejecta().value());
        }
    }
    eprintln!("the most a black hole ejects: {most} M☉");
    assert!(
        most < 0.6 * super::super::light_curves::GW170817_EJECTA_MASS.value(),
        "{most}"
    );
}

/// The share of the class's black-hole pairs that eject anything, over its sampler's ranks of the
/// hole's and the neutron star's masses (a grid of 1,000 × 1,000 midpoints): 8.9%, as measured,
/// most of them with the lightest neutron stars. Each made a 0.05 M☉ kilonova before P11.T9.b's
/// fix.
#[test]
fn few_of_the_class_s_black_holes_eject_anything() {
    use crate::tables::binary::{NSM_SAMPLER, NsmMark};
    let sampler = ClassSampler::new(&NSM_SAMPLER);
    let n = 1_000;
    let rank = |k: u32| (f64::from(k) + 0.5) / f64::from(n);
    let holes: Vec<f64> = (0..n)
        .map(|k| math::exp10(sampler.value_at(NsmMark::LogBlackHoleMass, 0.0, rank(k))))
        .collect();
    let stars: Vec<f64> = (0..n)
        .map(|k| sampler.value_at(NsmMark::NeutronStarMass, 0.0, rank(k)))
        .collect();
    let ejecting: u32 = holes
        .iter()
        .flat_map(|&bh| stars.iter().map(move |&ns| (bh, ns)))
        .map(|(bh, ns)| {
            let outflow = NsbhOutflow::of(SolarMasses::new(bh), SolarMasses::new(ns));
            u32::from(outflow.ejecta().value() > 0.0)
        })
        .sum();
    let share = f64::from(ejecting) / f64::from(n * n);
    eprintln!("{share} of the black holes eject");
    assert!((share - EJECTING_SHARE).abs() < 1e-4, "{share}");
}

/// The share of the class's black-hole pairs that eject, as measured.
const EJECTING_SHARE: f64 = 0.0893;
