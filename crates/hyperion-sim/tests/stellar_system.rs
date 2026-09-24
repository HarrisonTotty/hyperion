//! A grid system's stars from its record (plan 06, P06.T29.b, with plan 11's P11.T2.c): pinned
//! summaries across the five layers, companions that move no primary, the share of multiple
//! systems, order independence, a death inside the clock window, and the brainstorm's property
//! test of life and death over random systems and times.

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{BodyId, Layer};
use hyperion_sim::stellar::Phase;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::multiplicity::{
    MultiplicityContext, MultiplicityModel, RedrawAttempt, StarSlot, draw_hierarchy,
};
use hyperion_sim::stellar::system::{
    ClockDeath, StarModel, StarSummary, SystemExistence, SystemStars, draw_metallicity,
};
use hyperion_sim::time::{ClockWindow, Span, UniverseTime};
use hyperion_sim::units::{SolarMasses, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

const SEED: u64 = 0x0600_0029_b000_5eed;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The cell of `layer` that holds the point 26,000 ly from the centre along +y, in the plane,
/// shifted by `step` cells along x: near the solar circle, whatever the layer's cell size.
fn solar_cell(layer: Layer, step: i32, lift: i32) -> CellKey {
    let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
    CellKey::new(layer, [step, 26_000 / size + lift, 0]).expect("a cell of the grid")
}

/// The first `n` records of `layer` near the solar circle.
fn records_of(galaxy: &Galaxy, layer: Layer, n: usize) -> Vec<SystemRecord> {
    let mut found = Vec::new();
    let mut cell = Vec::new();
    for step in 0.. {
        generate_cell(galaxy, solar_cell(layer, step, 0), &mut cell);
        found.append(&mut cell);
        if found.len() >= n {
            found.truncate(n);
            return found;
        }
        assert!(step < 10_000, "layer {layer:?} is empty near the Sun");
    }
    unreachable!("the loop returns or panics")
}

fn years(y: i64) -> UniverseTime {
    UniverseTime::from_julian_years(y).expect("inside the clock")
}

/// Writes one star's summary, each line starting with `lead`: two spaces for a primary, as the
/// golden was first written, and a companion's body index after them for a companion, so that no
/// companion's line shares a label with a primary's.
fn write_star(w: &mut GoldenWriter, lead: &str, star: &StarSummary) {
    let state = star.state();
    w.line(&format!(
        "{lead}{:?} {:?} {}",
        state.phase(),
        star.kind(),
        star.classification()
    ));
    w.f64(&format!("{lead}age"), state.age().value());
    w.f64(&format!("{lead}mass"), state.mass().value());
    w.f64(&format!("{lead}core"), state.core_mass().value());
    w.f64(&format!("{lead}L"), state.luminosity().value());
    w.f64(&format!("{lead}R"), state.radius().value());
    w.f64(
        &format!("{lead}Teff"),
        state.effective_temperature().value(),
    );
    match star.absolute_magnitude_v() {
        Some(m) => w.f64(&format!("{lead}M_V"), m.value()),
        None => w.line(&format!("{lead}M_V none")),
    }
    match star.colour_b_v() {
        Some(c) => w.f64(&format!("{lead}B-V"), c.value()),
        None => w.line(&format!("{lead}B-V none")),
    }
    match star.remnant() {
        Some(r) => w.f64(&format!("{lead}remnant {:?}", r.kind()), r.mass().value()),
        None => w.line(&format!("{lead}remnant none")),
    }
    match star.death_in_window() {
        Some((t, kind)) => w.line(&format!("{lead}dies in the window at {t} by {kind:?}")),
        None => w.line(&format!("{lead}no death in the window")),
    }
}

/// The dozen systems the summaries golden pins: three, two, two, two and three of layers A to E
/// near the solar circle.
fn pinned_records(galaxy: &Galaxy) -> Vec<(Layer, SystemRecord)> {
    [
        (Layer::A, 3),
        (Layer::B, 2),
        (Layer::C, 2),
        (Layer::D, 2),
        (Layer::E, 3),
    ]
    .into_iter()
    .flat_map(|(layer, n)| {
        records_of(galaxy, layer, n)
            .into_iter()
            .map(move |record| (layer, record))
    })
    .collect()
}

/// The clock times the summaries golden pins, Julian years from the epoch.
const PINNED_YEARS: [i64; 3] = [-500, 0, 500];

/// Golden summaries for a dozen pinned systems across the five layers at t = −500, 0 and +500
/// years (P06.T29.b), new at generator version 11, and then their companions (P11.T2.c).
///
/// The primaries come first, written exactly as before plan 11's companions joined the systems, so
/// that `golden_diff.py` shows the companions as an extension and every primary's line as
/// unmoved; each multiple system's companions follow all the primaries, by body index, with
/// their initial masses and their summaries at the same three times.
#[test]
fn summaries_are_pinned() {
    let galaxy = milky_way();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let pinned = pinned_records(&galaxy);
    for (layer, record) in &pinned {
        let stars = SystemStars::generate(&galaxy, record);
        w.line("");
        w.u64_hex(&format!("{layer:?} system"), record.id().raw());
        w.f64("initial mass", record.primary_initial_mass().value());
        w.f64("age at epoch", record.age_at_epoch().value());
        w.f64("[Fe/H]", stars.primary().composition().fe_h().value());
        match stars.death_time() {
            ClockDeath::At(t, kind) => w.line(&format!("death at {t} by {kind:?}")),
            ClockDeath::BeyondClockRange => w.line("death beyond the clock's range"),
            ClockDeath::AlreadyRemnantAtBirth => w.line("already a remnant at birth"),
        }
        for y in PINNED_YEARS {
            let summary = stars.summary_at(years(y));
            w.line(&format!("t = {y} yr: {:?}", summary.existence()));
            if let Some(primary) = summary.stars().first() {
                write_star(&mut w, "  ", primary);
            }
        }
    }
    w.line("");
    w.line("Companions (plan 11, P11.T2.c), by system and body index");
    for (layer, record) in &pinned {
        let stars = SystemStars::generate(&galaxy, record);
        if stars.star_count() < 2 {
            continue;
        }
        w.line("");
        w.u64_hex(
            &format!("companions of {layer:?} system"),
            record.id().raw(),
        );
        for (k, model) in stars.stars().iter().enumerate().skip(1) {
            w.f64(&format!("[{k}] initial mass"), model.initial_mass().value());
        }
        for y in PINNED_YEARS {
            let summary = stars.summary_at(years(y));
            w.line(&format!(
                "companions at t = {y} yr: {:?}",
                summary.existence()
            ));
            for star in summary.stars().iter().skip(1) {
                write_star(&mut w, &format!("  [{}] ", star.body().body_index()), star);
            }
        }
    }
    golden!("stellar/summaries", w.as_str());
}

/// Plan 11's companions move no primary (P11.T2.c): the primary of each system the summaries golden
/// pins is plan 06's model, built from the record alone, and its summaries at the pinned times are
/// the same, bit for bit, as those of the system with its companions ruled out.
#[test]
fn companions_move_no_primary() {
    let galaxy = milky_way();
    let mut multiples = 0;
    for (_, record) in pinned_records(&galaxy) {
        let stars = SystemStars::generate(&galaxy, &record);
        let alone = SystemStars::generate_in(&galaxy, &record, MultiplicityContext::ForcedSingle);
        assert_eq!(alone.star_count(), 1);
        multiples += usize::from(stars.star_count() > 1);
        let plan06 = StarModel::new(
            record.primary_initial_mass(),
            draw_metallicity(&galaxy, &record),
            StarDraws::for_star(galaxy.seed(), BodyId::new(record.id(), 0)),
            record.age_at_epoch(),
        )
        .expect("a grid primary");
        assert_eq!(stars.primary(), &plan06, "{:?}", record.id());
        assert_eq!(alone.primary(), &plan06, "{:?}", record.id());
        assert_eq!(stars.death_time(), alone.death_time());
        for y in PINNED_YEARS {
            let (with, without) = (stars.summary_at(years(y)), alone.summary_at(years(y)));
            assert_eq!(with.existence(), without.existence());
            let (Some(a), Some(b)) = (with.stars().first(), without.stars().first()) else {
                assert!(with.stars().is_empty() && without.stars().is_empty());
                continue;
            };
            assert_eq!(a, b, "{:?} at {y} yr", record.id());
            // Every field the golden writes, by its bits.
            let (mut wa, mut wb) = (GoldenWriter::new(), GoldenWriter::new());
            write_star(&mut wa, "  ", a);
            write_star(&mut wb, "  ", b);
            assert_eq!(wa.as_str(), wb.as_str(), "{:?} at {y} yr", record.id());
            let brief = |s: &SystemStars| {
                let mut w = GoldenWriter::new();
                if let Some(b) = s.brief_at(years(y)) {
                    w.line(&format!("{:?} {}", b.kind(), b.class()));
                    if let Some(l) = b.log_luminosity() {
                        w.f64("log L", l.value());
                    }
                    w.f64("Teff", b.effective_temperature().value());
                }
                w.as_str().to_owned()
            };
            assert_eq!(brief(&stars), brief(&alone));
        }
    }
    assert!(multiples > 0, "no pinned system has a companion");
}

/// Each companion is its hierarchy slot's star (P11.T2.c): the slot's initial mass, the draws of
/// its own body at attempt 0, the system's composition and the record's age; the summary covers
/// every star by body index and carries the hierarchy while the system exists, and the brief
/// counts the stars.
#[test]
fn every_companion_is_its_slots_star_with_the_systems_composition_and_age() {
    let galaxy = milky_way();
    let mut companions = 0;
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        for record in records_of(&galaxy, layer, 12) {
            let stars = SystemStars::generate(&galaxy, &record);
            let hierarchy = stars.hierarchy();
            assert_eq!(
                hierarchy,
                &draw_hierarchy(
                    &galaxy,
                    &record,
                    MultiplicityContext::Free,
                    RedrawAttempt::FIRST
                )
            );
            assert_eq!(stars.stars().len(), usize::from(stars.star_count()));
            for (model, slot) in stars.stars().iter().zip(hierarchy.stars()) {
                hyperion_testkit::float::assert_same_bits(
                    model.initial_mass().value(),
                    slot.initial_mass().value(),
                );
                assert_eq!(model.composition(), stars.primary().composition());
                assert_eq!(model.age_at_epoch(), record.age_at_epoch());
                assert_eq!(
                    model.draws(),
                    &StarDraws::for_attempt(galaxy.seed(), slot.body(), 0)
                );
            }
            companions += stars.stars().len() - 1;
            let summary = stars.summary_at(UniverseTime::EPOCH);
            match summary.existence() {
                SystemExistence::Exists => {
                    assert_eq!(summary.hierarchy(), Some(hierarchy));
                    let bodies: Vec<BodyId> =
                        summary.stars().iter().map(StarSummary::body).collect();
                    let slots: Vec<BodyId> = hierarchy.stars().iter().map(StarSlot::body).collect();
                    assert_eq!(bodies, slots);
                    let brief = stars
                        .brief_at(UniverseTime::EPOCH)
                        .expect("a system that exists");
                    assert_eq!(brief.star_count(), stars.star_count());
                }
                SystemExistence::NotYetBorn => {
                    assert_eq!(summary.hierarchy(), None);
                    assert!(summary.stars().is_empty());
                }
            }
        }
    }
    assert!(companions > 0, "sixty systems and not one companion");
}

