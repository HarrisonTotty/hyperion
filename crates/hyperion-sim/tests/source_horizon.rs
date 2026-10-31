//! Plan 12, P12.T0: the source-horizon sweep.
//!
//! Plans 03–11 were written for the clock window ±H with retardation promised, and the brainstorm
//! defines retarded evaluation "back to the start of the source horizon", −(H + L). This sweep
//! evaluates one object of every source that exists at −(H + L), −H, 0 and +H: no panic, a finite
//! position inside the root cube padded by the unbound class's speed over H + L, a defined
//! existence and state, and both event constructions answering for a window at the horizon's
//! start.
//!
//! The sources built so far are the grid's seven layers, the catalogue features' members (plan 09,
//! P09.T21), the dwarf cores' and the streams' members (plan 10, P10.T7.b and P10.T8.b), the
//! supernova entry of plan 09's phase 4 as a value, and the catalogue classes' entries built so
//! far, the core collapses (P09.T33.a), the Type Ia (P09.T34.a), the luminous blue variables
//! (P09.T43), the accreting white dwarfs, fast and slow (P11.T8.a), the X-ray binaries (P11.T8.b),
//! and the stellar and neutron-star mergers (P11.T8.c–d). The displaced remnants (plan 08,
//! P08.T12), the galactic centre's members (P09.T24–T28) and the other catalogue classes (P09.T35)
//! are not placed yet; each is added here by the task that places it (P12.T0 as built).

use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, ROOT_HALF_WIDTH_LY};
use hyperion_sim::events::{
    EventSeries, EventSubject, EventsPerSecond, LinearClock, MonotonePhase, PoissonBins, RateModel,
    TimeWindow, tags,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::binary::{
    AccretingWdFast, AccretingWdSlow, NeutronStarMergers, StellarMergers, XrayBinaries, XrbSide,
};
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::lbv::LbvProcess;
use hyperion_sim::galaxy::catalogue_classes::supernova::{
    SupernovaEntry, SupernovaKind, SupernovaState,
};
use hyperion_sim::galaxy::catalogue_classes::type_ia::TypeIaProcess;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, ClassId, system_of,
};
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
use hyperion_sim::galaxy::features::members::FeatureInterior;
use hyperion_sim::galaxy::global_list::dwarf_core::DwarfCoreModel;
use hyperion_sim::galaxy::global_list::members::{StreamBandShares, stream_cell_members};
use hyperion_sim::galaxy::global_list::tube::TubeCell;
use hyperion_sim::galaxy::global_list::{Debris, TubeTable};
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, Existence, SystemRecord, generate_cell};
use hyperion_sim::galaxy::query::{UNBOUND_PAD_SPEED, position_at};
use hyperion_sim::galaxy::snr::{ExplosionEnergy, SiteGas, shell_window};
use hyperion_sim::id::{Layer, StreamMemberId, SystemId};
use hyperion_sim::observe::{Drift, Observer, TraceMotionError, Trajectory, retarded};
use hyperion_sim::stellar::brief::BriefModel;
use hyperion_sim::stellar::remnant::SupernovaType;
use hyperion_sim::stellar::system::{SystemExistence, SystemStars};
use hyperion_sim::time::{
    CLOCK_WINDOW_H, ClockWindow, LIGHT_CROSSING_L, SourceHorizon, Span, UniverseTime,
};
use hyperion_sim::units::consts::{METRES_PER_LIGHT_YEAR, SPEED_OF_LIGHT};
use hyperion_sim::units::{
    Dex, HydrogenPerCm3, KelvinPerCm3, KilometresPerSecond, LightYears, Seconds,
};

const SEED: u64 = 0x1200_0000_0000_0000;

/// The four times of the sweep: the horizon's start, the window's ends and the epoch.
const TIMES: [UniverseTime; 4] = [
    SourceHorizon::START,
    ClockWindow::START,
    UniverseTime::EPOCH,
    ClockWindow::END,
];

