//! Benchmarks of plan 10's global list and a stream's tube table made on the CPU pool's workers
//! (P10.T5.b), against the sim's serial builds, on the Milky Way fixture (seed 7, its full
//! potential), where perf-spray's design took its figures: the list about 0.11 s cold and 60 ms
//! with the centre held, and a tube about 0.25 s, on eight workers, against 0.49 s and 1.3–2.0 s
//! serial (pod, 16 vCPU, 2026-10-30). CI compiles these and never runs them.
//!
//! - The list cold: a fresh copy of the galaxy, its centre unread, and no spray table held.
//! - The list with the centre held: a copy whose centre was read, and no spray table held.
//! - The tube of the fixture's stream 0, a living globular's, its spray table held.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hyperion_server::compute::{
    CancelToken, CpuPool, GalaxyKey, Priority, SharedSprayForces, Shares, global_list_on,
    tube_table_on,
};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::global_list::{GlobalList, StreamNumber, TubeTable};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::{GENERATOR_VERSION, Seed};

/// The fixture's seed.
const SEED: u64 = 7;

/// A spray-table cache's budget, room for a few galaxies' tables.
const FORCES_BYTES: usize = 1 << 20;

/// The list and stream 0's tube, serial and on pools of 1, 4 and 8 workers.
fn on_the_pool(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the fixture builds")
        .with_full_potential();
    let key = GalaxyKey::new(SEED, GENERATOR_VERSION);
    let held = galaxy.clone();
    held.centre().expect("the fixture's centre builds");
    let list = GlobalList::build(&held);
    let spec = list
        .stream(StreamNumber::new(0).expect("stream 0"))
        .expect("the fixture has a stream 0")
        .clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let mut group = c.benchmark_group("global_list_pool");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    let cold_list = || {
        (
            Arc::new(galaxy.clone()),
            Arc::new(SharedSprayForces::new(FORCES_BYTES)),
        )
    };
    let held_list = || {
        (
            Arc::new(held.clone()),
            Arc::new(SharedSprayForces::new(FORCES_BYTES)),
        )
    };
    group.bench_function("GlobalList::build, serial (cold)", |b| {
        b.iter_batched_ref(
            cold_list,
            |(g, _)| GlobalList::build(g),
            BatchSize::PerIteration,
        );
    });
    group.bench_function("GlobalList::build, serial (centre held)", |b| {
        b.iter_batched_ref(
            held_list,
            |(g, _)| GlobalList::build(g),
            BatchSize::PerIteration,
        );
    });
    group.bench_function("TubeTable::build, serial (stream 0)", |b| {
        b.iter(|| TubeTable::build(&held, &spec).expect("the full potential"));
    });
    let held_arc = Arc::new(held.clone());
    for n in [1, 4, 8] {
        let workers = NonZeroUsize::new(n).expect("at least one");
        let pool = CpuPool::new(workers, INTERACTIVE_QUEUE_CAPACITY, BULK_QUEUE_CAPACITY)
            .expect("the pool starts");
        let shares = (Priority::Interactive, Shares::for_workers(workers));
        let token = CancelToken::new();
        let list_on = |(g, forces): &mut (Arc<Galaxy>, Arc<SharedSprayForces>)| {
            runtime
                .block_on(global_list_on(&pool, forces, (g, key), shares, &token))
                .expect("the list's jobs run")
        };
        assert_eq!(list_on(&mut held_list()), list);
        group.bench_function(format!("global_list_on, {n} workers (cold)"), |b| {
            b.iter_batched_ref(cold_list, list_on, BatchSize::PerIteration);
        });
        group.bench_function(format!("global_list_on, {n} workers (centre held)"), |b| {
            b.iter_batched_ref(held_list, list_on, BatchSize::PerIteration);
        });
        let forces = Arc::new(SharedSprayForces::new(FORCES_BYTES));
        let tube_on = || {
            runtime
                .block_on(tube_table_on(
                    &pool,
                    &forces,
                    (&held_arc, key),
                    &spec,
                    shares,
                    &token,
                ))
                .expect("the tube's jobs run")
        };
        assert_eq!(
            tube_on(),
            TubeTable::build(&held, &spec).expect("the full potential")
        );
        group.bench_function(format!("tube_table_on, {n} workers (stream 0)"), |b| {
            b.iter(tube_on);
        });
        runtime
            .block_on(pool.shutdown())
            .expect("the pool shuts down");
    }
    group.finish();
}

criterion_group!(benches, on_the_pool);
criterion_main!(benches);
