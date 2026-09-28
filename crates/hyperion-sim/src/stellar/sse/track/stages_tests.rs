//! Tests of the stages plan 06's P06.T14–T17 add to the track: very massive stars, the protostar
//! and the contraction, the post-AGB bridge, and the helium-excess hook.

use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

use super::super::continuity::find_jump;
use super::*;
use crate::math;
use crate::stellar::premain::{PROTOSTAR_YEARS, t_zams};
use crate::units::consts::{
    GRAVITATIONAL_CONSTANT, SOLAR_LUMINOSITY_W, SOLAR_MASS_KG, SPEED_OF_LIGHT,
};
use crate::units::{Dex, HeliumExcess};

fn composition(z: f64) -> Composition {
    Composition::from_fe_h(Dex::new(math::log10(z / 0.02)), HeliumExcess::ZERO)
}

fn full(m: f64, z: f64) -> Track {
    Track::full(SolarMasses::new(m), &composition(z), &StarDraws::median())
}

/// log₁₀ L and log₁₀ R of `track` at `age`.
fn logs(track: &Track, age: f64) -> [f64; 2] {
    let s = track.state_at(Years::new(age));
    [
        math::log10(s.luminosity().value()),
        math::log10(s.radius().value()),
    ]
}

/// The one-sided slopes in age of log₁₀ L and log₁₀ R just before and just after `age`.
fn slopes(track: &Track, age: f64) -> [[f64; 2]; 2] {
    // Small against the blend's curvature, which is large where the contraction is short.
    let h = 1e-7 * age;
    let (before, at, after) = (logs(track, age - h), logs(track, age), logs(track, age + h));
    [
        [(at[0] - before[0]) / h, (at[1] - before[1]) / h],
        [(after[0] - at[0]) / h, (after[1] - at[1]) / h],
    ]
}

/// Whether two slopes agree to 5% of the larger, or both are negligible against `scale` (per
/// year).
fn slopes_agree(a: f64, b: f64, scale: f64) -> bool {
    (a - b).abs() <= 0.05 * a.abs().max(b.abs()) || (a.abs().max(b.abs()) < 1e-3 * scale)
}

// -------------------------------------------------------------------------------------------------
// P06.T15: the protostar and the contraction.

/// The protostar meets its contraction at `t_p` and the contraction its main sequence at the
/// arrival, in L and R to 10⁻⁶ and in their slopes to 5% (P06.T15's test). At `t_p` the photosphere's
/// radius meets the contraction's slope; the luminosity's does not, since the accretion that
/// stops there leaves its own slope, so L's slope is checked at the arrival only.
#[test]
fn the_stages_before_the_main_sequence_meet_in_value_and_slope() {
    for z in [0.02, 0.001] {
        for m in [0.1, 0.3, 0.5, 0.8, 1.0, 1.4, 2.0, 3.0, 5.0] {
            let track = full(m, z);
            let arrival = track.main_sequence_start().expect("a main sequence");
            assert!(
                arrival > PROTOSTAR_YEARS,
                "{m} M☉ at Z = {z} arrives after t_p"
            );
            for (at, what) in [(PROTOSTAR_YEARS, "t_p"), (arrival, "the arrival")] {
                let before = logs(&track, at * (1.0 - 1e-12));
                let after = logs(&track, at);
                for k in 0..2 {
                    assert!(
                        (after[k] - before[k]).abs() < 1e-6 / core::f64::consts::LN_10,
                        "{m} M☉ at Z = {z}: a step of {} dex at {what}",
                        after[k] - before[k]
                    );
                }
                let [left, right] = slopes(&track, at);
                let scale = 1.0 / at;
                assert!(
                    slopes_agree(left[1], right[1], scale),
                    "{m} M☉ at Z = {z}: R's slope at {what}, {} against {}",
                    left[1],
                    right[1]
                );
                if at > PROTOSTAR_YEARS {
                    assert!(
                        slopes_agree(left[0], right[0], scale),
                        "{m} M☉ at Z = {z}: L's slope at {what}, {} against {}",
                        left[0],
                        right[0]
                    );
                }
            }
        }
    }
}

