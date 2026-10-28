//! The committed Gaussian-expansion table is exactly what `run mge --since <current>` would write
//! (plan 02, P02.T6.a; plan 15, P15.T1 and P15.T2), and plan 15's refit with free widths
//! (P15.T3.a), built in memory from `manifests/mge.refit.toml`, meets its figures.
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

mod common;

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use clap::Parser;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, TableText, Workspace};
use hyperion_fit::manifest::{Manifest, ManifestParamError};
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{RunTaskError, find, registry};
use hyperion_fit::tasks::mge::boxy::{self, BoxyRow};
use hyperion_fit::tasks::mge::refit::{self, Refit, RefitParams, figures};
use hyperion_fit::tasks::mge::{WIDTHS, fit, residuals};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::galaxy::consts::G;
use hyperion_sim::galaxy::fields::bulge::BoxyBulge;
use hyperion_sim::galaxy::fields::disc::{THIN_DISC_HOLE_LENGTHS, hole_mass_fraction};
use hyperion_sim::galaxy::potential::mge::Gaussian;
use hyperion_sim::galaxy::special::bessel_i0e;
use hyperion_sim::math;
use hyperion_sim::units::{LightYears, SolarMasses};
use hyperion_testkit::float::assert_same_bits;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/mge.rs");

const PI: f64 = core::f64::consts::PI;

/// Euler's constant γ.
const EULER_GAMMA: f64 = core::f64::consts::EULER_GAMMA;

#[test]
fn mge_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("mge").expect("mge is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the mge task runs");
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/mge.rs",
        "mge",
        COMMITTED,
        &planned.text,
    );
    assert!(!planned.body_changed);
}

/// Two runs give the same bytes: the fit has no randomness and no tolerance test.
#[test]
fn the_fit_is_deterministic() {
    assert_eq!(fit(), fit());
}

/// The acceptance figures the header records hold on P15.T3.a's range, s = 0.05–10: the
/// exponential within 1.5% (plan 02's 1% holds on the fit's own samples, not out to s = 10), the
/// bar within 3% of its central value, and its mass within 0.5%.
#[test]
fn the_expansions_follow_their_profiles() {
    let r = residuals(&fit());
    assert!(r.exp_worst_relative < 0.015, "{r:?}");
    assert!(r.bar_worst < 0.03, "{r:?}");
    assert!(r.bar_mass_relative.abs() < 0.005, "{r:?}");
}

// Plan 15's refit (P15.T3.a), in memory from its manifest.

/// The refit's manifest, as committed.
fn refit_manifest_path() -> PathBuf {
    Workspace::repository().manifests_dir.join("mge.refit.toml")
}

fn refit_params() -> RefitParams {
    let manifest = Manifest::load(&refit_manifest_path()).expect("the refit's manifest loads");
    RefitParams::from_manifest(&manifest).expect("the refit's manifest is the refit's")
}

fn the_refit() -> Refit {
    refit::refit(&refit_params())
}

/// P15.T3.a's figures: `MGE_EXP` under 0.5% of e^(−s) on 0.05–10 with 14 terms and its spherical
/// mass within 0.05%; `MGE_BAR` within 3% of its central value, its mass within 0.3% and its
/// cumulative mass within 5% at plan 02's edges, 0.06–2.5 half-lengths.
#[test]
fn the_refit_meets_plan_fifteen_s_figures() {
    let tables = the_refit();
    let f = figures(&tables);
    assert_eq!(tables.exp.len(), 14);
    assert!(f.exp_worst_relative < 0.005, "{f:?}");
    assert!(f.exp_spherical_mass.abs() < 5e-4, "{f:?}");
    assert!(f.bar_worst < 0.03, "{f:?}");
    assert!(f.bar_mass.abs() < 3e-3, "{f:?}");
    assert!(f.bar_cumulative_worst < 0.05, "{f:?}");
    assert!(refit::passes(&f), "{f:?}");
    // The polish of `MGE_EXP` ends on its own, before the manifest's cap.
    assert!(
        tables.steps[0] < refit_params().iterations(),
        "{:?}",
        tables.steps
    );
}

