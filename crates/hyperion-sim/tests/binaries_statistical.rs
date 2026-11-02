//! Plan 11's P11.T12, slow: the binaries' statistical suite, read from generated systems at Milky
//! Way parameters under every hold of generator version 27, galaxy and stellar.
//!
//! Each sample is of grid records that placement generates in a box of cells about the solar
//! circle's point 26,000 ly out along +y, each layer's cells visited in a fixed scattered order and
//! shared out in [`PARTS`] fixed parts, so that a sample is the same on any number of threads. A
//! record counts its layer's share of the galaxy's systems over the records examined in that
//! layer, so a sum over all five layers is one over the galaxy's systems there. The multiplicity
//! tests read each record's hierarchy at its first attempt, as the system stage draws it
//! ([`draw_hierarchy`] with [`grid_multiplicity`]'s context); the grid's redraws (P11.T7, about
//! 10⁻⁴ of systems) read the engine and are left out, so these tests read the multiplicity model
//! (P11.T1–T2) alone.
//!
//! - Multiplicity by primary mass, in seven bins, one per anchor of the model as ruling 74 left
//!   it: χ² of the drawn multiples against the model, and of the living primaries as drawn against
//!   the surveys the anchors come from.
//! - The companion counts of Sun-like primaries against Raghavan et al.'s (2010) 56 : 33 : 8 : 3.
//! - Kolmogorov–Smirnov tests of the first companion's period and mass ratio, for primaries the
//!   spine construction draws, against the model's laws, and the Sun-like period peak.
//! - All stars below 0.5 M☉ against the 20 pc census's 69%, under the default mass function, with
//!   Chabrier's as published and Kroupa's recorded.
//!
//! The engine's tests, to re-run once its fixes land, read [`SystemStars`] with every pair run
//! through the binary engine:
//!
//! - blue stragglers and hot subdwarfs in the old populations, against plan 06's class-fraction
//!   figures and the measured specific frequencies;
//! - the recycled pulsars' fields by companion, and the double neutron stars' merger rate against
//!   Pol et al. (2019);
//! - a Hertzsprung–Russell table of an old open cluster with its binaries, for the eye.

#![cfg(feature = "testing")]

use std::fmt::Write as _;
use std::num::NonZeroU32;

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::displaced::{DisplacedKind, PlacementClass};
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
use hyperion_sim::galaxy::features::cluster::ClusterModel;
use hyperion_sim::galaxy::features::members::{FeatureMemberSource, KeepInteriors};
use hyperion_sim::galaxy::imf::{
    BandShares, Chabrier, Kroupa, MASS_LIMIT_HI, MASS_LIMIT_LO, MassBand, MassFunction, Truncated,
};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CellKey, NoCache, SystemOrigin, SystemRecord, generate_cell,
};
use hyperion_sim::galaxy::query::{RangeQuery, range_query};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::observe::stars_of;
use hyperion_sim::orbit::peters_merger_time;
use hyperion_sim::stellar::Phase;
use hyperion_sim::stellar::binary::{
    AGE_OF_UNIVERSE, BinaryClass, BinaryTimeline, Component, HOT_SUBDWARF_MASSES, SupernovaRecord,
};
use hyperion_sim::stellar::multiplicity::{
    HierarchyNode, MAX_COMPANIONS, MultiplicityContext, MultiplicityModel, RedrawAttempt, SlotKind,
    StarIndex, SystemHierarchy, all_stars_fraction_below_as_drawn, draw_hierarchy,
};
use hyperion_sim::stellar::remnant::RemnantKind;
use hyperion_sim::stellar::system::{PairState, SystemStars, grid_multiplicity};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{Days, LightYears, SolarMasses, Years};
use hyperion_sim::{GeneratorVersion, Seed};
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample, regularised_gamma_q};
use hyperion_testkit::threads::in_shares;

/// The Milky Way fixture's seed.
const SEED: u64 = 0x0b17_0012_5eed_0001;

/// The generator version whose holds the suite runs under: every hold of 27, galaxy and stellar.
const ARM: u32 = 27;

/// The parts a layer's sample is shared out in, each with its own cells.
const PARTS: usize = 64;

/// The threads the suite's tests run their parts on: each reserves as many slots
/// (`.config/nextest.toml`).
const THREADS: usize = 8;

/// A box of cells: its centre and half its extent, ly along x, y and z of the galactic frame.
#[derive(Debug, Clone, Copy)]
struct Region {
    centre_ly: [i32; 3],
    half_ly: [i32; 3],
}

/// The solar circle's point, 26,000 ly out along +y, ly.
const SOLAR_POINT_LY: [i32; 3] = [0, 26_000, 0];

/// The local box: 8,000 ly along the solar circle, 4,000 ly across it and 64 ly either side of the
/// plane, where the stars are the mid-plane's.
const LOCAL: Region = Region {
    centre_ly: SOLAR_POINT_LY,
    half_ly: [4_000, 2_000, 64],
};

/// The wide box the living primaries of layer D are sought in: 32,000 ly along the solar circle,
/// 8,000 ly across it and 320 ly either side of the plane, which holds most young stars (their
/// scale height is about 300 ly).
const WIDE: Region = Region {
    centre_ly: SOLAR_POINT_LY,
    half_ly: [16_000, 4_000, 320],
};

/// The box layer E's living primaries are sought in: 64,000 ly along the solar circle, 16,000 ly
/// across it and 320 ly either side of the plane. Living stars of 9 M☉ and more that no class has
/// displaced are about one layer-E record in ten thousand there.
const LAYER_E_WIDE: Region = Region {
    centre_ly: SOLAR_POINT_LY,
    half_ly: [32_000, 8_000, 320],
};

/// A prime above every box's cell count, whose multiples visit a box's cells in a scattered order.
const CELL_STRIDE: u128 = 2_305_843_009_213_693_951;

/// The `galaxy` of [`SEED`] under every hold of [`ARM`].
fn milky_way() -> Galaxy {
    Galaxy::from_params_at_version(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GeneratorVersion::new(ARM),
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
}

/// `f` of each of the first `examined` records of `layer`'s cells in `region`, kept where it is
/// `Some`: [`layer_map_where`] examining every record.
fn layer_map<T: Send>(
    galaxy: &Galaxy,
    layer: Layer,
    region: Region,
    examined: usize,
    f: impl Fn(&SystemRecord) -> Option<T> + Sync,
) -> Vec<T> {
    layer_map_where(galaxy, layer, region, examined, |_| true, f)
}

/// `f` of each of the first `examined` records of `layer`'s cells in `region` that `select`
/// examines, kept where it is `Some`.
///
/// The box's cells are visited in the order `k × CELL_STRIDE mod cells`, so that no cell is read
/// twice and neighbouring records lie far apart. Part p of [`PARTS`] takes the cells of `k ≡ p`
/// and examines its share of the records, every selected record of each cell in turn; the parts
/// run on [`THREADS`] threads and come back in order.
///
/// # Panics
///
/// If a part's cells hold fewer selected records than its share.
fn layer_map_where<T: Send>(
    galaxy: &Galaxy,
    layer: Layer,
    region: Region,
    examined: usize,
    select: impl Fn(&SystemRecord) -> bool + Sync,
    f: impl Fn(&SystemRecord) -> Option<T> + Sync,
) -> Vec<T> {
    let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
    let half = region.half_ly.map(|h| (h / size).max(1));
    let low: [i32; 3] = std::array::from_fn(|axis| region.centre_ly[axis] / size - half[axis]);
    let span = half.map(|h| u128::try_from(2 * h).expect("a positive span"));
    let cells = span[0] * span[1] * span[2];
    let parts = u128::try_from(PARTS).expect("a small count");
    let part = |p: usize| {
        let quota = examined / PARTS + usize::from(p < examined % PARTS);
        let mut out = Vec::new();
        let (mut seen, mut cell) = (0, Vec::new());
        let mut k = u128::try_from(p).expect("a small index");
        while seen < quota {
            assert!(
                k < cells,
                "{layer:?}: the box's cells hold fewer than {examined} selected records"
            );
            let j = k * CELL_STRIDE % cells;
            let at = |axis: usize, j: u128| {
                low[axis] + i32::try_from(j % span[axis]).expect("an index inside the box")
            };
            let key = CellKey::new(
                layer,
                [at(0, j), at(1, j / span[0]), at(2, j / (span[0] * span[1]))],
            )
            .expect("a cell of the grid");
            generate_cell(galaxy, key, &mut cell);
            for record in cell.iter().filter(|r| select(r)) {
                if seen == quota {
                    break;
                }
                seen += 1;
                out.extend(f(record));
            }
            k += parts;
        }
        out
    };
    in_shares(PARTS, THREADS, part)
        .into_iter()
        .flatten()
        .collect()
}

/// What a record of `band` counts when `examined` of its layer's records were examined: the
/// band's share of the galaxy's systems over that number.
fn band_weight(galaxy: &Galaxy, band: MassBand, examined: usize) -> f64 {
    BandShares::of(galaxy.mass_function()).share(band) / count(examined)
}

/// A count as an `f64`.
fn count(n: usize) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts of at most a few million, which an f64 holds exactly"
    )]
    let x = n as f64;
    x
}

