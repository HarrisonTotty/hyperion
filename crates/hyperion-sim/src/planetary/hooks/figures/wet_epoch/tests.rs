//! P14.T48.b's volatile history on the Solar System table under plan 06's Sun, and the law's
//! cases (science-p14-t24b-laws §3.4 and §3.6).

use std::cell::Cell;
use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::irradiation::luminosity_flux;
use crate::planetary::derive::solar::{
    SOLAR_AGE, derive_historic, placed_about_the_sun, solar_disc, table_bodies,
};
use crate::planetary::hooks::figures::past_outgassing;
use crate::planetary::hooks::figures::tests::{MEDIAN_MAGMA_OCEAN_END, inputs_of};
use crate::planetary::hooks::surface::tests::{conditions_of, spin_of, table};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::Track;
use crate::units::SolarMasses;
use crate::units::consts::METRES_PER_AU;

/// Plan 06's track of a star of `mass` M☉ at solar composition with the median draws, to 6 Gyr.
fn track_of(mass: f64) -> Track {
    Track::to_age(
        SolarMasses::new(mass),
        &Composition::SOLAR,
        &StarDraws::median(),
        Years::new(6e9),
    )
}

/// Plan 06's Sun, the host whose state at each age the table's window reads.
fn sun() -> &'static Track {
    static SUN: OnceLock<Track> = OnceLock::new();
    SUN.get_or_init(|| track_of(1.0))
}

/// The window at a system age of a body on an orbit of `a_au` and `e` about the star of `track`,
/// at its state then.
fn window_about(track: &Track, a_au: f64, e: f64) -> impl Fn(Years) -> WindowAt + '_ {
    move |age| {
        let state = track.state_at(age);
        WindowAt::of([(
            luminosity_flux(state.luminosity(), Metres::new(a_au * METRES_PER_AU), e),
            state.effective_temperature(),
        )])
    }
}

/// The orbit about the Sun, semi-major axis (au) and eccentricity, of the table's body `name`, its
/// parent's for a moon (`derive::solar`'s).
fn table_orbit(name: &str) -> (f64, f64) {
    let &(_, _, _, a, e) = table_bodies().find(|(n, ..)| *n == name).unwrap();
    (a, e)
}

/// The history of the table's body `name`, derived as `body`, wet now if `liquid_water`, on the
/// orbit `(a_au, e)` about plan 06's Sun, from the median magma ocean's end to the Sun's age
/// `age`, its volcanic greenhouse failing below `threshold`.
fn history_of(
    name: &str,
    body: &DerivedBody,
    liquid_water: bool,
    (a_au, e): (f64, f64),
    (age, threshold): (Years, f64),
) -> VolatileHistory {
    let figures = inputs_of(body, name);
    VolatileHistory::derive_under(
        &VolatileHistoryInputs {
            atmosphere: body.atmosphere(),
            water: body.inventory().water(),
            radius: Metres::from(body.radius()),
            liquid_water,
            molten_until: Some(MEDIAN_MAGMA_OCEAN_END),
            age,
            window: window_about(sun(), a_au, e),
            outgassing: |t: Years| past_outgassing(&figures, t),
        },
        threshold,
    )
}

/// The table's bodies with their histories about plan 06's Sun at its age, each wet now by its
/// own surface conditions' flag (P14.T24.a).
pub(in crate::planetary::hooks::figures) fn table_histories()
-> Vec<(&'static str, DerivedBody, VolatileHistory)> {
    table()
        .into_iter()
        .map(|(name, body, conditions)| {
            let history = history_of(
                name,
                &body,
                conditions.liquid_water(),
                table_orbit(name),
                (SOLAR_AGE, VOLCANIC_GREENHOUSE_OUTGASSING),
            );
            (name, body, history)
        })
        .collect()
}

fn found<'a>(
    table: &'a [(&'static str, DerivedBody, VolatileHistory)],
    name: &str,
) -> &'a (&'static str, DerivedBody, VolatileHistory) {
    table.iter().find(|(n, ..)| *n == name).unwrap()
}