/// `expansion(table, x) = Σ w exp(−x² ÷ 2σ²)`, as plan 02's tests write it.
fn expansion(table: &[(f64, f64)], x: f64) -> f64 {
    table.iter().fold(0.0, |sum, &(w, s)| {
        sum + w * math::exp(-x * x / (2.0 * s * s))
    })
}

/// Plan 02's table tests (the sim's `tests/galaxy_potential.rs`, P02.T6.a and P02.T12.b) at their
/// thresholds, on the refit's tables: T3.c cannot land unless they pass.
#[test]
fn the_refit_passes_plan_two_s_table_tests() {
    let tables = the_refit();
    for table in [&tables.exp, &tables.bar] {
        assert!(table.iter().all(|&(w, s)| w >= 0.0 && s > 0.0), "{table:?}");
        assert!(
            table.windows(2).all(|pair| pair[0].1 < pair[1].1),
            "{table:?}"
        );
    }
    for i in 0..=4_000 {
        let s = 0.05 * math::exp(f64::from(i) / 4_000.0 * math::ln(160.0));
        let error = expansion(&tables.exp, s) / math::exp(-s) - 1.0;
        assert!(error.abs() < 0.01, "MGE_EXP at s = {s}: {error:e}");
        let x = THIN_DISC_HOLE_LENGTHS;
        let error = (expansion(&tables.holed_exp, s) - math::exp(-x / s - s)) / math::exp(-s);
        assert!(error.abs() < 0.01, "MGE_HOLED_EXP at s = {s}: {error:e}");
    }
    let two_pi = 2.0 * PI;
    let mass = tables.exp.iter().fold(0.0, |sum, &(w, s)| {
        sum + w * two_pi * two_pi.sqrt() * s * s * s
    });
    assert!(
        (mass / (8.0 * PI) - 1.0).abs() < 1e-3,
        "spherical mass {mass}"
    );
    let holed = tables
        .holed_exp
        .iter()
        .fold(0.0, |sum, &(w, sigma)| sum + w * sigma * sigma);
    let fraction = hole_mass_fraction(THIN_DISC_HOLE_LENGTHS);
    assert!(
        (holed / fraction - 1.0).abs() < 1e-5,
        "{holed} against {fraction}"
    );
    for (holed, plain) in tables.holed_exp.iter().zip(&tables.exp) {
        assert_same_bits(holed.1, plain.1);
    }
    assert!(tables.holed_exp.iter().any(|&(w, _)| w < 0.0));
    let f = figures(&tables);
    assert!(f.bar_worst < 0.03, "{f:?}");
    assert!(f.bar_mass.abs() < 5e-3, "{f:?}");
    assert!(f.bar_cumulative_worst < 0.07, "{f:?}");
}

#[test]
fn the_refit_is_deterministic() {
    assert_eq!(the_refit(), the_refit());
}

// The modified Bessel functions for Freeman's disc, by their power series (Abramowitz and Stegun
// 1964, 9.6.10, 9.6.11 and 9.6.13) below `ASYMPTOTIC_FROM` and their asymptotic series (9.7.1 and
// 9.7.2) from there, through `math`. The sim has only `galaxy::special::bessel_i0e`.

/// Where the asymptotic series take over. The power series of K cancel by about `I₀(y) ÷ K₀(y)`,
/// 10⁷ at 9, and the asymptotic series' smallest term is about 10⁻⁸ of the first there, so the two
/// forms meet at about 10⁻⁸. Freeman's disc reads `y` ≤ 4, where the power series keep twelve
/// digits.
const ASYMPTOTIC_FROM: f64 = 9.0;

/// Which modified Bessel function an asymptotic series is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// The first kind, `Iₙ`.
    I,
    /// The second kind, `Kₙ`.
    K,
}

