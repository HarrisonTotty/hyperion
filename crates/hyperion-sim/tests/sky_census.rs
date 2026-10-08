//! The census against its oracle (rendering plan R06, R06.T8.e): with the caps forced, the census,
//! with its mass skip and its flux bound, and the brute force, which generates every system of the
//! same cells whole, agree star for star and bit for bit, at the query's `n_max` and at a third of
//! the stars listed, where the overflow is compared too.
//!
//! The spheres are those a test can afford to generate whole, counted on the fixture on
//! 2026-10-04 (systems in the cells opened; about 0.9 ms a system on one thread under load, census
//! and brute force each):
//!
//! | Test | Place | Layers within | Cut | Systems |
//! | ---- | ----- | ------------- | --- | ------- |
//! | fast | near the Sun | every layer, 150 ly | 11 | 39,600 |
//! | fast | nuclear disc | A and B, 1.5 ly (the eight cells at the place) | 11 | 49,534 |
//! | slow | near the Sun | A 300 ly, B 500 ly | 7.95 | 246,600 |
//! | slow | near the Sun | C, D and E, 1,000 ly | 7.95 | 1.40 M |
//! | slow | nuclear disc | D and E, 20 ly (the eight cells at the place) | 7.95 | 0.88 M |
//! | slow | near the Sun at +H, in motion | every layer, 150 ly | 11 | 39,600 |
//!
//! The fixture is built without kinematic tables, so its systems keep their epoch positions. The
//! census pads its cells and its bound before the drift by each layer's pad speed, so the last row
//! runs the fast near-Sun test in the fixture built with them, whose systems move (R06.T8.j,
//! `decision-r06-pad-speed.md`); `sky::census::cell`'s tests check its plan and pad against an
//! independent walk in the same galaxy.
//!
//! The plan's 200 ly in the nuclear disc would generate 17.3 M systems for D and E alone, and the
//! 10 ly over every layer accepted on 2026-10-03 would generate 1.9 M, since a cell there holds up
//! to 124,000 (R06's Risks). Layers C and the brown dwarfs are compared near the Sun only. The slow
//! A/B check is the one where identity needs the envelope to bound every M and K dwarf. There
//! the census skipped nearly all of them until R06.T16.b. Its bound at twice the primary's mass
//! over ages from zero skipped some 40% of A's and none of B's, so B's skips were printed rather
//! than asserted (decision-r06-census-cost, 2026-10-05), as the nuclear disc's fast test's still
//! are. R06.T8.g's bound star by star restores B's check, and in the slow near-Sun tests each of
//! B, C, D and E must generate under a quarter of its records. D's and E's quarter waits for plan
//! 11's P11.T17.c, whose verdicts bound the pairs that P11.T17.a cannot: until then it is printed.
//!
//! Three merger products are pinned (R06.T16.b): the two first-giant-branch stars of K-dwarf
//! primaries with M-dwarf companions, in systems 5–7 Gyr old, that T8.e's oracle found the census
//! missing when its bound read the envelope at the primary's own mass and age, and one merger
//! still on its main sequence, in a system 267 Myr old. Each is now listed by the census of its
//! cell, which equals its oracle bit for bit; each lies beyond the bound at its primary's own mass
//! and age, and within the bound at twice that mass over ages from zero.
//!
//! On wasm32-wasip1, which has no threads, the oracle runs on one: there the fast identity tests
//! are slow tests (`just test-wasm-slow` runs them, a 32-bit check of the identity), and the slow
//! ones are left out, which would take hours.

#[expect(
    dead_code,
    reason = "these checks use the sky helpers of tests/common alone"
)]
mod common;

use std::num::NonZeroU32;
use std::sync::OnceLock;

use common::sky::{
    NUCLEAR_DISC_LY, Part, SUN_LY, assert_same_stars, brute_force_parts, brute_force_parts_of,
    brute_force_sky, census_parts, census_parts_of, every_layer, observer_in_nuclear_disc,
    observer_near_sun,
};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::caps::CAPPED_LAYERS;
use hyperion_sim::sky::census::{
    CensusTallies, GRID_STAR_BOUND, RecordLight, SkyQuery, StarBounds, flux_bound, merge_census,
    star_offset_bound,
};
use hyperion_sim::sky::envelope::{BrightnessEnvelope, max_star_mass};
use hyperion_sim::sky::luminosity::{LuminosityTables, REFERENCE_TIME};
use hyperion_sim::sky::phase::PhaseEnvelope;
use hyperion_sim::sky::photometry::absolute_v_of_state;
use hyperion_sim::stellar::Phase;
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::{LightYears, Magnitudes};
use hyperion_testkit::float;

