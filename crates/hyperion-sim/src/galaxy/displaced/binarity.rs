//! The binarity seam: the one stripped share the class quadrature and plan 11's systems both read
//! (plan 08, P08.T8.a).
//!
//! A progenitor whose envelope a companion strips dies with a lighter core and, below a
//! carbon–oxygen core of 3 M☉, may take the kick law's low mode (plan 06, design note 11), so the
//! share of stripped primaries sets how many remnants stay near their birth sites. It is built on
//! plan 11's [`stellar::multiplicity::stripped_share`](crate::stellar::multiplicity::stripped_share)
//! quadrature, `S(a)`: the primary's multiple fraction times the probability that its innermost
//! companion's periastron lies under a threshold `a(m₁, q)`, over plan 11's period, mass-ratio and
//! eccentricity laws ([`MultiplicityModel::default_v1`]).
//!
//! # Stripped is not interacting (ruling 123 of 2026-09-22)
//!
//! A companion strips the primary when the primary fills its Roche lobe before its core helium
//! runs out (Case A or B; Podsiadlowski et al. 2004), unless the transfer is unstable early enough
//! to merge the pair. Each case is a periastron: the primary's largest radius up to the end of a
//! stage ([`Track::max_radius_until`] at the median draws) over Eggleton's (1983) lobe fraction
//! `f(m₁ ÷ m₂)`:
//!
//! - `a_A` to the end of the main sequence, `a_HG` to the end of the Hertzsprung gap, `a_B` to the
//!   end of core helium burning (the death if the star dies in it);
//! - `a_merge` is `a_HG` for `q < 1 ÷ GAP_Q` (1/4), `a_A` for `1 ÷ GAP_Q ≤ q < 1 ÷ CODE_Q` (1/3),
//!   and 0 above: the binary engine's own onset ratios (`stellar::binary`'s `GAP_Q` and `CODE_Q`,
//!   BSE sections 2.6.1 and 2.6.3), read, not copied. A gap donor's common envelope counts as a
//!   merger, a giant's or supergiant's as stripped;
//! - stripped means a periastron in `(a_merge, a_B]` ([`stripping_band`]), and the share is `S(a_B)
//!   − S(a_merge)` ([`stripped_share`]), two calls of the quadrature: exact, and drawing nothing.
//!
//! [`interacting_periastron`] stays P11.T4.a's [`can_interact`] boundary, either star's largest
//! radius up to the primary's death, for the engine's gate and the hierarchy's interacting range
//! ([`interacting_share`]); ruling 123.3 checks stripped over interacting at 0.5–0.75.
//!
//! Nothing generated reads the seam yet: plan 11's hierarchy keeps its own provisional range until
//! P11.T1.d, which draws a marked innermost orbit from the stripping band and an unmarked one from
//! its complement, and the class table (P08.T9) is not built. [`kick_bins`](super::kick_bins) reads
//! it.
//!
//! [`can_interact`]: crate::stellar::binary::can_interact

use crate::orbit::roche_lobe_radius;
use crate::stellar::Composition;
use crate::stellar::binary::{CODE_Q, GAP_Q};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{MultiplicityModel, stripped_share as multiplicity_share};
use crate::stellar::sse::{MAX_INITIAL_MASS, MIN_INITIAL_MASS, Stage, Track};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{Metres, SolarMasses, Years};

/// A star's largest radius up to `until` (or its whole life), in solar radii, from its track at
/// the median draws; zero below the tracks' lightest mass. Tracks stop at 100 M☉ until P06.T14,
/// so a heavier star is taken as one of 100 M☉.
fn largest_radius(m: SolarMasses, comp: &Composition, until: Option<Years>) -> (f64, Years) {
    if m < MIN_INITIAL_MASS {
        return (0.0, Years::ZERO);
    }
    let m0 = if m > MAX_INITIAL_MASS {
        MAX_INITIAL_MASS
    } else {
        m
    };
    let draws = StarDraws::median();
    let track = match until {
        Some(age) => Track::to_age(m0, comp, &draws, age),
        None => Track::full(m0, comp, &draws),
    };
    let end = until
        .or_else(|| track.lifetime())
        .unwrap_or_else(|| track.built_until());
    (track.max_radius_until(end).value(), end)
}