/// The hierarchy the system stage draws for `record` at its first attempt.
fn hierarchy(galaxy: &Galaxy, record: &SystemRecord) -> SystemHierarchy {
    draw_hierarchy(
        galaxy,
        record,
        grid_multiplicity(record),
        RedrawAttempt::FIRST,
    )
}

/// The stars of `h`, brown dwarfs left out.
fn stellar_count(h: &SystemHierarchy) -> usize {
    h.stars()
        .iter()
        .filter(|s| s.kind() == SlotKind::Star)
        .count()
}

/// The primary's direct companions in `h`, as Moe and Di Stefano (2017, ApJS 230, 15, §9.1) count
/// them: each pair on the path from the root to the primary holds the primary in its inner member,
/// and its outer member's first star orbits the primary directly. Each is its mass ratio to the
/// primary and log₁₀(P ÷ 1 d) of its orbit; brown dwarfs are left out.
fn direct_companions(h: &SystemHierarchy) -> Vec<(f64, f64)> {
    let m1 = h.stars()[0].initial_mass().value();
    let mut out = Vec::new();
    let mut node = h.root();
    while let HierarchyNode::Pair {
        inner,
        outer,
        orbit,
    } = *h.node(node)
    {
        let star = h.star(h.first_star(outer));
        if star.kind() == SlotKind::Star {
            let period = Days::from(orbit.period()).value();
            out.push((star.initial_mass().value() / m1, math::log10(period)));
        }
        node = inner;
    }
    out
}

/// Companion 1, body 1, if it is a star: its initial mass and the period of the orbit of the pair
/// whose outer member it is the first star of, which in the spine construction is the orbit drawn
/// for it.
fn first_companion(h: &SystemHierarchy) -> Option<(SolarMasses, Days)> {
    let first = StarIndex::from_body(1).expect("body 1 is a star's");
    let star = h.stars().get(1)?;
    if star.kind() != SlotKind::Star {
        return None;
    }
    h.nodes().iter().find_map(|node| match *node {
        HierarchyNode::Pair { outer, orbit, .. } if h.first_star(outer) == first => {
            Some((star.initial_mass(), Days::from(orbit.period())))
        }
        _ => None,
    })
}

/// Whether `record`'s primary is living at the epoch: a record of placement class `Alive` is in
/// layers D and E (and outside them its class says nothing of its multiplicity), and a displaced
/// runaway, walkaway or survivor is.
fn living(record: &SystemRecord) -> bool {
    match record.placement_class() {
        PlacementClass::Alive => true,
        PlacementClass::Retained { .. } => false,
        PlacementClass::Displaced { kind, .. } => kind != DisplacedKind::Remnant,
    }
}

/// The survival probability of χ² `statistic` on `dof` degrees of freedom.
fn chi_square_p(statistic: f64, dof: u32) -> f64 {
    regularised_gamma_q(f64::from(dof) / 2.0, statistic / 2.0)
}

// ---------------------------------------------------------------------------------------------
// Multiplicity by primary mass.

/// A survey's figure for one bin of primary mass: the share of primaries with a stellar companion
/// (Duchêne and Kraus 2013, ARA&A 51, 269, Table 1, and Raghavan et al. 2010, ApJS 190, 1), or,
/// from 2 M☉ up, the share with no direct companion of q > 0.1 and log P < 8 (Moe and Di Stefano
/// 2017, ApJS 230, 15, Table 13, `F_n=0;q>0.1`), with its error below and above.
#[derive(Debug, Clone, Copy)]
struct Survey {
    /// The bin's primary masses, M☉.
    masses: (f64, f64),
    /// The anchor of the model the figure sets, M☉.
    anchor: f64,
    /// The figure.
    value: f64,
    /// Its errors below and above.
    error: (f64, f64),
    /// Whether the figure is Moe and Di Stefano's single fraction, counted as they count.
    counted_singles: bool,
}

/// The seven bins, one per anchor of `MultiplicityModel::default_v1` (ruling 74): Duchêne and
/// Kraus's ≲ 0.1, 0.1–0.5 and 0.7–1.3 M☉ (22 (+6 −4)%, 26 ± 3% and Raghavan et al.'s 44 ± 2%), and
/// Moe and Di Stefano's A/late-B, mid-B, early-B and O intervals.
const SURVEYS: [Survey; 7] = [
    Survey {
        masses: (0.08, 0.1),
        anchor: 0.09,
        value: 0.22,
        error: (0.04, 0.06),
        counted_singles: false,
    },
    Survey {
        masses: (0.1, 0.5),
        anchor: 0.25,
        value: 0.26,
        error: (0.03, 0.03),
        counted_singles: false,
    },
    Survey {
        masses: (0.7, 1.3),
        anchor: 1.0,
        value: 0.44,
        error: (0.02, 0.02),
        counted_singles: false,
    },
    Survey {
        masses: (2.0, 5.0),
        anchor: 3.5,
        value: 0.41,
        error: (0.08, 0.08),
        counted_singles: true,
    },
    Survey {
        masses: (5.0, 9.0),
        anchor: 7.0,
        value: 0.24,
        error: (0.08, 0.08),
        counted_singles: true,
    },
    Survey {
        masses: (9.0, 16.0),
        anchor: 12.0,
        value: 0.16,
        error: (0.09, 0.09),
        counted_singles: true,
    },
    Survey {
        masses: (16.0, MASS_LIMIT_HI),
        anchor: 28.0,
        value: 0.06,
        error: (0.06, 0.06),
        counted_singles: true,
    },
];

/// Where each layer's records are drawn for the multiplicity test, and how many are examined:
/// every record of layers A–C in the local box, and the living records of layers D and E in boxes
/// wide enough to hold them.
const MULTIPLICITY_DRAWS: [(Region, usize); 5] = [
    (LOCAL, 400_000),
    (LOCAL, 100_000),
    (LOCAL, 200_000),
    (WIDE, 400_000),
    (LAYER_E_WIDE, 3_000_000),
];

/// The fewest records of placement class `Alive` a bin is compared with its survey on.
const MIN_ALIVE: usize = 100;

/// One draw against the model: the weighted count the draw made multiple, every companion dropped
/// or not, and its expectation `Σ w p` and variance `Σ w² p (1 − p)`, p the model's drawn multiple
/// fraction at the primary's mass.
#[derive(Debug, Clone, Copy, Default)]
struct DrawTally {
    weight: f64,
    drawn: f64,
    expected: f64,
    variance: f64,
}

impl DrawTally {
    fn add(&mut self, w: f64, p: f64, multiple: bool) {
        self.weight += w;
        self.expected += w * p;
        self.variance += w * w * p * (1.0 - p);
        if multiple {
            self.drawn += w;
        }
    }

    /// The miss in standard deviations.
    fn z(&self) -> f64 {
        (self.drawn - self.expected) / self.variance.sqrt()
    }
}

/// Weighted tallies of one bin.
#[derive(Debug, Clone, Copy, Default)]
struct BinTally {
    /// Σ w over the living records of the bin.
    living: f64,
    /// Σ w over the displaced runaways, walkaways and survivors among them.
    displaced: f64,
    /// Records of placement class `Alive`.
    alive_records: usize,
    /// Σ w² over them.
    alive_squares: f64,
    /// Σ w over those with a stellar companion.
    multiple: f64,
    /// Σ w over those with no companion as Moe and Di Stefano count them.
    counted_single: f64,
    /// The draw against the model over them.
    alive: DrawTally,
}

/// What one living record of a bin adds: the bin, the model's drawn multiple fraction, whether it
/// is of placement class `Alive` (else a displaced runaway, walkaway or survivor), whether the draw
/// made it multiple, whether it shows a stellar companion, and whether it has none as Moe and Di
/// Stefano count.
type BinEntry = (usize, f64, bool, bool, bool, bool);

