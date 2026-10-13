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
//! sky is cancelled, unless `HYPERION_SKY_BENCH_UNTIL_LY` asks more (below): its full census near
//! the Sun is some 2.8 × 10⁴ CPU-s (R06.T17).
//!
//! `sky_near_sun_cold/session_first_reply` is a session's first sky: the sky is sent as soon as
//! the open is answered, with the tables' build still running, and timed from the open's answer
//! to the sky's first reply.
//!
//! For R06.T17, `HYPERION_SKY_BENCH_UNTIL_LY` sets the radius the last timed reply is complete to,
//! or `final` to time every reply to the last, the full census (some 2–3 × 10⁴ CPU-s near the
//! Sun, half an hour or more on 15 workers); and `HYPERION_WORKERS`, the server's own variable,
//! sets its pool's workers, as for the laptop's proxy of four. Each reply's stars are read from its
//! payload, and each states how many are brighter than V 3.0; a run to the last reply then times
//! the delivery of the final reply's stars brighter than V 3.0, the first reply that lists them
//! all, against a quarter of the full census's wall time (R13.T2's real boundary, decided
//! 2026-10-09; recorded, not gated, at version 21, `decision-r06-t17-findings.md` §1). Beside it
//! the run prints how many of them each reply lists, at what share of the final's wall (R06.T11.h).
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
//! are recorded in R06's Risks ("Deviations in T11.d, as built", "Deviations in T11.g, as built"
//! and "Deviations in T11.h, as built"), each with its load.
//!
//! R06.T17 measured it on 2026-10-11 and 2026-10-12, each run under the heavy-test lock on a shared
//! machine, so every figure is provisional (R06's Risks, "Deviations in T17, as built"):
//!
//! - on 15 workers at load 6–14, the first reply in 6.95 s wall and 93.9 CPU-s (met), 3,192 stars;
//!   the session's first reply 18.0 s after the open's answer; the tables 11.6 s and 47.0 CPU-s
//!   (over their 30 CPU-s);
//! - on four workers (`HYPERION_WORKERS=4`, load 14–22), the first reply in 17.9 s and 64.9 CPU-s;
//! - run whole (`final`, 15 workers, load 10–47), the final reply after 1,987 s and 25,286 CPU-s,
//!   66,614 stars listed and none overflowing. Its 805 stars brighter than V 3.0 are all listed only
//!   by the final reply, against a quarter of its wall, 497 s: missed.
//!
//! R06.T11.h ran it whole again on 2026-10-13, with E's shells from 1,000 ly a step ahead of C's
//! and D's (`final`, 15 workers, under the heavy-test lock, load 11–42; provisional). E's
//! 1,000–2,000 ly reply came after 1,160 s, 45.2% of the final reply's 2,567 s and 25,425 CPU-s,
//! listing 804 of the final reply's 805 stars brighter than V 3.0, where T17's replies held 773
//! until the final. The last of them lies in C's or D's last shell, so the delivery of all 805 is
//! still the final reply's: missed, against 642 s, and recorded, not gated, at version 21. The
//! final reply lists the same 66,614 stars, layer by layer, as T17's.

#[path = "../tests/common/mod.rs"]
mod common;

use std::collections::HashSet;
use std::env::VarError;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use common::{Frame, TestClient, TestServer};
use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_protocol::{
    EyeDto, GalacticPosition, OpenUniverseRequest, RequestBody, RequestId, ResponseBody,
    SKY_STAR_BYTES, ServerMessage, SkyRequest, SkyResponse, UniverseIdHex, UniverseTime,
};
use hyperion_server::SkyService;
use hyperion_server::config::ENV_WORKERS;
use hyperion_sim::sky::EyeObserver;
use tokio::runtime::Runtime;
use tracing_subscriber::EnvFilter;

/// The seed of the universe asked, the server sky tests' (`tests/sky.rs`).
const SEED: u64 = 0x4d2;

