//! Plan 11's P11.T11, slow: the binary engine wired into the system stage, and the checks that
//! rulings 114, 123.5, 137, 140.9 and 141.7 left for it.
//!
//! - The rare bright exception (P11.T11): the share of layer-A and -B systems whose combined
//!   luminosity exceeds a 0.75 M☉ star's at the same age by ten times, by population, under 10⁻³.
//! - Rucinski's (2002) contact binaries (ruling 114): contact pairs per main-sequence star fainter
//!   than `M_V` = +1.5, against 1/1,000–1/250.
//! - Massive companions counted (rulings 140.9 and 141.7): stars of 8 M☉ and up, of 15 M☉ and up
//!   and of 2.5–8 M☉ per solar mass formed, companions included, against the primaries alone and
//!   Kroupa's 0.0409 for 2.5–8 M☉.
//! - The stripped mark (ruling 123.5): a massive primary marked stripped loses its envelope to its
//!   companion before it dies and does not merge; one whose innermost orbit lies in the merger band
//!   merges.
//! - The stripped share per exploding star (ruling 137): hydrogen-poor core collapses over all core
//!   collapses, companions' included, in 0.28–0.40.

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::Population;
use hyperion_sim::galaxy::displaced::binarity;
use hyperion_sim::galaxy::imf::{MASS_BAND_EDGES, MassBand};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
use hyperion_sim::id::{BodyId, Layer};
use hyperion_sim::stellar::binary::{BinaryClass, Component, SegmentKind};
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::multiplicity::{
    HierarchyNode, MultiplicityContext, RedrawAttempt, SlotKind, draw_hierarchy,
    stripped_mark_min_mass,
};
use hyperion_sim::stellar::photometry::absolute_magnitude_v;
use hyperion_sim::stellar::system::{StarModel, SystemStars, draw_metallicity};
use hyperion_sim::stellar::{Phase, StarState};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{SolarMasses, Years};
use hyperion_testkit::lcg::Lcg;

const SEED: u64 = 0x0b17_0011_5eed_0001;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, std::num::NonZero::get)
}

/// `f` of every item of `items`, in one share per thread (run one after another on
/// wasm32-wasip1), each share's results folded with `fold` from `T::default()`, and the shares'
/// results folded again in share order: the same on any number of shares for a fold that is a sum
/// of counts.
fn par_fold<I: Sync, T: Default + Send>(
    items: &[I],
    f: impl Fn(&I, &mut T) + Sync,
    merge: impl Fn(&mut T, T),
) -> T {
    let n = threads();
    let f = &f;
    let share = |k: usize| {
        let mut acc = T::default();
        for item in items.iter().skip(k).step_by(n) {
            f(item, &mut acc);
        }
        acc
    };
    #[cfg(not(target_family = "wasm"))]
    let parts: Vec<T> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..n)
            .map(|k| {
                let share = &share;
                scope.spawn(move || share(k))
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().expect("a worker finishes"))
            .collect()
    });
    // wasm32-wasip1 has no threads: the shares run one after another (plan R04, T7.d).
    #[cfg(target_family = "wasm")]
    let parts: Vec<T> = (0..n).map(share).collect();
    let mut total = T::default();
    for part in parts {
        merge(&mut total, part);
    }
    total
}

/// The cell of `layer` that holds the point 26,000 ly from the centre along +y, shifted by `step`
/// cells along x and `lift` along y.
fn solar_cell(layer: Layer, step: i32, lift: i32) -> CellKey {
    let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
    CellKey::new(layer, [step, 26_000 / size + lift, 0]).expect("a cell of the grid")
}

