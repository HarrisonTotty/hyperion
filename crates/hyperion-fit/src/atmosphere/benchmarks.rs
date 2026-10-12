//! The tracer's benchmarks (plan R08, R08.T12.c; Design note 10): the published radiances the
//! tracer is held to before its references are trusted. They are slow tests, run by
//! `just test-slow atmosphere::benchmarks`.
//!
//! **What runs.** Every case is a plane-parallel slab, 1 km of uniform medium over a ground of
//! radius 10¹² m (whose curvature moves nothing at the precision tested), seen by pencil
//! detectors at its top and bottom, in the Stokes mode:
//!
//! - Natraj, Li and Yung 2009 (ApJ 691, 1909; tables at
//!   <https://web.gps.caltech.edu/~vijay/Rayleigh_Scattering_Tables/>, `CDS/`): Rayleigh with
//!   ρ = 0 at τ 0.5, μ₀ 0.2 over a black ground, and at τ 1, μ₀ 0.6 over a Lambertian albedo of
//!   0.8;
//! - Natraj and Hovenier 2012 (ApJ 748, 28; the same host's `STOKES/`): τ 2, μ₀ 0.5, albedo 0.25;
//! - Kokhanovsky et al. 2010 (JQSRT 111, 1931), their molecular case (Tables A1, A4 and A5):
//!   τ 0.3262, θ₀ 60°, black ground;
//! - IPRT Phase A (Emde et al. 2015, JQSRT 164, 8; results at <https://www.libradtran.org/iprt/>):
//!   A1 (Rayleigh, τ 0.5, ρ 0, 0.03 and 0.1), A2 (Rayleigh, τ 0.1, ρ 0.03, over a Lambertian
//!   albedo of 0.3) and A4 (prolate spheroids, τ 0.2, ω 0.787581, with IPRT's own scattering
//!   matrix, in `crates/hyperion-fit/data/iprt_phase_a/`), against the model PSTAR's results;
//! - (R08.T12.e) the spheres, whose matrices the client's Mie writes over each case's stated size
//!   distribution (`apps/hyperion/src/renderer/src/test/mieMatrices.ts`, into `sphere_matrices/`
//!   beside this file; `decision-r08-licences.md` row 6c): Kokhanovsky et al.'s aerosol (Tables A2,
//!   A4–A6: τ 0.3262, θ₀ 60°, black ground) and IPRT's A3 (water-soluble aerosol, τ 0.2, θ₀ 40°),
//!   against PSTAR's results. Their V tables check the sign of the client's b₂ (R08.T5.b).
//!   Kokhanovsky et al.'s one value at the sun itself is left out ([`SUN_EXCLUSION_DEG`]), as are
//!   seven values of their VZA 89° row, five intensities and two reflected −Q
//!   ([`KOKHANOVSKY_AEROSOL_HORIZON`]), and two signs of their Table A2 are corrected
//!   ([`KOKHANOVSKY_AEROSOL_ERRATA`]).
//! - (R08.T12.f) the two τ 5 clouds, whose forward peaks (280–620 sr⁻¹, about 1° wide) the
//!   tracer's VROOM handles (`transport`): Kokhanovsky et al.'s cloud (Tables A3–A6: τ 5, θ₀ 60°,
//!   black ground, the writer's matrix at its level 3), its one view at the sun left out and four
//!   signs of its transmitted V corrected ([`KOKHANOVSKY_CLOUD_ERRATA`]), and IPRT's A5 (a water
//!   cloud, τ 5, θ₀ 50°, in the principal plane and the almucantar), against PSTAR's results.
//!   Beside them, two checks of VROOM itself: against R08.T12.c's sunward draws alone in a
//!   Henyey–Greenstein slab where those converge, and energy conservation in a conservative slab
//!   of A5's matrix.
//!
//! **What waits** (R08's Risks, "Deviations in T12.c, as built" and "Deviations in T12.e, as
//! built"):
//!
//! - Garcia and Siewert 1985's Haze L and Cloud C1 (scalar, Legendre series): no lawful copy of
//!   the paper is held, so their values are pending for the owner, for access. The `legendre`
//!   phase kind they need is built and tested.
//! - IPRT A6 (an ocean surface): not run, since the tracer's ground is Lambertian.
//! - Loughman et al. 2004 (JGR 109, D06303): the publisher's free copy is behind a bot wall that
//!   refuses every fetch from here, so the case is pending for access.
//!
//! **Conventions.** Each benchmark's Stokes vector is the tracer's with the signs of its
//! [`Signs`]; relative azimuths map directly. Natraj, Li and Yung 2009 tabulate −Q and −U of the
//! tracer's (their Q = Iᵣ − Iₗ), and Natraj and Hovenier 2012 the tracer's own (Hovenier's).
//! Kokhanovsky et al. tabulate −Q, as their column heads say, Q referred to the meridian plane as
//! here; their radiances are π I ÷ (μ₀F₀). IPRT's radiances are I ÷ F₀, sr⁻¹, its viewing
//! direction's zenith and azimuth the tracer's, and PSTAR's U and V are −U and −V of the tracer's,
//! one difference of handedness (IPOL's and SHDOM's files carry the other signs of U and V; with
//! those negated, IPOL agrees with PSTAR to 10⁻⁶ of I in A1). Kokhanovsky et al.'s V is the
//! tracer's own ([`KOKHANOVSKY_SIGNS`]).
//!
//! **Statistics.** Design note 10 asks each value to agree to 3σ, with σ ≤ 0.3%. Every value's
//! standard error is held to 0.3% of the geometry's I. Over a benchmark of N values, a 3σ bound
//! on each would fail by chance with probability 1 − 0.9973ᴺ (19% for N = 78), against the
//! testkit's α = 10⁻³, so each value is held to the Bonferroni bound at α ÷ 2 over the N values
//! (4.5σ at N = 78), and each Stokes component's χ² over its geometries, which are independent
//! draws (each geometry its own stream, and each of a benchmark's slabs its own seed), to its
//! p-value at α ÷ 2 over the components: a systematic error of a fraction of σ
//! across the values fails the second, as a single outlier does the first. The count past 3σ and
//! the largest |z| are printed for the record. The benchmarks' own errors (their printed digits,
//! for IPRT the spread between PSTAR and IPOL, and for Kokhanovsky et al.'s spheres the spread
//! they state between Pstar and SCIATRAN) enter σ in quadrature. A value's σ is a standard error
//! only where it falls as N^(−1/2): every case is traced at N and at N ÷ 4 samples, and a value
//! whose σ at N stands more than 25% from half its σ at N ÷ 4 is left out of the assertions and
//! named, the set each benchmark leaves out pinned in [`EXPECTED_UNSCALED`]
//! (`decision-r08-t4b-t12c.md`, follow-up, ruling 7). R08.T12.f's sample counts were set from
//! pilot runs at other seeds, so that each σ falls under its cap with some 15% to spare, and the
//! benchmarks are traced afresh at their own (Design note 10, _Statistics_).
//!
//! **Licences** (`decision-r08-licences.md` row 6): only the values each test asserts are
//! committed, in the tests' own layout, with their tables cited: Natraj's and Kokhanovsky's
//! inline here, IPRT's (CC BY-SA 3.0) as selected rows in their own data files, never in a Rust
//! source.

use core::f64::consts::PI;
use std::fmt::Write as _;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::Path;

use hyperion_sim::math;
use hyperion_testkit::stats::{ALPHA, assert_p_value, regularised_gamma_q};
use serde_json::{Value, json};

use super::transport::Estimator;
use super::{AtmosphereCase, Polarisation, ReferenceRadiances, trace_reference_with};
use crate::data::load_dataset;
use Level::{Bottom, Top};

/// The threads each benchmark traces on.
const THREADS: usize = 4;

/// The largest standard error a value may carry, relative to its geometry's I (Design note 10).
const SIGMA_LIMIT: f64 = 3e-3;

/// How far a value's standard error may stand from N^(−1/2) scaling, as a fraction: its σ at the
/// run's N samples must lie within this of half its σ over the run's first N ÷ 4, or it is not
/// asserted (`decision-r08-t4b-t12c.md`, follow-up, ruling 7). A heavy-tailed estimate, whose σ̂ is
/// not a standard error, fails it.
const SIGMA_SCALING_TOLERANCE: f64 = 0.25;

/// The values each benchmark leaves out by [`SIGMA_SCALING_TOLERANCE`], by its name and as its
/// summary names them; a benchmark not listed leaves none out. Recorded from the runs at their
/// seeds and counts (R08's Risks, "Deviations in T12.e, as built", and "Deviations in T12.f, as
/// built" for the benchmarks VROOM runs since and R08.T12.f's own).
///
/// - Natraj and Hovenier's τ 2: three of its thick transmitted values, where T12.c measured the
///   estimator's coefficient of variation at about 3.
/// - R08.T12.f's check of VROOM against the sunward draws: the transmitted view at VZA 10°, φ 0°,
///   where the sunward draws' σ does not fall as N^(−1/2) (VROOM's does).
///
/// Kokhanovsky et al.'s aerosol left out its V at VZA 60°, φ 90°, reflected and transmitted, under
/// R08.T12.c's estimator; under VROOM, which its forward peak turns on since R08.T12.f, it leaves
/// out nothing.
const EXPECTED_UNSCALED: [(&str, &[&str]); 2] = [
    (
        "Natraj and Hovenier 2012, τ 2, μ₀ 0.5, A 0.25",
        &[
            "Bottom μ 0.1 φ 180 Q",
            "Bottom μ 1 φ 0 I",
            "Bottom μ 1 φ 0 U",
        ],
    ),
    (VROOM_CHECK, &["Bottom VZA 10 φ 0 I"]),
];

/// The slab's thickness, m.
const SLAB_M: f64 = 1000.0;

/// Where a detector stands in the slab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    /// At the top, looking down: the upwelling light.
    Top,
    /// At the bottom, looking up: the downwelling light.
    Bottom,
}

/// A Stokes component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Component {
    I,
    Q,
    U,
    V,
}

impl Component {
    /// The component's place in (I, Q, U, V).
    #[must_use]
    fn index(self) -> usize {
        match self {
            Self::I => 0,
            Self::Q => 1,
            Self::U => 2,
            Self::V => 3,
        }
    }
}

/// How a benchmark's Q, U and V stand to the tracer's: each the tracer's times its sign.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Signs {
    q: f64,
    u: f64,
    v: f64,
}

/// One value asserted.
#[derive(Debug, Clone, PartialEq)]
struct Comparison {
    label: String,
    component: Component,
    /// The tracer's value, in the benchmark's convention and unit.
    traced: f64,
    /// Its standard error.
    sigma: f64,
    /// Whether the standard error falls as N^(−1/2) ([`SIGMA_SCALING_TOLERANCE`]): a value whose
    /// does not is left out of the assertions, and named.
    sigma_scales: bool,
    /// The benchmark's value.
    expected: f64,
    /// The benchmark's own error.
    expected_sigma: f64,
    /// The geometry's I, in the same unit.
    intensity: f64,
}

/// A detector in the slab, looking along `zenith_deg` and `azimuth_deg` (the tracer's: the view's
/// zenith angle and its azimuth), with the sun at `sun`, its zenith angle and azimuth (degrees).
fn geometry(name: &str, level: Level, zenith_deg: f64, azimuth_deg: f64, sun: (f64, f64)) -> Value {
    json!({
        "name": name,
        "observer": { "heightM": if level == Top { SLAB_M } else { 0.0 } },
        "view": { "zenithDeg": zenith_deg, "azimuthDeg": azimuth_deg },
        "sunDirections": [{ "zenithDeg": sun.0, "azimuthDeg": sun.1 }]
    })
}

/// A slab of optical thickness `tau` and single-scattering albedo `omega` scattering by `phase`
/// over a Lambertian ground of `albedo`, at one wavelength, lit by an irradiance of `irradiance`,
/// its draws from `seed`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slab<'a> {
    name: &'a str,
    tau: f64,
    omega: f64,
    phase: &'a Value,
    albedo: f64,
    irradiance: f64,
    seed: u64,
}