/// The table's Mars, with its history.
fn mars() -> (DerivedBody, VolatileHistory) {
    let (_, body, history) = *found(&table_histories(), "Mars");
    (body, history)
}

/// A system age, yr, as thousands of millions of years before the Sun's age.
fn ago(age: f64) -> f64 {
    (SOLAR_AGE.value() - age) / 1e9
}

/// Pins a body's volatile history under `name`: whether it came from a colder state, and its wet
/// epoch's liquid, span, flow, runoff and paleo-inventory, or its having none.
pub(crate) fn pin_history(
    w: &mut hyperion_testkit::golden::GoldenWriter,
    name: &str,
    wet_epoch: Option<&WetEpoch>,
    from_colder: bool,
) {
    w.line(&format!("{name}_from_colder = {from_colder}"));
    let Some(epoch) = wet_epoch else {
        w.line(&format!("{name}_wet_epoch = None"));
        return;
    };
    w.line(&format!(
        "{name}_wet_liquid = {}",
        epoch.liquid().substance().key().as_str()
    ));
    w.f64(&format!("{name}_wet_start"), epoch.start().value());
    w.f64(&format!("{name}_wet_end"), epoch.end().value());
    w.f64(
        &format!("{name}_effective_flow"),
        epoch.effective_flow().value(),
    );
    w.f64(&format!("{name}_runoff"), epoch.runoff_m_per_year());
    w.f64(
        &format!("{name}_paleo_inventory"),
        epoch.paleo_inventory().value(),
    );
}

#[test]
fn the_table_s_epochs_are_the_ruling_s() {
    let table = table_histories();
    for (name, body, history) in &table {
        eprintln!(
            "{name}: {:?}, keeps air {}, water {:.3e} kg, H2O kept {}, from colder {}, {:?}",
            body.atmosphere().state(),
            body.atmosphere().keeps_atmosphere(),
            body.inventory().water().value(),
            body.atmosphere().retention().retains(Gas::Water),
            history.from_colder(),
            history.wet_epoch(),
        );
    }
    // Earth's runs to the present, from its magma ocean's end.
    let earth = found(&table, "Earth")
        .2
        .wet_epoch()
        .expect("Earth is wet now");
    assert_same_bits(earth.end().value(), 0.0);
    assert!((earth.start().value() - ago(MEDIAN_MAGMA_OCEAN_END.value())).abs() < 1e-12);
    assert!((earth.runoff_m_per_year() - WET_WORLD_RUNOFF_M_PER_YEAR).abs() < 1e-15);
    assert!((earth.effective_flow().value() / 1e9 - earth.start().value()).abs() < 1e-9);
    // Mars's from about 4.0 Ga to 3.6 Ga.
    let mars = found(&table, "Mars").2.wet_epoch().expect("Mars was wet");
    let (start, end) = (mars.start().value(), mars.end().value());
    eprintln!("Mars: {start:.4} to {end:.4} Ga, {mars:?}");
    assert!((3.9..4.1).contains(&start), "Mars starts at {start} Ga");
    assert!((3.5..3.7).contains(&end), "Mars ends at {end} Ga");
    assert!(end > 3.0);
    let flow = mars.effective_flow().value();
    let runoff = mars.runoff_m_per_year();
    assert!((1e5..1e8).contains(&flow), "{flow} yr");
    assert!((0.07..1.1).contains(&runoff), "{runoff} m/a");
    let incision = flow * crate::math::powf(runoff, 0.45);
    assert!((4e4..1.6e5).contains(&incision), "{incision} yr");
    assert_eq!(mars.liquid(), Gas::Water.substance());
    // Venus was never outside the runaway limit from its magma ocean's end; the Moon, Mercury,
    // Ganymede and Ceres keep no air, and Titan keeps no H₂O.
    for name in ["Venus", "Moon", "Mercury", "Ganymede", "Titan", "Ceres"] {
        assert_eq!(found(&table, name).2.wet_epoch(), None, "{name}");
    }
}

