//! The tests of the stellar and neutron-star mergers' tables fitted at 31 (P15.T10.b's merger step,
//! under every 31 hold; `mergers_hold`): a galaxy reads them; the fitted tables' shapes; and,
//! slow, the classes' Milky Way counts over ten seeds and each class's entries against the
//! engine's own mergers at 31 by a weighted two-sample Kolmogorov–Smirnov test at 1%, over the
//! half of the merger census that `hyperion-fit`'s tasks `binary_merger` and `binary_nsm` were not
//! fitted to (P15.T10.b's brief), a mark that misses pinned at its measured distance, a finding.
//! The distance from the half the tasks were fitted to is printed beside it: the shape's
//! resolution of the accreting white dwarfs' and the X-ray binaries' tests (`fitted_tables`),
//! which reads nothing here, since the classes' own structure leaves both distances far over the
//! critical one (the stellar mergers' ages and masses; plan 15's records).

use std::sync::OnceLock;

use hyperion_testkit::stats::{KolmogorovSmirnov, ks_critical_distance, ks_two_sample_weighted};

use super::*;
use crate::galaxy::catalogue_classes::binary::{MergerTableRules, NeutronStarMergers};
use crate::galaxy::catalogue_classes::grid::ClassProcess;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::math;
use crate::planetary::testing::in_shares;
use crate::rng::Seed;
use crate::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, MergerCensusPlan, MergerMember, NsmReading,
    StellarMergerReading, merger_census_bands, merger_census_in,
};
use crate::tables::binary::{
    CLASS_SHARES, CLASS_SHARES_31, MERGER_SAMPLER, MERGER_SAMPLER_31, MergerChannel, MergerMark,
    MergerPick, NSM_SAMPLER, NSM_SAMPLER_31, NsmMark, NsmPairKind, NsmPick,
};

/// The Milky Way fixture's seed: `hyperion-fit`'s manifests `binary_merger.toml`'s and
/// `binary_nsm.toml`'s, so that the census here runs the cells the fits left.
const SEED: u64 = 0x0b17_15a0_0000_0001;

/// The merger step's generator version: the manifests'.
const FIT_VERSION: crate::GeneratorVersion = crate::GeneratorVersion::new(31);

/// The fits' census galaxy: 31's rules with the scratch class shares
/// (`Galaxy::from_params_for_binary_refit`).
fn census_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_for_binary_refit(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            FIT_VERSION,
        )
        .expect("the Milky Way fixture builds")
    })
}

/// The Milky Way fixture of `seed` under every 31 hold.
fn at_31(seed: Seed) -> Galaxy {
    Galaxy::from_params_with_rules(
        seed,
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_31_hold(),
    )
    .expect("the Milky Way fixture builds")
}

/// The fixture under every 31 hold, the fitted tables its own.
fn fitted() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| at_31(Seed::new(SEED)))
}

/// A galaxy reads its rules' merger tables: one of the version's the scratch samplers and rows,
/// one under every 31 hold the fitted ones, the classes' hosts its rows'.
#[test]
fn from_31_a_galaxy_reads_the_fitted_merger_tables() {
    let version = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture builds");
    assert_eq!(version.rules().merger_tables, MergerTableRules::Scratch);
    assert_eq!(fitted().rules().merger_tables, MergerTableRules::Fitted);
    assert_eq!(
        version.rules().merger_tables.merger_sampler(),
        &MERGER_SAMPLER
    );
    assert_eq!(
        fitted().rules().merger_tables.merger_sampler(),
        &MERGER_SAMPLER_31
    );
    assert_eq!(version.rules().merger_tables.nsm_sampler(), &NSM_SAMPLER);
    assert_eq!(
        fitted().rules().merger_tables.nsm_sampler(),
        &NSM_SAMPLER_31
    );
    assert_ne!(MERGER_SAMPLER, MERGER_SAMPLER_31);
    assert_ne!(NSM_SAMPLER, NSM_SAMPLER_31);
    assert_eq!(fitted().rules().class_shares(), &CLASS_SHARES_31);
    for class in [CarvedClass::StellarMerger, CarvedClass::NeutronStarMerger] {
        assert_ne!(CLASS_SHARES_31.row(class), CLASS_SHARES.row(class));
    }
    for class in [
        CarvedClass::AccretingWdFast,
        CarvedClass::AccretingWdSlow,
        CarvedClass::XrayBinary,
    ] {
        assert_eq!(CLASS_SHARES_31.row(class), CLASS_SHARES.row(class));
    }
    // The classes' hosts at Milky Way parameters: the rows' totals, less the features' share of
    // the catalogue, inside the brainstorm's 0.2–0.5 red novae a year and about ten neutron-star
    // mergers.
    let stellar = StellarMergers::new(fitted())
        .hosts()
        .expected_hosts(fitted());
    let neutron = NeutronStarMergers::new(fitted())
        .hosts()
        .expected_hosts(fitted());
    let a_year = stellar / horizon_years();
    assert!((0.2..=0.5).contains(&a_year), "{stellar}: {a_year} a year");
    assert!((5.0..=20.0).contains(&neutron), "{neutron}");
}

