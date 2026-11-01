//! The tests of P11.T6's hold, [`IA_EXPLOSION_VERSION`]: a grid binary whose pooled Type Ia
//! candidate its explosion mark explodes at or before +H is redrawn, from version 27, through one
//! test, `SystemStars::exploded_pair`, which the system stage's redraw and P09.T35's hook
//! (`displaced::site::keeps_a_recent_type_ia`) read. Each fast test turns this hold's arm on alone
//! ([`marked_rules`]), so that the other holds move nothing it holds equal.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Under the arm a pinned layer-D system, whose first attempt's two white dwarfs merge at
//!   1.83 Gyr, a pooled candidate its mark explodes, is redrawn and keeps no Type Ia; under the
//!   held arm it keeps its first attempt. The cell's other systems are the same under both arms.
//!   Each attempt is reproducible alone.
//! - Slow, under every 27 hold on a Kroupa galaxy: over 10⁵ layer-D systems near the Sun none
//!   keeps an exploded binary, and exploded ÷ pooled is η to Poisson accuracy; over the galaxy's
//!   systems the share of layer D whose first attempt explodes, the grid's redraw rate, lies within
//!   0.3–4.5%, and the pool's rate over the Type Ia rate is recorded against the observed five to
//!   seven.
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's,
//! the server's and the fitting crate's tests and 31 slow tests of the binaries, the systems and
//! the bounds (2026-11-01): no golden; one fitted table, `sky_binary_light_c`, whose fingerprint
//! probe moves (`the_committed_tables_are_fresh`), so the bump refits the sky's binary-light
//! tables; and this module's tests. No census of the binaries or the systems moves out of its
//! bounds (`binary_carve`, `binary_system`, `binary_classes`, `binary_mergers`, `binary_awd`,
//! `binary_xrb`, `stellar_system`, `stellar_briefs`), and the hierarchy bound and the census's
//! star bound hold.

use std::sync::OnceLock;

use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::displaced::site::keeps_a_recent_type_ia;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::rng::Seed;
use crate::stellar::binary::IaPoolChannel;
use crate::stellar::multiplicity::{NodeIndex, RedrawAttempt};
use crate::stellar::system::{SystemStars, draw_metallicity, grid_multiplicity, primary_model};
use crate::time::{ClockWindow, UniverseTime};

/// The seed of the Milky Way fixture the fast tests share.
const SEED: u64 = 0x0b17_0006_0000_0003;

/// The pinned system's cell and candidate index: a layer-D record of 2.54 M☉ and 1.93 Gyr whose
/// first attempt's two white dwarfs, of 0.677 and 0.293 M☉, merge at 1.83 Gyr, a pooled candidate
/// its mark explodes (found among the first 2,245 layer-D systems of the cells along x).
const PINNED: ([i32; 3], usize) = ([24, 406, 0], 0);

/// The version's galaxy rules.
fn version() -> GalaxyRules {
    GalaxyRules::of_version(GENERATOR_VERSION)
}

/// The version's rules with this hold's arm on alone.
fn marked_rules() -> GalaxyRules {
    let version = version();
    GalaxyRules {
        stellar: StellarRules {
            ia_explosion: IaExplosionRules::Marked,
            ..version.stellar
        },
        ..version
    }
}

/// The Milky Way fixture, under the version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// [`held`]'s galaxy with this hold's arm on.
fn marked() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held().clone().with_rules(marked_rules()))
}

/// The records of the pinned cell in `galaxy`.
fn pinned_cell(galaxy: &Galaxy) -> Vec<SystemRecord> {
    let mut cell = Vec::new();
    generate_cell(
        galaxy,
        CellKey::new(Layer::D, PINNED.0).expect("a cell of the root cube"),
        &mut cell,
    );
    cell
}

