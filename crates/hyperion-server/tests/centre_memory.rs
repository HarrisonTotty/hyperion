//! The server's peak memory at the galactic centre, measured by hand (plan 12, the mergers'
//! balance, part 3): the server binary run as a process of its own on its default workers, asked
//! cold range queries at the black hole or subscribed to an alert walk there, and its peak resident
//! set (`VmHWM` of `/proc/<pid>/status`, KiB) read once it has answered.
//!
//! A class cell of the stellar mergers about the centre draws some 12.2 million candidates, each
//! of whose places a range query read first and kept: 557 MiB a cell, and the eight cells that
//! meet at the centre 4.35 GiB (fix-wasm-range-hits). Linux only: other systems have no
//! `/proc/<pid>/status`, and the test says so and ends before it starts a server.

mod common;

use std::process::Stdio;
use std::time::{Duration, Instant};

use common::alerts::{opened_with, subscribe_in};
use common::process::server_binary;
use common::{TestClient, patiently};
use hyperion_protocol::{
    AlertBandDto, CatalogueClassWire, GalacticPosition, MassLayer, QueryModeDto, RequestBody,
    ResponseBody, SystemsInRangeRequest, UniverseIdHex, UniverseTime,
};
use hyperion_sim::coords::GalacticPosition as SimPosition;
use hyperion_sim::time::UniverseTime as SimTime;
use tokio::io::{AsyncBufReadExt, BufReader};

/// The universe's seed: the Milky Way-like fixture's.
const SEED: u64 = 0x0926_0000;

/// A `/proc/<pid>/status` field of `pid`, KiB, or `None` where there is none: a process that has
/// ended, whose figure is then printed as missing, no decision reading it.
fn status_kib(pid: u32, field: &str) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let line = status.lines().find(|line| line.starts_with(field))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// The CPU seconds `pid` has used, user and system, at Linux's 100 ticks a second, or `None` where
/// `/proc/<pid>/stat` cannot be read: printed as missing, no decision reading it.
fn cpu_seconds(pid: u32) -> Option<f64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields: Vec<&str> = stat.get(stat.rfind(')')? + 2..)?.split(' ').collect();
    let ticks = |field: usize| fields.get(field - 3)?.parse::<f64>().ok();
    Some((ticks(14)? + ticks(15)?) / 100.0)
}

/// FNV-1a of `bytes`, 64-bit: a digest the same on any toolchain, to compare two trees' answers.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Waits until `pid` has used no CPU for two seconds running, for at most `bound`, and says whether
/// it was: the protocol tells a subscriber nothing when its background walks end, so the server's
/// CPU is the only sign.
async fn until_idle(pid: u32, bound: Duration) -> bool {
    let started = Instant::now();
    let mut last = cpu_seconds(pid).unwrap_or(0.0);
    let mut idle = 0;
    while idle < 4 && started.elapsed() < bound {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let now = cpu_seconds(pid).unwrap_or(last);
        idle = if now - last < 0.05 { idle + 1 } else { 0 };
        last = now;
    }
    idle >= 4
}

/// The cold range queries at the black hole at the epoch, from layer D under the server's limit,
/// from 50 ly halved until the census admits a layer, as the sim's centre tests find their radius:
/// the first that reads the class cells is the cold one. Prints each answer's systems and the
/// digest of its census and systems (the rest echoes the request, whose universe's ID is new each
/// run).
async fn range(client: &mut TestClient, universe: &UniverseIdHex, started: Instant) {
    let mut radius_ly = 50.0;
    loop {
        let answer = client
            .request(RequestBody::SystemsInRange(SystemsInRangeRequest {
                universe: universe.clone(),
                centre: GalacticPosition {
                    cell_ly: [0, 0, 0],
                    offset_m: [0.0; 3],
                },
                radius_ly,
                time: UniverseTime {
                    seconds: 0,
                    nanos: 0,
                },
                min_layer: MassLayer::D,
                limit: hyperion_server::limits::MAX_CENSUS_LIMIT,
                include_stellar: false,
                mode: QueryModeDto::Now,
            }))
            .await;
        let answer = match answer {
            Ok(ResponseBody::SystemsInRange(answer)) => answer,
            Ok(other) => panic!("expected the systems in range, got {other:?}"),
            Err(refused) => panic!("{radius_ly} ly refused: {refused:?}"),
        };
        let digest = fnv1a(format!("{:?} {:?}", answer.census, answer.systems).as_bytes());
        eprintln!(
            "{radius_ly} ly answered at {:.1} s: {} systems, digest {digest:016x}",
            started.elapsed().as_secs_f64(),
            answer.systems.len()
        );
        if !answer.systems.is_empty() || radius_ly < 0.01 {
            return;
        }
        radius_ly *= 0.5;
    }
}

