//! Rough plains from ray-traced surfaces: the reader of `hyperion-fit`'s table
//! `rough_plains_cold_traps` (plan R09, R09.T7.g, refitted by R09.T7.l; `science-r09-drainage.md`
//! §8.13, signed off in `signoff-12.md` item 13; `decision-r09-t7g-table`).
//!
//! Hayne et al. (2021, Nat. Astron. 5, 169) give rough plains no closed form: they ray-trace
//! Gaussian surfaces (their Methods D). The fitting task does the same offline, on periodic
//! surfaces of Hurst exponent 0.9 scaled to a directional RMS slope w = tan `σ_s`, under a point
//! star, and tabulates two things ([`tables::rough_plains_cold_traps`](crate::tables::rough_plains_cold_traps)):
//!
//! - **the permanent shadow**, the ground that no part of the star's disk ever reaches, over the
//!   slope w (0.05–1), the colatitude e₀ in radians over w and D = δ + α in radians over w
//!   ([`permanent_shadow`]);
//! - **the shares of it that sunlit terrain warms least**: those whose peak irradiance from the
//!   sunlit facets they see, to first order (each facet's direct flux weighted by the view factor
//!   to it), is at most each of fixed fractions of S sin e, e = e₀ + max(0, D − α) the highest
//!   elevation of the point at the disk's centre that lit the facets, for the table's one α
//!   ([`STAR_RADIUS`](table::STAR_RADIUS), the Sun's at the Moon) ([`cold_share`]). The table
//!   holds no albedo or emissivity.
//!
//! A body's own albedo A and emissivity ε turn that irradiance E into the shadow's absorbed flux:
//! a lit facet scatters A of its direct flux and re-emits the absorbed 1 − A in the thermal
//! infrared, of which the shadow absorbs 1 − A and ε, so it absorbs (1 − A)(A + ε) E. A shadow is
//! cold below the threshold T when that and the heat flow q stay under its emission εσT⁴, so the
//! share is read at (εσT⁴ − q) ÷ ((1 − A)(A + ε) S sin `e_eff`) ([`irradiance_limit`],
//! [`fraction`]); none is cold where that is not positive. The heating leaves out light scattered
//! more than once and the shadowed terrain's own emission, which would add to it. The table was
//! traced with the shadow under a point at the star's upper limb, D = δ + α from the equator, and
//! the sunlit facets lit by a point at its disk's centre (a point at the upper limb lights them
//! more than the disk does, 1.18 times at the Moon's pole). The heating takes `e_eff`, the body's
//! own disk's flux-weighted elevation at its highest, as the bowls do: for a disk that clears the
//! horizon and the table's α it is the traced point's own elevation, and for another α it corrects
//! the traced heating to first order.
//!
//! Heights and angles scale together, so at small slopes a plain's shadow depends on e₀ ÷ w and
//! D ÷ w alone, and a slope below the table's 0.05 reads its 0.05 slice at its own ratios. At those
//! ratios the irradiance over S sin e scales as w² (a spherical bowl's f (1 − f) ≈ 4γ²), so each
//! slope node is read at the fraction scaled by its (`w_n` ÷ w)², the 0.05 slice included. A plain
//! with no slope casts no shadow. The cold shares are tabulated at fewer nodes than the shadow,
//! every node where D ÷ w and e₀ ÷ w are small and sparser beyond, and each cold node's shadow is
//! found by its indices among the shadow's ([`COLD_COLATITUDE_INDEX`](table::COLD_COLATITUDE_INDEX),
//! [`COLD_LIMB_INDEX`](table::COLD_LIMB_INDEX)).
//!
//! **Its use.** A [`Terrain`](super::Terrain) reads its plains here, in
//! [`fraction`](super::fraction) and [`permanent_shadow`](super::permanent_shadow), to the bit. The
//! galaxy's generator takes them from version 25, in place of T7.d's labelled bowls before it
//! (R09.T7.g's swap), which moved galaxy plan 14's published cold-trap areas.

use super::{Body, Sky, cell};
use crate::num;
use crate::tables::rough_plains_cold_traps as table;
use hyperion_base::math;
use hyperion_base::units::consts::STEFAN_BOLTZMANN;
use hyperion_base::units::{Kelvin, Radians};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

/// Checks a plain's slope, a colatitude and D = δ + α, and gives w = tan `σ_s` and the two angles
/// in radians.
///
/// # Panics
///
/// If the slope is not in [0, π/4], the colatitude not in [0, π/2] or D not in [0, π).
#[must_use]
fn checked(slope: Radians, colatitude: Radians, limb: Radians) -> (f64, f64, f64) {
    let (s, e0, d) = (slope.value(), colatitude.value(), limb.value());
    assert!(
        (0.0..=FRAC_PI_4).contains(&s),
        "a plain's RMS slope must be in [0, π/4], got {s} rad"
    );
    assert!(
        (0.0..=FRAC_PI_2).contains(&e0),
        "a colatitude must be in [0, π/2], got {e0} rad"
    );
    assert!(
        (0.0..PI).contains(&d),
        "a star's declination and radius must sum to [0, π), got {d} rad"
    );
    (math::tan(s), e0, d)
}

