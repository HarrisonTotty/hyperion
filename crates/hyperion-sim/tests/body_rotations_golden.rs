//! The rotation laws the wire carries (plan 14, P14.T46.f), pinned in
//! `golden/frame/body_rotations.golden` for the client's TypeScript twin of `body_fixed_at`, which
//! is tested against it to 10⁻⁹ rad (rendering plan R07, item 5 of the rendering lanes' decisions).
//!
//! The bodies are every body with a frame in the three golden systems of the RM1 fixture universe
//! (`0x42006cba00000009`, `0x41ffecae00000004`, `0x42002cb200000009`, the scene fixture's) at the
//! epoch, and three synthetic laws built here that lock inside the clock window: a synchronous
//! body that locks 30 years after the epoch, one that locked 30 years before it, and a 3:2 body
//! on Mercury's eccentricity that locks 30 years after it. Each is pinned at the epoch, at ±1 h,
//! ±1 d and ±1 yr of it, and, for a body that locks within the window, at 1 s, 1 h, 1 d, 1 yr and
//! 10 yr either side of its lock, wherever the time lies in the window.
//!
//! # Line format
//!
//! After the `# generator_version` header and a comment line, each law is one `body` line and then
//! one `w` line per time:
//!
//! ```text
//! body <body id or synthetic name> pole=<x>,<y>,<z> node=<x>,<y>,<z> quarter=<x>,<y>,<z>
//!   obliquity=<rad> initial_rate=<rad/s> locked_rate=<rad/s> age_at_epoch=<s> locking_age=<s or ->
//!   locks_at=<seconds>,<nanos or -> resonance=<synchronous|three_to_two> clock_period=<s>
//!   clock_m0=<rad> sub_primary=<rad> phase_at_epoch=<rad> capture_phase=<rad>
//! w t=<seconds>,<nanos> w=<rad>
//! ```
//!
//! (the `body` line is one line, wrapped here). The fields are the wire's `BodyRotationDto`'s, in
//! its order; a time is the wire's `UniverseTime`, whole seconds from the epoch and nanoseconds; a
//! body ID is its wire form, `<system hex>.<body index hex>`. Every float is Rust's shortest
//! round-trip decimal, which `parseFloat` reads back to the same `f64`, and W is
//! `RotationLaw::angle_at` itself, in `[0, 2π)`. The TypeScript twin skips the two header lines.