impl Slab<'_> {
    /// The case of the slab seen from `geometries`.
    fn case(&self, geometries: &[Value]) -> AtmosphereCase {
        let extinction = self.tau / SLAB_M;
        let case = json!({
            "format": 1,
            "name": self.name,
            "seed": self.seed,
            "wavelengthsNm": [550],
            "shells": { "kind": "sphere", "radiusM": 1e12 },
            "topHeightM": SLAB_M,
            "groundAlbedo": [self.albedo],
            "terms": [{
                "name": "medium",
                "density": {
                    "kind": "tabulated", "altitudesM": [0.0, SLAB_M], "relative": [1.0, 1.0]
                },
                "scattering": [self.omega * extinction],
                "absorption": [(1.0 - self.omega) * extinction],
                "phase": self.phase
            }],
            "suns": [{ "name": "sun", "irradiance": [self.irradiance] }],
            "detectorHalfAngleDeg": 0.0,
            "geometries": geometries
        });
        AtmosphereCase::from_json(&case.to_string()).expect("a benchmark case is valid")
    }
}

/// Rayleigh's phase function of depolarisation factor `rho`.
fn rayleigh(rho: f64) -> Value {
    json!({ "kind": "rayleigh", "depolarisation": [rho] })
}

/// A benchmark's trace: the reference at its samples, and the same run's first quarter.
#[derive(Debug, Clone, PartialEq)]
struct Traced {
    full: ReferenceRadiances,
    /// The first quarter of the full run's samples: each sample draws from its own counter-based
    /// stream (the `atmosphere` module's reproducibility), so a trace at N ÷ 4 samples is exactly
    /// the run's first quarter.
    quarter: ReferenceRadiances,
}

/// Traces `case` in the Stokes mode with `samples` samples a geometry, a multiple of 4, and again
/// with a quarter of them, by the estimator every reference runs.
fn trace(case: &AtmosphereCase, samples: u64) -> Traced {
    trace_with(case, samples, Estimator::ByMedium)
}

/// [`trace`] by `estimator`.
fn trace_with(case: &AtmosphereCase, samples: u64, estimator: Estimator) -> Traced {
    assert_eq!(samples % 4, 0, "a benchmark's samples are a multiple of 4");
    let at = |n: u64| {
        trace_reference_with(
            case,
            Polarisation::Stokes,
            NonZeroU64::new(n).expect("a benchmark traces samples"),
            NonZeroUsize::new(THREADS).expect("a benchmark traces on threads"),
            estimator,
        )
        .expect("a benchmark case traces")
    };
    Traced {
        full: at(samples),
        quarter: at(samples / 4),
    }
}

/// Whether a standard error `full` at N samples is within [`SIGMA_SCALING_TOLERANCE`] of
/// `quarter`'s at N ÷ 4 scaled as N^(−1/2), that is of `quarter` ÷ 2; false for a σ that is NaN.
#[must_use]
pub(super) fn sigma_scales(full: f64, quarter: f64) -> bool {
    let expected = 0.5 * quarter;
    (full - expected).abs() <= SIGMA_SCALING_TOLERANCE * expected
}

/// How a benchmark's values stand to the tracer's: the scale into the benchmark's unit, and the
/// signs into its convention.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Convention {
    scale: f64,
    signs: Signs,
}

/// A benchmark's comparisons, gathered from one traced reference at a time.
#[derive(Debug, Clone, PartialEq)]
struct Comparisons {
    convention: Convention,
    values: Vec<Comparison>,
}

impl Comparisons {
    fn new(convention: Convention) -> Self {
        Self {
            convention,
            values: Vec::new(),
        }
    }

    /// Compares geometry `g` of `reference` with `expected`, I, Q, U and V in the benchmark's
    /// convention, each with its own error in `expected_sigma`; a value the benchmark does not
    /// give (`None`) is not compared.
    fn push(
        &mut self,
        reference: &Traced,
        g: usize,
        expected: [Option<f64>; 4],
        expected_sigma: [f64; 4],
        label: &str,
    ) {
        let Convention { scale, signs } = self.convention;
        let traced = &reference.full.geometries()[g];
        let stokes = traced.stokes().expect("a benchmark traces the Stokes mode");
        let quarter = &reference.quarter.geometries()[g];
        let quarter_stokes = quarter
            .stokes()
            .expect("a benchmark traces the Stokes mode");
        let quarter_sigma = [
            quarter.standard_error()[0],
            quarter_stokes.q_standard_error()[0],
            quarter_stokes.u_standard_error()[0],
            quarter_stokes.v_standard_error()[0],
        ];
        let intensity = scale * traced.radiance()[0];
        let values = [
            (
                Component::I,
                1.0,
                traced.radiance()[0],
                traced.standard_error()[0],
            ),
            (
                Component::Q,
                signs.q,
                stokes.q()[0],
                stokes.q_standard_error()[0],
            ),
            (
                Component::U,
                signs.u,
                stokes.u()[0],
                stokes.u_standard_error()[0],
            ),
            (
                Component::V,
                signs.v,
                stokes.v()[0],
                stokes.v_standard_error()[0],
            ),
        ];
        for ((((component, sign, value, sigma), expected), expected_sigma), quarter_sigma) in values
            .into_iter()
            .zip(expected)
            .zip(expected_sigma)
            .zip(quarter_sigma)
        {
            if let Some(expected) = expected {
                self.values.push(Comparison {
                    label: label.to_owned(),
                    component,
                    traced: sign * scale * value,
                    sigma: scale * sigma,
                    sigma_scales: sigma_scales(sigma, quarter_sigma),
                    expected,
                    expected_sigma,
                    intensity,
                });
            }
        }
    }
}

/// The values of `comparisons` to assert, those whose σ falls as N^(−1/2), and the labels of the
/// others, sorted.
///
/// # Panics
///
/// If a σ is not finite and ≥ 0, or no value is left to assert.
fn scaling_values<'a>(
    name: &str,
    comparisons: &'a [Comparison],
) -> (Vec<&'a Comparison>, Vec<String>) {
    for c in comparisons {
        assert!(
            c.sigma.is_finite() && c.sigma >= 0.0,
            "{name}, {} {:?}: σ {} is not a standard error",
            c.label,
            c.component,
            c.sigma
        );
    }
    // A value whose σ does not fall as N^(−1/2) carries no standard error, and is not asserted.
    let (scaling, unscaled): (Vec<&Comparison>, Vec<&Comparison>) =
        comparisons.iter().partition(|c| c.sigma_scales);
    let mut left_out: Vec<String> = unscaled
        .iter()
        .map(|c| format!("{} {:?}", c.label, c.component))
        .collect();
    left_out.sort();
    assert!(
        !scaling.is_empty(),
        "{name}: no value's σ falls as N^(−1/2), so nothing is asserted"
    );
    (scaling, left_out)
}

/// Asserts that the values benchmark `name` left out by [`SIGMA_SCALING_TOLERANCE`] are those
/// [`EXPECTED_UNSCALED`] records: the runs are deterministic, so a value that starts or stops
/// scaling fails the benchmark rather than vanishing from it.
fn assert_unscaled_as_recorded(name: &str, left_out: &[String]) {
    let mut expected: Vec<String> = EXPECTED_UNSCALED
        .iter()
        .find(|(benchmark, _)| *benchmark == name)
        .map_or(&[][..], |&(_, values)| values)
        .iter()
        .map(|&value| value.to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        left_out, expected,
        "{name}: the values whose σ does not fall as N^(−1/2) are not those recorded"
    );
}

/// Asserts that `comparisons` agree (module documentation, Statistics), and prints their
/// summary.
fn assert_agreement(name: &str, comparisons: &[Comparison]) {
    let (comparisons, left_out) = scaling_values(name, comparisons);
    let n = comparisons.len();
    let count = u32::try_from(n).expect("a benchmark asserts fewer than 2³² values");
    // The Bonferroni bound: two-sided, at α ÷ 2 over the N values.
    let bound = -math::normal_quantile(ALPHA / (4.0 * f64::from(count)));
    let mut summary = String::new();
    let mut largest = (0.0_f64, String::new());
    let mut past_three = 0;
    for c in &comparisons {
        assert!(
            c.sigma <= SIGMA_LIMIT * c.intensity,
            "{name}, {} {:?}: σ {} is above 0.3% of I = {}",
            c.label,
            c.component,
            c.sigma,
            c.intensity
        );
        let z = (c.traced - c.expected) / math::hypot(c.sigma, c.expected_sigma);
        if z.abs() > 3.0 {
            past_three += 1;
        }
        if z.abs() > largest.0 {
            largest = (z.abs(), format!("{} {:?}", c.label, c.component));
        }
        assert!(
            z.abs() <= bound,
            "{name}, {} {:?}: {} against {} ± {} (σ {}), z = {z:.2}, past the bound {bound:.2}",
            c.label,
            c.component,
            c.traced,
            c.expected,
            math::hypot(c.sigma, c.expected_sigma),
            c.sigma
        );
    }
    let components: Vec<Component> = {
        let mut seen: Vec<Component> = comparisons.iter().map(|c| c.component).collect();
        seen.sort();
        seen.dedup();
        seen
    };
    let families = u32::try_from(components.len()).expect("four components at most");
    for component in components {
        let zs: Vec<f64> = comparisons
            .iter()
            .filter(|c| c.component == component)
            .map(|c| (c.traced - c.expected) / math::hypot(c.sigma, c.expected_sigma))
            .collect();
        let chi_square: f64 = zs.iter().map(|z| z * z).sum();
        // The same χ² against the benchmark's values of the opposite sign: how firmly the run
        // tells the sign convention apart, printed for the record (I is not signed).
        let flipped: f64 = comparisons
            .iter()
            .filter(|c| c.component == component)
            .map(|c| {
                let z = (c.traced + c.expected) / math::hypot(c.sigma, c.expected_sigma);
                z * z
            })
            .sum();
        let dof = f64::from(u32::try_from(zs.len()).expect("fewer than 2³² values"));
        let p = regularised_gamma_q(0.5 * dof, 0.5 * chi_square);
        write!(
            summary,
            " {component:?}: χ² {chi_square:.1} for {dof}, p {p:.3}"
        )
        .expect("writing to a String cannot fail");
        if component != Component::I {
            write!(summary, " (sign flipped {flipped:.0})")
                .expect("writing to a String cannot fail");
        }
        summary.push(';');
        assert_p_value(
            &format!("{name}, χ² of {component:?}"),
            p,
            ALPHA / (2.0 * f64::from(families)),
        );
    }
    let worst_sigma = comparisons
        .iter()
        .map(|c| c.sigma / c.intensity)
        .fold(0.0, f64::max);
    eprintln!(
        "{name}: {n} values, largest |z| {:.2} ({}), {past_three} past 3σ, bound {bound:.2}, \
         largest σ ÷ I {worst_sigma:.2e};{summary} {} left out, their σ not falling as N^(−1/2): \
         {left_out:?}",
        largest.0,
        largest.1,
        left_out.len()
    );
    assert_unscaled_as_recorded(name, &left_out);
}

/// A row of Natraj's tables: the level, μ, the relative azimuth φ (degrees), and I, Q and U as
/// published, for πF₀ = π.
type NatrajRow = (Level, f64, f64, f64, f64, f64);

/// Natraj, Li and Yung 2009, `CDS/{I,Q,U}_{UP,DN}_TAU_0.5`, albedo 0, μ₀ = 0.2: T12.a's case.
#[expect(
    clippy::unreadable_literal,
    reason = "the values as the tables print them, to be checked digit by digit"
)]
const NATRAJ_2009_TAU_0_5: [NatrajRow; 26] = [
    (Top, 0.10, 0.0, 0.35082997, -0.01524525, 0.00000000),
    (Top, 0.10, 90.0, 0.21348201, -0.16821107, 0.06357361),
    (Top, 0.10, 180.0, 0.36354469, -0.02795997, 0.00000000),
    (Top, 0.20, 0.0, 0.26939388, -0.00608288, 0.00000000),
    (Top, 0.20, 90.0, 0.17169133, -0.13222805, 0.04872177),
    (Top, 0.20, 180.0, 0.28888259, -0.02557158, 0.00000000),
    (Top, 0.52, 0.0, 0.13097041, 0.01968930, 0.00000000),
    (Top, 0.52, 90.0, 0.09759154, -0.07273772, 0.02406753),
    (Top, 0.52, 180.0, 0.15600064, -0.00534093, 0.00000000),
    (Top, 0.84, 0.0, 0.06783499, 0.03668441, 0.00000000),
    (Top, 0.84, 90.0, 0.06393739, -0.04623546, 0.01057373),
    (Top, 0.84, 180.0, 0.08559886, 0.01892054, 0.00000000),
    (Top, 1.00, 0.0, 0.05300496, 0.03755859, 0.00000000),
    (Bottom, 0.10, 0.0, 0.13485741, -0.01538451, 0.00000000),
    (Bottom, 0.10, 90.0, 0.08801144, -0.05898050, 0.02077195),
    (Bottom, 0.10, 180.0, 0.13070302, -0.01123012, 0.00000000),
    (Bottom, 0.20, 0.0, 0.15838610, -0.01767297, 0.00000000),
    (Bottom, 0.20, 90.0, 0.10033646, -0.07022742, 0.02479837),
    (Bottom, 0.20, 180.0, 0.14846675, -0.00775363, 0.00000000),
    (Bottom, 0.52, 0.0, 0.12178394, -0.00542392, 0.00000000),
    (Bottom, 0.52, 90.0, 0.07796226, -0.05573965, 0.01823144),
    (Bottom, 0.52, 180.0, 0.10282324, 0.01353678, 0.00000000),
    (Bottom, 0.84, 0.0, 0.07348081, 0.01547851, 0.00000000),
    (Bottom, 0.84, 90.0, 0.05534670, -0.03883915, 0.00889191),
    (Bottom, 0.84, 180.0, 0.05854240, 0.03041692, 0.00000000),
    (Bottom, 1.00, 0.0, 0.04682203, 0.03225729, 0.00000000),
];

