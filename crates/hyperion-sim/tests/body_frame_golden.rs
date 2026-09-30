//! The body-frame rule's answers (plan R02, R02.T4.b), pinned in `golden/frame/body_frames.golden`
//! for the client's TypeScript twin, `selectCameraFrame` (R02.T8.a), which reads every line of it.
//!
//! The 200 cases are drawn from the testkit's `lcg`, so no domain tag is registered and nothing
//! here is generated output: a planet or two about a star, their moons and now and then a moon's
//! moon, with ratios of distance to Hill radius crowded about the entry (0.9) and exit (1)
//! boundaries, exact ties between siblings, and a current frame that is none, a candidate, or a
//! body no longer among the candidates.
//!
//! # Line format
//!
//! After the `# generator_version` header and a blank line, one case per line:
//!
//! ```text
//! current=<body id or -> answer=<body id or -> candidates=<id>/<parent or ->/<distance>/<hill>;…
//! ```
//!
//! A body ID is its wire form, `<system hex>.<body index hex>`; distances and Hill radii are
//! metres, written as Rust's shortest round-trip decimal, which `parseFloat` reads back to the same
//! `f64`. The TypeScript twin skips the header line.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::planetary::body_frame::{BodyFrameCandidate, select_body_frame};
use hyperion_sim::units::Metres;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

/// The number of cases.
const CASES: usize = 200;

/// Ratios on the rule's boundaries and just either side of them.
const BOUNDARIES: [f64; 8] = [
    0.9,
    1.0,
    0.899_999_9,
    0.900_000_1,
    0.999_999_9,
    1.000_000_1,
    0.95,
    0.0,
];

/// A ratio of distance to Hill radius: on or beside a boundary a third of the time, otherwise
/// anywhere in 0–1.2.
fn ratio(rng: &mut Lcg) -> f64 {
    if rng.next_below(3) == 0 {
        let pick = usize::try_from(rng.next_below(8)).expect("below 8");
        BOUNDARIES[pick]
    } else {
        rng.next_f64() * 1.2
    }
}

/// A Hill radius, log-uniform from 10⁶ to 10¹⁰ m.
fn hill(rng: &mut Lcg) -> f64 {
    hyperion_sim::math::exp10(6.0 + 4.0 * rng.next_f64())
}

/// A distinct body index for the next body, scattered so that ID order is not creation order.
fn next_index(rng: &mut Lcg, used: &mut Vec<u16>) -> u16 {
    loop {
        let index = u16::try_from(1 + rng.next_below(0x0fff)).expect("below 0x1000");
        if !used.contains(&index) {
            used.push(index);
            return index;
        }
    }
}

/// A candidate `id` orbiting `parent`; now and then an exact tie with the last of `siblings`.
fn candidate(
    rng: &mut Lcg,
    id: BodyId,
    parent: Option<BodyId>,
    siblings: &[BodyFrameCandidate],
) -> BodyFrameCandidate {
    if let Some(twin) = siblings.last().filter(|_| rng.next_below(6) == 0) {
        return BodyFrameCandidate::new(id, parent, twin.distance(), twin.hill_radius())
            .expect("a twin of a valid candidate is valid");
    }
    let radius = hill(rng);
    BodyFrameCandidate::new(
        id,
        parent,
        Metres::new(ratio(rng) * radius),
        Metres::new(radius),
    )
    .expect("a drawn candidate is valid")
}

/// One case: its candidates and the current frame.
fn draw_case(rng: &mut Lcg, system: SystemId) -> (Vec<BodyFrameCandidate>, Option<BodyId>) {
    let star = BodyId::new(system, 0);
    let mut used = Vec::new();
    let mut candidates = Vec::new();
    let mut planets = Vec::new();
    for _ in 0..=rng.next_below(3) {
        let id = BodyId::new(system, next_index(rng, &mut used));
        let parent = if rng.next_below(2) == 0 {
            Some(star)
        } else {
            None
        };
        let planet = candidate(rng, id, parent, &planets);
        planets.push(planet);
        candidates.push(planet);
        let mut moons = Vec::new();
        for _ in 0..rng.next_below(3) {
            let moon_id = BodyId::new(system, next_index(rng, &mut used));
            let moon = candidate(rng, moon_id, Some(id), &moons);
            moons.push(moon);
            candidates.push(moon);
            if rng.next_below(8) == 0 {
                let sub_id = BodyId::new(system, next_index(rng, &mut used));
                candidates.push(candidate(rng, sub_id, Some(moon_id), &[]));
            }
        }
    }
    let current = match rng.next_below(8) {
        0 | 1 => None,
        2 => Some(BodyId::new(system, next_index(rng, &mut used))),
        _ => {
            let count = u64::try_from(candidates.len()).expect("a handful of candidates");
            let pick = usize::try_from(rng.next_below(count)).expect("a handful of candidates");
            Some(candidates[pick].id())
        }
    };
    (candidates, current)
}

fn id_text(id: Option<BodyId>) -> String {
    id.map_or_else(|| "-".to_owned(), |id| id.to_string())
}

#[test]
fn body_frame_answers_are_pinned() {
    let system = SystemId::from_raw(0x0200_0800_2000_0000).expect("a valid system ID");
    let mut rng = Lcg::new(0x0b0d_f7a3_e5e1_ec70);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("");
    for _ in 0..CASES {
        let (candidates, current) = draw_case(&mut rng, system);
        let answer = select_body_frame(&candidates, current);
        let listed: Vec<String> = candidates
            .iter()
            .map(|c| {
                format!(
                    "{}/{}/{:?}/{:?}",
                    c.id(),
                    id_text(c.parent()),
                    c.distance().value(),
                    c.hill_radius().value()
                )
            })
            .collect();
        w.line(&format!(
            "current={} answer={} candidates={}",
            id_text(current),
            id_text(answer),
            listed.join(";")
        ));
    }
    golden!("frame/body_frames", w.as_str());
}
