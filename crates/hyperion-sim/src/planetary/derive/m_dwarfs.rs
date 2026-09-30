//! M dwarfs' inner planets are rocky (ruling 102.1): the rocky branch of the formation solve
//! about hosts under 0.6 M☉.
//!
//! Chen and Kipping's (2017) relation turns a planet into an enveloped sub-Neptune from about
//! 2 M⊕ up, its Neptunian class, wherever its host is. About the M dwarfs that overstates the
//! sub-Neptunes several times over:
//!
//! - Ment and Charbonneau (2023, AJ 165, 265, §4.4, §5.1 and Table 9), a volume-complete sample of
//!   363 hosts of 0.1–0.3 M☉ nearly complete above 1.5 R⊕, find seven planets, all of
//!   0.91–1.31 R⊕ and consistent with rock, a 1σ limit of 0.07 planets above 1.5 R⊕ per star at
//!   0.5–7 days (a median of 0.043), and rocky planets outnumbering the enveloped ones 14 to 1;
//!   they read it as late M dwarfs forming "fewer water worlds", or type I migration not bringing
//!   them in (§5.2). Dressing and Charbonneau's (2015) early M dwarfs, through the same table,
//!   have 0.29 ± 0.05 planets per star under 1.5 R⊕ and 0.18 ± 0.04 above it at 0.5–7 days.
//! - Luque and Pallé (2022, Science 377, 1211, abstract and Fig. 1), 34 planets about M dwarfs of
//!   mostly 0.3–0.5 M☉ with masses and radii to better than 25% and 8%: "rocky planets exist from
//!   0.3 to about 10 Earth masses" on the Earth-like curve, water worlds from 2–3 M⊕, puffy
//!   sub-Neptunes above about 6 M⊕ and 2.3 R⊕, and "the innermost planets are always rocky".
//!
//! So about a host under [`ROCKY_HOSTS`]' 0.6 M☉, blended away by 0.70 M☉ as the architecture's
//! early M dwarfs are ([`rocky_host_share`]), a body formed inside the snow line is rocky unless it
//! takes an envelope, with a probability rising with its mass ([`envelope_probability`]), which
//! is calibrated on those two anchors (P14.T10.b; ruling 102.1). A rocky outcome's composition is
//! the observed spread of rocky planets' core mass fractions ([`super::rocky`]) and its radius Zeng
//! et al.'s curves at it, as every rocky outcome's is; an enveloped one keeps Chen and Kipping's
//! scatter above the rock curve, and the solve's envelope. The brainstorm's "radius from mass
//! (Chen and Kipping 2017, refined by composition with Zeng et al. 2019)" admits it. FGK hosts are
//! unchanged, bit for bit. Recorded as a calibration: Zeng et al. 2019 was not re-read, and Luque
//! and Pallé's water-world class is disputed (not read here).
//!
//! The body's one rank still sets everything (design note 8): below 1 − p of its rank it is rocky,
//! at the same share of the rocky spread as before, and above it it is enveloped, at the same
//! share of Chen and Kipping's distribution above the rock curve. So the radius is continuous and
//! increasing in the rank, and at a host share of 0 the split is Chen and Kipping's own.

use crate::math;
use crate::planetary::architecture::{EARLY_M_DWARF_BLEND, EARLY_M_DWARF_MASSES, SUBSTELLAR_LIMIT};
use crate::units::{EarthMasses, SolarMasses};

/// The hosts whose inner planets are rocky unless they draw an envelope, and the mass by which the
/// rule is gone: up to 0.6 M☉, and none from 0.70 M☉ (ruling 102.1), the ends of the
/// architecture's early M dwarfs and of their upper blend.
pub const ROCKY_HOSTS: (SolarMasses, SolarMasses) = (EARLY_M_DWARF_MASSES.1, EARLY_M_DWARF_BLEND.1);

/// The planet mass at which a body formed inside the snow line about an M dwarf of
/// [`ENVELOPE_PIVOT_HOST`] takes an envelope half the time: 5 M⊕.
///
/// Calibrated on P14.T10.b's two anchors (ruling 102.1; see [`envelope_probability`]), with
/// [`ENVELOPE_HOST_SLOPE`]. Luque and Pallé (2022, abstract) put the water worlds from 2–3 M⊕ and
/// the puffy sub-Neptunes above about 6 M⊕ about hosts of mostly 0.3–0.5 M☉.
pub const ENVELOPE_HALF_MASS: EarthMasses = EarthMasses::new(5.0);