/// Up to three records from each of `cells` random generation cells within 4,000 ly of the solar
/// circle's point along x and 2,000 ly along y, the layers taken in turn: the pinned samples of the
/// property tests below.
fn sample_records(galaxy: &Galaxy, seed: u64, n: usize) -> Vec<SystemRecord> {
    let mut rng = Lcg::new(seed);
    let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];
    let mut cell = Vec::new();
    let mut records = Vec::with_capacity(n + 3);
    let mut visited = 0;
    while records.len() < n {
        let layer = layers[visited % layers.len()];
        visited += 1;
        let size = u64::from(layer.cell_size_ly());
        let (along, across) = (4_000 / size, 2_000 / size);
        let step = i32::try_from(rng.next_u64() % (2 * along + 1)).expect("small")
            - i32::try_from(along).expect("small");
        let rise = i32::try_from(rng.next_u64() % (2 * across + 1)).expect("small")
            - i32::try_from(across).expect("small");
        generate_cell(galaxy, solar_cell(layer, step, rise), &mut cell);
        records.extend(cell.drain(..).take(3));
    }
    records
}

/// Over a pinned sample of `n` systems near the solar circle, the systems the multiplicity draw
/// made multiple number the model's expectation, the sum of
/// [`MultiplicityModel::multiple_fraction`] over their primaries, to within 3.29 standard
/// deviations (α = 10⁻³, two-sided). A system whose every companion the stability test dropped
/// still counts as drawn multiple.
fn check_multiple_share(seed: u64, n: usize) {
    let galaxy = milky_way();
    let model = MultiplicityModel::default_v1();
    let (mut multiples, mut expected, mut variance) = (0_u32, 0.0, 0.0);
    let records = sample_records(&galaxy, seed, n);
    for record in &records {
        let stars = SystemStars::generate(&galaxy, record);
        let h = stars.hierarchy();
        multiples += u32::from(h.star_count() > 1 || h.dropped_companions() > 0);
        let p = model.multiple_fraction(record.primary_initial_mass());
        expected += p;
        variance += p * (1.0 - p);
    }
    let z = (f64::from(multiples) - expected) / variance.sqrt();
    assert!(
        z.abs() < 3.29,
        "{multiples} multiple systems of {} against {expected:.1} expected (z = {z:.2})",
        records.len()
    );
}

