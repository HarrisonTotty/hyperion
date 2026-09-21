//! Benchmarks of the determinism foundation: `math`, `rng`, `id` and their neighbours.
//!
//! Targets are recorded in the plan that owns each group. A regression is a finding to raise, not
//! a CI failure: CI compiles these and never runs them.

use std::fmt::Write as _;
use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{
    CatalogueSystemId, CentreMemberId, DwarfCoreMemberId, EventBin, FeatureCell, FeatureMemberId,
    FeatureRef, Layer, MemberSlot, PinnedId, StreamMemberId, SystemId, event_tags,
};
use hyperion_sim::math;
use hyperion_sim::rng::{
    EventKey, ObjectKey, PiecewiseLinear, PiecewisePowerLaw, PowerLaw, Stream, tags,
    threefry2x64_20,
};
use hyperion_testkit::lcg::Lcg;

/// The pinned `libm` functions that generation calls most: the soft `exp`, `ln`, `pow` and `fma`
/// are what plan 03 measures before considering `libm`'s `arch` feature.
fn math(c: &mut Criterion) {
    let mut group = c.benchmark_group("math");
    group.bench_function("exp", |b| b.iter(|| math::exp(black_box(0.5))));
    group.bench_function("ln", |b| b.iter(|| math::ln(black_box(1.5))));
    group.bench_function("powf", |b| {
        b.iter(|| math::powf(black_box(150.0), black_box(-2.3)));
    });
    group.bench_function("sin_cos", |b| b.iter(|| math::sin_cos(black_box(1.0))));
    group.bench_function("mul_add", |b| {
        b.iter(|| math::mul_add(black_box(1.5), black_box(9.46e15), black_box(0.25)));
    });
    group.finish();
}

/// Sixty-four IDs of every kind, in a fixed pseudo-random order of kinds and fields.
fn mixed_ids() -> Vec<SystemId> {
    let mut lcg = Lcg::new(0x1d5);
    let mut below = |n: u64| lcg.next_below(n);
    let mut ids = Vec::with_capacity(64);
    for i in 0..64 {
        let slot = if i % 3 == 0 {
            MemberSlot::FeatureLevel {
                index: u16::try_from(below(8_192)).unwrap(),
            }
        } else {
            MemberSlot::InCell {
                band: Layer::ALL[usize::try_from(below(7)).unwrap()],
                level: 0,
                cell: [0, 1, 2].map(|_| u8::try_from(below(16)).unwrap()),
                index: u16::try_from(below(8_192)).unwrap(),
            }
        };
        let id: SystemId = match i % 11 {
            0..=6 => {
                let layer = Layer::ALL[i % 7];
                let half = 65_536 / u64::from(layer.cell_size_ly());
                let c = [0, 1, 2].map(|_| {
                    i32::try_from(below(2 * half)).unwrap() - i32::try_from(half).unwrap()
                });
                let index = u32::try_from(below(1 << layer.index_bits())).unwrap();
                SystemId::from_parts(layer, GenCell::new(layer.cell_size(), c).unwrap(), index)
                    .unwrap()
            }
            7 => {
                let cell = FeatureCell::new([-3, 4, 15]).unwrap();
                let feature = FeatureRef::new(cell, u16::try_from(below(16_384)).unwrap()).unwrap();
                FeatureMemberId::new(feature, slot).unwrap().into()
            }
            8 => CentreMemberId::new(slot).unwrap().into(),
            9 => match i % 3 {
                0 => DwarfCoreMemberId::new(1, slot).unwrap().into(),
                1 => StreamMemberId::new(87, Layer::B, 5_121, [3, 23], 9)
                    .unwrap()
                    .into(),
                _ => PinnedId::new(below(1 << 58)).unwrap().into(),
            },
            _ => CatalogueSystemId::new(3, [8, -67, 26], u32::try_from(below(1 << 24)).unwrap(), 0)
                .unwrap()
                .into(),
        };
        ids.push(id);
    }
    ids
}