/// The host mass at which [`ENVELOPE_HALF_MASS`] applies: 0.35 M☉, where the early M dwarfs
/// begin.
pub const ENVELOPE_PIVOT_HOST: SolarMasses = SolarMasses::new(0.35);

/// How the half mass scales with the host: (M★ ÷ [`ENVELOPE_PIVOT_HOST`])^−1.5 (ruling 102.1), a
/// calibration.
///
/// With one half mass for every host the two anchors pull apart: the late M dwarfs' planets at
/// 0.5–7 days already lie above 1.5 R⊕ in nearly 0.12 of cases as dry rock (over four fifths of
/// them; a rocky planet of low core mass fraction reaches 1.5 R⊕ at about 3.3 M⊕), while the
/// early M dwarfs need envelopes to reach 0.10 per star there, and their planets at those periods
/// are the heavier by only about 0.15 dex. Ment and Charbonneau (2023, §5.2) read their late M
/// dwarfs' dearth of sub-Neptunes as those hosts forming "fewer water worlds", or type I migration
/// not bringing them in, against Dressing and Charbonneau's (2015) early M dwarfs with 1.6 rocky
/// planets to each sub-Neptune: a rate that falls with the host. At −1.5 the half mass is
/// 14.8 M⊕ at 0.17 M☉ and 2.9 M⊕ at 0.5 M☉.
pub const ENVELOPE_HOST_SLOPE: f64 = -1.5;

/// How sharply the envelope's probability rises about its half mass: the logistic's scale in
/// log₁₀ mass, 0.15 dex, a calibration (ruling 102.1).
pub const ENVELOPE_WIDTH_DEX: f64 = 0.15;

/// How far a host of `host` takes the rocky branch (ruling 102.1): 1 for a star up to 0.6 M☉, 0
/// from 0.70 M☉, and linear in ln M between, as the architecture's early M dwarfs' upper blend
/// is.
///
/// A host under [`SUBSTELLAR_LIMIT`] takes none: Ment and Charbonneau's and Luque and Pallé's
/// hosts are stars, a brown dwarf's chain of 0.01–2 M⊕ has no envelopes to lose, and a planet's
/// circumplanetary disc, in which its regular moons are derived (P14.T17.b), is no M dwarf.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::m_dwarfs::rocky_host_share;
/// use hyperion_sim::units::SolarMasses;
///
/// assert_eq!(rocky_host_share(SolarMasses::new(0.2)), 1.0);
/// assert_eq!(rocky_host_share(SolarMasses::new(1.0)), 0.0);
/// // A brown dwarf, or a planet's own disc.
/// assert_eq!(rocky_host_share(SolarMasses::new(0.05)), 0.0);
/// assert_eq!(rocky_host_share(SolarMasses::new(0.001)), 0.0);
/// let edge = rocky_host_share(SolarMasses::new(0.65));
/// assert!(edge > 0.0 && edge < 1.0);
/// ```
#[must_use]
pub fn rocky_host_share(host: SolarMasses) -> f64 {
    let (inner, outer) = ROCKY_HOSTS;
    let m = host.value();
    if host < SUBSTELLAR_LIMIT {
        0.0
    } else if m <= inner.value() {
        1.0
    } else if m >= outer.value() {
        0.0
    } else {
        1.0 - (math::ln(m / inner.value()) / math::ln(outer.value() / inner.value()))
            .clamp(0.0, 1.0)
    }
}