/// The system of `record` at `attempt` in `galaxy`, from its primary's model.
fn at(galaxy: &Galaxy, record: &SystemRecord, attempt: RedrawAttempt) -> SystemStars {
    let ctx = grid_multiplicity(record);
    let (primary, _) = primary_model(galaxy, record, draw_metallicity(galaxy, record));
    SystemStars::at_attempt(galaxy, record, &primary, ctx, attempt)
}

/// P11.T6 is held until the 26 → 27 bump: the rules in force are version 26's and before, which
/// read no explosion mark, and every 27 hold reads it.
#[test]
fn the_type_ia_explosion_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < IA_EXPLOSION_VERSION,
        "the bump to 27 puts P11.T6 on the record: a grid binary whose pooled Type Ia candidate \
         explodes at or before +H redraws. Delete `IA_EXPLOSION_VERSION`, `IaExplosionRules` with \
         its `Pooled` arm, `StellarRules::ia_explosion`, its `match` in \
         `SystemStars::exploded_pair` and this module, in the bump's own commit, and re-pin the \
         goldens its records list"
    );
    let at = |v| StellarRules::of_version(GeneratorVersion::new(v)).ia_explosion;
    assert_eq!(at(24), IaExplosionRules::Pooled);
    assert_eq!(at(26), IaExplosionRules::Pooled);
    assert_eq!(at(27), IaExplosionRules::Marked);
    assert_eq!(
        StellarRules::every_26_hold().ia_explosion,
        IaExplosionRules::Pooled
    );
    assert_eq!(
        StellarRules::every_27_hold().ia_explosion,
        IaExplosionRules::Marked
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar,
        StellarRules::every_27_hold()
    );
    assert_eq!(held().rules(), version());
}

/// Under the arm the pinned system's first attempt has a pair of white dwarfs whose merger at
/// 1.83 Gyr its mark explodes, so the grid redraws it and keeps an attempt with no Type Ia, the
/// primary unchanged; P09.T35's hook says so of both. Under the held arm the same first attempt
/// explodes nothing and is kept. Every other system of the cell is the same under both arms.
#[test]
fn under_the_arm_the_pinned_system_redraws_its_exploded_binary() {
    let (held_cell, marked_cell) = (pinned_cell(held()), pinned_cell(marked()));
    assert_eq!(held_cell, marked_cell, "placement reads no explosion mark");
    let record = &marked_cell[PINNED.1];
    let first = at(marked(), record, RedrawAttempt::FIRST);
    let (node, event) = first
        .exploded_pair()
        .expect("the pinned first attempt explodes");
    assert_eq!(node, NodeIndex::ROOT);
    assert_eq!(event.channel(), IaPoolChannel::Merger);
    assert!((event.age().value() - 1.83e9).abs() < 1e7, "{event:?}");
    assert!(keeps_a_recent_type_ia(&first));
    let kept = SystemStars::generate(marked(), record);
    assert!(kept.attempt() > RedrawAttempt::FIRST);
    assert_eq!(kept.exploded_pair(), None);
    assert_eq!(kept.carved_pair(), None);
    assert!(!keeps_a_recent_type_ia(&kept));
    assert_eq!(
        kept.primary(),
        first.primary(),
        "the primary is never redrawn"
    );
    let unmarked = SystemStars::generate(held(), record);
    assert_eq!(unmarked.attempt(), RedrawAttempt::FIRST);
    assert_eq!(unmarked.exploded_pair(), None);
    assert!(!keeps_a_recent_type_ia(&unmarked));
    assert_eq!(unmarked.hierarchy(), first.hierarchy());
    // The systems carry their rules; their attempts and states, which the record and every
    // reader see, are the same.
    for (i, record) in marked_cell.iter().enumerate() {
        if i != PINNED.1 {
            let [a, b] = [marked(), held()].map(|galaxy| SystemStars::generate(galaxy, record));
            assert_eq!(a.attempt(), b.attempt(), "{:?}", record.id());
            assert_eq!(a.hierarchy(), b.hierarchy(), "{:?}", record.id());
            for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
                assert_eq!(a.state_at(t), b.state_at(t), "{:?} at {t}", record.id());
            }
        }
    }
}