/// Natraj, Li and Yung 2009, `CDS/{I,Q,U}_{UP,DN}_TAU_1`, albedo 0.8, μ₀ = 0.6.
#[expect(
    clippy::unreadable_literal,
    reason = "the values as the tables print them, to be checked digit by digit"
)]
const NATRAJ_2009_TAU_1: [NatrajRow; 26] = [
    (Top, 0.20, 0.0, 0.56195382, 0.08445521, 0.00000000),
    (Top, 0.20, 60.0, 0.49089856, -0.03578087, 0.20191494),
    (Top, 0.20, 120.0, 0.53242771, -0.07731002, 0.15773810),
    (Top, 0.20, 180.0, 0.64501213, 0.00139689, 0.00000000),
    (Top, 0.52, 0.0, 0.47725195, 0.11269547, 0.00000000),
    (Top, 0.52, 60.0, 0.46085463, -0.01211067, 0.15935147),
    (Top, 0.52, 120.0, 0.53108204, -0.08233807, 0.07456667),
    (Top, 0.52, 180.0, 0.61770676, -0.02775934, 0.00000000),
    (Top, 0.84, 0.0, 0.43725670, 0.11163378, 0.00000000),
    (Top, 0.84, 60.0, 0.44906720, -0.00887924, 0.11006429),
    (Top, 0.84, 120.0, 0.50469021, -0.06450226, 0.00462844),
    (Top, 0.84, 180.0, 0.54850273, 0.00038775, 0.00000000),
    (Top, 1.00, 0.0, 0.46917958, 0.06476738, 0.00000000),
    (Bottom, 0.20, 0.0, 0.54343745, -0.01182454, 0.00000000),
    (Bottom, 0.20, 60.0, 0.48947168, -0.04927829, 0.07677722),
    (Bottom, 0.20, 120.0, 0.46930289, -0.02910950, 0.09788965),
    (Bottom, 0.20, 180.0, 0.50309986, 0.02851305, 0.00000000),
    (Bottom, 0.52, 0.0, 0.50768142, -0.02486171, 0.00000000),
    (Bottom, 0.52, 60.0, 0.44678528, -0.06285040, 0.05299454),
    (Bottom, 0.52, 120.0, 0.39713938, -0.01320450, 0.11236937),
    (Bottom, 0.52, 180.0, 0.40838961, 0.07443010, 0.00000000),
    (Bottom, 0.84, 0.0, 0.40181472, -0.00108813, 0.00000000),
    (Bottom, 0.84, 60.0, 0.36677945, -0.05263716, 0.00397016),
    (Bottom, 0.84, 120.0, 0.32219935, -0.00805706, 0.08795246),
    (Bottom, 0.84, 180.0, 0.31265451, 0.08807208, 0.00000000),
    (Bottom, 1.00, 0.0, 0.31234078, 0.05344101, 0.00000000),
];

/// Natraj and Hovenier 2012, `STOKES/{I,Q,U}_{UP,DN}_TAU_2`, albedo 0.25, μ₀ = 0.5.
#[expect(
    clippy::unreadable_literal,
    reason = "the values as the tables print them, to be checked digit by digit"
)]
const NATRAJ_HOVENIER_2012_TAU_2: [NatrajRow; 20] = [
    (Top, 0.10, 0.0, 0.52908133, -0.03666586, 0.00000000),
    (Top, 0.10, 90.0, 0.38996680, 0.14594872, -0.20141378),
    (Top, 0.10, 180.0, 0.56936408, 0.00361689, 0.00000000),
    (Top, 0.50, 0.0, 0.36592488, -0.08844616, 0.00000000),
    (Top, 0.50, 90.0, 0.34434233, 0.11370256, -0.12463336),
    (Top, 0.50, 180.0, 0.49055823, 0.03618719, 0.00000000),
    (Top, 0.90, 0.0, 0.26244302, -0.11109010, 0.00000000),
    (Top, 0.90, 90.0, 0.28930984, 0.08588871, -0.04780830),
    (Top, 0.90, 180.0, 0.34849795, -0.02503517, 0.00000000),
    (Top, 1.00, 0.0, 0.27723461, -0.08017274, 0.00000000),
    (Bottom, 0.10, 0.0, 0.15069939, -0.00095723, 0.00000000),
    (Bottom, 0.10, 90.0, 0.13217771, 0.01409473, -0.01902730),
    (Bottom, 0.10, 180.0, 0.14689393, -0.00476269, 0.00000000),
    (Bottom, 0.50, 0.0, 0.22452437, 0.01119809, 0.00000000),
    (Bottom, 0.50, 90.0, 0.18418650, 0.03343917, -0.03374152),
    (Bottom, 0.50, 180.0, 0.19078285, -0.02254344, 0.00000000),
    (Bottom, 0.90, 0.0, 0.21782802, -0.01125754, 0.00000000),
    (Bottom, 0.90, 90.0, 0.19125214, 0.03941798, -0.02137444),
    (Bottom, 0.90, 180.0, 0.17935403, -0.04973153, 0.00000000),
    (Bottom, 1.00, 0.0, 0.18851672, -0.03911481, 0.00000000),
];

/// The tables' last printed digit, a half unit of the eighth decimal, as a standard error (a
/// uniform error's, over √3).
const NATRAJ_ROUNDING: f64 = 0.5e-8 / 1.732_050_807_568_877_2;

/// Natraj, Li and Yung 2009's Q and U against the tracer's: both the negative (their
/// Q = Iᵣ − Iₗ).
const NATRAJ_2009_SIGNS: Signs = Signs {
    q: -1.0,
    u: -1.0,
    v: 1.0,
};

/// Natraj and Hovenier 2012's Q and U: the tracer's (Hovenier's convention).
const HOVENIER_SIGNS: Signs = Signs {
    q: 1.0,
    u: 1.0,
    v: 1.0,
};

/// Runs one of Natraj's tables: a Rayleigh slab of ρ = 0, its rows traced at πF₀ = π.
fn natraj(
    name: &str,
    tau: f64,
    mu0: f64,
    albedo: f64,
    rows: &[NatrajRow],
    signs: Signs,
    samples: u64,
) {
    let sun = (math::acos(mu0).to_degrees(), 0.0);
    let geometries: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(i, &(level, mu, phi, ..))| {
            let zenith = math::acos(mu).to_degrees();
            let zenith = if level == Top { 180.0 - zenith } else { zenith };
            geometry(&format!("{i}"), level, zenith, phi, sun)
        })
        .collect();
    let phase = rayleigh(0.0);
    let slab = Slab {
        name,
        tau,
        omega: 1.0,
        phase: &phase,
        albedo,
        irradiance: PI,
        seed: 0,
    };
    let reference = trace(&slab.case(&geometries), samples);
    let mut comparisons = Comparisons::new(Convention { scale: 1.0, signs });
    for (g, &(level, mu, phi, i, q, u)) in rows.iter().enumerate() {
        comparisons.push(
            &reference,
            g,
            [Some(i), Some(q), Some(u), None],
            [NATRAJ_ROUNDING; 4],
            &format!("{level:?} μ {mu} φ {phi}"),
        );
    }
    assert_agreement(name, &comparisons.values);
}

#[test]
#[ignore = "slow: Natraj et al. 2009's τ 0.5 Rayleigh table, 26 geometries, under a minute"]
fn stokes_radiances_match_natraj_2009_at_tau_half_over_a_black_ground() {
    natraj(
        "Natraj et al. 2009, τ 0.5, μ₀ 0.2, A 0",
        0.5,
        0.2,
        0.0,
        &NATRAJ_2009_TAU_0_5,
        NATRAJ_2009_SIGNS,
        500_000,
    );
}

#[test]
#[ignore = "slow: Natraj et al. 2009's τ 1 Rayleigh table over a Lambertian ground, 26 geometries"]
fn stokes_radiances_match_natraj_2009_at_tau_one_over_a_bright_ground() {
    natraj(
        "Natraj et al. 2009, τ 1, μ₀ 0.6, A 0.8",
        1.0,
        0.6,
        0.8,
        &NATRAJ_2009_TAU_1,
        NATRAJ_2009_SIGNS,
        400_000,
    );
}

#[test]
#[ignore = "slow: Natraj and Hovenier 2012's τ 2 Rayleigh table, 20 geometries, 2.5 minutes"]
fn stokes_radiances_match_natraj_and_hovenier_2012_at_tau_two() {
    natraj(
        "Natraj and Hovenier 2012, τ 2, μ₀ 0.5, A 0.25",
        2.0,
        0.5,
        0.25,
        &NATRAJ_HOVENIER_2012_TAU_2,
        HOVENIER_SIGNS,
        1_500_000,
    );
}

/// A row of Kokhanovsky et al. 2010's molecular case: the viewing zenith angle from the outer
/// normal and the relative azimuth (degrees), then I, −Q and U, normalised to π I ÷ (μ₀F₀); U is
/// tabulated at 90° alone.
type KokhanovskyRow = (f64, f64, f64, f64, Option<f64>);

/// Kokhanovsky et al. 2010, the reflected light of Table A1 (I and −Q, three of its six viewing
/// angles) and the Rayleigh column of Table A4 (U at 90°): τ 0.3262, ω 1, θ₀ 60°, no
/// depolarisation, black ground.
const KOKHANOVSKY_REFLECTED: [KokhanovskyRow; 9] = [
    (0.0, 0.0, 0.143_398_1, 0.072_531_29, None),
    (0.0, 90.0, 0.143_398_1, -0.072_531_29, Some(0.0)),
    (0.0, 180.0, 0.143_398_1, 0.072_531_29, None),
    (40.0, 0.0, 0.156_051_7, 0.114_061_2, None),
    (40.0, 90.0, 0.174_639_7, -0.086_057_30, Some(0.073_220_31)),
    (40.0, 180.0, 0.268_231_7, 0.001_881_170, None),
    (89.0, 0.0, 0.858_131_9, 0.066_842_38, None),
    (89.0, 90.0, 0.545_870_6, -0.259_285_9, Some(0.391_710_5)),
    (89.0, 180.0, 0.871_804_5, 0.053_169_80, None),
];

/// The same for the transmitted light: Table A1 and the Rayleigh column of Table A5.
const KOKHANOVSKY_TRANSMITTED: [KokhanovskyRow; 9] = [
    (0.0, 0.0, 0.139_692_3, 0.070_406_28, None),
    (0.0, 90.0, 0.139_692_3, -0.070_406_28, Some(0.0)),
    (0.0, 180.0, 0.139_692_3, 0.070_406_28, None),
    (40.0, 0.0, 0.259_092_0, 0.001_549_839, None),
    (40.0, 90.0, 0.168_967_6, -0.082_952_91, Some(0.070_555_35)),
    (40.0, 180.0, 0.150_994_9, 0.109_646_9, None),
    (89.0, 0.0, 0.559_963_9, 0.023_242_66, None),
    (89.0, 90.0, 0.361_735_5, -0.166_642_1, Some(0.242_425_5)),
    (89.0, 180.0, 0.551_502_1, 0.031_704_48, None),
];

/// Kokhanovsky et al.'s stated accuracy, "correct at least for the first six digits" (Table A1's
/// caption), as a standard error relative to I.
const KOKHANOVSKY_ACCURACY: f64 = 1e-6;