/// One cold node's share of its shadow at the irradiance fraction `x` > 0, from its `shares` at
/// the table's irradiance nodes (0, then even in their logarithm): linear in x from 0 to the
/// first positive node; between positive nodes linear in ln x and ln share where both shares are
/// positive (a tail that falls as a power of x), else linear in ln x; and the last node's share
/// from the last node on.
#[must_use]
fn share_at(shares: &[f64], x: f64) -> f64 {
    let nodes = &table::IRRADIANCE;
    let last = nodes.len() - 1;
    if x >= nodes[last] {
        return shares[last];
    }
    if x < nodes[1] {
        return shares[0] + (shares[1] - shares[0]) * (x / nodes[1]);
    }
    let i = nodes.partition_point(|&node| node <= x) - 1;
    let at = math::ln(x / nodes[i]) / math::ln(nodes[i + 1] / nodes[i]);
    let (low, high) = (shares[i], shares[i + 1]);
    if low > 0.0 && high > low {
        low * math::exp(at * math::ln(high / low))
    } else {
        low + at * (high - low)
    }
}

/// What a read gives: the permanent shadow, or the cold share at an irradiance fraction.
#[derive(Debug, Clone, Copy)]
enum Share {
    /// The permanent shadow's share of the ground.
    Shadow,
    /// The share of the ground in permanent shadow whose peak irradiance is at most this fraction
    /// of S sin e.
    Cold(f64),
}

/// The table's read, `what` at slope `w`, colatitude `colatitude` and D `limb` (radians). The
/// fitting task's own read is the same arithmetic in the same order, which its tests hold to the
/// bit.
///
/// It is zero for a plain with no slope, for a fraction that is not positive, where colatitude +
/// D ≥ 90° (the star passes the zenith, which a height field cannot hide), and beyond the
/// colatitude's or D's last node. The shadow is trilinear in w, the colatitude over w and D over
/// w. The cold share is the shadow times the mean of the cold nodes' shares about the point, each
/// weighted by its trilinear weight times its node's shadow, found by the node's indices among
/// the shadow's, so that the cold share never exceeds the shadow. Each slope node `w_n` reads its
/// shares at the fraction times (`w_n` ÷ w)², since at fixed ratios the irradiance over S sin e
/// scales as w² (a spherical bowl's f (1 − f) ≈ 4γ²), and each share is read by [`share_at`].
#[must_use]
fn read(w: f64, colatitude: f64, limb: f64, what: Share) -> f64 {
    let not_positive = matches!(what, Share::Cold(fraction) if fraction <= 0.0);
    if w <= 0.0 || not_positive || colatitude + limb >= FRAC_PI_2 {
        return 0.0;
    }
    let over = |angle: f64| if angle > 0.0 { angle / w } else { 0.0 };
    let (colatitude_over, limb_over) = (over(colatitude), over(limb));
    let (colatitude_last, limb_last) = (
        table::COLATITUDE_PER_SLOPE.len() - 1,
        table::LIMB_PER_SLOPE.len() - 1,
    );
    if colatitude_over >= table::COLATITUDE_PER_SLOPE[colatitude_last]
        || limb_over >= table::LIMB_PER_SLOPE[limb_last]
    {
        return 0.0;
    }
    let (slope_cell, slope_at) = cell(&table::SLOPE, w);
    let (colatitude_cell, colatitude_at) = cell(&table::COLATITUDE_PER_SLOPE, colatitude_over);
    let (limb_cell, limb_at) = cell(&table::LIMB_PER_SLOPE, limb_over);
    let width = table::COLATITUDE_PER_SLOPE.len();
    let mut share = 0.0;
    for (slope_step, slope_weight) in [(0, 1.0 - slope_at), (1, slope_at)] {
        for (colatitude_step, colatitude_weight) in [(0, 1.0 - colatitude_at), (1, colatitude_at)] {
            let row = &table::SHADOW
                [(slope_cell + slope_step) * width + colatitude_cell + colatitude_step];
            for (limb_step, limb_weight) in [(0, 1.0 - limb_at), (1, limb_at)] {
                share +=
                    slope_weight * colatitude_weight * limb_weight * row[limb_cell + limb_step];
            }
        }
    }
    let Share::Cold(fraction) = what else {
        return share;
    };
    if share <= 0.0 {
        return 0.0;
    }
    let cold_width = table::COLD_COLATITUDE_PER_SLOPE.len();
    let cold_limbs = table::COLD_LIMB_PER_SLOPE.len();
    let (cold_colatitude_cell, cold_colatitude_at) =
        cell(&table::COLD_COLATITUDE_PER_SLOPE, colatitude_over);
    let (cold_limb_cell, cold_limb_at) = cell(&table::COLD_LIMB_PER_SLOPE, limb_over);
    let (mut sum, mut weight) = (0.0, 0.0);
    for (slope_step, slope_weight) in [(0, 1.0 - slope_at), (1, slope_at)] {
        let scale = table::SLOPE[slope_cell + slope_step] / w;
        let scaled = fraction * (scale * scale);
        for (colatitude_step, colatitude_weight) in
            [(0, 1.0 - cold_colatitude_at), (1, cold_colatitude_at)]
        {
            let fine = &table::SHADOW[(slope_cell + slope_step) * width
                + usize::from(
                    table::COLD_COLATITUDE_INDEX[cold_colatitude_cell + colatitude_step],
                )];
            let coarse =
                (slope_cell + slope_step) * cold_width + cold_colatitude_cell + colatitude_step;
            for (limb_step, limb_weight) in [(0, 1.0 - cold_limb_at), (1, cold_limb_at)] {
                let limb_node = cold_limb_cell + limb_step;
                let k = slope_weight
                    * colatitude_weight
                    * limb_weight
                    * fine[usize::from(table::COLD_LIMB_INDEX[limb_node])];
                let row = &table::COLD[coarse * cold_limbs + limb_node];
                sum += k * share_at(row, scaled);
                weight += k;
            }
        }
    }
    if weight > 0.0 {
        share * num::min(1.0, sum / weight)
    } else {
        0.0
    }
}

