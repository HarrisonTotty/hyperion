//! The tests of P11.T4's merged capture, on the record since the 30 folds: an oxygen–neon dwarf's
//! coalescence that holds the capture mass collapses at once by electron capture
//! (`Engine::product_member`; research-leans-45 item 1).
//!
//! - The research's pair (8.03 + 2.00 M☉, [Fe/H] 0.086, periastron 650 R☉, e 0.7): its coalescence
//!   leaves a neutron star by electron capture, where under every 29 hold it left a living product.
//! - Slow: the research's probe, 44,550 drawn pairs of layer E's lowest masses through
//!   `pair_light::pair_rows`, against the version's E table.
//!
//! The server golden `systems_in_range_briefs`' GPF 000N1Z D-36, whose carbon–oxygen dwarf's
//! coalescence with a helium star that its own track types oxygen–neon collapses at the merger into
//! a 1.26 M☉ neutron star, not later into one of 1.45 M☉, is `co_chandrasekhar_hold`'s golden test.

use super::*;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::stellar::Composition;
use crate::stellar::Phase;
use crate::stellar::binary::{BinaryInput, BinaryTimeline, SegmentKind, evolve};
use crate::stellar::draws::StarDraws;
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{
    Dex, GravitationalParameter, HeliumExcess, Metres, Radians, SolarMasses, Years,
};

/// A pair of `m1` and `m2` M☉ at metallicity `fe_h` dex, periastron `periastron_rsun` R☉ and
/// eccentricity `e`, with `draws`, under `rules`: research-leans-45's probe's pair (its orientation
/// and mean anomaly), 10 Gyr old.
fn probe_pair(
    [m1, m2]: [f64; 2],
    fe_h: f64,
    periastron_rsun: f64,
    e: f64,
    draws: [StarDraws; 2],
    rules: StellarRules,
) -> BinaryInput {
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::new(periastron_rsun / (1.0 - e) * SOLAR_RADIUS_M),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::new(e).expect("an eccentricity below 1"),
        Orientation::new(Radians::new(1.0), Radians::new(0.3), Radians::new(2.0))
            .expect("an orientation"),
        Radians::new(1.0),
    )
    .expect("an orbit");
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
        orbit,
        draws,
        Years::new(1.0e10),
    )
    .expect("a pair")
    .with_rules(rules)
}

/// The start of `timeline`'s first merger.
fn merger_age(timeline: &BinaryTimeline) -> Years {
    timeline
        .segments()
        .iter()
        .find(|s| s.kind() == SegmentKind::Merged)
        .expect("the pair coalesces")
        .start()
}

/// research-leans-45's pair, 8.03 + 2.00 M☉ at [Fe/H] 0.086, periastron 650 R☉, e 0.7, median
/// draws: its primary's common envelope leaves an oxygen–neon dwarf, which coalesces at about 1.489
/// Gyr in its companion's common envelope around a core of about 1.53 M☉, over the capture mass.
/// The product collapses at the coalescence by electron capture into Mandel and Müller's 1.26 M☉
/// neutron star, and nothing lives after it (under every 29 hold it lived on, a giant 0.04 Myr
/// later).
#[test]
fn an_oxygen_neon_dwarf_s_coalescence_past_the_capture_mass_collapses() {
    use crate::stellar::remnant::collapse::electron_capture_remnant;

    let run = |rules: StellarRules| {
        let input = probe_pair(
            [8.03, 2.00],
            0.086,
            650.0,
            0.7,
            [StarDraws::median(), StarDraws::median()],
            rules,
        );
        evolve(&input, Years::new(1.6e9))
    };
    let captured = run(StellarRules::of_version(crate::GENERATOR_VERSION));
    let at = merger_age(&captured);
    assert!(
        (1.48e9..1.50e9).contains(&at.value()),
        "the coalescence at {at:?}"
    );
    let collapse = captured
        .supernovae()
        .iter()
        .find(|s| s.age().value().total_cmp(&at.value()).is_eq())
        .expect("a collapse at the coalescence");
    assert_eq!(collapse.remnant(), electron_capture_remnant());
    for age in [at.value() + 4.0e4, at.value() + 1.0e7] {
        let state = captured.state_at(Years::new(age));
        assert!(
            state.stars().iter().all(|s| !s.phase().is_living()),
            "{age:.4e} yr: {state:?}"
        );
        assert_eq!(state.stars()[0].phase(), Phase::NeutronStar, "{state:?}");
    }
}