/// The bins' tallies over [`MULTIPLICITY_DRAWS`].
fn bin_tallies(galaxy: &Galaxy) -> [BinTally; SURVEYS.len()] {
    let model = MultiplicityModel::default_v1();
    let mut bins = [BinTally::default(); SURVEYS.len()];
    for (&band, &(region, examined)) in MassBand::ALL.iter().zip(&MULTIPLICITY_DRAWS) {
        let w = band_weight(galaxy, band, examined);
        let entries: Vec<BinEntry> = layer_map(galaxy, band.layer(), region, examined, |r| {
            let m1 = r.primary_initial_mass();
            let bin = SURVEYS
                .iter()
                .position(|s| (s.masses.0..s.masses.1).contains(&m1.value()))?;
            if !living(r) {
                return None;
            }
            let alive = r.placement_class() == PlacementClass::Alive;
            if !alive {
                return Some((bin, 0.0, false, false, false, false));
            }
            let h = hierarchy(galaxy, r);
            let stars = stellar_count(&h);
            let counted = direct_companions(&h)
                .iter()
                .filter(|&&(q, x)| q > 0.1 && x < 8.0)
                .count();
            Some((
                bin,
                model.drawn_multiple_fraction(m1),
                true,
                stars > 1 || h.dropped_companions() > 0,
                stars > 1,
                counted == 0,
            ))
        });
        for (bin, p, alive, drawn, multiple, counted_single) in entries {
            let t = &mut bins[bin];
            t.living += w;
            if !alive {
                t.displaced += w;
                continue;
            }
            t.alive.add(w, p, drawn);
            t.alive_records += 1;
            t.alive_squares += w * w;
            if multiple {
                t.multiple += w;
            }
            if counted_single {
                t.counted_single += w;
            }
        }
    }
    bins
}

/// P11.T12: multiplicity by primary mass in seven bins, over living primaries of placement class
/// `Alive`.
///
/// A remnant is left out: plan 08's kick classes, which keep a remnant in the field or carry it
/// off, are selected by plan 06's companion-stripped mark, with which a primary is always multiple
/// (Design note 1), so a retained remnant of 5–16 M☉ is multiple more often than the model's figure
/// for all primaries. Plan 08's displaced runaways and walkaways are left out too, single by
/// construction, as are the surveys' cluster stars here, which plan 09's features hold; their share
/// of the living primaries is recorded (de Wit et al. 2005, A&A 437, 247, find most Galactic field
/// O stars to be runaways; recalled, not checked against the paper in the pod).
///
/// - Against the model: the weighted count the draw made multiple in each bin, every companion
///   dropped or not, against `Σ w p` with p the model's
///   [`drawn_multiple_fraction`](MultiplicityModel::drawn_multiple_fraction), χ² on seven degrees
///   of freedom at α = 10⁻³. A failure is a wrong state of the draw.
/// - Against the surveys: each bin's share with a stellar companion as drawn (or, from 2 M☉ up,
///   with none counted as Moe and Di Stefano count), against [`SURVEYS`] in units of the survey's
///   error and the sample's together, χ² recorded. Each bin holds at least [`MIN_ALIVE`] records.
///   A bin more than five errors out is a wrong state; a χ² above its α = 10⁻³ limit is a finding.
#[test]
#[ignore = "slow: 4.1 × 10⁶ generated records, 8 × 10⁵ hierarchies"]
fn multiplicity_by_primary_mass_follows_the_model_and_the_surveys() {
    let bins = bin_tallies(&milky_way());
    let (mut model_chi, mut survey_chi, mut worst) = (0.0, 0.0, 0.0_f64);
    for (s, t) in SURVEYS.iter().zip(&bins) {
        let z = t.alive.z();
        model_chi += z * z;
        let weight = t.alive.weight;
        let generated = if s.counted_singles {
            t.counted_single / weight
        } else {
            t.multiple / weight
        };
        let sampled = (generated * (1.0 - generated) * t.alive_squares).sqrt() / weight;
        let surveyed = if generated < s.value {
            s.error.0
        } else {
            s.error.1
        };
        let pull = (generated - s.value) / math::hypot(surveyed, sampled);
        survey_chi += pull * pull;
        worst = worst.max(pull.abs());
        println!(
            "{}–{} M☉ (anchor {}): {} records alive, drawn multiple {:.4} against the model's \
             {:.4} (z = {z:+.2}); {} {generated:.4} ± {sampled:.4} against {} (−{} +{}), pull \
             {pull:+.2}; displaced {:.4} of the living",
            s.masses.0,
            s.masses.1,
            s.anchor,
            t.alive_records,
            t.alive.drawn / weight,
            t.alive.expected / weight,
            if s.counted_singles {
                "counted singles"
            } else {
                "multiples"
            },
            s.value,
            s.error.0,
            s.error.1,
            t.displaced / t.living,
        );
    }
    let dof = u32::try_from(SURVEYS.len()).expect("seven bins");
    let model_p = chi_square_p(model_chi, dof);
    let survey_p = chi_square_p(survey_chi, dof);
    println!(
        "χ² against the model {model_chi:.2} on {dof} (p = {model_p:.3}); against the surveys \
         {survey_chi:.2} on {dof} (p = {survey_p:.3}), the worst bin {worst:.2} errors out"
    );
    for (s, t) in SURVEYS.iter().zip(&bins) {
        assert!(
            t.alive_records >= MIN_ALIVE,
            "{}–{} M☉: only {} records alive",
            s.masses.0,
            s.masses.1,
            t.alive_records
        );
    }
    assert_p_value("multiplicity against the model", model_p, ALPHA);
    assert!(worst <= 5.0, "a bin lies {worst:.1} errors from its survey");
}

// ---------------------------------------------------------------------------------------------
// Sun-like companion counts.

/// Raghavan et al.'s (2010, ApJS 190, 1, §5.2 and Figure 16) shares of single, double, triple and
/// higher-order systems among solar-type stars within 25 pc, with their errors: 56 ± 2, 33 ± 2,
/// 8 ± 1 and 3 ± 1%.
const RAGHAVAN: [(f64, f64); MAX_COMPANIONS + 1] =
    [(0.56, 0.02), (0.33, 0.02), (0.08, 0.01), (0.03, 0.01)];

/// The Sun-like primaries, M☉: Duchêne and Kraus's 0.7–1.3 M☉ bin, which holds the model's
/// solar-type anchor.
const SUN_LIKE: (f64, f64) = (0.7, 1.3);

/// P11.T12: the companion counts of Sun-like primaries.
///
/// - Against the model: the companions each record's draw asked for, those the stability test
///   dropped included, against `Σ w pmf` of the model's
///   [`companion_count_pmf`](MultiplicityModel::companion_count_pmf), Pearson's χ² on the
///   weighted counts at their effective number on three degrees of freedom, α = 10⁻³.
/// - Against Raghavan et al.: the shares of one to four stars as generated against [`RAGHAVAN`],
///   each in units of its error; one more than five errors out is a wrong state, and the χ² on
///   three degrees of freedom is recorded.
#[test]
#[ignore = "slow: 3 × 10⁵ generated hierarchies"]
fn sun_like_companion_counts_follow_the_model_and_raghavan() {
    let galaxy = milky_way();
    let model = MultiplicityModel::default_v1();
    let (mut weight, mut squares) = (0.0, 0.0);
    let mut asked = [0.0; MAX_COMPANIONS + 1];
    let mut expected = [0.0; MAX_COMPANIONS + 1];
    let mut shown = [0.0; MAX_COMPANIONS + 1];
    let mut records = 0;
    for (band, examined) in [(MassBand::B, 100_000), (MassBand::C, 200_000)] {
        let w = band_weight(&galaxy, band, examined);
        let counts = layer_map(&galaxy, band.layer(), LOCAL, examined, |r| {
            let m1 = r.primary_initial_mass();
            if !(SUN_LIKE.0..SUN_LIKE.1).contains(&m1.value())
                || grid_multiplicity(r) != MultiplicityContext::Free
            {
                return None;
            }
            let h = hierarchy(&galaxy, r);
            let stars = stellar_count(&h);
            Some((
                stars,
                stars - 1 + usize::from(h.dropped_companions()),
                model.companion_count_pmf(m1),
            ))
        });
        for (stars, drawn, pmf) in counts {
            records += 1;
            weight += w;
            squares += w * w;
            asked[drawn] += w;
            shown[stars - 1] += w;
            for (e, p) in expected.iter_mut().zip(pmf) {
                *e += w * p;
            }
        }
    }
    let effective = weight * weight / squares;
    let model_chi: f64 = asked
        .iter()
        .zip(&expected)
        .map(|(o, e)| effective * (o - e) * (o - e) / (e * weight))
        .sum();
    let model_p = chi_square_p(model_chi, 3);
    let mut survey_chi = 0.0;
    let mut worst = 0.0_f64;
    for (k, ((o, e), (share, error))) in shown.iter().zip(&expected).zip(RAGHAVAN).enumerate() {
        let generated = o / weight;
        let pull = (generated - share) / error;
        survey_chi += pull * pull;
        worst = worst.max(pull.abs());
        println!(
            "{} star(s): {:.4} as generated, {:.4} asked for, the model's {:.4}; Raghavan's \
             {share} ± {error}, pull {pull:+.2}",
            k + 1,
            generated,
            asked[k] / weight,
            e / weight,
        );
    }
    let survey_p = chi_square_p(survey_chi, 3);
    println!(
        "{records} Sun-like records ({effective:.0} effective): χ² against the model \
         {model_chi:.2} on 3 (p = {model_p:.3}); against Raghavan et al. {survey_chi:.2} on 3 \
         (p = {survey_p:.3})"
    );
    assert_p_value("Sun-like counts against the model", model_p, ALPHA);
    assert!(
        worst <= 5.0,
        "a count's share lies {worst:.1} errors from Raghavan et al.'s"
    );
}

