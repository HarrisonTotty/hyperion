//! P14.T50.d, an envelope's formation draws: E's spread against its σ, the Solar System's four
//! giants inside the central 68% of their own draws, the drawn giants' C/O against JWST's span,
//! the lean towards solids for migrated giants, the solids' factor's limits, the median ranks as
//! T50.b's, the same draws twice, the hold, and the golden.

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

use super::tests::{synthetic, table_giant};
use super::*;
use crate::GENERATOR_VERSION;
use crate::id::SystemId;
use crate::planetary::chem::AbundanceDraws;
use crate::planetary::derive::composition::giant_heavy_elements;
use crate::planetary::derive::radius::CoreComposition;
use crate::rng::tags::SELFTEST_STREAM;

/// How many envelopes each statistical test draws.
const DRAWS: u16 = 10_000;

/// The universe the tests draw in.
const SEED: Seed = Seed::new(0x0e14_50d0_0000_0001);

/// The `n`-th body the tests draw for, all of one system.
fn body(n: u16) -> BodyId {
    BodyId::new(
        SystemId::from_raw(0x0200_0800_2000_0000).expect("a valid system ID"),
        n,
    )
}

/// The `n`-th body's draws.
fn draws(n: u16) -> EnvelopeDraws {
    EnvelopeDraws::for_body(SEED, body(n))
}

/// The value at the quantile `p` of `sorted`, ascending.
fn quantile(sorted: &[f64], p: f64) -> f64 {
    let last = sorted.len() - 1;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "an index into a slice of at most 10⁴, from a rank in [0, 1]"
    )]
    let at = (p * last as f64).round() as usize;
    sorted[at.min(last)]
}

/// The share of `values` at most `x`.
fn rank_of(values: &[f64], x: f64) -> f64 {
    let below = values.iter().filter(|&&v| v <= x).count();
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts of at most 10⁴ are exact in an f64"
    )]
    let share = below as f64 / values.len() as f64;
    share
}

/// The largest E the bound allows the envelope `inputs` describes (f = 1): its trend times an
/// unbounded scatter, held to the bound alone.
fn bound(inputs: &EnvelopeInputs) -> f64 {
    let trend = EnvelopeAbundances::derive(inputs).unwrap();
    let host = host_per_hydrogen(inputs.fe_h, inputs.alpha_fe, &inputs.host);
    enrichment(
        inputs.mass,
        inputs.fractions.envelope(),
        trend.per_hydrogen(Element::He),
        metal_mass_per_hydrogen(&host),
        f64::INFINITY,
    )
}

/// Asserts the drawn envelope `drawn` of the inputs `inputs` finite, its E within the bound and
/// the floor and its solids' share within [0, 1].
fn assert_within_the_bound_and_the_floor(inputs: &EnvelopeInputs, drawn: &EnvelopeAbundances) {
    let e = drawn.enrichment();
    assert!((1.0..=bound(inputs)).contains(&e), "{e}");
    assert!((0.0..=1.0).contains(&drawn.solids_share()));
    for (element, n) in drawn.elements() {
        assert!(n.is_finite() && n > 0.0, "{element:?} {n}");
    }
    let (x, y, z) = drawn.mass_fractions();
    assert!((x + y + z - 1.0).abs() < 1e-12);
    assert!(drawn.carbon_to_oxygen().is_finite() && drawn.carbon_to_oxygen() > 0.0);
}