/// Below 0.5 M☉ the contraction's last stretch is a continuity device, not a Henyey track
/// (ruling 124.7). At solar metallicity `T_eff` stays within 200 K and 7% of the Hayashi track's (the
/// birthline's) temperature through the whole contraction (the research measured 2,935 → 2,768 K
/// at 0.1 M☉, 3,763 → 3,704 K at 0.45). At lower Z the zero-age main sequence it ends on is hotter
/// than the solar birthline (by up to 300 K at 0.15 M☉ and Z = 0.004), so there the blend is held to
/// within 200 K of the range between its two ends: it never overshoots either.
#[test]
fn the_blend_below_half_a_solar_mass_stays_near_the_hayashi_temperature() {
    for z in [0.02, 0.004, 0.001] {
        for m in [0.1, 0.15, 0.2, 0.3, 0.4, 0.45] {
            let track = full(m, z);
            let hayashi = math::exp10(crate::stellar::premain::Protostar::birthline_start(m)[1]);
            let arrival = track.main_sequence_start().expect("a main sequence");
            let zams = track
                .state_at(Years::new(arrival))
                .effective_temperature()
                .value();
            let (lo, hi) = if (z - 0.02_f64).abs() < 1e-12 {
                (hayashi, hayashi)
            } else {
                (hayashi.min(zams), hayashi.max(zams))
            };
            for i in 0..=2_000 {
                let x = f64::from(i) / 2_000.0;
                let age = PROTOSTAR_YEARS * math::powf(arrival / PROTOSTAR_YEARS, x);
                let t = track
                    .state_at(Years::new(age))
                    .effective_temperature()
                    .value();
                let off = (lo - t).max(t - hi).max(0.0);
                assert!(
                    off < 200.0 && off < 0.07 * hayashi,
                    "{m} M☉ at Z = {z}, {age} yr: {t} K against {lo}–{hi}"
                );
            }
        }
    }
}

/// A heavy star is on its main sequence when accretion ends: its protostar hands over at `t_p`, at
/// the fractional age it has reached, continuously.
#[test]
fn a_heavy_star_accretes_onto_its_main_sequence() {
    for m in [8.0, 20.0, 60.0, 150.0] {
        let track = full(m, 0.02);
        assert_eq!(track.main_sequence_start(), Some(PROTOSTAR_YEARS), "{m} M☉");
        let arrival = t_zams(SolarMasses::new(m), &composition(0.02)).value() * 1e6;
        assert!(arrival < PROTOSTAR_YEARS, "{m} M☉ arrives at {arrival} yr");
        let ms = &track.segments()[1];
        assert!(matches!(ms.model, model::Model::MainSequence { .. }));
        assert!(matches!(ms.coordinate, Coordinate::Fraction { tau0 } if tau0 > 0.0));
        let before = logs(&track, PROTOSTAR_YEARS * (1.0 - 1e-12));
        let after = logs(&track, PROTOSTAR_YEARS);
        assert!(
            (after[0] - before[0]).abs() < 1e-6 && (after[1] - before[1]).abs() < 1e-6,
            "{m} M☉: {before:?} to {after:?}"
        );
    }
}

/// The plan's check on the contraction: a 1 M☉ star at 2 Myr has 1–3 L☉ and 3,900–4,500 K
/// (Baraffe et al. 2015 give 1.19 L☉ and 4,350 K).
#[test]
fn a_solar_mass_star_at_two_megayears_is_a_t_tauri_star() {
    let track = Track::to_age(
        SolarMasses::new(1.0),
        &Composition::SOLAR,
        &StarDraws::median(),
        Years::new(2e6),
    );
    let s = track.state_at(Years::new(2e6));
    assert_eq!(s.phase(), Phase::PreMainSequence);
    assert!((1.0..3.0).contains(&s.luminosity().value()), "{s:?}");
    assert!(
        (3_900.0..4_500.0).contains(&s.effective_temperature().value()),
        "{s:?}"
    );
}