/// ID decoding, building and designations. Plan 01's target: `SystemId::from_raw` under 5 ns
/// per ID on the mixed batch.
fn id(c: &mut Criterion) {
    let ids = mixed_ids();
    let raws: Vec<u64> = ids.iter().map(|id| id.raw()).collect();
    let mut group = c.benchmark_group("id");
    group.throughput(Throughput::Elements(u64::try_from(raws.len()).unwrap()));
    group.bench_function("from_raw_mixed", |b| {
        b.iter(|| {
            for &raw in &raws {
                let _ = black_box(SystemId::from_raw(black_box(raw)));
            }
        });
    });
    group.throughput(Throughput::Elements(1));
    let cell = GenCell::new(CellSize::Ly8, [3_250, -12, 700]).unwrap();
    group.bench_function("from_parts_layer_a", |b| {
        b.iter(|| SystemId::from_parts(Layer::A, black_box(cell), black_box(42)));
    });
    group.throughput(Throughput::Elements(u64::try_from(ids.len()).unwrap()));
    let mut text = String::with_capacity(64);
    group.bench_function("designation_mixed", |b| {
        b.iter(|| {
            for id in &ids {
                text.clear();
                write!(text, "{}", id.designation()).unwrap();
                black_box(&text);
            }
        });
    });
    group.finish();
}

/// The block function, stream set-up and event keys. Plan 01's targets: one Threefry2x64-20 block
/// under 20 ns; opening a stream and drawing four words, a thinning candidate's budget, under
/// 50 ns; deriving an event key and opening one bin, under 60 ns. The bin's first word is drawn so
/// that the open is not optimised away: two blocks in all.
fn rng(c: &mut Criterion) {
    let mut group = c.benchmark_group("rng");
    group.bench_function("threefry_block", |b| {
        b.iter(|| threefry2x64_20(black_box([0x5eed, 0x7a9]), black_box([42, 7])));
    });
    let key = ObjectKey::from(
        SystemId::from_parts(
            Layer::A,
            GenCell::new(CellSize::Ly8, [3_250, -12, 700]).unwrap(),
            42,
        )
        .unwrap(),
    );
    group.bench_function("open_and_draw_four", |b| {
        b.iter(|| {
            let mut stream = Stream::open(
                black_box(Seed::new(9)),
                tags::SELFTEST_STREAM,
                black_box(key),
            );
            [
                stream.next_u64(),
                stream.next_u64(),
                stream.next_u64(),
                stream.next_u64(),
            ]
        });
    });
    let star = SystemId::from_parts(
        Layer::A,
        GenCell::new(CellSize::Ly8, [3_250, -12, 700]).unwrap(),
        42,
    )
    .unwrap();
    let bin = EventBin::new(-12_345).unwrap();
    group.bench_function("event_key_and_bin", |b| {
        b.iter(|| {
            let key = EventKey::derive(
                black_box(Seed::new(9)),
                event_tags::SELF_TEST,
                black_box(star).into(),
            );
            key.bin_stream(black_box(bin)).next_u64()
        });
    });
    group.finish();
}

/// Every sampler on an open stream. Plan 01's targets: Poisson at a mean of 1.2 under 40 ns and
/// at 1,000 under 150 ns.
fn samplers(c: &mut Criterion) {
    let mut stream = Stream::open(Seed::new(11), tags::SELFTEST_STREAM, ObjectKey::galaxy());
    let massive = PowerLaw::new(2.3, 8.0, 150.0).unwrap();
    let kroupa =
        PiecewisePowerLaw::continuous(&[0.01, 0.08, 0.5, 150.0], &[0.3, 1.3, 2.3]).unwrap();
    let history = PiecewiseLinear::new(&[0.0, 2.0, 6.0, 13.0], &[0.2, 1.0, 0.7, 0.0]).unwrap();
    let mut group = c.benchmark_group("samplers");
    group.bench_function("uniform", |b| b.iter(|| stream.uniform()));
    group.bench_function("standard_normal", |b| b.iter(|| stream.standard_normal()));
    group.bench_function("log_normal_dex", |b| {
        b.iter(|| stream.log_normal_dex(black_box(8.0), black_box(0.11)));
    });
    group.bench_function("poisson_1.2", |b| b.iter(|| stream.poisson(black_box(1.2))));
    group.bench_function("poisson_1000", |b| {
        b.iter(|| stream.poisson(black_box(1_000.0)));
    });
    group.bench_function("power_law", |b| b.iter(|| stream.power_law(&massive)));
    group.bench_function("piecewise_power_law", |b| {
        b.iter(|| kroupa.sample(&mut stream));
    });
    group.bench_function("piecewise_linear", |b| {
        b.iter(|| history.sample(&mut stream));
    });
    // The thinning step: one mark picks one of five components or rejects.
    let densities = [0.25, 1.5, 0.5, 0.1, 0.05];
    group.bench_function("mark_pick_weighted", |b| {
        b.iter(|| {
            stream
                .mark()
                .pick_weighted(black_box(&densities), black_box(2.5))
        });
    });
    group.finish();
}

criterion_group!(foundation, math, id, rng, samplers);
criterion_main!(foundation);