#[test]
fn multiple_systems_follow_the_model() {
    check_multiple_share(0x0611_2c00_0000_0001, 400);
}

#[test]
#[ignore = "slow: 10⁴ systems, each with every star's track"]
fn multiple_systems_follow_the_model_over_ten_thousand_systems() {
    check_multiple_share(0x0611_2c00_0000_0002, 10_000);
}

/// Generating a system's stars does not depend on what was generated before, and the same record
/// gives the same stars twice.
#[test]
fn systems_are_the_same_whatever_the_order() {
    let galaxy = milky_way();
    let mut records = Vec::new();
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        records.extend(records_of(&galaxy, layer, 4));
    }
    hyperion_testkit::order::assert_order_independent(&records, |r| {
        let stars = SystemStars::generate(&galaxy, r);
        (stars.summary_at(years(-250)), stars.brief_at(years(250)))
    });
    for record in &records {
        assert_eq!(
            SystemStars::generate(&galaxy, record),
            SystemStars::generate(&galaxy, record)
        );
    }
}

/// A star whose death falls 100 years after the epoch is living at +99 years and a remnant at
/// +101, under the same ID, and the summary says when it dies.
#[test]
fn a_star_dying_at_a_hundred_years_is_living_at_99_and_a_remnant_at_101() {
    let galaxy = milky_way();
    let template = records_of(&galaxy, Layer::E, 1)[0];
    let build = |age: f64| {
        SystemRecord::from_parts(
            template.id(),
            *template.epoch_position(),
            SystemOrigin::Grid(template.component().expect("a grid record")),
            template.population(),
            SolarMasses::new(20.0),
            Years::new(age),
        )
    };
    let lifetime = SystemStars::generate(&galaxy, &build(1e6))
        .primary()
        .lifetime()
        .expect("a star of 20 M☉ dies");
    let record = build(lifetime.value() - 100.0);
    assert_eq!(
        draw_metallicity(&galaxy, &record),
        draw_metallicity(&galaxy, &build(1e6)),
        "the draw is flat in age below 8 Gyr"
    );
    let stars = SystemStars::generate(&galaxy, &record);
    let ClockDeath::At(t, _) = stars.death_time() else {
        panic!("the death is on the clock: {:?}", stars.death_time());
    };
    let since = t
        .checked_since(years(100))
        .expect("inside the clock")
        .as_seconds_f64();
    assert!(since.abs() < 1e3, "{since} s from +100 yr");
    let at = |y: i64| stars.summary_at(years(y)).stars()[0];
    assert!(at(99).state().phase().is_living());
    assert!(at(101).state().phase().is_remnant());
    assert_eq!(at(99).death_in_window().map(|(when, _)| when), Some(t));
    assert!(at(101).remnant().is_some() && at(99).remnant().is_none());
}

