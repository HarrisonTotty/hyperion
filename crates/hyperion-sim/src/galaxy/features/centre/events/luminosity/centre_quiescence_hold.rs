//! The tests of the centre's quiescence, the hot flow's efficiency from generator version 25
//! (plan 09, P09.T30.c; decision-galaxy-25-train §1.5 and §1.6): the hot flow moving the centre's
//! luminosity off η at every rate and nothing else (D2, with the census C1 of what moves), its law
//! beyond the golden, the same seed twice and in any order (D4), and Ponti et al.'s limit at the
//! epoch over 200 seeds.
//!
//! The version's light is pinned by the goldens, `centre_luminosity` and `centre_victims` among
//! them, and by D2's digests. Where a test needs the light a tail would give at η whatever its
//! rate, P09.T30.b's law, it computes it directly (`eta_luminosity_watts_at`).

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::order::assert_order_independent;

use super::tests::{
    Entry, WATTS_PER_ERG_S, centre_luminosity_entries, eta_luminosity_watts_at,
    eta_peak_luminosity_watts, swept_victims, years_after,
};
use super::*;
use crate::Seed;
use crate::galaxy::features::centre::testing::{milky_way_centre, milky_way_galaxy};
use crate::galaxy::params::GalaxyParams;
use crate::time::Span;

/// Ponti et al.'s (2010, ApJ 714, 732) limit on Sgr A*'s mean luminosity over the last 60–90
/// years, 10³⁶ erg/s.
const PONTI_LIMIT_ERG_S: f64 = 1e36;

/// The digest (`f64_digest`) of the `centre_luminosity` golden's values, in the golden's order.
/// Blessed natively on x86-64; the wasip1 suite runs the test that pins it. A later change that
/// moves the centre's luminosity re-pins it by name (decision-galaxy-25-train §1.3): re-pinned in
/// `MGE_BOXY`'s potential (P15.T3.c; it was `0xacdc_f23d_013f_d38e`).
const HOT_FLOW_DIGEST: u64 = 0x178e_a6fa_bfab_be18;

/// The digest of the hot flow's law where the golden does not reach it: `radiative_efficiency`
/// inside each of its three ranges and above the switch to η, and a tail whose peak is below the
/// switch (a 1 M☉, 10 R☉ star at 10⁸ M☉) at its peak and ten fallback times on. Blessed natively
/// on x86-64, as [`HOT_FLOW_DIGEST`].
const HOT_FLOW_LAW_DIGEST: u64 = 0x7fe1_828b_a0fa_d5a7;

/// D2 and the census of what moves (decision-galaxy-25-train §1.5–§1.6), the hot flow against η
/// at every rate ([`eta_luminosity_watts_at`]): every tail of the fixture peaks as it would under
/// η (every peak at the Milky Way's black hole is above the hot flow's switch), and of the
/// `centre_luminosity` golden's entries only the Sun-like tail's after its first 25 years and the
/// tails' and totals move, each finite and fainter. The moved figures are printed for the plan's
/// records and the version's values pinned by their digest.
#[test]
fn the_hot_flow_moves_only_the_centre_s_luminosity_pinned() {
    let galaxy = milky_way_galaxy();
    let victims = CentreVictims::new(galaxy, milky_way_centre()).unwrap();
    for victim in victims.victims() {
        let tail = victim.tail().unwrap();
        assert_same_bits(
            tail.peak_luminosity_watts(),
            eta_peak_luminosity_watts(tail),
        );
    }

    let (old, new) = (
        centre_luminosity_entries(galaxy, eta_luminosity_watts_at),
        centre_luminosity_entries(galaxy, DisruptionTail::luminosity_watts_at),
    );
    assert_eq!(old.len(), new.len());
    let mut at = String::new();
    let mut moved = Vec::new();
    let mut values = Vec::new();
    for (a, b) in old.iter().zip(&new) {
        match (a, b) {
            (Entry::Line(x), Entry::Line(y)) => {
                assert_eq!(x, y);
                at.clone_from(x);
            }
            (Entry::Value(label, x), Entry::Value(same, y)) => {
                assert_eq!(label, same);
                assert!(y.is_finite() && *y >= 0.0, "{at} {label}: {y:e}");
                values.push(*y);
                if x.total_cmp(y).is_ne() {
                    assert!(y < x, "{at} {label}: {x:e} W, then {y:e} W");
                    let place = if at.is_empty() {
                        label.trim().to_owned()
                    } else {
                        format!("{at}: {}", label.trim())
                    };
                    moved.push((place, *x, *y));
                }
            }
            _ => panic!("the entries line up: {a:?} and {b:?}"),
        }
    }
    for (label, x, y) in &moved {
        eprintln!(
            "moved: {label}: {:.4e} -> {:.4e} erg/s (x{:.4})",
            x / WATTS_PER_ERG_S,
            y / WATTS_PER_ERG_S,
            y / x
        );
    }
    let digest = f64_digest(&values);
    eprintln!(
        "{} of {} values moved; the hot flow's digest {digest:#018x}",
        moved.len(),
        values.len()
    );
    let sun_like: Vec<&str> = moved
        .iter()
        .filter(|(label, ..)| label.contains("Sun-like"))
        .map(|(label, ..)| label.as_str())
        .collect();
    assert_eq!(
        sun_like,
        [
            "a Sun-like tail at 100 yr (W)",
            "a Sun-like tail at 1000 yr (W)",
            "a Sun-like tail at 100000 yr (W)"
        ]
    );
    assert!(moved.iter().all(|(label, ..)| {
        label.contains("Sun-like") || label.ends_with("tails (W)") || label.ends_with("total (W)")
    }));
    // As measured: the three Sun-like values, and the tails and totals at all fifteen instants.
    assert_eq!((moved.len(), values.len()), (33, 52));
    assert_eq!(digest, HOT_FLOW_DIGEST);
}

