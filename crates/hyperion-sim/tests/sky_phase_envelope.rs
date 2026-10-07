//! The phase envelope against dense tracks (rendering plan R06, R06.T8.m). Slow.
//!
//! - `phase_envelope_bounds_dense_tracks`: over 10⁵ single stars of random initial mass
//!   (log-uniform over 0.0124–150 M☉), \[Fe/H\] (−2.6 to +0.4, past both of the tracks' clamps),
//!   η draw (−7.5 to +7σ, past the η = 0 extreme) and every other draw a realised star has, each at
//!   ages across its life, no star is brighter in V than the checked-in table,
//!   `tables::sky_phase_envelope`, read through `PhaseEnvelope::fitted`: its margin included. A
//!   third of each star's ages are log-uniform from 10⁴ years to 1.5 × 10¹⁰ years, and two thirds
//!   uniform in relative age over the fine bins, 0.8–1.05 of the table's lifetime, where every
//!   giant phase lies, wherever the star reaches them by 1.5 × 10¹⁰ years. It prints the most
//!   margin any star used and how many checks used any, which R06's Risks record. Run once on an
//!   independent seed (2026-10-07, `0x7a3_1f00_5eed_0002`, stars' seeds offset by 9 × 10⁶), it used
//!   at most 0.006 mag; this seed's 0.068 is in-sample, the seed whose first run set the spreads.
//!
//! The draws other than η are a realised star's (`StarDraws::for_star` on a seed of each star's
//! own), so the companion-stripped mark, which moves a single star's fate in the electron-capture
//! window, is held to the table too, though the table samples the median draws but η.

use std::collections::BTreeMap;

use hyperion_sim::Seed;
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::math;
use hyperion_sim::sky::envelope::{MARGIN_MAG, MAX_AGE_YEARS};
use hyperion_sim::sky::phase::{FINE_BIN_WIDTH, FINE_BINS, FINE_START, PhaseEnvelope};
use hyperion_sim::sky::photometry::absolute_v_of_state;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use hyperion_sim::stellar::sse::{MIN_INITIAL_MASS, Track};
use hyperion_sim::stellar::substellar;
use hyperion_sim::units::{Dex, HeliumExcess, Magnitudes, SolarMasses, Years};
use hyperion_testkit::lcg::Lcg;

/// Single stars checked.
const STARS: u32 = 100_000;

/// Ages a star is checked at: a third log-uniform over its life, two thirds in the fine bins.
const AGES: u32 = 12;

/// The least and greatest initial mass drawn, M☉: the brightness envelope's mass nodes' span.
const MASSES: (f64, f64) = (0.012_409_725_041_601_22, 150.0);

/// What the run saw, and every star brighter than the table.
#[derive(Debug, Default)]
struct Tally {
    checks: u64,
    shining: u64,
    fine: u64,
    using_margin: u64,
    worst_use: Option<(f64, String)>,
    violations: BTreeMap<String, (u64, f64, String)>,
}

impl Tally {
    /// Holds V `v` (absolute, or `None` if dark) of the star `what`, in phase `phase`, to the
    /// table's `bound` at its age.
    fn hold(&mut self, v: Option<Magnitudes>, bound: Option<Magnitudes>, what: &str, phase: &str) {
        self.checks += 1;
        let Some(v) = v else {
            return;
        };
        self.shining += 1;
        let (v, bound) = (v.value(), bound.map_or(f64::INFINITY, Magnitudes::value));
        // How much of the margin the star uses: positive where it is brighter than the table
        // without its margin.
        let used = bound + MARGIN_MAG - v;
        self.using_margin += u64::from(used > 0.0);
        if self.worst_use.as_ref().is_none_or(|(w, _)| used > *w) {
            self.worst_use = Some((used, format!("{what}: V {v:.4} against {bound:.4}")));
        }
        if v < bound {
            let entry = self.violations.entry(phase.to_owned()).or_insert((
                0,
                f64::NEG_INFINITY,
                String::new(),
            ));
            entry.0 += 1;
            if used > entry.1 {
                entry.1 = used;
                entry.2 = format!("{what}: V {v:.4} against {bound:.4}");
            }
        }
    }
}

