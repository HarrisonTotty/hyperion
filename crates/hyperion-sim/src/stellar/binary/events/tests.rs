//! The binary events' tests (P11.T9.b): the kinds' tags, both constructions' listings in any order
//! of asking and over split intervals, events in strict order, the state after n events a closed
//! form in n, the events active at an instant, the curves the events carry, the bursts' regimes,
//! the mergers' one-shots and a golden. The galaxy's nova rate is a slow test
//! (`tests/binary_events.rs`).

use std::sync::OnceLock;

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::events::testing::{assert_partition_independent, partition};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::{
    AccretingWdFast, AccretingWdSlow, AwdSide, NeutronStarMergers, StellarMergers, XrayBinaries,
};
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
use crate::galaxy::params::GalaxyParams;
use crate::stellar::binary::light_curves::GW170817_EJECTA_MASS;
use crate::stellar::binary::scan::{engine_calls, scan_cell, scan_merger_cell, scan_nsm_cell};
use crate::tables::binary::NsmPairKind;
use crate::time::SourceHorizon;

/// The seed of the Milky Way fixture the tests share.
const SEED: u64 = 0x0b17_09b0_0000_0001;

/// The fixture; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// The accreting white dwarf entries of a run of cells through the inner disc, both classes.
fn awd_entries() -> &'static [AwdEntry] {
    static ENTRIES: OnceLock<Vec<AwdEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let galaxy = galaxy();
        let fast = AccretingWdFast::new(galaxy);
        let slow = AccretingWdSlow::new(galaxy);
        let mut entries = Vec::new();
        for x in -4..4 {
            for side in [AwdSide::Fast, AwdSide::Slow] {
                let key = CatalogueCellKey::new(side.class(), [x, 20, 0]).unwrap();
                let cell = match side {
                    AwdSide::Fast => {
                        CatalogueClassCell::generate(galaxy, &fast, key).into_entries()
                    }
                    AwdSide::Slow => {
                        CatalogueClassCell::generate(galaxy, &slow, key).into_entries()
                    }
                };
                entries.extend(cell.into_iter().map(|e| e.value().clone()));
            }
        }
        entries
    })
}

/// The X-ray binaries' scan marks of the centre block and the inner-disc band.
fn xrb_hosts() -> &'static [XrbScanMarks] {
    static HOSTS: OnceLock<Vec<XrbScanMarks>> = OnceLock::new();
    HOSTS.get_or_init(|| {
        let galaxy = galaxy();
        let process = XrayBinaries::new(galaxy);
        let mut hosts = Vec::new();
        let cells = (-4..4)
            .flat_map(|x| (-4..4).map(move |y| [x, y, 0]))
            .chain((-12..12).flat_map(|x| (10..14).map(move |y| [x, y, 0])));
        for cell in cells {
            let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, cell).unwrap();
            scan_cell(galaxy, &process, key, &mut hosts);
        }
        hosts
    })
}

/// The stellar mergers' scan marks of a run of cells through the solar circle.
fn stellar_mergers() -> &'static [MergerScanMarks] {
    static HOSTS: OnceLock<Vec<MergerScanMarks>> = OnceLock::new();
    HOSTS.get_or_init(|| {
        let galaxy = galaxy();
        let process = StellarMergers::new(galaxy);
        let mut hosts = Vec::new();
        for x in -2..2 {
            let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, 6, 0]).unwrap();
            scan_merger_cell(galaxy, &process, key, &mut hosts);
        }
        assert!(hosts.len() >= 8, "{} stellar mergers", hosts.len());
        hosts
    })
}

/// The first merger of two neutron stars of the galaxy's 4,096 ly cells in and just below the
/// plane, in x, y, z order: the galaxy holds about ten mergers, and most of its black holes' make no
/// kilonova (P11.T9.b).
fn neutron_star_merger() -> &'static NsmScanMarks {
    static HOST: OnceLock<NsmScanMarks> = OnceLock::new();
    HOST.get_or_init(|| {
        let galaxy = galaxy();
        let process = NeutronStarMergers::new(galaxy);
        let mut hosts = Vec::new();
        let cells = (-16..16).flat_map(|x| (-16..16).flat_map(move |y| [[x, y, 0], [x, y, -1]]));
        cells
            .into_iter()
            .find_map(|cell| {
                let key = CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, cell).unwrap();
                scan_nsm_cell(galaxy, &process, key, &mut hosts);
                hosts
                    .iter()
                    .find(|host| host.kind() == NsmPairKind::DoubleNeutronStar)
                    .copied()
            })
            .expect("the galaxy has a merger of two neutron stars in the disc")
    })
}