/// Near the Sun's place on the solar circle, ly, as the sim's and the server's sky tests stand.
const SUN_LY: [i32; 3] = [0, 26_000, 68];

/// The radius, ly, that the last reply timed is complete to in every layer: the third shell's
/// edge, unless `HYPERION_SKY_BENCH_UNTIL_LY` sets another ([`until_ly`]).
const LAST_TIMED_LY: f64 = 500.0;

/// The variable that sets the radius the last timed reply is complete to, ly, or `final`.
const ENV_UNTIL_LY: &str = "HYPERION_SKY_BENCH_UNTIL_LY";

/// The apparent V brighter than which the final reply's stars are timed to their delivery, the
/// V 3.0 gate's (R06.T17).
const BRIGHT_V: f64 = 3.0;

/// The radius, ly, the last timed reply is complete to: [`ENV_UNTIL_LY`]'s, infinite for `final`,
/// or [`LAST_TIMED_LY`].
///
/// # Panics
///
/// If the variable is set to neither `final` nor a positive radius, or is not Unicode.
fn until_ly() -> f64 {
    match std::env::var(ENV_UNTIL_LY) {
        Ok(text) if text == "final" => f64::INFINITY,
        Ok(text) => match text.parse::<f64>() {
            Ok(ly) if ly > 0.0 => ly,
            Ok(ly) => panic!("{ENV_UNTIL_LY}={text}: a radius of {ly} ly is not positive"),
            Err(error) => panic!("{ENV_UNTIL_LY}={text}: neither `final` nor a radius ({error})"),
        },
        Err(VarError::NotPresent) => LAST_TIMED_LY,
        Err(VarError::NotUnicode(text)) => {
            panic!("{ENV_UNTIL_LY}={} is not Unicode", text.display())
        }
    }
}

/// The pool's workers that `HYPERION_WORKERS` sets, the server's own variable, or `None` for the
/// server's default.
///
/// # Panics
///
/// If the variable is set to anything but a positive count, or is not Unicode.
fn workers() -> Option<NonZeroUsize> {
    match std::env::var(ENV_WORKERS) {
        Ok(text) => Some(text.parse().unwrap_or_else(|error| {
            panic!("{ENV_WORKERS}={text}: not a positive count ({error})")
        })),
        Err(VarError::NotPresent) => None,
        Err(VarError::NotUnicode(text)) => {
            panic!("{ENV_WORKERS}={} is not Unicode", text.display())
        }
    }
}

/// A listed star as the wire carries it (Design note 17): its direction's and distance's bits,
/// which tell one star from another across replies, and its apparent V in millimagnitudes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WireStar {
    place: [u32; 4],
    v_mmag: i16,
}

/// The stars of a reply's `payload`, whose response is `response`.
///
/// # Panics
///
/// If the payload is shorter than the response's star bytes, or holds a part of a star.
fn wire_stars(response: &SkyResponse, payload: &[u8]) -> Vec<WireStar> {
    let stars_bytes = usize::try_from(response.stars_bytes).expect("a payload in memory");
    assert!(
        payload.len() >= stars_bytes,
        "a payload of {} bytes for {stars_bytes} bytes of stars",
        payload.len()
    );
    let (stars, rest) = payload[..stars_bytes].as_chunks::<SKY_STAR_BYTES>();
    assert!(rest.is_empty(), "whole stars");
    let word = |star: &[u8; SKY_STAR_BYTES], at: usize| {
        u32::from_le_bytes(star[at..at + 4].try_into().expect("4 bytes"))
    };
    stars
        .iter()
        .map(|star| WireStar {
            place: [word(star, 0), word(star, 4), word(star, 8), word(star, 12)],
            v_mmag: i16::from_le_bytes([star[16], star[17]]),
        })
        .collect()
}

/// Whether `star` is brighter than [`BRIGHT_V`].
fn bright(star: &WireStar) -> bool {
    f64::from(star.v_mmag) < BRIGHT_V * 1_000.0
}

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