// ---------------------------------------------------------------------------------------------
// Periods and mass ratios.

/// The primaries the spine construction draws, M☉: the direct construction's weight is 0 below
/// 1.5 M☉ (ruling 81).
const SPINE_MAX_MASS: f64 = 1.5;

/// The longest log₁₀(P ÷ 1 d) the period test reads: far inside every orbit's window near the
/// solar circle, where the tidal cut leaves periods up to about 10¹⁰ days, so that below it the
/// first companion's period is the model's law restricted to it, exactly.
const KS_MAX_LOG_PERIOD: f64 = 8.0;

/// Raghavan et al.'s (2010, §6.1) peak of solar-type companions' periods, log₁₀(P ÷ 1 d) = 5.03,
/// and the Verification's window about it, 0.1 dex.
const SUN_LIKE_PEAK: (f64, f64) = (5.03, 0.1);

/// The primaries the peak is asserted for, M☉: about the model's solar-type anchor, where its
/// period law is Raghavan et al.'s log-normal. Across Duchêne and Kraus's 0.7–1.3 M☉ the law is
/// interpolated towards the M dwarfs' shorter periods, and the bin's median is recorded.
const SOLAR_ANCHOR: (f64, f64) = (0.95, 1.05);

/// The median of `values`, which it sorts.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// P11.T12: the first companion's period and mass ratio, from generated systems.
///
/// For each record of a primary below [`SPINE_MAX_MASS`] whose first companion is a star, its
/// period below [`KS_MAX_LOG_PERIOD`] goes through the model's period law at the primary's mass
/// restricted there, `F(x) ÷ F(8)`, and its mass ratio through the mass-ratio law at that mass
/// and period. Both are uniform if the draw follows the model: a one-sample Kolmogorov–Smirnov
/// test of each at α = 10⁻³. The median log period of the first companions of primaries about 1
/// M☉ ([`SOLAR_ANCHOR`]), all periods, lies within 0.1 dex of Raghavan et al.'s peak (the
/// Verification's "Sun-like periods peak within 0.1 dex of 10⁵ days"; the median of a log-normal
/// is its peak); that of the whole Sun-like bin is recorded.
#[test]
#[ignore = "slow: 7 × 10⁵ generated hierarchies"]
fn first_companions_periods_and_mass_ratios_pass_kolmogorov_smirnov() {
    let galaxy = milky_way();
    let model = MultiplicityModel::default_v1();
    let (mut periods, mut ratios) = (Vec::new(), Vec::new());
    let (mut sun_like, mut solar) = (Vec::new(), Vec::new());
    for (band, examined) in [
        (MassBand::A, 400_000),
        (MassBand::B, 100_000),
        (MassBand::C, 200_000),
    ] {
        let companions = layer_map(&galaxy, band.layer(), LOCAL, examined, |r| {
            let m1 = r.primary_initial_mass();
            if m1.value() >= SPINE_MAX_MASS {
                return None;
            }
            let (m2, period) = first_companion(&hierarchy(&galaxy, r))?;
            Some((m1, m2, period))
        });
        for (m1, m2, period) in companions {
            let x = math::log10(period.value());
            if (SUN_LIKE.0..SUN_LIKE.1).contains(&m1.value()) {
                sun_like.push(x);
            }
            if (SOLAR_ANCHOR.0..SOLAR_ANCHOR.1).contains(&m1.value()) {
                solar.push(x);
            }
            if x >= KS_MAX_LOG_PERIOD {
                continue;
            }
            let law = model.period_distribution(m1);
            periods.push(law.cdf(x) / law.cdf(KS_MAX_LOG_PERIOD));
            ratios.push(
                model
                    .mass_ratio_distribution(m1, period)
                    .cdf(m2.value() / m1.value()),
            );
        }
    }
    let period_ks = ks_one_sample(&mut periods, |u| u);
    let ratio_ks = ks_one_sample(&mut ratios, |u| u);
    let peak = median(&mut solar);
    let bin = median(&mut sun_like);
    println!(
        "{} first companions below log P = 8: periods D = {:.5} (p = {:.3}), mass ratios D = \
         {:.5} (p = {:.3}); median log P {peak:.3} of {} about 1 M☉ and {bin:.3} of {} over \
         {}–{} M☉, against Raghavan et al.'s {} ± {}",
        periods.len(),
        period_ks.statistic,
        period_ks.p_value,
        ratio_ks.statistic,
        ratio_ks.p_value,
        solar.len(),
        sun_like.len(),
        SUN_LIKE.0,
        SUN_LIKE.1,
        SUN_LIKE_PEAK.0,
        SUN_LIKE_PEAK.1,
    );
    assert_p_value("first companions' periods", period_ks.p_value, ALPHA);
    assert_p_value("first companions' mass ratios", ratio_ks.p_value, ALPHA);
    assert!(
        (peak - SUN_LIKE_PEAK.0).abs() <= SUN_LIKE_PEAK.1,
        "the median log period about 1 M☉ is {peak:.3}, more than {} dex from {}",
        SUN_LIKE_PEAK.1,
        SUN_LIKE_PEAK.0
    );
}

// ---------------------------------------------------------------------------------------------
// All stars below 0.5 M☉.

/// The 20 pc census's share of all its stars of 0.08 M☉ or more below 0.5 M☉, 68.8% (Kirkpatrick
/// et al. 2024, ApJS 271, 55, Table 18), and the plan's window about its 69% under the default
/// mass function, ± 1 point (P11.T12).
const CENSUS_BELOW: (f64, f64, f64) = (0.688, 0.68, 0.70);

/// The census holds no neutron-star or black-hole system, so the fit's figure is over systems
/// whose primary lies below 8 M☉ (plan 15's P15.T4.b, ruling 138.4).
const CENSUS_PRIMARY_MAX: f64 = 8.0;

/// The records examined in each layer for the all-stars test, every one of the local box: each
/// layer's share of the ratio's variance about equal, for a statistical error of about 0.04
/// points.
const ALL_STARS_SIZES: [usize; 5] = [600_000, 300_000, 500_000, 50_000, 5_000];

/// One record of the all-stars test: its primary's initial mass, and its stars below 0.5 M☉ and
/// its stars, brown dwarfs left out.
type StarCounts = (f64, f64, f64);

/// The records of each layer whose primary's mass the quadrature's integrand is read at, for the
/// layers' breakdown: the integrand's mass-ratio law is itself a quadrature over the period, too
/// costly for every record.
const INTEGRAND_RECORDS: usize = 5_000;