use std::f64::consts::TAU;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::SystemId;
use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
use hyperion_sim::planetary::derive::rotation::{RotationLawParts, SpinOrbitResonance};
use hyperion_sim::planetary::frames::{BodyFixedFrame, FrameSpin};
use hyperion_sim::planetary::record::Section;
use hyperion_sim::planetary::{self, SystemContext};
use hyperion_sim::time::{ClockWindow, Span, UniverseTime};
use hyperion_sim::units::consts::{METRES_PER_AU, SECONDS_PER_JULIAN_YEAR};
use hyperion_sim::units::{GravitationalParameter, Metres, Radians, Seconds, SolarMasses, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The seed of the RM1 fixture universe (the server's scene fixture's).
const SYSTEMS_SEED: u64 = 0x5eed_0000_0014_0032;

/// Plan 14's three golden systems in that universe: Solar-like, a wide binary and a close binary.
const SYSTEMS: [u64; 3] = [
    0x4200_6cba_0000_0009,
    0x41ff_ecae_0000_0004,
    0x4200_2cb2_0000_0009,
];

/// A Julian year, s, whole.
const JULIAN_YEAR_S: i64 = 31_557_600;

/// The offsets from the epoch every law is pinned at, s.
const AROUND_EPOCH_S: [i64; 7] = [
    -JULIAN_YEAR_S,
    -86_400,
    -3_600,
    0,
    3_600,
    86_400,
    JULIAN_YEAR_S,
];

/// The offsets from a lock inside the window a law is pinned at, s.
const AROUND_LOCK_S: [i64; 10] = [
    -10 * JULIAN_YEAR_S,
    -JULIAN_YEAR_S,
    -86_400,
    -3_600,
    -1,
    1,
    3_600,
    86_400,
    JULIAN_YEAR_S,
    10 * JULIAN_YEAR_S,
];

/// One pinned law: its name in the file, and its frame.
struct Pinned {
    name: String,
    frame: BodyFixedFrame,
}

/// Every body with a frame in the three golden systems at the epoch, in system then index order.
fn golden_bodies() -> Vec<Pinned> {
    let galaxy = Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let mut out = Vec::new();
    for raw in SYSTEMS {
        let id = SystemId::from_raw(raw).expect("a pinned ID is well formed");
        let context = SystemContext::for_system(&galaxy, id).expect("a pinned ID names a system");
        let system = planetary::generate(galaxy.seed(), &context);
        let snapshot = system.snapshot_at(&context, UniverseTime::EPOCH);
        for record in snapshot.bodies() {
            if let Section::Ok(frame) = record.rotation() {
                out.push(Pinned {
                    name: format!("{raw:016x}.{:04x}", record.index().get()),
                    frame: *frame,
                });
            }
        }
    }
    assert!(
        out.len() > 10,
        "the golden systems hold {} bodies with a frame",
        out.len()
    );
    out
}

/// A synthetic law on an orbit of eccentricity `e` about a 0.2 M☉ star at 0.07 au, in a system
/// 4 Gyr old, that locks `lock_years` after the epoch (before it where negative).
fn synthetic(name: &str, e: f64, lock_years: f64) -> Pinned {
    let orientation = Orientation::new(Radians::new(0.3), Radians::new(1.0), Radians::new(2.0))
        .expect("a valid orientation");
    let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(0.2));
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::new(0.07 * METRES_PER_AU),
        mu,
        Eccentricity::new(e).expect("an elliptic orbit"),
        orientation,
        Radians::new(0.7),
    )
    .expect("a valid orbit");
    let age = Years::new(4e9);
    let spin = FrameSpin {
        obliquity: Radians::new(0.2),
        pole_azimuth: Radians::new(0.4),
        primordial_period: Seconds::new(54_000.0),
        locking_time: Seconds::new(
            Seconds::from(age).value() + lock_years * SECONDS_PER_JULIAN_YEAR,
        ),
        phase_at_epoch: Radians::new(1.1),
    };
    let frame = BodyFixedFrame::new(&orbit, &spin, age).expect("a valid law");
    assert!(
        frame.rate().locks_at().is_some(),
        "{name} locks within the clock's range"
    );
    Pinned {
        name: name.to_owned(),
        frame,
    }
}

/// The times `law` is pinned at, in order, each inside the window.
fn times(parts: &RotationLawParts) -> Vec<UniverseTime> {
    let mut out: Vec<UniverseTime> = AROUND_EPOCH_S
        .iter()
        .map(|&s| {
            UniverseTime::EPOCH
                .checked_add(Span::from_seconds(s))
                .expect("in range")
        })
        .collect();
    if let Some(lock) = parts.locks_at.filter(|&at| ClockWindow::contains(at)) {
        out.extend(
            AROUND_LOCK_S
                .iter()
                .filter_map(|&s| lock.checked_add(Span::from_seconds(s))),
        );
    }
    out.retain(|&t| ClockWindow::contains(t));
    out.sort();
    out.dedup();
    out
}

/// `x` as the file writes it: finite, so that its shortest round-trip decimal stands for its bits
/// alone (a NaN's would not).
fn float(x: f64) -> String {
    assert!(x.is_finite(), "a pinned value is finite: {x}");
    format!("{x:?}")
}

/// A vector as the file writes it.
fn vector(v: [f64; 3]) -> String {
    format!("{},{},{}", float(v[0]), float(v[1]), float(v[2]))
}