/// P14.T50.d: at the median ranks in situ, the drawn envelope is P14.T50.b's to the bit, over
/// masses from 1.5 M⊕ to 13 `M_J`, every disc zone, two hosts and two effective temperatures.
#[test]
fn at_the_median_ranks_in_situ_the_drawn_envelope_is_t50b_s_to_the_bit() {
    let masses = [
        (1.5, 0.002),
        (10.0, 0.05),
        (50.0, 0.55),
        (318.0, 0.82),
        (4_000.0, 0.95),
    ];
    for (earths, share) in masses {
        for zone in [1_800.0, 900.0, 300.0, 100.0, 30.0, 15.0] {
            for host in [(0.0, 0.0), (-0.5, 0.3)] {
                for t_eff in [90.0, 1_300.0] {
                    let inputs = synthetic(earths, share, zone, host, t_eff);
                    let trend = EnvelopeAbundances::derive(&inputs).unwrap();
                    let median = EnvelopeAbundances::derive_drawn(
                        &inputs,
                        &EnvelopeDraws::MEDIAN,
                        Migration::InSitu,
                    )
                    .unwrap();
                    for ((_, a), (_, b)) in trend.elements().zip(median.elements()) {
                        assert_same_bits(a, b);
                    }
                    assert_same_bits(trend.enrichment(), median.enrichment());
                    assert_same_bits(trend.ice_line_factor(), median.ice_line_factor());
                    assert_same_bits(median.solids_share(), 0.0);
                }
            }
        }
    }
}

/// P14.T50.d: over 10⁴ drawn envelopes of the table's Jupiter, log₁₀ E about its trend has its
/// median at 0 and its 16th and 84th percentiles at ∓[`ENRICHMENT_SCATTER`], to 0.015 dex (four
/// times the quantiles' sampling error), the floor (at −1.63σ) and the bound (at +2.7σ) clipping
/// only the tails; every E lies within them.
#[test]
fn the_enrichment_s_spread_is_its_stated_sigma() {
    let (_, inputs) = table_giant("Jupiter", HostAbundances::protosolar());
    let trend = EnvelopeAbundances::derive(&inputs).unwrap().enrichment();
    let mut offsets: Vec<f64> = (0..DRAWS)
        .map(|n| {
            let drawn =
                EnvelopeAbundances::derive_drawn(&inputs, &draws(n), Migration::InSitu).unwrap();
            assert_within_the_bound_and_the_floor(&inputs, &drawn);
            math::log10(drawn.enrichment() / trend)
        })
        .collect();
    offsets.sort_by(f64::total_cmp);
    let sigma = ENRICHMENT_SCATTER.value();
    let (p16, p50, p84) = (
        quantile(&offsets, 0.16),
        quantile(&offsets, 0.5),
        quantile(&offsets, 0.84),
    );
    eprintln!("Jupiter's log E about its trend: p16 {p16:.4}, p50 {p50:.4}, p84 {p84:.4}");
    assert!((p16 + sigma).abs() < 0.015, "{p16}");
    assert!(p50.abs() < 0.015, "{p50}");
    assert!((p84 - sigma).abs() < 0.015, "{p84}");
    // The floor holds the low tail at E = 1.
    assert_same_bits(offsets[0], math::log10(1.0 / trend));
}