/// P11.T12: the all-stars test (the brainstorm's "Sizing the layers").
///
/// The generated systems of all five layers, each weighted by its layer's share of systems,
/// count their stars (brown dwarfs left out) by initial mass. The share below 0.5 M☉ under the
/// default mass function, Chabrier's with plan 15's fitted scale, is asserted in 69 ± 1%; a miss
/// is a finding against the companions or the scale, not a reason to widen the window. The same
/// share over layers A–D, the census's systems, is recorded beside the fit's quadrature. Chabrier's
/// function as published (`high_mass_scale` 1) and Kroupa's are recorded, from the same systems
/// with each primary reweighted by the ratio of the two functions' densities, each normalised over
/// the stellar range, which leaves its companions as drawn: the model's companions depend on the
/// primary's mass alone. Each figure stands beside the as-drawn quadrature
/// ([`all_stars_fraction_below_as_drawn`]), which takes every companion's mass ratio against the
/// primary, where the spine construction draws a companion inside a secondary against that star
/// (most of the generated systems' surplus below 0.5 M☉ in layers B–D), and leaves out the direct
/// construction's subsystems, the stability test's drops and plan 08's displaced singles; the
/// default's stars per system and stars below 0.5 M☉ per system are recorded by layer against the
/// quadrature's integrand.
#[test]
#[ignore = "slow: 1.5 × 10⁶ generated hierarchies"]
fn all_stars_below_half_a_solar_mass_match_the_census() {
    let galaxy = milky_way();
    let model = MultiplicityModel::default_v1();
    let drawn = galaxy.mass_function();
    let published = Chabrier::new(1.0).expect("a positive scale");
    let functions: [(&str, &dyn MassFunction); 3] = [
        ("the default", drawn),
        ("Chabrier as published", &published),
        ("Kroupa", &Kroupa),
    ];
    let half = SolarMasses::new(0.5);
    let layers: Vec<(f64, Vec<StarCounts>)> = MassBand::ALL
        .iter()
        .zip(ALL_STARS_SIZES)
        .map(|(&band, examined)| {
            let counts = layer_map(&galaxy, band.layer(), LOCAL, examined, |r| {
                let h = hierarchy(&galaxy, r);
                let (mut below, mut stars) = (0.0, 0.0);
                for star in h.stars().iter().filter(|s| s.kind() == SlotKind::Star) {
                    stars += 1.0;
                    if star.initial_mass() < half {
                        below += 1.0;
                    }
                }
                Some((r.primary_initial_mass().value(), below, stars))
            });
            (band_weight(&galaxy, band, examined), counts)
        })
        .collect();
    for (band, (_, counts)) in MassBand::ALL.iter().zip(&layers) {
        let mean = |pick: fn(&StarCounts) -> f64| {
            counts.iter().map(pick).sum::<f64>() / count(counts.len())
        };
        // The quadrature's integrand at the first records' primaries: stars per system, 1 + CF,
        // and stars below 0.5 M☉, the primary's 1 or 0 + CF × the mass-ratio law at 0.5 ÷ m₁.
        let first = &counts[..INTEGRAND_RECORDS.min(counts.len())];
        let (mut stars, mut below) = (0.0, 0.0);
        for &(m, _, _) in first {
            let m1 = SolarMasses::new(m);
            let companions = model.drawn_companion_frequency(m1);
            stars += 1.0 + companions;
            below += f64::from(u8::from(m1 < half))
                + companions * model.drawn_companion_mass_ratio_cdf(m1, 0.5 / m);
        }
        println!(
            "layer {:?}: stars per system {:.4} against the quadrature's {:.4}; below 0.5 M☉ \
             {:.4} against {:.4}",
            band.layer(),
            mean(|c| c.2),
            stars / count(first.len()),
            mean(|c| c.1),
            below / count(first.len()),
        );
    }
    let total_drawn = drawn.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
    let mut figures = Vec::new();
    for (name, f) in functions {
        let total = f.integral(MASS_LIMIT_LO, MASS_LIMIT_HI);
        // A record of layer L counts its layer's weight, (∫_L ξ_d ÷ ∫ξ_d) ÷ n_L, times
        // (ξ_f(m) ÷ ∫ξ_f) ÷ (ξ_d(m) ÷ ∫ξ_d): over the layer's records that sums to f's share of
        // systems in the layer, and over the layer's primaries it weighs each mass as f does.
        let weighted: Vec<Vec<(f64, f64, f64)>> = layers
            .iter()
            .map(|(w, counts)| {
                counts
                    .iter()
                    .map(|&(m, b, s)| (w * (f.pdf(m) / total) / (drawn.pdf(m) / total_drawn), b, s))
                    .collect()
            })
            .collect();
        // Over all five layers, then over layers A–D: the ratio Σ w b ÷ Σ w s and its error,
        // √(Σ w² (b − R s)²) ÷ Σ w s, the records being independent.
        let ratio_of = |layers: &[Vec<(f64, f64, f64)>]| {
            let records = || layers.iter().flatten();
            let below: f64 = records().map(|(w, b, _)| w * b).sum();
            let stars: f64 = records().map(|(w, _, s)| w * s).sum();
            let ratio = below / stars;
            let spread: f64 = records()
                .map(|(w, b, s)| {
                    let r = w * (b - ratio * s);
                    r * r
                })
                .sum();
            (ratio, spread.sqrt() / stars)
        };
        let (all, all_error) = ratio_of(&weighted);
        let (census, census_error) = ratio_of(&weighted[..4]);
        println!(
            "{name}: all stars below 0.5 M☉ {all:.4} ± {all_error:.4} over layers A–E \
             (quadrature {:.4}), {census:.4} ± {census_error:.4} over A–D (quadrature {:.4}); the \
             census's {}",
            all_stars_fraction_below_as_drawn(f, &model, half),
            all_stars_fraction_below_as_drawn(&Truncated::new(f, CENSUS_PRIMARY_MAX), &model, half),
            CENSUS_BELOW.0,
        );
        figures.push(all);
    }
    let default = figures[0];
    assert!(
        (CENSUS_BELOW.1..=CENSUS_BELOW.2).contains(&default),
        "all stars below 0.5 M☉ under the default: {default:.4}, outside {:?}",
        (CENSUS_BELOW.1, CENSUS_BELOW.2)
    );
}

// ---------------------------------------------------------------------------------------------
// Blue stragglers and hot subdwarfs in old populations (the engine).

/// An old population and the box its records are sought in, where it holds most systems: the
/// thick disc 4,000–6,000 ly above the solar circle, the bulge about plan 06's reference point
/// 3,000 ly out along the bar, and the halo 12,000–20,000 ly above the solar circle.
const OLD_POPULATIONS: [(Population, Region); 3] = [
    (
        Population::ThickDisc,
        Region {
            centre_ly: [0, 26_000, 5_000],
            half_ly: [8_000, 2_000, 1_000],
        },
    ),
    (
        Population::Bulge,
        Region {
            centre_ly: [3_000, 0, 0],
            half_ly: [1_000, 1_000, 500],
        },
    ),
    (
        Population::Halo,
        Region {
            centre_ly: [0, 26_000, 16_000],
            half_ly: [16_000, 8_000, 4_000],
        },
    ),
];

/// The records of each population examined in layers B–D, where its giants, horizontal-branch
/// stars, blue stragglers and hot subdwarfs lie (turn-offs of 0.8–1 M☉).
const OLD_SIZES: [(MassBand, usize); 3] = [
    (MassBand::B, 50_000),
    (MassBand::C, 100_000),
    (MassBand::D, 20_000),
];

/// The records of layer A examined, in the local box, for its stars per system: they never leave
/// the main sequence in the age of the universe, so those are its multiplicity's, the same in
/// every population.
const OLD_LAYER_A_SIZE: usize = 100_000;

/// Plan 06's window on giants and horizontal-branch stars in the old populations, 0.3–3% of
/// objects (P06.T31, `old_populations_have_nothing_living_above_the_turn_off`), which counts each
/// system's primary: the window is asserted on the primaries, like for like, and the share of all
/// stars, companions in, is recorded.
const GIANTS_AND_HB_OF_OBJECTS: (f64, f64) = (0.003, 0.03);

/// Blue stragglers per horizontal-branch star where they are measured, each counted as its survey
/// counts it ([`BlueCount`]): the inner halo's 1.5–2.0 per blue horizontal-branch star at |Z| ≈ 5
/// kpc, both classes A-type stars at [`BLUE_STAR_MIN_TEFF_K`] or hotter (Santucci et al. 2015,
/// ApJ 801, 116; Preston and Sneden's 2000 figure of about 4, AJ 120, 1014, is the local halo's),
/// taken on 0.6–5.5 (research's ruling of 2026-11-02; recalled here, not read at source); and the
/// bulge's 0.29–0.37 of all its stragglers per horizontal-branch star (Clarkson et al. 2011, ApJ
/// 735, 37; recalled, not checked against the paper), taken within a factor of three either way.
const BLUE_STRAGGLERS_PER_HB: [(Population, BlueCount, f64, f64); 2] = [
    (Population::Halo, BlueCount::AType, 0.6, 5.5),
    (Population::Bulge, BlueCount::All, 0.29 / 3.0, 0.37 * 3.0),
];

