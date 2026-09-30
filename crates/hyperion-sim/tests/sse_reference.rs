//! The track integrator against the published SSE code (plan 06, P06.T12.b).
//!
//! `tests/data/sse/` holds P06.T12.a's reference vectors: for the 16 masses × 5 metallicities of
//! the plan, the first step of every stellar type SSE passes through (its age, mass, core mass, L
//! and R) and up to 20 samples inside the phases, run with steps a hundred times finer than SSE
//! ships with (ruling 40 of 2026-09-22; `README.md` there gives the provenance). Each star is
//! built here with [`TrackOptions::hurley2000`] (HPT's winds and remnants, no bridges), η = 0.5
//! (the median draw) and no pre-main sequence, and compared at the plan's tolerances:
//!
//! - the same phases in the same order, apart from a phase that lasts under 10⁻⁴ of the star's
//!   lifetime in one code and is missing in the other (an envelope lost within a few decades of a
//!   phase's end, where the two codes' steps fall either side of the knife edge);
//! - each phase's start within 1% in age, and its mass and core mass within 1% (2% on the
//!   thermally pulsing AGB, where SSE's own steps jitter the mass most);
//! - log L and log R within 0.02 dex at each sample, taken at the same fraction of the phase's
//!   time in both codes;
//! - the lifetime within 1%, the same kind of remnant, and its mass within 0.02 M☉.
//!
//! Ruling 29 declared two points SSE artefacts, 60 M☉ at Z = 10⁻⁴ and 10⁻³, where SSE at constant
//! mass runs a thermally pulsing AGB that HPT do not describe. With the wind, neither star reaches
//! the AGB in either code (the luminous-blue-variable wind strips both in the Hertzsprung gap), so
//! the artefact does not arise and nothing is exempted; a test below keeps that true.
//!
//! The paper checks of P06.T12.c are here too, where the public API reaches them: the
//! initial–final mass relation of HPT's Fig. 18 under HPT's recipes, and Cummings et al.'s (2018)
//! semi-empirical relation under the generator's (plan 06, design note 9). The third, the
//! main-sequence timescales of HPT's Fig. 5, reads a crate-private function and is a unit test of
//! `stellar::sse::evolve`.

use std::collections::BTreeMap;

use hyperion_sim::math;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::remnant::RemnantKind;
use hyperion_sim::stellar::sse::{Track, TrackOptions};
use hyperion_sim::stellar::{Composition, Phase};
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses, Years};

/// The metallicities of P06.T12.a, with their files' names.
const METALLICITIES: [(f64, &str); 5] = [
    (1e-4, "z0.0001.csv"),
    (1e-3, "z0.001.csv"),
    (4e-3, "z0.004.csv"),
    (0.02, "z0.02.csv"),
    (0.03, "z0.03.csv"),
];

/// A phase shorter than this share of the lifetime may be missing from the other code.
const SLIVER: f64 = 1e-4;

/// A group of SSE's stellar types, in the order a single star passes through them: 1 for the main
/// sequence (types 0 and 1), 2–7 as SSE numbers them, 8 for the helium Hertzsprung gap and giant
/// branch together (types 8 and 9, which differ only in which of two radii is smaller), and 10–15
/// for the remnants.
type Group = u8;

/// The group of one of SSE's stellar types.
fn group_of_kw(kw: u8) -> Group {
    match kw {
        0 | 1 => 1,
        9 => 8,
        other => other,
    }
}

/// The group of one of our phases.
fn group_of(phase: Phase) -> Group {
    match phase {
        Phase::MainSequence => 1,
        Phase::HertzsprungGap => 2,
        Phase::FirstGiantBranch => 3,
        Phase::CoreHeliumBurning => 4,
        Phase::EarlyAgb => 5,
        Phase::ThermallyPulsingAgb => 6,
        Phase::HeliumMainSequence => 7,
        Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch => 8,
        Phase::HeliumWhiteDwarf => 10,
        Phase::CarbonOxygenWhiteDwarf => 11,
        Phase::OxygenNeonWhiteDwarf => 12,
        Phase::NeutronStar => 13,
        Phase::BlackHole => 14,
        Phase::NoRemnant => 15,
        Phase::Protostar | Phase::PreMainSequence | Phase::PostAgb | Phase::Substellar => {
            panic!("no track under HPT's options passes through {phase:?}")
        }
    }
}

/// The remnant of SSE's remnant types.
fn remnant_of_kw(kw: u8) -> RemnantKind {
    match kw {
        10..=12 => RemnantKind::WhiteDwarf,
        13 => RemnantKind::NeutronStar,
        14 => RemnantKind::BlackHole,
        15 => RemnantKind::None,
        other => panic!("SSE type {other} is not a remnant"),
    }
}