/// Up to three records from each of random cells of `layer` within 4,000 ly of the solar circle's
/// point along x and 2,000 ly along y, until there are `n`.
fn records_of(galaxy: &Galaxy, layer: Layer, n: usize, salt: u64) -> Vec<SystemRecord> {
    let mut rng = Lcg::new(SEED ^ salt);
    let size = u64::from(layer.cell_size_ly());
    let (along, across) = ((4_000 / size).max(1), (2_000 / size).max(1));
    let mut cell = Vec::new();
    let mut records = Vec::with_capacity(n + 3);
    while records.len() < n {
        let step = i32::try_from(rng.next_u64() % (2 * along + 1)).expect("small")
            - i32::try_from(along).expect("small");
        let rise = i32::try_from(rng.next_u64() % (2 * across + 1)).expect("small")
            - i32::try_from(across).expect("small");
        generate_cell(galaxy, solar_cell(layer, step, rise), &mut cell);
        records.extend(cell.drain(..).take(3));
    }
    records.truncate(n);
    records
}

fn count_f64(n: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts of at most 10⁶, which an f64 holds exactly"
    )]
    let x = n as f64;
    x
}

// ---------------------------------------------------------------------------------------------
// The bright exception and Rucinski's contact binaries.

/// Population slots of the tallies, in `hyperion_sim::galaxy::POPULATIONS` order.
fn population_slot(p: Population) -> usize {
    hyperion_sim::galaxy::POPULATIONS
        .iter()
        .position(|&q| q == p)
        .expect("every population is listed")
}

/// One layer's systems at the epoch.
#[derive(Debug, Clone, Copy, Default)]
struct EpochTally {
    /// Systems, and those brighter than ten 0.75 M☉ stars of their age, by population.
    systems: [u64; 7],
    bright: [u64; 7],
    /// Main-sequence stars fainter than `M_V` = +1.5, and pairs in contact at the epoch.
    faint_main_sequence: u64,
    contact: u64,
}

fn merge_epoch(total: &mut EpochTally, part: EpochTally) {
    for i in 0..7 {
        total.systems[i] += part.systems[i];
        total.bright[i] += part.bright[i];
    }
    total.faint_main_sequence += part.faint_main_sequence;
    total.contact += part.contact;
}

/// A 0.75 M☉ star's luminosity at the age and composition of `stars`' primary, L☉, or `None` if it
/// has not formed.
fn reference_luminosity(stars: &SystemStars) -> Option<f64> {
    let primary = stars.primary();
    StarModel::new(
        SolarMasses::new(0.75),
        *primary.composition(),
        StarDraws::median(),
        primary.age_at_epoch(),
    )
    .expect("a 0.75 M_sun star")
    .state_at(UniverseTime::EPOCH)
    .map(|s| s.luminosity().value())
}

fn is_faint_main_sequence(state: &StarState) -> bool {
    state.phase() == Phase::MainSequence
        && absolute_magnitude_v(state).is_some_and(|m| m.value() > 1.5)
}