/// The first X-ray binary host of `kind`.
fn xrb_of(kind: XrbHostKind) -> &'static XrbScanMarks {
    xrb_hosts()
        .iter()
        .find(|h| h.kind() == kind)
        .unwrap_or_else(|| panic!("the sample holds a {kind:?} host"))
}

/// The first accreting white dwarf entry that is a dwarf nova at the epoch.
fn dwarf_nova() -> &'static AwdEntry {
    awd_entries()
        .iter()
        .find(|e| {
            e.timeline().class_at(e.record().age_at_epoch())
                == BinaryClass::CataclysmicVariable(CvKind::DwarfNova)
        })
        .expect("the sample holds a dwarf nova")
}

/// The first fast accreting white dwarf entry.
fn fast_host() -> &'static AwdEntry {
    awd_entries()
        .iter()
        .find(|e| e.scan().side() == AwdSide::Fast)
        .expect("the sample holds a fast host")
}

/// `years` Julian years after the epoch.
fn at_years(years: i64) -> UniverseTime {
    UniverseTime::from_julian_years(years).unwrap()
}

/// A window from `from` to `to` Julian years after the epoch.
fn window_years(from: i64, to: i64) -> TimeWindow {
    TimeWindow::new(at_years(from), at_years(to)).unwrap()
}

/// The events of `system` in `window`.
fn listing(system: &BinarySystem<'_>, window: TimeWindow) -> Vec<BinaryEvent> {
    let mut out = Vec::new();
    events_in(system, window, &mut out);
    out
}

/// Each sample system, its name and a window over which it has some events but not too many.
fn systems() -> Vec<(&'static str, BinarySystem<'static>, TimeWindow)> {
    let seed = galaxy().seed();
    let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap();
    let burst_days = TimeWindow::new(
        UniverseTime::EPOCH,
        UniverseTime::EPOCH
            .checked_add(Span::from_seconds(60 * 86_400))
            .unwrap(),
    )
    .unwrap();
    vec![
        (
            "nova, from scan marks",
            BinarySystem::accreting_white_dwarf_scan(seed, fast_host().scan()),
            horizon,
        ),
        (
            "dwarf nova",
            BinarySystem::accreting_white_dwarf(seed, dwarf_nova()),
            window_years(-200, 0),
        ),
        (
            "neutron-star transient",
            BinarySystem::xray_binary(seed, xrb_of(XrbHostKind::NeutronStarTransient)),
            window_years(-100_000, 0),
        ),
        (
            "black-hole transient",
            BinarySystem::xray_binary(seed, xrb_of(XrbHostKind::BlackHoleTransient)),
            window_years(-100_000, 0),
        ),
        (
            "persistent neutron star's bursts",
            BinarySystem::xray_binary(seed, xrb_of(XrbHostKind::NeutronStarPersistent)),
            burst_days,
        ),
        (
            "Be/X giant outbursts",
            BinarySystem::xray_binary(seed, xrb_of(XrbHostKind::BeX)),
            window_years(-2_000, 0),
        ),
        (
            "luminous red nova",
            BinarySystem::stellar_merger(seed, &stellar_mergers()[0]),
            horizon,
        ),
        (
            "kilonova",
            BinarySystem::neutron_star_merger(seed, neutron_star_merger()),
            horizon,
        ),
    ]
}

/// `n` cuts of `window`, evenly spaced and then nudged by a nanosecond, so that cuts fall both on
/// and beside events.
fn cuts(window: TimeWindow, n: i64) -> Vec<UniverseTime> {
    let span = window.end().checked_since(window.start()).unwrap();
    (1..n)
        .map(|k| {
            let step = Span::from_seconds_f64(
                span.as_seconds_f64() * f64::from(i32::try_from(k).unwrap())
                    / f64::from(i32::try_from(n).unwrap()),
            )
            .unwrap();
            window.start().checked_add(step).unwrap()
        })
        .collect()
}

// 1. The kinds.

