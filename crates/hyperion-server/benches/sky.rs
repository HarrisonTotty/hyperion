//! The server's first sky near the Sun, cold (rendering plan R06, R06.T11.d; T17's first-sky
//! budget): a fresh server with the default workers, its galaxy's tables, cell cache and every
//! other cache empty, asked one eye-only sky near the Sun, timed from the request's sending to its
//! first reply arriving whole, binary frames and all. A miss is a finding to raise, not a CI
//! failure: CI compiles these and never runs them.
//!
//! `sky_near_sun_cold/first_reply` starts a server over the real socket for each iteration (its
//! start, and the universe's creation and opening, are not timed), sends the default eye's sky at
//! (0, 26,000, 68) ly at the epoch with the default `n_max`, the derived caps by the eye's
//! visibility and the request's illumination, and stops the clock at the first `partial_response`
//! or `response`. So it holds the galaxy's tables' build, the illumination, the eye's cut and
//! visibility, the caps and plan, the band's march, the first step of shells (every layer's first
//! shell: C, D and E to 500 ly), and that reply's merge, band, limit map, eye offsets, payload and
//! transfer. The sky is then cancelled: its full census near the Sun is of the order of 10⁴ CPU-s
//! (R06.T17), beyond any bench. Each iteration prints what its first reply states, and the
//! process's CPU time over it where `/proc/self/stat` gives it (Linux, at 100 ticks a second), and
//! the handler logs, to stderr, when each of the sky's phases is done and each step's census jobs'
//! summed time (`RUST_LOG`, `hyperion_server::requests::sky=debug` by default). Criterion's
//! `sample_size(10)` is hours of runs while the census near the Sun costs what R06's Risks record
//! ("Deviations in T11.d, as built"): run it with `--test`, one iteration, until it is cheaper.
//!
//! The galaxy is the server's own for the sky tests' seed, `Galaxy::new(seed)` with its full
//! potential, not the sim benches' `milky_way_like` fixture, which no server builds. The budget is
//! T17's (decided 2026-10-03 and 2026-10-05): the first sky in at most 150 CPU-s and 10 s wall on
//! the development machine's default workers. Figures are recorded in R06's Risks ("Deviations in
//! T11.d, as built"), each with its load.

#[path = "../tests/common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use common::{Frame, TestServer};
use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_protocol::{
    EyeDto, GalacticPosition, OpenUniverseRequest, RequestBody, ResponseBody, ServerMessage,
    SkyRequest, SkyResponse, UniverseTime,
};
use hyperion_server::SkyService;
use hyperion_sim::sky::EyeObserver;
use tokio::runtime::Runtime;
use tracing_subscriber::EnvFilter;

/// The seed of the universe asked, the server sky tests' (`tests/sky.rs`).
const SEED: u64 = 0x4d2;

/// Near the Sun's place on the solar circle, ly, as the sim's and the server's sky tests stand.
const SUN_LY: [i32; 3] = [0, 26_000, 68];

/// The process's CPU time so far, s, from `/proc/self/stat`'s user and system ticks at Linux's
/// usual 100 a second, or `None` where there is no such file.
fn cpu_seconds() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // The fields after the command's closing parenthesis, from the state: utime and stime are the
    // 14th and 15th of the line, the 12th and 13th after it.
    let rest = stat.rsplit_once(')')?.1;
    let mut fields = rest.split_whitespace().skip(11);
    let user: f64 = fields.next()?.parse().ok()?;
    let system: f64 = fields.next()?.parse().ok()?;
    Some((user + system) / 100.0)
}

