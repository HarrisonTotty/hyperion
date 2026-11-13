//! The wind's reach table (P11.T12's fix): its storage, its assembly, its reader, and its screen
//! against the walk of the generator's own wide pairs.

use super::*;
use crate::id::Layer;
use crate::stellar::binary::evolve::{interacts, own_members};
use crate::stellar::binary::reach::{FE_H_NODES, fe_h_of, mass_node_msun};
use crate::stellar::binary::testing::{Mix, generated_pairs, milky_way};
use crate::stellar::binary::wind_accretion::onset;
use crate::units::consts::SOLAR_RADIUS_M;

/// The seed of the tests' pairs and queries.
const SEED: u64 = 0x0b17_0012_3e1d_0029;

#[test]
fn stored_values_round_up_and_read_back() {
    assert_eq!(to_centidex(f64::NEG_INFINITY), Some(NO_WIND_CENTIDEX));
    hyperion_testkit::float::assert_same_bits(centidex_to_rsun2_per_msun(NO_WIND_CENTIDEX), 0.0);
    for v in [-14.3, -3.0, 0.0, 0.004, 2.71, 9.9] {
        let k = to_centidex(v).expect("storable");
        let back = math::log10(centidex_to_rsun2_per_msun(k));
        assert!(back >= v + SUM_MARGIN_DEX - 1e-12, "{v} → {k} → {back}");
        assert!(
            back < v + SUM_MARGIN_DEX + 0.01 + 1e-12,
            "{v} → {k} → {back}"
        );
    }
    for v in [f64::INFINITY, f64::NAN, 400.0, -400.0] {
        assert_eq!(to_centidex(v), None, "{v}");
    }
}

/// Nodes of the grid's shape, each with the sum `sum(mass, fe_h, bin)` (dex; −∞ for none) and AGB
/// figures `agb(mass, fe_h)`.
fn synthetic(
    sum: impl Fn(usize, usize, usize) -> f64,
    agb: impl Fn(usize, usize) -> Option<(f64, f64, f64, f64)>,
) -> Vec<WindReachNode> {
    let per_mass = FE_H_CELLS * FE_H_SUBDIVISIONS + 1;
    (0..=MASS_CELLS * MASS_SUBDIVISIONS)
        .flat_map(|m| (0..per_mass).map(move |z| (m, z)))
        .map(|(m, z)| {
            let mut node = WindReachNode::NONE;
            node.log_sum_rsun2_per_msun = std::array::from_fn(|k| sum(m, z, k));
            if let Some((start, lost, least, dust)) = agb(m, z) {
                node.agb_start_years = start;
                node.agb_lost_msun = lost;
                node.agb_least_msun = least;
                node.agb_dust_rsun = dust;
            }
            node
        })
        .collect()
}

#[test]
fn assemble_refuses_a_grid_of_the_wrong_shape_or_an_unsound_sample() {
    let nodes = synthetic(|_, _, _| 1.0, |_, _| None);
    assert!(assemble(&nodes).is_ok());
    assert!(matches!(
        assemble(&nodes[1..]),
        Err(AssembleWindReachError::WrongShape { .. })
    ));
    for spoil in [
        |n: &mut WindReachNode| n.log_sum_rsun2_per_msun[3] = f64::NAN,
        |n: &mut WindReachNode| n.log_sum_rsun2_per_msun[3] = f64::INFINITY,
        |n: &mut WindReachNode| n.agb_start_years = f64::NAN,
        |n: &mut WindReachNode| n.agb_lost_msun = -1.0,
        |n: &mut WindReachNode| n.agb_least_msun = f64::NAN,
        |n: &mut WindReachNode| n.agb_dust_rsun = -2.0,
    ] {
        let mut bad = nodes.clone();
        spoil(&mut bad[17]);
        assert_eq!(
            assemble(&bad),
            Err(AssembleWindReachError::UnsoundSample { sample: 17 })
        );
    }
    let huge = synthetic(|m, _, _| if m == 9 { 500.0 } else { 1.0 }, |_, _| None);
    assert!(matches!(
        assemble(&huge),
        Err(AssembleWindReachError::UnstorableCell { .. })
    ));
}