/// How a survey counts blue stragglers per horizontal-branch star.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlueCount {
    /// Every blue straggler per horizontal-branch star.
    All,
    /// The A-type ones per blue horizontal-branch star, both at [`BLUE_STAR_MIN_TEFF_K`] or
    /// hotter.
    AType,
}

/// The least effective temperature of an A-type blue straggler and of a blue horizontal-branch
/// star as Santucci et al. (2015) count them, K: 7,650 (recalled, not read at source).
const BLUE_STAR_MIN_TEFF_K: f64 = 7_650.0;

/// The widest blue stragglers per horizontal-branch star that is not absurd: a population of
/// them with none, or with thirty times its horizontal branch, would be a class absurd.
const BLUE_STRAGGLERS_ABSURD: (f64, f64) = (0.0, 30.0);

/// What one record of an old population shows at the epoch: its stars (objects), its first-giant-
/// branch and core-helium-burning stars (the hot ones, at [`BLUE_STAR_MIN_TEFF_K`] or more, the
/// blue horizontal branch), its pairs of class blue straggler (those whose main-sequence star is
/// as hot, A-type) and hot subdwarf, its helium main-sequence stars of the hot subdwarfs' masses,
/// paired or not, and its primary alone as plan 06 counts it (an object, and a giant or
/// horizontal-branch star).
#[derive(Debug, Clone, Copy, Default)]
struct OldTally {
    objects: f64,
    giants: f64,
    horizontal_branch: f64,
    blue_horizontal_branch: f64,
    blue_stragglers: f64,
    a_type_blue_stragglers: f64,
    hot_subdwarfs: f64,
    subdwarf_stars: f64,
    primaries: f64,
    evolved_primaries: f64,
}

impl OldTally {
    /// The tally of `stars` at the epoch.
    fn of(stars: &SystemStars) -> Self {
        let mut t = Self::default();
        let Some(now) = stars.state_at(UniverseTime::EPOCH) else {
            return t;
        };
        let (low, high) = HOT_SUBDWARF_MASSES;
        if let Some(primary) = now
            .stars()
            .first()
            .filter(|p| p.phase() != Phase::NoRemnant)
        {
            t.primaries = 1.0;
            if matches!(
                primary.phase(),
                Phase::FirstGiantBranch | Phase::CoreHeliumBurning
            ) {
                t.evolved_primaries = 1.0;
            }
        }
        for star in now.stars() {
            if star.phase() == Phase::NoRemnant {
                continue;
            }
            t.objects += 1.0;
            let hot = star.effective_temperature().value() >= BLUE_STAR_MIN_TEFF_K;
            match star.phase() {
                Phase::FirstGiantBranch => t.giants += 1.0,
                Phase::CoreHeliumBurning => {
                    t.horizontal_branch += 1.0;
                    if hot {
                        t.blue_horizontal_branch += 1.0;
                    }
                }
                Phase::HeliumMainSequence if star.mass() >= low && star.mass() <= high => {
                    t.subdwarf_stars += 1.0;
                }
                _ => {}
            }
        }
        assert_eq!(
            stars.pairs().len(),
            now.pairs().len(),
            "a state for every pair"
        );
        for (timeline, pair) in stars.pairs().iter().zip(now.pairs()) {
            match pair.class() {
                BinaryClass::BlueStraggler => {
                    t.blue_stragglers += 1.0;
                    let a_type = timeline.stars().iter().any(|&i| {
                        let star = &now.stars()[usize::from(i.get())];
                        star.phase() == Phase::MainSequence
                            && star.effective_temperature().value() >= BLUE_STAR_MIN_TEFF_K
                    });
                    if a_type {
                        t.a_type_blue_stragglers += 1.0;
                    }
                }
                BinaryClass::HotSubdwarf => t.hot_subdwarfs += 1.0,
                _ => {}
            }
        }
        t
    }

    /// `self` plus `w` times `other`.
    fn add(&mut self, w: f64, other: &Self) {
        self.objects += w * other.objects;
        self.giants += w * other.giants;
        self.horizontal_branch += w * other.horizontal_branch;
        self.blue_horizontal_branch += w * other.blue_horizontal_branch;
        self.blue_stragglers += w * other.blue_stragglers;
        self.a_type_blue_stragglers += w * other.a_type_blue_stragglers;
        self.hot_subdwarfs += w * other.hot_subdwarfs;
        self.subdwarf_stars += w * other.subdwarf_stars;
        self.primaries += w * other.primaries;
        self.evolved_primaries += w * other.evolved_primaries;
    }
}

/// P11.T12: blue stragglers and hot subdwarfs in the old populations, from generated systems run
/// through the binary engine (re-run after the engine's fixes).
///
/// Each population's records of layers B–D in its box ([`OLD_POPULATIONS`]), and layer A's stars
/// per system from the local box, are weighted by the population's band shares and tallied at the
/// epoch ([`OldTally`]). Plan 06's figures for its class fractions are the reference: its giants and
/// horizontal-branch primaries, 0.3–3% of primaries, still hold with the companions drawn, and the
/// blue stragglers and hot subdwarfs (the pair classes, and the helium stars of the hot subdwarfs'
/// masses, single or paired) are each counted per horizontal-branch star. Every population has
/// blue stragglers, short of thirty per horizontal-branch star, and fewer hot subdwarfs than
/// horizontal-branch stars, and the old populations together have hot subdwarfs; a figure outside
/// its measured window ([`BLUE_STRAGGLERS_PER_HB`]), counted as its survey counts it (the halo's
/// A-type stragglers per blue horizontal-branch star, like for like), is a finding.
#[test]
#[ignore = "slow: 5.1 × 10⁵ generated systems, their pairs run through the engine"]
fn old_populations_hold_blue_stragglers_and_hot_subdwarfs() {
    let galaxy = milky_way();
    let local_a = layer_map(&galaxy, Layer::A, LOCAL, OLD_LAYER_A_SIZE, |r| {
        Some(OldTally::of(&SystemStars::generate(&galaxy, r)))
    });
    let mut a = OldTally::default();
    for each in &local_a {
        a.add(1.0 / count(local_a.len()), each);
    }
    let mut subdwarfs = 0.0;
    for (population, region) in OLD_POPULATIONS {
        let mut t = OldTally::default();
        t.add(galaxy.shares().share(MassBand::A, population), &a);
        for (band, examined) in OLD_SIZES {
            let w = galaxy.shares().share(band, population) / count(examined);
            let tallies = layer_map_where(
                &galaxy,
                band.layer(),
                region,
                examined,
                |r| r.population() == population,
                |r| Some(OldTally::of(&SystemStars::generate(&galaxy, r))),
            );
            for each in &tallies {
                t.add(w, each);
            }
        }
        let hb = t.horizontal_branch;
        let primaries = t.evolved_primaries / t.primaries;
        let giants_and_hb = (t.giants + hb) / t.objects;
        let per_hb = t.blue_stragglers / hb;
        println!(
            "{population:?}: giants and horizontal branch {primaries:.4} of primaries (plan 06's \
             0.003–0.03), {giants_and_hb:.4} of all stars; per horizontal-branch star, blue \
             stragglers {per_hb:.3}, hot subdwarfs {:.4} as pairs and {:.4} as helium stars of \
             0.32–0.8 M☉",
            t.hot_subdwarfs / hb,
            t.subdwarf_stars / hb,
        );
        if let Some(&(_, counted, lo, hi)) = BLUE_STRAGGLERS_PER_HB
            .iter()
            .find(|(p, _, _, _)| *p == population)
        {
            let (measured, what) = match counted {
                BlueCount::All => (per_hb, "blue stragglers per horizontal-branch star"),
                BlueCount::AType => (
                    t.a_type_blue_stragglers / t.blue_horizontal_branch,
                    "A-type blue stragglers per blue horizontal-branch star",
                ),
            };
            println!(
                "{population:?}: {what} {measured:.3} against the measured window {lo:.2}–{hi:.2}: \
                 {}",
                if (lo..=hi).contains(&measured) {
                    "inside"
                } else {
                    "a finding"
                }
            );
        }
        assert!(
            (GIANTS_AND_HB_OF_OBJECTS.0..=GIANTS_AND_HB_OF_OBJECTS.1).contains(&primaries),
            "{population:?}: giants and horizontal branch {primaries:.4} of primaries, outside \
             {GIANTS_AND_HB_OF_OBJECTS:?}"
        );
        assert!(
            per_hb > BLUE_STRAGGLERS_ABSURD.0 && per_hb < BLUE_STRAGGLERS_ABSURD.1,
            "{population:?}: {per_hb:.3} blue stragglers per horizontal-branch star"
        );
        assert!(
            t.subdwarf_stars < hb,
            "{population:?}: more hot subdwarfs than horizontal-branch stars"
        );
        subdwarfs += t.hot_subdwarfs;
    }
    assert!(subdwarfs > 0.0, "no hot subdwarf in the old populations");
}