/// Kokhanovsky et al.'s −Q, U and V against the tracer's: their Q is the tracer's, their U the
/// tracer's negative, and their V the tracer's (R08.T12.e). R08.T12.c took their V as the tracer's
/// negative, the one difference of handedness that IPRT's has; their aerosol's V table decided it
/// the other way (χ² 17.7 for 12 values against 3,488 with the sign flipped, and 10.9 for 10
/// against 3,452 once the N^(−1/2) check left two out, with T5.b's b₂), and SCIATRAN's own output
/// of the same case, which carries U as printed and V of the opposite sign to Table A6 at every
/// row (`SCIATRAN_BENCHMARK_RESULTS/aerosol_refl_N_240.dat` in the RTSOS repository, below), shows
/// why: SCIATRAN's V is the tracer's negative, as PSTAR's is, and the paper prints its negative
/// (`decision-r08-t4b-t12c.md`, follow-up, ruling 2).
const KOKHANOVSKY_SIGNS: Signs = Signs {
    q: -1.0,
    u: -1.0,
    v: 1.0,
};

#[test]
#[ignore = "slow: Kokhanovsky et al. 2010's molecular case, 18 geometries"]
fn stokes_radiances_match_kokhanovsky_2010_molecular_layer() {
    let (mu0, sun) = (0.5, (60.0, 0.0));
    let mut geometries = Vec::new();
    for &(vza, phi, ..) in &KOKHANOVSKY_REFLECTED {
        geometries.push(geometry(
            &format!("R {vza} {phi}"),
            Top,
            180.0 - vza,
            phi,
            sun,
        ));
    }
    for &(vza, phi, ..) in &KOKHANOVSKY_TRANSMITTED {
        geometries.push(geometry(&format!("T {vza} {phi}"), Bottom, vza, phi, sun));
    }
    let phase = rayleigh(0.0);
    let slab = Slab {
        name: "Kokhanovsky et al. 2010, molecular",
        tau: 0.3262,
        omega: 1.0,
        phase: &phase,
        albedo: 0.0,
        irradiance: 1.0,
        seed: 0,
    };
    let reference = trace(&slab.case(&geometries), 250_000);
    let mut comparisons = Comparisons::new(Convention {
        scale: PI / mu0,
        signs: KOKHANOVSKY_SIGNS,
    });
    let rows = KOKHANOVSKY_REFLECTED
        .iter()
        .map(|row| ("R", row))
        .chain(KOKHANOVSKY_TRANSMITTED.iter().map(|row| ("T", row)));
    for (g, (light, &(vza, phi, i, minus_q, u))) in rows.enumerate() {
        comparisons.push(
            &reference,
            g,
            [Some(i), Some(minus_q), u, None],
            [KOKHANOVSKY_ACCURACY * i; 4],
            &format!("{light} VZA {vza} φ {phi}"),
        );
    }
    assert_agreement("Kokhanovsky et al. 2010, molecular", &comparisons.values);
}

/// A sphere benchmark's scattering matrix, as the client's Mie-matrix writer wrote it (R08.T12.e,
/// `apps/hyperion/src/renderer/src/test/mieMatrices.ts`): its `tabulated` phase in the case
/// format, and the single-scattering albedo and asymmetry of the same sums over size.
#[derive(Debug, Clone, PartialEq)]
struct SphereTable {
    phase: Value,
    single_scattering_albedo: f64,
    asymmetry: f64,
}

/// The writer's files, by case: its vitest checks that each is what the client's Mie writes now.
const SPHERE_TABLES: [(&str, &str); 4] = [
    (
        "kokhanovsky-aerosol",
        include_str!("sphere_matrices/kokhanovsky-aerosol.json"),
    ),
    (
        "kokhanovsky-cloud",
        include_str!("sphere_matrices/kokhanovsky-cloud.json"),
    ),
    ("iprt-a3", include_str!("sphere_matrices/iprt-a3.json")),
    ("iprt-a5", include_str!("sphere_matrices/iprt-a5.json")),
];

/// The writer's table for case `name`.
#[must_use]
fn sphere_table(name: &str) -> SphereTable {
    let text = SPHERE_TABLES
        .iter()
        .find(|(case, _)| *case == name)
        .map(|&(_, text)| text)
        .expect("the writer writes every sphere case");
    let file: Value = serde_json::from_str(text).expect("the writer's file is JSON");
    assert_eq!(file["name"], name, "the file is the case's");
    let number = |key: &str| {
        file[key]
            .as_f64()
            .expect("the writer's file holds its optics")
    };
    SphereTable {
        phase: file["phase"].clone(),
        single_scattering_albedo: number("singleScatteringAlbedo"),
        asymmetry: number("asymmetry"),
    }
}

/// A row of Kokhanovsky et al.'s Tables A2 (aerosol) and A3 (cloud): the viewing zenith angle from
/// the outer normal and the relative azimuth (degrees), then I and −Q reflected and I and −Q
/// transmitted, normalised to π I ÷ (μ₀F₀); a value left out (`None`) is not committed
/// ([`KOKHANOVSKY_AEROSOL_HORIZON`], [`SUN_EXCLUSION_DEG`]).
type KokhanovskyLayerRow = (f64, f64, Option<f64>, Option<f64>, Option<f64>, Option<f64>);

/// Kokhanovsky et al. 2010, Table A2: the aerosol layer, τ 0.3262, ω 1, θ₀ 60°, black ground; the
/// values as printed (SCIATRAN's), less the nine left out: the seven of
/// [`KOKHANOVSKY_AEROSOL_HORIZON`] and the transmitted I and −Q at the sun (VZA 60°, φ 0°,
/// [`SUN_EXCLUSION_DEG`]).
#[expect(
    clippy::unreadable_literal,
    reason = "the values as the table prints them, to be checked digit by digit"
)]
const KOKHANOVSKY_AEROSOL: [KokhanovskyLayerRow; 18] = [
    (
        0.0,
        0.0,
        Some(0.1436885E-01),
        Some(0.1968219E-03),
        Some(0.6809063E-01),
        Some(-0.7518252E-02),
    ),
    (
        0.0,
        90.0,
        Some(0.1436885E-01),
        Some(-0.1968219E-03),
        Some(0.6809063E-01),
        Some(0.7518252E-02),
    ),
    (
        0.0,
        180.0,
        Some(0.1436885E-01),
        Some(0.1968219E-03),
        Some(0.6809063E-01),
        Some(-0.7518252E-02),
    ),
    (
        20.0,
        0.0,
        Some(0.2091866E-01),
        Some(-0.6995574E-03),
        Some(0.2068393E+00),
        Some(-0.1153345E-01),
    ),
    (
        20.0,
        90.0,
        Some(0.1592941E-01),
        Some(-0.3235966E-03),
        Some(0.6628790E-01),
        Some(0.6934062E-02),
    ),
    (
        20.0,
        180.0,
        Some(0.2438049E-01),
        Some(0.1556273E-02),
        Some(0.2959081E-01),
        Some(-0.3156305E-02),
    ),
    (
        40.0,
        0.0,
        Some(0.4936534E-01),
        Some(-0.5210978E-02),
        Some(0.7707225E+00),
        Some(-0.2496497E-02),
    ),
    (
        40.0,
        90.0,
        Some(0.2230096E-01),
        Some(0.5724128E-03),
        Some(0.6451028E-01),
        Some(0.5735927E-02),
    ),
    (
        40.0,
        180.0,
        Some(0.6320488E-01),
        Some(0.1077220E-01),
        Some(0.2285936E-01),
        Some(0.9314070E-04),
    ),
    (
        60.0,
        0.0,
        Some(0.1801996E+00),
        Some(-0.1575206E-01),
        None,
        None,
    ),
    (
        60.0,
        90.0,
        Some(0.4006522E-01),
        Some(-0.4402361E-03),
        Some(0.7393476E-01),
        Some(0.5195650E-02),
    ),
    (
        60.0,
        180.0,
        Some(0.1995878E+00),
        Some(0.1224099E-02),
        Some(0.3589802E-01),
        Some(0.2253185E-02),
    ),
    (
        80.0,
        0.0,
        Some(0.1009490E+01),
        Some(-0.4498639E-01),
        Some(0.2509366E+01),
        Some(-0.6418075E-02),
    ),
    (
        80.0,
        90.0,
        Some(0.1092879E+00),
        Some(0.4325278E-02),
        Some(0.1328353E+00),
        Some(0.7285748E-02),
    ),
    (
        80.0,
        180.0,
        Some(0.2390010E+00),
        Some(0.3671108E-01),
        Some(0.1357944E+00),
        Some(0.1182277E-01),
    ),
    (89.0, 0.0, None, None, None, Some(-0.3150199E-01)),
    (
        89.0,
        90.0,
        None,
        None,
        Some(0.1514427E+00),
        Some(0.7653060E-02),
    ),
    (
        89.0,
        180.0,
        None,
        Some(0.1085128E+00),
        None,
        Some(0.7152042E-01),
    ),
];

/// A row of Kokhanovsky et al.'s Tables A4–A6 at φ 90°: the viewing zenith angle (degrees), then U
/// and V reflected and U and V transmitted.
type KokhanovskyPerpendicularRow = (f64, f64, f64, f64, f64);

/// Kokhanovsky et al. 2010, the aerosol columns of Tables A4 (U reflected), A5 (U transmitted)
/// and A6 (V reflected and transmitted).
#[expect(
    clippy::unreadable_literal,
    reason = "the values as the tables print them, to be checked digit by digit"
)]
const KOKHANOVSKY_AEROSOL_PERPENDICULAR: [KokhanovskyPerpendicularRow; 6] = [
    (0.0, 0.0, 0.0, 0.0, 0.0),
    (
        20.0,
        0.6485331E-04,
        0.4181747E-05,
        -0.2740344E-02,
        -0.3480572E-05,
    ),
    (
        40.0,
        0.4139335E-03,
        0.6002111E-05,
        -0.4707951E-02,
        -0.3988220E-05,
    ),
    (
        60.0,
        -0.2534693E-03,
        0.1781059E-05,
        -0.6199702E-02,
        0.4991464E-05,
    ),
    (
        80.0,
        -0.5388941E-02,
        -0.4169772E-04,
        -0.9904365E-02,
        -0.2378265E-04,
    ),
    (
        89.0,
        -0.8434487E-02,
        -0.1045741E-03,
        -0.1046631E-01,
        -0.2380531E-04,
    ),
];

/// Kokhanovsky et al.'s stated spread for their aerosol: the largest relative difference of
/// Pstar's results from SCIATRAN's (the tables' values) in each of I, Q, U and V, relative to the
/// component itself (their Table 1 for I, Table 2 for Q and U, §5 for V). Table 2 leaves out the
/// values of Q and U near zero, and Table A2's caption claims only "at least the same two
/// significant figures in most cases", so a value's spread in Q or U is the larger of its own and
/// the spread in I as a fraction of I, `max(s_c·|c|, s_I·I)`: near zero the reference's accuracy is
/// the inter-code spread's absolute scale, which I's sets. V's stays relative to itself, since a
/// floor at `s_I·I` would exceed |V| (`decision-r08-t4b-t12c.md`, follow-up, ruling 4).
const KOKHANOVSKY_AEROSOL_SPREAD: [f64; 4] = [9e-4, 2e-3, 2e-3, 2e-3];

/// A detector of Kokhanovsky et al.'s tables: reflected ([`Top`]) or transmitted ([`Bottom`]), at
/// a viewing zenith angle from the outer (reflected) or inner (transmitted) normal and a relative
/// azimuth.
#[derive(Debug, Clone, Copy, PartialEq)]
struct View {
    level: Level,
    vza_deg: f64,
    phi_deg: f64,
}

impl View {
    /// Whether the view is at `vza_deg` and `phi_deg` and on `level`.
    #[must_use]
    fn is(self, level: Level, vza_deg: f64, phi_deg: f64) -> bool {
        self.level == level
            && (self.vza_deg - vza_deg).abs() < 1e-9
            && (self.phi_deg - phi_deg).abs() < 1e-9
    }
}

/// How near the sun a view of Kokhanovsky et al.'s spheres may look: IPRT's rows were chosen at
/// 15° or more from it (R08.T12.c), and Design note 10 compares no geometry within 3° of a sun. It
/// removes their one value at the sun itself (transmitted, VZA 60°, φ 0°), whose I and −Q are not
/// committed, and was fixed before either sphere case was traced (ruling 6). Its −Q also fails
/// [`KOKHANOVSKY_AEROSOL_HORIZON`]'s source rule: SCIATRAN's own output differs from it by 0.5%,
/// beyond Q's 0.2% (and its I by 0.046%).
const SUN_EXCLUSION_DEG: f64 = 15.0;