/// The source horizon's span, Julian years: L + 2H.
fn horizon_years() -> f64 {
    crate::time::SourceHorizon::END
        .since_epoch()
        .as_julian_years_f64()
        - crate::time::SourceHorizon::START
            .since_epoch()
            .as_julian_years_f64()
}

/// The fitted samplers are in shape: each mark's quantiles non-decreasing at every node it has,
/// its limits a range, the marks conditional on the layer across (0, 4); each choice's weights a
/// share each. (A mark kept from the scratch table keeps its own quantiles, which its limits may
/// cut.)
#[test]
fn the_fitted_merger_tables_are_in_shape() {
    let in_order = |q: &[f64; 17], (lo, hi): (f64, f64), what: &str| {
        assert!(q.windows(2).all(|w| w[0] <= w[1]), "{what}: {q:?}");
        assert!(lo < hi, "{what}: {lo}–{hi}");
    };
    for mark in MergerMark::ALL {
        let law = MERGER_SAMPLER_31.mark(mark);
        in_order(law.residual(), law.limits(), &format!("{mark:?}"));
        for row in law.node_residuals().into_iter().flatten() {
            in_order(row, law.limits(), &format!("{mark:?} node"));
        }
        let span = law.median().map(crate::tables::binary::MedianCurve::span);
        assert!(
            span == Some((0.0, 4.0)) || span == Some((0.0, 1.0)),
            "{span:?}"
        );
    }
    for mark in NsmMark::ALL {
        let law = NSM_SAMPLER_31.mark(mark);
        in_order(law.residual(), law.limits(), &format!("{mark:?}"));
    }
    for weights in [
        MERGER_SAMPLER_31.pick(MergerPick::Layer).weights(),
        MERGER_SAMPLER_31.pick(MergerPick::EngineChannel).weights(),
        NSM_SAMPLER_31.pick(NsmPick::PairKind).weights(),
    ] {
        assert!(weights.iter().all(|&w| w >= 0.0));
        assert!(
            (weights.iter().sum::<f64>() - 1.0).abs() < 1e-12,
            "{weights:?}"
        );
    }
}

/// The 1% level of the tests here.
const KS_LEVEL: f64 = 0.01;

/// A mark's comparison: the distances of the class's sample from the engine's held-out half and
/// from its fitted half, and the 1% critical distance of the first (module documentation).
#[derive(Debug, Clone, Copy)]
struct Resolved {
    held_out: KolmogorovSmirnov,
    in_sample: f64,
    critical: f64,
}

impl Resolved {
    /// The comparison of `class` against the engine's `held_out` and `fitted` halves, each
    /// `(value, weight)` pairs.
    fn of(class: &[(f64, f64)], held_out: &[(f64, f64)], fitted: &[(f64, f64)]) -> Self {
        let held_out = ks_two_sample_weighted(&mut class.to_vec(), &mut held_out.to_vec());
        let in_sample = ks_two_sample_weighted(&mut class.to_vec(), &mut fitted.to_vec());
        Self {
            held_out,
            in_sample: in_sample.statistic,
            critical: ks_critical_distance(held_out.effective_n, KS_LEVEL),
        }
    }

    /// Whether the held-out distance is within the 1% critical distance.
    fn passes(&self) -> bool {
        self.held_out.statistic <= self.critical
    }
}