/// P14.T50.d: each of the Solar System's four giants (the table's, about a protosolar host) lies
/// inside the central 68% of its own 10⁴ drawn envelopes in situ: its measured C/H (Guillot et
/// al. 2022, Table 2, as the [module](super) cites it) and, as its carbon over its sulphur needs,
/// P14.T50.b's C/O, by its mid-rank among the drawn C/O (the share below it and half the share at
/// it).
#[test]
fn the_solar_system_s_giants_sit_inside_the_central_68_percent_of_their_draws() {
    let measured = [
        ("Jupiter", 1.19e-3),
        ("Saturn", 2.50e-3),
        ("Uranus", 1.95e-2),
        ("Neptune", 2.45e-2),
    ];
    for (name, carbon) in measured {
        let (_, inputs) = table_giant(name, HostAbundances::protosolar());
        let gas_limit = EnvelopeAbundances::derive(&inputs)
            .unwrap()
            .carbon_to_oxygen();
        let drawn: Vec<EnvelopeAbundances> = (0..DRAWS)
            .map(|n| {
                EnvelopeAbundances::derive_drawn(&inputs, &draws(n), Migration::InSitu).unwrap()
            })
            .collect();
        let carbons: Vec<f64> = drawn.iter().map(|e| e.per_hydrogen(Element::C)).collect();
        let carbon_rank = rank_of(&carbons, carbon);
        let ratios: Vec<f64> = drawn
            .iter()
            .map(EnvelopeAbundances::carbon_to_oxygen)
            .collect();
        let at = |x: f64| (x / gas_limit - 1.0).abs() < 1e-12;
        let (below, level) = (
            ratios.iter().filter(|&&x| x < gas_limit && !at(x)).count(),
            ratios.iter().filter(|&&x| at(x)).count(),
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts of at most 10⁴ are exact in an f64"
        )]
        let mid_rank = (below as f64 + 0.5 * level as f64) / f64::from(DRAWS);
        eprintln!(
            "{name}: its measured C/H at rank {carbon_rank:.3}; T50.b's C/O {gas_limit:.3} at \
             mid-rank {mid_rank:.3}"
        );
        assert!(
            (0.16..=0.84).contains(&carbon_rank),
            "{name}: {carbon_rank}"
        );
        assert!((0.16..=0.84).contains(&mid_rank), "{name}: {mid_rank}");
    }
}

/// A drawn giant of the population [`population`] builds: its envelope, its host's C/O and
/// whether it was placed as migrated from its snow line.
struct Drawn {
    envelope: EnvelopeAbundances,
    host: f64,
    migrated: bool,
}

/// 10⁴ giants of 0.1–3 `M_J`, log-uniform, each with Thorngren et al.'s heavy elements
/// ([`giant_heavy_elements`]), about hosts of \[Fe/H\] uniform on ±0.4 at drawn C/O
/// ranks, each drawn on its own body's stream: the even ones migrated from their snow line at a
/// `T_eff` of 1,300 K, a hot Jupiter's, whose solids' rank reads as `migrated` says, and the odd
/// ones in situ from the snow line to 30 times it, log-uniform (170 K to 31 K: from the H₂O line
/// to beyond the CO₂ line), at 100 K.
fn population(migrated: Migration) -> Vec<Drawn> {
    (0..DRAWS)
        .map(|n| {
            let mut stream = Stream::open(SEED, SELFTEST_STREAM, ObjectKey::galaxy_item(n.into()));
            let mut uniform = || stream.uniform_open();
            let mass = EarthMasses::from(JupiterMasses::new(0.1 * math::powf(30.0, uniform())));
            let fe_h = Dex::new(-0.4 + 0.8 * uniform());
            let host = HostAbundances::of(
                fe_h,
                &AbundanceDraws {
                    carbon_to_oxygen: UnitUniform::new(uniform()).unwrap(),
                    magnesium_to_silicon: UnitUniform::HALF,
                },
            );
            let snow = 2.255 * crate::units::consts::METRES_PER_AU;
            let farther = math::powf(30.0, uniform());
            let placed_migrated = n % 2 == 0;
            let (migration, formation_distance, t_eff) = if placed_migrated {
                (migrated, snow, 1_300.0)
            } else {
                (Migration::InSitu, snow * farther, 100.0)
            };
            let heavy = giant_heavy_elements(mass).value() / mass.value();
            let inputs = EnvelopeInputs {
                mass,
                fractions: MassFractions::of(CoreComposition::EARTH_LIKE, 1.0 - heavy),
                formation_distance: Metres::new(formation_distance),
                snow_line: Metres::new(snow),
                fe_h,
                alpha_fe: Dex::ZERO,
                host,
                effective_temperature: Kelvin::new(t_eff),
                age: Years::new(4.6e9),
                surface_gravity: JUPITER_SURFACE_GRAVITY,
                primordial_helium_per_hydrogen: Element::He.protosolar_abundance_per_hydrogen(),
            };
            let envelope = EnvelopeAbundances::derive_drawn(&inputs, &draws(n), migration).unwrap();
            assert_within_the_bound_and_the_floor(&inputs, &envelope);
            Drawn {
                envelope,
                host: host.carbon_to_oxygen(),
                migrated: placed_migrated,
            }
        })
        .collect()
}