/// Most stars below 0.5 M☉ in the young disc (born in the last 100 Myr) are still contracting,
/// as the brainstorm states: their arrivals are 140–550 Myr (Baraffe et al. 2015).
#[test]
fn most_young_stars_below_half_a_solar_mass_are_before_the_main_sequence() {
    let mut rng = Lcg::new(0x796f_756e);
    let n = 400;
    let mut young = 0;
    for _ in 0..n {
        // Kroupa's m^−1.3 over 0.1–0.5 M☉, by its inverse cumulative distribution.
        let u = rng.next_f64();
        let (lo, hi) = (math::powf(0.1, -0.3), math::powf(0.5, -0.3));
        let m = math::powf(lo + u * (hi - lo), -1.0 / 0.3);
        let age = 1e8 * rng.next_f64();
        let s = Track::to_age(
            SolarMasses::new(m),
            &Composition::SOLAR,
            &StarDraws::median(),
            Years::new(age),
        )
        .state_at(Years::new(age));
        young += u32::from(matches!(
            s.phase(),
            Phase::Protostar | Phase::PreMainSequence
        ));
    }
    assert!(young * 2 > n, "{young} of {n} before the main sequence");
}

/// The arrival follows Baraffe et al. (2015) to 1.4 M☉ and the Kelvin–Helmholtz time above:
/// 38.4 Myr at 1 M☉, 140 at 0.5, 230 at 0.2 (within the fit's 0.037 dex, and 10%), falling below
/// `t_p` near 6 M☉; and a star's lifetime counts it.
#[test]
fn the_arrival_follows_baraffe_and_the_kelvin_helmholtz_time() {
    let arrival = |m: f64| t_zams(SolarMasses::new(m), &Composition::SOLAR).value();
    for (m, bhac15) in [(0.2, 229.8), (0.5, 139.5), (1.0, 38.39), (1.4, 17.42)] {
        let off = math::log10(arrival(m) / bhac15).abs();
        assert!(off < 0.04, "{m} M☉: {} Myr against {bhac15}", arrival(m));
    }
    assert!(arrival(5.0) > 0.5 && arrival(7.0) < 0.5);
    let mut last = f64::INFINITY;
    // Up to 100 M☉: above it the corrected zero-age radius (P06.T14) shrinks faster than the
    // luminosity grows, and the arrival, some 0.02 Myr, well inside t_p, rises again slightly.
    for i in 0..=300 {
        let m = math::exp10(-1.0 + 3.0 * f64::from(i) / 300.0);
        let t = arrival(m);
        assert!(t < last, "the arrival falls with mass: {m} M☉ at {t} Myr");
        last = t;
    }
    let sun = full(1.0, 0.02);
    let start = sun.main_sequence_start().expect("a main sequence");
    assert!((start * 1e-6 / arrival(1.0) - 1.0).abs() < 1e-12);
    assert!(sun.lifetime().expect("dies").value() > start + 1e10);
}

// -------------------------------------------------------------------------------------------------
// P06.T14: very massive stars.

/// Yusof et al.'s (2013) non-rotating models at 120 and 150 M☉ (Tables 2 and 3): log L, log `T_eff`
/// on the zero-age main sequence and the core hydrogen-burning lifetime, Myr, at Z = 0.014 and
/// 0.006.
const GENEVA: [(f64, f64, f64, f64, f64); 4] = [
    (0.014, 120.0, 6.231, 4.729, 2.671),
    (0.014, 150.0, 6.383, 4.736, 2.497),
    (0.006, 120.0, 6.227, 4.750, 2.675),
    (0.006, 150.0, 6.379, 4.758, 2.492),
];