/// `I₀(y)`, `I₁(y)`, `K₀(y)`, `K₁(y)` for `y > 0`.
fn bessel(y: f64) -> [f64; 4] {
    if y >= ASYMPTOTIC_FROM {
        return [
            asymptotic(y, 0, Kind::I),
            asymptotic(y, 1, Kind::I),
            asymptotic(y, 0, Kind::K),
            asymptotic(y, 1, Kind::K),
        ];
    }
    let quarter = 0.25 * y * y;
    // Σ (y²/4)^k ÷ (k!)² and Σ (y²/4)^k ÷ (k! (k+1)!), with the harmonic numbers H_k for K.
    let (mut i0, mut i1, mut k0, mut k1) = (0.0, 0.0, 0.0, 0.0);
    let (mut t0, mut t1, mut harmonic) = (1.0, 1.0, 0.0);
    for k in 0..200_u32 {
        let kf = f64::from(k);
        i0 += t0;
        i1 += t1;
        k0 += t0 * harmonic;
        // ψ(k + 1) + ψ(k + 2) = 2 H_k + 1 ÷ (k + 1) − 2γ.
        k1 += t1 * (2.0 * harmonic + 1.0 / (kf + 1.0) - 2.0 * EULER_GAMMA);
        if t0 < 1e-18 * i0 && t1 < 1e-18 * i1 {
            break;
        }
        t0 *= quarter / ((kf + 1.0) * (kf + 1.0));
        t1 *= quarter / ((kf + 1.0) * (kf + 2.0));
        harmonic += 1.0 / (kf + 1.0);
    }
    let half = 0.5 * y;
    let log_half = math::ln(half);
    let i1 = half * i1;
    [
        i0,
        i1,
        -(log_half + EULER_GAMMA) * i0 + k0,
        1.0 / y + log_half * i1 - 0.5 * half * k1,
    ]
}

/// `Iₙ(y)` or `Kₙ(y)` by its asymptotic series, summed while the terms fall: the terms alternate
/// in sign for `I` and all add for `K`.
fn asymptotic(y: f64, n: u32, kind: Kind) -> f64 {
    let mu = 4.0 * f64::from(n * n);
    let (mut sum, mut term) = (1.0, 1.0);
    for j in 1..60_u32 {
        let odd = f64::from(2 * j - 1);
        let next = term * (mu - odd * odd) / (f64::from(j) * 8.0 * y);
        if next.abs() >= term.abs() {
            break;
        }
        term = next;
        sum += if kind == Kind::K || j % 2 == 0 {
            term
        } else {
            -term
        };
    }
    if kind == Kind::K {
        (PI / (2.0 * y)).sqrt() * math::exp(-y) * sum
    } else {
        math::exp(y) / (2.0 * PI * y).sqrt() * sum
    }
}

/// The series against reference values, `I₀` against the sim's `bessel_i0e`, and the Wronskian
/// `I₀K₁ + I₁K₀ = 1 ÷ y` (Abramowitz and Stegun 1964, 9.6.15) on both sides of the switch to the
/// asymptotic series, where the two forms also agree. The reference values are mpmath's
/// `besseli` and `besselk` (1.4.1) at 40 digits, rounded to the nearest `f64`; Abramowitz and
/// Stegun's Table 9.8 (pp. 416–417) agrees with them to its ten decimals, within about one unit
/// of the last.
#[test]
fn the_bessel_functions_match_their_tables_and_their_wronskian() {
    for (y, want) in [
        (
            1.0,
            [
                1.266_065_877_752_008_4,
                0.565_159_103_992_485,
                0.421_024_438_240_708_34,
                0.601_907_230_197_234_6,
            ],
        ),
        (
            2.0,
            [
                2.279_585_302_336_067_3,
                1.590_636_854_637_329,
                0.113_893_872_749_533_44,
                0.139_865_881_816_522_43,
            ],
        ),
    ] {
        for (got, want) in bessel(y).into_iter().zip(want) {
            assert!(
                (got / want - 1.0).abs() < 1e-13,
                "at {y}: {got} against {want}"
            );
        }
    }
    for i in 0..=60 {
        let y = 0.05 * math::exp(f64::from(i) / 60.0 * math::ln(400.0));
        let [i0, i1, k0, k1] = bessel(y);
        // Twelve digits where Freeman's disc reads them, about eight where the forms meet.
        let digits = if y <= 4.0 { 1e-12 } else { 1e-7 };
        assert!(
            (i0 * math::exp(-y) / bessel_i0e(y) - 1.0).abs() < digits,
            "I₀ at {y}"
        );
        assert!(
            (y * (i0 * k1 + i1 * k0) - 1.0).abs() < digits,
            "Wronskian at {y}"
        );
    }
    let y = ASYMPTOTIC_FROM;
    let series = bessel(y * (1.0 - 1e-15));
    for (n, kind, s) in [
        (0, Kind::I, series[0]),
        (1, Kind::I, series[1]),
        (0, Kind::K, series[2]),
        (1, Kind::K, series[3]),
    ] {
        let a = asymptotic(y, n, kind);
        assert!((a / s - 1.0).abs() < 1e-7, "{kind:?}{n}: {a} against {s}");
    }
}

