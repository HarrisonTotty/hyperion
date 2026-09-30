//! Plan 11's P11.T7, slow: the grid half of the complementarity between the grid and the binary
//! catalogue classes.
//!
//! Over 10⁵ grid systems near the solar circle, stratified by layer (30,000 of layer A, 20,000
//! each of B, C and D, 10,000 of E), no system that `SystemStars::generate` keeps has a pair in a
//! carved class at any age of the source horizon, and the attempts it kept are recorded: no system
//! runs through all eight.

use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::stellar::multiplicity::MAX_REDRAWS;
use hyperion_sim::stellar::system::SystemStars;
use hyperion_testkit::lcg::Lcg;

const SEED: u64 = 0x0b17_0007_ca47_0001;

/// The layers and the systems drawn from each.
const STRATA: [(Layer, usize); 5] = [
    (Layer::A, 30_000),
    (Layer::B, 20_000),
    (Layer::C, 20_000),
    (Layer::D, 20_000),
    (Layer::E, 10_000),
];

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
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

/// What one layer's systems came to: how many kept each attempt, and how many have a pair.
#[derive(Debug, Clone, Copy, Default)]
struct Tally {
    attempts: [u64; MAX_REDRAWS as usize],
    with_pairs: u64,
    carved: u64,
}

#[test]
#[ignore = "slow: 10⁵ grid systems, each pair of two stars run through the binary engine"]
fn no_grid_system_is_in_a_carved_class_and_none_uses_every_attempt() {
    let galaxy = milky_way();
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let mut total = Tally::default();
    for (salt, &(layer, n)) in (0_u64..).zip(&STRATA) {
        let records = records_of(&galaxy, layer, n, salt);
        let (records, galaxy) = (&records, &galaxy);
        // Each thread tallies its share; the sums are the same on any number of threads.
        let tallies: Vec<Tally> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..threads)
                .map(|k| {
                    scope.spawn(move || {
                        let mut tally = Tally::default();
                        for record in records.iter().skip(k).step_by(threads) {
                            let stars = SystemStars::generate(galaxy, record);
                            tally.attempts[usize::from(stars.attempt().get())] += 1;
                            tally.with_pairs += u64::from(!stars.pairs().is_empty());
                            if let Some(carved) = stars.carved_pair() {
                                eprintln!("{:?} keeps a carved pair: {carved:?}", record.id());
                                tally.carved += 1;
                            }
                        }
                        tally
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|w| w.join().expect("a worker finishes"))
                .collect()
        });
        let mut layer_tally = Tally::default();
        for t in &tallies {
            for (sum, a) in layer_tally.attempts.iter_mut().zip(t.attempts) {
                *sum += a;
            }
            layer_tally.with_pairs += t.with_pairs;
            layer_tally.carved += t.carved;
        }
        let redrawn: u64 = layer_tally.attempts[1..].iter().sum();
        println!(
            "layer {layer:?}: {n} systems, {} with a pair run through the engine, {redrawn} \
             redrawn ({:.2e}); attempts kept {:?}",
            layer_tally.with_pairs,
            u64_as_f64(redrawn) / usize_as_f64(n),
            layer_tally.attempts
        );
        for (sum, a) in total.attempts.iter_mut().zip(layer_tally.attempts) {
            *sum += a;
        }
        total.with_pairs += layer_tally.with_pairs;
        total.carved += layer_tally.carved;
    }
    println!("all layers: attempts kept {:?}", total.attempts);
    assert_eq!(total.carved, 0, "grid systems kept a carved pair");
    assert_eq!(
        total.attempts[usize::from(MAX_REDRAWS) - 1],
        0,
        "a system ran through every attempt"
    );
}

#[expect(
    clippy::cast_precision_loss,
    reason = "counts of at most 10⁵, which an f64 holds exactly"
)]
fn u64_as_f64(n: u64) -> f64 {
    n as f64
}

#[expect(
    clippy::cast_precision_loss,
    reason = "counts of at most 10⁵, which an f64 holds exactly"
)]
fn usize_as_f64(n: usize) -> f64 {
    n as f64
}