/// Each kind has its own tag of plan 11's block, and the tag names the kind back.
#[test]
fn each_kind_holds_its_tag() {
    let numbers: Vec<u16> = BinaryEventKind::ALL
        .iter()
        .map(|k| k.tag().number())
        .collect();
    assert_eq!(
        numbers,
        [0x0300, 0x0301, 0x0302, 0x0303, 0x0304, 0x0305, 0x0306]
    );
    for kind in BinaryEventKind::ALL {
        assert_eq!(BinaryEventKind::of_tag(kind.tag()), Some(kind));
    }
    assert_eq!(BinaryEventKind::of_tag(event_tags::SUPERNOVA), None);
}

// 2. The constructions.

/// Both constructions give the same events over a window as over its pieces, for every kind the
/// samples hold, cut evenly and beside the events themselves.
#[test]
fn every_kind_lists_the_same_events_over_split_intervals() {
    for (name, system, window) in systems() {
        let whole = listing(&system, window);
        assert!(!whole.is_empty(), "{name}: no event in its window");
        assert_partition_independent(window, &cuts(window, 7), |w| listing(&system, w));
        let mut beside: Vec<UniverseTime> = whole
            .iter()
            .take(3)
            .flat_map(|e| {
                [
                    e.onset(),
                    e.onset().checked_add(Span::new(0, 1).unwrap()).unwrap(),
                ]
            })
            .filter(|&t| window.contains(t) || t == window.end())
            .collect();
        beside.sort();
        beside.dedup();
        assert_partition_independent(window, &beside, |w| listing(&system, w));
    }
}

/// The pieces of a window give the same events whatever order they are asked in.
#[test]
fn any_order_of_asking_gives_the_same_events() {
    for (_, system, window) in systems() {
        let pieces = partition(window, &cuts(window, 5));
        assert_order_independent(&pieces, |w| listing(&system, *w));
    }
}

/// Within a kind the onsets rise strictly and every ID is its own; across kinds the listing is in
/// time order, ties by ID.
#[test]
fn events_are_strictly_ordered() {
    for (name, system, window) in systems() {
        let events = listing(&system, window);
        assert!(
            events
                .windows(2)
                .all(|p| (p[0].onset(), p[0].id()) < (p[1].onset(), p[1].id())),
            "{name}"
        );
        for kind in BinaryEventKind::ALL {
            let onsets: Vec<UniverseTime> = events
                .iter()
                .filter(|e| e.kind() == kind)
                .map(BinaryEvent::onset)
                .collect();
            assert!(onsets.windows(2).all(|p| p[0] < p[1]), "{name} {kind:?}");
        }
    }
}

/// No event leaves a state a later one reads: every event of a source radiates the host's one
/// curve, so the light after n events is n times one event's, a closed form in n.
#[test]
fn the_state_after_n_events_is_a_closed_form_in_n() {
    for (name, system, window) in systems() {
        let events = listing(&system, window);
        for kind in BinaryEventKind::ALL {
            let of_kind: Vec<&BinaryEvent> = events.iter().filter(|e| e.kind() == kind).collect();
            let Some(first) = of_kind.first() else {
                continue;
            };
            assert!(of_kind.iter().all(|e| e.curve() == first.curve()), "{name}");
            let one = first.curve().radiated_joules();
            let total = of_kind
                .iter()
                .fold(0.0, |sum, e| sum + e.curve().radiated_joules());
            #[expect(clippy::cast_precision_loss, reason = "a count of some hundreds")]
            let n = of_kind.len() as f64;
            assert!((total / (n * one) - 1.0).abs() < 1e-12, "{name} {kind:?}");
        }
    }
}

/// The events active at an instant are those of the listing that began before it and have not
/// faded: at an event's onset and just before its end it is active, at its end not.
#[test]
fn active_at_lists_the_running_events() {
    for (name, system, window) in systems() {
        let events = listing(&system, window);
        for event in events.iter().take(5) {
            let end = event.onset().checked_add(event.duration()).unwrap();
            let before_end = end.checked_sub(Span::new(0, 1).unwrap()).unwrap();
            for (t, running) in [(event.onset(), true), (before_end, true), (end, false)] {
                let mut active = Vec::new();
                active_at(&system, t, &mut active);
                assert_eq!(active.contains(event), running, "{name} at {t}");
                assert!(active.iter().all(|e| e.is_active_at(t)), "{name}");
            }
        }
    }
}