#[test]
#[ignore = "slow: 1.6 × 10⁵ grid systems of layers A to D, their pairs run through the engine"]
fn binary_system_bright_exception_and_contact_binaries() {
    let galaxy = milky_way();
    let strata = [
        (Layer::A, 50_000),
        (Layer::B, 40_000),
        (Layer::C, 40_000),
        (Layer::D, 30_000),
    ];
    let mut contact_per_system = 0.0;
    let mut faint_per_system = 0.0;
    let mut bright_ab = [0_u64; 7];
    let mut systems_ab = [0_u64; 7];
    for (salt, (layer, n)) in (0_u64..).zip(strata) {
        let records = records_of(&galaxy, layer, n, 0x0b_0100 + salt);
        let galaxy = &galaxy;
        let tally = par_fold(
            &records,
            |record, acc: &mut EpochTally| {
                let stars = SystemStars::generate(galaxy, record);
                let Some(now) = stars.state_at(UniverseTime::EPOCH) else {
                    return;
                };
                let slot = population_slot(record.population());
                acc.systems[slot] += 1;
                if let Some(l) = reference_luminosity(&stars)
                    && now.luminosity().value() > 10.0 * l
                {
                    acc.bright[slot] += 1;
                }
                acc.faint_main_sequence += now
                    .stars()
                    .iter()
                    .filter(|s| is_faint_main_sequence(s))
                    .count() as u64;
                acc.contact += now
                    .pairs()
                    .iter()
                    .filter(|p| p.class() == BinaryClass::Contact)
                    .count() as u64;
            },
            merge_epoch,
        );
        let systems: u64 = tally.systems.iter().sum();
        let band = MassBand::ALL[layer_index(layer)];
        // The layer's weight in the solar neighbourhood: its share of the young and old thin
        // discs' systems, which hold nearly all of them.
        let weight = galaxy.shares().share(band, Population::OldThinDisc);
        contact_per_system += weight * count_f64(tally.contact) / count_f64(systems);
        faint_per_system += weight * count_f64(tally.faint_main_sequence) / count_f64(systems);
        println!(
            "layer {layer:?}: {systems} systems, {} faint main-sequence stars, {} contact pairs, \
             bright by population {:?} of {:?}",
            tally.faint_main_sequence, tally.contact, tally.bright, tally.systems
        );
        if matches!(layer, Layer::A | Layer::B) {
            for i in 0..7 {
                bright_ab[i] += tally.bright[i];
                systems_ab[i] += tally.systems[i];
            }
        }
    }
    let rucinski = contact_per_system / faint_per_system;
    println!(
        "contact pairs per main-sequence star fainter than M_V = +1.5: {rucinski:.2e} (Rucinski 2002: 1e-3 to 4e-3)"
    );
    for (i, (&bright, &systems)) in bright_ab.iter().zip(&systems_ab).enumerate() {
        if systems == 0 {
            continue;
        }
        let share = count_f64(bright) / count_f64(systems);
        println!(
            "{:?}: {bright} of {systems} layer-A and -B systems brighter than ten 0.75 M☉ stars ({share:.2e})",
            hyperion_sim::galaxy::POPULATIONS[i]
        );
        assert!(share < 1e-3, "{share}");
    }
    let bright: u64 = bright_ab.iter().sum();
    let systems: u64 = systems_ab.iter().sum();
    assert!(count_f64(bright) / count_f64(systems) < 1e-3);
}

fn layer_index(layer: Layer) -> usize {
    match layer {
        Layer::A => 0,
        Layer::B => 1,
        Layer::C => 2,
        Layer::D => 3,
        _ => 4,
    }
}

// ---------------------------------------------------------------------------------------------
// Massive stars counted with their companions (rulings 140.9 and 141.7).

/// Stars per system above a cut, primaries and companions apart.
#[derive(Debug, Clone, Copy, Default)]
struct MassTally {
    systems: u64,
    primaries: [u64; 3],
    companions: [u64; 3],
}

/// The mass ranges counted: 8 M☉ and up (core collapses), 15 M☉ and up (ionising, O9.5V), and
/// 2.5–8 M☉ (the Type Ia delay's progenitors).
const RANGES: [(f64, f64); 3] = [(8.0, 150.0), (15.0, 150.0), (2.5, 8.0)];