/// The C/O of `drawn`'s giants placed as migrated or not as `migrated` says (every one for
/// `None`), ascending.
fn ratios(drawn: &[Drawn], migrated: Option<bool>) -> Vec<f64> {
    let mut out: Vec<f64> = drawn
        .iter()
        .filter(|d| migrated.is_none_or(|m| d.migrated == m))
        .map(|d| d.envelope.carbon_to_oxygen())
        .collect();
    out.sort_by(f64::total_cmp);
    out
}

/// P14.T50.d: the C/O of 10⁴ drawn giants ([`population`]) spans JWST's: some below HD 189733 b's
/// 0.2 (Fu et al. 2024) and some above 0.7, every one finite within the bound and the floor; of
/// the migrated ones, at least 95% below 0.7, which Fu et al. (2025) favour, and none above 0.9,
/// which they strongly disfavour (both as decision-p14-t50b-envelope §2 reads them).
#[test]
fn the_drawn_giants_carbon_to_oxygen_spans_jwst_s() {
    let drawn = population(Migration::Inward);
    let (all, hot, cold) = (
        ratios(&drawn, None),
        ratios(&drawn, Some(true)),
        ratios(&drawn, Some(false)),
    );
    let line = |what: &str, r: &[f64]| {
        eprintln!(
            "{what}: {} giants, C/O {:.3} to {:.3}; p1 {:.3}, p16 {:.3}, p50 {:.3}, p84 {:.3}, \
             p99 {:.3}; below 0.2 {:.4}, above 0.7 {:.4}, above 0.9 {:.4}",
            r.len(),
            r[0],
            r[r.len() - 1],
            quantile(r, 0.01),
            quantile(r, 0.16),
            quantile(r, 0.5),
            quantile(r, 0.84),
            quantile(r, 0.99),
            rank_of(r, 0.2),
            1.0 - rank_of(r, 0.7),
            1.0 - rank_of(r, 0.9),
        );
    };
    line("every giant", &all);
    line("migrated", &hot);
    line("in situ", &cold);
    assert!(all[0] < 0.2, "{}", all[0]);
    assert!(all[all.len() - 1] > 0.7, "{}", all[all.len() - 1]);
    assert!(rank_of(&hot, 0.7) >= 0.95, "{}", rank_of(&hot, 0.7));
    assert!(hot[hot.len() - 1] < 0.9, "{}", hot[hot.len() - 1]);
}

/// P14.T50.d: the plan's lean towards solids for hot Jupiters, tested on [`population`]'s migrated
/// giants. Read in situ, their median keeps the gas phase's C/O, 11/7 of its host's, and about a
/// fifth lie below their host's; read as migrated (the lean adopted), the median falls to about its
/// host's and twice as many lie below, towards JWST's sub-stellar hot Jupiters
/// (decision-p14-t50b-envelope §2).
#[test]
fn the_lean_towards_solids_brings_the_migrated_giants_towards_sub_stellar() {
    let migrated = |drawn: &[Drawn]| -> (f64, f64) {
        let mut factors: Vec<f64> = drawn
            .iter()
            .filter(|d| d.migrated)
            .map(|d| d.envelope.ice_line_factor())
            .collect();
        factors.sort_by(f64::total_cmp);
        let below = drawn
            .iter()
            .filter(|d| d.migrated && d.envelope.carbon_to_oxygen() < d.host)
            .count();
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts of at most 10⁴ are exact in an f64"
        )]
        let share = below as f64 / factors.len() as f64;
        (quantile(&factors, 0.5), share)
    };
    let (median_without, below_without) = migrated(&population(Migration::InSitu));
    let (median_with, below_with) = migrated(&population(Migration::Inward));
    eprintln!(
        "the migrated giants without the lean: median factor {median_without:.3}, \
         {below_without:.3} sub-stellar; with it: median factor {median_with:.3}, {below_with:.3} \
         sub-stellar"
    );
    assert!(
        (median_without - 11.0 / 7.0).abs() < 1e-3,
        "{median_without}"
    );
    assert!(median_with < 1.1, "{median_with}");
    assert!(
        below_without < 0.25 && below_with > 0.35,
        "{below_without} {below_with}"
    );
}