/// D2's second pin, where the golden does not reach the hot flow's law: `radiative_efficiency`
/// inside each of its three ranges and above the switch to η, and a tail whose peak is below the
/// switch at its peak and ten fallback times on, by their digest (blessed natively on x86-64, and
/// run in the wasip1 suite).
#[test]
fn the_hot_flow_s_law_beyond_the_golden_is_pinned() {
    let low_peak = DisruptionTail::new(
        UniverseTime::EPOCH,
        SolarMasses::new(1e8),
        SolarMasses::new(1.0),
        SolarRadii::new(10.0),
    );
    let later = UniverseTime::EPOCH
        .checked_add(Span::from_seconds_f64(10.0 * low_peak.fallback_time().value()).unwrap())
        .unwrap();
    let law: Vec<f64> = [1e-9, 1e-6, 1e-4, 1e-3, 5e-3, 1e-2, 15.0]
        .into_iter()
        .map(radiative_efficiency)
        .chain([
            low_peak.peak_luminosity_watts(),
            low_peak.luminosity_watts_at(later),
        ])
        .collect();
    let law_digest = f64_digest(&law);
    eprintln!("the hot flow's law: {law:?}, digest {law_digest:#018x}");
    assert_eq!(law_digest, HOT_FLOW_LAW_DIGEST);
}

/// D4: the same seed twice, a galaxy built afresh and the fixture, gives the same victims and the
/// same light to the bit, and the light at instants in any order is the same.
#[test]
fn the_hot_flow_gives_the_same_seed_the_same_light_twice_and_in_any_order() {
    let built =
        Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like()).unwrap();
    assert_eq!(&built, milky_way_galaxy());
    let once = CentreLuminosityModel::of(&built).unwrap();
    let twice = CentreLuminosityModel::new(milky_way_galaxy(), milky_way_centre()).unwrap();
    assert_eq!(once, twice);
    let times: Vec<UniverseTime> = (-20..=20)
        .map(|k| years_after(UniverseTime::EPOCH, 50 * k))
        .chain(
            twice
                .victims()
                .victims()
                .iter()
                .flat_map(|v| [10, 30, 500].map(|y| years_after(v.time(), y))),
        )
        .collect();
    for &t in &times {
        let (a, b) = (luminosity_at(&once, t), luminosity_at(&twice, t));
        assert_same_bits(a.tails_watts(), b.tails_watts());
        assert_same_bits(a.total_watts(), b.total_watts());
    }
    assert_order_independent(&times, |&t| luminosity_at(&twice, t));
}

/// P09.T30.c's test of Ponti et al.'s limit (2010, ApJ 714, 732: "an upper limit of 10³⁶ erg s⁻¹
/// to the mean luminosity of Sgr A* in the last 60–90 years"), as main ruled it: over P09.T30.b's
/// 200 seeds at Milky Way values, the centre's luminosity at the epoch, its quiescence, flares and
/// tails, is at most 10³⁶ erg/s in at least half the seeds. The count is pinned as measured, with
/// the figures printed for the plan's records.
#[test]
fn at_milky_way_values_the_centre_is_within_ponti_s_limit_at_the_epoch_in_most_seeds() {
    let mut light: Vec<f64> = swept_victims()
        .map(|(seed, victims)| {
            let model = CentreLuminosityModel {
                flares: CentreFlareModel::new(seed),
                victims,
            };
            luminosity_at(&model, UniverseTime::EPOCH).total_watts() / WATTS_PER_ERG_S
        })
        .collect();
    light.sort_by(f64::total_cmp);
    let count = |limit: f64| light.iter().filter(|&&l| l <= limit).count();
    let n = light.len();
    eprintln!(
        "{n} seeds at the epoch: least {:.3e}, median {:.3e}, 90th percentile {:.3e}, greatest \
         {:.3e} erg/s; {} at or below 1e34, {} at or below 3.5e35, {} at or below 1e36",
        light[0],
        light[n / 2],
        light[n * 9 / 10],
        light[n - 1],
        count(1e34),
        count(3.5e35),
        count(PONTI_LIMIT_ERG_S)
    );
    let within = count(PONTI_LIMIT_ERG_S);
    assert!(
        within >= 100,
        "{within} of 200 seeds within Ponti et al.'s limit"
    );
    assert_eq!(within, 149);
}
