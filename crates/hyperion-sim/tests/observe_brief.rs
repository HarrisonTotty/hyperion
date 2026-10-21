//! Plan 12, P12.T2, and plan 11, P11.T11.a: an observed row and the observed summary read one state
//! of a system at one emitted time, and a range row in now mode and the summary at one clock time,
//! whatever its primary's companion has made of it.
//!
//! The landing look of P12.T11 (look-72) found GDG 0E0V00 E-141 of the universe LOOK72 read as a
//! single star's red supergiant in its observed range row and as the hot subdwarf its companion
//! had stripped it to in its observed summary, at the same emitted time. The row read the
//! primary's single-star track and the summary its pair's timeline (`SystemStars::summary_at`);
//! the row now reads the summary's primary, `SystemStars::brief_at`, the one brief of a system
//! (P11.T11.a), and the now-mode row's model (`BriefModel`) reads it wherever its pair can have
//! changed the primary. These tests build the galaxy as the server does, with its full potential,
//! so that each system moves on its own line.

use std::sync::OnceLock;

use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticDisplacement, GalacticPosition};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell, resolve};
use hyperion_sim::galaxy::query::{SystemHit, position_at};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::observe::{Drift, Observer, Trajectory, observe_hit, summary_observed};
use hyperion_sim::stellar::binary::{Component, Segment, SegmentKind};
use hyperion_sim::stellar::brief::{BriefModel, BriefRoute};
use hyperion_sim::stellar::multiplicity::{MultiplicityContext, StarIndex};
use hyperion_sim::stellar::system::{StarSummary, StellarBrief, SystemStars};
use hyperion_sim::stellar::{ObjectKind, Phase};
use hyperion_sim::time::{ClockWindow, Span, UniverseTime};
use hyperion_sim::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};
use hyperion_sim::units::{LightYears, Years};
use hyperion_testkit::float::bits;

/// The seed of look-72's universe, LOOK72.
const LOOK72: u64 = 0xa49f_649b_ef43_4b20;

/// GDG 0E0V00 E-141: a 10.96 M☉ primary that a 6.38 M☉ companion strips in a common envelope at
/// 20.195 Myr, and a neutron star at the epoch.
const STRIPPED: &str = "820e6ee00000008d";

/// A pinned case-A donor of LOOK72 whose companion strips it on its main sequence, and which
/// collapses there at its pin (`decision-observed-class-findings.md` §B).
const CASE_A: &str = "822c72e00000008c";

/// LOOK72's galaxy, as the server builds it ([`Galaxy::with_full_potential`]), once per process.
///
/// The build takes some 5 s in a test build, and nextest runs each test in a process of its own,
/// so the properties run in one test, [`observed_rows_and_summaries_read_one_state`].
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(LOOK72)).with_full_potential())
}

/// An observed row and the observed summary read one state (look-72), each property below on one
/// build of LOOK72's galaxy ([`galaxy`]), in turn.
#[test]
fn observed_rows_and_summaries_read_one_state() {
    look72_stripped_primary_is_the_stripped_star_in_the_row_and_the_summary();
    stripped_primaries_read_one_state_before_during_and_after_stripping();
    look72_now_rows_read_the_pair();
}

/// The brief at `t` of `record`'s primary as its single-star self: the system of its primary
/// alone, which is plan 06's single-star system, as every row read it before P11.T11.a.
fn single_star_brief(record: &SystemRecord, t: UniverseTime) -> Option<StellarBrief> {
    SystemStars::generate_in(galaxy(), record, MultiplicityContext::ForcedSingle).brief_at(t)
}

/// `id`'s record in LOOK72.
fn record_of(id: &str) -> SystemRecord {
    let id: SystemId = id.parse().expect("a system ID");
    resolve(galaxy(), id).expect("a grid system of LOOK72")
}

/// What `observer` reads of `record`'s system, whose stars are `stars`, at one emitted time: the
/// time, the range row's brief then ([`observe_hit`]) and the summary's primary then
/// ([`summary_observed`]).
fn row_and_summary(
    record: &SystemRecord,
    stars: &SystemStars,
    observer: &Observer,
) -> (UniverseTime, Option<StellarBrief>, Option<StarSummary>) {
    let galaxy = galaxy();
    let position = position_at(galaxy, record, observer.time());
    let distance = LightYears::from(observer.position().distance_to(&position));
    let hit = SystemHit::new(*record, position, distance);
    let seen = observe_hit(galaxy, &hit, stars, observer).expect("a grid system's line");
    let (summary, retardation) =
        summary_observed(galaxy, record, stars, observer).expect("a grid system's line");
    assert_eq!(seen.retardation(), &retardation, "one emitted time");
    (
        retardation.emitted(),
        seen.brief_then(),
        summary.stars().first().copied(),
    )
}