/// Measures the server's peak memory for one scenario, `HYPERION_CENTRE_SCENARIO`: `range`, the
/// cold range queries at the black hole ([`range`]); or `walk`, an alerts subscription there to the
/// stellar mergers and the slow accreting white dwarfs through V limits of 10⁻¹² and 10⁻¹⁰ W m⁻²,
/// waited on until the server is idle. Prints the peak and present resident sets, KiB, the server's
/// CPU seconds and the wall time.
///
/// Measured (pod, 32 vCPU, release, 2026-11-05; the mergers' balance, part 3): `range` peaked at
/// 5,658,388–5,821,104 KiB with every class cell's places kept; at 984,840–1,043,500 KiB with a
/// cell past the whole class-cell budget read as the query walks it; and at 337,384–338,772 KiB
/// past a quarter of it, the answers' digests the same; `walk` at some 491,000 KiB throughout,
/// reading no class cell.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "measurement, run by hand: HYPERION_CENTRE_SCENARIO=range cargo test --release -p hyperion-server --test centre_memory -- --ignored --nocapture"]
async fn the_servers_peak_memory_at_the_black_hole() {
    if !cfg!(target_os = "linux") {
        eprintln!("no /proc here: nothing measured");
        return;
    }
    let scenario = std::env::var("HYPERION_CENTRE_SCENARIO").unwrap_or_else(|_| "range".into());
    assert!(
        matches!(scenario.as_str(), "range" | "walk"),
        "HYPERION_CENTRE_SCENARIO is `range` or `walk`, not {scenario:?}"
    );
    let dir = tempfile::tempdir().unwrap();
    let mut child = server_binary()
        .args(["--port", "0", "--data-dir"])
        .arg(dir.path())
        .env("NO_COLOR", "1")
        .env("RUST_LOG", "info")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let pid = child.id().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let addr = patiently("waiting for the server's `listening` line", async {
        loop {
            let line = lines
                .next_line()
                .await
                .unwrap()
                .expect("the server listens");
            let addr = line
                .split_whitespace()
                .find_map(|field| field.strip_prefix("addr="));
            if let Some(addr) = addr.filter(|_| line.contains("listening")) {
                return addr.to_owned();
            }
        }
    })
    .await;
    // The server's log is read on, so that it never blocks on a full pipe.
    let drain =
        tokio::spawn(
            async move { while lines.next_line().await.expect("stdout reads").is_some() {} },
        );
    let mut client = TestClient::connect(&format!("ws://{addr}/ws")).await;
    client.hello().await;
    let universe = client.create_universe("Centre", SEED).await.id;
    let idle = (status_kib(pid, "VmHWM:"), cpu_seconds(pid));
    assert!(idle.0.is_some(), "the server's /proc/{pid}/status reads");
    let started = Instant::now();
    if scenario == "walk" {
        let centre = SimPosition::from_light_years([0.0, 0.0, 0.0]).unwrap();
        let request = subscribe_in(
            &universe,
            (&centre, SimTime::EPOCH),
            AlertBandDto::V,
            (1e-12, 1e-10),
            &[
                CatalogueClassWire::StellarMerger,
                CatalogueClassWire::AccretingWhiteDwarfSlow,
            ],
            None,
        );
        let (_, contacts, census) = opened_with(&mut client, request).await;
        eprintln!(
            "subscribed in {:.1} s: {} contacts, census {census:?}",
            started.elapsed().as_secs_f64(),
            contacts.len()
        );
        let quiet = until_idle(pid, Duration::from_mins(15)).await;
        eprintln!("idle: {quiet}");
    } else {
        // A peak resident set is a high-water mark, final once the query has answered.
        range(&mut client, &universe, started).await;
    }
    let cpu = cpu_seconds(pid)
        .zip(idle.1)
        .map(|(now, before)| now - before);
    eprintln!(
        "{scenario} at the black hole: peak RSS {:?} KiB (idle server {:?} KiB), now {:?} KiB, \
         {cpu:.1?} CPU-s, {:.1} s",
        status_kib(pid, "VmHWM:"),
        idle.0,
        status_kib(pid, "VmRSS:"),
        started.elapsed().as_secs_f64()
    );
    child.start_kill().unwrap();
    child.wait().await.expect("the killed server is reaped");
    patiently("the server's log to end", drain)
        .await
        .expect("the log drain ends");
}