/// A cell takes its samples' largest sum, never falling with age, and its samples' widest AGB
/// figures; the reader takes the largest over a mass interval's cells, and the AGB's only from
/// its start.
#[test]
fn a_cell_takes_its_samples_largest_and_the_reader_its_cells() {
    // Mass node 13 (in cell 1) loses a wind from bin 40, more than its neighbours, and falls back
    // at bin 60, which the cell does not.
    let nodes = synthetic(
        |m, _, k| match (m, k) {
            (13, 40..60) => 3.0,
            (13, 60..) => -1.0,
            (_, 50..) => 0.0,
            _ => f64::NEG_INFINITY,
        },
        |m, _| (m == 13).then_some((2.0e9, 0.4, 0.6, 800.0)),
    );
    let cells = assemble(&nodes).expect("sound");
    let table = WindReachTable::of(&cells);
    let cell = FE_H_CELLS + 2;
    assert_eq!(cells.log_sum_centidex[cell][39], NO_WIND_CENTIDEX);
    assert_eq!(cells.log_sum_centidex[cell][40], 310);
    assert_eq!(cells.log_sum_centidex[cell][104], 310);
    assert_eq!(cells.log_sum_centidex[2 * FE_H_CELLS][50], 10);
    assert!((cells.agb_lost_msun[cell] - 0.44).abs() < 1e-12);
    assert!((cells.agb_least_msun[cell] - 0.6 / 1.1).abs() < 1e-12);
    assert!((cells.agb_dust_rsun[cell] - 880.0).abs() < 1e-9);
    // Cell 1 at FE_H_NODES[2]'s interval holds mass nodes 8–16.
    let (lo, hi) = (mass_node_msun(9), mass_node_msun(15));
    let fe_h = f64::midpoint(FE_H_NODES[2], FE_H_NODES[3]);
    let before = table.reach(lo, hi, fe_h, 1.0e9).expect("inside the table");
    assert_eq!(before.agb, None);
    let after = table.reach(lo, hi, fe_h, 3.0e9).expect("inside the table");
    assert!(after.agb.is_some());
    assert!(after.sum_rsun2_per_msun >= before.sum_rsun2_per_msun);
    let wider = table
        .reach(mass_node_msun(3), hi, fe_h, 3.0e9)
        .expect("inside the table");
    assert_eq!(wider, after);
    assert_eq!(table.reach(0.05, 1.0, fe_h, 1.0e9), None);
    assert_eq!(table.reach(1.0, 1.0, fe_h, 2.0e10), None);
    assert_eq!(table.reach(1.0, 1.0, f64::NAN, 1.0e9), None);
}

/// The generator's table reads: the cooling fits lose no wind, a red dwarf loses none on its main
/// sequence, and the Sun loses its envelope on its giant branches, reaching its AGB by 13 Gyr.
#[test]
fn the_generator_reads_its_table() {
    let table = WindReachTable::generator();
    let solar = 0.0;
    let at = |m: f64, age: f64| table.reach(m, m, solar, age).expect("inside the table");
    hyperion_testkit::float::assert_same_bits(at(0.09, 1.3e10).sum_rsun2_per_msun, 0.0);
    assert_eq!(at(0.09, 1.3e10).agb, None);
    assert_eq!(at(0.3, 1.3e10).agb, None);
    let sun = at(1.0, 1.3e10);
    assert!(
        sun.sum_rsun2_per_msun > at(1.0, 4.6e9).sum_rsun2_per_msun,
        "{sun:?}"
    );
    let agb = sun.agb.expect("the Sun reaches its AGB by 13 Gyr");
    assert!(
        (0.1..1.0).contains(&agb.lost_msun) && (0.4..1.0).contains(&agb.least_msun),
        "{agb:?}"
    );
    assert!(agb.dust_rsun > 100.0, "{agb:?}");
    assert_eq!(at(1.0, 1.0e9).agb, None);
}

/// Every row of the generator's table never falls with age.
#[test]
fn the_generators_rows_never_fall_with_age() {
    let table = WindReachTable::generator();
    for row in table.log_sum_centidex {
        assert!(row.windows(2).all(|w| w[0] <= w[1]), "{row:?}");
    }
}