/// The row's `brief` and the summary's `primary` are one state: the same kind and class, the
/// same luminosity and temperature to the bit, and the system's kept star count.
fn assert_one_state(
    brief: Option<StellarBrief>,
    primary: Option<&StarSummary>,
    stars: &SystemStars,
    what: &str,
) {
    let (Some(brief), Some(primary)) = (brief, primary) else {
        assert!(
            brief.is_none() && primary.is_none(),
            "{what}: a brief {brief:?} with a primary {primary:?}"
        );
        return;
    };
    assert_eq!(
        (brief.kind(), brief.class()),
        (primary.kind(), primary.classification()),
        "{what}"
    );
    let state = primary.state();
    assert_eq!(
        bits(brief.effective_temperature().value()),
        bits(state.effective_temperature().value()),
        "{what}: T_eff"
    );
    let luminosity = state.luminosity().value();
    match brief.log_luminosity() {
        Some(log) => assert_eq!(
            bits(log.value()),
            bits(math::log10(luminosity)),
            "{what}: log L"
        ),
        None => assert!(luminosity <= 0.0, "{what}: L = {luminosity} L☉ has no log"),
    }
    assert_eq!(brief.star_count(), stars.star_count(), "{what}");
}

/// An observer at the epoch, as many light-years along +z from where `record`'s system was at
/// `emitted` as `emitted` is years before the epoch: the light it receives left the system about
/// `emitted`.
fn observer_of(record: &SystemRecord, emitted: UniverseTime) -> Observer {
    let site = Drift::of_record(galaxy(), record)
        .expect("a grid system's line")
        .position_at(emitted);
    let light_years = -emitted.since_epoch().as_julian_years_f64();
    let place = site
        .translated(GalacticDisplacement::new([
            0.0,
            0.0,
            light_years * METRES_PER_LIGHT_YEAR,
        ]))
        .expect("inside the root cube");
    Observer::new(place, UniverseTime::EPOCH).expect("inside the root cube and the clock window")
}

/// look-72's case: GDG 0E0V00 E-141 seen from 20,000 ly at the epoch. The light left 2.5 Myr after
/// its companion stripped it in a common envelope, so both the row and the summary show the
/// stripped helium star, of 2–4 M☉ as stripping leaves an 11 M☉ primary (Götberg et al. 2018, A&A
/// 615, A78; Laplace et al. 2020, A&A 637, A6), where its single-star track is a red supergiant
/// then. Which kind the stripped star is called is the classifier's, and the test leaves it to
/// the row and the summary agreeing.
///
/// Run on its one build of the galaxy by [`observed_rows_and_summaries_read_one_state`].
fn look72_stripped_primary_is_the_stripped_star_in_the_row_and_the_summary() {
    let galaxy = galaxy();
    let record = record_of(STRIPPED);
    let stars = SystemStars::generate(galaxy, &record);
    // look-72's observer: 20,000 ly along +x of where the system is at the epoch.
    let [x, y, z] = position_at(galaxy, &record, UniverseTime::EPOCH).to_light_years_f64();
    let observer = Observer::new(
        GalacticPosition::from_light_years([x + 20_000.0, y, z]).expect("in range"),
        UniverseTime::EPOCH,
    )
    .expect("inside the root cube and the clock window");
    let (emitted, brief, primary) = row_and_summary(&record, &stars, &observer);
    let light_age = UniverseTime::EPOCH
        .checked_since(emitted)
        .expect("the light left before the epoch")
        .as_julian_years_f64();
    // 20,000 years, give or take its drift along the line of sight (60 years is 0.3 per cent of c).
    assert!((light_age - 20_000.0).abs() < 60.0, "{light_age} yr");
    let primary = primary.expect("the system was born then");
    assert_eq!(primary.state().phase(), Phase::HeliumMainSequence);
    let mass = primary.state().mass().value();
    assert!((2.0..4.0).contains(&mass), "a stripped star of {mass} M☉");
    assert_one_state(brief, Some(&primary), &stars, STRIPPED);
    assert!(
        brief.is_some_and(|b| b.kind() != ObjectKind::Supergiant),
        "{brief:?}"
    );
    // Its single-star track is a red supergiant then, which the row read before the fix.
    let own = stars.primary().state_at(emitted).expect("formed");
    assert_eq!(own.phase(), Phase::EarlyAgb);
    assert_eq!(
        single_star_brief(&record, emitted).map(|b| (b.kind(), b.class().to_string())),
        Some((ObjectKind::Supergiant, "M6.5Ib".to_owned()))
    );
    // At the epoch it is a neutron star either way.
    assert_eq!(
        stars.brief_at(UniverseTime::EPOCH).map(|b| b.kind()),
        Some(ObjectKind::NeutronStar)
    );
}