/// At 120 and 150 M☉ the corrected zero-age L and R and main-sequence lifetime are within 10% of
/// the grid's, and every lifetime is above 2 Myr (P06.T14's test). The tracks' own main sequences,
/// on which the generator's winds lower the mass and so lengthen the effective-age clock, last
/// 1–16% longer than `τ_H` (a finding, recorded in the plan), which this test bounds at 25%.
#[test]
fn very_massive_stars_meet_the_geneva_grid() {
    for (z, m, log_l, log_t, tau_h) in GENEVA {
        let comp = composition(z);
        let c = ZCoeffs::new(comp.z_fit());
        let ms = super::super::ms::MainSequence::new(SolarMasses::new(m), &c);
        let zams = ms.at(crate::units::Megayears::ZERO);
        let grid_r = math::exp10(0.5 * log_l - 2.0 * (log_t - math::log10(5_772.0)));
        let l_off = zams.luminosity.value() / math::exp10(log_l) - 1.0;
        let r_off = zams.radius.value() / grid_r - 1.0;
        let t_off = ms.t_ms().value() / tau_h - 1.0;
        assert!(
            l_off.abs() < 0.1 && r_off.abs() < 0.1 && t_off.abs() < 0.1,
            "{m} M☉ at Z = {z}: L {l_off:+.3}, R {r_off:+.3}, t_MS {t_off:+.3}"
        );
        let track = full(m, z);
        let segment = track
            .segments()
            .iter()
            .find(|s| matches!(s.model, model::Model::MainSequence { .. }))
            .expect("a main sequence");
        // The main sequence's own length, from the arrival: t_p less the arrival lies before it.
        let arrival = t_zams(SolarMasses::new(m), &comp).value() * 1e6;
        let track_off = (segment.end - arrival) / (tau_h * 1e6) - 1.0;
        assert!(
            track_off.abs() < 0.25,
            "{m} M☉ at Z = {z}: the track's main sequence {track_off:+.3}"
        );
        assert!(track.lifetime().expect("dies").value() > 2e6);
    }
}

/// The electron-scattering Eddington factor `Γ_e` = `κ_e` L ÷ (4π c G M) at X = 0, `κ_e` = 0.02 m² kg⁻¹,
/// the least opacity a photosphere can have (ruling 124.1).
fn eddington_x0(s: &StarState) -> f64 {
    0.02 * s.luminosity().value() * SOLAR_LUMINOSITY_W
        / (4.0
            * core::f64::consts::PI
            * SPEED_OF_LIGHT
            * GRAVITATIONAL_CONSTANT
            * s.mass().value()
            * SOLAR_MASS_KG)
}

/// P06.T14's Eddington check, as ruling 124.1 restates it: on the main sequences of 100–150 M☉
/// tracks `Γ_e` at X = 0 stays below 0.75 at every metallicity (measured at most 0.65, under Sanyal
/// et al.'s 2015 0.7; Yusof et al.'s end-of-hydrogen Γ is 0.63–0.72). At the initial X it reaches
/// 1.14 at 150 M☉ and Z = 10⁻⁴, a finding: the photosphere there has to be helium-enriched, which
/// the tracks do not model, and which P06.T24.a's `WNh` class should read by `Γ_e` (Gräfener et al.
/// 2011).
#[test]
fn very_massive_main_sequences_stay_below_the_eddington_limit() {
    for z in [1e-4, 0.001, 0.006, 0.014, 0.03] {
        for m in [100.0, 110.0, 120.0, 135.0, 150.0] {
            let track = full(m, z);
            let ms = track
                .segments()
                .iter()
                .find(|s| matches!(s.model, model::Model::MainSequence { .. }))
                .expect("a main sequence");
            for i in 0..=1_000 {
                let age = ms.start + (ms.end - ms.start) * f64::from(i) / 1_000.0;
                let s = track.state_at(Years::new(age.min(ms.end * (1.0 - 1e-15))));
                let g = eddington_x0(&s);
                assert!(g < 0.75, "{m} M☉ at Z = {z}: Γ_e(X = 0) = {g} in {s:?}");
            }
        }
    }
}