/// The sun's zenith angle in Kokhanovsky et al.'s cases.
const KOKHANOVSKY_SUN_ZENITH_DEG: f64 = 60.0;

/// The angle between a transmitted view and the direction towards a sun of zenith angle
/// `sun_zenith_deg` at relative azimuth 0, degrees.
#[must_use]
fn angle_from_sun_deg(view: View, sun_zenith_deg: f64) -> f64 {
    let (vza, sun, azimuth) = (
        view.vza_deg.to_radians(),
        sun_zenith_deg.to_radians(),
        view.phi_deg.to_radians(),
    );
    let cosine =
        math::cos(vza) * math::cos(sun) + math::sin(vza) * math::sin(sun) * math::cos(azimuth);
    math::acos(cosine.clamp(-1.0, 1.0)).to_degrees()
}

/// A layer of Kokhanovsky et al.'s spheres: its tables and the writer's matrix.
#[derive(Debug, Clone, Copy)]
struct KokhanovskyLayer<'a> {
    name: &'a str,
    table: &'a str,
    tau: f64,
    rows: &'a [KokhanovskyLayerRow],
    perpendicular: &'a [KokhanovskyPerpendicularRow],
    /// The reflected views whose printed −Q carries the wrong sign ([`KOKHANOVSKY_AEROSOL_ERRATA`]).
    errata: &'a [View],
    /// The transmitted views whose printed V carries the wrong sign ([`KOKHANOVSKY_CLOUD_ERRATA`]).
    circular_errata: &'a [View],
    /// The stated spread of each of I, Q, U and V at a view, relative to the component
    /// ([`KOKHANOVSKY_AEROSOL_SPREAD`], [`kokhanovsky_cloud_spread`]).
    spread: fn(View) -> [f64; 4],
    samples: u64,
}

/// Seven values of Kokhanovsky et al.'s VZA 89° row for the aerosol, left out (not committed) by a
/// source rule (`decision-r08-t4b-t12c.md`, follow-up, ruling 3): a printed value that differs
/// from the authors' own output of the same run by more than the paper's stated Pstar–SCIATRAN
/// spread (0.09% in I, 0.2% in Q and U) has no defined reference. Every other value of Table A2
/// but the one at the sun ([`SUN_EXCLUSION_DEG`]) equals SCIATRAN's own output of the case, to
/// 5 × 10⁻⁵ or better (the RTSOS copy of
/// [`KOKHANOVSKY_AEROSOL_ERRATA`]'s source, and `aerosol_trans_N_240.txt` beside it, SHA-256
/// `bca0734ad2a09b3846ce1f7b60f9f9ee1ebbe808ef407f2930b76e9d797c8e43`, for the transmitted light),
/// but at 89° the printed reflected I at φ 0°, 90° and 180° differs from it by −0.51%, −0.74% and
/// −0.59%, the transmitted I at φ 0° and 180° by −0.62% and −0.69%, and the reflected Q at φ 0° and
/// 90° by −0.45% and −0.75%. The paper is inconsistent there: its text gives SCIATRAN 240 streams
/// for the aerosol, Table A2's caption 480. The tracer, an independent method, matches SCIATRAN's
/// output at the five intensities to 0.007%–0.14% (pilot, 1.2 × 10⁶ samples), recorded but not the
/// criterion.
const KOKHANOVSKY_AEROSOL_HORIZON: [(View, Component); 7] = [
    (
        View {
            level: Top,
            vza_deg: 89.0,
            phi_deg: 0.0,
        },
        Component::I,
    ),
    (
        View {
            level: Top,
            vza_deg: 89.0,
            phi_deg: 90.0,
        },
        Component::I,
    ),
    (
        View {
            level: Top,
            vza_deg: 89.0,
            phi_deg: 180.0,
        },
        Component::I,
    ),
    (
        View {
            level: Bottom,
            vza_deg: 89.0,
            phi_deg: 0.0,
        },
        Component::I,
    ),
    (
        View {
            level: Bottom,
            vza_deg: 89.0,
            phi_deg: 180.0,
        },
        Component::I,
    ),
    (
        View {
            level: Top,
            vza_deg: 89.0,
            phi_deg: 0.0,
        },
        Component::Q,
    ),
    (
        View {
            level: Top,
            vza_deg: 89.0,
            phi_deg: 90.0,
        },
        Component::Q,
    ),
];

/// Two signs of Kokhanovsky et al.'s Table A2, the aerosol's reflected −Q at φ 90°, VZA 40° and
/// 60°, printed +0.5724128E−03 and −0.4402361E−03 (ruling 1). SCIATRAN's own output of the case
/// (240 streams in a hemisphere, the paper's Table A2 claiming 480; their values agree to the
/// seventh digit away from the horizon) carries the same magnitudes with the opposite signs, on a
/// curve that changes sign once, between 50° and 55°, where the printed column alternates at 20°,
/// 40°, 60° and 80°; the tracer gives −5.84 × 10⁻⁴ and +4.66 × 10⁻⁴, each within 1σ of the
/// corrected value and 64σ and 31σ from the printed one. The values are kept as printed and these
/// two negated. Source:
/// `validation/benchmark/Kokhanovsky_case/no_Ocean/Kok_data/SCIATRAN_BENCHMARK_RESULTS/aerosol_refl_N_240.dat`
/// of <https://github.com/aoog-umbc/rtsos> (the file as of commit 26def61e, at `main`'s 3a8610fc,
/// fetched 2026-10-10, SHA-256 `11b1f31c686a5b0359ae8342376e975ad8417f230ac1b3ca443e2e633a92c822`;
/// the repository states no licence), read only to check the printed signs; none of its values is
/// committed.
const KOKHANOVSKY_AEROSOL_ERRATA: [View; 2] = [
    View {
        level: Top,
        vza_deg: 40.0,
        phi_deg: 90.0,
    },
    View {
        level: Top,
        vza_deg: 60.0,
        phi_deg: 90.0,
    },
];

/// The case of a layer of Kokhanovsky et al.'s spheres, its draws from `seed`: every view of its
/// Table A2 or A3, reflected and then transmitted.
#[must_use]
fn kokhanovsky_case(layer: &KokhanovskyLayer<'_>, seed: u64) -> AtmosphereCase {
    let sun = (KOKHANOVSKY_SUN_ZENITH_DEG, 0.0);
    let sphere = sphere_table(layer.table);
    let mut geometries = Vec::new();
    for &(vza, phi, ..) in layer.rows {
        geometries.push(geometry(
            &format!("R {vza} {phi}"),
            Top,
            180.0 - vza,
            phi,
            sun,
        ));
    }
    for &(vza, phi, ..) in layer.rows {
        geometries.push(geometry(&format!("T {vza} {phi}"), Bottom, vza, phi, sun));
    }
    let slab = Slab {
        name: layer.name,
        tau: layer.tau,
        omega: sphere.single_scattering_albedo,
        phase: &sphere.phase,
        albedo: 0.0,
        irradiance: 1.0,
        seed,
    };
    slab.case(&geometries)
}

/// The comparisons of a layer of Kokhanovsky et al.'s spheres traced as `reference` (from
/// [`kokhanovsky_case`]): every committed value of its Table A2 or A3, reflected and transmitted,
/// with U and V at φ 90° from Tables A4–A6.
fn kokhanovsky_comparisons(layer: &KokhanovskyLayer<'_>, reference: &Traced) -> Vec<Comparison> {
    let mu0 = 0.5;
    let mut comparisons = Comparisons::new(Convention {
        scale: PI / mu0,
        signs: KOKHANOVSKY_SIGNS,
    });
    let perpendicular = |vza_deg: f64, phi_deg: f64| {
        let row = layer
            .perpendicular
            .iter()
            .find(|row| (row.0 - vza_deg).abs() < 1e-9)
            .copied()
            .expect("Tables A4-A6 give every viewing angle");
        if (phi_deg - 90.0).abs() < 1e-9 {
            ([Some(row.1), Some(row.2)], [Some(row.3), Some(row.4)])
        } else {
            ([None, None], [None, None])
        }
    };
    let rows = layer.rows.len();
    for (g, &(vza, phi, i_r, minus_q_r, i_t, minus_q_t)) in layer.rows.iter().enumerate() {
        let ([u_r, v_r], [u_t, v_t]) = perpendicular(vza, phi);
        let erratum = layer.errata.iter().any(|view| view.is(Top, vza, phi));
        let minus_q_r = minus_q_r.map(|q| if erratum { -q } else { q });
        let circular_erratum = layer
            .circular_errata
            .iter()
            .any(|view| view.is(Bottom, vza, phi));
        let v_t = v_t.map(|v| if circular_erratum { -v } else { v });
        for (index, level, expected) in [
            (g, Top, [i_r, minus_q_r, u_r, v_r]),
            (rows + g, Bottom, [i_t, minus_q_t, u_t, v_t]),
        ] {
            let spread = (layer.spread)(View {
                level,
                vza_deg: vza,
                phi_deg: phi,
            });
            // The floor of Q's and U's spread and the printed digits scale with I: the traced I,
            // since the printed one may be left out.
            let intensity = PI / mu0 * reference.full.geometries()[index].radiance()[0];
            let sigma = core::array::from_fn(|c| {
                let value = expected[c].unwrap_or(0.0);
                let relative = spread[c] * value.abs();
                // Near Q = 0 and U = 0 the reference's accuracy is the inter-code spread's absolute
                // scale, which I's sets (ruling 4).
                let spread = if c == Component::Q.index() || c == Component::U.index() {
                    relative.max(spread[Component::I.index()] * intensity)
                } else {
                    relative
                };
                math::hypot(spread, KOKHANOVSKY_ACCURACY * intensity)
            });
            let light = match level {
                Top => "R",
                Bottom => "T",
            };
            comparisons.push(
                reference,
                index,
                expected,
                sigma,
                &format!("{light} VZA {vza} φ {phi}"),
            );
        }
    }
    comparisons.values
}

/// Runs a layer of Kokhanovsky et al.'s spheres at its samples and seed 0, and asserts its
/// comparisons' agreement.
fn kokhanovsky_layer(layer: &KokhanovskyLayer<'_>) {
    let reference = trace(&kokhanovsky_case(layer, 0), layer.samples);
    assert_agreement(layer.name, &kokhanovsky_comparisons(layer, &reference));
}

#[test]
#[ignore = "slow: Kokhanovsky et al. 2010's aerosol layer, 36 geometries, two minutes"]
fn stokes_radiances_match_kokhanovsky_2010_aerosol_layer() {
    kokhanovsky_layer(&KokhanovskyLayer {
        name: "Kokhanovsky et al. 2010, aerosol",
        table: "kokhanovsky-aerosol",
        tau: 0.3262,
        rows: &KOKHANOVSKY_AEROSOL,
        perpendicular: &KOKHANOVSKY_AEROSOL_PERPENDICULAR,
        errata: &KOKHANOVSKY_AEROSOL_ERRATA,
        circular_errata: &[],
        spread: |_| KOKHANOVSKY_AEROSOL_SPREAD,
        samples: 1_200_000,
    });
}