/// The events of each X-ray kind carry X-rays at their peak and those of the novae and the
/// mergers none; the Provides' functions read the event's curve.
#[test]
fn the_light_curves_are_the_events_curves() {
    for (name, system, window) in systems() {
        for event in listing(&system, window).iter().take(3) {
            let peak = event.curve().peak();
            assert_eq!(
                light_curve(event, peak, Band::V),
                event.curve().in_band(peak, Band::V)
            );
            let xrays = xray_luminosity(event, peak);
            assert_eq!(xrays, event.curve().xray_luminosity(peak));
            match event.kind() {
                BinaryEventKind::Nova
                | BinaryEventKind::DwarfNova
                | BinaryEventKind::LuminousRedNova
                | BinaryEventKind::Kilonova => {
                    assert_eq!(xrays, Watts::ZERO, "{name}");
                }
                BinaryEventKind::XrayTransient
                | BinaryEventKind::XrayBurst
                | BinaryEventKind::BeXGiantOutburst => {
                    assert!(xrays > Watts::ZERO, "{name}");
                }
            }
            assert_eq!(
                light_curve(event, Span::new(-1, 0).unwrap(), Band::V),
                Watts::ZERO
            );
        }
    }
}

/// The novae are the scan marks' own phase: an entry lists the novae its scan marks list, with
/// the same IDs and times, and a slow host's in the source horizon are its listed eruptions.
#[test]
fn the_novae_are_the_scan_marks_phase() {
    let seed = galaxy().seed();
    let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap();
    // A dwarf nova's outbursts would fill the horizon; its novae are the same phase.
    let not_dwarf_novae = awd_entries().iter().filter(|e| {
        e.timeline().class_at(e.record().age_at_epoch())
            != BinaryClass::CataclysmicVariable(CvKind::DwarfNova)
    });
    for entry in not_dwarf_novae.take(20) {
        let novae: Vec<BinaryEvent> =
            listing(&BinarySystem::accreting_white_dwarf(seed, entry), horizon)
                .into_iter()
                .filter(|e| e.kind() == BinaryEventKind::Nova)
                .collect();
        let mut cycles = Vec::new();
        entry.scan().eruptions_in(horizon, &mut cycles);
        let times: Vec<UniverseTime> = novae.iter().map(BinaryEvent::onset).collect();
        assert_eq!(
            times,
            cycles.iter().map(CycleEvent::time).collect::<Vec<_>>()
        );
        if entry.scan().side() == AwdSide::Slow {
            assert_eq!(times.as_slice(), entry.scan().eruptions());
        }
    }
}

/// A dwarf nova's outbursts recur on its cycle, some cycles skipped: the share of its cycles with
/// no outburst is the skip share, by a binomial test at α, and its cycle is within its limits.
#[test]
fn a_dwarf_nova_recurs_on_its_cycle() {
    use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};
    let seed = galaxy().seed();
    let entry = dwarf_nova();
    let system = BinarySystem::accreting_white_dwarf(seed, entry);
    let source = system
        .sources()
        .into_iter()
        .flatten()
        .find(|s| s.kind == BinaryEventKind::DwarfNova)
        .expect("a dwarf nova has a dwarf nova source");
    let Construction::Phase { clock, phase } = source.construction else {
        panic!("a dwarf nova's construction is a monotone phase");
    };
    let cycles = 20_000_i64;
    let skipped = (0..cycles)
        .filter(|&n| phase.event(&source.series, &clock, n).is_none())
        .count();
    let skipped = u64::try_from(skipped).unwrap();
    let total = u64::try_from(cycles).unwrap();
    #[expect(clippy::cast_precision_loss, reason = "a count of 20,000")]
    let n = total as f64;
    let fit = chi_square_gof(
        &[skipped, total - skipped],
        &[DWARF_NOVA_SKIP_SHARE * n, (1.0 - DWARF_NOVA_SKIP_SHARE) * n],
    );
    println!("{skipped} of {total} cycles skipped");
    assert_p_value("the dwarf nova's skipped cycles", fit.p_value, ALPHA);
    let cycle = clock.period().as_seconds_f64() / 86_400.0;
    let (shortest, longest) = DWARF_NOVA_CYCLES_DAYS;
    assert!((shortest..=longest).contains(&cycle), "{cycle}");
}