/// P11.T4's merged capture is deterministic. research-leans-45's pair, which reaches it, evolved
/// twice gives the same timeline, and its coalescence is collapsed at once both times
/// (`merged_capture::merged_captures` rises by one a run).
#[test]
fn the_merged_capture_is_deterministic() {
    use crate::stellar::binary::merged_capture::merged_captures;

    let input = probe_pair(
        [8.03, 2.00],
        0.086,
        650.0,
        0.7,
        [StarDraws::median(), StarDraws::median()],
        StellarRules::of_version(crate::GENERATOR_VERSION),
    );
    let runs = [(), ()].map(|()| {
        let before = merged_captures();
        let timeline = evolve(&input, Years::new(1.6e9));
        (timeline, merged_captures() - before)
    });
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[0].1, 1, "the capture is taken once a run");
}

/// research-leans-45's probe: 44,550 drawn pairs over 11 primaries of 8.0–8.2 M☉, 9 companions of
/// 1.5–2.3 M☉, 10 periastra of 562–1,778 R☉ log-even, 9 eccentricities of 0–0.9 and 5 [Fe/H] of
/// 0–0.176, each through `pair_light::pair_rows` and against the version's E table: the pairs with
/// a changed star brighter than the table at some bin, and the least room. As measured (2026-11-06,
/// at generator version 28 under every 30 hold, against version 28's table; pod, 16 vCPU): none,
/// the least room +1.19 mag, so E's table needs no slice of its own (research-leans-45's fallback),
/// where under every 29 hold 164 pairs passed it, the least room −1.45 mag (the research measured
/// 173 and −1.45 against the table refitted at 29, all of them the merged channel).
#[test]
#[ignore = "slow: 44,550 pairs through the binary engine and the pair-light walk"]
fn the_merged_channel_s_pairs_against_e_s_table() {
    use crate::galaxy::placement::CellKey;
    use crate::id::{BodyId, Layer};
    use crate::planetary::testing::in_shares;
    use crate::rng::Seed;
    use crate::stellar::binary::pair_light::{AGE_BINS, PairLightTable, from_cmag, pair_rows};
    use crate::stellar::photometry::absolute_v_of_state;

    let table = PairLightTable::generator(Layer::E).expect("E's table");
    let even = |lo: f64, hi: f64, n: u32, k: u32| lo + (hi - lo) * f64::from(k) / f64::from(n - 1);
    let mut points = Vec::with_capacity(44_550);
    for a in 0..11 {
        for b in 0..9 {
            for c in 0..10 {
                for d in 0..9 {
                    for f in 0..5 {
                        let p = 562.0 * crate::math::powf(1_778.0 / 562.0, f64::from(c) / 9.0);
                        points.push((
                            [even(8.0, 8.2, 11, a), even(1.5, 2.3, 9, b)],
                            p,
                            even(0.0, 0.9, 9, d),
                            even(0.0, 0.176, 5, f),
                        ));
                    }
                }
            }
        }
    }
    let key = CellKey::new(Layer::E, [3, 5, 7]).expect("a cell");
    let seed = Seed::new(0x5eed_0017_b0b0_f17e);
    let rules = StellarRules::of_version(crate::GENERATOR_VERSION);
    let shares = in_shares(64, 16, |share| {
        let mut past = 0_usize;
        let mut least = f64::INFINITY;
        for (k, &(masses, p, e, fe)) in points.iter().enumerate().skip(share).step_by(64) {
            let id = key
                .candidate_id(u32::try_from(k).expect("a small index"))
                .expect("under the capacity");
            let draws = [0, 1].map(|s| StarDraws::for_star(seed, BodyId::new(id, s)));
            let input = probe_pair(masses, fe, p, e, draws, rules);
            let Some(cell) = table.cell_of(masses[0], masses[1], fe, p) else {
                continue;
            };
            let rows = pair_rows(&input, &absolute_v_of_state);
            let mut room = f64::INFINITY;
            for bin in 0..AGE_BINS {
                let v = rows.changed_mag()[bin];
                if v.is_finite() && v < 1.0e300 {
                    room = room.min(v - from_cmag(table.changed_cmag(cell, bin)));
                }
            }
            past += usize::from(room < 0.0);
            least = least.min(room);
        }
        (past, least)
    });
    let past: usize = shares.iter().map(|s| s.0).sum();
    let least = shares.iter().map(|s| s.1).fold(f64::INFINITY, f64::min);
    println!(
        "{past} of {} pairs past E's table, the least room {least:.2} mag",
        points.len()
    );
    assert_eq!(past, 0);
    assert!((least - 1.19).abs() < 0.006, "{least}");
}