#[test]
#[ignore = "slow: 2 × 10⁵ hierarchies of primaries of 2.5 M☉ and up"]
fn binary_system_massive_companions_counted_per_solar_mass_formed() {
    let galaxy = milky_way();
    let imf = galaxy.mass_function();
    let lo = MASS_BAND_EDGES[3];
    let share_above =
        imf.integral(lo, MASS_BAND_EDGES[5]) / imf.integral(MASS_BAND_EDGES[0], MASS_BAND_EDGES[5]);
    let n = 200_000_u32;
    let at = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("inside");
    let component = galaxy.fields().component_id(0).expect("components");
    let mut lcg = Lcg::new(SEED ^ 0x0b_0200);
    let records: Vec<SystemRecord> = (0..n)
        .map(|i| {
            let m = imf.quantile_in(lo, MASS_BAND_EDGES[5], lcg.next_f64());
            let key = CellKey::new(
                Layer::E,
                [
                    i32::try_from(i / 64 % 256).expect("small"),
                    i32::try_from(i / 64 / 256).expect("small"),
                    0,
                ],
            )
            .expect("a cell");
            SystemRecord::from_parts(
                key.candidate_id(i % 64).expect("a candidate"),
                at,
                SystemOrigin::Grid(component),
                Population::YoungThinDisc,
                SolarMasses::new(m),
                Years::new(1.0e7),
            )
        })
        .collect();
    let galaxy_ref = &galaxy;
    let tally = par_fold(
        &records,
        |record, acc: &mut MassTally| {
            let h = draw_hierarchy(
                galaxy_ref,
                record,
                MultiplicityContext::Free,
                RedrawAttempt::FIRST,
            );
            acc.systems += 1;
            for (k, star) in h.stars().iter().enumerate() {
                if star.kind() != SlotKind::Star {
                    continue;
                }
                let m = star.initial_mass().value();
                for (r, &(a, b)) in RANGES.iter().enumerate() {
                    if m >= a && m < b {
                        if k == 0 {
                            acc.primaries[r] += 1;
                        } else {
                            acc.companions[r] += 1;
                        }
                    }
                }
            }
        },
        |t, p| {
            t.systems += p.systems;
            for r in 0..3 {
                t.primaries[r] += p.primaries[r];
                t.companions[r] += p.companions[r];
            }
        },
    );
    let formed = galaxy.mean_formed_mass().value();
    for (r, &(a, b)) in RANGES.iter().enumerate() {
        let per = |c: u64| count_f64(c) / count_f64(tally.systems) * share_above / formed;
        let (primaries, all) = (
            per(tally.primaries[r]),
            per(tally.primaries[r] + tally.companions[r]),
        );
        println!(
            "{a}-{b} M☉ per M☉ formed: primaries {primaries:.5}, with companions {all:.5} (×{:.3})",
            all / primaries
        );
    }
    let type_ia = count_f64(tally.primaries[2] + tally.companions[2]) / count_f64(tally.systems)
        * share_above
        / formed;
    println!(
        "2.5-8 M☉ stars per M☉ formed, companions included: {type_ia:.5} against Kroupa's 0.0409 ({:+.1}%)",
        100.0 * (type_ia / 0.0409 - 1.0)
    );
    assert!(tally.companions[0] > 0);
}

// ---------------------------------------------------------------------------------------------
// The stripped mark (ruling 123.5) and the stripped share per exploding star (ruling 137).

/// What the massive systems' pairs did.
#[derive(Debug, Clone, Copy, Default)]
struct MarkTally {
    marked: u64,
    marked_stripped: u64,
    marked_merged: u64,
    merger_band: u64,
    merger_band_merged: u64,
    collapses: u64,
    stripped_collapses: u64,
}

fn merge_marks(t: &mut MarkTally, p: MarkTally) {
    t.marked += p.marked;
    t.marked_stripped += p.marked_stripped;
    t.marked_merged += p.marked_merged;
    t.merger_band += p.merger_band;
    t.merger_band_merged += p.merger_band_merged;
    t.collapses += p.collapses;
    t.stripped_collapses += p.stripped_collapses;
}

/// Whether `phase` is a naked helium star's: hydrogen-poor.
fn is_helium_star(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
    )
}

#[test]
#[ignore = "slow: 4,000 massive systems run to their primaries' deaths through the engine"]
fn binary_system_stripped_marks_and_the_share_per_exploding_star() {
    let galaxy = milky_way();
    let imf = galaxy.mass_function();
    let at = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("inside");
    let component = galaxy.fields().component_id(0).expect("components");
    let mut lcg = Lcg::new(SEED ^ 0x0b_0300);
    let n = 4_000_u32;
    let records: Vec<SystemRecord> = (0..n)
        .map(|i| {
            let m = imf.quantile_in(8.0, 150.0, lcg.next_f64());
            let key = CellKey::new(
                Layer::E,
                [
                    i32::try_from(i / 64 % 256).expect("small"),
                    100 + i32::try_from(i / 64 / 256).expect("small"),
                    0,
                ],
            )
            .expect("a cell");
            SystemRecord::from_parts(
                key.candidate_id(i % 64).expect("a candidate"),
                at,
                SystemOrigin::Grid(component),
                Population::YoungThinDisc,
                SolarMasses::new(m),
                // Old enough that every star of 8 M☉ and up has died by +H.
                Years::new(6.0e7),
            )
        })
        .collect();
    let galaxy_ref = &galaxy;
    let tally = par_fold(
        &records,
        |record, acc: &mut MarkTally| tally_marks(galaxy_ref, record, acc),
        merge_marks,
    );
    println!("{tally:?}");
    let share = count_f64(tally.stripped_collapses) / count_f64(tally.collapses);
    println!(
        "marked primaries stripped and not merged before their deaths: {} of {}; merged {}; \
         merger band merged {} of {}; hydrogen-poor share of core collapses {share:.3} \
         (ruling 137: 0.28-0.40)",
        tally.marked_stripped,
        tally.marked,
        tally.marked_merged,
        tally.merger_band_merged,
        tally.merger_band
    );
    assert!(tally.marked > 500 && tally.collapses > 2_000, "{tally:?}");
}