/// A dwarf nova's cycle reads the line `classify` splits on: every non-magnetic cataclysmic
/// variable of the sample is a dwarf nova exactly when its rate lies under `cv_line`.
#[test]
fn the_dwarf_novae_line_is_classify_s() {
    let mut checked = 0;
    for entry in awd_entries() {
        let class = entry.timeline().class_at(entry.record().age_at_epoch());
        let dwarf = match class {
            BinaryClass::CataclysmicVariable(CvKind::DwarfNova) => true,
            BinaryClass::CataclysmicVariable(CvKind::NovaLike) => false,
            _ => continue,
        };
        let binary = entry.binary();
        let line = cv_line(
            binary.white_dwarf().mass(),
            binary.donor().mass(),
            binary.orbit().semi_major_axis(),
        );
        assert_eq!(binary.transfer_rate() < line, dwarf, "{class:?}");
        checked += 1;
    }
    assert!(checked > 20, "{checked}");
}

/// The bursts' regimes: a helium burst between a hundredth and a tenth of the Eddington rate,
/// mixed below and above, none at or above it; the recurrence is hours at 10⁻⁹ M☉ yr⁻¹.
#[test]
fn the_bursts_follow_their_regimes() {
    assert_eq!(burst_fuel(0.005), Some(BurstFuel::HydrogenHelium));
    assert_eq!(burst_fuel(0.05), Some(BurstFuel::Helium));
    assert_eq!(burst_fuel(0.5), Some(BurstFuel::HydrogenHelium));
    assert_eq!(burst_fuel(1.0), None);
    assert_eq!(burst_fuel(0.0), None);
    let (fuel, period) =
        burst_cycle(SolarMasses::new(1.45), SolarMassesPerYear::new(1.0e-9)).unwrap();
    assert_eq!(fuel, BurstFuel::Helium);
    let hours = period.as_seconds_f64() / 3_600.0;
    println!("a burst every {hours:.2} h at 1e-9 M_sun/yr");
    assert!((2.0..20.0).contains(&hours), "{hours}");
    assert!(burst_cycle(SolarMasses::new(1.45), SolarMassesPerYear::new(1.0e-7)).is_none());
    // Just under the Eddington rate the bursts recur every 20 minutes or more.
    let edd = eddington_rate(XrayAccretor::NeutronStar, SolarMasses::new(2.2));
    let (_, fastest) = burst_cycle(
        SolarMasses::new(2.2),
        SolarMassesPerYear::new(edd.value() * 0.999),
    )
    .unwrap();
    assert!(fastest.as_seconds_f64() > 20.0 * 60.0, "{fastest:?}");
}

/// A supergiant host has no event of its own: its X-rays are steady (as a persistent black hole's
/// are, a kind too rare for the sample to hold).
#[test]
fn a_steady_host_lists_no_event() {
    let system = BinarySystem::xray_binary(galaxy().seed(), xrb_of(XrbHostKind::Supergiant));
    assert_eq!(listing(&system, window_years(-10_000, 0)), []);
}

/// A merger's one-shot is bin 0, number 0 of its tag on its system, at its merger time, with its
/// curve from its marks: a red nova of the pair's total mass, and two neutron stars' kilonova of
/// GW170817's ejecta. A window lists it exactly when it holds the merger, and it is active from the
/// merger. The remnant sets nothing: every merger the class draws leaves a black hole of the pair's
/// mass, which its own tests pin (`catalogue_classes::binary::nsm`).
#[test]
fn a_merger_s_one_shot_is_bin_0_number_0_at_its_merger() {
    let seed = galaxy().seed();
    let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap();
    let red_novae = stellar_mergers().iter().take(8).map(|m| {
        let curve = LuminousRedNovaCurve::of(m.total_mass()).unwrap();
        (
            BinarySystem::stellar_merger(seed, m),
            BinaryEventKind::LuminousRedNova,
            SystemId::from(m.id()),
            m.merger_time(),
            BinaryLightCurve::from(curve),
        )
    });
    let nsm = neutron_star_merger();
    let kilonova = (
        BinarySystem::neutron_star_merger(seed, nsm),
        BinaryEventKind::Kilonova,
        SystemId::from(nsm.id()),
        nsm.merger_time(),
        BinaryLightCurve::from(KilonovaCurve::of(GW170817_EJECTA_MASS).unwrap()),
    );
    for (system, kind, id, at, curve) in red_novae.chain([kilonova]) {
        let events = listing(&system, horizon);
        assert_eq!(events.len(), 1, "{kind:?}");
        let event = events[0];
        assert_eq!(event.kind(), kind);
        assert_eq!(event.onset(), at);
        assert_eq!(event.curve(), &curve);
        assert_eq!(event.id().subject(), EventSubject::System(id));
        assert_eq!(event.id().word().tag(), kind.tag());
        assert_eq!(event.id().word().bin(), EventBin::new(0).unwrap());
        assert_eq!(event.id().word().number(), 0);
        let just_after = at.checked_add(Span::new(0, 1).unwrap()).unwrap();
        assert_eq!(
            listing(&system, TimeWindow::new(at, just_after).unwrap()),
            events
        );
        assert_eq!(
            listing(&system, TimeWindow::new(SourceHorizon::START, at).unwrap()),
            []
        );
        let mut active = Vec::new();
        active_at(&system, at, &mut active);
        assert_eq!(active, events);
    }
}