/// The `body` line of a law.
fn body_line(pinned: &Pinned) -> String {
    let frame = &pinned.frame;
    let parts = frame.rate().parts();
    let locking_age = parts
        .locking_age
        .map_or_else(|| "-".to_owned(), |age| float(age.value()));
    let locks_at = parts.locks_at.map_or_else(
        || "-".to_owned(),
        |at| format!("{},{}", at.seconds(), at.subsec_nanos()),
    );
    let resonance = match parts.resonance {
        SpinOrbitResonance::Synchronous => "synchronous",
        SpinOrbitResonance::ThreeToTwo => "three_to_two",
    };
    format!(
        "body {} pole={} node={} quarter={} obliquity={} initial_rate={} locked_rate={} \
         age_at_epoch={} locking_age={locking_age} locks_at={locks_at} resonance={resonance} \
         clock_period={} clock_m0={} sub_primary={} phase_at_epoch={} capture_phase={}",
        pinned.name,
        vector(frame.pole()),
        vector(frame.equator_node()),
        vector(frame.equator_quarter()),
        float(frame.obliquity().value()),
        float(parts.initial_rate),
        float(parts.locked_rate),
        float(parts.age_at_epoch.value()),
        float(parts.clock_period.value()),
        float(parts.clock_mean_anomaly_at_epoch.value()),
        float(parts.sub_primary_angle.value()),
        float(parts.phase_at_epoch.value()),
        float(parts.capture_phase.value()),
    )
}

/// Every pinned law: the golden systems' bodies, then the synthetic ones.
fn pinned_laws() -> Vec<Pinned> {
    let mut laws = golden_bodies();
    laws.push(synthetic("synthetic_synchronous_ahead", 0.02, 30.0));
    laws.push(synthetic("synthetic_synchronous_behind", 0.02, -30.0));
    laws.push(synthetic("synthetic_three_to_two_ahead", 0.2056, 30.0));
    laws
}

/// The file's text.
fn golden_text(laws: &[Pinned]) -> String {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(&format!(
        "# seed 0x{SYSTEMS_SEED:016x}, milky_way_like, the three golden systems at the epoch and \
         three synthetic laws"
    ));
    for pinned in laws {
        w.line(&body_line(pinned));
        for t in times(&pinned.frame.rate().parts()) {
            let angle = pinned.frame.rate().angle_at(t).value();
            w.line(&format!(
                "w t={},{} w={}",
                t.seconds(),
                t.subsec_nanos(),
                float(angle)
            ));
        }
    }
    w.finish()
}

/// One law as the twin reads it back from a `body` line.
struct Parsed {
    initial_rate: f64,
    locked_rate: f64,
    age_at_epoch: f64,
    locking_age: Option<f64>,
    locks_at: Option<(i64, u32)>,
    three_to_two: bool,
    clock_period: f64,
    clock_m0: f64,
    sub_primary: f64,
    phase_at_epoch: f64,
    capture_phase: f64,
}

/// The value of `key` on a `body` line.
fn field<'a>(line: &'a str, key: &str) -> &'a str {
    line.split(' ')
        .find_map(|token| {
            token
                .strip_prefix(key)
                .and_then(|rest| rest.strip_prefix('='))
        })
        .unwrap_or_else(|| panic!("no {key} on {line}"))
}

/// A `seconds,nanos` time.
fn time(text: &str) -> (i64, u32) {
    let (seconds, nanos) = text.split_once(',').expect("seconds,nanos");
    (seconds.parse().unwrap(), nanos.parse().unwrap())
}

fn parse(line: &str) -> Parsed {
    let float = |key: &str| field(line, key).parse::<f64>().unwrap();
    Parsed {
        initial_rate: float("initial_rate"),
        locked_rate: float("locked_rate"),
        age_at_epoch: float("age_at_epoch"),
        locking_age: Some(field(line, "locking_age"))
            .filter(|text| *text != "-")
            .map(|text| text.parse().unwrap()),
        locks_at: Some(field(line, "locks_at"))
            .filter(|text| *text != "-")
            .map(time),
        three_to_two: field(line, "resonance") == "three_to_two",
        clock_period: float("clock_period"),
        clock_m0: float("clock_m0"),
        sub_primary: float("sub_primary"),
        phase_at_epoch: float("phase_at_epoch"),
        capture_phase: float("capture_phase"),
    }
}