#[test]
fn mars_s_paleo_inventory_is_a_global_layer_of_some_hundreds_of_metres() {
    let (body, history) = mars();
    let epoch = history.wet_epoch().unwrap();
    let layer_m = epoch.paleo_inventory().value() / 1_000.0;
    eprintln!(
        "Mars: {:.3e} kg of water, {:.3e} kg/m2, a {layer_m:.0} m layer",
        body.inventory().water().value(),
        epoch.paleo_inventory().value()
    );
    let r = Metres::from(body.radius()).value();
    assert_same_bits(
        epoch.paleo_inventory().value(),
        body.inventory().water().value() / (4.0 * core::f64::consts::PI * r * r),
    );
    // Inside the 100-1,500 m global-equivalent layer inferred for early Mars (Scheller et al.
    // 2021, Science 372, 56, abstract).
    assert!((100.0..1_500.0).contains(&layer_m), "{layer_m} m");
}

#[test]
fn the_greenhouse_s_edge_is_mars_s_outgassing_at_3_6_ga() {
    let (body, _) = mars();
    let figures = inputs_of(&body, "Mars");
    let fitted = past_outgassing(&figures, Years::new(SOLAR_AGE.value() - 3.60e9));
    eprintln!("Mars's O at 3.60 Ga: {fitted:?}");
    assert!(
        (fitted - VOLCANIC_GREENHOUSE_OUTGASSING).abs() < 1e-9,
        "refit VOLCANIC_GREENHOUSE_OUTGASSING to {fitted:?}"
    );
    // So Mars's epoch ends where it falls through it, at 3.60 Ga.
    let end = mars().1.wet_epoch().unwrap().end().value();
    assert!((end - 3.60).abs() < 1e-6, "{end} Ga");
}

#[test]
fn a_past_epoch_s_flow_fraction_puts_mars_at_2_2_e5_years() {
    let epoch = *mars().1.wet_epoch().unwrap();
    let span = (epoch.start().value() - epoch.end().value()) * 1e9;
    let fitted = MARS_EFFECTIVE_FLOW.value() / span;
    eprintln!("Mars's span {span:.6e} yr, fraction {fitted:?}");
    assert!(
        (fitted / PAST_EPOCH_FLOW_FRACTION - 1.0).abs() < 1e-9,
        "refit PAST_EPOCH_FLOW_FRACTION to {fitted:?}"
    );
    assert!((epoch.effective_flow().value() / MARS_EFFECTIVE_FLOW.value() - 1.0).abs() < 1e-6);
    assert!((epoch.runoff_m_per_year() - PAST_EPOCH_RUNOFF_M_PER_YEAR).abs() < 1e-15);
}

#[test]
fn a_tenth_either_way_in_the_greenhouse_s_edge_moves_mars_s_end_under_0_2_gyr() {
    let (body, history) = mars();
    let end = history.wet_epoch().unwrap().end().value();
    for factor in [0.9, 1.1] {
        let moved = history_of(
            "Mars",
            &body,
            false,
            table_orbit("Mars"),
            (SOLAR_AGE, factor * VOLCANIC_GREENHOUSE_OUTGASSING),
        );
        let moved_end = moved.wet_epoch().unwrap().end().value();
        eprintln!("O_w x {factor}: Mars ends at {moved_end:.4} Ga");
        assert!(
            (moved_end - end).abs() < 0.2,
            "{moved_end} against {end} Ga"
        );
    }
}

