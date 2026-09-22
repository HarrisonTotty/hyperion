//! Benchmarks of the event constructions (plan 06, P06.T27.d): the events of a window of ten Poisson
//! bins, and one root of a monotone phase.
//!
//! A regression is a finding to raise, not a CI failure: CI compiles these and never runs them.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::events::{
    EventSeries, EventsPerSecond, LinearClock, MonotonePhase, PoissonBins, RateModel, TimeWindow,
    octaves_to_span, tags,
};
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::time::{SourceHorizon, Span, UniverseTime};

const DAY: i64 = 86_400;

/// Flares at a mean of 20 a day, modulated by a slow sinusoid so that thinning rejects some.
struct Flares;

impl RateModel for Flares {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        EventsPerSecond::per_day(30.0)
    }

    fn rate(&self, t: UniverseTime) -> EventsPerSecond {
        let day = t.seconds().rem_euclid(DAY);
        #[expect(clippy::cast_precision_loss, reason = "seconds of a day")]
        let phase = day as f64 / 86_400.0;
        EventsPerSecond::per_day(
            20.0 + 10.0 * hyperion_sim::math::sin(std::f64::consts::TAU * phase),
        )
    }
}

fn star() -> EventSeries {
    let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), 0);
    EventSeries::new(Seed::new(0xbe7c), tags::STAR_FLARE, body.into())
}

/// Ten day-long bins at about 20 events each: the cost of one star's flares over ten days.
fn poisson_bins(c: &mut Criterion) {
    let bins = PoissonBins::new(DAY, 1).unwrap();
    let series = star();
    let window = TimeWindow::new(
        UniverseTime::new(-5 * DAY, 0).unwrap(),
        UniverseTime::new(5 * DAY, 0).unwrap(),
    )
    .unwrap();
    let mut out = Vec::with_capacity(512);
    c.bench_function("events/poisson_bins_ten_bins", |b| {
        b.iter(|| {
            out.clear();
            bins.events_in(black_box(&series), black_box(window), &Flares, &mut out);
            black_box(out.len())
        });
    });
}

/// One crossing of a monotone phase whose octaves span the source horizon: a three-year period,
/// as for Vela-like glitches, and 16 octaves of a four-cycle lattice.
fn phase_root(c: &mut Criterion) {
    let clock = LinearClock::new(Span::from_julian_years(3).unwrap(), UniverseTime::EPOCH).unwrap();
    let horizon = SourceHorizon::END
        .checked_since(SourceHorizon::START)
        .unwrap();
    let octaves = octaves_to_span(4, clock.cycles_in(horizon));
    let phase = MonotonePhase::new(0.3, 4, octaves).unwrap();
    let series = star();
    let mut cycle = -1_000_i64;
    c.bench_function("events/monotone_phase_root", |b| {
        b.iter(|| {
            cycle += 7;
            black_box(phase.event(black_box(&series), &clock, cycle))
        });
    });
}

criterion_group!(benches, poisson_bins, phase_root);
criterion_main!(benches);