// ---------------------------------------------------------------------------------------------
// Recycled pulsars and double neutron stars (the engine).

/// The records examined for the recycling and double-neutron-star tests, in the wide box: layer D's
/// pairs recycle pulsars beside white dwarfs, layer E's beside neutron stars and white dwarfs.
const RECYCLING_SIZES: [(MassBand, usize); 2] = [(MassBand::D, 30_000), (MassBand::E, 40_000)];

/// What a recycled pulsar's companion is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Companion {
    /// A carbon–oxygen or oxygen–neon white dwarf: an intermediate-mass donor's core.
    HeavyWhiteDwarf,
    /// A neutron star: a Case BB donor's remnant.
    NeutronStar,
    /// A helium white dwarf: a low-mass donor's core.
    HeliumWhiteDwarf,
}

/// Each companion's window on its recycled pulsars' median field, G, recalled and not checked
/// against the sources in the pod: beside heavy white dwarfs, the mildly recycled pulsars of
/// 10⁹–10¹⁰ G; beside neutron stars, the double neutron stars' first-born pulsars of 3 × 10⁹–3 ×
/// 10¹⁰ G (B1913+16's 2.3 × 10¹⁰ G, J0737−3039A's 6.3 × 10⁹ G); beside helium white dwarfs, the
/// millisecond pulsars of 10⁸–10⁹ G (Tauris, Langer and Kramer 2012, MNRAS 425, 1601, §1).
const RECYCLED_FIELDS: [(Companion, &str, f64, f64); 3] = [
    (
        Companion::HeavyWhiteDwarf,
        "a CO/ONe white dwarf",
        1e9,
        1e10,
    ),
    (Companion::NeutronStar, "a neutron star", 3e9, 3e10),
    (
        Companion::HeliumWhiteDwarf,
        "a helium white dwarf",
        1e8,
        1e9,
    ),
];

/// The fields, G, outside which a recycled pulsar's median would be absurd: below 10⁷ G no pulsar
/// is known, and at 10¹² G and above it is a young pulsar's, not recycled.
const RECYCLED_FIELD_ABSURD: (f64, f64) = (1e7, 1e12);

/// A recycled pulsar beside a remnant companion: its field, G, and its period, ms.
#[derive(Debug, Clone, Copy)]
struct Recycled {
    field: f64,
    period_ms: f64,
    companion: Companion,
}

/// Every recycled pulsar of `timeline` beside a white-dwarf or neutron-star companion, read at the
/// first segment start where it is recycled and its companion is that remnant (as
/// `binary_classes.rs` reads them).
fn recycled_pulsars(timeline: &BinaryTimeline) -> Vec<Recycled> {
    let mut seen = [false; 2];
    let mut out = Vec::new();
    for segment in timeline.segments() {
        let age = segment.start();
        let context = timeline.class_context(age);
        let state = timeline.state_at(age);
        for c in Component::BOTH {
            let i = c.index();
            if seen[i] {
                continue;
            }
            let Some(pulsar) = context.pulsar(c).filter(|p| p.recycled()) else {
                continue;
            };
            let companion = match state.stars()[1 - i].phase() {
                Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
                    Companion::HeavyWhiteDwarf
                }
                Phase::NeutronStar => Companion::NeutronStar,
                Phase::HeliumWhiteDwarf => Companion::HeliumWhiteDwarf,
                _ => continue,
            };
            seen[i] = true;
            out.push(Recycled {
                field: pulsar.state().field().value(),
                period_ms: pulsar.state().period().value() * 1e3,
                companion,
            });
        }
    }
    out
}

/// Whether `timeline`'s two supernovae left a bound double neutron star, and if so whether
/// gravitational waves merge it within the age of the universe (Peters 1964).
fn double_neutron_star(timeline: &BinaryTimeline) -> Option<bool> {
    let [first, second] = timeline.supernovae() else {
        return None;
    };
    let neutron = |r: &SupernovaRecord| r.remnant().kind() == RemnantKind::NeutronStar && r.bound();
    if !(neutron(first) && neutron(second)) {
        return None;
    }
    let after = timeline.state_at(Years::new(second.age().value() * (1.0 + 1e-9)));
    let orbit = after.orbit()?;
    let [a, b] = after.stars();
    if a.phase() != Phase::NeutronStar || b.phase() != Phase::NeutronStar {
        return None;
    }
    Some(
        peters_merger_time(
            a.mass(),
            b.mass(),
            orbit.semi_major_axis(),
            orbit.eccentricity(),
        ) <= AGE_OF_UNIVERSE,
    )
}

/// Vigna-Gómez et al.'s (2018, MNRAS 481, 4009, Table 2) fiducial Galactic rate of merging double
/// neutron stars, 24.04 per Myr, from 0.24% of binaries with a primary of 8 M☉ or more, of which a
/// share 0.73 merge: the scale ruling 132.2 takes a yield to a rate by.
const VG18: (f64, f64, f64) = (24.04, 0.0024, 0.73);

/// Pol, McLaughlin and Lorimer's (2019, ApJ 870, 71) Galactic rate of merging double neutron stars
/// at 90% confidence, per Myr: 42 (+30 −14). Ruling 132.2 fails a rate only above twice its upper
/// limit.
#[expect(
    clippy::doc_markdown,
    reason = "McLaughlin is an author's name, not code"
)]
const POL_RATE: (f64, f64, f64) = (28.0, 42.0, 72.0);

/// What one layer-D or -E record's pairs leave: its recycled pulsars, whether it is multiple, and
/// its bound double neutron stars and those that merge.
type RecyclingEntry = (Vec<Recycled>, bool, u32, u32);

/// The 25th, 50th and 75th percentiles of `values`, which it sorts; NaN for none.
fn quartiles(values: &mut [f64]) -> [f64; 3] {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n == 0 {
        return [f64::NAN; 3];
    }
    [values[n / 4], values[n / 2], values[(3 * n) / 4]]
}

