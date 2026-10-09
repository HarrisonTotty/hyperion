//! The server's first sky near the Sun, cold (rendering plan R06, R06.T11.d and T11.g; T17's
//! first-sky budget): a fresh server with the default workers, its cell cache and every other
//! cache empty, asked one eye-only sky near the Sun, timed from the request's sending to its first
//! reply arriving whole, binary frames and all. A miss is a finding to raise, not a CI failure: CI
//! compiles these and never runs them.
//!
//! `sky_near_sun_cold/first_reply` starts a server over the real socket for each iteration (its
//! start and the universe's creation are not timed), opens the universe, which starts the galaxy's
//! sky tables (R06.T11.g), and waits until the server holds them, timing their build from the
//! open's answer, wall and the process's CPU, as a figure of its own: the tables are a per-galaxy
//! cost under their own 30 CPU-s budget, not the sky's (decided 2026-10-08,
//! `decision-r06-t11d-first-sky.md` §2). Only then does its clock start, never after a warm-up sky,
//! which would warm the cell cache. It sends the default eye's sky at (0, 26,000, 68) ly at the
//! epoch with the default `n_max`, the derived caps by the eye's visibility and the request's
//! illumination, and times the first `partial_response` or `response`: the illumination, the eye's
//! cut and visibility, the caps and plan, the band's march, the first step of shells (every
//! layer's first shell: C, D and E to 125 ly), and that reply's merge, band, limit map, eye
//! offsets, payload and transfer. It then times the replies complete to 250 and 500 ly, and the
//! sky is cancelled: its full census near the Sun is of the order of 10⁴ CPU-s (R06.T17), beyond
//! any bench.
//!
//! `sky_near_sun_cold/session_first_reply` is a session's first sky: the sky is sent as soon as
//! the open is answered, with the tables' build still running, and timed from the open's answer
//! to the sky's first reply.
//!
//! Each iteration prints what each timed reply states and the process's CPU time to it where
//! `/proc/self/stat` gives it (Linux, at 100 ticks a second). The handler logs, to stderr, when
//! each of the sky's phases is done and, step by step, each layer's records read, systems
//! generated and census jobs' time, and the tables' service logs their build (`RUST_LOG`,
//! `hyperion_server::requests::sky=debug,hyperion_server::compute::sky_tables=info` by default).
//! Criterion's `sample_size(10)` is many minutes of runs: run it with `--test`, one iteration.
//!
//! The galaxy is the server's own for the sky tests' seed, `Galaxy::new(seed)` with its full
//! potential, not the sim benches' `milky_way_like` fixture, which no server builds. The budget is
//! T17's (decided 2026-10-03 and 2026-10-05, read with the tables held on 2026-10-08): the first
//! sky in at most 150 CPU-s and 10 s wall on the development machine's default workers. Figures
//! are recorded in R06's Risks ("Deviations in T11.d, as built" and "Deviations in T11.g, as
//! built"), each with its load.

#[path = "../tests/common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use common::{Frame, TestClient, TestServer};
use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_protocol::{
    EyeDto, GalacticPosition, OpenUniverseRequest, RequestBody, RequestId, ResponseBody,
    ServerMessage, SkyRequest, SkyResponse, UniverseIdHex, UniverseTime,
};
use hyperion_server::SkyService;
use hyperion_sim::sky::EyeObserver;
use tokio::runtime::Runtime;
use tracing_subscriber::EnvFilter;

/// The seed of the universe asked, the server sky tests' (`tests/sky.rs`).
const SEED: u64 = 0x4d2;

/// Near the Sun's place on the solar circle, ly, as the sim's and the server's sky tests stand.
const SUN_LY: [i32; 3] = [0, 26_000, 68];

/// The radius, ly, that the last reply timed is complete to in every layer: the third shell's
/// edge.
const LAST_TIMED_LY: f64 = 500.0;

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

/// The process's CPU time since `before`, s, where `/proc/self/stat` gives both.
fn cpu_since(before: Option<f64>) -> Option<f64> {
    before
        .zip(cpu_seconds())
        .map(|(before, after)| after - before)
}

/// The default eye's sky near the Sun at the epoch.
fn sky(universe: UniverseIdHex) -> SkyRequest {
    SkyRequest {
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
    }
}

/// A fresh server serving the sky, a client of it and a universe of [`SEED`] created, not opened.
async fn fresh() -> (tempfile::TempDir, TestServer, TestClient, UniverseIdHex) {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .sky_service(SkyService::Served)
        .build();
    let server = TestServer::start_with(config).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Sky", SEED).await.id;
    (data_dir, server, client, universe)
}

/// Opens `universe`, which starts its galaxy's sky tables.
async fn open(client: &mut TestClient, universe: &UniverseIdHex) {
    let open = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(open).await {
        Ok(ResponseBody::OpenUniverse(_)) => {}
        other => panic!("expected the opened universe, got {other:?}"),
    }
}

/// The least radius, ly, that a reply is complete to over its layers not yet final, or `None` for
/// a final reply.
fn least_edge(response: &SkyResponse) -> Option<f64> {
    response
        .census
        .iter()
        .filter(|layer| !layer.is_final)
        .map(|layer| layer.complete_to_ly)
        .reduce(f64::min)
}

/// One timed reply: its time from the clock's start, the process's CPU time to it, and the reply.
struct Timed {
    elapsed: Duration,
    cpu: Option<f64>,
    response: SkyResponse,
}