/// Listing events runs no engine.
/// A black hole that swallows its neutron star whole lists no kilonova, at its merger or ever, and
/// one that tears it apart lists one of its ejecta (P11.T9.b): the fixture's merger with the
/// class's usual pair, a 6.65 M☉ hole at q 5, and with a 3 M☉ hole and a 1.2 M☉ neutron star.
#[test]
fn a_black_hole_that_swallows_its_neutron_star_lists_no_kilonova() {
    let seed = galaxy().seed();
    let horizon = TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap();
    let m = SolarMasses::new;
    let whole = neutron_star_merger().with_black_hole_pair(m(6.65), m(1.33));
    assert_eq!(whole.ejecta_mass().value(), 0.0);
    let system = BinarySystem::neutron_star_merger(seed, &whole);
    assert_eq!(listing(&system, horizon), []);
    let mut active = Vec::new();
    active_at(&system, whole.merger_time(), &mut active);
    assert_eq!(active, []);
    let torn = neutron_star_merger().with_black_hole_pair(m(3.0), m(1.2));
    let events = listing(&BinarySystem::neutron_star_merger(seed, &torn), horizon);
    let [kilonova] = events.as_slice() else {
        panic!("one kilonova: {events:?}");
    };
    assert_eq!(kilonova.kind(), BinaryEventKind::Kilonova);
    assert_eq!(kilonova.onset(), torn.merger_time());
    let curve = KilonovaCurve::of(torn.ejecta_mass()).unwrap();
    assert_eq!(kilonova.curve(), &BinaryLightCurve::from(curve));
}

#[test]
fn listing_events_runs_no_engine() {
    let before = engine_calls();
    for (_, system, window) in systems() {
        let _ = listing(&system, window);
    }
    assert_eq!(engine_calls(), before);
}

/// The clock's origin stream lies past every stream a construction reads: a phase's noise reads
/// event streams 1 to its octaves, at most [`MonotonePhase::MAX_OCTAVES`].
#[test]
fn the_clock_origin_stream_is_its_own() {
    assert!(u32::from(CLOCK_ORIGIN_STREAM) > MonotonePhase::MAX_OCTAVES);
}

// 3. The golden.

/// Each sample system's first events, pinned: kind, ID word, onset, the curve's peak time and
/// luminosity, the duration, the light in V and K and the X-rays at the peak; the mergers' one
/// event each among them.
#[test]
fn binary_events_golden() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let pin = |w: &mut GoldenWriter, name: &str, event: &BinaryEvent| {
        let id = event.id();
        let label = format!("{name} {:?}", event.kind());
        w.line(&format!(
            "{label} subject {:?} bin {} number {} onset {} + {} ns",
            id.subject(),
            id.word().bin().get(),
            id.word().number(),
            event.onset().seconds(),
            event.onset().subsec_nanos()
        ));
        let peak = event.curve().peak();
        let duration = event.duration();
        w.line(&format!(
            "{label} peak at {} + {} ns, duration {} + {} ns",
            peak.seconds(),
            peak.subsec_nanos(),
            duration.seconds(),
            duration.subsec_nanos()
        ));
        w.f64(
            &format!("{label} peak"),
            event.curve().peak_luminosity().value(),
        );
        w.f64(
            &format!("{label} V"),
            light_curve(event, peak, Band::V).value(),
        );
        w.f64(
            &format!("{label} K"),
            light_curve(event, peak, Band::K).value(),
        );
        w.f64(
            &format!("{label} xrays"),
            xray_luminosity(event, peak).value(),
        );
    };
    for (name, system, window) in systems() {
        for event in listing(&system, window).iter().take(6) {
            pin(&mut w, name, event);
        }
    }
    golden!("stellar/binary_events", w.as_str());
}