/// P14.T50.d: the solids' factor is the gas phase's at a share of 0, bit for bit; the disc's own,
/// 1, at E = 1 and a share of 1; falls with the share; tends to the solids' own C/O at a large E;
/// and is 1 where the zone holds no solids or no gas.
#[test]
fn the_solids_factor_runs_from_the_gas_phase_to_eq_2() {
    // Within each zone between 1,500 K and the CO line, with the solids' own C/O over 0.545.
    let zones = [
        (900.0, 0.0),
        (300.0, 0.6 / 1.4 / (2.4 / 4.4)),
        (100.0, 0.6 / 2.3 / (2.4 / 4.4)),
        (30.0, 0.9 / 2.9 / (2.4 / 4.4)),
    ];
    for (zone, solids) in zones {
        let t = Kelvin::new(zone);
        assert_same_bits(solids_ice_line_factor(t, 2.65, 0.0), ice_line_factor(t));
        assert!(
            (solids_ice_line_factor(t, 1.0, 1.0) - 1.0).abs() < 1e-12,
            "{zone}"
        );
        let mut last = ice_line_factor(t);
        for k in 1..=10 {
            let factor = solids_ice_line_factor(t, 2.65, f64::from(k) / 10.0);
            assert!(factor < last, "{zone} {k}: {factor} after {last}");
            last = factor;
        }
        let saturated = solids_ice_line_factor(t, 1e9, 1.0);
        assert!(
            (saturated - solids).abs() < 1e-6,
            "{zone}: {saturated} against {solids}"
        );
    }
    for zone in [1_800.0, 15.0] {
        for share in [0.0, 0.5, 1.0] {
            let factor = solids_ice_line_factor(Kelvin::new(zone), 50.0, share);
            assert!((factor - 1.0).abs() < 1e-15, "{zone} {share}");
        }
    }
}

/// P14.T50.d: the solids' share in situ is 0 below the median rank and linear above it, and a
/// migrated body's is its rank.
#[test]
fn the_solids_share_reads_its_rank_by_migration() {
    let rank = |u| UnitUniform::new(u).unwrap();
    for u in [0.01, 0.25, 0.5] {
        assert_same_bits(solids_share(rank(u), Migration::InSitu), 0.0);
        assert_same_bits(solids_share(rank(u), Migration::Inward), u);
    }
    for (u, s) in [(0.6, 0.2), (0.75, 0.5), (0.99, 0.98)] {
        assert!(
            (solids_share(rank(u), Migration::InSitu) - s).abs() < 1e-12,
            "{u}"
        );
    }
}

/// P14.T50.d's determinism: the same seed and body give the same draws and the same envelope
/// twice, to the bit; another body or seed gives other draws; the draws are words 0 and 1 of the
/// body's stream, in that order.
#[test]
fn the_same_seed_gives_the_same_draws_twice() {
    let (_, inputs) = table_giant("Saturn", HostAbundances::protosolar());
    for n in [0, 1, 517, 9_999] {
        let (a, b) = (draws(n), draws(n));
        assert_same_bits(a.enrichment.value(), b.enrichment.value());
        assert_same_bits(a.solids.value(), b.solids.value());
        for migration in [Migration::InSitu, Migration::Inward] {
            let first = EnvelopeAbundances::derive_drawn(&inputs, &a, migration).unwrap();
            let second = EnvelopeAbundances::derive_drawn(&inputs, &b, migration).unwrap();
            for ((_, x), (_, y)) in first.elements().zip(second.elements()) {
                assert_same_bits(x, y);
            }
        }
        assert_ne!(draws(n), draws(n + 1));
        assert_ne!(draws(n), EnvelopeDraws::for_body(Seed::new(1), body(n)));
        let mut stream = Stream::open(
            SEED,
            tags::BODY_ENVELOPE_FORMATION,
            ObjectKey::from(body(n)),
        );
        assert_same_bits(a.enrichment.value(), stream.uniform_open());
        assert_same_bits(a.solids.value(), stream.uniform_open());
    }
}