/// What `record`'s system adds to the marks' tally.
fn tally_marks(galaxy: &Galaxy, record: &SystemRecord, acc: &mut MarkTally) {
    let stars = SystemStars::generate(galaxy, record);
    let comp = draw_metallicity(galaxy, record);
    let m1 = record.primary_initial_mass();
    let mark = StarDraws::for_star(galaxy.seed(), BodyId::new(record.id(), 0)).stripped();
    let primary_pair = stars.pairs().iter().find(|p| p.stars()[0].get() == 0);
    if m1 >= stripped_mark_min_mass(&comp)
        && let Some(pair) = primary_pair
    {
        let timeline = pair.timeline();
        let death = timeline.supernova_ages()[0];
        let before = |kind: fn(&SegmentKind) -> bool| {
            timeline
                .segments()
                .iter()
                .any(|s| kind(&s.kind()) && death.is_none_or(|d| s.start() < d))
        };
        let merged = before(|k| *k == SegmentKind::Merged);
        let stripped = before(|k| {
            matches!(
                k,
                SegmentKind::StableTransfer {
                    donor: Component::Primary
                } | SegmentKind::CommonEnvelope
            )
        });
        if binarity::is_stripped(mark, m1, &comp) {
            acc.marked += 1;
            acc.marked_stripped += u64::from(stripped && !merged);
            acc.marked_merged += u64::from(merged);
        } else {
            let h = stars.hierarchy();
            let HierarchyNode::Pair { outer, orbit, .. } = *h.node(pair.node()) else {
                unreachable!()
            };
            let q = h.node_mass(outer) / m1;
            let band = binarity::stripping_band(m1, q, &comp);
            if orbit.periapsis() <= band.merge {
                acc.merger_band += 1;
                acc.merger_band_merged += u64::from(merged);
            }
        }
    }
    // Every core collapse of the system's stars by +H: from each pair's timeline for a
    // paired star, from its own model otherwise.
    for (k, model) in stars.stars().iter().enumerate() {
        let paired = stars.pairs().iter().find_map(|p| {
            p.stars()
                .iter()
                .position(|s| usize::from(s.get()) == k)
                .map(|c| (p, c))
        });
        match paired {
            Some((pair, c)) => {
                let timeline = pair.timeline();
                if let Some(sn) = timeline
                    .supernovae()
                    .iter()
                    .find(|s| s.component() == Component::of_index(c))
                {
                    acc.collapses += 1;
                    let just_before = Years::new((sn.age().value() - 1.0).max(0.0));
                    let phase = timeline.state_at(just_before).stars()[c].phase();
                    acc.stripped_collapses += u64::from(is_helium_star(phase));
                }
            }
            None => {
                if model.initial_mass().value() >= 8.0
                    && model.death().is_some_and(|d| d.kind().is_sudden())
                {
                    acc.collapses += 1;
                    // A hydrogen-poor progenitor keeps under a tenth of a solar mass of envelope.
                    let envelope = model.death().expect("dies").progenitor().envelope_mass();
                    acc.stripped_collapses += u64::from(envelope.value() < 0.1);
                }
            }
        }
    }
}