/// How far outside the root cube a source may be at any time of the horizon: the unbound class's
/// speed over H + L, 877 ly.
fn horizon_pad() -> LightYears {
    let beta =
        hyperion_sim::units::MetresPerSecond::from(UNBOUND_PAD_SPEED).value() / SPEED_OF_LIGHT;
    let span = CLOCK_WINDOW_H
        .checked_add(LIGHT_CROSSING_L)
        .expect("H + L fits the clock");
    LightYears::new(beta * span.as_julian_years_f64())
}

/// Asserts that `p` is finite and inside the root cube padded by [`horizon_pad`].
#[track_caller]
fn assert_in_padded_cube(what: &str, p: &GalacticPosition) {
    let limit = f64::from(ROOT_HALF_WIDTH_LY) + horizon_pad().value();
    for c in p.to_light_years_f64() {
        assert!(c.is_finite() && c.abs() <= limit, "{what} at {p:?}");
    }
}

/// A rate model of `per_day` events a day, its own bound.
#[derive(Debug, Clone, Copy)]
struct Daily(f64);

impl RateModel for Daily {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        EventsPerSecond::per_day(self.0)
    }

    fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
        EventsPerSecond::per_day(self.0)
    }
}

/// A grid system's position, existence, stars, brief and retarded reading at every time of the
/// sweep, through plan 03's drift, the observer's [`Drift`], plan 06's `SystemStars` and its range
/// brief.
fn sweep_record(galaxy: &Galaxy, record: &SystemRecord, stars: Option<&SystemStars>) {
    let line = Drift::of_record(galaxy, record).expect("a grid record's line is traced");
    for t in TIMES {
        let drifted = position_at(galaxy, record, t);
        assert_in_padded_cube("plan 03's drift", &drifted);
        let followed = line.position_at(t);
        assert_in_padded_cube("the observer's drift", &followed);
        // The two drifts are the same line. Plan 03's rounds the product v × t, whose last place
        // is 256 m at 250 km/s over H + L and 4 km at the unbound class's 3,000 km/s.
        let gap = drifted.distance_to(&followed).value();
        assert!(gap < 1e4, "the drifts part by {gap} m at {t}");
        let exists = record.existence_at(t) == Existence::Exists;
        assert!(record.age_at(t).value().is_finite());
        if let Some(stars) = stars {
            let summary = stars.summary_at(t);
            assert_eq!(summary.time(), t);
            assert_eq!(summary.existence() == SystemExistence::Exists, exists);
            assert_eq!(
                summary.stars().is_empty(),
                !exists,
                "{:?} at {t}",
                record.id()
            );
            assert_eq!(
                stars.brief_at(t).is_some(),
                exists,
                "{:?} at {t}",
                record.id()
            );
            // The range brief is built for the clock window (P06.T38.e): it must answer at the
            // horizon's start, and inside the window it is the full system's (ruling 90.4).
            let brief = BriefModel::new(galaxy, record);
            let range = brief.brief_at(t);
            if ClockWindow::contains(t) {
                assert_eq!(
                    range.map(|b| b.kind()),
                    stars.brief_at(t).map(|b| b.kind()),
                    "{:?} at {t}",
                    record.id()
                );
            }
        }
    }
    // Seen from 50,000 ly at each end of the window, the light left inside the horizon.
    for t in [ClockWindow::START, ClockWindow::END] {
        let away =
            GalacticPosition::from_light_years([-20_000.0, -20_000.0, 30_000.0]).expect("in range");
        let seen = retarded(&Observer::new(away, t).expect("inside"), &line);
        assert!(SourceHorizon::contains(seen.emitted()));
        assert_in_padded_cube("an apparent position", seen.apparent_position());
    }
}