/// P14.T50.d's golden, `planetary/envelope_formation`: the draws of three bodies, and with each
/// the table's four giants about a protosolar host, in situ and migrated, and a grid of bodies
/// from 1.5 M⊕ to 13 `M_J` in every zone, one draw and host a body in turn: E, the factor, the
/// solids' share, C/H, N/H, O/H and C/O. Nothing on the record reads them below 26.
#[test]
fn the_formation_draws_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut pin = |label: &str, got: &EnvelopeAbundances| {
        w.f64(&format!("{label} enrichment"), got.enrichment());
        w.f64(&format!("{label} ice_line_factor"), got.ice_line_factor());
        w.f64(&format!("{label} solids_share"), got.solids_share());
        for element in [Element::C, Element::N, Element::O] {
            w.f64(
                &format!("{label} {}/H", element.symbol()),
                got.per_hydrogen(element),
            );
        }
        w.f64(&format!("{label} C/O"), got.carbon_to_oxygen());
    };
    for n in 0..3 {
        let drawn = draws(n);
        pin_ranks(&mut pin, n, &drawn);
    }
    let masses = [
        (1.5, 0.002),
        (5.0, 0.02),
        (10.0, 0.05),
        (17.0, 0.12),
        (50.0, 0.55),
        (95.0, 0.71),
        (318.0, 0.82),
        (1_000.0, 0.9),
        (4_000.0, 0.95),
    ];
    let zones = [1_800.0, 900.0, 300.0, 100.0, 30.0, 15.0];
    let hosts = [
        ((-0.6, 0.3), 80.0),
        ((0.0, 0.0), 132.0),
        ((0.3, -0.04), 400.0),
    ];
    let mut k = 0_u16;
    for &(earths, envelope) in &masses {
        for zone in zones {
            let (host, t_eff) = hosts[usize::from(k) % hosts.len()];
            let inputs = synthetic(earths, envelope, zone, host, t_eff);
            for migration in [Migration::InSitu, Migration::Inward] {
                let got = EnvelopeAbundances::derive_drawn(&inputs, &draws(k), migration).unwrap();
                pin(
                    &format!("{earths} M⊕ {envelope} at {zone} K body {k} {migration:?}"),
                    &got,
                );
            }
            k += 1;
        }
    }
    golden!("planetary/envelope_formation", w.as_str());
}

/// Pins body `n`'s draws `drawn` and, at them, the table's four giants about a protosolar host in
/// situ and migrated, through `pin`.
fn pin_ranks(pin: &mut impl FnMut(&str, &EnvelopeAbundances), n: u16, drawn: &EnvelopeDraws) {
    for name in ["Jupiter", "Saturn", "Uranus", "Neptune"] {
        let (_, inputs) = table_giant(name, HostAbundances::protosolar());
        for migration in [Migration::InSitu, Migration::Inward] {
            let got = EnvelopeAbundances::derive_drawn(&inputs, drawn, migration).unwrap();
            pin(
                &format!(
                    "body {n} ranks {:.17} {:.17} {name} {migration:?}",
                    drawn.enrichment.value(),
                    drawn.solids.value()
                ),
                &got,
            );
        }
    }
}