/// A system not yet born has no stars and no brief.
#[test]
fn a_system_not_yet_born_has_no_stars() {
    let galaxy = milky_way();
    let template = records_of(&galaxy, Layer::A, 1)[0];
    let unborn = SystemRecord::from_parts(
        template.id(),
        GalacticPosition::from_light_years([0.0, 26_000.0, 30.0]).expect("finite"),
        SystemOrigin::Grid(template.component().expect("a grid record")),
        Population::YoungThinDisc,
        SolarMasses::new(0.4),
        Years::new(-300.0),
    );
    let stars = SystemStars::generate(&galaxy, &unborn);
    let before = stars.summary_at(UniverseTime::EPOCH);
    assert_eq!(before.existence(), SystemExistence::NotYetBorn);
    assert!(before.stars().is_empty());
    assert_eq!(stars.brief_at(UniverseTime::EPOCH), None);
    let after = stars.summary_at(years(400));
    assert_eq!(after.existence(), SystemExistence::Exists);
    // Until P06.T15 the track starts on the zero-age main sequence.
    assert_eq!(after.stars()[0].state().phase(), Phase::MainSequence);
}

/// The brainstorm's property test over `n` random systems and times: no star in a living phase is
/// older than its lifetime, every remnant is older, and no state has a non-finite or non-positive
/// L, R or `T_eff`, black holes and `NoRemnant` excepted.
fn check_life_and_death(seed: u64, n: usize) {
    let galaxy = milky_way();
    let mut rng = Lcg::new(seed);
    let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];
    let mut cell = Vec::new();
    let mut checked = 0;
    let mut remnants = 0;
    while checked < n {
        let layer = layers[checked % layers.len()];
        // Cells within 4,000 ly of the solar circle's point along x and 2,000 ly along y.
        let size = u64::from(layer.cell_size_ly());
        let (along, across) = (4_000 / size, 2_000 / size);
        let step = i32::try_from(rng.next_u64() % (2 * along + 1)).expect("small")
            - i32::try_from(along).expect("small");
        let rise = i32::try_from(rng.next_u64() % (2 * across + 1)).expect("small")
            - i32::try_from(across).expect("small");
        generate_cell(&galaxy, solar_cell(layer, step, rise), &mut cell);
        for record in cell.drain(..).take(3) {
            let stars = SystemStars::generate(&galaxy, &record);
            let offset = i64::try_from(rng.next_u64() % 2_001).expect("small") - 1_000;
            let t = UniverseTime::EPOCH
                .checked_add(Span::from_seconds(offset * 31_557_600))
                .expect("inside the window");
            assert!(ClockWindow::contains(t));
            let summary = stars.summary_at(t);
            if summary.stars().is_empty() {
                continue;
            }
            let age = record.age_at(t).value();
            // Every star of the system, companions included (P11.T2.c).
            for (star, model) in summary.stars().iter().zip(stars.stars()) {
                let state = star.state();
                let what = format!("{:?} at {t}: {state:?}", star.body());
                match model.lifetime() {
                    Some(life) if state.phase().is_living() => {
                        assert!(age < life.value(), "{what} outlives {life:?}");
                    }
                    Some(life) => {
                        remnants += 1;
                        assert!(age >= life.value(), "{what} is a remnant before {life:?}");
                    }
                    None => assert_eq!(state.phase(), Phase::Substellar, "{what}"),
                }
                if !matches!(state.phase(), Phase::BlackHole | Phase::NoRemnant) {
                    for (name, v) in [
                        ("L", state.luminosity().value()),
                        ("R", state.radius().value()),
                        ("T_eff", state.effective_temperature().value()),
                    ] {
                        assert!(v.is_finite() && v > 0.0, "{what}: {name} = {v}");
                    }
                }
            }
            checked += 1;
        }
    }
    assert!(remnants > n / 20, "only {remnants} remnants in {n} systems");
}

#[test]
fn living_stars_are_younger_than_their_lifetimes_and_remnants_older() {
    check_life_and_death(0x0629_b000_0000_0001, 400);
}

#[test]
#[ignore = "slow: 10⁵ random systems, each built with its whole life"]
fn a_hundred_thousand_systems_live_and_die_in_order() {
    check_life_and_death(0x0629_b000_0000_0002, 100_000);
}