/// The parts of the census's cells the fits took, of [`CENSUS_PARTS`].
const FITTED_PARTS: u64 = 32;

/// The parts every census's cells are cut into, the fits'.
const CENSUS_PARTS: u64 = 64;

/// The census half of `records` records of `class`'s bands at the slab `site`, read by `read`, from
/// parts `parts`, on 16 threads, the parts joined in order: the fit's slab census (`hyperion-fit`'s
/// `tasks::binary_classes::mergers`), whose site and records are its manifest's.
fn census_half<R: Send>(
    class: CarvedClass,
    (site, records): (CensusSite, u64),
    parts: std::ops::Range<u64>,
    read: impl Fn(&MergerMember) -> Option<R> + Sync,
) -> Vec<R> {
    let galaxy = census_galaxy();
    let jobs: Vec<u64> = parts.collect();
    let per_part = records.div_ceil(FITTED_PARTS);
    in_shares(jobs.len(), 16, |i| {
        let plan = MergerCensusPlan {
            bands: merger_census_bands(class),
            site,
            whose: CensusRecords::All,
            records: per_part,
        };
        merger_census_in(
            galaxy,
            class,
            plan,
            CensusPart::new(jobs[i], CENSUS_PARTS),
            |m| read(&m),
        )
        .into_members()
    })
    .into_iter()
    .flatten()
    .flatten()
    .collect()
}

/// `n` of `process`'s entries in `cells` in `galaxy`, by `read`, past each cell's count as well as
/// inside it, as `engine_channels_hold`'s catalogue draws them: every candidate index is a draw of
/// the same process, its marks on streams of its own ID, and its test the class's; 16 runs, one a
/// thread, joined in order.
fn entries<P, T>(
    galaxy: &Galaxy,
    process: &P,
    cells: &[CatalogueCellKey],
    n: usize,
    read: impl Fn(P::Entry) -> Option<T> + Sync,
) -> Vec<T>
where
    P: ClassProcess + Sync,
    T: Send,
{
    let per_thread = n.div_ceil(16);
    let mut out: Vec<T> = in_shares(16, 16, |thread| {
        let bounds: Vec<_> = cells.iter().map(|key| process.bound(galaxy, key)).collect();
        let mut found = Vec::with_capacity(per_thread);
        let mut index = u32::try_from(thread).expect("a thread's index") << 20;
        while found.len() < per_thread {
            for (key, &bound) in cells.iter().zip(&bounds) {
                let id = key.candidate_id(index, 0).expect("an index under 2^24");
                if let Some(value) = process
                    .candidate(galaxy, key, id, bound)
                    .and_then(|c| process.test(galaxy, c))
                    .and_then(&read)
                {
                    found.push(value);
                }
            }
            index += 1;
        }
        found.truncate(per_thread);
        found
    })
    .into_iter()
    .flatten()
    .collect();
    out.truncate(n);
    out
}

/// The entries the tests draw of each class.
const ENTRIES: usize = 10_000;

/// The stellar mergers' slab census's records: `binary_merger.toml`'s.
const MERGER_RECORDS: u64 = 4_000_000;

/// The neutron-star mergers' slab census's records: `binary_nsm.toml`'s.
const NSM_RECORDS: u64 = 8_000_000;

/// The neutron-star mergers' slab: `binary_nsm.toml`'s `slab_site`, 32,000 by 8,000 by 3,000 ly
/// about the solar circle, whose layer-D cells hold its records.
const NSM_SLAB: CensusSite = CensusSite::new([0, 26_000, 0], [16_000, 4_000, 1_500]);

/// What the stellar merger test compares of a merger: log₁₀ of the pair's total initial mass, its
/// mass ratio, log₁₀ of the primary's initial mass and log₁₀ of its merger age, years.
const MERGER_COMPARED: [&str; 4] = [
    "log total initial mass (M_sun)",
    "mass ratio",
    "log primary initial mass (M_sun)",
    "log merger age (yr)",
];

/// A stellar merger's compared values, of its primary's initial mass `m1`, its mass ratio `q` and
/// its merger age `t`.
fn merger_values(m1: f64, q: f64, t: f64) -> [f64; 4] {
    [
        math::log10(m1 * (1.0 + q)),
        q,
        math::log10(m1),
        math::log10(t),
    ]
}