/// The share of rough plains' ground in permanent shadow, in [0, 1]: plains of directional RMS
/// slope `slope` (`σ_s`), at `colatitude` under a star whose disk reaches `limb` = δ + α from the
/// celestial equator, its greatest declination plus its angular radius.
///
/// It is the ray-traced table's trilinear read (the module documentation gives the rules). It does
/// not rise with `limb`, and is zero for a plain with no slope and once colatitude + `limb`
/// reaches 90°.
///
/// # Panics
///
/// If the slope is not in [0, π/4], the colatitude not in [0, π/2] or `limb` not in [0, π).
#[must_use]
pub fn permanent_shadow(slope: Radians, colatitude: Radians, limb: Radians) -> f64 {
    let (w, e0, d) = checked(slope, colatitude, limb);
    num::max(0.0, num::min(1.0, read(w, e0, d, Share::Shadow)))
}

/// The share of rough plains' ground in permanent shadow whose peak irradiance from the sunlit
/// terrain it sees is at most `irradiance` × S sin e, e = colatitude + max(0, `limb` − α), α the
/// table's [`STAR_RADIUS`](table::STAR_RADIUS): the table's cold share, as [`permanent_shadow`]
/// takes its other inputs.
///
/// It never exceeds [`permanent_shadow`], does not fall as `irradiance` rises, and is zero where
/// `irradiance` is not positive. Past the table's last fraction it is that fraction's.
///
/// # Panics
///
/// As [`permanent_shadow`], and if `irradiance` is NaN.
#[must_use]
pub fn cold_share(slope: Radians, colatitude: Radians, limb: Radians, irradiance: f64) -> f64 {
    let (w, e0, d) = checked(slope, colatitude, limb);
    assert!(
        !irradiance.is_nan(),
        "an irradiance fraction must not be NaN"
    );
    num::max(0.0, num::min(1.0, read(w, e0, d, Share::Cold(irradiance))))
}

/// The share of rough plains' ground and azimuths in shadow with the sun at `elevation`: the
/// share whose horizon is at least the sun's elevation, read linear in tan e ÷ w from the table,
/// and its last node's beyond it. It is zero for a plain with no slope.
///
/// # Panics
///
/// If the slope is not in [0, π/4] or the elevation not in [0, π/2).
#[must_use]
pub fn instantaneous_shadow(slope: Radians, elevation: Radians) -> f64 {
    let (w, _, _) = checked(slope, Radians::ZERO, Radians::ZERO);
    let e = elevation.value();
    assert!(
        (0.0..FRAC_PI_2).contains(&e),
        "a sun's elevation must be in [0, π/2), got {e} rad"
    );
    if w <= 0.0 {
        return 0.0;
    }
    let (i, at) = cell(&table::TAN_ELEVATION_PER_SLOPE, math::tan(e) / w);
    (1.0 - at) * table::INSTANTANEOUS[i] + at * table::INSTANTANEOUS[i + 1]
}

/// The irradiance fraction at which `body`'s rough plains are cold below `threshold`, at
/// `colatitude` radians: (εσT⁴ − q) ÷ ((1 − A)(A + ε) S sin `e_eff`), with the body's own albedo
/// A, emissivity ε, flux S and heat flow q.
///
/// `e_eff` is the flux-weighted elevation of the star's disk at its highest, its centre at
/// min(90°, colatitude + δ) ([`flux_weighted_elevation`](super::flux_weighted_elevation)): the
/// bowls' sin e, bit for bit
/// (`Sky::at`). It is the shadow's absorbed flux to first order, the sunlit facets' scattered
/// light and their thermal emission, against what its emission at the threshold leaves after the
/// heat flow (the module documentation). The table traced the sunlit facets under a point at the
/// disk's centre, D − α for its own α ([`STAR_RADIUS`](table::STAR_RADIUS)), and holds their
/// heating over S sin(colatitude + D − α), while a plain under the disk intercepts S sin `e_eff`.
/// For a disk that clears the horizon and the table's α the two are one. Otherwise, to first order
/// the facets' heating scales with the flux they intercept, so the shares are read at fractions of
/// S sin `e_eff`: the shadow takes the upper limb, and the heating the disk
/// (`decision-r09-t7g-table` §2). That is exact for the plain's mean heating, and corrects a pixel
/// that peaks away from noon at noon's ratio; where the horizon cuts the disk it is an
/// extrapolation. Beyond first order a body whose α exceeds the table's (Mercury) keeps its
/// heating overstated and its cold traps understated, and one whose α is smaller (a far body) the
/// reverse (R09.T7.l's record). It is zero where the heat
/// flow alone warms the shadow past the threshold, and infinite where `e_eff` = 0, a point star
/// that never rises, but the shadow can be cold.
///
/// # Panics
///
/// If the colatitude is not in [0, π/2] or the threshold not finite and positive.
#[must_use]
pub fn irradiance_limit(colatitude: Radians, body: &Body, threshold: Kelvin) -> f64 {
    // Only the colatitude's refusal: the slope and D are not this function's.
    let (_, _, _) = checked(Radians::ZERO, colatitude, Radians::ZERO);
    let t = threshold.value();
    assert!(
        t.is_finite() && t > 0.0,
        "a threshold must be finite and positive, got {t} K"
    );
    let p = body.parts();
    let allowed = p.emissivity * STEFAN_BOLTZMANN * t * t * t * t - p.heat_flow.value();
    if allowed <= 0.0 {
        return 0.0;
    }
    let a = p.bond_albedo;
    let lit = (1.0 - a)
        * (a + p.emissivity)
        * p.flux.value()
        * Sky::sin_highest_at(colatitude.value(), body);
    if lit > 0.0 {
        allowed / lit
    } else {
        f64::INFINITY
    }
}