/// Kokhanovsky et al. 2010, Table A3: the cloud layer, τ 5, ω 1, θ₀ 60°, black ground, its
/// matrix from the writer's Mie over their size distribution (a lognormal of a₀ 5 µm, σ 0.4, to
/// `a_end` 100 µm, m 1.339 at 412 nm); the values as printed (SCIATRAN's, 360 streams in a
/// hemisphere, which equal SCIATRAN's own output of the case to 10⁻⁶ everywhere, R08.T12.e), less
/// the transmitted I and −Q at the sun (VZA 60°, φ 0°, [`SUN_EXCLUSION_DEG`]). Their source rule
/// (ruling 3) leaves nothing of the cloud out.
#[expect(
    clippy::unreadable_literal,
    reason = "the values as the table prints them, to be checked digit by digit"
)]
const KOKHANOVSKY_CLOUD: [KokhanovskyLayerRow; 18] = [
    (
        0.0,
        0.0,
        Some(0.2230386E+00),
        Some(0.1458306E-01),
        Some(0.4626634E+00),
        Some(-0.1300066E-01),
    ),
    (
        0.0,
        90.0,
        Some(0.2230386E+00),
        Some(-0.1458306E-01),
        Some(0.4626634E+00),
        Some(0.1300066E-01),
    ),
    (
        0.0,
        180.0,
        Some(0.2230386E+00),
        Some(0.1458306E-01),
        Some(0.4626634E+00),
        Some(-0.1300066E-01),
    ),
    (
        20.0,
        0.0,
        Some(0.2619955E+00),
        Some(0.2192489E-02),
        Some(0.7871585E+00),
        Some(-0.1575456E-01),
    ),
    (
        20.0,
        90.0,
        Some(0.2416682E+00),
        Some(-0.1164763E-01),
        Some(0.4634229E+00),
        Some(0.1180246E-01),
    ),
    (
        20.0,
        180.0,
        Some(0.3301769E+00),
        Some(0.7511787E-01),
        Some(0.3147638E+00),
        Some(-0.3126918E-02),
    ),
    (
        40.0,
        0.0,
        Some(0.4587779E+00),
        Some(-0.9805261E-02),
        Some(0.1248475E+01),
        Some(-0.2553568E-02),
    ),
    (
        40.0,
        90.0,
        Some(0.3007135E+00),
        Some(-0.5759207E-02),
        Some(0.4646386E+00),
        Some(0.9685827E-02),
    ),
    (
        40.0,
        180.0,
        Some(0.3761572E+00),
        Some(0.2496259E-02),
        Some(0.2823214E+00),
        Some(0.6388771E-02),
    ),
    (
        60.0,
        0.0,
        Some(0.9851353E+00),
        Some(-0.3790092E-01),
        None,
        None,
    ),
    (
        60.0,
        90.0,
        Some(0.3921383E+00),
        Some(-0.2105265E-02),
        Some(0.4388206E+00),
        Some(0.8307814E-02),
    ),
    (
        60.0,
        180.0,
        Some(0.6797278E+00),
        Some(0.9801442E-03),
        Some(0.2889342E+00),
        Some(0.1341101E-01),
    ),
    (
        80.0,
        0.0,
        Some(0.2186265E+01),
        Some(-0.8132911E-01),
        Some(0.6816960E+00),
        Some(0.2234633E-02),
    ),
    (
        80.0,
        90.0,
        Some(0.4003249E+00),
        Some(-0.2089776E-02),
        Some(0.3124113E+00),
        Some(0.5702380E-02),
    ),
    (
        80.0,
        180.0,
        Some(0.5330462E+00),
        Some(-0.2216694E-02),
        Some(0.2304723E+00),
        Some(0.1257132E-01),
    ),
    (
        89.0,
        0.0,
        Some(0.2776746E+01),
        Some(-0.9160169E-01),
        Some(0.3971848E+00),
        Some(-0.6617358E-03),
    ),
    (
        89.0,
        90.0,
        Some(0.2728933E+00),
        Some(-0.4403001E-02),
        Some(0.1986522E+00),
        Some(0.3243696E-02),
    ),
    (
        89.0,
        180.0,
        Some(0.4380821E+00),
        Some(0.1232541E-01),
        Some(0.1522583E+00),
        Some(0.7785649E-02),
    ),
];

/// Kokhanovsky et al. 2010, the cloud columns of Tables A4 (U reflected), A5 (U transmitted) and
/// A6 (V reflected and transmitted), as printed.
#[expect(
    clippy::unreadable_literal,
    reason = "the values as the tables print them, to be checked digit by digit"
)]
const KOKHANOVSKY_CLOUD_PERPENDICULAR: [KokhanovskyPerpendicularRow; 6] = [
    (0.0, 0.0, 0.0, 0.0, 0.0),
    (
        20.0,
        0.4223818E-02,
        0.3947605E-06,
        -0.4523557E-02,
        0.4382299E-05,
    ),
    (
        40.0,
        0.4437616E-02,
        -0.1453557E-04,
        -0.6480793E-02,
        0.3722931E-05,
    ),
    (
        60.0,
        0.4442162E-02,
        -0.4132179E-04,
        -0.5535509E-02,
        0.5919457E-08,
    ),
    (
        80.0,
        0.5205060E-02,
        -0.4582790E-04,
        -0.2947084E-02,
        -0.5441439E-05,
    ),
    (
        89.0,
        0.6632575E-02,
        -0.3166770E-04,
        -0.1604392E-02,
        -0.7481497E-05,
    ),
];

/// Four signs of Kokhanovsky et al.'s Table A6, the cloud's transmitted V at VZA 20°, 40°, 80° and
/// 89° (φ 90°), negated in code as a cited erratum (`decision-r08-t4b-t12c.md`, follow-up, ruling
/// 2). SCIATRAN's own output of the case (the RTSOS copy, `cloud_trans_N_360.dat`, SHA-256
/// `6dc2059e…489`, read only; none of its values is committed) carries V of the opposite sign to
/// the printed one in every V column of the paper but these four entries, where it carries the
/// printed sign: the paper prints −(SCIATRAN's V) elsewhere, so these take the same convention.
/// The 60° entry, 5.9 × 10⁻⁹, a numerical zero, already has it. The cloud's V cannot decide its
/// sign by a run (|V| ≤ 1.2 × 10⁻⁴ of I against σ ≤ 3 × 10⁻³ of I), so it rests on the sources.
const KOKHANOVSKY_CLOUD_ERRATA: [View; 4] = [
    View {
        level: Bottom,
        vza_deg: 20.0,
        phi_deg: 90.0,
    },
    View {
        level: Bottom,
        vza_deg: 40.0,
        phi_deg: 90.0,
    },
    View {
        level: Bottom,
        vza_deg: 80.0,
        phi_deg: 90.0,
    },
    View {
        level: Bottom,
        vza_deg: 89.0,
        phi_deg: 90.0,
    },
];

/// Kokhanovsky et al.'s stated spread for their cloud at `view`, relative to each of I, Q, U and
/// V (`decision-r08-t4b-t12c.md`, follow-up, ruling 4): Pstar's largest difference from SCIATRAN.
/// I's is 0.2%, Table 1's comment ("for reflected light, and also for transmitted light and not
/// very oblique observation angles differences are below 0.2%"), and its 0.6% for the transmitted
/// views at VZA 80° and 89°; Q's 0.5% and U's 0.2% (Table 2), each floored at I's spread times I
/// as the aerosol's are; V's 0.5%, relative to itself (§5).
#[must_use]
fn kokhanovsky_cloud_spread(view: View) -> [f64; 4] {
    let intensity = if view.level == Bottom && view.vza_deg >= 80.0 {
        6e-3
    } else {
        2e-3
    };
    [intensity, 5e-3, 2e-3, 5e-3]
}

#[test]
#[ignore = "slow: Kokhanovsky et al. 2010's cloud layer, τ 5, 36 geometries, with VROOM"]
fn stokes_radiances_match_kokhanovsky_2010_cloud_layer() {
    kokhanovsky_layer(&KOKHANOVSKY_CLOUD_LAYER);
}

/// Kokhanovsky et al.'s cloud layer as its benchmark runs it.
const KOKHANOVSKY_CLOUD_LAYER: KokhanovskyLayer<'static> = KokhanovskyLayer {
    name: "Kokhanovsky et al. 2010, cloud",
    table: "kokhanovsky-cloud",
    tau: 5.0,
    rows: &KOKHANOVSKY_CLOUD,
    perpendicular: &KOKHANOVSKY_CLOUD_PERPENDICULAR,
    errata: &[],
    circular_errata: &KOKHANOVSKY_CLOUD_ERRATA,
    spread: kokhanovsky_cloud_spread,
    samples: 3_400_000,
};

#[test]
fn kokhanovsky_cloud_leaves_out_exactly_the_view_at_the_sun() {
    // The cloud's table matches SCIATRAN's own output everywhere, so only the transmitted I and
    // −Q of the views within 15° of the sun are left out (ruling 6).
    let mut empty = Vec::new();
    for &(vza_deg, phi_deg, i_r, minus_q_r, i_t, minus_q_t) in &KOKHANOVSKY_CLOUD {
        for (level, value) in [
            (Top, i_r),
            (Top, minus_q_r),
            (Bottom, i_t),
            (Bottom, minus_q_t),
        ] {
            if value.is_none() {
                empty.push((level, vza_deg, phi_deg));
            }
        }
        let near = angle_from_sun_deg(
            View {
                level: Bottom,
                vza_deg,
                phi_deg,
            },
            KOKHANOVSKY_SUN_ZENITH_DEG,
        ) < SUN_EXCLUSION_DEG;
        assert_eq!(i_t.is_none(), near, "VZA {vza_deg} φ {phi_deg}");
        assert_eq!(minus_q_t.is_none(), near, "VZA {vza_deg} φ {phi_deg}");
    }
    // The view's I and −Q, and nothing else.
    assert_eq!(empty.len(), 2);
    assert!(
        empty.iter().all(|&(level, vza, phi)| View {
            level: Bottom,
            vza_deg: 60.0,
            phi_deg: 0.0
        }
        .is(level, vza, phi)),
        "{empty:?}"
    );
}

#[test]
fn kokhanovsky_aerosol_leaves_out_exactly_the_ruled_values() {
    // The table's empty cells are exactly those the two rules name: the seven of the 89° row
    // (ruling 3), and the transmitted I and −Q of every view within 15° of the sun (ruling 6), so
    // that no value the benchmark does not assert is committed (`decision-r08-licences.md` row 6c).
    let mut empty = Vec::new();
    for &(vza_deg, phi_deg, i_r, minus_q_r, i_t, minus_q_t) in &KOKHANOVSKY_AEROSOL {
        for (level, component, value) in [
            (Top, Component::I, i_r),
            (Top, Component::Q, minus_q_r),
            (Bottom, Component::I, i_t),
            (Bottom, Component::Q, minus_q_t),
        ] {
            if value.is_none() {
                empty.push((
                    View {
                        level,
                        vza_deg,
                        phi_deg,
                    },
                    component,
                ));
            }
        }
    }
    let mut ruled: Vec<(View, Component)> = KOKHANOVSKY_AEROSOL_HORIZON.to_vec();
    for &(vza_deg, phi_deg, ..) in &KOKHANOVSKY_AEROSOL {
        let view = View {
            level: Bottom,
            vza_deg,
            phi_deg,
        };
        if angle_from_sun_deg(view, KOKHANOVSKY_SUN_ZENITH_DEG) < SUN_EXCLUSION_DEG {
            ruled.extend([(view, Component::I), (view, Component::Q)]);
        }
    }
    let order = |a: &(View, Component), b: &(View, Component)| {
        (a.0.level == Bottom)
            .cmp(&(b.0.level == Bottom))
            .then(a.0.vza_deg.total_cmp(&b.0.vza_deg))
            .then(a.0.phi_deg.total_cmp(&b.0.phi_deg))
            .then(a.1.cmp(&b.1))
    };
    empty.sort_by(order);
    ruled.sort_by(order);
    assert_eq!(empty, ruled);
    assert_eq!(empty.len(), 9);
}

/// A row of an IPRT case's selected results: depolarisation, altitude (0 the bottom, 1 the top),
/// sza, saa, vza, vaa, and I, Q, U and V ÷ F₀.
#[derive(Debug, Clone, Copy, PartialEq)]
struct IprtRow {
    depolarisation: f64,
    level: Level,
    sun: (f64, f64),
    view: (f64, f64),
    stokes: [f64; 4],
}

/// The text of `file` in the `iprt_phase_a` dataset, its SHA-256 checked against its
/// `PROVENANCE.toml`.
fn iprt_file(file: &str) -> String {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let dataset = load_dataset(&data, "iprt_phase_a", None).expect("the IPRT dataset loads");
    let (_, bytes) = dataset
        .files
        .into_iter()
        .find(|(name, _)| name == file)
        .expect("the IPRT dataset lists the file");
    String::from_utf8(bytes).expect("the IPRT files are ASCII")
}

/// The numbers of each line of `text` that is not a comment.
fn numeric_rows(text: &str) -> Vec<Vec<f64>> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            line.split_whitespace()
                .map(|w| w.parse().expect("an IPRT row holds numbers"))
                .collect()
        })
        .collect()
}

/// An IPRT case's selected rows.
fn iprt_rows(file: &str) -> Vec<IprtRow> {
    numeric_rows(&iprt_file(file))
        .into_iter()
        .map(|w| IprtRow {
            depolarisation: w[0],
            level: if w[1] > 0.5 { Top } else { Bottom },
            sun: (w[2], w[3]),
            view: (w[4], w[5]),
            stokes: [w[6], w[7], w[8], w[9]],
        })
        .collect()
}