/// One cold first sky near the Sun on a fresh server: the time from the request's sending to its
/// first reply's arrival, and that reply.
async fn first_reply() -> (Duration, Option<f64>, SkyResponse) {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .sky_service(SkyService::Served)
        .build();
    let server = TestServer::start_with(config).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Sky", SEED).await.id;
    let open = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(open).await {
        Ok(ResponseBody::OpenUniverse(_)) => {}
        other => panic!("expected the opened universe, got {other:?}"),
    }
    let request = SkyRequest {
        universe,
        observer: GalacticPosition {
            cell_ly: SUN_LY,
            offset_m: [0.0; 3],
        },
        time: UniverseTime::default(),
        eye: Some(EyeDto {
            field_factor: EyeObserver::DEFAULT_FIELD_FACTOR,
            age_years: EyeObserver::DEFAULT_AGE_YEARS,
            pigmentation: EyeObserver::DEFAULT_PIGMENTATION,
        }),
        camera_limit_v: None,
        n_max: None,
        cone: None,
        exclude_system: None,
    };
    let cpu_before = cpu_seconds();
    let started = Instant::now();
    let id = client.send_request(RequestBody::Sky(request)).await;
    let response = loop {
        match client.next_frame().await {
            Frame::Binary(..) => {}
            Frame::Message(
                ServerMessage::PartialResponse {
                    id: answered,
                    body: ResponseBody::Sky(response),
                }
                | ServerMessage::Response {
                    id: answered,
                    body: ResponseBody::Sky(response),
                },
            ) if answered == id => break *response,
            Frame::Message(other) => panic!("expected the sky's first reply, got {other:?}"),
        }
    };
    let elapsed = started.elapsed();
    let cpu = cpu_before
        .zip(cpu_seconds())
        .map(|(before, after)| after - before);
    if !response.is_final {
        // The rest of the sky is given up: its later replies' frames until its `cancelled`.
        client.cancel(id).await;
        loop {
            match client.next_frame().await {
                Frame::Message(ServerMessage::RequestError { id: ended, .. }) if ended == id => {
                    break;
                }
                Frame::Binary(..) | Frame::Message(ServerMessage::PartialResponse { .. }) => {}
                Frame::Message(other) => panic!("expected the sky's `cancelled`, got {other:?}"),
            }
        }
    }
    client.close().await;
    server.stop().await;
    (elapsed, cpu, response)
}

/// Prints what a first reply states: its cut, its stars, and each layer's radius and finality.
fn describe(elapsed: Duration, cpu: Option<f64>, response: &SkyResponse) {
    let layers: Vec<String> = response
        .census
        .iter()
        .map(|layer| {
            format!(
                "{:?} {} ly{} ({} listed)",
                layer.layer,
                layer.complete_to_ly,
                if layer.is_final { " final" } else { "" },
                layer.listed
            )
        })
        .collect();
    eprintln!(
        "first reply in {:.2} s wall, {} CPU-s: cut V {:.3}, {} listed, {} overflow, final {}; {}",
        elapsed.as_secs_f64(),
        cpu.map_or_else(|| "?".to_owned(), |cpu| format!("{cpu:.1}")),
        response.cut_v,
        response.listed,
        response.overflow,
        response.is_final,
        layers.join(", ")
    );
}

fn sky_near_sun_cold(c: &mut Criterion) {
    // The sky's phases and each step's census work, logged by the handler, to stderr, unless
    // `RUST_LOG` says otherwise.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("hyperion_server::requests::sky=debug"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
    let runtime = Runtime::new().expect("a Tokio runtime");
    let mut group = c.benchmark_group("sky_near_sun_cold");
    // Each iteration is a cold server's first sky, seconds of every worker.
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(1))
        .measurement_time(Duration::from_secs(120));
    group.bench_function("first_reply", |b| {
        b.iter_custom(|iterations| {
            runtime.block_on(async {
                let mut total = Duration::ZERO;
                for _ in 0..iterations {
                    let (elapsed, cpu, response) = first_reply().await;
                    describe(elapsed, cpu, &response);
                    total += elapsed;
                }
                total
            })
        });
    });
    group.finish();
}

criterion_group!(benches, sky_near_sun_cold);
criterion_main!(benches);