/// The largest periastron at which stars of `r_1` and `r_2` solar radii and masses `m1` and `m2`
/// fill a Roche lobe (module documentation).
fn threshold(r_1: f64, r_2: f64, m1: f64, m2: f64) -> Metres {
    let unit = Metres::new(SOLAR_RADIUS_M);
    let reach = |r: f64, m: f64, other: f64| {
        if r > 0.0 && m > 0.0 && other > 0.0 {
            r * SOLAR_RADIUS_M / roche_lobe_radius(m / other, unit).value() * SOLAR_RADIUS_M
        } else {
            0.0
        }
    };
    Metres::new(reach(r_1, m1, m2).max(reach(r_2, m2, m1)))
}

/// The largest periastron at which a pair of primary `m1`, mass ratio `q` and composition `comp`
/// interacts before the primary's core collapse: P11.T4.a's [`can_interact`] threshold (module
/// documentation, "The threshold").
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::interacting_periastron;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::{AstronomicalUnits, SolarMasses};
///
/// // A 15 M☉ red supergiant reaches a companion several au out.
/// let a = interacting_periastron(SolarMasses::new(15.0), 0.5, &Composition::SOLAR);
/// let au = AstronomicalUnits::from(a).value();
/// assert!((2.0..40.0).contains(&au), "{au} au");
/// ```
///
/// [`can_interact`]: crate::stellar::binary::can_interact
#[must_use]
pub fn interacting_periastron(m1: SolarMasses, q: f64, comp: &Composition) -> Metres {
    let (r_1, death) = largest_radius(m1, comp, None);
    let m2 = m1 * q;
    let (r_2, _) = largest_radius(m2, comp, Some(death));
    threshold(r_1, r_2, m1.value(), m2.value())
}

/// The periastra a companion of mass ratio `q` strips its primary between (ruling 123.2): a
/// periastron in `(merge, strip]` is Case A or B transfer that does not merge the pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrippingBand {
    /// `a_merge`: at or inside it the pair merges (0 for `q ≥ 1/3`).
    pub merge: Metres,
    /// `a_B`: beyond it the primary never fills its lobe before its core helium runs out.
    pub strip: Metres,
}

/// The primary's largest radii, solar radii, up to the end of its main sequence, of its
/// Hertzsprung gap and of core helium burning, from its full track at the median draws (at 100
/// M☉ above the tracks' range).
fn stage_radii(m1: SolarMasses, comp: &Composition) -> [f64; 3] {
    let m0 = if m1 > MAX_INITIAL_MASS {
        MAX_INITIAL_MASS
    } else {
        m1
    };
    let track = Track::full(m0, comp, &StarDraws::median());
    let death = track.lifetime().unwrap_or_else(|| track.built_until());
    [
        Stage::MainSequence,
        Stage::HertzsprungGap,
        Stage::CoreHeliumBurning,
    ]
    .map(|stage| {
        let end = track
            .stage_end(stage)
            .map_or(death, |e| if e < death { e } else { death });
        track.max_radius_until(end).value()
    })
}

/// The band of `radii` ([`stage_radii`]) for a companion of mass ratio `q`.
fn band_of(radii: [f64; 3], q: f64) -> StrippingBand {
    let [r_a, r_hg, r_b] = radii;
    let fraction = roche_lobe_radius(1.0 / q, Metres::new(1.0)).value();
    let reach = |r: f64| Metres::new(r * SOLAR_RADIUS_M / fraction);
    let merge = if q < 1.0 / GAP_Q {
        reach(r_hg)
    } else if q < 1.0 / CODE_Q {
        reach(r_a)
    } else {
        Metres::ZERO
    };
    StrippingBand {
        merge,
        strip: reach(r_b),
    }
}

/// The stripping band of a pair of primary `m1`, mass ratio `q` and composition `comp` (module
/// documentation, "Stripped is not interacting"): what P11.T1.d draws a marked innermost orbit
/// from.
///
/// # Panics
///
/// If `q` is not positive and finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::stripping_band;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::{AstronomicalUnits, SolarMasses};
///
/// let m = SolarMasses::new(15.0);
/// // An equal-mass companion never merges; a light one merges out to the gap's reach.
/// let twin = stripping_band(m, 0.9, &Composition::SOLAR);
/// let light = stripping_band(m, 0.2, &Composition::SOLAR);
/// assert!(twin.merge.value().abs() < f64::EPSILON && light.merge > twin.merge);
/// assert!(AstronomicalUnits::from(twin.strip).value() > 1.0);
/// ```
#[must_use]
pub fn stripping_band(m1: SolarMasses, q: f64, comp: &Composition) -> StrippingBand {
    band_of(stage_radii(m1, comp), q)
}