/// Attempt n of the pinned system, its pairs and their explosion marks, is a pure function of the
/// record and n, whatever was drawn before it; and the kept attempt is the one `generate` keeps.
#[test]
fn attempt_n_is_reproducible_alone() {
    let record = &pinned_cell(marked())[PINNED.1];
    let attempts: Vec<RedrawAttempt> = RedrawAttempt::all().collect();
    assert_order_independent(&attempts, |&attempt| {
        let stars = at(marked(), record, attempt);
        (stars.exploded_pair(), stars.hierarchy().clone())
    });
    let kept = SystemStars::generate(marked(), record);
    assert_eq!(at(marked(), record, kept.attempt()), kept);
}

/// The slow tests, compiled out on WebAssembly, which has no threads to run their 2.4 × 10⁵ systems'
/// pairs on.
#[cfg(not(target_family = "wasm"))]
mod slow {
    use std::collections::BTreeSet;

    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::stats::assert_poisson_count;
    use hyperion_testkit::threads::in_shares;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::catalogue_classes::type_ia::DelayTimeDistribution;
    use crate::galaxy::fields::Component;
    use crate::galaxy::imf::{BandShares, MASS_BAND_EDGES, MassBand, MassFunctionKind};
    use crate::galaxy::placement::SystemOrigin;
    use crate::stellar::binary::carve::grid_redraws;
    use crate::stellar::binary::ia::{IaExplosionMark, marked_explosion};
    use crate::stellar::testing::reference_position;
    use crate::tables::binary::IA_YIELD;
    use crate::units::SolarMasses;

    /// The seed of the slow tests' Kroupa galaxy, and of their choice of records.
    const SLOW_SEED: u64 = 0x0b17_0006_0000_0004;

    /// The shares the slow tests' records are run in, on as many threads of their own
    /// (`.config/nextest.toml` reserves them).
    const SHARES: usize = 16;

    /// The slow tests' galaxy: drawn under Kroupa's mass function, built under every 27 hold.
    fn kroupa() -> Galaxy {
        let seed = Seed::new(SLOW_SEED);
        Galaxy::from_params_with_rules(
            seed,
            GalaxyParams::from_seed(seed, MassFunctionKind::Kroupa),
            GalaxyRules::every_27_hold(),
        )
        .expect("a drawn galaxy builds")
    }

    /// The records of distinct cells of `layer` drawn at random within 4,000 ly of the Sun-like
    /// point along x, 2,000 ly along y and 1,000 ly of the plane, every record of each cell, so
    /// that the sample is the volume's, until there are at least `n`.
    fn records_of(galaxy: &Galaxy, layer: Layer, n: usize, salt: u64) -> Vec<SystemRecord> {
        let mut rng = Lcg::new(SLOW_SEED ^ salt);
        let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
        let half = [4_000 / size, 2_000 / size, (1_000 / size).max(1)];
        let cells: usize = half
            .iter()
            .map(|&h| usize::try_from(2 * h + 1).expect("a small count"))
            .product();
        let mut pick = |h: i32| {
            let width = u64::try_from(2 * h + 1).expect("a small width");
            i32::try_from(rng.next_below(width)).expect("a small index") - h
        };
        let (mut seen, mut cell) = (BTreeSet::new(), Vec::new());
        let mut records = Vec::with_capacity(n + 1_024);
        while records.len() < n {
            assert!(
                seen.len() < cells,
                "layer {layer:?}'s volume holds under {n} records"
            );
            let at = [pick(half[0]), 26_000 / size + pick(half[1]), pick(half[2])];
            if seen.insert(at) {
                let key = CellKey::new(layer, at).expect("a cell of the root cube");
                generate_cell(galaxy, key, &mut cell);
                records.append(&mut cell);
            }
        }
        records
    }