/// A Mars twin at 1 au lies in the window from its magma ocean's end, and dries as its
/// volcanic greenhouse fails, 0.8–1.2 Gyr after its formation (science-p14-t24b-laws §3.4).
///
/// The generated twin at 1 au keeps no H₂O (P14.T13.b's Jeans test at its worst exobase), so it
/// has no epoch: the check reads the table's Mars, which keeps its water, on the twin's orbit,
/// where its air, water and interior are Mars's (recorded in plan 14's Risks).
#[test]
fn a_mars_twin_at_1_au_dries_0_8_to_1_2_gyr_after_its_formation() {
    let disc = solar_disc();
    let &(_, kg, km, _, e) = table_bodies().find(|(n, ..)| *n == "Mars").unwrap();
    let placed = placed_about_the_sun(&disc, (kg, km, 1.0, e));
    let twin = derive_historic(&placed, &disc, SOLAR_AGE);
    let conditions = conditions_of(&twin, spin_of("Mars")).unwrap();
    eprintln!(
        "generated twin: {:?}, liquid {}, water {:.3e} kg, H2O kept {} at an exobase of {:.0}",
        twin.atmosphere().state(),
        conditions.liquid_water(),
        twin.inventory().water().value(),
        twin.atmosphere().retention().retains(Gas::Water),
        twin.atmosphere().exobase_temperature().value(),
    );
    assert!(!twin.atmosphere().retention().retains(Gas::Water));
    let generated = history_of(
        "Mars",
        &twin,
        conditions.liquid_water(),
        (1.0, e),
        (SOLAR_AGE, VOLCANIC_GREENHOUSE_OUTGASSING),
    );
    assert_eq!(generated.wet_epoch(), None);
    let (mars, _) = mars();
    let history = history_of(
        "Mars",
        &mars,
        false,
        (1.0, e),
        (SOLAR_AGE, VOLCANIC_GREENHOUSE_OUTGASSING),
    );
    let epoch = history
        .wet_epoch()
        .expect("a Mars that keeps its water at 1 au was wet");
    eprintln!("Mars at 1 au: {epoch:?}");
    let dried = SOLAR_AGE.value() / 1e9 - epoch.end().value();
    assert!(
        (0.8..1.2).contains(&dried),
        "dried {dried} Gyr after formation"
    );
    assert!((epoch.start().value() - ago(MEDIAN_MAGMA_OCEAN_END.value())).abs() < 1e-12);
}

#[test]
fn an_m_dwarf_s_planet_inside_its_pre_main_sequence_runaway_has_none() {
    let star = track_of(0.2);
    let now = star.state_at(SOLAR_AGE);
    // At 0.8 S⊕ now, inside the window, and wet now, with Earth's air and water.
    let a_au = (now.luminosity().value() / 0.8).sqrt();
    let window = window_about(&star, a_au, 0.0);
    let early = window(MEDIAN_MAGMA_OCEAN_END);
    eprintln!(
        "0.2 M☉: {:.4e} L☉ now at {a_au:.4} au; at the magma ocean's end {:.3} of the runaway's \
         flux",
        now.luminosity().value(),
        early.runaway()
    );
    assert!(window(SOLAR_AGE).in_window());
    assert!(early.inside_runaway());
    let table = table_histories();
    let earth = &found(&table, "Earth").1;
    let figures = inputs_of(earth, "Earth");
    let inputs = |molten_until: Years| VolatileHistoryInputs {
        atmosphere: earth.atmosphere(),
        water: earth.inventory().water(),
        radius: Metres::from(earth.radius()),
        liquid_water: true,
        molten_until: Some(molten_until),
        age: SOLAR_AGE,
        window: &window,
        outgassing: |t: Years| past_outgassing(&figures, t),
    };
    let history = VolatileHistory::derive(&inputs(MEDIAN_MAGMA_OCEAN_END));
    assert_eq!(history.wet_epoch(), None);
    // The same planet, had its surface formed after its host's runaway phase, would be wet to
    // the present.
    let late = (1..64)
        .map(|k| Years::new(f64::from(k) * 0.05e9))
        .find(|&t| !window(t).inside_runaway())
        .unwrap();
    let after = VolatileHistory::derive(&inputs(late));
    eprintln!(
        "from {:.2} Gyr: {:?}",
        late.value() / 1e9,
        after.wet_epoch()
    );
    assert_same_bits(after.wet_epoch().unwrap().end().value(), 0.0);
}