/// The share of primaries of initial mass `m` and composition `comp` whose envelope a companion
/// strips before their core helium runs out, mergers excluded: `S(a_B) − S(a_merge)` over plan
/// 11's [`stripped_share`](crate::stellar::multiplicity::stripped_share) quadrature and model
/// ([`MultiplicityModel::default_v1`]), the primary's track built once (ruling 123.2).
///
/// # Panics
///
/// If `m` is not positive and finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::binarity::stripped_share;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::SolarMasses;
///
/// // About a third of 16–20 M☉ primaries are stripped (Sana et al. 2012; ruling 123).
/// let share = stripped_share(SolarMasses::new(18.0), &Composition::SOLAR);
/// assert!((0.25..0.45).contains(&share), "{share}");
/// ```
#[must_use]
pub fn stripped_share(m: SolarMasses, comp: &Composition) -> f64 {
    let radii = stage_radii(m, comp);
    let model = MultiplicityModel::default_v1();
    let within = |edge: fn(&StrippingBand) -> Metres| {
        multiplicity_share(
            &model,
            m,
            comp,
            |_: SolarMasses, q: f64, _: &Composition| edge(&band_of(radii, q)),
        )
    };
    within(|band| band.strip) - within(|band| band.merge)
}

/// The share of primaries of initial mass `m` and composition `comp` that interact with their
/// innermost companion before they die, mergers and Case C included: `S` at
/// [`interacting_periastron`], the primary's track built once.
///
/// # Panics
///
/// If `m` is not positive and finite.
#[must_use]
pub fn interacting_share(m: SolarMasses, comp: &Composition) -> f64 {
    let (r_1, death) = largest_radius(m, comp, None);
    multiplicity_share(
        &MultiplicityModel::default_v1(),
        m,
        comp,
        |m1: SolarMasses, q: f64, comp: &Composition| {
            let m2 = m1 * q;
            let (r_2, _) = largest_radius(m2, comp, Some(death));
            threshold(r_1, r_2, m1.value(), m2.value())
        },
    )
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::math;
    use crate::stellar::remnant::RemnantKind;
    use crate::units::{Dex, HeliumExcess};

    fn layer_e(i: i32) -> SolarMasses {
        SolarMasses::new(8.0 * math::exp(f64::from(i) / 32.0 * math::ln(150.0 / 8.0)))
    }

    /// P08.T8.a with ruling 123.2: the seam is plan 11's quadrature at the band's two edges, bit
    /// for bit, and the interacting share is it at `can_interact`'s boundary, at masses across
    /// layer E's band and three metallicities.
    #[test]
    fn the_seam_is_plan_elevens_quadrature_at_the_bands_edges() {
        let model = MultiplicityModel::default_v1();
        for fe_h in [-1.5, 0.0, 0.3] {
            let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            for m in (0..33).step_by(8).map(layer_e) {
                let at = |edge: fn(&StrippingBand) -> Metres| {
                    multiplicity_share(&model, m, &comp, |m1, q, c| edge(&stripping_band(m1, q, c)))
                };
                let share = stripped_share(m, &comp);
                assert_same_bits(share, at(|b| b.strip) - at(|b| b.merge));
                assert!((0.0..=1.0).contains(&share), "{share} at {m:?}");
                let interacting = multiplicity_share(&model, m, &comp, interacting_periastron);
                assert_same_bits(interacting_share(m, &comp), interacting);
                assert!(share <= interacting, "{share} over {interacting} at {m:?}");
            }
        }
    }

    /// The band's edges: `a_merge` is the gap's reach below q = 1/4, the main sequence's to 1/3 and
    /// 0 above, never beyond `a_B`, and `a_B` never beyond `can_interact`'s boundary.
    #[test]
    fn the_band_follows_the_engines_onset_ratios() {
        let comp = Composition::SOLAR;
        for m in [9.0, 15.0, 40.0] {
            let m = SolarMasses::new(m);
            let radii = stage_radii(m, &comp);
            assert!(radii[0] <= radii[1] && radii[1] <= radii[2], "{radii:?}");
            let light = stripping_band(m, 0.2, &comp);
            let middle = stripping_band(m, 0.3, &comp);
            let heavy = stripping_band(m, 0.5, &comp);
            let reach = |r: f64, q: f64| {
                r * SOLAR_RADIUS_M / roche_lobe_radius(1.0 / q, Metres::new(1.0)).value()
            };
            assert!((light.merge.value() / reach(radii[1], 0.2) - 1.0).abs() < 1e-12);
            assert!((middle.merge.value() / reach(radii[0], 0.3) - 1.0).abs() < 1e-12);
            assert!(heavy.merge.value().abs() < f64::EPSILON);
            for (band, q) in [(light, 0.2), (middle, 0.3), (heavy, 0.5)] {
                assert!(band.merge <= band.strip);
                assert!(
                    band.strip.value()
                        <= interacting_periastron(m, q, &comp).value() * (1.0 + 1e-12)
                );
            }
        }
        assert!((1.0 / GAP_Q - 0.25).abs() < 1e-15 && (1.0 / CODE_Q - 1.0 / 3.0).abs() < 1e-15);
    }

    /// The threshold is `can_interact`'s: a pair just inside it interacts before the primary's
    /// death, one just outside does not.
    #[test]
    fn the_threshold_is_can_interacts_boundary() {
        use crate::orbit::{Eccentricity, KeplerElements, Orientation};
        use crate::stellar::binary::{BinaryInput, can_interact};
        use crate::units::{GravitationalParameter, Radians};
        let comp = Composition::SOLAR;
        for (m1, q) in [(12.0, 0.6), (25.0, 0.3), (9.0, 0.95)] {
            let m1 = SolarMasses::new(m1);
            let a = interacting_periastron(m1, q, &comp);
            let death = Track::full(m1, &comp, &StarDraws::median())
                .lifetime()
                .expect("a full track dies");
            let pair = |factor: f64| {
                let orbit = KeplerElements::from_semi_major_axis(
                    Metres::new(a.value() * factor),
                    GravitationalParameter::from_solar_masses(m1 * (1.0 + q)),
                    Eccentricity::CIRCULAR,
                    Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))
                        .unwrap(),
                    Radians::new(0.0),
                )
                .unwrap();
                let input = BinaryInput::new(
                    m1,
                    m1 * q,
                    comp,
                    orbit,
                    [StarDraws::median(), StarDraws::median()],
                    death,
                )
                .unwrap();
                can_interact(&input, death)
            };
            assert!(pair(0.999), "{m1:?} q {q} just inside");
            assert!(!pair(1.001), "{m1:?} q {q} just outside");
        }
    }

    /// Ruling 123.3: over neutron-star progenitors (the masses of layer E whose track at the
    /// median draws leaves a neutron star), weighted by the default mass function, the stripped
    /// share lies in 0.25–0.33 and stripped ÷ interacting in 0.5–0.75; over the whole band (P11.T1.d's
    /// check) the share lies in 0.20–0.33.
    #[test]
    fn the_band_averaged_stripped_share() {
        let imf = MassFunctionKind::default().to_mass_function();
        let comp = Composition::SOLAR;
        // [stripped, interacting, weight] over the band and over its neutron-star progenitors.
        let (mut band, mut ns) = ([0.0; 3], [0.0; 3]);
        for i in 0..33 {
            let m = layer_e(i);
            let end = i == 0 || i == 32;
            let w = imf.pdf(m.value()) * m.value() * if end { 0.5 } else { 1.0 };
            let (stripped, interacting) = (stripped_share(m, &comp), interacting_share(m, &comp));
            if i % 8 == 0 {
                eprintln!(
                    "{:.1} M☉: stripped {stripped:.4}, interacting {interacting:.4}",
                    m.value()
                );
            }
            let m0 = if m > MAX_INITIAL_MASS {
                MAX_INITIAL_MASS
            } else {
                m
            };
            let neutron_star = Track::full(m0, &comp, &StarDraws::median())
                .remnant()
                .is_some_and(|r| r.kind() == RemnantKind::NeutronStar);
            for (sum, on) in [(&mut band, true), (&mut ns, neutron_star)] {
                if on {
                    sum[0] += w * stripped;
                    sum[1] += w * interacting;
                    sum[2] += w;
                }
            }
        }
        let (layer, progenitors) = (band[0] / band[2], ns[0] / ns[2]);
        let ratio = ns[0] / ns[1];
        eprintln!(
            "stripped share: layer E {layer:.4}, neutron-star progenitors {progenitors:.4}; \
             stripped ÷ interacting {ratio:.4}"
        );
        assert!((0.25..=0.33).contains(&progenitors), "{progenitors}");
        assert!((0.20..=0.33).contains(&layer), "{layer}");
        assert!((0.5..=0.75).contains(&ratio), "{ratio}");
    }
}