/// The Milky Way fixture the sim's own sky tests use.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
            .expect("the Milky Way-like parameters are valid")
    })
}

/// The eye's query to apparent V `cut`: a star is kept to the cut alone, as without the eye
/// (R06.T8.k).
fn eye_query(observer: Observer, cut: Magnitudes) -> SkyQuery {
    SkyQuery::builder(observer, cut)
        .eye(EyeObserver::default())
        .build()
        .expect("a valid query")
}

/// The parts' tallies summed.
fn tallies(parts: &[Part]) -> CensusTallies {
    let mut sum = CensusTallies::default();
    for (_, t) in parts {
        sum.add(t);
    }
    sum
}

/// The systems each capped layer generated, census then brute force.
type Generated = Vec<(Layer, u64, u64)>;

/// Whether [`agree`] asserts that the census skips some systems, or only prints how many it
/// skipped. The skips are the census's cost, not its identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Skips {
    /// The census must skip at least one system.
    Asserted,
    /// The share skipped is printed, a figure for R06.T17 and the census's cost ruling
    /// (`decision-r06-census-cost.md`).
    Recorded,
}

/// Asserts that the census of `query` with `radii` forced and its oracle agree star for star and
/// bit for bit, at the query's `n_max` and at a third of the listed, and that the census lists
/// stars; and, as `skips` says, that it skips systems. Returns each layer's generated systems.
fn agree(what: &str, query: SkyQuery, radii: &[(Layer, LightYears)], skips: Skips) -> Generated {
    agree_in(galaxy(), what, query, radii, skips)
}

/// [`agree`] in the galaxy `g`.
fn agree_in(
    g: &Galaxy,
    what: &str,
    query: SkyQuery,
    radii: &[(Layer, LightYears)],
    skips: Skips,
) -> Generated {
    let n_max = query.n_max();
    let census = census_parts(g, query.clone(), radii);
    let brute = brute_force_parts(g, query, radii);
    assert_eq!(census.len(), brute.len(), "{what}: the cells opened");
    let (c, b) = (tallies(&census), tallies(&brute));
    let mut generated = Vec::new();
    for &layer in &CAPPED_LAYERS {
        let (cl, bl) = (c.layer(layer), b.layer(layer));
        eprintln!(
            "{what}: {layer:?}: {} cells, {} generated of {}, {} accepted",
            cl.cells(),
            cl.generated(),
            bl.generated(),
            cl.accepted(),
        );
        assert_eq!(cl.accepted(), bl.accepted(), "{what}: {layer:?} accepted");
        generated.push((layer, cl.generated(), bl.generated()));
    }
    let (whole, oracle) = (
        merge_census(census.iter().cloned(), n_max),
        merge_census(brute.iter().cloned(), n_max),
    );
    assert!(!whole.listed().is_empty(), "{what}: the census lists stars");
    assert_same_stars(whole.listed(), oracle.listed(), what);
    assert_same_stars(whole.overflow(), oracle.overflow(), what);
    let third = u32::try_from(whole.listed().len() / 3).expect("at most n_max");
    let third = NonZeroU32::new(third.max(1)).expect("at least one");
    let (cut, cut_oracle) = (merge_census(census, third), merge_census(brute, third));
    assert!(!cut.overflow().is_empty(), "{what}: a third overflows");
    assert_same_stars(cut.listed(), cut_oracle.listed(), what);
    assert_same_stars(cut.overflow(), cut_oracle.overflow(), what);
    for &layer in &CAPPED_LAYERS {
        assert_eq!(
            cut.tallies().layer(layer).listed(),
            cut_oracle.tallies().layer(layer).listed(),
            "{what}: {layer:?} listed"
        );
    }
    let (skipping, every): (u64, u64) = generated
        .iter()
        .fold((0, 0), |(s, e), &(_, c, b)| (s + c, e + b));
    eprintln!("{what}: the census generates {skipping} of {every} systems");
    if skips == Skips::Asserted {
        assert!(
            skipping < every,
            "{what}: the census generates {skipping} of {every} systems, so skips none"
        );
    }
    generated
}