/// Freeman's (1970, ApJ 160, 811) razor-thin exponential disc of `mass` and scale `length`: its
/// circular speed squared at radius `r`, in (km/s)², `v_c² = 4πGΣ₀R_d y² [I₀K₀ − I₁K₁]` at `y = R
/// ÷ 2R_d`, which with `M = 2πΣ₀R_d²` is `(G M ÷ R_d) 2y² [I₀K₀ − I₁K₁]`.
fn freeman(mass: SolarMasses, length: LightYears, r: LightYears) -> f64 {
    let y = r / (length * 2.0);
    let [i0, i1, k0, k1] = bessel(y);
    G * mass.value() / length.value() * 2.0 * y * y * (i0 * k0 - i1 * k1)
}

/// A double-exponential disc from the refit's `MGE_EXP`, its terms combined as the sim's
/// `potential::mge::double_exponential` combines them: radial term `(wᵢ, sᵢ)` and vertical term
/// `(wⱼ, sⱼ)` make a Gaussian of width `sᵢ L` and axis ratio `sⱼ H ÷ sᵢ L`, of mass in proportion
/// to `wᵢ sᵢ² wⱼ sⱼ`, the masses scaled to `mass`.
fn disc(
    table: &[(f64, f64); WIDTHS],
    mass: SolarMasses,
    length: LightYears,
    height: LightYears,
) -> Vec<Gaussian> {
    let terms: Vec<(f64, LightYears, f64)> = table
        .iter()
        .filter(|&&(w, _)| w > 0.0)
        .flat_map(|&(wi, si)| {
            table
                .iter()
                .filter(|&&(w, _)| w > 0.0)
                .map(move |&(wj, sj)| {
                    (
                        wi * si * si * wj * sj,
                        length * si,
                        (height * sj) / (length * si),
                    )
                })
        })
        .collect();
    let total = terms.iter().fold(0.0, |sum, &(m, _, _)| sum + m);
    terms
        .into_iter()
        .map(|(m, sigma, q)| {
            Gaussian::new(mass * (m / total), sigma, q).expect("a term of a disc is a Gaussian")
        })
        .collect()
}

/// The circular speed squared of `gaussians` at radius `r`, in (km/s)².
fn v_circ_sq(gaussians: &[Gaussian], r: LightYears) -> f64 {
    gaussians.iter().fold(0.0, |sum, g| sum + g.v_circ_sq(r))
}