/// The benchmark's own error in each of I, Q, U and V, relative to I: PSTAR's largest difference
/// from IPOL (whose U and V negated) at the case's rows, and in A4's V from MYSTIC, which agrees
/// with PSTAR there to 1.4 × 10⁻⁶ of I (measured on the fetched files, R08's Risks). A3's and
/// A5's are IPOL's (R08.T12.e; MYSTIC's noise is larger there, 2.5 × 10⁻³ of I in A3's I), A5's
/// over its principal plane and almucantar both (`iprt_phase_a/README.md`).
const IPRT_SPREAD: [(&str, [f64; 4]); 5] = [
    ("a1_rayleigh.txt", [1.1e-6, 7.7e-7, 1.1e-6, 1.1e-6]),
    ("a2_lambert.txt", [6.9e-6, 2.3e-6, 2.5e-6, 2.5e-6]),
    ("a4_spheroid.txt", [1.8e-4, 2.9e-6, 2.7e-6, 1.4e-6]),
    ("a3_sphere.txt", [9.7e-5, 4.7e-5, 1.1e-4, 3.6e-6]),
    ("a5_cloud.txt", [6.5e-4, 3.8e-4, 2.2e-5, 5.2e-7]),
];

/// PSTAR's U and V against the tracer's.
const IPRT_SIGNS: Signs = Signs {
    q: 1.0,
    u: -1.0,
    v: -1.0,
};

/// Whether a benchmark publishes V.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Circular {
    Published,
    NotPublished,
}

/// An IPRT case: its rows' file and the slab they were computed for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct IprtCase {
    name: &'static str,
    file: &'static str,
    tau: f64,
    omega: f64,
    albedo: f64,
    circular: Circular,
    samples: u64,
}

/// Runs an IPRT case, each row's phase from `phase(depolarisation)`.
fn iprt(case: IprtCase, phase: &dyn Fn(f64) -> Value) {
    let rows = iprt_rows(case.file);
    let spread = IPRT_SPREAD
        .iter()
        .find(|(f, _)| *f == case.file)
        .map(|&(_, s)| s)
        .expect("each IPRT case has its spread");
    // One slab per depolarisation, since the phase is the medium's, each with its own draws (its
    // index the seed), so that the geometries' values stay independent across slabs.
    let mut depolarisations: Vec<f64> = rows.iter().map(|r| r.depolarisation).collect();
    depolarisations.sort_by(f64::total_cmp);
    depolarisations.dedup();
    let mut comparisons = Comparisons::new(Convention {
        scale: 1.0,
        signs: IPRT_SIGNS,
    });
    for (seed, rho) in (0..).zip(depolarisations) {
        let chosen: Vec<&IprtRow> = rows
            .iter()
            .filter(|r| (r.depolarisation - rho).abs() < 1e-12)
            .collect();
        let geometries: Vec<Value> = chosen
            .iter()
            .enumerate()
            .map(|(i, r)| geometry(&format!("{i}"), r.level, r.view.0, r.view.1, r.sun))
            .collect();
        let phase = phase(rho);
        let slab = Slab {
            name: case.name,
            tau: case.tau,
            omega: case.omega,
            phase: &phase,
            albedo: case.albedo,
            irradiance: 1.0,
            seed,
        };
        let reference = trace(&slab.case(&geometries), case.samples);
        for (g, r) in chosen.iter().enumerate() {
            let [i, q, u, v] = r.stokes;
            let v = match case.circular {
                Circular::Published => Some(v),
                Circular::NotPublished => None,
            };
            comparisons.push(
                &reference,
                g,
                [Some(i), Some(q), Some(u), v],
                spread.map(|s| s * i),
                &format!(
                    "ρ {rho} {:?} sza {} saa {} vza {} vaa {}",
                    r.level, r.sun.0, r.sun.1, r.view.0, r.view.1
                ),
            );
        }
    }
    assert_agreement(case.name, &comparisons.values);
}

#[test]
#[ignore = "slow: IPRT Phase A case A1, Rayleigh at three depolarisations, 82 geometries"]
fn stokes_radiances_match_pstar_on_iprt_a1_rayleigh() {
    let case = IprtCase {
        name: "IPRT A1",
        file: "a1_rayleigh.txt",
        tau: 0.5,
        omega: 1.0,
        albedo: 0.0,
        circular: Circular::NotPublished,
        samples: 200_000,
    };
    iprt(case, &rayleigh);
}

#[test]
#[ignore = "slow: IPRT Phase A case A2, Rayleigh over a Lambertian ground, 33 geometries"]
fn stokes_radiances_match_pstar_on_iprt_a2_lambertian_surface() {
    let case = IprtCase {
        name: "IPRT A2",
        file: "a2_lambert.txt",
        tau: 0.1,
        omega: 1.0,
        albedo: 0.3,
        circular: Circular::NotPublished,
        samples: 100_000,
    };
    iprt(case, &rayleigh);
}

/// IPRT A4's spheroids: their single-scattering albedo and asymmetry parameter (the matrix file's
/// header and its first Legendre moment over 3).
const SPHEROID_OMEGA: f64 = 0.787_581;
const SPHEROID_ASYMMETRY: f64 = 2.515_05 / 3.0;

/// IPRT A4's matrix as a `tabulated` phase: its angles taken to u = √(Θ ÷ π), the elements over
/// 4π (P₁ averages 1 over the sphere), a₁ = P1, b₁ = P2, a₃ = P3, b₂ = P4, a₂ = P5, a₄ = P6.
fn spheroid_phase() -> Value {
    let rows = numeric_rows(&iprt_file("a4_spheroid_matrix.txt"));
    let u: Vec<f64> = rows.iter().map(|r| (r[0] / 180.0).sqrt()).collect();
    let element = |k: usize| vec![rows.iter().map(|r| r[k] / (4.0 * PI)).collect::<Vec<f64>>()];
    json!({
        "kind": "tabulated",
        "u": u,
        "a1": element(1),
        "asymmetry": [SPHEROID_ASYMMETRY],
        "matrix": {
            "a2": element(5), "a3": element(3), "a4": element(6),
            "b1": element(2), "b2": element(4)
        }
    })
}

#[test]
#[ignore = "slow: IPRT Phase A case A4, spheroids with IPRT's matrix, 34 geometries"]
fn stokes_radiances_match_pstar_on_iprt_a4_spheroids() {
    let phase = spheroid_phase();
    let case = IprtCase {
        name: "IPRT A4",
        file: "a4_spheroid.txt",
        tau: 0.2,
        omega: SPHEROID_OMEGA,
        albedo: 0.0,
        circular: Circular::Published,
        samples: 400_000,
    };
    iprt(case, &|_| phase.clone());
}

/// Runs one of IPRT's sphere cases with the writer's matrix of `table` (R08.T12.e).
fn iprt_sphere(name: &'static str, file: &'static str, table: &str, tau: f64, samples: u64) {
    let sphere = sphere_table(table);
    let case = IprtCase {
        name,
        file,
        tau,
        omega: sphere.single_scattering_albedo,
        albedo: 0.0,
        circular: Circular::Published,
        samples,
    };
    iprt(case, &|_| sphere.phase.clone());
}

#[test]
#[ignore = "slow: IPRT Phase A case A3, spheres with the client's Mie matrix, 34 geometries"]
fn stokes_radiances_match_pstar_on_iprt_a3_spheres() {
    iprt_sphere("IPRT A3", "a3_sphere.txt", "iprt-a3", 0.2, 400_000);
}

#[test]
#[ignore = "slow: IPRT Phase A case A5, a τ 5 water cloud, the client's Mie matrix, 62 geometries"]
fn stokes_radiances_match_pstar_on_iprt_a5_water_cloud() {
    iprt_sphere("IPRT A5", "a5_cloud.txt", "iprt-a5", 5.0, 1_700_000);
}

#[test]
fn sphere_matrix_tables_make_valid_cases_and_hold_the_tracers_normalisation() {
    // Every table the client's Mie-matrix writer wrote passes the case format's load checks (its
    // elements bounded by a₁, its integral and mean cosine within 10⁻⁴ of 1 and of the Mie
    // asymmetry), carries its whole matrix, and was normalised under the tracer's own linear-in-u
    // rule, so that its integral is 1 to its written digits.
    for (name, _) in SPHERE_TABLES {
        let sphere = sphere_table(name);
        assert!(
            sphere.single_scattering_albedo > 0.0 && sphere.single_scattering_albedo <= 1.0,
            "{name}"
        );
        let slab = Slab {
            name,
            tau: 1.0,
            omega: sphere.single_scattering_albedo,
            phase: &sphere.phase,
            albedo: 0.0,
            irradiance: 1.0,
            seed: 0,
        };
        let case = slab.case(&[geometry("g", Top, 180.0, 0.0, (40.0, 0.0))]);
        assert!(case.terms()[0].phase.has_matrix(), "{name}");
        let u: Vec<f64> = serde_json::from_value(sphere.phase["u"].clone()).unwrap();
        let a1: Vec<Vec<f64>> = serde_json::from_value(sphere.phase["a1"].clone()).unwrap();
        let table = super::optics::tabulated(&u, &a1[0], None, 0);
        assert!(
            (table.normalisation() - 1.0).abs() < 1e-9,
            "{name}: {}",
            table.normalisation()
        );
        assert!(
            (table.mean_cosine() - sphere.asymmetry).abs() < 1e-4,
            "{name}: {} against {}",
            table.mean_cosine(),
            sphere.asymmetry
        );
    }
}

#[test]
fn a_standard_error_scales_only_within_a_quarter_of_n_to_the_minus_half() {
    // At N ÷ 4 a σ of 2 predicts 1 at N: within 25% it scales, beyond it it does not, and a NaN
    // never does.
    assert!(sigma_scales(1.0, 2.0));
    assert!(sigma_scales(1.25, 2.0));
    assert!(sigma_scales(0.75, 2.0));
    assert!(!sigma_scales(1.26, 2.0));
    assert!(!sigma_scales(0.74, 2.0));
    assert!(!sigma_scales(2.0, 2.0));
    assert!(sigma_scales(0.0, 0.0));
    assert!(!sigma_scales(f64::NAN, 2.0));
    assert!(!sigma_scales(1.0, f64::NAN));
}

#[test]
fn iprt_files_load_against_their_checksums_and_make_valid_cases() {
    // The fast half of the benchmarks: the IPRT files load against their recorded SHA-256s and
    // parse, and the spheroids' matrix makes a valid case (its integral and mean cosine within
    // 10⁻⁴ of IPRT's, read linearly in u).
    assert_eq!(iprt_rows("a1_rayleigh.txt").len(), 82);
    assert_eq!(iprt_rows("a2_lambert.txt").len(), 33);
    assert_eq!(iprt_rows("a4_spheroid.txt").len(), 34);
    assert_eq!(iprt_rows("a3_sphere.txt").len(), 34);
    assert_eq!(iprt_rows("a5_cloud.txt").len(), 62);
    let phase = spheroid_phase();
    let slab = Slab {
        name: "spheroids",
        tau: 0.2,
        omega: SPHEROID_OMEGA,
        phase: &phase,
        albedo: 0.0,
        irradiance: 1.0,
        seed: 0,
    };
    let case = slab.case(&[geometry("g", Top, 180.0, 0.0, (40.0, 0.0))]);
    assert_eq!(case.terms().len(), 1);
    assert!(case.terms()[0].phase.has_matrix());
}

/// The sun's zenith angle in R08.T12.f's two checks of VROOM, IPRT A5's.
const VROOM_CHECK_SUN_ZENITH_DEG: f64 = 50.0;

/// The views of R08.T12.f's check of VROOM against the sunward draws: of 57 views at view zenith
/// 0°–85° and relative azimuth 0°–180°, every transmitted one at least [`SUN_EXCLUSION_DEG`] from
/// the sun, the 14 transmitted views where the sunward draws' σ was smallest in a pilot at other
/// seeds (0.09%–0.14% of I at 3.2 × 10⁶ samples), and the two reflected views where it was (0.16%
/// and 0.18%; R08's Risks, "Deviations in T12.f, as built"), so that the estimator the check holds
/// VROOM to converges there. The other reflected views of so forward a slab, dim, and its views far
/// from the sun in azimuth need 2–10 times the samples for the same σ.
const VROOM_CHECK_VIEWS: [(Level, f64, f64); 16] = [
    (Bottom, 0.0, 0.0),
    (Bottom, 10.0, 0.0),
    (Bottom, 10.0, 30.0),
    (Bottom, 20.0, 0.0),
    (Bottom, 20.0, 30.0),
    (Bottom, 20.0, 60.0),
    (Bottom, 30.0, 0.0),
    (Bottom, 30.0, 30.0),
    (Bottom, 40.0, 30.0),
    (Bottom, 40.0, 60.0),
    (Bottom, 60.0, 30.0),
    (Bottom, 70.0, 0.0),
    (Bottom, 70.0, 30.0),
    (Bottom, 80.0, 0.0),
    (Top, 80.0, 0.0),
    (Top, 85.0, 0.0),
];