/// When `stars`' primary begins to lose its envelope to its companion and when that ends, its ages
/// then: the start of the first segment of the primary's pair in which it donates mass by
/// Roche-lobe overflow or a common envelope ejects an envelope, and the start of the next segment
/// that does neither.
fn stripping(stars: &SystemStars) -> Option<(Years, Years)> {
    let pair = stars
        .pairs()
        .iter()
        .find(|pair| pair.stars()[0] == StarIndex::PRIMARY)?;
    let strips = |kind| {
        matches!(
            kind,
            SegmentKind::StableTransfer {
                donor: Component::Primary
            } | SegmentKind::CommonEnvelope
        )
    };
    let segments = pair.timeline().segments();
    let first = segments.iter().position(|s| strips(s.kind()))?;
    let end = segments[first..]
        .iter()
        .find(|s| !strips(s.kind()))
        .map_or(pair.timeline().until(), Segment::start);
    Some((segments[first].start(), end))
}

/// The clock time at which a star whose age at the epoch is `age_at_epoch` is `age` old.
fn clock_at(age_at_epoch: Years, age: Years) -> UniverseTime {
    let span =
        Span::from_seconds_f64((age.value() - age_at_epoch.value()) * SECONDS_PER_JULIAN_YEAR)
            .expect("inside the clock's range");
    UniverseTime::EPOCH
        .checked_add(span)
        .expect("inside the clock's range")
}

/// Before, during and after a companion strips the primary, the row and the summary read one
/// state: systems of layer E near GDG 0E0V00 E-141 whose primary its pair strips, each re-aged so
/// that its stripping begins 40,000 years before the epoch (a real record with only its age at the
/// epoch changed, which moves its history on the clock and nothing else), and observed at the
/// epoch from where the light that left 20,000 years before, during and 20,000 years after its
/// stripping arrives.
///
/// Run on its one build of the galaxy by [`observed_rows_and_summaries_read_one_state`].
fn stripped_primaries_read_one_state_before_during_and_after_stripping() {
    // Years from the start of the stripping to the epoch.
    const LEAD_YR: f64 = 40_000.0;
    let galaxy = galaxy();
    let home = record_of(STRIPPED);
    let [x, y, z] = home.epoch_position().to_light_years_f64();
    let mut cell = Vec::new();
    let mut systems = 0;
    let mut changed = 0;
    let mut helium = 0;
    for k in -20..20 {
        let position =
            GalacticPosition::from_light_years([x + 128.0 * f64::from(k), y, z]).expect("in range");
        let key = CellKey::containing(Layer::E, &position).expect("inside the root cube");
        generate_cell(galaxy, key, &mut cell);
        for grown in &cell {
            let Some((first, _)) = stripping(&SystemStars::generate(galaxy, grown)) else {
                continue;
            };
            let record = SystemRecord::from_parts(
                grown.id(),
                *grown.epoch_position(),
                grown.origin(),
                grown.population(),
                grown.primary_initial_mass(),
                Years::new(first.value() + LEAD_YR),
            );
            let stars = SystemStars::generate(galaxy, &record);
            // The re-aged pair runs to a later age; its history up to the old one is the same.
            let Some((onset_yr, end_yr)) = stripping(&stars)
                .filter(|(onset, _)| (onset.value() - first.value()).abs() < 1_000.0)
                .map(|(onset, end)| (onset.value(), end.value()))
            else {
                continue;
            };
            let age = record.age_at_epoch();
            let during_yr = onset_yr + (0.5 * (end_yr - onset_yr)).min(LEAD_YR / 2.0);
            let after_yr = (end_yr + 20_000.0).min(age.value() - 1_000.0);
            for (when, at_yr) in [
                ("before", onset_yr - 20_000.0),
                ("during", during_yr),
                ("after", after_yr),
            ] {
                let observer = observer_of(&record, clock_at(age, Years::new(at_yr)));
                let (emitted, brief, primary) = row_and_summary(&record, &stars, &observer);
                let what = format!("{} {when} its stripping, at {at_yr:.0} yr", record.id());
                assert_one_state(brief, primary.as_ref(), &stars, &what);
                if when == "after" {
                    let single = single_star_brief(&record, emitted).map(|b| (b.kind(), b.class()));
                    changed += usize::from(single != brief.map(|b| (b.kind(), b.class())));
                    helium += usize::from(primary.is_some_and(|p| {
                        matches!(
                            p.state().phase(),
                            Phase::HeliumMainSequence
                                | Phase::HeliumHertzsprungGap
                                | Phase::HeliumGiantBranch
                        )
                    }));
                }
            }
            systems += 1;
            if systems == 6 {
                break;
            }
        }
        if systems == 6 {
            break;
        }
    }
    assert_eq!(
        systems, 6,
        "six stripped primaries within 2,560 ly of look-72's"
    );
    // The rows read the pair where the single-star track would have said otherwise.
    assert!(changed > 0, "no row reads a primary its pair has changed");
    assert!(helium > 0, "no primary is a stripped helium star after");
}