/// The circular speed of a thin exponential disc built from the refit's `MGE_EXP` is within 0.5%
/// of Freeman's form over 0.2–8 scale lengths. The height is 10⁻⁵ of the scale length. A tenth of
/// that moves the speed by under 5 × 10⁻⁵ of itself, a hundredth of the tolerance, so the
/// thickness takes no measurable share of it. At 10⁻⁴ it was 1.9 × 10⁻⁴ at 0.2 scale lengths.
#[test]
fn the_refit_disc_s_circular_speed_follows_freeman() {
    let tables = the_refit();
    let (mass, length) = (SolarMasses::new(5e10), LightYears::new(10_000.0));
    let thin = disc(&tables.exp, mass, length, length * 1e-5);
    let thinner = disc(&tables.exp, mass, length, length * 1e-6);
    let (mut speed, mut thickness) = ((0.0, 0.0), (0.0, 0.0));
    for i in 0..=120 {
        let x = 0.2 * math::exp(f64::from(i) / 120.0 * math::ln(40.0));
        let r = length * x;
        let v2 = v_circ_sq(&thin, r);
        let error = ((v2 / freeman(mass, length, r)).sqrt() - 1.0).abs();
        if error > speed.0 {
            speed = (error, x);
        }
        let error = ((v2 / v_circ_sq(&thinner, r)).sqrt() - 1.0).abs();
        if error > thickness.0 {
            thickness = (error, x);
        }
    }
    assert!(
        speed.0 < 0.005,
        "speed off Freeman's by {:e} at {} scale lengths",
        speed.0,
        speed.1
    );
    assert!(
        thickness.0 < 5e-5,
        "thickness moves the speed by {:e} at {} scale lengths",
        thickness.0,
        thickness.1
    );
}

// The refit's manifest end to end.

/// The scratch run through the command line: `run mge --manifest manifests/mge.refit.toml --out
/// <scratch>`, its header and items parsed back.
#[test]
fn the_refit_manifest_runs_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("mge.rs");
    let manifest = refit_manifest_path();
    // A fixed few threads, not every core. The task ignores the count, which is named so that the
    // test stays small beside the others if the task ever uses it.
    let cli = Cli::try_parse_from([
        "hyperion-fit",
        "run",
        "mge",
        "--manifest",
        manifest.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--threads",
        "2",
    ])
    .unwrap();
    let mut log = Vec::new();
    run(&cli, &mut log).unwrap();
    let text = fs::read_to_string(&out).unwrap();
    let split = TableText::of_file(&text);
    let header = split.header().unwrap();
    assert_eq!(header.task.as_deref(), Some("mge"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/mge.refit.toml")
    );
    // Every figure of T3.a passes; T3.b's density figure is its ruled structural miss, so the
    // header is provisional by P15.T3.b with that figure in its acceptance line.
    assert_eq!(
        header.kind,
        Some(HeaderKind::Provisional {
            by: "P15.T3.b".to_owned()
        })
    );
    assert!(
        header
            .acceptance
            .as_deref()
            .is_some_and(|a| a.contains("`MGE_EXP` within")
                && a.contains("cumulative mass")
                && a.contains(
                    "`MGE_BOXY` at c = 3, 3.5 and 4: mass-weighted rms relative density error"
                )
                && a.contains("a structural miss, ruled 2026-10-23")),
        "{header:?}"
    );
    // P15.T3.c adds `MGE_BOXY` to the task's `items()`; until then the refit's list is here.
    assert_eq!(
        common::declared(&split.body),
        ["MGE_EXP", "MGE_HOLED_EXP", "MGE_BAR", "MGE_BOXY"]
    );
    // Its rows' type is the sim's, which T3.c adds: three rows of sixteen terms.
    assert!(
        split
            .body
            .starts_with("\nuse crate::galaxy::potential::mge::BoxyTerm;\n")
    );
    assert_eq!(
        split.body.matches("        BoxyTerm { weight: ").count(),
        boxy::ROWS * boxy::TERMS
    );
    assert_eq!(
        common::declared(&TableText::of_file(COMMITTED).body),
        find("mge").unwrap().items()
    );
}

/// The task's result on a manifest of `params`.
fn run_on(params: &str) -> Result<(), RunTaskError> {
    let manifest = Manifest::from_bytes(
        Path::new("crates/hyperion-fit/manifests/mge.test.toml"),
        format!("task = \"mge\"\n\n[params]\n{params}").into_bytes(),
    )
    .unwrap();
    find("mge")
        .unwrap()
        .run(&manifest, NonZeroUsize::MIN)
        .map(|_| ())
}