/// One row of a reference file.
#[derive(Debug, Clone, Copy)]
struct Row {
    phase_start: bool,
    kw: u8,
    fraction: f64,
    age_years: f64,
    mass: f64,
    core_mass: f64,
    log_l: f64,
    log_r: f64,
}

/// The reference rows of one metallicity's file, by initial mass as the file writes it.
fn reference(file: &str) -> BTreeMap<String, (f64, Vec<Row>)> {
    let path = format!("{}/tests/data/sse/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut stars: BTreeMap<String, (f64, Vec<Row>)> = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with("kind,") || line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        assert_eq!(fields.len(), 9, "{file}: {line}");
        let number = |i: usize| -> f64 {
            fields[i]
                .parse()
                .unwrap_or_else(|e| panic!("{file}: {line}: {e}"))
        };
        let m0 = number(1);
        let row = Row {
            phase_start: match fields[0] {
                "phase" => true,
                "sample" => false,
                other => panic!("{file}: unknown kind {other}"),
            },
            kw: fields[2].parse().expect("a stellar type"),
            fraction: number(3),
            age_years: number(4) * 1e6,
            mass: number(5),
            core_mass: number(6),
            log_l: number(7),
            log_r: number(8),
        };
        stars
            .entry(fields[1].to_owned())
            .or_insert_with(|| (m0, Vec::new()))
            .1
            .push(row);
    }
    stars
}

fn composition(z: f64) -> Composition {
    Composition::from_fe_h(Dex::new(math::log10(z / 0.02)), HeliumExcess::ZERO)
}

/// The star of initial mass `m0` at metal fraction `z` under HPT's options, as SSE ran it.
fn track(m0: f64, z: f64) -> Track {
    Track::full_with(
        SolarMasses::new(m0),
        &composition(z),
        &StarDraws::median(),
        TrackOptions::hurley2000(),
    )
}

/// Our phases' time spans: the first age at which the star is in `group` or a later one, found by
/// bisection on the group of the phase at the age, which never falls with age.
struct Spans<'a> {
    track: &'a Track,
    lifetime: f64,
}