/// **A finding, flagged and pinned, not clamped (ruling 124.1):** after the main sequence HPT's
/// formulae pass `Γ_e(X` = 0) = 1, as the published SSE code does (1.01–1.04), at the instant a
/// massive star is stripped (the envelope under 1% of the star, the giant's luminosity still on)
/// and in metal-poor 150 M☉ red supergiants (log L 6.9–7.7). The research's probe measured, for
/// the largest `Γ_e(X` = 0) after the main sequence and the time spent above 1:
///
/// | Track | max `Γ_e(X` = 0) | Time above 1 |
/// | --- | --- | --- |
/// | 150 M☉, Z = 10⁻⁴ | 6.73 | 88 kyr |
/// | 150 M☉, Z = 0.001 | 8.04 | 50 kyr |
/// | 120 M☉, Z = 0.006 | 1.03 | 11 kyr |
/// | 80 M☉, Z = 10⁻⁴ | 1.19 | 46 kyr |
/// | 150 M☉, Z = 0.014; 80 M☉, Z = 0.03 | 0.89; 0.60 | none |
///
/// The test holds each maximum within 20% and each time within a factor of two, so a change that
/// moves them is seen. P06.T39's wind, which grows with `Γ_e` above 0.7, is to bring every living
/// state under 1 within 10³ years.
#[test]
fn post_main_sequence_eddington_excursions_are_pinned() {
    for (m, z, max, above) in [
        (150.0, 1e-4, 6.73, 88e3),
        (150.0, 0.001, 8.04, 50e3),
        (120.0, 0.006, 1.03, 11e3),
        (80.0, 1e-4, 1.19, 46e3),
        (150.0, 0.014, 0.89, 0.0),
        (80.0, 0.03, 0.60, 0.0),
    ] {
        let track = full(m, z);
        let ms_end = track
            .segments()
            .iter()
            .find(|s| matches!(s.model, model::Model::MainSequence { .. }))
            .expect("a main sequence")
            .end;
        let death = track.lifetime().expect("dies").value();
        let n = 40_000;
        let step = (death - ms_end) / f64::from(n);
        let (mut largest, mut time) = (0.0_f64, 0.0);
        for i in 0..n {
            let s = track.state_at(Years::new(ms_end + step * (f64::from(i) + 0.5)));
            if !s.phase().is_living() {
                continue;
            }
            let g = eddington_x0(&s);
            largest = largest.max(g);
            if g > 1.0 {
                time += step;
            }
        }
        assert!(
            (largest / max - 1.0).abs() < 0.2,
            "{m} M☉ at Z = {z}: max Γ_e(X = 0) {largest} against {max}"
        );
        if above > 0.0 {
            assert!(
                (0.5..2.0).contains(&(time / above)),
                "{m} M☉ at Z = {z}: {time} yr above 1 against {above}"
            );
        } else {
            assert!(time.abs() < 1e-9, "{m} M☉ at Z = {z}: {time} yr above 1");
        }
    }
}

// -------------------------------------------------------------------------------------------------
// P06.T16.a: the post-AGB bridge.