    /// `n` grid records of `layer` (mass band `band`) of the galaxy as a whole, built from their
    /// parts: each of a density component chosen by its share of the galaxy's systems, its
    /// primary's initial mass drawn from the mass function in the band and its age from the
    /// component's ages, born by the epoch, at its population's reference position
    /// ([`reference_position`]), and keyed as candidate i of one cell of the layer. So the sample
    /// is the galaxy's systems of the band, every population by its count.
    fn galaxy_wide(
        galaxy: &Galaxy,
        layer: Layer,
        band: MassBand,
        n: usize,
        salt: u64,
    ) -> Vec<SystemRecord> {
        let mut rng = Lcg::new(SLOW_SEED ^ salt);
        let fields = galaxy.fields();
        let components = fields.components();
        let total: f64 = components.iter().map(Component::count).sum();
        let (lo, hi) = (
            MASS_BAND_EDGES[band.index()],
            MASS_BAND_EDGES[band.index() + 1],
        );
        let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
        let key = CellKey::new(layer, [0, 26_000 / size, 0]).expect("a cell of the root cube");
        let mut records = Vec::with_capacity(n);
        let mut index = 0;
        while records.len() < n {
            let mut pick = rng.next_f64() * total;
            let c = components
                .iter()
                .position(|component| {
                    pick -= component.count();
                    pick < 0.0
                })
                .unwrap_or(components.len() - 1);
            let mass = galaxy.mass_function().quantile_in(lo, hi, rng.next_f64());
            let age = components[c].ages().quantile(rng.next_f64());
            if age.value() <= 0.0 {
                continue;
            }
            let population = components[c].population();
            let at = reference_position(population);
            records.push(SystemRecord::from_parts(
                key.candidate_id(index).expect("under the cell's capacity"),
                GalacticPosition::from_light_years([at.x, at.y, at.z]).expect("inside the cube"),
                SystemOrigin::Grid(fields.component_id(c).expect("a component of the galaxy")),
                population,
                SolarMasses::new(mass),
                age,
            ));
            index += 1;
        }
        records
    }

    /// What a layer's records come to.
    #[derive(Debug, Default, Clone, Copy)]
    struct IaTally {
        /// Records the grid redraws, of a grid origin under their own multiplicity.
        grid: u64,
        /// Pairs of the first attempts with a pooled candidate, mergers then accretions.
        pooled: [u64; 2],
        /// Those of them whose explosion mark explodes them.
        exploded: [u64; 2],
        /// Grid records whose first attempt explodes, which the grid redraws for it.
        redrawn: u64,
        /// Grid records whose first attempt has a carved pair and none exploded.
        carved: u64,
        /// Kept systems with an exploded pair, which must be none.
        kept_exploded: u64,
    }

    impl IaTally {
        /// The tally of `galaxy`'s `records`.
        fn of(galaxy: &Galaxy, records: &[SystemRecord]) -> Self {
            let mut tally = Self::default();
            for record in records {
                let ctx = grid_multiplicity(record);
                if !grid_redraws(record, ctx) {
                    continue;
                }
                tally.grid += 1;
                let kept = SystemStars::generate_in(galaxy, record, ctx);
                if kept.exploded_pair().is_some() {
                    eprintln!("{:?} keeps an exploded pair", record.id());
                    tally.kept_exploded += 1;
                }
                let first = if kept.attempt() == RedrawAttempt::FIRST {
                    kept
                } else {
                    at(galaxy, record, RedrawAttempt::FIRST)
                };
                let exploded = first.exploded_pair().is_some();
                tally.redrawn += u64::from(exploded);
                tally.carved += u64::from(!exploded && first.carved_pair().is_some());
                for pair in first.pairs() {
                    let Some(event) = pair.timeline().pooled_ia() else {
                        continue;
                    };
                    let c = match event.channel() {
                        IaPoolChannel::Merger => 0,
                        IaPoolChannel::Accretion => 1,
                    };
                    tally.pooled[c] += 1;
                    let key = first
                        .hierarchy()
                        .pair_key(pair.node())
                        .expect("a pair's node has a key");
                    let mark = IaExplosionMark::of(galaxy.seed(), key, RedrawAttempt::FIRST);
                    let marked = marked_explosion(pair.timeline(), mark, &IA_YIELD);
                    tally.exploded[c] += u64::from(marked.is_some());
                }
            }
            tally
        }

