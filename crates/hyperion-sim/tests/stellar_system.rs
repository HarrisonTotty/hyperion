//! A grid system's stars from its record (plan 06, P06.T29.b): pinned summaries across the five
//! layers, order independence, a death inside the clock window, and the brainstorm's property
//! test of life and death over random systems and times.

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::stellar::Phase;
use hyperion_sim::stellar::system::{
    ClockDeath, StarSummary, SystemExistence, SystemStars, draw_metallicity,
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

fn write_star(w: &mut GoldenWriter, star: &StarSummary) {
    let state = star.state();
    w.line(&format!(
        "  {:?} {:?} {}",
        state.phase(),
        star.kind(),
        star.classification()
    ));
    w.f64("  age", state.age().value());
    w.f64("  mass", state.mass().value());
    w.f64("  core", state.core_mass().value());
    w.f64("  L", state.luminosity().value());
    w.f64("  R", state.radius().value());
    w.f64("  Teff", state.effective_temperature().value());
    match star.absolute_magnitude_v() {
        Some(m) => w.f64("  M_V", m.value()),
        None => w.line("  M_V none"),
    }
    match star.colour_b_v() {
        Some(c) => w.f64("  B-V", c.value()),
        None => w.line("  B-V none"),
    }
    match star.remnant() {
        Some(r) => w.f64(&format!("  remnant {:?}", r.kind()), r.mass().value()),
        None => w.line("  remnant none"),
    }
    match star.death_in_window() {
        Some((t, kind)) => w.line(&format!("  dies in the window at {t} by {kind:?}")),
        None => w.line("  no death in the window"),
    }
}

/// Golden summaries for a dozen pinned systems across the five layers at t = −500, 0 and +500
/// years (P06.T29.b), new at generator version 11.
#[test]
fn summaries_are_pinned() {
    let galaxy = milky_way();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (layer, n) in [
        (Layer::A, 3),
        (Layer::B, 2),
        (Layer::C, 2),
        (Layer::D, 2),
        (Layer::E, 3),
    ] {
        for record in records_of(&galaxy, layer, n) {
            let stars = SystemStars::generate(&galaxy, &record);
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
            for y in [-500, 0, 500] {
                let summary = stars.summary_at(years(y));
                w.line(&format!("t = {y} yr: {:?}", summary.existence()));
                for star in summary.stars() {
                    write_star(&mut w, star);
                }
            }
        }
    }
    golden!("stellar/summaries", w.as_str());
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
            let Some(star) = summary.stars().first() else {
                continue;
            };
            let state = star.state();
            let age = record.age_at(t).value();
            let what = format!("{:?} at {t}: {state:?}", record.id());
            match stars.primary().lifetime() {
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