/// P11.T12: the recycled pulsars' fields and the double neutron stars' merger rate, from generated
/// systems of layers D and E run through the binary engine (re-run after the engine's fixes).
///
/// Each record's stars are drawn under the model's own multiplicity
/// ([`SystemStars::generate_in`] with [`MultiplicityContext::Free`]), so that plan 08's displaced
/// remnants, single by construction, keep the pairs their kicks are drawn from: the yields are the
/// engine's per massive system, not the solar neighbourhood's.
///
/// - Recycling: each companion's recycled pulsars' median field lies inside
///   [`RECYCLED_FIELD_ABSURD`]; against its measured window ([`RECYCLED_FIELDS`]) it is recorded, a
///   miss a finding. The quartiles of field and period, and the helium white dwarfs' share of the
///   recycled (most binary millisecond pulsars have one: ruling p11-channels' finding F6), are
///   recorded.
/// - Double neutron stars: the merging ones per multiple layer-E system, scaled by [`VG18`] to a
///   Galactic rate, at most twice Pol et al.'s upper limit (ruling 132.2); the rate is recorded
///   against Pol et al.'s.
#[test]
#[ignore = "slow: 7 × 10⁴ massive generated systems, their pairs run through the engine"]
fn recycled_fields_and_the_double_neutron_star_merger_rate() {
    let galaxy = milky_way();
    let mut recycled = Vec::new();
    let (mut multiples, mut bound, mut merging) = (0_u32, 0_u32, 0_u32);
    for (band, examined) in RECYCLING_SIZES {
        let entries: Vec<RecyclingEntry> = layer_map(&galaxy, band.layer(), WIDE, examined, |r| {
            let stars = SystemStars::generate_in(&galaxy, r, MultiplicityContext::Free);
            let (mut found, mut pairs, mut merge) = (Vec::new(), 0, 0);
            for pair in stars.pairs() {
                found.extend(recycled_pulsars(pair.timeline()));
                if let Some(merges) = double_neutron_star(pair.timeline()) {
                    pairs += 1;
                    merge += u32::from(merges);
                }
            }
            Some((found, stellar_count(stars.hierarchy()) > 1, pairs, merge))
        });
        for (found, multiple, pairs, merge) in entries {
            recycled.extend(found);
            if band == MassBand::E {
                multiples += u32::from(multiple);
                bound += pairs;
                merging += merge;
            }
        }
    }
    let helium = recycled
        .iter()
        .filter(|r| r.companion == Companion::HeliumWhiteDwarf)
        .count();
    println!(
        "{} recycled pulsars, {helium} ({:.1}%) beside helium white dwarfs",
        recycled.len(),
        100.0 * count(helium) / count(recycled.len().max(1))
    );
    for (companion, name, lo, hi) in RECYCLED_FIELDS {
        let set: Vec<&Recycled> = recycled
            .iter()
            .filter(|r| r.companion == companion)
            .collect();
        let mut fields: Vec<f64> = set.iter().map(|r| r.field).collect();
        let mut periods: Vec<f64> = set.iter().map(|r| r.period_ms).collect();
        let [f1, f2, f3] = quartiles(&mut fields);
        let [p1, p2, p3] = quartiles(&mut periods);
        println!(
            "recycled beside {name}: {}, field {f1:.2e} / {f2:.2e} / {f3:.2e} G, period {p1:.1} / \
             {p2:.1} / {p3:.1} ms (quartiles); the median field against {lo:.0e}–{hi:.0e} G: {}",
            set.len(),
            if (lo..=hi).contains(&f2) {
                "inside"
            } else {
                "a finding"
            }
        );
        assert!(!set.is_empty(), "no pulsar recycled beside {name}");
        assert!(
            (RECYCLED_FIELD_ABSURD.0..RECYCLED_FIELD_ABSURD.1).contains(&f2),
            "the median field beside {name}, {f2:.2e} G, is not a recycled pulsar's"
        );
    }
    let (rate, yield_, share) = VG18;
    let galactic = rate * (f64::from(merging) / f64::from(multiples)) / (yield_ * share);
    println!(
        "{bound} bound double neutron stars in {multiples} multiple layer-E systems, {merging} \
         merging within the age of the universe: about {galactic:.0} per Myr in the Galaxy by \
         VG18's yield, against Pol et al.'s {} ({}–{})",
        POL_RATE.1, POL_RATE.0, POL_RATE.2
    );
    assert!(bound > 0, "no bound double neutron star");
    assert!(
        galactic <= 2.0 * POL_RATE.2,
        "about {galactic:.0} merging double neutron stars per Myr, above twice Pol et al.'s upper \
         limit of {}: re-open the common envelope's α λ with an advisor (ruling 132.2)",
        POL_RATE.2
    );
}

// ---------------------------------------------------------------------------------------------
// A cluster's Hertzsprung–Russell diagram, for the eye.

/// The variable naming the file a cluster's Hertzsprung–Russell table is written to; unset, the
/// test writes nothing.
const HR_DUMP_VAR: &str = "HYPERION_HR_DUMP";

/// How far from the Sun the cluster is sought, ly.
const HR_SEARCH_LY: f64 = 3_000.0;

/// The youngest cluster taken, years: old enough that its turn-off is below its blue stragglers'
/// reach, as M67's (about 4 Gyr) is.
const HR_MIN_AGE: f64 = 1e9;

/// The query's radius in the cluster's half-mass radii, within its reach: most of its bound
/// members, and few of the field's systems.
const HR_HALF_MASS_RADII: f64 = 3.0;

/// The query's census limit on expected systems, which admits every layer of a cluster of 10⁴–10⁵
/// members (the default, 4,096, stops at its remnants).
const HR_CENSUS_LIMIT: NonZeroU32 = NonZeroU32::new(1 << 20).expect("2²⁰ is not zero");

/// P11.T12: a Hertzsprung–Russell table of the most massive old open cluster within
/// [`HR_SEARCH_LY`] of the Sun, its members' stars with binaries, for the check by eye that its
/// diagram shows a binary sequence and blue stragglers (the Verification's "by eye").
///
/// Its members are those a range query of [`HR_HALF_MASS_RADII`] about it returns from its
/// interior, in the galaxy with its kinematic tables, each star at the
/// epoch a row: its system, body, phase, mass, log L and log `T_eff`, and the class of the first
/// pair the engine ran, if any. The table goes to the file [`HR_DUMP_VAR`] names, for the lane's
/// outbox; it is not a golden. The test asserts only that the census reaches layer A, that the
/// cluster holds members, some of them multiple, and that every star's luminosity and temperature
/// are finite.
#[test]
#[ignore = "slow: an old open cluster's members, their pairs run through the engine"]
fn an_old_open_cluster_s_hertzsprung_russell_table() {
    // A feature's members are defined only with the galaxy's kinematic tables.
    let galaxy = milky_way().with_full_potential();
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("inside the galaxy");
    let (feature, model) = FeatureCatalogue::near(
        &galaxy,
        &sun,
        LightYears::new(HR_SEARCH_LY),
        &NoFeatureCache,
    )
    .filter(|f| matches!(f.process(), FeatureProcess::OldOpenCluster(_)))
    .filter(|f| f.age_at_epoch().is_some_and(|a| a.value() >= HR_MIN_AGE))
    .filter_map(|f| ClusterModel::from_record(&galaxy, &f).map(|m| (f, m)))
    .max_by(|a, b| a.1.mass().value().total_cmp(&b.1.mass().value()))
    .expect("an old open cluster lies within 3,000 ly of the Sun");
    let interiors = KeepInteriors::new(&galaxy);
    let source = FeatureMemberSource::new(&NoFeatureCache, &interiors);
    let radius = LightYears::new(
        (HR_HALF_MASS_RADII * model.half_mass_radius().value()).min(feature.reach().value()),
    );
    let query = RangeQuery::builder(*feature.position(), radius)
        .limit(HR_CENSUS_LIMIT)
        .build()
        .expect("a query");
    let result = range_query(&galaxy, &mut NoCache::new(), &[&source], &query);
    let mut rows = String::from("system\tbody\tphase\tmass\tlog_l\tlog_teff\tpair_class\n");
    assert_eq!(
        result.census().complete_down_to(),
        Some(Layer::A),
        "the census reaches every stellar layer"
    );
    let (mut members, mut multiple, mut stragglers) = (0, 0, 0);
    for hit in result.systems() {
        if !matches!(hit.record().origin(), SystemOrigin::FeatureMember { .. }) {
            continue;
        }
        let stars = stars_of(&galaxy, &interiors, hit.record()).expect("a member resolves");
        let Some(now) = stars.state_at(UniverseTime::EPOCH) else {
            continue;
        };
        members += 1;
        multiple += usize::from(now.stars().len() > 1);
        let class = now.pairs().first().map(PairState::class);
        stragglers += usize::from(class == Some(BinaryClass::BlueStraggler));
        for (body, star) in now.stars().iter().enumerate() {
            let (l, t) = (
                star.luminosity().value(),
                star.effective_temperature().value(),
            );
            assert!(
                l.is_finite() && t.is_finite(),
                "{:?} body {body}: L {l}, T {t}",
                hit.id()
            );
            writeln!(
                rows,
                "{:?}\t{body}\t{:?}\t{:.4}\t{:.4}\t{:.4}\t{}",
                hit.id(),
                star.phase(),
                star.mass().value(),
                math::log10(l.max(1e-12)),
                math::log10(t.max(1.0)),
                class.map_or_else(|| "-".to_owned(), |c| format!("{c:?}")),
            )
            .expect("writing to a string");
        }
    }
    println!(
        "{:?}: {:.0} M☉, {:.2} Gyr; {members} members, {multiple} of them multiple, {stragglers} \
         blue stragglers",
        feature.id(),
        model.mass().value(),
        feature.age_at_epoch().map_or(0.0, |a| a.value() / 1e9),
    );
    if let Some(path) = std::env::var_os(HR_DUMP_VAR) {
        std::fs::write(&path, rows).expect("the table's file is writable");
    }
    assert!(members > 0, "the cluster shows no member");
    assert!(multiple > 0, "no member of the cluster is multiple");
}