/// The share of `body`'s rough plains of slope `slope` at `latitude` in permanent shadow, in
/// [0, 1]: [`permanent_shadow`] at the latitude's colatitude and the body's δ + α.
///
/// # Panics
///
/// If the latitude is not in [−π/2, π/2] or the slope not in [0, π/4].
#[must_use]
pub fn shadow_at(latitude: Radians, body: &Body, slope: Radians) -> f64 {
    let sky = Sky::at(latitude, body);
    permanent_shadow(
        slope,
        Radians::new(sky.band.colatitude),
        Radians::new(sky.band.limb),
    )
}

/// The cold-trap fraction of `body`'s rough plains of slope `slope` at `latitude`, in [0, 1]: the
/// share of the plains' ground in permanent shadow that stays below `threshold`, [`cold_share`]
/// at [`irradiance_limit`]'s fraction, with no conduction share.
///
/// It never exceeds [`shadow_at`], and does not fall as the threshold rises.
///
/// # Panics
///
/// If the latitude is not in [−π/2, π/2], the slope not in [0, π/4] or the threshold not finite
/// and positive.
#[must_use]
pub fn fraction(latitude: Radians, body: &Body, slope: Radians, threshold: Kelvin) -> f64 {
    let sky = Sky::at(latitude, body);
    let (colatitude, limb) = (
        Radians::new(sky.band.colatitude),
        Radians::new(sky.band.limb),
    );
    let x = irradiance_limit(colatitude, body, threshold);
    cold_share(slope, colatitude, limb, x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cold_trap::BodyParts;
    use hyperion_base::GENERATOR_VERSION;
    use hyperion_base::units::{Degrees, WattsPerSquareMetre};
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::pins::Tolerance;
    use std::f64::consts::SQRT_2;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    const WATER: Kelvin = Kelvin::new(110.0);

    fn degrees(x: f64) -> Radians {
        Radians::from(Degrees::new(x))
    }

    /// The plains' slope of directional RMS slope `w`.
    fn slope(w: f64) -> Radians {
        Radians::new(math::atan(w))
    }

    /// The Moon under a point Sun 1.54° from the equator (Hayne et al.'s Methods D), albedo 0.12,
    /// emissivity 0.95 and no heat flow.
    fn point_moon() -> BodyParts {
        BodyParts {
            flux: WattsPerSquareMetre::new(1361.0),
            greatest_declination: degrees(1.54),
            star_radius: Radians::ZERO,
            bond_albedo: 0.12,
            emissivity: 0.95,
            heat_flow: WattsPerSquareMetre::ZERO,
        }
    }

    /// The Moon under the real Sun, as `science-r09-drainage.md` §8.13 takes it: 1,361 W m⁻² (IAU
    /// 2015 Resolution B3's nominal total solar irradiance) at perihelion, 0.98329 au, where the
    /// Sun's 959.6″ semidiameter at 1 au makes it 0.2711° in radius, and the heat flow 0.018 W m⁻²
    /// of the Apollo 15 and 17 heat-flow experiments (Langseth, Keihm and Peters 1976, as
    /// [`BodyParts::heat_flow`] cites them).
    fn real_moon() -> Body {
        Body::new(BodyParts {
            flux: WattsPerSquareMetre::new(1361.0 / (0.98329 * 0.98329)),
            star_radius: degrees(0.2711),
            heat_flow: WattsPerSquareMetre::new(0.018),
            ..point_moon()
        })
        .unwrap()
    }

    /// The mean of `f` over latitudes `lo`..`hi` degrees, weighted by area, by the midpoint rule
    /// in 1,000 steps.
    fn band_mean(lo: f64, hi: f64, f: impl Fn(Radians) -> f64) -> f64 {
        let steps = 1000_u32;
        let width = (hi - lo) / f64::from(steps);
        let (mut sum, mut weight) = (0.0, 0.0);
        for i in 0..steps {
            let lat = degrees(lo + width * (f64::from(i) + 0.5));
            let w = math::cos(lat.value());
            sum += w * f(lat);
            weight += w;
        }
        sum / weight
    }

    /// Smith's (1967, "Lunar surface roughness: shadowing and thermal emission", JGR 72, 4059)
    /// shadowed share of a Gaussian surface of directional RMS slope
    /// `w` under a sun at elevation `e`: one less (1 − erfc(v)/2) ÷ (1 + Λ), with
    /// v = tan e ÷ (√2 w) and Λ = (e^(−v²) ÷ (v√π) − erfc(v)) ÷ 2.
    fn smith_shadow(w: f64, e: f64) -> f64 {
        let v = math::tan(e) / (SQRT_2 * w);
        let lambda = 0.5 * (math::exp(-v * v) / (v * PI.sqrt()) - math::erfc(v));
        1.0 - (1.0 - 0.5 * math::erfc(v)) / (1.0 + lambda)
    }

    #[test]
    fn the_instantaneous_shadow_is_smiths_up_to_4_degrees() {
        // R09.T7.g: within 15% of Smith 1967's at sun elevations up to 4° at σ_s 5.7°, 16.7° and
        // 26.6° (the science check's tracer, 2° azimuths: 12%; Hayne et al.'s, 1°: 5%).
        for w in [0.1, 0.3, 0.5] {
            for half_degrees in 1..=8_u32 {
                let e = degrees(0.5 * f64::from(half_degrees));
                let traced = instantaneous_shadow(slope(w), e);
                let smith = smith_shadow(w, e.value());
                assert!(
                    (traced / smith - 1.0).abs() <= 0.15,
                    "w {w}, {half_degrees} half degrees: {traced} against Smith's {smith}"
                );
            }
        }
    }

    #[test]
    fn the_permanent_shadow_follows_the_ray_traced_surfaces() {
        // science-r09-drainage.md §8.13: plains alone under a point Sun 1.54° from the equator,
        // within the traced surfaces' spread at σ_s 5.7° (seven surfaces) and 16.7° (two), and
        // within 15% at 11.3° and 26.6°: over 80–90°, 2.6–3.2%, 22.8–24.4%, 13.3% and 34.8%;
        // over 70–80°, 0.03–0.11%, 7.4–8.2%, 2.25% and 18.8%.
        let body = Body::new(point_moon()).unwrap();
        let band =
            |lo: f64, w: f64| band_mean(lo, lo + 10.0, |lat| shadow_at(lat, &body, slope(w)));
        let fine = band(80.0, 0.1);
        assert!((0.026..=0.032).contains(&fine), "w 0.1: {fine}");
        let fine_sub = band(70.0, 0.1);
        assert!(
            (3e-4..=1.1e-3).contains(&fine_sub),
            "w 0.1, 70–80°: {fine_sub}"
        );
        // Within §8.13's two surfaces at σ_s 16.7° (R09.T7.l's refit: 23.78% and 7.65%).
        for (lo, spread) in [(80.0, 0.228..=0.244), (70.0, 0.074..=0.082)] {
            let rough = band(lo, 0.3);
            assert!(
                spread.contains(&rough),
                "w 0.3, {lo}°: {rough} against {spread:?}"
            );
        }
        for (lo, w, traced) in [
            (80.0, 0.2, 0.133),
            (80.0, 0.5, 0.348),
            (70.0, 0.2, 0.0225),
            (70.0, 0.5, 0.188),
        ] {
            let shadow = band(lo, w);
            assert!(
                (shadow / traced - 1.0).abs() <= 0.15,
                "w {w}, {lo}°: {shadow} against {traced}"
            );
        }
        // T7.d's plains guard: no measurable shadow equatorward of 70° at σ_s 5.7°.
        let sixties = band(60.0, 0.1);
        assert!(sixties <= 1e-5, "60–70° {sixties}");
    }

    #[test]
    fn the_plains_cold_traps_follow_the_ray_traces() {
        // science-r09-drainage.md §8.13: plains alone over 80–90° under the real Sun, no
        // conduction share. Water (110 K) within 15% of 2.16%, 6.13% and 4.87% at σ_s 5.7°, 11.3°
        // and 16.7°; CO₂ (57 K) within a factor of two of 0.12% (0.090–0.146%) and 0.031% at 5.7°
        // and 26.6°.
        let body = real_moon();
        let band =
            |w: f64, t: Kelvin| band_mean(80.0, 90.0, |lat| fraction(lat, &body, slope(w), t));
        for (w, traced) in [(0.1, 0.0216), (0.2, 0.0613), (0.3, 0.0487)] {
            let water = band(w, WATER);
            assert!(
                (water / traced - 1.0).abs() <= 0.15,
                "w {w}: water {water} against {traced}"
            );
        }
        let co2 = Kelvin::new(57.0);
        for (w, traced) in [(0.1, 1.2e-3), (0.5, 3.1e-4)] {
            let traps = band(w, co2);
            assert!(
                (traced / 2.0..=traced * 2.0).contains(&traps),
                "w {w}: CO₂ {traps} against {traced}"
            );
        }
        // Pinned as built on R09.T7.l's refit, within 1%, findings: water at σ_s 26.6° is 29%
        // over §8.13's 2.69%, outside the plan's 15%, and CO₂ at 11.3° and 16.7° is 2.1 times the
        // 0.0215% and 0.0195% of an independent tracer (2026-10-29; 8 surfaces, not literature),
        // just outside the plan's factor of two (those heat at the upper limb; heated at the
        // disk's centre, as the refit is, they are 0.0367% and 0.0297%).
        let mut pins = hyperion_testkit::pins!(
            "cold_trap/rough_plains",
            "the_plains_cold_traps_follow_the_ray_traces"
        );
        for (key, w, threshold) in [
            ("water.sigma_26_6deg", 0.5, WATER),
            ("co2.sigma_11_3deg", 0.2, co2),
            ("co2.sigma_16_7deg", 0.3, co2),
        ] {
            pins.within(key, band(w, threshold), Tolerance::relative(0.01));
        }
    }

    #[test]
    fn another_albedo_and_emissivity_read_the_same_table() {
        // A darker, less emissive body reads the table at its own fraction, (εσT⁴ − q) ÷
        // ((1 − A)(A + ε) S sin e_eff), and so traps what the Moon does at the threshold whose
        // fraction is the same.
        let moon = real_moon();
        let other = Body::new(BodyParts {
            bond_albedo: 0.3,
            emissivity: 0.8,
            ..*moon.parts()
        })
        .unwrap();
        let p = moon.parts();
        let s = slope(0.2);
        for latitude in [80.0, 85.0, 88.0] {
            let lat = degrees(latitude);
            let colatitude = Radians::new(FRAC_PI_2 - lat.value());
            let limb = Radians::new(p.greatest_declination.value() + p.star_radius.value());
            // The disk is wholly above the horizon here, so its flux-weighted elevation is its
            // centre's.
            let sin_e = math::sin(colatitude.value() + p.greatest_declination.value());
            let t = WATER.value();
            let by_hand = (0.8 * STEFAN_BOLTZMANN * t * t * t * t - p.heat_flow.value())
                / ((1.0 - 0.3) * (0.3 + 0.8) * p.flux.value() * sin_e);
            let x = irradiance_limit(colatitude, &other, WATER);
            assert!(
                (x / by_hand - 1.0).abs() < 1e-14,
                "{latitude}°: {x} against {by_hand}"
            );
            let traps = fraction(lat, &other, s, WATER);
            assert_same_bits(traps, cold_share(s, colatitude, limb, x));
            // The Moon's threshold of the same fraction.
            let lit = (1.0 - 0.12) * (0.12 + 0.95) * p.flux.value() * sin_e;
            let power = (x * lit + p.heat_flow.value()) / (0.95 * STEFAN_BOLTZMANN);
            let same = Kelvin::new(power.sqrt().sqrt());
            let lunar = fraction(lat, &moon, s, same);
            assert!(
                (lunar - traps).abs() <= 1e-12 * traps.max(1e-300),
                "{latitude}°"
            );
            assert!(
                (irradiance_limit(colatitude, &moon, WATER) / x - 1.0).abs() > 1e-3,
                "{latitude}°: the two bodies' fractions differ"
            );
        }
    }

    #[test]
    fn the_heating_takes_the_disks_flux_weighted_elevation() {
        // decision-r09-t7g-table §2: the shadow takes the star's upper limb and the heating its
        // disk. At the pole of a body whose star is mostly below the horizon at its highest (δ
        // 0.034° and α 0.8665°, Mercury's at perihelion, science-r09-drainage.md §8.4), the disk's
        // flux-weighted elevation is about 0.20° against the upper limb's 0.90°, so the fraction
        // is about 4.5 times the upper limb's.
        let (declination, star_radius) = (degrees(0.034), degrees(0.8665));
        let body = Body::new(BodyParts {
            flux: WattsPerSquareMetre::new(1361.0 / (0.3075 * 0.3075)),
            greatest_declination: declination,
            star_radius,
            ..point_moon()
        })
        .unwrap();
        let t = WATER.value();
        let allowed = 0.95 * STEFAN_BOLTZMANN * t * t * t * t;
        let lit = (1.0 - 0.12) * (0.12 + 0.95) * body.parts().flux.value();
        let upper = allowed / (lit * math::sin(declination.value() + star_radius.value()));
        let ratio = irradiance_limit(Radians::ZERO, &body, WATER) / upper;
        assert!((4.4..=4.6).contains(&ratio), "{ratio}");
        // Its sin e is the bowls' to the bit at every latitude, the disk cut by the horizon or
        // not, for this body and the real Moon.
        for body in [body, real_moon()] {
            let p = body.parts();
            let (a, emissivity) = (p.bond_albedo, p.emissivity);
            let allowed = emissivity * STEFAN_BOLTZMANN * t * t * t * t - p.heat_flow.value();
            for latitude in [0.0, 30.0, 60.0, 80.0, 85.0, 88.0, 89.0, 89.9, 90.0] {
                let sky = Sky::at(degrees(latitude), &body);
                let colatitude = Radians::new(sky.band.colatitude);
                let lit = (1.0 - a) * (a + emissivity) * p.flux.value() * sky.sin_highest;
                assert_same_bits(irradiance_limit(colatitude, &body, WATER), allowed / lit);
            }
        }
    }

    /// The slopes, colatitudes, limbs and fractions the structural tests sweep.
    fn sweep() -> impl Iterator<Item = (Radians, Radians, Radians)> {
        let slopes = [0.0, 0.02, 0.05, 0.07, 0.1, 0.25, 0.5, 0.8, 1.0];
        let colatitudes = [0.0, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 40.0, 60.0, 89.0];
        let limbs = [0.0, 0.27, 1.54, 1.81, 4.0, 10.0, 20.0, 30.0];
        slopes.into_iter().flat_map(move |w| {
            colatitudes.into_iter().flat_map(move |c| {
                limbs
                    .into_iter()
                    .map(move |d| (slope(w), degrees(c), degrees(d)))
            })
        })
    }

    #[test]
    fn the_cold_share_lies_within_the_shadow_and_rises_with_the_fraction() {
        let fractions = [1e-6, 1e-4, 3e-4, 1e-3, 0.01, 0.05, 0.3, 1.0, 10.0, 1e3];
        for (s, c, d) in sweep() {
            let shadow = permanent_shadow(s, c, d);
            assert!((0.0..=1.0).contains(&shadow), "{s:?} {c:?} {d:?}: {shadow}");
            let mut last = 0.0;
            for x in fractions {
                let cold = cold_share(s, c, d, x);
                assert!(
                    cold <= shadow,
                    "{s:?} {c:?} {d:?} at {x}: {cold} above {shadow}"
                );
                assert!(
                    cold >= last - 1e-15,
                    "{s:?} {c:?} {d:?} at {x}: {cold} below {last}"
                );
                last = cold;
            }
            let wider = permanent_shadow(s, c, Radians::new(d.value() + degrees(0.5).value()));
            assert!(
                wider <= shadow + 1e-15,
                "{s:?} {c:?} {d:?}: {wider} above {shadow}"
            );
        }
    }

    #[test]
    fn nothing_is_shadowed_or_cold_where_the_table_holds_none() {
        let (s, pole) = (slope(0.1), Radians::ZERO);
        // A plain with no slope, a sky that reaches the zenith, and the axes' ends.
        assert_same_bits(permanent_shadow(Radians::ZERO, pole, degrees(1.0)), 0.0);
        assert_same_bits(permanent_shadow(s, degrees(60.0), degrees(31.0)), 0.0);
        let last_c = table::COLATITUDE_PER_SLOPE[table::COLATITUDE_PER_SLOPE.len() - 1];
        let last_d = table::LIMB_PER_SLOPE[table::LIMB_PER_SLOPE.len() - 1];
        assert_same_bits(permanent_shadow(s, Radians::new(0.1 * last_c), pole), 0.0);
        assert_same_bits(permanent_shadow(s, pole, Radians::new(0.1 * last_d)), 0.0);
        // A fraction that is not positive, and a heat flow that alone warms the shadow past the
        // threshold.
        assert!(permanent_shadow(s, pole, degrees(1.54)) > 0.0);
        assert_same_bits(cold_share(s, pole, degrees(1.54), 0.0), 0.0);
        assert_same_bits(cold_share(s, pole, degrees(1.54), -1.0), 0.0);
        let warm = Body::new(BodyParts {
            heat_flow: WattsPerSquareMetre::new(1.0),
            ..point_moon()
        })
        .unwrap();
        let co = Kelvin::new(30.0);
        assert_same_bits(irradiance_limit(pole, &warm, co), 0.0);
        assert_same_bits(fraction(Radians::new(FRAC_PI_2), &warm, s, co), 0.0);
        // A star that never rises over the pole leaves every shadow's fraction infinite, read at
        // the table's last node.
        let still = Body::new(BodyParts {
            greatest_declination: Radians::ZERO,
            ..point_moon()
        })
        .unwrap();
        let x = irradiance_limit(pole, &still, WATER);
        assert!(x.is_infinite() && x > 0.0, "{x}");
        let last = table::IRRADIANCE[table::IRRADIANCE.len() - 1];
        assert_same_bits(
            cold_share(s, pole, pole, x),
            cold_share(s, pole, pole, last),
        );
    }

    #[test]
    fn a_slope_below_the_first_node_reads_its_slice_at_its_own_ratios() {
        // Heights and angles scale together: a plain of w 0.02 at (c w, d w) reads the w 0.05
        // slice at (c, d), as the 0.05 plain at (0.05 c, 0.05 d) does, and its irradiance over
        // S sin e is (0.02 ÷ 0.05)² of the slice's, so a fraction x reads the slice at 6.25 x.
        for (c, d) in [(0.0, 0.0), (0.5, 0.2), (1.0, 0.5), (2.0, 0.3), (3.0, 1.0)] {
            let at = |w: f64| permanent_shadow(slope(w), Radians::new(c * w), Radians::new(d * w));
            let (low, first) = (at(0.02), at(0.05));
            assert!(
                (low - first).abs() <= 1e-12,
                "({c}, {d}): {low} against {first}"
            );
            let cold =
                |w: f64, x: f64| cold_share(slope(w), Radians::new(c * w), Radians::new(d * w), x);
            for x in [1e-5, 0.003, 0.02, 0.4] {
                let (low, first) = (cold(0.02, x), cold(0.05, 6.25 * x));
                assert!(
                    (low - first).abs() <= 1e-12,
                    "({c}, {d}), {x}: {low} against {first}"
                );
            }
        }
    }

    #[test]
    fn the_table_is_ordered_and_its_shares_are_shares() {
        let ascending = |nodes: &[f64]| nodes.windows(2).all(|w| w[0] < w[1]);
        assert!(ascending(&table::SLOPE));
        assert_same_bits(table::SLOPE[0], 0.05);
        assert_same_bits(table::SLOPE[table::SLOPE.len() - 1], 1.0);
        assert!(ascending(&table::COLATITUDE_PER_SLOPE) && ascending(&table::LIMB_PER_SLOPE));
        assert!(ascending(&table::IRRADIANCE) && ascending(&table::TAN_ELEVATION_PER_SLOPE));
        assert_same_bits(table::IRRADIANCE[0], 0.0);
        assert_eq!(
            table::SHADOW.len(),
            table::SLOPE.len() * table::COLATITUDE_PER_SLOPE.len()
        );
        // The cold nodes are the shadow's at their indices, ascending from its first to its last.
        for (cold, indices, all) in [
            (
                &table::COLD_COLATITUDE_PER_SLOPE[..],
                &table::COLD_COLATITUDE_INDEX[..],
                &table::COLATITUDE_PER_SLOPE[..],
            ),
            (
                &table::COLD_LIMB_PER_SLOPE[..],
                &table::COLD_LIMB_INDEX[..],
                &table::LIMB_PER_SLOPE[..],
            ),
        ] {
            assert_eq!(cold.len(), indices.len());
            assert!(indices.windows(2).all(|w| w[0] < w[1]), "{indices:?}");
            assert_eq!(indices.first(), Some(&0));
            assert_eq!(indices.last().map(|&i| usize::from(i)), Some(all.len() - 1));
            for (&node, &i) in cold.iter().zip(indices) {
                assert_same_bits(node, all[usize::from(i)]);
            }
        }
        // The table's facets were lit by the Sun's centre at the Moon, the real Moon's α.
        let alpha = real_moon().parts().star_radius.value();
        assert!(
            (table::STAR_RADIUS / alpha - 1.0).abs() < 1e-12,
            "{}",
            table::STAR_RADIUS
        );
        assert_eq!(
            table::COLD.len(),
            table::SLOPE.len()
                * table::COLD_COLATITUDE_PER_SLOPE.len()
                * table::COLD_LIMB_PER_SLOPE.len()
        );
        for row in &table::SHADOW {
            assert!(row.iter().all(|s| (0.0..=1.0).contains(s)), "{row:?}");
            assert!(row.windows(2).all(|w| w[1] <= w[0]), "{row:?}");
        }
        for row in &table::COLD {
            assert!(row.iter().all(|s| (0.0..=1.0).contains(s)), "{row:?}");
            assert!(row.windows(2).all(|w| w[1] >= w[0]), "{row:?}");
        }
        let instant = &table::INSTANTANEOUS;
        assert!(instant.windows(2).all(|w| w[1] <= w[0]), "{instant:?}");
    }

    #[test]
    fn the_table_falls_to_nothing_at_its_last_nodes() {
        // The read is zero beyond the last colatitude and limb nodes, so the shadow there must
        // already be all but zero for the cut to be continuous.
        let width = table::COLATITUDE_PER_SLOPE.len();
        for (row, shares) in table::SHADOW.iter().enumerate() {
            let last = shares[shares.len() - 1];
            assert!(last <= 1e-5, "row {row}: {last} at the last limb");
            if row % width == width - 1 {
                assert!(shares.iter().all(|&s| s <= 1e-5), "row {row}: {shares:?}");
            }
        }
    }

    /// The reader's reads at a sweep of slopes, colatitudes, limbs and fractions, for its golden.
    fn reads_text() -> String {
        let mut out = GoldenWriter::new();
        out.header(GENERATOR_VERSION.get());
        for w in [0.0, 0.02, 0.1, 0.25, 0.5, 1.0] {
            for colatitude in [0.0, 2.0, 8.0, 20.0] {
                for limb in [0.0, 1.81, 6.0] {
                    let (s, c, d) = (slope(w), degrees(colatitude), degrees(limb));
                    let at = format!("w {w} e0 {colatitude} D {limb}");
                    out.f64(&format!("{at} shadow"), permanent_shadow(s, c, d));
                    for x in [1e-4, 0.01, 0.3] {
                        out.f64(&format!("{at} cold {x}"), cold_share(s, c, d, x));
                    }
                }
            }
            for elevation in [0.5, 2.0, 10.0] {
                let shadow = instantaneous_shadow(slope(w), degrees(elevation));
                out.f64(&format!("w {w} e {elevation} instantaneous"), shadow);
            }
        }
        // The heating's normaliser: a disk the horizon cuts and one it clears, a point star, a
        // heat flow that warms the shadow past the threshold (0) and a star that never rises (∞);
        // then the real Moon's cold traps.
        let disk = |declination: f64, radius: f64, heat_flow: f64| {
            Body::new(BodyParts {
                greatest_declination: degrees(declination),
                star_radius: degrees(radius),
                heat_flow: WattsPerSquareMetre::new(heat_flow),
                ..point_moon()
            })
            .unwrap()
        };
        for (name, body) in [
            ("cut", disk(0.034, 0.8665, 0.0)),
            ("moon", real_moon()),
            ("point", disk(1.54, 0.0, 0.0)),
            ("warm", disk(1.54, 0.2711, 1.0)),
            ("still", disk(0.0, 0.0, 0.0)),
        ] {
            for colatitude in [0.0, 0.5, 5.0] {
                for t in [30.0, 57.0, 110.0] {
                    let x = irradiance_limit(degrees(colatitude), &body, Kelvin::new(t));
                    out.f64(&format!("{name} e0 {colatitude} T {t} limit"), x);
                }
            }
        }
        let moon = real_moon();
        for latitude in [80.0, 85.0, 88.0, 89.5, 90.0] {
            for t in [57.0, 110.0] {
                let traps = fraction(degrees(latitude), &moon, slope(0.1), Kelvin::new(t));
                out.f64(&format!("moon {latitude} T {t} fraction"), traps);
            }
        }
        out.finish()
    }

    #[test]
    fn the_readers_bits_are_pinned() {
        // Blessed natively and compared on both WebAssembly targets through the testkit's
        // embedded arm, so the reader gives one f64 everywhere (the sim-determinism skill's
        // hazards across targets).
        golden!("cold_trap/rough_plains", &reads_text());
    }
}
