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
use hyperion_sim::stellar::binary::SegmentKind;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::multiplicity::{
    MAX_COMPANIONS, MultiplicityContext, MultiplicityModel, RedrawAttempt, StarSlot,
    draw_hierarchy, draw_star_count,
};
use hyperion_sim::stellar::rotation::Magnetism;
use hyperion_sim::stellar::system::{
    ClockDeath, RemnantDetail, StarModel, StarSummary, SystemExistence, SystemStars,
    draw_metallicity,
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
    // P11.T11: the class of the pair the binary engine ran the star in.
    w.line(&format!("{lead}binary class {:?}", star.binary_class()));
    // P06.T21–T22: a neutron star's pulsar state, a black hole's spin.
    match star.remnant_detail() {
        Some(RemnantDetail::NeutronStar(pulsar)) => {
            w.line(&format!("{lead}pulsar {:?}", pulsar.class()));
            w.f64(&format!("{lead}pulsar P"), pulsar.period().value());
            w.f64(&format!("{lead}pulsar Pdot"), pulsar.period_derivative());
            w.f64(&format!("{lead}pulsar B"), pulsar.field().value());
        }
        Some(RemnantDetail::BlackHole(hole)) => {
            w.f64(&format!("{lead}black hole spin"), hole.spin());
        }
        None => {}
    }
    // P06.T25: rotation, magnetism and activity.
    match star.rotation() {
        Some(spin) => {
            w.f64(&format!("{lead}rotation P"), spin.period().value());
            w.f64(&format!("{lead}v_eq"), spin.equatorial_speed().value());
        }
        None => w.line(&format!("{lead}rotation none")),
    }
    match star.magnetism() {
        Some(Magnetism::Fossil { field }) => w.f64(&format!("{lead}fossil field"), field.value()),
        Some(Magnetism::Dynamo { field }) => w.f64(&format!("{lead}dynamo field"), field.value()),
        None => w.line(&format!("{lead}no field")),
    }
    match star.activity() {
        Some(activity) => w.f64(
            &format!("{lead}log Lx/Lbol {:?}", activity.level()),
            activity.log_lx_lbol(),
        ),
        None => w.line(&format!("{lead}no activity")),
    }
    // P06.T26.a–c: variability.
    match star.variability() {
        Some(v) => {
            w.f64(&format!("{lead}{:?} P", v.kind()), v.period().value());
            w.f64(
                &format!("{lead}{:?} amplitude", v.kind()),
                v.amplitude().value(),
            );
        }
        None => w.line(&format!("{lead}not variable")),
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
/// pins is plan 06's model, built from the record alone, with plan 06's death, and its brief and,
/// unless the binary engine ran it in a pair (P11.T11), its summaries at the pinned times are the
/// same, bit for bit, as those of the system with its companions ruled out.
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
            let paired = stars.pairs().iter().any(|p| p.stars()[0].get() == 0);
            if !paired {
                assert_eq!(a, b, "{:?} at {y} yr", record.id());
                // Every field the golden writes, by its bits.
                let (mut wa, mut wb) = (GoldenWriter::new(), GoldenWriter::new());
                write_star(&mut wa, "  ", a);
                write_star(&mut wb, "  ", b);
                assert_eq!(wa.as_str(), wb.as_str(), "{:?} at {y} yr", record.id());
            }
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

/// The star count the wire documents as 1 to 5 (`StellarBriefDto::star_count`) is the sampler's
/// range: 1 to 2 + [`MAX_COMPANIONS`], three stellar companions at most (rulings 74 and 81) and a
/// bound brown dwarf (P11.T2.d). Checked where the capped count's mean is highest, over layer E's O
/// and B primaries, which also reach the stellar cap, so that four stars is a count the sampler
/// draws, not only a bound; five needs a brown dwarf beside them, which this sample need not meet.
#[test]
fn star_counts_run_from_one_to_five_even_in_layer_e() {
    const RECORDS: usize = 400;
    let galaxy = milky_way();
    let most = u8::try_from(2 + MAX_COMPANIONS).expect("a small cap");
    assert_eq!(most, 5, "the wire's doc says 1 to 5");
    let mut by_count = [0_usize; 6];
    for record in records_of(&galaxy, Layer::E, RECORDS) {
        let count = draw_star_count(
            &galaxy,
            &record,
            MultiplicityContext::Free,
            RedrawAttempt::FIRST,
        );
        assert!(
            (1..=most).contains(&count),
            "{:?}: {count} stars",
            record.id()
        );
        by_count[usize::from(count)] += 1;
    }
    assert!(
        by_count[4] + by_count[5] > 0,
        "no quadruple in {RECORDS} layer-E systems: {by_count:?}"
    );
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
/// [`MultiplicityModel::drawn_multiple_fraction`] over their primaries (ruling 81's blend of the
/// spine and direct constructions), to within 3.29 standard
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
        let p = model.drawn_multiple_fraction(record.primary_initial_mass());
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
        (
            stars.summary_at(years(-250)),
            stars.brief_at(years(250)),
            stars.state_at(years(600)),
            stars.attempt(),
            stars.pairs().to_vec(),
        )
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
    // A star 100 years after the onset of its collapse is a protostar (P06.T15.a).
    assert_eq!(after.stars()[0].state().phase(), Phase::Protostar);
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
            // Every star of the system, companions included (P11.T2.c). A star its pair has
            // changed (P11.T11: an accretor, a merged-away star) lives and dies by the pair, not
            // its own model's lifetime.
            for (star, model) in summary.stars().iter().zip(stars.stars()) {
                let state = star.state();
                let what = format!("{:?} at {t}: {state:?}", star.body());
                if model.state_at(t).as_ref() != Some(state) {
                    continue;
                }
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

/// Regression (P11.T4.h): record 0x61f85aa800000001 of the census's galaxy. Its 0.84 M☉
/// companion loses its envelope to its wind on the early AGB. The early AGB's remnant τ once fell
/// from at least 1 to 0 as the mass passed the helium core, where SSE's type 5 never goes, and the
/// mass-loss integrator stepped towards the envelope's loss without end, a knot at each pass, until
/// the machine ran out of memory. Each star's fate is found, and the companion dies when its
/// envelope is gone, its carbon–oxygen core inside its helium core.
#[test]
fn a_companion_stripped_by_its_wind_on_the_early_agb_has_a_fate() {
    use hyperion_sim::stellar::remnant::{DeathKind, Stripping};

    let galaxy = milky_way();
    let id = hyperion_sim::id::SystemId::from_raw(0x61f8_5aa8_0000_0001).expect("a grid ID");
    let record = hyperion_sim::galaxy::placement::resolve(&galaxy, id).expect("it resolves");
    let stars = SystemStars::generate(&galaxy, &record);
    assert_eq!(stars.star_count(), 3);
    let deaths: Vec<_> = stars
        .stars()
        .iter()
        .map(|model| model.death().expect("a star dies"))
        .collect();
    let stripped = deaths
        .iter()
        .find(|death| death.progenitor().co_core_mass() < death.progenitor().helium_core_mass())
        .expect("the companion loses its envelope on its early AGB");
    assert_eq!(stripped.kind(), DeathKind::EnvelopeLoss, "{stripped:?}");
    assert_eq!(
        stripped.progenitor().stripping(),
        Stripping::Wind,
        "{stripped:?}"
    );
    assert!(stripped.age().value() > 1e10, "{stripped:?}");
}

/// Regression (found by rendering plan R06.T5.c), then P11.T4.g: record 0x81fd865fd000000f of
/// seed `0x0926_0000` (15.05 + 12.65 M☉, bulge, layer E). A common envelope strips the primary to a
/// 4.93 M☉ helium star at 15.457 Myr, and from 15.688 Myr the main-sequence star feeds it. Its
/// held state was once an early-AGB supergiant of 1,054 R☉ with no envelope (M = Mc), which
/// touched its companion and recursed through the common envelope and the merger. Now a star with
/// no envelope is a helium star from that step (HPT section 6; BSE `hrdiag`): the primary stays
/// compact (under 3 R☉) to its pinned collapse at 15.896 Myr, no living hydrogen giant has
/// M ≤ Mc, and the pair meets no common envelope, contact or merger after 15.69 Myr. Between
/// 15.69 Myr and the collapse `swell` (BSE's rule for a helium star fed hydrogen) still makes the
/// accretor a core-helium-burning giant with a thin envelope again and again, each stripped back
/// at once: `swell`'s core is the open finding of P11.T4.g, outside it.
#[test]
fn a_held_bare_core_beside_a_main_sequence_star_stays_a_helium_star() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let id = hyperion_sim::id::SystemId::from_raw(0x81fd_865f_d000_000f).expect("a grid ID");
    let record = hyperion_sim::galaxy::placement::resolve(&galaxy, id).expect("it resolves");
    let stars = SystemStars::generate(&galaxy, &record);
    assert_eq!(stars.star_count(), 4);
    let pair = stars
        .pairs()
        .iter()
        .find(|pair| {
            let first = pair.timeline().state_at(Years::new(1.0e7));
            (first.stars()[0].mass().value() - 15.0).abs() < 0.1
        })
        .expect("the 15 M☉ pair");
    let timeline = pair.timeline();
    let collapse = timeline
        .supernovae()
        .first()
        .expect("the primary's collapse")
        .age()
        .value();
    assert!((collapse / 15.896e6 - 1.0).abs() < 1e-4, "{collapse}");
    let helium_from = 15.457e6;
    let giant = |phase: Phase| {
        matches!(
            phase,
            Phase::HertzsprungGap
                | Phase::FirstGiantBranch
                | Phase::CoreHeliumBurning
                | Phase::EarlyAgb
                | Phase::ThermallyPulsingAgb
        )
    };
    let mut checked = 0;
    for segment in timeline.segments() {
        let (from, to) = (segment.start().value(), segment.end().value());
        if from > 15.69e6 && from < collapse {
            assert!(
                matches!(
                    segment.kind(),
                    SegmentKind::Detached | SegmentKind::StableTransfer { .. }
                ),
                "{:?} at {from} yr",
                segment.kind()
            );
        }
        for k in 0..=4_u32 {
            let t = from + (to - from) * f64::from(k) / 4.0;
            if !(helium_from..collapse).contains(&t) {
                continue;
            }
            let primary = timeline.state_at(Years::new(t)).stars()[0];
            assert!(
                primary.radius().value() < 3.0,
                "{:?} of {} R☉ at {t} yr",
                primary.phase(),
                primary.radius().value()
            );
            if giant(primary.phase()) {
                assert_eq!(primary.phase(), Phase::CoreHeliumBurning, "at {t} yr");
                // At the segment's start, a step; inside a step the core may outgrow the mass
                // until the next one strips it.
                if k == 0 {
                    assert!(
                        primary.mass().value() > primary.core_mass().value(),
                        "at {t} yr"
                    );
                }
            } else {
                assert!(
                    matches!(
                        primary.phase(),
                        Phase::HeliumMainSequence
                            | Phase::HeliumHertzsprungGap
                            | Phase::HeliumGiantBranch
                    ),
                    "{:?} at {t} yr",
                    primary.phase()
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 20, "{checked}");
    let state = stars
        .state_at(UniverseTime::EPOCH)
        .expect("the system exists");
    assert_eq!(state.stars().len(), 4);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.u64_hex("system", id.raw());
    for (k, star) in state.stars().iter().enumerate() {
        w.line(&format!("[{k}] {:?}", star.phase()));
        w.f64(&format!("[{k}] mass"), star.mass().value());
        w.f64(&format!("[{k}] core mass"), star.core_mass().value());
        w.f64(&format!("[{k}] luminosity"), star.luminosity().value());
        w.f64(&format!("[{k}] radius"), star.radius().value());
    }
    golden!("stellar/held_bare_core_helium_star", w.as_str());
}

/// Regression (P11's protostar mergers, 2026-10-05; rendering plan R06's tables): three young
/// systems of the R06 luminosity fit's galaxy (`sky::binary_light`'s seed, `0x5b1a_0005_0000_5eed`;
/// layer D, [Fe/H] 0, ages 0.30–0.39 Myr), whose pairs' protostars overfill their orbits. Built to
/// their own ages, each such pair was merged at age zero into a 0.01 M☉ cooling star beside
/// nothing, which broke mass conservation and lit a dark protostar at an absolute V magnitude of
/// 17.5; built to a later age and read back, the same pairs were two protostars. Now no star
/// interacts before the first of a pair has arrived on the main sequence (P11.T4.i; before it,
/// before both had): every star is its own model's protostar, no pair is run, and each pair run to
/// an age past its arrival reads back the same two stars.
#[test]
fn young_pairs_whose_protostars_overfill_their_orbits_stay_protostars() {
    use hyperion_sim::id::SystemId;
    use hyperion_sim::orbit::roche_lobe_radius;
    use hyperion_sim::stellar::Composition;
    use hyperion_sim::stellar::binary::{can_interact, evolve};
    use hyperion_sim::units::consts::SOLAR_RADIUS_M;
    use hyperion_sim::units::{Dex, HeliumExcess};

    let galaxy = Galaxy::from_params(
        Seed::new(0x5b1a_0005_0000_5eed),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way-like gas is mostly neutral");
    let component = galaxy
        .fields()
        .component_id(0)
        .expect("a galaxy has components");
    let at = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the root cube");
    let composition = Composition::from_fe_h(Dex::new(0.0), HeliumExcess::ZERO);
    let mut overfilled = 0;
    for (raw, mass, age) in [
        (
            0x6214_5968_0000_005c,
            5.019_768_120_424_701,
            351_800.520_694_040_17,
        ),
        (
            0x6214_5968_0000_0090,
            2.527_505_005_945_29,
            296_917.301_111_903_4,
        ),
        (
            0x6214_5968_0000_0091,
            5.084_150_481_054_733,
            386_014.450_187_192_36,
        ),
    ] {
        let record = SystemRecord::from_parts(
            SystemId::from_raw(raw).expect("a grid ID"),
            at,
            SystemOrigin::Grid(component),
            galaxy.fields().component(component).population(),
            SolarMasses::new(mass),
            Years::new(age),
        );
        let stars =
            SystemStars::generate_with(&galaxy, &record, &composition, MultiplicityContext::Free);
        assert!(stars.pairs().is_empty(), "{raw:#x}: no pair is run yet");
        let state = stars
            .state_at(UniverseTime::EPOCH)
            .expect("the system exists");
        for (star, model) in state.stars().iter().zip(stars.stars()) {
            assert_eq!(
                Some(*star),
                model.state_at(UniverseTime::EPOCH),
                "{raw:#x}: each star is its own"
            );
            if model.initial_mass().value() >= 0.08 {
                assert_eq!(star.phase(), Phase::Protostar, "{raw:#x}: {star:?}");
            }
        }
        for (i, j, orbit, input) in star_pairs(&stars) {
            let until = Years::new(age + 1.0e3);
            assert!(!can_interact(&input, until), "{raw:#x}: before the arrival");
            let later = evolve(&input, Years::new(1.0e8)).state_at(Years::new(age));
            assert_eq!(
                later.stars(),
                &[state.stars()[i], state.stars()[j]],
                "{raw:#x}"
            );
            let [r0, r1] = later.stars().map(|s| s.radius().value());
            let [m0, m1] = later.stars().map(|s| s.mass().value());
            let periastron = orbit.periapsis();
            if r0 >= roche_lobe_radius(m0 / m1, periastron).value() / SOLAR_RADIUS_M
                || r1 >= roche_lobe_radius(m1 / m0, periastron).value() / SOLAR_RADIUS_M
            {
                overfilled += 1;
            }
        }
    }
    assert!(overfilled > 0, "a pair's protostars overfill their orbit");
}

/// Each pair of two stars of `stars` above 0.08 M☉: the stars' indices, the pair's orbit and the
/// binary engine's input for it, as plan 11's system stage builds it.
fn star_pairs(
    stars: &SystemStars,
) -> Vec<(
    usize,
    usize,
    hyperion_sim::orbit::KeplerElements,
    hyperion_sim::stellar::binary::BinaryInput,
)> {
    use hyperion_sim::stellar::binary::BinaryInput;
    use hyperion_sim::stellar::multiplicity::HierarchyNode;

    let hierarchy = stars.hierarchy();
    hierarchy
        .pairs()
        .filter_map(|(node, orbit)| {
            let HierarchyNode::Pair { inner, outer, .. } = *hierarchy.node(node) else {
                unreachable!("pairs are pairs");
            };
            let (HierarchyNode::Star(a), HierarchyNode::Star(b)) =
                (*hierarchy.node(inner), *hierarchy.node(outer))
            else {
                return None;
            };
            let [i, j] = [a, b].map(|s| usize::from(s.get()));
            let [first, second] = [&stars.stars()[i], &stars.stars()[j]];
            if second.initial_mass().value() < 0.08 {
                return None;
            }
            let input = BinaryInput::new(
                first.initial_mass(),
                second.initial_mass(),
                *first.composition(),
                *orbit,
                [first.draws().clone(), second.draws().clone()],
                first.age_at_epoch(),
            )
            .expect("a pair of stars");
            Some((i, j, *orbit, input))
        })
        .collect()
}