impl Spans<'_> {
    fn start(&self, group: Group) -> f64 {
        if group <= 1 {
            return 0.0;
        }
        let at = |age: f64| group_of(self.track.state_at(Years::new(age)).phase());
        let (mut lo, mut hi) = (0.0, self.lifetime);
        if at(hi) < group {
            return self.lifetime;
        }
        for _ in 0..200 {
            let mid = lo + 0.5 * (hi - lo);
            if mid <= lo || mid >= hi {
                break;
            }
            if at(mid) >= group {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }

    /// The living groups the track passes through, each with its start and end.
    fn living(&self) -> BTreeMap<Group, (f64, f64)> {
        let mut spans = BTreeMap::new();
        for group in 1..=8 {
            let (start, end) = (self.start(group), self.start(group + 1));
            if end > start {
                spans.insert(group, (start, end));
            }
        }
        spans
    }
}

/// SSE's living groups with their starts and ends, from the phase rows.
fn sse_living(rows: &[Row]) -> BTreeMap<Group, (f64, f64)> {
    let starts: Vec<(Group, f64)> = rows
        .iter()
        .filter(|r| r.phase_start)
        .map(|r| (group_of_kw(r.kw), r.age_years))
        .collect();
    let mut spans = BTreeMap::new();
    for pair in starts.windows(2) {
        let ((group, start), (_, end)) = (pair[0], pair[1]);
        if group < 10 {
            let span = spans.entry(group).or_insert((start, end));
            span.1 = end;
        }
    }
    spans
}

/// Every comparison of one star, as the failures it finds.
fn compare(m0: f64, z: f64, rows: &[Row]) -> Vec<String> {
    let what = format!("{m0} M☉ at Z = {z}");
    let mut failures = Vec::new();
    let track = track(m0, z);
    let death = track.death().expect("a full track dies");
    let lifetime = death.age().value();
    let spans = Spans {
        track: &track,
        lifetime,
    };
    let ours = spans.living();
    let theirs = sse_living(rows);
    let sse_death = rows
        .iter()
        .find(|r| r.phase_start && r.kw >= 10)
        .expect("SSE's star dies");
    let sliver = |span: (f64, f64)| span.1 - span.0 < SLIVER * lifetime;
    for (group, span) in &ours {
        if !theirs.contains_key(group) && !sliver(*span) {
            failures.push(format!(
                "{what}: our group {group} lasts {span:?}, SSE's is missing"
            ));
        }
    }
    for (group, span) in &theirs {
        if !ours.contains_key(group) && !sliver(*span) {
            failures.push(format!(
                "{what}: SSE's group {group} lasts {span:?}, ours is missing"
            ));
        }
    }
    // Phase starts: age, mass and core mass.
    for row in rows.iter().filter(|r| r.phase_start && r.kw < 10) {
        let group = group_of_kw(row.kw);
        let (Some(&span), Some(&sse_span)) = (ours.get(&group), theirs.get(&group)) else {
            continue;
        };
        if sliver(span) || sliver(sse_span) || sse_span.0 < row.age_years {
            continue;
        }
        let at = what.clone() + &format!(", start of group {group}");
        if row.age_years > 0.0 && (span.0 / row.age_years - 1.0).abs() > 0.01 {
            failures.push(format!("{at}: age {} against {}", span.0, row.age_years));
        }
        let tolerance = if group == 6 { 0.02 } else { 0.01 };
        let state = track.state_at(Years::new(span.0));
        let mass = state.mass().value();
        if (mass / row.mass - 1.0).abs() > tolerance {
            failures.push(format!("{at}: mass {mass} against {}", row.mass));
        }
        let core = state.core_mass().value();
        let core_off = if row.core_mass > 0.0 {
            (core / row.core_mass - 1.0).abs() > tolerance
        } else {
            core > 1e-9
        };
        if core_off {
            failures.push(format!("{at}: core {core} against {}", row.core_mass));
        }
    }
    compare_samples(&what, &track, rows, &ours, &theirs, &mut failures);
    compare_death(&what, &track, sse_death, &mut failures);
    failures
}

/// The samples' L and R at the same fraction of the phase, where both codes have the phase.
fn compare_samples(
    what: &str,
    track: &Track,
    rows: &[Row],
    ours: &BTreeMap<Group, (f64, f64)>,
    theirs: &BTreeMap<Group, (f64, f64)>,
    failures: &mut Vec<String>,
) {
    let lifetime = track.lifetime().expect("a full track dies").value();
    let sliver = |span: (f64, f64)| span.1 - span.0 < SLIVER * lifetime;
    for row in rows.iter().filter(|r| !r.phase_start) {
        let group = group_of_kw(row.kw);
        let (Some(&span), Some(&sse_span)) = (ours.get(&group), theirs.get(&group)) else {
            failures.push(format!(
                "{what}: a sample in group {group}, which one code lacks"
            ));
            continue;
        };
        if sliver(span) || sliver(sse_span) {
            continue;
        }
        let age = span.0 + row.fraction * (span.1 - span.0);
        let state = track.state_at(Years::new(age));
        let log_l = math::log10(state.luminosity().value());
        let log_r = math::log10(state.radius().value());
        let at = format!("{what}, group {group} at {:.3} of the phase", row.fraction);
        if (log_l - row.log_l).abs() > 0.02 {
            failures.push(format!("{at}: log L {log_l} against {}", row.log_l));
        }
        if (log_r - row.log_r).abs() > 0.02 {
            failures.push(format!("{at}: log R {log_r} against {}", row.log_r));
        }
    }
}

/// The lifetime, and the remnant's kind and mass.
fn compare_death(what: &str, track: &Track, sse_death: &Row, failures: &mut Vec<String>) {
    let lifetime = track.lifetime().expect("a full track dies").value();
    if (lifetime / sse_death.age_years - 1.0).abs() > 0.01 {
        failures.push(format!(
            "{what}: lifetime {lifetime} against {}",
            sse_death.age_years
        ));
    }
    let remnant = track.remnant().expect("a full track leaves its remnant");
    if remnant.kind() != remnant_of_kw(sse_death.kw) {
        failures.push(format!(
            "{what}: remnant {:?} against SSE's type {}",
            remnant.kind(),
            sse_death.kw
        ));
    }
    if (remnant.mass().value() - sse_death.mass).abs() > 0.02 {
        failures.push(format!(
            "{what}: remnant mass {} against {}",
            remnant.mass().value(),
            sse_death.mass
        ));
    }
}

/// Every star of one metallicity against SSE.
fn check(z: f64, file: &str) {
    let stars = reference(file);
    assert_eq!(stars.len(), 16, "{file} holds the 16 masses of P06.T12.a");
    let failures: Vec<String> = stars
        .values()
        .flat_map(|(m0, rows)| compare(*m0, z, rows))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn tracks_match_sse_at_z_0_0001() {
    check(METALLICITIES[0].0, METALLICITIES[0].1);
}

#[test]
fn tracks_match_sse_at_z_0_001() {
    check(METALLICITIES[1].0, METALLICITIES[1].1);
}

#[test]
fn tracks_match_sse_at_z_0_004() {
    check(METALLICITIES[2].0, METALLICITIES[2].1);
}

#[test]
fn tracks_match_sse_at_z_0_02() {
    check(METALLICITIES[3].0, METALLICITIES[3].1);
}

#[test]
fn tracks_match_sse_at_z_0_03() {
    check(METALLICITIES[4].0, METALLICITIES[4].1);
}

/// The reference files cover what the comparison needs: every star dies, and has samples.
#[test]
fn the_reference_vectors_are_complete() {
    for (_, file) in METALLICITIES {
        for (m0, rows) in reference(file).values() {
            assert!(
                rows.iter().any(|r| r.phase_start && r.kw >= 10),
                "{file}: {m0} M☉ has no death"
            );
            let samples = rows.iter().filter(|r| !r.phase_start).count();
            assert!(
                (10..=20).contains(&samples),
                "{file}: {m0} M☉ has {samples} samples"
            );
        }
    }
}

/// Ruling 29's two SSE artefacts need no exemption: with the wind, neither star reaches the AGB in
/// SSE or here, so SSE's pulses at constant mass never come into it.
#[test]
fn ruling_29s_artefact_does_not_arise_with_the_wind() {
    for (z, file) in [METALLICITIES[0], METALLICITIES[1]] {
        let stars = reference(file);
        let (_, rows) = &stars["60"];
        assert!(
            rows.iter().all(|r| r.kw != 5 && r.kw != 6),
            "SSE's 60 M☉ star at Z = {z} reaches the AGB"
        );
        let track = track(60.0, z);
        let spans = Spans {
            lifetime: track.lifetime().expect("a full track dies").value(),
            track: &track,
        };
        let ours = spans.living();
        assert!(
            !ours.contains_key(&5) && !ours.contains_key(&6),
            "our 60 M☉ star at Z = {z} reaches the AGB: {ours:?}"
        );
    }
}

/// P06.T12.c: the white dwarf masses of HPT's formulae, read from their Fig. 18 (journal page
/// 562), against the tracks under HPT's recipes, to the plan's 0.05 M☉. The figure's two solid
/// lines, at Z = 0.02 and 0.004, were traced on the journal's 799-dpi bitmap, with the axes
/// calibrated on their major ticks, to about ±0.003 M☉. The tracks agree with both to 0.0025 M☉.
#[test]
fn the_initial_final_mass_relation_matches_hpt_figure_18() {
    const Z_SOLAR: [(f64, f64); 17] = [
        (1.25, 0.546),
        (1.5, 0.576),
        (1.75, 0.607),
        (2.0, 0.636),
        (2.25, 0.645),
        (2.5, 0.689),
        (2.75, 0.719),
        (3.0, 0.748),
        (3.5, 0.809),
        (4.0, 0.867),
        (4.5, 0.931),
        (5.0, 0.999),
        (5.5, 1.068),
        (6.0, 1.140),
        (6.5, 1.213),
        (7.0, 1.288),
        (7.5, 1.362),
    ];
    const Z_LOW: [(f64, f64); 16] = [
        (1.25, 0.592),
        (1.5, 0.626),
        (1.75, 0.664),
        (2.0, 0.701),
        (2.25, 0.728),
        (2.5, 0.775),
        (2.75, 0.816),
        (3.0, 0.858),
        (3.5, 0.890),
        (4.0, 0.947),
        (4.5, 1.019),
        (5.0, 1.094),
        (5.5, 1.171),
        (6.0, 1.249),
        (6.5, 1.328),
        (7.0, 1.409),
    ];
    for (z, relation) in [(0.02, &Z_SOLAR[..]), (0.004, &Z_LOW[..])] {
        for &(m0, figure) in relation {
            let remnant = track(m0, z)
                .remnant()
                .expect("a full track leaves its remnant");
            assert_eq!(
                remnant.kind(),
                RemnantKind::WhiteDwarf,
                "{m0} M☉ at Z = {z}"
            );
            let mass = remnant.mass().value();
            assert!(
                (mass - figure).abs() < 0.05,
                "{m0} M☉ at Z = {z}: a white dwarf of {mass} M☉ against HPT's {figure}"
            );
        }
    }
}

/// The semi-empirical initial–final mass relation of Cummings et al. (2018, ApJ 866, 21, eqs.
/// 4–6, their adopted fit on MIST ages), M☉: 0.080 m + 0.489 up to 2.85 M☉, 0.187 m + 0.184 up
/// to 3.60, and 0.107 m + 0.471 up to 7.20. Its clusters are near solar ([Fe/H] −0.15 to +0.15),
/// and its scatter is 0.06 M☉.
fn cummings_2018(m0: f64) -> f64 {
    if m0 < 2.85 {
        0.080 * m0 + 0.489
    } else if m0 < 3.60 {
        0.187 * m0 + 0.184
    } else {
        0.107 * m0 + 0.471
    }
}

/// P06.T12.c and design note 9: under the generator's recipes, a star of solar metallicity ([Fe/H]
/// = 0, Z = 0.02 in HPT's scale) leaves the white dwarf of Cummings et al.'s relation to 0.08 M☉
/// over its range, 0.85–7.2 M☉. The track's white dwarf is the core it ends with, so a failure is
/// a finding against the AGB wind, not a reason to bolt on a second relation. The worst is
/// +0.069 M☉ at 7.2 M☉, and −0.05 M☉ near 1 M☉.
#[test]
fn modern_white_dwarf_masses_follow_cummings_2018() {
    let solar = composition(0.02);
    for i in 0..=127_u32 {
        let m0 = 0.85 + 0.05 * f64::from(i);
        if m0 > 7.2 + 1e-9 {
            break;
        }
        let remnant = Track::full(SolarMasses::new(m0), &solar, &StarDraws::median())
            .remnant()
            .expect("a full track leaves its remnant");
        assert_eq!(remnant.kind(), RemnantKind::WhiteDwarf, "{m0} M☉");
        let (mass, relation) = (remnant.mass().value(), cummings_2018(m0));
        assert!(
            (mass - relation).abs() < 0.08,
            "{m0} M☉: a white dwarf of {mass} M☉ against Cummings et al.'s {relation}"
        );
    }
}

/// Ruling 92: under the generator's recipes, the white dwarf of every initial mass from 0.7 to
/// 3 M☉ is lighter at higher metallicity, at every [Fe/H] of a 0.125 dex grid from −2.2 to +0.3
/// (Z is held at 0.03 above +0.176, so the last points are equal), and it falls by 0.03–0.10 M☉
/// per dex at 1.5–2 M☉ between −2.2 and +0.176. Detailed models fall by 0.04–0.08 M☉ per dex at
/// 1.5 M☉ (Meng, Chen and Han 2008, A&A 487, 625, App. A) and about 0.05 at 1.5 and 2 M☉
/// (Romero, Campos and Kepler 2015, MNRAS 450, 3708, Table 1). With HPT's printed giant radii
/// between their calibration metallicities the mass swung ±0.08 M☉ about that trend, reversing
/// three times: at 1.5 M☉, 0.664 M☉ at [Fe/H] −1.425 rose to 0.673 at −1.3 and 0.566 at −0.925 to
/// 0.677 at −0.55. Measured: −0.080 M☉ per dex at 1.5 M☉, −0.090 at 1.75 and −0.099 at 2.
#[test]
#[ignore = "slow: about 380 full tracks"]
fn white_dwarf_masses_fall_with_metallicity() {
    const MASSES: [f64; 18] = [
        0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.75, 1.9, 2.0, 2.1, 2.25, 2.5, 2.75, 3.0,
    ];
    let white_dwarf = |m0: f64, fe_h: f64| {
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let remnant = Track::full(SolarMasses::new(m0), &composition, &StarDraws::median())
            .remnant()
            .expect("a full track leaves its remnant");
        assert_eq!(
            remnant.kind(),
            RemnantKind::WhiteDwarf,
            "{m0} M☉ at [Fe/H] {fe_h}"
        );
        remnant.mass().value()
    };
    let top = math::log10(0.03 / 0.02);
    for m0 in MASSES {
        let masses: Vec<(f64, f64)> = (0..=20)
            .map(|k| {
                let fe_h = -2.2 + 0.125 * f64::from(k);
                (fe_h, white_dwarf(m0, fe_h))
            })
            .collect();
        for pair in masses.windows(2) {
            let [(_, before), (fe_h, after)] = [pair[0], pair[1]];
            assert!(
                after <= before + 1e-9,
                "{m0} M☉: the white dwarf rises from {before} to {after} M☉ at [Fe/H] {fe_h}"
            );
        }
        if (1.5..=2.0).contains(&m0) {
            let slope = (white_dwarf(m0, top) - masses[0].1) / (top + 2.2);
            assert!(
                (-0.10..=-0.03).contains(&slope),
                "{m0} M☉: {slope} M☉ per dex"
            );
        }
    }
}