/// The screen holds every wind-fed pair of the generator's (P11.T12's fix): of 3,000 pairs of
/// layers C, D and E at their records' ages and as many again (those whose primary has a track)
/// redrawn 10–10⁴ au wide at 0.85–1.15 of their primaries' lifetimes, every pair whose star takes
/// 10⁻⁴ M☉ of its partner's wind by then, the lobe and decay tests passing it over, is one the
/// screen says may take it.
#[test]
fn the_screen_holds_every_wind_fed_pair() {
    let galaxy = milky_way(SEED);
    let mut pairs = Vec::new();
    for (salt, layer) in (0_u64..).zip([Layer::C, Layer::D, Layer::E]) {
        pairs.extend(generated_pairs(&galaxy, layer, 1_000, SEED ^ salt));
    }
    // The same pairs on orbits 10–10⁴ au wide, each at an age about its primary's late phases.
    let mut mix = Mix(SEED ^ 0x0de5);
    let generated = pairs.len();
    for i in 0..generated {
        let (input, _) = &pairs[i];
        let [m1, m2] = input.masses();
        if m1 < MIN_INITIAL_MASS {
            continue;
        }
        let total = m1.value() + m2.value();
        let au = crate::math::exp10(1.0 + 3.0 * mix.unit());
        let mu = crate::units::GravitationalParameter::from_solar_masses(SolarMasses::new(total));
        let periapsis = input.orbit().periapsis().value();
        let orbit = input
            .orbit()
            .scaled(au * crate::units::consts::METRES_PER_AU / periapsis, mu)
            .expect("a wider orbit");
        let lifetime = crate::stellar::sse::Track::full(
            track_mass(m1),
            input.composition(),
            &input.draws()[0],
        )
        .lifetime()
        .map_or(1.0e10, Years::value);
        let age = (lifetime * (0.85 + 0.3 * mix.unit())).min(1.4e10);
        let wide = crate::stellar::binary::BinaryInput::new(
            m1,
            m2,
            *input.composition(),
            orbit,
            input.draws().clone(),
            input.age_at_epoch(),
        )
        .expect("the same pair");
        pairs.push((wide, age));
    }
    let share = |k: usize| {
        let (mut fed, mut screened, mut missed) = (0_u32, 0_u32, Vec::new());
        for (input, age) in pairs.iter().skip(k).step_by(8) {
            let members = own_members(input, *age, None);
            if interacts(input, &members, *age) {
                continue;
            }
            let [m1, m2] = input.masses().map(SolarMasses::value);
            let mut ends = [[m1, m1], [m2, m2]];
            ends.sort_by(|x, y| x[0].total_cmp(&y[0]));
            let r_p = input.orbit().periapsis().value() / SOLAR_RADIUS_M;
            let may = may_take_wind(ends, r_p, fe_h_of(input.composition()), *age);
            screened += u32::from(may);
            if onset(input, &members, *age).is_some() {
                fed += 1;
                if !may {
                    missed.push(format!(
                        "{m1:.4} + {m2:.4} M_sun, periastron {r_p:.4e} R_sun, [Fe/H] {:.3}, age \
                         {age:.4e} yr",
                        input.composition().fe_h().value()
                    ));
                }
            }
        }
        (fed, screened, missed)
    };
    let parts = crate::planetary::testing::in_shares(8, 8, share);
    let fed: u32 = parts.iter().map(|p| p.0).sum();
    let screened: u32 = parts.iter().map(|p| p.1).sum();
    let missed: Vec<&String> = parts.iter().flat_map(|p| &p.2).collect();
    println!("{fed} wind-fed pairs, {screened} the screen holds of the passed-over");
    assert!(
        missed.is_empty(),
        "wind-fed pairs the screen passes: {missed:#?}"
    );
    assert!(fed > 200, "the pairs reach the wind: {fed}");
}

/// The table's reader and the bound's verdicts are pinned bit for bit (the determinism audit):
/// the reach at every 61st mass node, each node of `FE_H_NODES` and every 13th age edge, and the
/// verdicts of 256 fixed queries.
#[test]
fn the_wind_reach_and_the_29_verdicts_are_pinned() {
    use crate::stellar::binary::{PairLight, pair_light_bound};
    use crate::units::Metres;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    let table = WindReachTable::generator();
    for n in (0..=MASS_CELLS * MASS_SUBDIVISIONS).step_by(61) {
        let m = mass_node_msun(n);
        for &fe_h in &FE_H_NODES {
            for k in (0..AGE_BINS).step_by(13) {
                let age = age_edge_years(k);
                let label = format!("mass node {n} [Fe/H] {fe_h:.3} age bin {k}");
                let reach = table.reach(m, m, fe_h, age).expect("inside the table");
                w.f64(&format!("{label}: sum"), reach.sum_rsun2_per_msun);
                if let Some(agb) = reach.agb {
                    w.f64(&format!("{label}: agb lost"), agb.lost_msun);
                    w.f64(&format!("{label}: agb least"), agb.least_msun);
                    w.f64(&format!("{label}: agb dust"), agb.dust_rsun);
                }
            }
        }
    }
    let mut mix = Mix(SEED ^ 0x0001_9029);
    let log_uniform =
        |u: f64, lo: f64, hi: f64| lo * crate::math::exp(u * crate::math::ln(hi / lo));
    for i in 0..256_u32 {
        let a = log_uniform(mix.unit(), 0.08, 150.0);
        let b = log_uniform(mix.unit(), 0.08, a);
        let p = log_uniform(mix.unit(), 1.0, 1.0e6);
        let age = log_uniform(mix.unit(), 1.0e5, 1.4e10);
        let fe_h = -2.5 + 2.8 * mix.unit();
        let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let verdict = pair_light_bound(
            SolarMasses::new(a),
            SolarMasses::new(b),
            Metres::new(p * SOLAR_RADIUS_M),
            &comp,
            Years::new(age)..=Years::new(age),
        );
        w.line(&format!(
            "query {i:03}: {a:.6} + {b:.6} M☉, periastron {p:.6e} R☉, age {age:.6e} yr, \
             [Fe/H] {fe_h:.4}: {verdict:?}"
        ));
        if let Some(PairLight::Bright(m)) = verdict {
            w.f64(&format!("query {i:03}: M_V"), m.value());
        }
    }
    golden!("stellar/wind_reach", w.as_str());
}