/// P12.T0: one object of every source that exists, at −(H + L), −H, 0 and +H.
#[test]
#[ignore = "slow: the source-horizon sweep over every source that exists"]
#[expect(
    clippy::too_many_lines,
    reason = "one block per source, in the order the module documentation lists them"
)]
fn source_horizon_sweep_every_source_answers_at_the_horizons_start() {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential();

    // The grid: the first systems of a cell at the solar circle and one in the bulge, every layer.
    let mut cell = Vec::new();
    let mut swept = 0;
    for layer in Layer::ALL {
        let size = f64::from(layer.cell_size_ly());
        for ly in [[0.0, 26_000.0, 0.0], [1_500.0, 300.0, 100.0]] {
            let at = GalacticPosition::from_light_years(ly).expect("in range");
            let key = CellKey::containing(layer, &at).expect("inside the cube");
            generate_cell(&galaxy, key, &mut cell);
            if cell.is_empty() {
                // A sparse layer's cell may be empty; its neighbour along x is tried once.
                let next = GalacticPosition::from_light_years([ly[0] + size, ly[1], ly[2]])
                    .expect("in range");
                generate_cell(
                    &galaxy,
                    CellKey::containing(layer, &next).unwrap(),
                    &mut cell,
                );
            }
            for record in cell.iter().take(3) {
                let stellar = !matches!(layer, Layer::BrownDwarf | Layer::RoguePlanet);
                let stars = stellar.then(|| SystemStars::generate(&galaxy, record));
                sweep_record(&galaxy, record, stars.as_ref());
                swept += 1;
            }
        }
    }
    assert!(swept >= 20, "only {swept} grid systems swept");

    // Layer E's dead: systems already remnants at the epoch, alive or dead at −(H + L).
    let mut dead = 0;
    'cells: for x in -8..8 {
        generate_cell(
            &galaxy,
            CellKey::new(Layer::E, [x, 203, 0]).unwrap(),
            &mut cell,
        );
        for record in &cell {
            let stars = SystemStars::generate(&galaxy, record);
            if stars.brief_at(UniverseTime::EPOCH).is_some_and(|b| {
                matches!(
                    b.kind(),
                    hyperion_sim::stellar::ObjectKind::NeutronStar
                        | hyperion_sim::stellar::ObjectKind::BlackHole
                        | hyperion_sim::stellar::ObjectKind::WhiteDwarf
                )
            }) {
                sweep_record(&galaxy, record, Some(&stars));
                dead += 1;
                if dead == 3 {
                    break 'cells;
                }
            }
        }
    }
    assert!(
        dead >= 1,
        "no remnant among 16 layer-E cells of the solar circle"
    );

    // A catalogue feature's members: an open cluster near the Sun, one member of each band it has.
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    let interior = FeatureCatalogue::near(&galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
        .find_map(|f| FeatureInterior::of(&galaxy, &f))
        .expect("a feature with members lies within 3,000 ly of the Sun");
    let mut members = Vec::new();
    let mut member_swept = 0;
    for band in [MassBand::A, MassBand::C, MassBand::E] {
        let found = interior.grid().owned_cells().find_map(|c| {
            interior.members_in_cell(&galaxy, band, c, &mut members);
            members.first().map(|(m, _)| *m)
        });
        let Some(member) = found else { continue };
        let line = Drift::of_member(&member);
        assert_eq!(Ok(line), Drift::of_record(&galaxy, member.record()));
        let stars = member.stars(&galaxy);
        for t in TIMES {
            assert_in_padded_cube("a member", &line.position_at(t));
            let exists = member.record().existence_at(t) == Existence::Exists;
            let summary = stars.summary_at(t);
            assert_eq!(summary.existence() == SystemExistence::Exists, exists);
            assert_eq!(stars.brief_at(t).is_some(), exists);
        }
        for t in [ClockWindow::START, ClockWindow::END] {
            let away = GalacticPosition::from_light_years([-20_000.0, -20_000.0, 30_000.0])
                .expect("in range");
            let seen = retarded(&Observer::new(away, t).expect("inside"), &line);
            assert!(SourceHorizon::contains(seen.emitted()));
            assert_in_padded_cube("a member's apparent position", seen.apparent_position());
        }
        member_swept += 1;
    }
    assert!(
        member_swept >= 1,
        "the feature has no member in bands A, C or E"
    );

    // A dwarf core's members (plan 10, P10.T7.b): one of each band its cells have, on the line
    // its own velocity gives it, which the system record alone cannot.
    let debris = Debris::generate(&galaxy);
    let core = debris
        .cores()
        .iter()
        .map(|spec| DwarfCoreModel::from_spec(&galaxy, spec))
        .find(|core| core.check_index_headroom().is_ok())
        .expect("the fixture keeps a core under its index");
    let mut core_members = Vec::new();
    let mut core_swept = 0;
    for band in [MassBand::A, MassBand::C, MassBand::E] {
        let found = core.grid().owned_cells().find_map(|c| {
            core.members_in_cell(&galaxy, band, c, &mut core_members);
            core_members.first().map(|(m, _)| *m)
        });
        let Some(member) = found else { continue };
        let line = Drift::of_global_member(&member);
        assert_eq!(
            Drift::of_record(&galaxy, member.record()),
            Err(TraceMotionError::GlobalListMember(member.record().id()))
        );
        let stars = member.stars(&galaxy);
        for t in TIMES {
            assert_in_padded_cube("a core member", &line.position_at(t));
            let exists = member.record().existence_at(t) == Existence::Exists;
            let summary = stars.summary_at(t);
            assert_eq!(summary.existence() == SystemExistence::Exists, exists);
            assert_eq!(stars.brief_at(t).is_some(), exists);
        }
        for t in [ClockWindow::START, ClockWindow::END] {
            let away = GalacticPosition::from_light_years([-20_000.0, -20_000.0, 30_000.0])
                .expect("in range");
            let seen = retarded(&Observer::new(away, t).expect("inside"), &line);
            assert!(SourceHorizon::contains(seen.emitted()));
            assert_in_padded_cube(
                "a core member's apparent position",
                seen.apparent_position(),
            );
        }
        core_swept += 1;
    }
    assert!(core_swept >= 1, "the core has no member in bands A, C or E");

    // A stream's members (plan 10, P10.T8.b): the cheapest stream to spray whose progenitor lies
    // well inside the root cube, one member of each band its axis cells have.
    let spec = debris
        .streams()
        .iter()
        .filter(|s| {
            let m = s.position().metres();
            (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt() < 4e4 * METRES_PER_LIGHT_YEAR
        })
        .min_by_key(|s| {
            s.orbit()
                .fixed_step(Seconds::from(s.stripping_time()))
                .expect("a finite stripping time")
                .count()
        })
        .expect("a stream inside 40,000 ly");
    let tube = TubeTable::build(&galaxy, spec).expect("the full potential");
    let shares = StreamBandShares::of(&galaxy, spec);
    let mut stream_members = Vec::new();
    let mut stream_swept = 0;
    for band in [MassBand::A, MassBand::C] {
        let found = (0..tube.along_cells()).find_map(|along| {
            let id = StreamMemberId::new(
                spec.number().get(),
                band.layer(),
                u16::try_from(along).expect("at most 16,384 cells"),
                [32, 32],
                0,
            )
            .expect("a cell's ID");
            stream_cell_members(
                &galaxy,
                &tube,
                &shares,
                TubeCell::from(id),
                band,
                &mut stream_members,
            );
            stream_members.first().copied()
        });
        let Some(member) = found else { continue };
        let line = Drift::of_global_member(&member);
        let stars = member.stars(&galaxy);
        for t in TIMES {
            assert_in_padded_cube("a stream member", &line.position_at(t));
            let exists = member.record().existence_at(t) == Existence::Exists;
            assert_eq!(
                stars.summary_at(t).existence() == SystemExistence::Exists,
                exists
            );
            assert_eq!(stars.brief_at(t).is_some(), exists);
        }
        for t in [ClockWindow::START, ClockWindow::END] {
            let away = GalacticPosition::from_light_years([-20_000.0, -20_000.0, 30_000.0])
                .expect("in range");
            let seen = retarded(&Observer::new(away, t).expect("inside"), &line);
            assert!(SourceHorizon::contains(seen.emitted()));
            assert_in_padded_cube(
                "a stream member's apparent position",
                seen.apparent_position(),
            );
        }
        stream_swept += 1;
    }
    assert!(
        stream_swept >= 1,
        "the stream has no member in bands A or C on its axis"
    );

    // The supernova entry as a value: exploding at the horizon's start and at the epoch.
    let site = SiteGas::uniform(HydrogenPerCm3::new(1.0), KelvinPerCm3::new(3_800.0))
        .expect("a uniform site");
    let window = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
    let system = SystemId::from_raw(0xF000_0007_0000_0000).expect("a catalogue ID");
    let year = Span::from_julian_years(1).expect("in range");
    for explosion in [
        SourceHorizon::START.checked_add(year).expect("in range"),
        UniverseTime::EPOCH,
    ] {
        let entry = SupernovaEntry::new(
            system,
            explosion,
            SupernovaKind::CoreCollapse(SupernovaType::IIP),
            window,
        )
        .with_remnant(KilometresPerSecond::new(400.0), None);
        for t in TIMES {
            let state = entry.state_at(t);
            match state {
                SupernovaState::Progenitor {
                    until_explosion, ..
                } => {
                    assert!(t < explosion);
                    assert_eq!(t.checked_add(until_explosion), Some(explosion));
                }
                SupernovaState::Supernova { age, remnant, .. }
                | SupernovaState::BareRemnant { age, remnant } => {
                    assert!(t >= explosion && age.value().is_finite());
                    assert!(remnant.offset().is_some_and(|o| o.value().is_finite()));
                }
            }
        }
    }

    // A core-collapse entry of the class grid (plan 09, P09.T33.a): its system on its line broken
    // at the explosion, its record's existence, its stars and its supernova's state. Its line is
    // not one, so the observer's drift refuses it, as it says.
    let process = CoreCollapseProcess::new(&galaxy);
    let entry = (8..28)
        .map(|y| {
            CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, y, 0])
                .expect("a cell of the root cube")
        })
        .flat_map(|key| CatalogueClassCell::generate(&galaxy, &process, key).into_entries())
        .next()
        .expect("the inner disc has recent deaths");
    let catalogue = system_of(&galaxy, entry.id()).expect("an entry resolves");
    let record = catalogue.record();
    let stars = catalogue
        .stars(&galaxy)
        .expect("a core collapse's stars are the stellar stage's");
    assert!(matches!(
        Drift::of_record(&galaxy, record),
        Err(TraceMotionError::CatalogueMotionNotTraced(_))
    ));
    for t in TIMES {
        let p = catalogue.position_at(t);
        assert_in_padded_cube("a core-collapse entry", &p);
        assert_eq!(position_at(&galaxy, record, t), p);
        let exists = record.existence_at(t) == Existence::Exists;
        assert!(record.age_at(t).value().is_finite());
        let summary = stars.summary_at(t);
        assert_eq!(summary.existence() == SystemExistence::Exists, exists);
        assert_eq!(
            stars.brief_at(t).is_some(),
            exists,
            "{:?} at {t}",
            record.id()
        );
        match entry.value().supernova().state_at(t) {
            SupernovaState::Progenitor { .. } => assert!(t < entry.value().site().death()),
            SupernovaState::Supernova { age, .. } | SupernovaState::BareRemnant { age, .. } => {
                assert!(t >= entry.value().site().death() && age.value().is_finite());
            }
        }
    }

    // A luminous-blue-variable host of the class grid (plan 09, P09.T43): its system on its
    // straight line, its record's existence, and its stars, whose window is the entry's. Its line
    // is a catalogue system's, so the observer's drift refuses it, as it says.
    let process = LbvProcess::new(&galaxy);
    let entry = (3..8)
        .flat_map(|y| [[0, y, 0], [0, y, -1]])
        .map(|cell| {
            CatalogueCellKey::new(ClassId::LUMINOUS_BLUE_VARIABLE, cell)
                .expect("a cell of the root cube")
        })
        .flat_map(|key| CatalogueClassCell::generate(&galaxy, &process, key).into_entries())
        .next()
        .expect("the disc has luminous-blue-variable hosts");
    let catalogue = system_of(&galaxy, entry.id()).expect("an entry resolves");
    let record = catalogue.record();
    let stars = catalogue
        .stars(&galaxy)
        .expect("a host's stars are the stellar stage's");
    assert_eq!(stars.lbv_window(), Some(entry.value().window()));
    assert!(matches!(
        Drift::of_record(&galaxy, record),
        Err(TraceMotionError::CatalogueMotionNotTraced(_))
    ));
    for t in TIMES {
        let p = catalogue.position_at(t);
        assert_in_padded_cube("a luminous-blue-variable host", &p);
        assert_eq!(position_at(&galaxy, record, t), p);
        let exists = record.existence_at(t) == Existence::Exists;
        assert!(record.age_at(t).value().is_finite());
        let summary = stars.summary_at(t);
        assert_eq!(summary.existence() == SystemExistence::Exists, exists);
        assert_eq!(
            stars.brief_at(t).is_some(),
            exists,
            "{:?} at {t}",
            record.id()
        );
    }

    // A Type Ia entry of the class grid (plan 09, P09.T34.a), one with a survivor: each member on
    // its line, the system's straight and the survivor's broken at the explosion, its record's
    // existence and its supernova's state. Neither member has stars of the stellar stage's, and the
    // observer's drift refuses both lines.
    let process = TypeIaProcess::new(&galaxy);
    let entry = [[0, 0, 0], [-1, -1, -1], [0, 0, -1], [-1, 0, 0]]
        .into_iter()
        .map(|cell| CatalogueCellKey::new(ClassId::TYPE_IA, cell).expect("a cell about the centre"))
        .flat_map(|key| CatalogueClassCell::generate(&galaxy, &process, key).into_entries())
        .find(|entry| entry.members() == 2)
        .expect("the centre has a recent Type Ia with a survivor");
    for member in 0..entry.members() {
        let id = entry.member_id(member).expect("a member of the entry");
        let catalogue = system_of(&galaxy, id).expect("a member resolves");
        let record = catalogue.record();
        assert!(catalogue.stars(&galaxy).is_none());
        assert!(matches!(
            Drift::of_record(&galaxy, record),
            Err(TraceMotionError::CatalogueMotionNotTraced(_))
        ));
        for t in TIMES {
            let p = catalogue.position_at(t);
            assert_in_padded_cube("a Type Ia's member", &p);
            assert_eq!(position_at(&galaxy, record, t), p);
            assert!(record.age_at(t).value().is_finite());
            match entry.value().supernova().state_at(t) {
                SupernovaState::Progenitor { .. } => assert!(t < entry.value().site().death()),
                SupernovaState::Supernova { age, remnant, .. }
                | SupernovaState::BareRemnant { age, remnant } => {
                    assert!(t >= entry.value().site().death() && age.value().is_finite());
                    assert!(
                        remnant
                            .survivor_offset()
                            .is_some_and(|offset| offset.value().is_finite())
                    );
                }
            }
        }
    }

    // An accreting white dwarf of each class of the class grid (plan 11, P11.T8.a): its system on
    // its one straight line, its record's existence, its stars (its pair, from its marks) and its
    // novae, the series its scan marks carry, for a window at the horizon's start.
    let fast = AccretingWdFast::new(&galaxy);
    let slow = AccretingWdSlow::new(&galaxy);
    let first_entry = |class: ClassId| {
        (-4..4).find_map(|x| {
            let key = CatalogueCellKey::new(class, [x, 50, 0]).expect("a cell of the root cube");
            let entries = if class == ClassId::ACCRETING_WHITE_DWARF {
                CatalogueClassCell::generate(&galaxy, &fast, key).into_entries()
            } else {
                CatalogueClassCell::generate(&galaxy, &slow, key).into_entries()
            };
            entries.into_iter().next()
        })
    };
    for class in [
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
    ] {
        let entry = first_entry(class).expect("the solar circle has accreting white dwarfs");
        let catalogue = system_of(&galaxy, entry.id()).expect("an entry resolves");
        let record = catalogue.record();
        let stars = catalogue
            .stars(&galaxy)
            .expect("an accreting white dwarf's stars are its pair's");
        for t in TIMES {
            let p = catalogue.position_at(t);
            assert_in_padded_cube("an accreting white dwarf", &p);
            assert_eq!(position_at(&galaxy, record, t), p);
            assert_eq!(record.existence_at(t), Existence::Exists);
            assert!(record.age_at(t).value().is_finite());
            assert_eq!(stars.summary_at(t).existence(), SystemExistence::Exists);
            assert!(stars.brief_at(t).is_some(), "{:?} at {t}", record.id());
            assert!(stars.state_at(t).is_some_and(|s| s.pairs().len() == 1));
        }
        let opening = TimeWindow::new(
            SourceHorizon::START,
            SourceHorizon::START.checked_add(year).expect("in range"),
        )
        .expect("ordered");
        let mut novae = Vec::new();
        entry.value().scan().eruptions_in(opening, &mut novae);
        assert!(novae.iter().all(|e| opening.contains(e.time())));
    }

    // A stellar merger of the class grid (plan 11, P11.T8.c): its system on its one straight line,
    // its record's existence, and its stars, the contact pair before its merger time and the
    // merged star from it, under the one ID.
    let mergers = StellarMergers::new(&galaxy);
    let stellar = (-2..2)
        .find_map(|x| {
            let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, 6, 0])
                .expect("a cell of the root cube");
            CatalogueClassCell::generate(&galaxy, &mergers, key)
                .into_entries()
                .into_iter()
                .next()
        })
        .expect("the solar circle has stellar mergers");
    // A neutron-star merger, the first of the galaxy's, walked cell by cell from the centre's.
    let neutron_stars = NeutronStarMergers::new(&galaxy);
    let compact = (0..16)
        .flat_map(|r: i32| {
            (-r..r.max(1)).flat_map(move |x| {
                (-r..r.max(1)).flat_map(move |y| (-1..1).map(move |z| [x, y, z]))
            })
        })
        .find_map(|cell| {
            let key = CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, cell)
                .expect("a cell of the root cube");
            CatalogueClassCell::generate(&galaxy, &neutron_stars, key)
                .into_entries()
                .into_iter()
                .next()
        })
        .expect("the galaxy has a neutron-star merger in the disc");
    for (id, merger, what) in [
        (
            stellar.id(),
            stellar.value().scan().merger_time(),
            "a stellar merger",
        ),
        (
            compact.id(),
            compact.value().scan().merger_time(),
            "a neutron-star merger",
        ),
    ] {
        let catalogue = system_of(&galaxy, id).expect("an entry resolves");
        let record = catalogue.record();
        let stars = catalogue
            .stars(&galaxy)
            .expect("a merger's stars are its pair's");
        for t in TIMES {
            let p = catalogue.position_at(t);
            assert_in_padded_cube(what, &p);
            assert_eq!(position_at(&galaxy, record, t), p);
            assert_eq!(record.existence_at(t), Existence::Exists);
            assert!(record.age_at(t).value().is_finite());
            assert_eq!(stars.summary_at(t).existence(), SystemExistence::Exists);
            assert!(stars.brief_at(t).is_some(), "{:?} at {t}", record.id());
            let state = stars.state_at(t).expect("the system exists");
            let present = state
                .stars()
                .iter()
                .filter(|s| s.phase() != hyperion_sim::stellar::Phase::NoRemnant)
                .count();
            assert_eq!(present, if t < merger { 2 } else { 1 }, "{what} at {t}");
        }
    }

    // An X-ray binary of the class grid (plan 11, P11.T8.b), a low-mass host from the cells about
    // the centre and a high-mass one from the inner disc: its system on its one straight line, its
    // record's existence and its stars (its pair, from its marks).
    let xrb = XrayBinaries::new(&galaxy);
    let xrb_entry = |cells: Vec<[i32; 3]>, side: XrbSide| {
        cells.into_iter().find_map(|cell| {
            let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, cell).expect("a cell");
            CatalogueClassCell::generate(&galaxy, &xrb, key)
                .into_entries()
                .into_iter()
                .find(|e| XrbSide::of(e.value().record().population()) == side)
        })
    };
    let block = |xs: std::ops::Range<i32>, ys: std::ops::Range<i32>| -> Vec<[i32; 3]> {
        xs.flat_map(|x| ys.clone().map(move |y| [x, y, 0]))
            .collect()
    };
    let low = xrb_entry(block(-4..4, -4..4), XrbSide::LowMass)
        .expect("the centre has low-mass X-ray binaries");
    let high = xrb_entry(block(-12..12, 10..14), XrbSide::HighMass)
        .expect("the inner disc has high-mass X-ray binaries");
    for entry in [low, high] {
        let catalogue = system_of(&galaxy, entry.id()).expect("an entry resolves");
        let record = catalogue.record();
        let stars = catalogue
            .stars(&galaxy)
            .expect("an X-ray binary's stars are its pair's");
        for t in TIMES {
            let p = catalogue.position_at(t);
            assert_in_padded_cube("an X-ray binary", &p);
            assert_eq!(position_at(&galaxy, record, t), p);
            assert_eq!(entry.value().scan().position_at(t), p);
            assert_eq!(record.existence_at(t), Existence::Exists);
            assert!(record.age_at(t).value().is_finite());
            assert_eq!(stars.summary_at(t).existence(), SystemExistence::Exists);
            assert!(stars.brief_at(t).is_some(), "{:?} at {t}", record.id());
            assert!(stars.state_at(t).is_some_and(|s| s.pairs().len() == 1));
        }
    }

    // Both event constructions answer for a window at the horizon's start.
    let first = TimeWindow::new(
        SourceHorizon::START,
        SourceHorizon::START.checked_add(year).expect("in range"),
    )
    .expect("ordered");
    let subject = EventSubject::System(system);
    let flares = EventSeries::new(Seed::new(SEED), tags::STAR_FLARE, subject);
    let bins = PoissonBins::new(86_400, 2).expect("a day's bins");
    let mut events = Vec::new();
    bins.events_in(&flares, first, &Daily(1.0), &mut events);
    assert!(
        events.len() > 300,
        "{} flares in the horizon's first year",
        events.len()
    );
    assert!(events.iter().all(|e| first.contains(e.time())));
    let mut active = Vec::new();
    bins.active_at(&flares, SourceHorizon::START, &Daily(1.0), &mut active);
    assert!(active.iter().all(|e| e.time() <= SourceHorizon::START));
    let cycles = EventSeries::new(Seed::new(SEED), tags::STAR_VARIABILITY_CYCLE, subject);
    let clock =
        LinearClock::new(Span::from_seconds(86_400), SourceHorizon::START).expect("a daily clock");
    let phase = MonotonePhase::new(0.3, 4, 12).expect("a bounded noise");
    let mut cycle_events = Vec::new();
    phase.events_in(&cycles, &clock, first, &mut cycle_events);
    assert!(
        (300..=430).contains(&cycle_events.len()),
        "{} daily cycles in the horizon's first year",
        cycle_events.len()
    );
    assert!(cycle_events.iter().all(|e| first.contains(e.time())));
}