/// The refit's manifest's text with the line starting `key =` replaced by `line`.
fn refit_params_with(key: &str, line: &str) -> String {
    let text = fs::read_to_string(refit_manifest_path()).unwrap();
    let params = text.split_once("[params]\n").unwrap().1;
    params
        .lines()
        .map(|l| {
            if l.starts_with(&format!("{key} =")) {
                line
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A manifest is the first cut's or the refit's: another method, a key the refit does not read,
/// a value its tables fix, or a first-cut manifest with a key of the refit's is refused, before any
/// fitting.
#[test]
fn a_manifest_that_is_neither_cut_nor_refit_is_refused() {
    let param = |result: Result<(), RunTaskError>| match result {
        Err(RunTaskError::Param(e)) => e,
        other => panic!("expected a parameter error, got {other:?}"),
    };
    assert_eq!(
        param(run_on(&refit_params_with("method", "method = \"simplex\""))),
        ManifestParamError::new(
            "method",
            "\"free_widths\" (plan 15's refit), or absent for the first cut"
        )
    );
    assert_eq!(
        param(run_on(&refit_params_with(
            "widths",
            "widths = 14\nsweeps = 50000"
        ))),
        ManifestParamError::new("sweeps", "absent: the refit reads no such key")
    );
    assert_eq!(
        param(run_on(&refit_params_with("widths", "widths = 16"))),
        ManifestParamError::new("widths", "14, the tables' number of terms")
    );
    assert_eq!(
        param(run_on(&refit_params_with("bar_end", "bar_end = 0.2"))),
        ManifestParamError::new(
            "bar_end",
            "0.15, the bar's nominal shape, which `bar_profile` is written for"
        )
    );
    let first_cut = fs::read_to_string(Workspace::repository().manifests_dir.join("mge.toml"))
        .unwrap()
        .split_once("[params]\n")
        .unwrap()
        .1
        .to_owned();
    assert_eq!(
        param(run_on(&format!("{first_cut}bar_rounds = 6\n"))),
        ManifestParamError::new("bar_rounds", "absent: the task reads no such key")
    );
}

/// The refit's values out of their ranges are refused, before any fitting: a range that falls,
/// an array of the wrong length, too few samples, too large a difference step, no rounds, a
/// negative weight, and for the boxy bulge another number of terms, boxinesses that fall, a
/// split of q below 1 or outside the axis ratios' range, too few latitudes, and a grid past 100
/// scale lengths.
#[test]
fn the_refit_refuses_values_out_of_range() {
    let param = |key: &str, line: &str| match run_on(&refit_params_with(key, line)) {
        Err(RunTaskError::Param(e)) => e,
        other => panic!("expected a parameter error for {line}, got {other:?}"),
    };
    assert_eq!(
        param("exp_range", "exp_range = [12.0, 0.001]"),
        ManifestParamError::new("exp_range", "an array of two positive numbers, ascending")
    );
    assert_eq!(
        param("bar_minimax_scales", "bar_minimax_scales = [0.03, 0.003]"),
        ManifestParamError::new("bar_minimax_scales", "an array of three positive numbers")
    );
    assert_eq!(
        param("exp_samples", "exp_samples = 5"),
        ManifestParamError::new("exp_samples", "a count from 14 to 1000000")
    );
    assert_eq!(
        param("difference_step", "difference_step = 0.1"),
        ManifestParamError::new(
            "difference_step",
            "a step in ln width above 0 and below 0.01"
        )
    );
    assert_eq!(
        param("bar_rounds", "bar_rounds = 0"),
        ManifestParamError::new("bar_rounds", "a count from 1 to 1000000")
    );
    assert_eq!(
        param("bar_mass_weight", "bar_mass_weight = -1.0"),
        ManifestParamError::new("bar_mass_weight", "a finite number of at least 0")
    );
    assert_eq!(
        param("boxy_terms", "boxy_terms = 12"),
        ManifestParamError::new("boxy_terms", "16, the terms of each row of `MGE_BOXY`")
    );
    assert_eq!(
        param("boxy_boxiness", "boxy_boxiness = [4.0, 3.5, 3.0]"),
        ManifestParamError::new("boxy_boxiness", "an array of 3 positive numbers, ascending")
    );
    assert_eq!(
        param("boxy_split", "boxy_split = 0.9"),
        ManifestParamError::new(
            "boxy_split",
            "a number above 1 whose inverse and itself lie within `boxy_axis_ratios`"
        )
    );
    assert_eq!(
        param("boxy_axis_ratios", "boxy_axis_ratios = [0.05, 1.0]"),
        ManifestParamError::new(
            "boxy_split",
            "a number above 1 whose inverse and itself lie within `boxy_axis_ratios`"
        )
    );
    // Two latitudes would leave fewer residuals than the fit's 48 parameters.
    assert_eq!(
        param("boxy_angles", "boxy_angles = 2"),
        ManifestParamError::new("boxy_angles", "a count from 3 to 1000000")
    );
    assert_eq!(
        param("boxy_range", "boxy_range = [0.001, 800.0]"),
        ManifestParamError::new("boxy_range", "a range that ends at 100 or below")
    );
}

// Plan 15's boxy bulge (P15.T3.b), in memory from the refit's manifest.

/// The spherical exponential's mass inside `r`, `8π [1 − e^(−r) (1 + r + r² ÷ 2)]`: the boxy body
/// at `c` = 2.
fn spherical_exponential_mass(r: f64) -> f64 {
    8.0 * PI * (1.0 - math::exp(-r) * (1.0 + r + 0.5 * r * r))
}

/// The references against the spherical exponential, the body at `c` = 2: the direct mass to
/// 10⁻¹², and both rules for the midplane force, `M(< R) ÷ R²`, the brute-force one to 10⁻⁶ and
/// the multipole one to 10⁻⁸. At `c` = 3, 3.5 and 4 the whole mass is `3! V(c)` with `V` the sim's
/// `BoxyBulge::unit_volume` to 10⁻¹², the mass inside 60 scale lengths is the whole to 10⁻¹², and
/// the multipole rule, which the figures and the fit read, agrees with the brute-force integral
/// to 10⁻⁶ at the six radii.
#[test]
fn the_boxy_references_hold_to_their_closed_forms_and_to_each_other() {
    for r in boxy::MASS_RADII.into_iter().chain([8.0, 30.0]) {
        let (got, want) = (boxy::enclosed_mass(2.0, r), spherical_exponential_mass(r));
        assert!(
            (got / want - 1.0).abs() < 1e-12,
            "mass at {r}: {got} against {want}"
        );
    }
    assert!((boxy::total_mass(2.0) / (8.0 * PI) - 1.0).abs() < 1e-12);
    for c in [3.0, 3.5, 4.0] {
        let whole = boxy::total_mass(c);
        let volume = 6.0 * BoxyBulge::unit_volume(c);
        assert!(
            (whole / volume - 1.0).abs() < 1e-12,
            "c = {c}: {whole} against {volume}"
        );
        let inside = boxy::enclosed_mass(c, 60.0);
        assert!(
            (inside / whole - 1.0).abs() < 1e-12,
            "c = {c}: {inside} against {whole}"
        );
    }
    for r in boxy::FORCE_RADII {
        let want = spherical_exponential_mass(r) / (r * r);
        let brute = boxy::midplane_force(2.0, r);
        assert!(
            (brute / want - 1.0).abs() < 1e-6,
            "brute force at {r}: {brute} against {want}"
        );
        let multipole = boxy::multipole_force(2.0, r);
        assert!(
            (multipole / want - 1.0).abs() < 1e-8,
            "multipole at {r}: {multipole} against {want}"
        );
    }
    for c in [3.0, 3.5, 4.0] {
        for r in boxy::FORCE_RADII {
            let (brute, multipole) = (boxy::midplane_force(c, r), boxy::multipole_force(c, r));
            assert!(
                (multipole / brute - 1.0).abs() < 1e-6,
                "c = {c} at {r}: multipole {multipole} against brute force {brute}"
            );
        }
    }
}

/// The refit's three rows of `MGE_BOXY`, from its own `MGE_EXP`, as the manifest runs them.
fn boxy_rows() -> [BoxyRow; 3] {
    let params = refit_params();
    boxy::fit(&refit::refit(&params).exp, params.boxy())
}

/// The inward pull in the plane of `terms` at radius `r`, `G` = 1: the sim's Gaussians, as T3.c
/// builds them from the table.
fn boxy_pull(terms: &[boxy::Term], r: f64) -> f64 {
    terms.iter().fold(0.0, |sum, t| {
        let g = Gaussian::new(SolarMasses::new(t.mass()), LightYears::new(t.width), t.q).unwrap();
        sum + g.v_circ_sq(LightYears::new(r)) / (G * r)
    })
}

/// P15.T3.b's figures for each boxiness, and plan 02's table checks for each row of `MGE_BOXY`:
///
/// - the mass inside spheres of 0.5, 1, 2 and 4 scale lengths within 1% of the direct integral,
///   and the whole mass within 1%;
/// - the midplane force within 1.9% of the multipole rule at the six radii, so within 2% of the
///   brute-force integral, which the references test holds to it, and within 2% at 17 more radii
///   between 0.2 and 12 scale lengths, so that the fit, whose force rows sit at the six, holds
///   between them too;
/// - the density's mass-weighted rms error is the structural miss main ruled on 2026-10-23
///   (issue #94): above plan 15's 3%, which no non-negative sum of aligned Gaussians reaches, and
///   held to the recorded best (13.3%, 17.5% and 20.7%) to half a percentage point;
/// - non-negative weights, strictly ascending widths and positive axis ratios, which plan 02's
///   `the_expansions_have_non_negative_weights_and_ascending_widths` gains at T3.c.
#[test]
fn mge_boxy_meets_plan_fifteen_s_mass_and_force_figures_and_holds_its_density_miss() {
    let recorded = [0.133, 0.175, 0.207];
    for (row, best) in boxy_rows().iter().zip(recorded) {
        let f = boxy::figures(row);
        assert!(f.worst_enclosed() < 0.01, "{f:?}");
        assert!(f.total.abs() < 0.01, "{f:?}");
        assert!(f.worst_force() < 0.019, "{f:?}");
        for i in 0..=16 {
            let r = 0.2 * math::exp(f64::from(i) / 16.0 * math::ln(60.0));
            let error = boxy_pull(row.terms(), r) / boxy::multipole_force(row.c(), r) - 1.0;
            assert!(
                error.abs() < 0.02,
                "c = {} at {r}: force off by {error:e}",
                row.c()
            );
        }
        assert!(
            f.rms_density > 0.03 && f.rms_density < best + 0.005,
            "{f:?}"
        );
        assert!(!f.passes(), "{f:?}");
        // Every term carries weight: the seed splits after its weights are solved.
        assert!(
            row.terms()
                .iter()
                .all(|t| t.weight > 0.0 && t.width > 0.0 && t.q > 0.0),
            "{row:?}"
        );
        assert!(
            row.terms()
                .windows(2)
                .all(|pair| pair[0].width < pair[1].width),
            "{row:?}"
        );
    }
}

/// Two fits of the boxiest row, the one Levenberg–Marquardt works hardest on, from the same
/// one-dimensional table give the same bits: compared as their `Debug` text, the shortest
/// decimals that read back to each bit pattern, so that a zero's sign counts, as it would in the
/// table's literals. The rows are fitted independently by the same code; all three twice would
/// take some 10 s.
#[test]
fn mge_boxy_is_deterministic() {
    let params = refit_params();
    let exp = refit::refit(&params).exp;
    let c = params.boxy().boxiness()[2];
    let once = format!("{:?}", boxy::fit_row(c, &exp, params.boxy()));
    assert_eq!(once, format!("{:?}", boxy::fit_row(c, &exp, params.boxy())));
}