#[test]
fn mars_came_from_colder_and_earth_did_not() {
    let table = table_histories();
    assert!(found(&table, "Mars").2.from_colder());
    assert!(!found(&table, "Earth").2.from_colder());
}

#[test]
fn the_epoch_is_continuous_in_time() {
    let table = table_histories();
    for name in ["Earth", "Mars"] {
        let (_, body, history) = found(&table, name);
        let liquid = history.wet_epoch().unwrap().end().value() <= 0.0;
        let epoch = *history.wet_epoch().unwrap();
        for later in [1.0, 1e3, 1e6] {
            let age = Years::new(SOLAR_AGE.value() + later);
            let moved = history_of(
                name,
                body,
                liquid,
                table_orbit(name),
                (age, VOLCANIC_GREENHOUSE_OUTGASSING),
            );
            let moved = moved.wet_epoch().unwrap();
            // The same ages since formation, to well under a year.
            let since = |ago_gyr: f64, at: f64| at - ago_gyr * 1e9;
            let start = since(epoch.start().value(), SOLAR_AGE.value());
            let moved_start = since(moved.start().value(), age.value());
            assert!(
                (moved_start - start).abs() < 1.0,
                "{name}: {moved_start} {start}"
            );
            if liquid {
                assert_same_bits(moved.end().value(), 0.0);
            } else {
                let end = since(epoch.end().value(), SOLAR_AGE.value());
                let moved_end = since(moved.end().value(), age.value());
                assert!((moved_end - end).abs() < 1.0, "{name}: {moved_end} {end}");
            }
        }
    }
}

#[test]
fn the_reference_flood_intermittency_is_five_per_cent() {
    assert_same_bits(REFERENCE_FLOOD_INTERMITTENCY, 0.05);
}

/// A body lit by one star of 5,772 K at the flux `flux(age)`, S⊕, with Earth's air and water,
/// wet now if `liquid_water`, from 0.1 Gyr to 4.5 Gyr, outgassing `outgassing(age)`.
fn synthetic(
    earth: &DerivedBody,
    liquid_water: bool,
    flux: impl Fn(f64) -> f64,
    outgassing: impl Fn(f64) -> f64,
) -> VolatileHistory {
    VolatileHistory::derive(&VolatileHistoryInputs {
        atmosphere: earth.atmosphere(),
        water: earth.inventory().water(),
        radius: Metres::from(earth.radius()),
        liquid_water,
        molten_until: Some(Years::new(1e8)),
        age: Years::new(4.5e9),
        window: |age: Years| {
            WindowAt::of([(EarthFluxes::new(flux(age.value())), Kelvin::new(5_772.0))])
        },
        outgassing: |age: Years| outgassing(age.value()),
    })
}

/// The Sun's runaway-greenhouse and early-Mars limits at 5,772 K, S⊕.
fn limits() -> (f64, f64) {
    let t = Kelvin::new(5_772.0);
    (
        HabitableLimit::RunawayGreenhouse.flux(t).value(),
        HabitableLimit::EarlyMars.flux(t).value(),
    )
}