/// R06.T8.g's share: of each layer of `asserted` in `generated`, the census generates under a
/// quarter of the systems its oracle generates; each layer of `printed` is printed only, its
/// quarter waiting for plan 11's P11.T17.c. Only the slow near-Sun tests, which wasm32-wasip1
/// leaves out, ask it.
#[cfg(not(target_family = "wasm"))]
fn generates_under_a_quarter(
    what: &str,
    generated: &[(Layer, u64, u64)],
    asserted: &[Layer],
    printed: &[Layer],
) {
    for &(layer, census, brute) in generated {
        let quarter = census.saturating_mul(4) < brute;
        if asserted.contains(&layer) {
            assert!(
                quarter,
                "{what}: {layer:?} generates {census} of {brute} systems, not under a quarter"
            );
        } else if printed.contains(&layer) {
            eprintln!(
                "{what}: {layer:?} generates {census} of {brute} systems ({}under a quarter; \
                 asserted from P11.T17.c)",
                if quarter { "" } else { "not " }
            );
        }
    }
}

/// Each listed layer within its radius in light-years.
fn within_ly(radii: &[(Layer, f64)]) -> Vec<(Layer, LightYears)> {
    radii
        .iter()
        .map(|&(layer, ly)| (layer, LightYears::new(ly)))
        .collect()
}

/// Censuses the cell of grid system `raw` for `query` beside its oracle and asserts that the two
/// agree star for star and bit for bit, and that the system's listed star is a merger product in
/// `phase`: heavier than its primary was born and no heavier than [`max_star_mass`] allows, its
/// companion merged away, brighter than the bound at its primary's own mass and age (R06.T8.b's,
/// under which the census would have skipped it for `query`), and within R06.T16.b's bound.
fn pinned_merger(raw: u64, query: &SkyQuery, phase: Phase) {
    let g = galaxy();
    let id = SystemId::from_raw(raw).expect("a system ID");
    let what = format!("{id:?}");
    let key = CellKey::of(id).expect("a grid system");
    let (census, oracle) = (
        census_parts_of(g, query, &[key]),
        brute_force_parts_of(g, query, &[key]),
    );
    assert_same_stars(&census[0].0, &oracle[0].0, &what);
    let star = census[0]
        .0
        .iter()
        .find(|s| s.system() == id)
        .unwrap_or_else(|| panic!("{what} is listed"));
    let mut records = Vec::new();
    generate_cell(g, key, &mut records);
    let record = records.iter().find(|r| r.id() == id).expect("in its cell");
    let state = SystemStars::generate(g, record)
        .state_at(star.emitted())
        .expect("born");
    let merged = &state.stars()[usize::from(star.star().get())];
    let m1 = record.primary_initial_mass();
    assert_eq!(merged.phase(), phase, "{what}");
    assert!(
        merged.mass() > m1,
        "{what}: {:?} from {m1:?}",
        merged.mass()
    );
    assert!(merged.mass() <= max_star_mass(m1), "{what}");
    assert!(
        state.stars().iter().any(|s| s.phase() == Phase::NoRemnant),
        "{what}: a merger leaves its companion no remnant"
    );
    let envelope = BrightnessEnvelope::build(g);
    let age = record.age_at(star.emitted());
    let at_m1 = envelope
        .brightest(
            record.layer(),
            record.component().expect("a grid record"),
            m1,
            (age, age),
        )
        .expect("the primary shines")
        .value()
        - 2.5 * math::log10(f64::from(GRID_STAR_BOUND));
    let m_v = absolute_v_of_state(merged).expect("it shines").value();
    assert!(m_v < at_m1, "{what}: M_V {m_v} within {at_m1}");
    // T8.b's census skipped a system whose bound could not pass the cut at its nearest; the eye
    // adds nothing to the cut since R06.T8.k.
    let nearest = star.distance().value() - star_offset_bound(g, record).value();
    let faintest =
        query.cut().value() - 5.0 * math::log10(nearest / (10.0 * LIGHT_YEARS_PER_PARSEC));
    assert!(
        at_m1 > faintest,
        "{what}: the bound at m₁, {at_m1}, passes {faintest}"
    );
    let bound = flux_bound(&envelope, record, star.emitted()).expect("a bound");
    assert!(bound.value() <= m_v, "{what}: {} over {m_v}", bound.value());
    // R06.T8.g's bound star by star holds it too: its pair is one plan 11 cannot bound, so the
    // widened bound above alone holds the record, or one whose verdict bounds the product.
    let ages = (age, age);
    let light = StarBounds::of(g, record, ages).brightest(PhaseEnvelope::shared(), ages);
    eprintln!("{what}: M_V {m_v}, bound star by star {light:?}");
    match light {
        RecordLight::Unbounded => {}
        RecordLight::Brightest(m) => {
            assert!(m.value() <= m_v, "{what}: {} over {m_v}", m.value());
        }
        RecordLight::Dark => panic!("{what}: M_V {m_v} where its bound says none can shine"),
    }
}