/// Whether `stars`' pair has changed its primary at `t`: the summary's primary then is not its own
/// model's state.
fn pair_has_changed_primary(stars: &SystemStars, t: UniverseTime) -> bool {
    let own = stars.primary().state_at(t);
    let summary = stars.summary_at(t);
    summary.stars().first().map(StarSummary::state) != own.as_ref()
}

/// P11.T11.a: the now-mode rows of look-72's two systems read the summary's primary. Each of GDG
/// 0E0V00 E-141 (820E6EE00000008D, stripped in a common envelope) and 822C72E00000008C (a case-A
/// donor that collapses on its main sequence) is re-aged so that an age at which its pair has
/// changed its primary, the first such at the start or middle of a segment of its pair's
/// timeline, falls at the epoch, a real record with only its age at the epoch changed. Its range
/// row's model takes the paired route, and the row at both ends of the clock window and at the
/// epoch is the summary's primary then, with the first attempt's star count. At the epoch the
/// stripped primary's row is not the class its single-star track has then, which the row read
/// before P11.T11.a.
///
/// Run on its one build of the galaxy by [`observed_rows_and_summaries_read_one_state`].
fn look72_now_rows_read_the_pair() {
    let galaxy = galaxy();
    for id in [STRIPPED, CASE_A] {
        let grown = record_of(id);
        let stars = SystemStars::generate(galaxy, &grown);
        let pair = stars
            .pairs()
            .iter()
            .find(|pair| pair.stars()[0] == StarIndex::PRIMARY)
            .unwrap_or_else(|| panic!("{id}: the engine ran the primary's pair"));
        let age = grown.age_at_epoch();
        let segments = pair.timeline().segments();
        let candidates: Vec<f64> = segments
            .iter()
            .zip(
                segments
                    .iter()
                    .skip(1)
                    .map(Segment::start)
                    .chain([pair.timeline().until()]),
            )
            .flat_map(|(segment, end)| {
                let onset = segment.start().value();
                [onset + 1.0, f64::midpoint(onset, end.value())]
            })
            .filter(|&at| {
                at < age.value() && pair_has_changed_primary(&stars, clock_at(age, Years::new(at)))
            })
            .collect();
        // The first age at which the pair has made the primary another class than its single-star
        // track's, if there is one, and otherwise the first at which it has changed it at all.
        let reclassed = |at: f64| {
            let t = clock_at(age, Years::new(at));
            let summary = stars.summary_at(t);
            let paired = summary
                .stars()
                .first()
                .map(|p| (p.kind(), p.classification()));
            paired != single_star_brief(&grown, t).map(|b| (b.kind(), b.class()))
        };
        let changed = candidates
            .iter()
            .copied()
            .find(|&at| reclassed(at))
            .or_else(|| candidates.first().copied())
            .unwrap_or_else(|| panic!("{id}: its pair never changes its primary"));
        let record = SystemRecord::from_parts(
            grown.id(),
            *grown.epoch_position(),
            grown.origin(),
            grown.population(),
            grown.primary_initial_mass(),
            Years::new(changed),
        );
        let stars = SystemStars::generate(galaxy, &record);
        assert!(
            pair_has_changed_primary(&stars, UniverseTime::EPOCH),
            "{id} re-aged to {changed} yr"
        );
        let model = BriefModel::new(galaxy, &record);
        assert_eq!(model.route(), BriefRoute::Paired, "{id}");
        assert_eq!(model.star_count(), stars.star_count(), "{id}: not redrawn");
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            let summary = stars.summary_at(t);
            let what = format!("{id} re-aged to {changed:.0} yr, at {t}");
            assert_one_state(model.brief_at(t), summary.stars().first(), &stars, &what);
        }
        if id == STRIPPED {
            let row = model
                .brief_at(UniverseTime::EPOCH)
                .map(|b| (b.kind(), b.class()));
            let single =
                single_star_brief(&record, UniverseTime::EPOCH).map(|b| (b.kind(), b.class()));
            assert_ne!(
                row, single,
                "{id}: the row reads the pair, not the single-star track"
            );
        }
    }
}