/// The replies to the sky `id`, each timed from `started` and `cpu_before`, until one is complete
/// to `until_ly` in every layer or final; then the sky is cancelled and its later frames drained.
async fn replies_until(
    client: &mut TestClient,
    id: RequestId,
    (started, cpu_before): (Instant, Option<f64>),
    until_ly: f64,
) -> Vec<Timed> {
    let mut timed = Vec::new();
    loop {
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
            ) if answered == id => {
                let reached = least_edge(&response).is_none_or(|edge| edge >= until_ly);
                let last = response.is_final;
                timed.push(Timed {
                    elapsed: started.elapsed(),
                    cpu: cpu_since(cpu_before),
                    response: *response,
                });
                if last {
                    return timed;
                }
                if reached {
                    break;
                }
            }
            Frame::Message(other) => panic!("expected the sky's replies, got {other:?}"),
        }
    }
    // The rest of the sky is given up: its later replies' frames until its `cancelled`.
    client.cancel(id).await;
    loop {
        match client.next_frame().await {
            Frame::Message(ServerMessage::RequestError { id: ended, .. }) if ended == id => break,
            Frame::Message(ServerMessage::Response { id: ended, .. }) if ended == id => break,
            Frame::Binary(..) | Frame::Message(ServerMessage::PartialResponse { .. }) => {}
            Frame::Message(other) => panic!("expected the sky's `cancelled`, got {other:?}"),
        }
    }
    timed
}

/// One cold first sky near the Sun on a fresh server whose tables are held, and the replies after
/// it to [`LAST_TIMED_LY`], each timed from the request's sending; and the tables' build, wall and
/// CPU, from the open's answer.
async fn first_reply() -> (Duration, Option<f64>, Vec<Timed>) {
    let (data_dir, server, mut client, universe) = fresh().await;
    open(&mut client, &universe).await;
    let (opened, cpu_opened) = (Instant::now(), cpu_seconds());
    server
        .stats_until("the galaxy's sky tables are held", |stats| {
            stats.sky_tables().cache().entries() == 1
        })
        .await;
    let (tables_wall, tables_cpu) = (opened.elapsed(), cpu_since(cpu_opened));
    let clock = (Instant::now(), cpu_seconds());
    let id = client.send_request(RequestBody::Sky(sky(universe))).await;
    let replies = replies_until(&mut client, id, clock, LAST_TIMED_LY).await;
    client.close().await;
    server.stop().await;
    drop(data_dir);
    (tables_wall, tables_cpu, replies)
}

/// A session's first sky: asked as soon as the open is answered, its tables still building, and
/// timed from the open's answer.
async fn session_first_reply() -> Timed {
    let (data_dir, server, mut client, universe) = fresh().await;
    open(&mut client, &universe).await;
    let clock = (Instant::now(), cpu_seconds());
    let id = client.send_request(RequestBody::Sky(sky(universe))).await;
    let mut replies = replies_until(&mut client, id, clock, 0.0).await;
    client.close().await;
    server.stop().await;
    drop(data_dir);
    replies.remove(0)
}

/// Prints what a timed reply states: its cut, its stars, and each layer's radius and finality.
fn describe(what: &str, timed: &Timed) {
    let response = &timed.response;
    let layers: Vec<String> = response
        .census
        .iter()
        .map(|layer| {
            format!(
                "{:?} {} ly{} ({} listed, {} cells, {} generated)",
                layer.layer,
                layer.complete_to_ly,
                if layer.is_final { " final" } else { "" },
                layer.listed,
                layer.cells,
                layer.candidates_opened
            )
        })
        .collect();
    eprintln!(
        "{what} in {:.2} s wall, {} CPU-s: cut V {:.3}, {} listed, {} overflow, final {}; {}",
        timed.elapsed.as_secs_f64(),
        timed
            .cpu
            .map_or_else(|| "?".to_owned(), |cpu| format!("{cpu:.1}")),
        response.cut_v,
        response.listed,
        response.overflow,
        response.is_final,
        layers.join(", ")
    );
}

fn sky_near_sun_cold(c: &mut Criterion) {
    // The sky's phases, each step's census work layer by layer, and the tables' build, logged by
    // the server, to stderr, unless `RUST_LOG` says otherwise.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "hyperion_server::requests::sky=debug,hyperion_server::compute::sky_tables=info",
        )
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
    let runtime = Runtime::new().expect("a Tokio runtime");
    let mut group = c.benchmark_group("sky_near_sun_cold");
    // Each iteration is a cold server's first sky, seconds of every worker, and then minutes of
    // its later replies.
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(1))
        .measurement_time(Duration::from_secs(120));
    group.bench_function("first_reply", |b| {
        b.iter_custom(|iterations| {
            runtime.block_on(async {
                let mut total = Duration::ZERO;
                for _ in 0..iterations {
                    let (tables_wall, tables_cpu, replies) = first_reply().await;
                    eprintln!(
                        "the galaxy's sky tables held {:.2} s after the open's answer, {} CPU-s",
                        tables_wall.as_secs_f64(),
                        tables_cpu.map_or_else(|| "?".to_owned(), |cpu| format!("{cpu:.1}"))
                    );
                    for (k, timed) in replies.iter().enumerate() {
                        let what = match least_edge(&timed.response) {
                            Some(edge) => format!("reply {k}, complete to {edge} ly,"),
                            None => format!("reply {k}, final,"),
                        };
                        describe(&what, timed);
                    }
                    total += replies.first().expect("a first reply").elapsed;
                }
                total
            })
        });
    });
    group.bench_function("session_first_reply", |b| {
        b.iter_custom(|iterations| {
            runtime.block_on(async {
                let mut total = Duration::ZERO;
                for _ in 0..iterations {
                    let timed = session_first_reply().await;
                    describe("the session's first reply, from the open's answer,", &timed);
                    total += timed.elapsed;
                }
                total
            })
        });
    });
    group.finish();
}

criterion_group!(benches, sky_near_sun_cold);
criterion_main!(benches);