/// T8.e's two merged giants near the Sun, which its oracle found the census missing (R06's Risks,
/// "Merger products outshine the flux bound until T16.b"), listed at the eye's cut, 7.95: in B,
/// a 1.19 M☉ giant of a 0.64 M☉ primary at V 7.27 from 354 ly; in C, a 1.12 M☉
/// core-helium-burning giant of a 0.76 M☉ primary at V 7.08 from 620 ly. Before P11.T4.i (the
/// integration branch's version 20) the second was a 1.23 M☉ giant on its first giant branch at
/// V 7.44, and the first was at V 7.53. T4.i, which starts a pair at its first arrival on the main
/// sequence (generator version 21), merges the first pair at 107.3 Myr rather than 136.0 Myr and
/// the second at 58.4 rather than 144.2 Myr, so at the same age the second is past its helium
/// flash.
#[test]
fn the_merged_giants_near_the_sun_are_listed_as_their_oracle_lists_them() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(7.95));
    for (raw, phase) in [
        (0x21fe_5648_7ff0_0001, Phase::FirstGiantBranch),
        (0x4204_6c99_ff00_000a, Phase::CoreHeliumBurning),
    ] {
        pinned_merger(raw, &query, phase);
    }
}

/// A merger on its main sequence near the Sun: a 0.90 M☉ primary and its near twin, merged into
/// one 1.80 M☉ star of M<sub>V</sub> 1.98 in a system 267 Myr old, at V 5.87 from 191 ly. Its
/// primary's own bound is M<sub>V</sub> 2.35, so a camera's cut of 5.95 lists it where T8.b's
/// census skipped it.
#[test]
fn a_main_sequence_merger_is_listed_as_its_oracle_lists_it() {
    let query = SkyQuery::builder(observer_near_sun(galaxy()), Magnitudes::new(5.95))
        .build()
        .expect("a valid query");
    pinned_merger(0x41fe_eca2_0000_0000, &query, Phase::MainSequence);
}

#[test]
fn the_dark_tables_hold_no_star_at_the_reference_time() {
    let g = galaxy();
    let dark = LuminosityTables::dark(g);
    assert_eq!(dark.time(), REFERENCE_TIME);
    let zero = float::bits(0.0);
    let places = [SUN_LY, NUCLEAR_DISC_LY].map(|ly| {
        PointLy::from(&GalacticPosition::from_light_years(ly).expect("in the root cube"))
    });
    for id in g.fields().component_ids() {
        for layer in Layer::ALL {
            let at = places.iter().map(|p| dark.get_at(id, layer, p));
            for f in std::iter::once(dark.get(id, layer)).chain(at) {
                // Light ages at the snapshots, between them and beyond the last, read as a query
                // at the epoch reads them.
                for years in [0, 500, 1_000, 2_000, 10_000, 100_000, 262_144, 400_000] {
                    let ago = Span::from_julian_years(years).expect("a span");
                    let ago = dark.age_for(UniverseTime::EPOCH, ago);
                    let what = format!("{id:?} {layer:?} {years} years");
                    assert_eq!(float::bits(f.total_light(ago).value()), zero, "{what}");
                    assert_eq!(float::bits(f.stars_per_system(ago)), zero, "{what}");
                    assert_eq!(float::bits(f.dark_per_system(ago)), zero, "{what}");
                    assert_eq!(float::bits(f.remnants_per_system(ago)), zero, "{what}");
                    for m in [-12.0, -5.0, 0.0, 5.0, 10.0, 20.0, 25.0] {
                        let m = Magnitudes::new(m);
                        assert_eq!(float::bits(f.count_brighter_than(m, ago)), zero, "{what}");
                        let light = f.light_fainter_than(m, ago).value();
                        assert_eq!(float::bits(light), zero, "{what}");
                        assert_eq!(f.colour_fainter_than(m, ago), None, "{what}");
                    }
                }
            }
        }
    }
    assert!(dark.heap_bytes() > 0, "the dark tables hold their zeros");
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "slow: on wasm32-wasip1, which has no threads, the oracle runs on one"
)]
fn the_census_is_its_oracle_150_ly_from_the_sun() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(11.0));
    agree(
        "every layer within 150 ly of the Sun",
        query,
        &every_layer(LightYears::new(150.0)),
        Skips::Asserted,
    );
}