/// The end of the AGB is continuous under the generator's options: through the loss of the
/// envelope, the crossing and the hand-over to the white dwarf, L and R have no jump of more than
/// 1%, which removes T10.d's declared step (P06.T16.a's test). The crossing holds its luminosity
/// and ends at the white dwarf's radius.
#[test]
fn the_end_of_the_agb_is_continuous_through_the_bridge() {
    const TOLERANCE: f64 = 0.004_3;
    let mut crossed = 0;
    for z in [0.02, 0.004, 0.0003] {
        for m in [0.9, 1.0, 1.5, 2.0, 3.0, 5.0, 7.0] {
            let track = full(m, z);
            // A heavy star at low Z may end otherwise (an electron-capture collapse, or a naked
            // helium giant's white dwarf).
            let Some(start) = track.post_agb_start() else {
                continue;
            };
            crossed += 1;
            let death = track.lifetime().expect("dies").value();
            let crossing = death - start.value();
            assert!(crossing > 0.0);
            let (a, b) = (start.value() - 10.0 * crossing, death + 10.0 * crossing);
            for (k, name) in [(0, "log L"), (1, "log R")] {
                let f = |age: f64| logs(&track, age)[k];
                if let Some((lo, hi)) = find_jump(f, a, b, TOLERANCE) {
                    panic!(
                        "{m} M☉ at Z = {z}: {name} jumps by {} between {lo} and {hi}",
                        f(hi) - f(lo)
                    );
                }
            }
            let [l0, _] = logs(&track, start.value());
            let [l1, r1] = logs(&track, death - 1e-9 * crossing);
            let after = track.state_at(Years::new(death));
            assert!(after.phase().is_remnant());
            let core = after.mass().value();
            let fade = super::post_agb::knee_fade_dex(core);
            assert!(
                (l0 - l1 - fade).abs() < 1e-6,
                "{m} M☉: the crossing fades by {} dex, not {fade}",
                l0 - l1
            );
            assert!(
                (r1 - math::log10(after.radius().value())).abs() < 1e-6,
                "{m} M☉: the knee is the inflated white dwarf's radius"
            );
            assert_eq!(
                track
                    .state_at(Years::new(f64::midpoint(start.value(), death)))
                    .phase(),
                Phase::PostAgb
            );
        }
    }
    assert!(crossed >= 18, "only {crossed} post-AGB crossings");
}