#[test]
#[ignore = "slow: builds 10^5 tracks"]
fn phase_envelope_bounds_dense_tracks() {
    let table = PhaseEnvelope::fitted();
    let system = SystemId::from_raw(0x4204_6c99_ff00_000a).expect("a grid system's id");
    let mut lcg = Lcg::new(0x5e6_0008_d000_0001);
    let mut tally = Tally::default();
    let (ln_lo, ln_hi) = (math::ln(MASSES.0), math::ln(MASSES.1));
    #[expect(clippy::cast_precision_loss, reason = "250 bins")]
    let fine_end = FINE_START + FINE_BIN_WIDTH * FINE_BINS as f64;
    for star in 0..STARS {
        let m = math::exp(ln_lo + (ln_hi - ln_lo) * lcg.next_f64()).min(MASSES.1);
        let fe_h = -2.6 + 3.0 * lcg.next_f64();
        let eta = StandardNormal::new(-7.5 + 14.5 * lcg.next_f64()).expect("finite");
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let realised = StarDraws::for_star(Seed::new(u64::from(star)), BodyId::new(system, 0));
        let draws = StarDraws::from_parts(StarDrawsParts {
            eta,
            ..realised.parts().clone()
        });
        let lifetime = table
            .lifetime(SolarMasses::new(m), &composition, eta)
            .value();
        let ages: Vec<f64> = (0..AGES)
            .map(|k| {
                let in_fine = k % 3 != 0 && FINE_START * lifetime < MAX_AGE_YEARS;
                if in_fine {
                    let r = FINE_START + (fine_end - FINE_START) * lcg.next_f64();
                    (r * lifetime).min(MAX_AGE_YEARS)
                } else {
                    math::exp(
                        math::ln(1e4) + (math::ln(MAX_AGE_YEARS) - math::ln(1e4)) * lcg.next_f64(),
                    )
                }
            })
            .collect();
        let oldest = ages.iter().copied().fold(0.0, f64::max);
        let track = (m >= MIN_INITIAL_MASS.value()).then(|| {
            Track::to_age(
                SolarMasses::new(m),
                &composition,
                &draws,
                Years::new(oldest),
            )
        });
        for &age in &ages {
            let state = match &track {
                Some(track) => track.state_at(Years::new(age)),
                None => substellar::cooling(SolarMasses::new(m), Years::new(age), &composition)
                    .expect("a brown dwarf of the envelope's span is inside the fits"),
            };
            let bound = table.brightest(
                SolarMasses::new(m),
                &composition,
                eta,
                (Years::new(age), Years::new(age)),
            );
            let r = age / lifetime;
            tally.fine += u64::from((FINE_START..fine_end).contains(&r));
            let what = format!(
                "star {star}: m {m:.5} [Fe/H] {fe_h:.3} η draw {:.3} age {age:.6e} (relative \
                 {r:.5}) {:?}",
                eta.value(),
                state.phase()
            );
            tally.hold(
                absolute_v_of_state(&state),
                bound,
                &what,
                &format!("{:?}", state.phase()),
            );
        }
    }
    let (worst, at) = tally.worst_use.clone().unwrap_or_default();
    eprintln!(
        "{} checks of {STARS} stars, {} shining, {} in the fine bins, {} using any of the \
         margin; the most margin used: {worst:.4} mag of the {MARGIN_MAG} mag margin ({at}); {} \
         phases with violations",
        tally.checks,
        tally.shining,
        tally.fine,
        tally.using_margin,
        tally.violations.len()
    );
    assert!(tally.violations.is_empty(), "{:#?}", tally.violations);
}