/// Seconds from the epoch to `(seconds, nanos)`, as the wire's client forms them.
fn offset((seconds, nanos): (i64, u32)) -> f64 {
    // Whole seconds within the window (±3.2 × 10¹⁰) are exact in f64.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the window's whole seconds are exact in f64"
    )]
    let whole = seconds as f64;
    whole + f64::from(nanos) / 1e9
}

/// W at `t` from a parsed law alone, by the closed forms of `BodyRotationDto`'s documentation:
/// what the client's twin computes.
fn twin_angle(law: &Parsed, t: (i64, u32)) -> f64 {
    let (w0, wl, se) = (law.initial_rate, law.locked_rate, law.age_at_epoch);
    let rate = |x: f64| {
        law.locking_age
            .map_or(w0, |tau| w0 + (wl - w0) * (x / tau).clamp(0.0, 1.0))
    };
    let swept = |a: f64, b: f64| {
        let span = b - a;
        rate(se + a) * span
            + law
                .locking_age
                .map_or(0.0, |tau| (wl - w0) * span * span / (2.0 * tau))
    };
    let p = if law.three_to_two { 3.0 } else { 1.0 };
    let locked = |s: f64| {
        let period = law.clock_period;
        let m = law.clock_m0 + TAU * s.rem_euclid(period) / period;
        law.sub_primary + p * m.rem_euclid(TAU)
    };
    let s = offset(t);
    let w = match law.locks_at {
        None => law.phase_at_epoch + swept(0.0, s),
        Some(lock) if t >= lock => locked(s),
        Some(lock) => {
            let d = law
                .locking_age
                .expect("a body that locks has a locking age")
                - se;
            if d > 0.0 {
                let share = s.max(0.0) / d;
                law.phase_at_epoch + swept(0.0, s) + law.capture_phase * share * share
            } else {
                locked(offset(lock)) - swept(s, d)
            }
        }
    };
    w.rem_euclid(TAU)
}

/// The difference of two angles, wrapped into `[−π, π)`.
fn centred(x: f64) -> f64 {
    (x + TAU / 2.0).rem_euclid(TAU) - TAU / 2.0
}

/// P14.T46.f: the laws are pinned, and the file holds everything the twin needs: W read back from
/// each `body` line by the closed forms alone matches the pinned `angle_at` to 10⁻⁹ rad, and the
/// pinned W is `angle_at` bit for bit.
#[test]
fn the_body_rotations_are_pinned_for_the_client_twin() {
    let laws = pinned_laws();
    let text = golden_text(&laws);
    let mut current: Option<Parsed> = None;
    let mut checked = 0_u32;
    let mut locks_in_window = 0_u32;
    for line in text.lines().skip(2) {
        if line.starts_with("body ") {
            let parsed = parse(line);
            if parsed.locks_at.is_some_and(|(s, n)| {
                ClockWindow::contains(UniverseTime::new(s, n).expect("a valid time"))
            }) {
                locks_in_window += 1;
            }
            current = Some(parsed);
            continue;
        }
        let law = current.as_ref().expect("a w line follows a body line");
        let t = time(field(line, "t"));
        let pinned: f64 = field(line, "w").parse().unwrap();
        let twin = twin_angle(law, t);
        assert!(
            centred(twin - pinned).abs() < 1e-9,
            "{line}: the twin gives {twin}"
        );
        checked += 1;
    }
    assert!(
        locks_in_window >= 3,
        "{locks_in_window} laws lock in the window"
    );
    assert!(checked > 100, "{checked} times");
    // The pinned decimal is `angle_at` bit for bit.
    for pinned in &laws {
        for t in times(&pinned.frame.rate().parts()) {
            let angle = pinned.frame.rate().angle_at(t).value();
            let reread: f64 = format!("{angle:?}").parse().unwrap();
            assert!(
                reread.total_cmp(&angle).is_eq(),
                "{}: {angle:?} reads back as {reread:?}",
                pinned.name
            );
        }
    }
    golden!("frame/body_rotations", &text);
}