/// T8.e's 150 ly identity test in motion (R06.T8.j; `decision-r06-pad-speed.md`): in the fixture
/// built with its kinematic tables, whose systems move, with the observer at +H, the census and
/// its oracle agree as they do at rest. Slow: the galaxy's kinematic tables are built first.
#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: builds the fixture's kinematic tables, then the 150 ly identity test"]
fn the_census_is_its_oracle_150_ly_from_the_sun_in_motion() {
    use common::sky::{moving, observer_near_sun_at};
    use hyperion_sim::time::CLOCK_WINDOW_H;

    let g = moving(galaxy());
    let plus_h = UniverseTime::EPOCH
        .checked_add(CLOCK_WINDOW_H)
        .expect("+H is on the clock");
    let query = eye_query(observer_near_sun_at(&g, plus_h), Magnitudes::new(11.0));
    agree_in(
        &g,
        "every layer within 150 ly of the Sun at +H, in motion",
        query,
        &every_layer(LightYears::new(150.0)),
        Skips::Asserted,
    );
}

/// The plan's oracle, every layer within one radius, is the census with every cap forced to it
/// by `with_caps_forced`.
#[test]
fn brute_force_sky_is_the_census_with_every_cap_forced() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(11.0));
    let radius = LightYears::new(40.0);
    let forced = query
        .clone()
        .with_caps_forced(radius)
        .expect("a valid forced cap");
    let per_layer = query
        .clone()
        .with_caps_forced_per_layer(&every_layer(radius))
        .expect("a valid forced cap");
    assert_eq!(forced, per_layer, "every layer forced to the radius");
    let n_max = query.n_max();
    let census = merge_census(
        census_parts(galaxy(), query.clone(), &every_layer(radius)),
        n_max,
    );
    let oracle = brute_force_sky(galaxy(), query, radius);
    assert!(!census.listed().is_empty(), "the census lists stars");
    assert_same_stars(census.listed(), oracle.listed(), "within 40 ly");
    assert_same_stars(census.overflow(), oracle.overflow(), "within 40 ly");
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "slow: on wasm32-wasip1, which has no threads, the oracle runs on one"
)]
fn the_census_is_its_oracle_in_the_nuclear_disc() {
    let query = eye_query(observer_in_nuclear_disc(galaxy()), Magnitudes::new(11.0));
    // Recorded, not asserted (decision-r06-census-cost, 2026-10-05): since R06.T16.b's bound at
    // twice the primary's mass over ages from zero, the census here skips none of A and B.
    agree(
        "A and B within 1.5 ly in the nuclear disc",
        query,
        &within_ly(&[(Layer::A, 1.5), (Layer::B, 1.5)]),
        Skips::Recorded,
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: generates every system of A within 300 ly and B within 500 ly of the Sun"]
fn the_census_is_its_oracle_for_the_dwarfs_near_the_sun() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(7.95));
    let generated = agree(
        "A within 300 ly and B within 500 ly of the Sun",
        query,
        &within_ly(&[(Layer::A, 300.0), (Layer::B, 500.0)]),
        Skips::Asserted,
    );
    for &(layer, census, brute) in &generated {
        // B's check was printed, not asserted, from R06.T16.b's bound at twice the primary's
        // mass over ages from zero until R06.T8.g's bound star by star (decision-r06-census-cost,
        // 2026-10-05), which restores it.
        if matches!(layer, Layer::A | Layer::B) {
            assert!(
                census < brute,
                "{layer:?}: the census generates {census} of {brute} systems"
            );
        }
    }
    generates_under_a_quarter("the dwarfs near the Sun", &generated, &[Layer::B], &[]);
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: generates every system of layers C to E within 1,000 ly of the Sun"]
fn the_census_is_its_oracle_1000_ly_from_the_sun() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(7.95));
    let generated = agree(
        "C to E within 1,000 ly of the Sun",
        query,
        &within_ly(&[
            (Layer::C, 1_000.0),
            (Layer::D, 1_000.0),
            (Layer::E, 1_000.0),
        ]),
        Skips::Asserted,
    );
    generates_under_a_quarter(
        "C to E near the Sun",
        &generated,
        &[Layer::C],
        &[Layer::D, Layer::E],
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: generates every system of the D and E cells at a place in the nuclear disc"]
fn the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc() {
    let query = eye_query(observer_in_nuclear_disc(galaxy()), Magnitudes::new(7.95));
    agree(
        "D and E within 20 ly in the nuclear disc",
        query,
        &within_ly(&[(Layer::D, 20.0), (Layer::E, 20.0)]),
        Skips::Asserted,
    );
}