/// Ruling 124.2's checks on the knee and the young white dwarf, over the tracks' own AGB white
/// dwarfs: none of `M_f` ≤ 0.85 M☉ is ever above 10^5.65 K; and the fade from the knee to log L = 2
/// of one of 0.58–0.83 M☉.
///
/// Since ruling 127.1 the dwarf fades on MB16's median shape to 10 L☉ before the Montreal law
/// takes over, and the window is 0.3–30 kyr (the fit gives 3–9 kyr; MB16 0.5–50 kyr). Before, on
/// the law's bright end, it took 46–102 kyr.
#[test]
fn young_white_dwarfs_are_no_hotter_and_fade_as_miller_bertolami_s() {
    let mut faded = 0;
    for z in [0.02, 0.004, 0.0003] {
        for m in [1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0] {
            let track = full(m, z);
            let Some(start) = track.post_agb_start() else {
                continue;
            };
            let death = track.lifetime().expect("dies").value();
            let core = track.remnant().expect("a remnant").mass().value();
            let hottest = (0..=4_000)
                .map(|k| {
                    let age =
                        start.value() + (death - start.value() + 1e6) * f64::from(k) / 4_000.0;
                    track
                        .state_at(Years::new(age))
                        .effective_temperature()
                        .value()
                })
                .fold(0.0_f64, f64::max);
            if core <= 0.85 {
                assert!(
                    math::log10(hottest) < 5.65,
                    "{m} M☉ at Z = {z}: a {core} M☉ dwarf at {hottest} K"
                );
            }
            if (0.58..=0.83).contains(&core) {
                // The first age after the knee at which L falls below 100 L☉, by bisection.
                let (mut lo, mut hi) = (death, death + 1e6);
                assert!(track.state_at(Years::new(hi)).luminosity().value() < 100.0);
                for _ in 0..60 {
                    let mid = f64::midpoint(lo, hi);
                    if track.state_at(Years::new(mid)).luminosity().value() < 100.0 {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let years = hi - death;
                assert!(
                    (300.0..30_000.0).contains(&years),
                    "{m} M☉ at Z = {z}: a {core} M☉ dwarf fades to 100 L☉ in {years} yr"
                );
                faded += 1;
            }
        }
    }
    assert!(faded >= 6, "only {faded} dwarfs of 0.58–0.83 M☉");
}

/// Ruling 127.1: a bridged white dwarf reaches 10 L☉ at `t₁` after its knee, where the cooling law
/// takes over, with L and R continuous there to 10⁻⁶; a table-routed dwarf's remnant (its knee and
/// origin rebuilt from its mass and knee alone) is the track's to rounding through the join.
#[test]
fn the_fade_hands_over_to_the_cooling_law_at_ten_solar_luminosities() {
    let mut joined = 0;
    for z in [0.02, 0.004] {
        for m in [1.2, 2.0, 3.0, 5.0] {
            let track = full(m, z);
            if track.post_agb_start().is_none() {
                continue;
            }
            let death = track.lifetime().expect("dies").value();
            let remnant = track.remnant_model().expect("a remnant");
            let Some(knee) = remnant.knee else {
                panic!("{m} M☉ at Z = {z}: a bridged dwarf carries its knee");
            };
            let t1 = super::post_agb::fade_years(remnant.mass.value());
            let at = death + t1;
            let (before, after) = (logs(&track, at * (1.0 - 1e-15)), logs(&track, at));
            assert!(
                (after[0] - 1.0).abs() < 1e-6,
                "{m} M☉: log L {} at t₁",
                after[0]
            );
            for k in 0..2 {
                assert!(
                    (after[k] - before[k]).abs() < 1e-6,
                    "{m} M☉ at Z = {z}: a step of {} at t₁",
                    after[k] - before[k]
                );
            }
            let rebuilt = RemnantModel {
                origin: bridged_origin(
                    RemnantRecipe::MandelMuller2020,
                    crate::stellar::remnant::white_dwarf::WhiteDwarfCore::of(remnant.phase)
                        .expect("a white dwarf"),
                    remnant.mass,
                    crate::stellar::composition::Z_SOLAR,
                ),
                knee: Some(knee),
                ..remnant
            };
            for dt in [10.0, 1e3, 0.5 * t1, 2.0 * t1, 1e8] {
                let age = Years::new(death + dt);
                let (a, b) = (
                    track.state_at(age).luminosity().value(),
                    track.remnant_state_at(rebuilt, age).luminosity().value(),
                );
                assert!(
                    (a / b - 1.0).abs() < 1e-12,
                    "{m} M☉ at +{dt}: {a} against {b}"
                );
            }
            joined += 1;
        }
    }
    assert!(joined >= 6, "only {joined} bridged dwarfs");
}

/// Under [`Bridges::Instant`] the AGB still hands over to the white dwarf directly, with HPT's
/// perturbation, as in SSE (P06.T12.b's comparison runs so).
#[test]
fn without_bridges_the_agb_hands_over_directly() {
    let track = Track::full_with(
        SolarMasses::new(2.0),
        &Composition::SOLAR,
        &StarDraws::median(),
        TrackOptions::default().with_bridges(Bridges::Instant),
    );
    assert!(track.post_agb_start().is_none());
    assert!(
        track
            .segments()
            .iter()
            .all(|s| !matches!(s.model, model::Model::PostAgb { .. }))
    );
}

/// The fast lifetime, the remnant build and the full track agree on a bridged star's death and
/// remnant, bit for bit (the crossing's length and its last luminosity are the same whatever the
/// build keeps).
#[test]
fn every_build_agrees_on_a_bridged_death() {
    for m in [1.0, 2.0, 4.0] {
        let (comp, d) = (Composition::SOLAR, StarDraws::median());
        let track = Track::full(SolarMasses::new(m), &comp, &d);
        let options = TrackOptions::default();
        assert_eq!(
            bits(lifetime_of(SolarMasses::new(m), &comp, &d, options).value()),
            bits(track.lifetime().expect("dies").value())
        );
        let (remnant, _) = remnant_of(SolarMasses::new(m), &comp, &d, options);
        assert_eq!(Some(remnant), track.remnant_model(), "{m} M☉");
    }
}

// -------------------------------------------------------------------------------------------------
// P06.T17: the helium-excess hook.

/// A table that is not the identity, for the hook's tests: s = −4 at every mass and metallicity
/// (ΔY = 0.1 shortens the timescales by e^−0.4 = 0.67, the "about a third" plan 06 expects of a
/// 0.8 M☉ star), and a shift of log `T_eff` of 0.5 per unit ΔY on the horizontal branch.
const TEST_TABLE: HeliumTable = HeliumTable {
    log_z_nodes: crate::tables::helium::LOG_Z_NODES,
    lifetime_slope: [[-4.0, 0.0, 0.0]; 4],
    hb_temperature_shift: [0.5, 0.0],
};

fn with_helium(m: f64, comp: &Composition, table: &HeliumTable) -> Track {
    Track::build_with_helium(
        SolarMasses::new(m),
        comp,
        &StarDraws::median(),
        TrackOptions::default(),
        Resolution::GENERATOR,
        None,
        HeliumHook::of(comp, table),
    )
}

fn state_bits(track: &Track, ages: &[f64]) -> Vec<[u64; 3]> {
    ages.iter()
        .map(|&a| {
            let s = track.state_at(Years::new(a));
            [
                bits(s.luminosity().value()),
                bits(s.radius().value()),
                bits(s.mass().value()),
            ]
        })
        .collect()
}

/// With ΔY = 0 a track is bit-identical whatever the table holds, since the hook is never built;
/// and the committed table, every coefficient zero, is the identity at any ΔY (P06.T17's test).
#[test]
fn without_a_helium_excess_the_track_is_bit_identical_whatever_the_table() {
    let ages: Vec<f64> = (0..400)
        .map(|i| math::exp10(5.0 + 5.2 * f64::from(i) / 399.0))
        .collect();
    for (m, fe_h) in [(0.8, -1.5), (1.0, 0.0), (3.0, -0.5), (20.0, 0.2)] {
        let plain = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let rich = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(0.1));
        assert!(HeliumHook::of(&plain, &TEST_TABLE).is_none());
        let reference = state_bits(&full_of(m, &plain), &ages);
        assert_eq!(
            reference,
            state_bits(&with_helium(m, &plain, &TEST_TABLE), &ages)
        );
        assert_eq!(
            reference,
            state_bits(&with_helium(m, &rich, &HeliumTable::COMMITTED), &ages),
            "{m} M☉: the committed table is the identity"
        );
    }
}

fn full_of(m: f64, comp: &Composition) -> Track {
    Track::full(SolarMasses::new(m), comp, &StarDraws::median())
}

/// A table that is not the identity moves a helium-rich star as plan 15 expects: a 0.8 M☉ star of
/// \[Fe/H\] = −1.5 with ΔY = 0.1 dies about a third sooner, and its horizontal branch is hotter
/// than with the lifetime's correction alone (the shorter giant branch loses less mass, which by
/// itself cools the branch).
#[test]
fn a_helium_excess_shortens_the_life_and_heats_the_horizontal_branch() {
    const LIFETIME_ONLY: HeliumTable = HeliumTable {
        hb_temperature_shift: [0.0; 2],
        ..TEST_TABLE
    };
    let plain = Composition::from_fe_h(Dex::new(-1.5), HeliumExcess::ZERO);
    let rich = Composition::from_fe_h(Dex::new(-1.5), HeliumExcess::new(0.1));
    let (normal, enriched) = (full_of(0.8, &plain), with_helium(0.8, &rich, &TEST_TABLE));
    let ratio =
        enriched.lifetime().expect("dies").value() / normal.lifetime().expect("dies").value();
    assert!((0.6..0.8).contains(&ratio), "lifetime ratio {ratio}");
    let normal = with_helium(0.8, &rich, &LIFETIME_ONLY);
    let hb = |track: &Track| {
        let segment = track
            .segments()
            .iter()
            .find(|s| matches!(s.model, model::Model::CoreHeliumBurning { .. }))
            .expect("a horizontal branch");
        track.state_at(Years::new(f64::midpoint(segment.start, segment.end)))
    };
    let (a, b) = (hb(&normal), hb(&enriched));
    assert!(
        b.effective_temperature().value() > a.effective_temperature().value() * 1.05,
        "{a:?} against {b:?}"
    );
}