/// A fresh server serving the sky, a client of it and a universe of [`SEED`] created, not opened;
/// its pool [`workers`]' where the variable sets them.
async fn fresh() -> (tempfile::TempDir, TestServer, TestClient, UniverseIdHex) {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let mut config = TestServer::config(data_dir.path()).sky_service(SkyService::Served);
    if let Some(workers) = workers() {
        config = config.workers(workers);
    }
    let config = config.build();
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

/// One timed reply: its time from the clock's start, the process's CPU time to it, the reply, and
/// its listed stars, read from its payload.
struct Timed {
    elapsed: Duration,
    cpu: Option<f64>,
    response: SkyResponse,
    stars: Vec<WireStar>,
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
    // Each reply's chunks arrive in order before its response (R03.T15).
    let mut payload = Vec::new();
    loop {
        match client.next_frame().await {
            Frame::Binary(_, chunk) => payload.extend_from_slice(&chunk),
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
                let (elapsed, cpu) = (started.elapsed(), cpu_since(cpu_before));
                let stars = wire_stars(&response, &payload);
                payload.clear();
                timed.push(Timed {
                    elapsed,
                    cpu,
                    response: *response,
                    stars,
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
    let replies = replies_until(&mut client, id, clock, until_ly()).await;
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
        "{what} in {:.2} s wall, {} CPU-s: cut V {:.3}, {} listed ({} brighter than V {BRIGHT_V}), \
         {} overflow, final {}; {}",
        timed.elapsed.as_secs_f64(),
        timed
            .cpu
            .map_or_else(|| "?".to_owned(), |cpu| format!("{cpu:.1}")),
        response.cut_v,
        response.listed,
        timed.stars.iter().filter(|star| bright(star)).count(),
        response.overflow,
        response.is_final,
        layers.join(", ")
    );
}

/// Prints, for a run to the final reply, how many of the final reply's stars brighter than V 3.0
/// each reply lists, at what share of the final reply's time (R06.T11.h: E's far shell's reply,
/// against T17's), and when they were all delivered: the first reply listing every one of them,
/// against a quarter of the final reply's time, the gate (R06.T17; decided 2026-10-09 against the
/// capped census's wall time; recorded, not gated, at version 21, `decision-r06-t17-findings.md`
/// §1).
fn report_bright_delivery(replies: &[Timed]) {
    let Some(last) = replies.last().filter(|timed| timed.response.is_final) else {
        return;
    };
    let wanted: HashSet<WireStar> = last.stars.iter().copied().filter(bright).collect();
    let wall = last.elapsed.as_secs_f64();
    for (k, timed) in replies.iter().enumerate() {
        let listed: HashSet<WireStar> = timed.stars.iter().copied().collect();
        let held = wanted.intersection(&listed).count();
        let at = timed.elapsed.as_secs_f64();
        eprintln!(
            "reply {k}, {at:.2} s after the request, {:.1}% of the final's {wall:.2} s, lists \
             {held} of the final reply's {} stars brighter than V {BRIGHT_V}",
            100.0 * at / wall,
            wanted.len()
        );
    }
    let delivered = replies.iter().position(|timed| {
        let listed: HashSet<WireStar> = timed.stars.iter().copied().collect();
        wanted.is_subset(&listed)
    });
    let quarter = last.elapsed.as_secs_f64() / 4.0;
    match delivered {
        Some(k) => {
            let at = replies[k].elapsed.as_secs_f64();
            eprintln!(
                "the final reply's {} stars brighter than V {BRIGHT_V} were all delivered by reply \
                 {k}, {at:.2} s after the request, against a quarter of the final's {:.2} s, \
                 {quarter:.2} s: {}",
                wanted.len(),
                last.elapsed.as_secs_f64(),
                if at <= quarter { "met" } else { "missed" }
            );
        }
        None => eprintln!("the final reply's stars brighter than V {BRIGHT_V}: not all delivered"),
    }
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
                    report_bright_delivery(&replies);
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