/// The probability that a body of `mass` formed inside the snow line of an M dwarf of `host` takes
/// an envelope (ruling 102.1): a logistic in log₁₀ mass, 1 ÷ (1 + 10^((log₁₀ `m_h` − log₁₀ m) ÷
/// [`ENVELOPE_WIDTH_DEX`])), with half mass `m_h` = [`ENVELOPE_HALF_MASS`] ×
/// (M★ ÷ [`ENVELOPE_PIVOT_HOST`])^[`ENVELOPE_HOST_SLOPE`].
///
/// It is capped by what Chen and Kipping's distribution itself leaves above the rock curve at the
/// mass, so that an M dwarf never has more envelopes than an FGK star would
/// ([`envelope_share`]). The anchors it is calibrated on, P14.T10.b's, are Ment and Charbonneau's
/// (2023, Table 9 and §5.1) late M dwarfs of 0.1–0.3 M☉, whose planets at 0.5–7 days lie above
/// 1.5 R⊕ in 0.02–0.12 of cases (their 14 to 1) and at most 0.16 per star at 1.4 R⊕ or more
/// (their model 5, 90%), and Dressing and Charbonneau's (2015) early M dwarfs of 0.4–0.6 M☉, with
/// 0.10–0.26 per star above 1.5 R⊕ and 0.19–0.39 below it (0.18 ± 0.04 and 0.29 ± 0.05, ±2σ).
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::m_dwarfs::{
///     ENVELOPE_HALF_MASS, ENVELOPE_PIVOT_HOST, envelope_probability,
/// };
/// use hyperion_sim::units::{EarthMasses, SolarMasses};
///
/// let early = ENVELOPE_PIVOT_HOST;
/// assert!((envelope_probability(ENVELOPE_HALF_MASS, early) - 0.5).abs() < 1e-12);
/// assert!(envelope_probability(EarthMasses::new(1.0), early) < 0.01);
/// assert!(envelope_probability(EarthMasses::new(20.0), early) > 0.95);
/// // A late M dwarf's planets take envelopes less often at the same mass.
/// let late = SolarMasses::new(0.17);
/// assert!(envelope_probability(ENVELOPE_HALF_MASS, late) < 0.05);
/// ```
#[must_use]
pub fn envelope_probability(mass: EarthMasses, host: SolarMasses) -> f64 {
    let half = ENVELOPE_HALF_MASS.value()
        * math::powf(
            host.value() / ENVELOPE_PIVOT_HOST.value(),
            ENVELOPE_HOST_SLOPE,
        );
    let x = (math::log10(half) - math::log10(mass.value())) / ENVELOPE_WIDTH_DEX;
    1.0 / (1.0 + math::exp10(x))
}

/// The share of a body's ranks that are enveloped, formed inside the snow line about a host of
/// `host`, where Chen and Kipping's own distribution leaves `chen_kipping` of its window above the
/// rock curve: (1 − s) × `chen_kipping` + s × min(p, `chen_kipping`), with s the host's
/// [`rocky_host_share`] and p [`envelope_probability`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::m_dwarfs::{envelope_probability, envelope_share};
/// use hyperion_sim::units::{EarthMasses, SolarMasses};
///
/// let mass = EarthMasses::new(3.0);
/// let (sun, m_dwarf) = (SolarMasses::new(1.0), SolarMasses::new(0.4));
/// assert_eq!(envelope_share(mass, sun, 0.6), 0.6);
/// assert_eq!(envelope_share(mass, m_dwarf, 0.6), envelope_probability(mass, m_dwarf));
/// assert_eq!(envelope_share(mass, m_dwarf, 0.0), 0.0);
/// ```
#[must_use]
pub fn envelope_share(mass: EarthMasses, host: SolarMasses, chen_kipping: f64) -> f64 {
    let host_share = rocky_host_share(host);
    let own = envelope_probability(mass, host).min(chen_kipping);
    (1.0 - host_share) * chen_kipping + host_share * own
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    #[test]
    fn the_host_share_is_continuous_and_falls_across_its_blend() {
        let (inner, outer) = ROCKY_HOSTS;
        assert_same_bits(rocky_host_share(inner), 1.0);
        assert!(rocky_host_share(outer).abs() < 1e-12);
        let mut last = 1.0;
        for k in 0..=100 {
            let m = inner.value() + (outer.value() - inner.value()) * f64::from(k) / 100.0;
            let share = rocky_host_share(SolarMasses::new(m));
            assert!(
                share <= last && (0.0..=1.0).contains(&share),
                "{m}: {share}"
            );
            assert!(last - share < 0.02, "{m}: steps from {last} to {share}");
            last = share;
        }
    }

    #[test]
    fn the_envelope_probability_rises_with_mass_and_never_exceeds_chen_and_kipping_s() {
        let mut last = 0.0;
        for k in 0..=200 {
            let m = EarthMasses::new(math::exp10(-1.0 + 2.5 * f64::from(k) / 200.0));
            let p = envelope_probability(m, ENVELOPE_PIVOT_HOST);
            assert!(p >= last && (0.0..=1.0).contains(&p), "{}: {p}", m.value());
            last = p;
            for ck in [0.0, 0.2, 0.9] {
                for host in [0.1, 0.5, 0.65, 1.0] {
                    let e = envelope_share(m, SolarMasses::new(host), ck);
                    assert!(e <= ck + 1e-15 && e >= 0.0, "{}: {e}", m.value());
                }
            }
        }
    }
}