/// The class against the engine at 31 (P15.T10.b's merger step; slow): the class's entries in the
/// stellar merger reference cells against the engine's mergers its channels cover, weighted as
/// mergers in the horizon, at 1%, a miss a finding pinned in [`MERGER_MISSES`]; the channels' and
/// the layers' shares printed.
#[test]
#[ignore = "slow: two censuses of 4 × 10⁶ records through the binary engine and 10,000 entries"]
fn the_stellar_merger_sampler_against_the_engine() {
    let galaxy = census_galaxy();
    let read = |m: &MergerMember| {
        StellarMergerReading::of(galaxy, m).filter(|r| r.channel.is_some() && r.weight > 0.0)
    };
    let [held_out, fitted_half] = [FITTED_PARTS..CENSUS_PARTS, 0..FITTED_PARTS].map(|parts| {
        census_half(
            CarvedClass::StellarMerger,
            (CensusSite::SOLAR_CIRCLE, MERGER_RECORDS),
            parts,
            read,
        )
    });
    let cells = [[-1, 6, -1], [0, 6, -1], [-1, 6, 0], [0, 6, 0]].map(|at| {
        CatalogueCellKey::new(ClassId::STELLAR_MERGER, at).expect("a cell of the root cube")
    });
    let drawn_in = |galaxy: &Galaxy| -> Vec<([f64; 4], MergerChannel)> {
        let process = StellarMergers::new(galaxy);
        entries(galaxy, &process, &cells, ENTRIES, |e| {
            let scan = e.scan();
            let (m1, m2) = (scan.primary_mass().value(), scan.secondary_mass().value());
            Some((
                merger_values(m1, m2 / m1, scan.merger_age().value()),
                scan.channel(),
            ))
        })
    };
    let class = drawn_in(fitted());
    // The scratch class in the same cells, under the version's rules, for the record.
    let version = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture builds");
    let scratch = drawn_in(&version);
    let engine = |half: &[StellarMergerReading]| -> Vec<([f64; 4], f64)> {
        half.iter()
            .map(|r| {
                (
                    merger_values(r.primary_mass, r.mass_ratio, r.merger_age),
                    r.weight,
                )
            })
            .collect()
    };
    let (held, fit) = (engine(&held_out), engine(&fitted_half));
    let share = |c: MergerChannel| {
        let total: f64 = held_out.iter().map(|r| r.weight).sum();
        let engine: f64 = held_out
            .iter()
            .filter(|r| r.channel == Some(c))
            .map(|r| r.weight)
            .sum::<f64>()
            / total;
        #[expect(clippy::cast_precision_loss, reason = "counts of thousands")]
        let drawn = class.iter().filter(|e| e.1 == c).count() as f64 / class.len() as f64;
        (engine, drawn)
    };
    println!(
        "{} held-out and {} fitted engine mergers, {} entries",
        held_out.len(),
        fitted_half.len(),
        class.len()
    );
    for c in MergerChannel::ALL {
        println!("  {c:?}: engine {:.3?} (engine, class)", share(c));
    }
    let mut misses = Vec::new();
    for (k, name) in MERGER_COMPARED.iter().enumerate() {
        let drawn: Vec<(f64, f64)> = class.iter().map(|e| (e.0[k], 1.0)).collect();
        let pick = |values: &[([f64; 4], f64)]| -> Vec<(f64, f64)> {
            values.iter().map(|v| (v.0[k], v.1)).collect()
        };
        let r = Resolved::of(&drawn, &pick(&held), &pick(&fit));
        let before = Resolved::of(
            &scratch.iter().map(|e| (e.0[k], 1.0)).collect::<Vec<_>>(),
            &pick(&held),
            &pick(&fit),
        );
        println!(
            "  {name}: D {} (p {:.3e}) against {:.6} + in-sample {:.6}; the scratch class's D {:.4}",
            r.held_out.statistic,
            r.held_out.p_value,
            r.critical,
            r.in_sample,
            before.held_out.statistic
        );
        println!(
            "    10/50/90%: engine {:.3?}, class {:.3?}, scratch {:.3?}",
            quantiles(&pick(&held)),
            quantiles(&drawn),
            quantiles(&scratch.iter().map(|e| (e.0[k], 1.0)).collect::<Vec<_>>())
        );
        let pinned = MERGER_MISSES.iter().find(|m| m.0 == k);
        let held_as_pinned = pinned.is_some_and(|m| (r.held_out.statistic - m.1).abs() < 1e-6);
        if (pinned.is_none() && !r.passes()) || (pinned.is_some() && !held_as_pinned) {
            misses.push(format!(
                "{name}: D {} against {} + {}",
                r.held_out.statistic, r.critical, r.in_sample
            ));
        }
    }
    assert!(misses.is_empty(), "{misses:#?}");
}