        fn add(mut self, other: Self) -> Self {
            self.grid += other.grid;
            for c in 0..2 {
                self.pooled[c] += other.pooled[c];
                self.exploded[c] += other.exploded[c];
            }
            self.redrawn += other.redrawn;
            self.carved += other.carved;
            self.kept_exploded += other.kept_exploded;
            self
        }
    }

    /// A count as an `f64`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts of at most 10⁵, which an f64 holds exactly"
    )]
    fn count_f64(n: u64) -> f64 {
        n as f64
    }

    /// The tally of `galaxy`'s `records`, in [`SHARES`] shares on threads of their own: share k
    /// the records k, k + 16, …, so the counts are the same on any number of threads.
    fn tally(galaxy: &Galaxy, records: &[SystemRecord]) -> IaTally {
        in_shares(SHARES, SHARES, |k| {
            let mine: Vec<SystemRecord> = records.iter().skip(k).step_by(SHARES).copied().collect();
            IaTally::of(galaxy, &mine)
        })
        .into_iter()
        .fold(IaTally::default(), IaTally::add)
    }

    /// Prints `t`, the tally of `records` of `layer` from `sample`, and asserts that none was kept
    /// with an exploded binary and that exploded ÷ pooled is η, a sixth, to Poisson accuracy.
    fn check(sample: &str, layer: Layer, records: &[SystemRecord], t: &IaTally) {
        let (pooled, exploded) = (t.pooled[0] + t.pooled[1], t.exploded[0] + t.exploded[1]);
        println!(
            "{sample}, layer {layer:?}: {} records, {} redrawn by the grid; {pooled} pooled \
             candidates ({} mergers, {} accretions), {exploded} exploded ({} and {}); first \
             attempts redrawn for a Type Ia {} ({:.3}%), for a carved class alone {}; kept with an \
             exploded pair {}",
            records.len(),
            t.grid,
            t.pooled[0],
            t.pooled[1],
            t.exploded[0],
            t.exploded[1],
            t.redrawn,
            100.0 * count_f64(t.redrawn) / count_f64(t.grid),
            t.carved,
            t.kept_exploded
        );
        assert_eq!(
            t.kept_exploded, 0,
            "{sample}: layer {layer:?} keeps an exploded binary"
        );
        assert_poisson_count(
            &format!("{sample}: layer {layer:?}'s exploded candidates"),
            exploded,
            count_f64(pooled) / 6.0,
            1e-3,
        );
    }

    /// P11.T6 under every 27 hold, on a drawn Kroupa galaxy with the scratch yield, over 10⁵ grid
    /// records of layer D, every record of cells drawn at random near the Sun-like point: no system
    /// the grid keeps has an exploded binary at any t ≤ +H, and exploded ÷ pooled is η to Poisson
    /// accuracy. The share whose first attempt explodes is printed: the solar neighbourhood's
    /// redraw rate, 271 of 99,890, 0.271%, on 2026-11-01 (the galaxy's, which the plan's band
    /// bounds, is the next test's).
    #[test]
    #[ignore = "slow: 10⁵ layer-D systems near the Sun under every 27 hold, their pairs run"]
    fn under_every_27_hold_no_grid_system_keeps_an_exploded_binary() {
        let galaxy = kroupa();
        let records = records_of(&galaxy, Layer::D, 100_000, 0);
        let t = tally(&galaxy, &records);
        check("near the Sun", Layer::D, &records, &t);
        assert!(t.grid >= 95_000, "{} layer-D grid records", t.grid);
    }

    /// P11.T6's rates at the galaxy's parameters, under every 27 hold on a drawn Kroupa galaxy with
    /// the scratch yield, over 10⁵ records of layer D, 3 × 10⁴ of C and 5,000 of E of the galaxy as
    /// a whole ([`galaxy_wide`]): the share of layer D whose first attempt explodes, the grid's
    /// Type Ia redraw rate, lies within 0.3–4.5% (the brainstorm's 2–4% once P11.T15 lands);
    /// exploded ÷ pooled is η to Poisson accuracy and none is kept; and the pool's candidates to
    /// date per solar mass formed, over the delay-time distribution's Type Ia to the same records'
    /// ages, is recorded against the observed five to seven for the double white dwarf mergers
    /// (Maoz, Hallakoun and Badenes 2018, MNRAS 476, 2584; plan 15 accepts 4.5–7). Layers C to E
    /// are weighed by the mass function's band shares; A and B, below 0.75 M☉, make no white dwarf
    /// within a Hubble time.
    ///
    /// Measured on 2026-11-01: 333 of layer D's 10⁵ redrawn, 0.333%, near the band's floor; 344
    /// of 1,940 candidates exploded; the pool 0.25 times the Type Ia for the mergers and 0.66 for
    /// every candidate, so no η of at most 1 brings the grid's explosions to the observed rate
    /// (plan 11's Risks, "Deviations in P11.T6, as built").
    #[test]
    #[ignore = "slow: 1.35 × 10⁵ systems of the galaxy under every 27 hold, their pairs run"]
    fn under_every_27_hold_the_galaxy_s_type_ia_redraw_rate_is_in_its_band() {
        let galaxy = kroupa();
        let shares = BandShares::of(galaxy.mass_function());
        let strata = [
            (Layer::C, MassBand::C, 30_000),
            (Layer::D, MassBand::D, 100_000),
            (Layer::E, MassBand::E, 5_000),
        ];
        let (mut per_formed, mut ia, mut d_rate) = ([0.0; 2], 0.0, 0.0);
        for (salt, &(layer, band, n)) in (1_u64..).zip(&strata) {
            let records = galaxy_wide(&galaxy, layer, band, n, salt);
            let t = tally(&galaxy, &records);
            check("galaxy-wide", layer, &records, &t);
            let all = count_f64(u64::try_from(records.len()).expect("a count"));
            for (sum, pooled) in per_formed.iter_mut().zip(t.pooled) {
                *sum += shares.share(band) * count_f64(pooled) / all;
            }
            if layer == Layer::D {
                d_rate = count_f64(t.redrawn) / count_f64(t.grid);
                // Summed in the records' order, so the figure is the same on any threads.
                let sum: f64 = records
                    .iter()
                    .map(|r| DelayTimeDistribution::cumulative_per_solar_mass(r.age_at_epoch()))
                    .sum();
                ia = sum / all;
            }
        }
        let formed = galaxy.mean_formed_mass().value();
        let (mergers, both) = (
            per_formed[0] / formed,
            (per_formed[0] + per_formed[1]) / formed,
        );
        println!(
            "the pool to date: {mergers:.3e} double white dwarf mergers and {both:.3e} candidates \
             per M☉ formed, against {ia:.3e} Type Ia: {:.2} and {:.2} times the Type Ia \
             (observed: five to seven for the mergers)",
            mergers / ia,
            both / ia
        );
        assert!(ia > 0.0 && mergers > 0.0);
        assert!(
            (0.003..=0.045).contains(&d_rate),
            "layer D's Type Ia redraw rate {d_rate} lies outside 0.3–4.5%"
        );
    }
}