#[test]
fn a_runaway_ends_a_wet_world_s_epoch_and_a_dimming_host_a_past_one() {
    let table = table_histories();
    let earth = &found(&table, "Earth").1;
    let (runaway, early_mars) = limits();
    // Brightening through the runaway limit at 3 Gyr: wet now by its flag, but its water was lost
    // there.
    let brightening = |t: f64| runaway * (0.9 + 0.1 * (t - 1e8) / 2.9e9);
    let lost = synthetic(earth, true, brightening, |_| 1.0);
    let epoch = lost.wet_epoch().unwrap();
    assert!((epoch.end().value() - 1.5).abs() < 1e-9, "{epoch:?}");
    assert!((epoch.start().value() - 4.4).abs() < 1e-12);
    assert!(
        (epoch.effective_flow().value() - PAST_EPOCH_FLOW_FRACTION * 2.9e9).abs() < 1e-3,
        "{epoch:?}"
    );
    assert!((epoch.runoff_m_per_year() - PAST_EPOCH_RUNOFF_M_PER_YEAR).abs() < 1e-15);
    // Dimming through the early-Mars limit at 2 Gyr, its volcanoes never failing.
    let dimming = |t: f64| early_mars * (1.2 - 0.2 * (t - 1e8) / 1.9e9);
    let frozen = synthetic(earth, false, dimming, |_| 1.0);
    let epoch = frozen.wet_epoch().unwrap();
    assert!((epoch.end().value() - 2.5).abs() < 1e-9, "{epoch:?}");
    // A world whose greenhouse still holds and that is not wet now kept no record of a warmer
    // past.
    let steady = synthetic(
        earth,
        false,
        |_| f64::midpoint(runaway, early_mars),
        |_| 1.0,
    );
    assert_eq!(steady.wet_epoch(), None);
    // One whose volcanoes failed before the window opened had none either.
    let opening = |t: f64| early_mars * (0.9 + 0.2 * (t - 1e8) / 4.4e9);
    let spent = synthetic(earth, false, opening, |t| if t < 1e9 { 1.0 } else { 0.1 });
    assert_eq!(spent.wet_epoch(), None);
}

#[test]
fn a_window_that_opens_after_a_runaway_holds_no_epoch() {
    let table = table_histories();
    let earth = &found(&table, "Earth").1;
    let (runaway, _) = limits();
    // Inside the runaway limit until 1 Gyr, as an M dwarf's planet in its pre-main sequence.
    let fading = |t: f64| runaway * if t < 1e9 { 1.5 } else { 0.8 };
    assert_eq!(synthetic(earth, true, fading, |_| 1.0).wet_epoch(), None);
}

#[test]
fn a_body_still_molten_dry_or_airless_has_no_history() {
    let table = table_histories();
    let earth = &found(&table, "Earth").1;
    let figures = inputs_of(earth, "Earth");
    let (sky, outgassing) = (window_about(sun(), 1.0, 0.0167), |t: Years| {
        past_outgassing(&figures, t)
    });
    let inputs = VolatileHistoryInputs {
        atmosphere: earth.atmosphere(),
        water: earth.inventory().water(),
        radius: Metres::from(earth.radius()),
        liquid_water: true,
        molten_until: Some(MEDIAN_MAGMA_OCEAN_END),
        age: SOLAR_AGE,
        window: &sky,
        outgassing: &outgassing,
    };
    assert!(VolatileHistory::derive(&inputs).wet_epoch().is_some());
    let molten = VolatileHistoryInputs {
        molten_until: Some(Years::new(SOLAR_AGE.value() + 1.0)),
        ..inputs
    };
    assert_eq!(VolatileHistory::derive(&molten), VolatileHistory::NONE);
    let dry = VolatileHistoryInputs {
        water: Kilograms::ZERO,
        ..inputs
    };
    assert_eq!(VolatileHistory::derive(&dry).wet_epoch(), None);
    let moon = &found(&table, "Moon").1;
    let airless = VolatileHistoryInputs {
        atmosphere: moon.atmosphere(),
        ..inputs
    };
    assert!(!moon.atmosphere().keeps_atmosphere());
    assert_eq!(VolatileHistory::derive(&airless).wet_epoch(), None);
}