#[test]
fn vroom_check_views_keep_away_from_the_sun() {
    // Every transmitted view of the check of VROOM against the sunward draws is at least 15°
    // from the sun, as Kokhanovsky et al.'s and IPRT's are, and none repeats.
    for (k, &(level, vza_deg, phi_deg)) in VROOM_CHECK_VIEWS.iter().enumerate() {
        let view = View {
            level,
            vza_deg,
            phi_deg,
        };
        let angle = angle_from_sun_deg(view, VROOM_CHECK_SUN_ZENITH_DEG);
        assert!(
            level == Top || angle >= SUN_EXCLUSION_DEG,
            "{view:?}: {angle}°"
        );
        assert!(
            VROOM_CHECK_VIEWS[..k]
                .iter()
                .all(|&(l, z, p)| !(view.is(l, z, p))),
            "{view:?} repeats"
        );
    }
}

/// The comparisons of R08.T12.f's check of VROOM against the sunward draws: a conservative slab of
/// τ 2 scattering by Henyey–Greenstein at g 0.9 (a table without polarisation, so that I alone is
/// compared), over a black ground, seen from `views`, traced with VROOM at `seeds[0]` and
/// `samples[0]`, and with R08.T12.c's sunward draws alone at `seeds[1]` and `samples[1]`. Each
/// value is VROOM's I against the sunward draws', with their two standard errors, and its σ is
/// taken to fall as N^(−1/2) only where both sides' do.
fn vroom_against_sunward_draws(
    views: &[(Level, f64, f64)],
    seeds: [u64; 2],
    samples: [u64; 2],
) -> Vec<Comparison> {
    let phase = super::case::tests::tabulated_phase_json(1024, 1, |c| {
        [
            super::optics::tests::henyey_greenstein(0.9, c),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ]
    });
    let sun = (VROOM_CHECK_SUN_ZENITH_DEG, 0.0);
    let geometries: Vec<Value> = views
        .iter()
        .map(|&(level, vza, phi)| {
            let zenith = if level == Top { 180.0 - vza } else { vza };
            geometry(&format!("{level:?} {vza} {phi}"), level, zenith, phi, sun)
        })
        .collect();
    let slab = |seed: u64| Slab {
        name: VROOM_CHECK,
        tau: 2.0,
        omega: 1.0,
        phase: &phase,
        albedo: 0.0,
        irradiance: 1.0,
        seed,
    };
    let on = trace_with(
        &slab(seeds[0]).case(&geometries),
        samples[0],
        Estimator::Vroom,
    );
    let off = trace_with(
        &slab(seeds[1]).case(&geometries),
        samples[1],
        Estimator::SunwardDraws,
    );
    views
        .iter()
        .enumerate()
        .map(|(g, &(level, vza, phi))| {
            let side = |traced: &Traced| {
                let (full, quarter) = (
                    &traced.full.geometries()[g],
                    &traced.quarter.geometries()[g],
                );
                let sigma = full.standard_error()[0];
                (
                    full.radiance()[0],
                    sigma,
                    sigma_scales(sigma, quarter.standard_error()[0]),
                )
            };
            let ((i_on, sigma_on, on_scales), (i_off, sigma_off, off_scales)) =
                (side(&on), side(&off));
            Comparison {
                label: format!("{level:?} VZA {vza} φ {phi}"),
                component: Component::I,
                traced: i_on,
                sigma: sigma_on,
                sigma_scales: on_scales && off_scales,
                expected: i_off,
                expected_sigma: sigma_off,
                intensity: i_off,
            }
        })
        .collect()
}

/// The name of R08.T12.f's check of VROOM against the sunward draws.
const VROOM_CHECK: &str = "VROOM against the sunward draws, Henyey–Greenstein g 0.9, τ 2";

#[test]
#[ignore = "slow: VROOM on and off in a Henyey–Greenstein slab of g 0.9 and τ 2, 16 geometries"]
fn vroom_agrees_with_the_sunward_draws_where_they_converge() {
    // R08.T12.f's check that the variance reduction is unbiased: the two estimators, each at its
    // own seed so that they are independent, held to each other by the agreement test, each
    // value's z = (on − off) ÷ √(σ_on² + σ_off²), with both σ at most 0.3% of I. The views are
    // those where the sunward draws converge best (`VROOM_CHECK_VIEWS`); a value whose σ still
    // does not fall as N^(−1/2) on either side is left out and named, and the set is pinned in
    // `EXPECTED_UNSCALED`.
    let values = vroom_against_sunward_draws(&VROOM_CHECK_VIEWS, [0, 1], VROOM_CHECK_SAMPLES);
    for c in values.iter().filter(|c| c.sigma_scales) {
        assert!(
            c.expected_sigma <= SIGMA_LIMIT * c.expected,
            "{VROOM_CHECK}, {}: the sunward draws' σ {} is above 0.3% of I = {}",
            c.label,
            c.expected_sigma,
            c.expected
        );
    }
    assert_agreement(VROOM_CHECK, &values);
}

/// The samples a geometry of R08.T12.f's check of VROOM against the sunward draws, VROOM's and
/// the sunward draws': set from the pilot (R08's Risks), so that each σ is some 0.25% of I at most.
const VROOM_CHECK_SAMPLES: [u64; 2] = [700_000, 1_600_000];

/// One balance of R08.T12.f's energy check: the plane albedo, the diffuse transmission and the
/// direct beam against the incident flux, with the standard error of the two traced terms and
/// whether both fall as N^(−1/2).
#[derive(Debug, Clone, PartialEq)]
struct Balance {
    name: String,
    total: f64,
    incident: f64,
    sigma: f64,
    sigma_scales: bool,
}

/// The balances of R08.T12.f's energy check: a slab of τ 5 scattering by IPRT A5's cloud matrix
/// (the writer's) with ω set to 1, over a black ground, its fluxes' hemispheres traced in the
/// Stokes mode at `seed` and `samples` with the suns 0°, 50° and 75° from the zenith; the vector
/// I and the scalar radiance of the same paths each balanced.
fn energy_balances(seed: u64, samples: u64) -> Vec<Balance> {
    let sphere = sphere_table("iprt-a5");
    let aggregates: Vec<Value> = [0.0, VROOM_CHECK_SUN_ZENITH_DEG, 75.0]
        .iter()
        .map(|&zenith| {
            json!({
                "name": format!("sun {zenith}"),
                "sunDirections": [{ "zenithDeg": zenith, "azimuthDeg": 0.0 }]
            })
        })
        .collect();
    let case = json!({
        "format": 1,
        "name": "A5's matrix, ω 1",
        "seed": seed,
        "wavelengthsNm": [800],
        "shells": { "kind": "sphere", "radiusM": 1e12 },
        "topHeightM": SLAB_M,
        "groundAlbedo": [0.0],
        "terms": [{
            "name": "cloud",
            "density": {
                "kind": "tabulated", "altitudesM": [0.0, SLAB_M], "relative": [1.0, 1.0]
            },
            "scattering": [5.0 / SLAB_M],
            "absorption": [0.0],
            "phase": sphere.phase
        }],
        "suns": [{ "name": "sun", "irradiance": [1.0] }],
        "detectorHalfAngleDeg": 0.0,
        "geometries": [],
        "aggregates": aggregates
    });
    let case = AtmosphereCase::from_json(&case.to_string()).expect("the slab is a valid case");
    let traced = trace_with(&case, samples, Estimator::ByMedium);
    let mut balances = Vec::new();
    for (aggregate, quarter) in traced
        .full
        .aggregates()
        .iter()
        .zip(traced.quarter.aggregates())
    {
        let scalar = aggregate
            .scalar()
            .expect("the Stokes mode scores the scalar fluxes");
        let quarter_scalar = quarter
            .scalar()
            .expect("the Stokes mode scores the scalar fluxes");
        for (label, [up, up_sigma, down, down_sigma], [up_quarter, down_quarter]) in [
            (
                "vector I",
                [
                    aggregate.upwelling_top()[0],
                    aggregate.upwelling_top_standard_error()[0],
                    aggregate.diffuse_ground()[0],
                    aggregate.diffuse_ground_standard_error()[0],
                ],
                [
                    quarter.upwelling_top_standard_error()[0],
                    quarter.diffuse_ground_standard_error()[0],
                ],
            ),
            (
                "scalar",
                [
                    scalar.upwelling_top()[0],
                    scalar.upwelling_top_standard_error()[0],
                    scalar.diffuse_ground()[0],
                    scalar.diffuse_ground_standard_error()[0],
                ],
                [
                    quarter_scalar.upwelling_top_standard_error()[0],
                    quarter_scalar.diffuse_ground_standard_error()[0],
                ],
            ),
        ] {
            balances.push(Balance {
                name: format!("{}, {label}", aggregate.name()),
                total: up + aggregate.direct_ground()[0] + down,
                incident: aggregate.incident_top()[0],
                sigma: math::hypot(up_sigma, down_sigma),
                sigma_scales: sigma_scales(up_sigma, up_quarter)
                    && sigma_scales(down_sigma, down_quarter),
            });
        }
    }
    balances
}

#[test]
#[ignore = "slow: energy conservation with VROOM in a conservative τ 5 slab of IPRT A5's matrix"]
fn vroom_conserves_energy_in_a_conservative_slab_of_the_a5_cloud() {
    // R08.T12.f: the plane albedo, the diffuse transmission and the direct beam add to the
    // incident flux for ω = 1. The fluxes' hemispheres are traced with VROOM, which the matrix's
    // forward peak turns on, and the balances are held to the agreement test: each to the
    // Bonferroni bound z_N over the balances asserted (3.93 for six; too few for a χ²), each σ at
    // most 0.3% of the incident flux. A balance whose σ does not fall as N^(−1/2) is left out and
    // named, as a benchmark's value is, and the set is pinned in `EXPECTED_UNSCALED`.
    let balances = energy_balances(0, ENERGY_CHECK_SAMPLES);
    assert_eq!(balances.len(), 6);
    let (asserted, unscaled): (Vec<&Balance>, Vec<&Balance>) =
        balances.iter().partition(|b| b.sigma_scales);
    let mut left_out: Vec<String> = unscaled.iter().map(|b| b.name.clone()).collect();
    left_out.sort();
    assert!(
        !asserted.is_empty(),
        "{ENERGY_CHECK}: no balance's σ falls as N^(−1/2)"
    );
    let count = u32::try_from(asserted.len()).expect("six balances at most");
    let bound = -math::normal_quantile(ALPHA / (4.0 * f64::from(count)));
    for b in asserted {
        let z = (b.total - b.incident) / b.sigma;
        eprintln!(
            "{ENERGY_CHECK}, {}: {:.6} against {:.6} ± {:.2e}, z {z:+.2}, bound {bound:.2}",
            b.name, b.total, b.incident, b.sigma
        );
        assert!(
            b.sigma <= SIGMA_LIMIT * b.incident,
            "{}: σ {} above 0.3% of the incident {}",
            b.name,
            b.sigma,
            b.incident
        );
        assert!(z.abs() <= bound, "{}: z = {z:.2}, past {bound:.2}", b.name);
    }
    eprintln!("{ENERGY_CHECK}: left out, their σ not falling as N^(−1/2): {left_out:?}");
    assert_unscaled_as_recorded(ENERGY_CHECK, &left_out);
}

/// The name of R08.T12.f's energy check.
const ENERGY_CHECK: &str = "VROOM's energy check, A5's matrix at ω 1";

/// The samples each flux of R08.T12.f's energy check takes: set from a pilot at another seed (σ
/// at most 0.49% of the incident flux at 4 × 10⁵ samples, the sun overhead; R08's Risks), so that
/// each σ is some 0.25% at most.
const ENERGY_CHECK_SAMPLES: u64 = 1_500_000;