/// The weighted 10th, 50th and 90th percentiles of `values`, `(value, weight)` pairs.
fn quantiles(values: &[(f64, f64)]) -> [f64; 3] {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = sorted.iter().map(|v| v.1).sum();
    [0.1, 0.5, 0.9].map(|p| {
        let mut below = 0.0;
        for &(v, w) in &sorted {
            below += w;
            if below >= p * total {
                return v;
            }
        }
        sorted.last().map_or(f64::NAN, |v| v.0)
    })
}

/// The stellar merger marks whose test misses at 1%, each a finding (plan 15's records): the mark's
/// place in [`MERGER_COMPARED`] and its held-out distance, as measured to a millionth (2026-11-05).
/// Every one misses, the fitted half's distance as far as the held-out one's: the class's mergers
/// are older than the engine's (log age's median 9.80 against 9.15), since it draws one age rank
/// over every component and the rows give one rate a system to a population, where the engine's
/// mergers lie in its younger sub-discs, and so they are lighter (the primary's median 1.17 M☉
/// against 1.85) with lighter companions under the lower turn-off. The scratch class's distances
/// were 0.629, 0.223, 0.606 and 0.503.
const MERGER_MISSES: [(usize, f64); 4] = [
    (0, 0.525_064_647_661_472_9),
    (1, 0.346_258_297_103_203_3),
    (2, 0.468_483_474_837_497_64),
    (3, 0.482_128_431_390_944_2),
];

/// What the neutron-star merger test compares of a merger: each neutron star's mass at the merger,
/// log₁₀ of a black hole's, and log₁₀ of the merger age, years.
const NSM_COMPARED: [&str; 3] = [
    "neutron star mass (M_sun)",
    "log black hole mass (M_sun)",
    "log merger age (yr)",
];

/// A neutron-star merger's compared values, mark by mark, of its kind's compact masses and merger
/// age `t`: each neutron star's mass, its black hole's log mass, and its log age.
fn nsm_values(neutron: &[f64], hole: Option<f64>, t: f64) -> [Vec<f64>; 3] {
    [
        neutron.to_vec(),
        hole.map(math::log10).into_iter().collect(),
        vec![math::log10(t)],
    ]
}