#[test]
fn the_history_reads_a_fixed_number_of_ages() {
    let table = table_histories();
    let (_, mars, _) = found(&table, "Mars");
    let figures = inputs_of(mars, "Mars");
    let (windows, outgassings) = (Cell::new(0_u32), Cell::new(0_u32));
    let (a, e) = table_orbit("Mars");
    let sky = window_about(sun(), a, e);
    let inputs = VolatileHistoryInputs {
        atmosphere: mars.atmosphere(),
        water: mars.inventory().water(),
        radius: Metres::from(mars.radius()),
        liquid_water: false,
        molten_until: Some(MEDIAN_MAGMA_OCEAN_END),
        age: SOLAR_AGE,
        window: |t: Years| {
            windows.set(windows.get() + 1);
            sky(t)
        },
        outgassing: |t: Years| {
            outgassings.set(outgassings.get() + 1);
            past_outgassing(&figures, t)
        },
    };
    let first = VolatileHistory::derive(&inputs);
    let counts = (windows.get(), outgassings.get());
    eprintln!("{counts:?} evaluations");
    // Two for the colder start, the grid once, and the full halvings of the start's and the end's
    // crossings; the outgassing at the grid's ages from the window's opening to the first dry one,
    // and at each of the end's halvings inside the window.
    assert_eq!(counts.0, 2 + WINDOW_AGES + 2 * CROSSING_HALVINGS);
    assert!(counts.1 > CROSSING_HALVINGS && counts.1 <= WINDOW_AGES + CROSSING_HALVINGS);
    assert_eq!(VolatileHistory::derive(&inputs), first);
    assert_eq!(
        (windows.get(), outgassings.get()),
        (2 * counts.0, 2 * counts.1)
    );
}

#[test]
fn the_past_outgassing_at_the_present_is_the_figures_and_zero_out_of_range() {
    let table = table_histories();
    let mut temperate = 0_u32;
    for (name, body, _) in &table {
        let inputs = inputs_of(body, name);
        let figures = crate::planetary::hooks::figures::GlobalFigures::derive(&inputs)
            .unwrap()
            .unwrap();
        if body.atmosphere().state() == SurfaceState::Temperate {
            assert_same_bits(past_outgassing(&inputs, inputs.age), figures.outgassing());
            temperate += 1;
        }
        for age in [f64::NAN, -1.0, f64::INFINITY] {
            assert_same_bits(past_outgassing(&inputs, Years::new(age)), 0.0);
        }
    }
    assert!(temperate >= 2, "{temperate}");
    // A gas envelope has no solid surface, so no outgassing at any age: Earth under Jupiter's.
    let giants = crate::planetary::derive::solar::solar_system();
    let (_, jupiter) = giants.iter().find(|(n, _)| *n == "Jupiter").unwrap();
    assert_eq!(jupiter.atmosphere().state(), SurfaceState::GasEnvelope);
    let earth = &found(&table, "Earth").1;
    let enveloped = crate::planetary::hooks::figures::FiguresInputs {
        atmosphere: jupiter.atmosphere(),
        ..inputs_of(earth, "Earth")
    };
    assert_same_bits(past_outgassing(&enveloped, Years::new(1e9)), 0.0);
}

#[test]
fn a_body_with_no_draw_starts_at_the_draw_s_median() {
    use crate::planetary::params::{MAGMA_OCEAN_END_EARLIEST, MAGMA_OCEAN_END_LATEST};

    let median = (MAGMA_OCEAN_END_EARLIEST.value() * MAGMA_OCEAN_END_LATEST.value()).sqrt() * 1e6;
    assert!((UNDRAWN_HISTORY_START.value() / median - 1.0).abs() < 1e-12);
    assert_same_bits(
        UNDRAWN_HISTORY_START.value(),
        MEDIAN_MAGMA_OCEAN_END.value(),
    );
    // Mars with no draw has the history of Mars at the median draw.
    let (mars, history) = mars();
    let figures = inputs_of(&mars, "Mars");
    let (a, e) = table_orbit("Mars");
    let undrawn = VolatileHistory::derive(&VolatileHistoryInputs {
        atmosphere: mars.atmosphere(),
        water: mars.inventory().water(),
        radius: Metres::from(mars.radius()),
        liquid_water: false,
        molten_until: None,
        age: SOLAR_AGE,
        window: window_about(sun(), a, e),
        outgassing: |t: Years| past_outgassing(&figures, t),
    });
    assert_eq!(undrawn, history);
}

