//! The brightness envelope against dense tracks, and the premise of the census's n × F₁ bound
//! (rendering plan R06, R06.T6.b; Design notes 8 and 10). Slow.
//!
//! - `envelope_bounds_dense_tracks`: for 10⁴ masses drawn in each layer's band, at a metallicity
//!   and a Reimers η drawn across the envelope's span (\[Fe/H\] −2.3 to +0.4, η within ±3.5σ), and
//!   at ages drawn across 10⁴ years to 13.5 Gyr, no star is brighter in V than the envelope, and
//!   none uses more than 0.1 mag of its 0.3 mag margin.
//!
//! The plan's second test, that V never falls with mass along the early phases at a fixed age (the
//! premise of Design note 10's n × F₁ bound), was dropped (decided 2026-10-03): the premise is false
//! in V by up to at least 0.38 mag on the pre-main sequence, and since the multiple-system ruling
//! (2026-10-02, item 2) n × F₁ is used only for single stars, where it is the star's own flux.

use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::sky::envelope::{BrightnessEnvelope, MARGIN_MAG};
use hyperion_sim::sky::photometry::absolute_v_of_state;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use hyperion_sim::stellar::sse::{MIN_INITIAL_MASS, Track};
use hyperion_sim::stellar::substellar;
use hyperion_sim::units::{Dex, HeliumExcess, Magnitudes, SolarMasses, Years};
use hyperion_testkit::lcg::Lcg;

/// Masses per layer.
const MASSES: u32 = 10_000;

/// Ages sampled per track.
const AGES: u32 = 24;

#[test]
#[ignore = "slow: builds 5 × 10^4 tracks"]
fn envelope_bounds_dense_tracks() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let envelope = BrightnessEnvelope::build(&galaxy);
    let component = galaxy.fields().component_ids().next().expect("a component");
    let mut lcg = Lcg::new(0x5e6_0006_b000_0001);
    let mut worst_use = f64::NEG_INFINITY;
    let mut violations: std::collections::BTreeMap<String, (u32, f64, String)> =
        std::collections::BTreeMap::new();
    for band in [
        MassBand::BrownDwarf,
        MassBand::A,
        MassBand::B,
        MassBand::C,
        MassBand::D,
        MassBand::E,
    ] {
        let layer = Layer::from(band);
        let (ln_lo, ln_hi) = (math::ln(band.lo()), math::ln(band.hi()));
        for _ in 0..MASSES {
            let m = math::exp(ln_lo + (ln_hi - ln_lo) * lcg.next_f64());
            let fe_h = -2.3 + 2.7 * lcg.next_f64();
            // From η = 0, no Reimers wind, the physical extreme, to +7σ.
            let eta = -0.5 / 0.07 + (7.0 + 0.5 / 0.07) * lcg.next_f64();
            let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            let ages: Vec<f64> = (0..AGES)
                .map(|_| {
                    math::exp(math::ln(1e4) + (math::ln(1.35e10) - math::ln(1e4)) * lcg.next_f64())
                })
                .collect();
            let states: Vec<_> = if m < MIN_INITIAL_MASS.value() {
                ages.iter()
                    .map(|&a| {
                        substellar::cooling(SolarMasses::new(m), Years::new(a), &composition)
                            .expect("inside the fits")
                    })
                    .collect()
            } else {
                let draws = StarDraws::from_parts(StarDrawsParts {
                    eta: StandardNormal::new(eta).expect("finite"),
                    ..StarDrawsParts::MEDIAN
                });
                let track = Track::to_age(
                    SolarMasses::new(m),
                    &composition,
                    &draws,
                    Years::new(1.35e10),
                );
                ages.iter()
                    .map(|&a| track.state_at(Years::new(a)))
                    .collect()
            };
            for (&age, state) in ages.iter().zip(&states) {
                let Some(v) = absolute_v_of_state(state) else {
                    continue;
                };
                let bound = envelope
                    .brightest(
                        layer,
                        component,
                        SolarMasses::new(m),
                        (Years::new(age), Years::new(age)),
                    )
                    .map_or(f64::INFINITY, Magnitudes::value);
                // How much of the margin the star uses: positive where it is brighter than the
                // envelope without its margin.
                let used = (bound + MARGIN_MAG) - v.value();
                worst_use = worst_use.max(used);
                if v.value() < bound || used > 0.1 {
                    let entry = violations
                        .entry(format!("{band:?} {:?}", state.phase()))
                        .or_insert((0, f64::NEG_INFINITY, String::new()));
                    entry.0 += 1;
                    if used > entry.1 {
                        entry.1 = used;
                        entry.2 = format!(
                            "m {m} [Fe/H] {fe_h:.2} η {eta:.2} age {age:.4e}: V {} against {bound}",
                            v.value()
                        );
                    }
                }
            }
        }
    }
    eprintln!(
        "the most margin used: {worst_use:.4} mag; {} violations",
        violations.len()
    );
    assert!(violations.is_empty(), "{violations:#?}");
}