/// The class against the engine at 31 (P15.T10.b's merger step; slow): the class's entries in the
/// neutron-star merger cells about the solar circle against the engine's mergers of a neutron star
/// with a neutron star or a black hole, weighted as mergers in the horizon, at 1%, a miss a
/// finding pinned in [`NSM_MISSES`]; the pair kinds' shares printed.
#[test]
#[ignore = "slow: two censuses of 8 × 10⁶ records of layers D and E through the binary engine"]
fn the_neutron_star_merger_sampler_against_the_engine() {
    let galaxy = census_galaxy();
    let read =
        |m: &MergerMember| NsmReading::of(galaxy, m).filter(|r| r.kind.is_some() && r.weight > 0.0);
    let [held_out, fitted_half] = [FITTED_PARTS..CENSUS_PARTS, 0..FITTED_PARTS].map(|parts| {
        census_half(
            CarvedClass::NeutronStarMerger,
            (NSM_SLAB, NSM_RECORDS),
            parts,
            read,
        )
    });
    let cells = [[-1, 6, -1], [0, 6, -1], [-1, 6, 0], [0, 6, 0]].map(|at| {
        CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, at).expect("a cell of the root cube")
    });
    let process = NeutronStarMergers::new(fitted());
    let class: Vec<([Vec<f64>; 3], NsmPairKind)> =
        entries(fitted(), &process, &cells, ENTRIES, |e| {
            let scan = e.scan();
            let [first, second] = scan.masses().map(crate::units::SolarMasses::value);
            let (neutron, hole) = match scan.kind() {
                NsmPairKind::DoubleNeutronStar => (vec![first, second], None),
                NsmPairKind::NeutronStarBlackHole => (vec![second], Some(first)),
            };
            Some((
                nsm_values(&neutron, hole, scan.merger_age().value()),
                scan.kind(),
            ))
        });
    let engine = |half: &[NsmReading]| -> Vec<([Vec<f64>; 3], f64)> {
        half.iter()
            .map(|r| {
                let neutron: Vec<f64> = r.neutron_star_masses.into_iter().flatten().collect();
                (
                    nsm_values(&neutron, r.black_hole_mass, r.merger_age),
                    r.weight,
                )
            })
            .collect()
    };
    let (held, fit) = (engine(&held_out), engine(&fitted_half));
    println!(
        "{} held-out and {} fitted engine mergers, {} entries",
        held_out.len(),
        fitted_half.len(),
        class.len()
    );
    let mut misses = Vec::new();
    for (k, name) in NSM_COMPARED.iter().enumerate() {
        let drawn: Vec<(f64, f64)> = class
            .iter()
            .flat_map(|e| e.0[k].iter().map(|&v| (v, 1.0)))
            .collect();
        let pick = |values: &[([Vec<f64>; 3], f64)]| -> Vec<(f64, f64)> {
            values
                .iter()
                .flat_map(|v| v.0[k].iter().map(move |&x| (x, v.1)))
                .collect()
        };
        if drawn.is_empty() || pick(&held).is_empty() {
            println!("  {name}: no values on one side");
            continue;
        }
        let r = Resolved::of(&drawn, &pick(&held), &pick(&fit));
        println!(
            "  {name}: D {} (p {:.3e}) against {:.6} + in-sample {:.6}",
            r.held_out.statistic, r.held_out.p_value, r.critical, r.in_sample
        );
        let pinned = NSM_MISSES.iter().find(|m| m.0 == k);
        let held_as_pinned = pinned.is_some_and(|m| (r.held_out.statistic - m.1).abs() < 1e-6);
        if (pinned.is_none() && !r.passes()) || (pinned.is_some() && !held_as_pinned) {
            misses.push(format!(
                "{name}: D {} against {} + {}",
                r.held_out.statistic, r.critical, r.in_sample
            ));
        }
    }
    assert!(misses.is_empty(), "{misses:#?}");
}

/// The neutron-star merger marks whose test misses at 1%, each a finding: the mark's place in
/// [`NSM_COMPARED`] and its held-out distance, as measured to a millionth (2026-11-07, v29). The
/// log merger age misses, the fitted half's distance (0.718) as far as the held-out one's: the
/// class's ages are not the engine's, as the stellar mergers' are not ([`MERGER_MISSES`]), since
/// it draws one age rank over every component and its row gives one rate a system to a
/// population. The masses pass: each neutron star's (0.040 against 0.053) and the black hole's
/// (0.075 against 0.086).
const NSM_MISSES: [(usize, f64); 1] = [(2, 0.728_585_729_481_149)];

/// The classes at Milky Way parameters under every 31 hold (slow): over ten Milky Way seeds, the
/// stellar mergers' red novae, the mergers in the source horizon over its span, inside the
/// brainstorm's 0.2–0.5 a year, and the neutron-star mergers' entries about ten, inside 5–20.
#[test]
#[ignore = "slow: ten galaxies built"]
fn the_fitted_merger_rows_give_the_milky_way_counts() {
    let horizon = horizon_years();
    for k in 0..10_u64 {
        let galaxy = at_31(Seed::new(0x0b17_08a0_0000_0100 | k));
        let stellar = StellarMergers::new(&galaxy).hosts().expected_hosts(&galaxy);
        let neutron = NeutronStarMergers::new(&galaxy)
            .hosts()
            .expected_hosts(&galaxy);
        let a_year = stellar / horizon;
        println!(
            "seed {k}: {stellar:.0} stellar mergers, {a_year:.3} a year; {neutron:.2} neutron-star mergers"
        );
        assert!((0.2..=0.5).contains(&a_year), "{a_year}");
        assert!((5.0..=20.0).contains(&neutron), "{neutron}");
    }
}