/// The start condition, P14.T48.a's reading of T48.b ([`window_entry`]; science-p14-t24b-laws
/// §4.6): every body with an epoch entered the window, on the table and in the law's cases; a
/// frozen world whose greenhouse still holds entered it with no epoch; and a window that opens
/// only after a runaway, a dry body, an airless one and one still molten never entered it.
#[test]
fn every_body_with_an_epoch_entered_the_window() {
    let table = table_histories();
    for (name, body, history) in &table {
        let (a, e) = table_orbit(name);
        let entry = window_entry(&WindowEntryInputs {
            atmosphere: body.atmosphere(),
            water: body.inventory().water(),
            radius: Metres::from(body.radius()),
            molten_until: Some(MEDIAN_MAGMA_OCEAN_END),
            age: SOLAR_AGE,
            window: window_about(sun(), a, e),
        });
        eprintln!("{name}: {entry:?}, epoch {}", history.wet_epoch().is_some());
        if history.wet_epoch().is_some() {
            assert_eq!(entry, WindowEntry::Entered, "{name}");
        }
    }
    let earth = &found(&table, "Earth").1;
    let (runaway, early_mars) = limits();
    let entry_of = |flux: &dyn Fn(f64) -> f64, water: Kilograms, atmosphere: &Atmosphere| {
        window_entry(&WindowEntryInputs {
            atmosphere,
            water,
            radius: Metres::from(earth.radius()),
            molten_until: Some(Years::new(1e8)),
            age: Years::new(4.5e9),
            window: |age: Years| {
                WindowAt::of([(EarthFluxes::new(flux(age.value())), Kelvin::new(5_772.0))])
            },
        })
    };
    let water = earth.inventory().water();
    let steady = |_: f64| f64::midpoint(runaway, early_mars);
    assert_eq!(
        entry_of(&steady, water, earth.atmosphere()),
        WindowEntry::Entered
    );
    assert_eq!(synthetic(earth, false, steady, |_| 1.0).wet_epoch(), None);
    let fading = |t: f64| runaway * if t < 1e9 { 1.5 } else { 0.8 };
    assert_eq!(
        entry_of(&fading, water, earth.atmosphere()),
        WindowEntry::Never
    );
    assert_eq!(
        entry_of(&steady, Kilograms::ZERO, earth.atmosphere()),
        WindowEntry::Never
    );
    let moon = &found(&table, "Moon").1;
    assert_eq!(
        entry_of(&steady, water, moon.atmosphere()),
        WindowEntry::Never
    );
    let molten = window_entry(&WindowEntryInputs {
        atmosphere: earth.atmosphere(),
        water,
        radius: Metres::from(earth.radius()),
        molten_until: Some(Years::new(5e9)),
        age: Years::new(4.5e9),
        window: |_: Years| WindowAt::of([(EarthFluxes::new(1.0), Kelvin::new(5_772.0))]),
    });
    assert_eq!(molten, WindowEntry::Never);
}

/// The start condition reads the window at the history's grid ages in order, from the first, and
/// stops where the window opens or a runaway comes.
#[test]
fn the_start_condition_stops_where_the_window_opens() {
    let table = table_histories();
    let earth = &found(&table, "Earth").1;
    let reads = Cell::new(0_u32);
    let entry = window_entry(&WindowEntryInputs {
        atmosphere: earth.atmosphere(),
        water: earth.inventory().water(),
        radius: Metres::from(earth.radius()),
        molten_until: Some(Years::new(1e8)),
        age: Years::new(4.5e9),
        window: |age: Years| {
            reads.set(reads.get() + 1);
            let flux = if age.value() < 1e9 { 0.2 } else { 1.0 };
            WindowAt::of([(EarthFluxes::new(flux), Kelvin::new(5_772.0))])
        },
    });
    assert_eq!(entry, WindowEntry::Entered);
    // 1 Gyr is past the grid's 13th age, 0.1 + 12 × 4.4 ÷ 63 = 0.938 Gyr, and before its 14th.
    assert_eq!(reads.get(), 14);
}
