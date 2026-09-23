//! The dust lanes: the young arms' sharp profile again, at a radius shifted outward so that the
//! lanes lie inside the arms (plan 07, Design note 6).
//!
//! The lane factor on the neutral layer is `lane(R, θ) = S(R + d ÷ cos p, θ)`, with `S` plan 02's
//! sharp arm factor `1 + f(R) A (g − 1)` built on the galaxy's own [`ArmGeometry`] with the lanes'
//! own width `σ_w` and share A ([`SharpArm::new`]), `d` the lanes' perpendicular offset and `p` the
//! arms' pitch. Reading the arm pattern at a larger radius puts the pattern's ridge at a smaller
//! one: at every azimuth the lane's ridge lies `δR = d ÷ cos p` inside the young arm's, which is a
//! perpendicular distance `d` across a logarithmic spiral of pitch `p`, and the same as the phase
//! offset `n δR ÷ (R tan p)` the brainstorm asks for, with the offset fixed in light-years rather
//! than in phase for the reason the brainstorm gives for arm widths. The gas overtakes the trailing
//! arms inside corotation and shocks on their inner, concave edges, which is why the offset is
//! inward.
//!
//! Because `S` averages exactly 1 around the circle of radius `R + δR` — its profile `g` does for
//! every concentration `k`, and the fade-in `f` is constant on the circle — the lane factor averages
//! exactly 1 around the circle of radius `R`, and the lanes move no gas between radii. Between the
//! arms, where `g` vanishes, the factor is `1 − f A`; on a ridge it is `1 + f A (1 ÷ I₀ₑ(k) − 1)`,
//! about 3 at 26,000 ly for the Milky Way fixture. Inside the bar the fade-in switches it off.
//!
//! Radii are light-years and azimuths radians from +x, counter-clockwise, as plan 02's arms have
//! them.

use crate::galaxy::bounds::{CellBox, ScalarRange};
use crate::galaxy::fields::arms::{ArmGeometry, SharpArm};
use crate::galaxy::gas::params::LaneParams;
use crate::math;
use crate::units::LightYears;

/// The lane factor of one galaxy: plan 02's sharp arm with the lanes' width and share, read at a
/// radius shifted by `d ÷ cos p`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::arms::ArmGeometry;
/// use hyperion_sim::galaxy::gas::lanes::Lanes;
/// use hyperion_sim::galaxy::gas::params::GasParams;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let geometry = ArmGeometry::of(&GalaxyParams::milky_way_like());
/// let gas = GasParams::milky_way_like();
/// let lanes = Lanes::new(geometry, gas.lane());
/// // At 26,000 ly the young arm's ridge is at this azimuth; the lane's lies 460 ly further in.
/// let r = 26_000.0;
/// let theta = geometry.ridge_azimuth(r, 0);
/// let in_lane = lanes.factor(r - lanes.shift().value(), theta);
/// assert!((2.5..3.5).contains(&in_lane));
/// // Between the arms the lanes leave 1 − A of the neutral gas.
/// let between = lanes.factor(r, theta + 0.25 * std::f64::consts::PI);
/// assert!((between - (1.0 - gas.lane().fraction())).abs() < 0.01);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lanes {
    arm: SharpArm,
    /// `δR = d ÷ cos p`, ly: how much further out than the point the arm pattern is read.
    shift: f64,
}

impl Lanes {
    /// The lanes of `lane` on the arms `geometry`.
    ///
    /// # Panics
    ///
    /// Never: [`LaneParams`] holds a width of 150–300 ly and a share of 0.08–0.20, which
    /// [`SharpArm::new`] accepts, and an arm's pitch lies strictly between 0 and π ÷ 2, so its
    /// cosine is positive.
    #[must_use]
    pub fn new(geometry: ArmGeometry, lane: LaneParams) -> Self {
        let arm = SharpArm::new(geometry, lane.width(), lane.fraction())
            .expect("a lane's width is 150–300 ly and its share 0.08–0.20");
        Self {
            arm,
            shift: lane.offset().value() / math::cos(geometry.pitch().value()),
        }
    }

    /// The sharp arm the lanes are made of: the galaxy's geometry with the lanes' width and share.
    #[must_use]
    pub fn arm(&self) -> &SharpArm {
        &self.arm
    }

    /// The radial shift `δR = d ÷ cos p`: how far inside the young arm's ridge, at the same
    /// azimuth, the lane's ridge lies.
    #[must_use]
    pub fn shift(&self) -> LightYears {
        LightYears::new(self.shift)
    }

    /// The lane factor at cylindrical radius `r ≥ 0` (ly) and azimuth `theta` (radians), which
    /// multiplies the neutral layer's density there: `S(r + δR, θ)`.
    ///
    /// It averages exactly 1 around every circle, lies between `1 − A` and the ridge's value, and is
    /// 1 to within 10⁻³ inside half the bar's half-length, where the arms' fade-in is off. At
    /// `r = 0`, where the azimuth is undefined, it reads the pattern at `δR` and whatever azimuth
    /// it is given, which the fade-in holds within 10⁻⁸ of 1.
    #[must_use]
    pub fn factor(&self, r: f64, theta: f64) -> f64 {
        let at = self.arm.geometry().point_polar(r + self.shift, theta);
        self.arm.factor(&at)
    }

    /// An upper bound on the lane factor at every point of `cell` (plan 07, P07.T6.b).
    ///
    /// The factor at `(R, θ)` is the sharp arm at `(R + δR, θ)`, so it is bounded by
    /// [`SharpArm::sup`] over the shifted radii and the phase the pattern takes at them. That phase,
    /// `φ(R + δR, θ) = n (θ + ln((R + δR) ÷ L) ÷ tan p)`, has an in-plane gradient of magnitude
    /// `n √(cot² p ÷ (R + δR)² + 1 ÷ R²)`, below the unshifted phase's `n ÷ (R sin p)` since
    /// `δR > 0`, so the half-width [`ArmGeometry::phase_range`] takes about the cell's centre
    /// bounds it too: the range is the shifted phase at the centre ± `n` × the cell's in-plane
    /// half-diagonal ÷ (`R_min` sin p), every phase if `R_min` is 0 or the half-width reaches π.
    /// [`SharpArm::sup`] carries plan 02's margins (`BOUND_MARGIN`, `COS_SLACK`,
    /// `ROUNDING_SLACK`), which cover the rounding of `R + δR` and of the phase as the factor
    /// computes them.
    #[must_use]
    pub fn sup(&self, cell: &CellBox) -> f64 {
        let radii = cell.r_cyl_range();
        let shifted = ScalarRange::new(radii.lo + self.shift, radii.hi + self.shift);
        self.arm.sup(shifted, self.phase_range(cell))
    }

    /// The range of the shifted pattern's phase over `cell`: [`sup`](Self::sup)'s documentation.
    fn phase_range(&self, cell: &CellBox) -> ScalarRange {
        let every = ScalarRange::new(f64::NEG_INFINITY, f64::INFINITY);
        let geometry = self.arm.geometry();
        let r_min = cell.r_cyl_range().lo;
        if r_min <= 0.0 {
            return every;
        }
        let half_width = geometry.phase_rate(r_min) * cell.in_plane_half_diagonal();
        if half_width >= core::f64::consts::PI {
            return every;
        }
        let centre = cell.centre();
        let r = (centre.x * centre.x + centre.y * centre.y).sqrt();
        let phase = geometry.phase_polar(r + self.shift, math::atan2(centre.y, centre.x));
        ScalarRange::new(phase - half_width, phase + half_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::gas::params::GasParams;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;

    const TAU: f64 = core::f64::consts::TAU;

    /// The Milky Way fixture's lanes on the fixture's arms: four arms at 12°, lanes 450 ly inside
    /// them, 200 ly wide, gathering 12% of the neutral gas.
    fn milky_way() -> (ArmGeometry, GasParams, Lanes) {
        let geometry = ArmGeometry::of(&GalaxyParams::milky_way_like());
        let gas = GasParams::milky_way_like();
        (geometry, gas, Lanes::new(geometry, gas.lane()))
    }

    /// The lanes of `count` seeds' own galaxies, so that both arm counts, the whole range of
    /// pitches and bars, and the whole range of the lanes' own offset, width and share are seen.
    fn drawn(count: u64) -> Vec<(ArmGeometry, GasParams, Lanes)> {
        (0..count)
            .map(|n| {
                let seed = Seed::new(0x0700_5eed_1a4e_0000 | n);
                let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
                let geometry = ArmGeometry::of(&galaxy);
                let gas = GasParams::from_galaxy(seed, &galaxy).unwrap();
                (geometry, gas, Lanes::new(geometry, gas.lane()))
            })
            .collect()
    }

    /// The mean of the factor around the circle of radius `r`, by an `n`-point sum over equally
    /// spaced azimuths: the trapezoid rule of a periodic function, which converges geometrically.
    fn azimuthal_mean(lanes: &Lanes, r: f64, n: u32) -> f64 {
        let sum: f64 = (0..n)
            .map(|j| lanes.factor(r, TAU * f64::from(j) / f64::from(n)))
            .sum();
        sum / f64::from(n)
    }

    /// The lanes move no gas: around each of twenty circles out to the root cube's edge, and inside
    /// the bar, the factor averages 1 to 10⁻⁹, for the fixture and for sixteen drawn galaxies.
    ///
    /// The narrowest lane on the widest circle, 150 ly at 65,000 ly with four arms, spans about
    /// 0.011 rad of azimuth, so 16,384 azimuths put some thirty samples across its width, where
    /// the trapezoid rule is exact far past 10⁻⁹.
    #[test]
    fn the_lane_factor_averages_one_around_every_circle() {
        let mut all = vec![milky_way()];
        all.extend(drawn(16));
        for (_, gas, lanes) in all {
            for i in 1..=20 {
                let r = 65_000.0 * f64::from(i) / 20.0;
                let mean = azimuthal_mean(&lanes, r, 16_384);
                assert!(
                    (mean - 1.0).abs() < 1e-9,
                    "at {r} ly the lanes of {:?} average {mean}",
                    gas.lane()
                );
            }
        }
    }

    /// The radius near `guess` (ly) at which `factor(·, theta)` peaks, found by a scan in steps of
    /// one light-year over ±1,000 ly: a resolution of a few tenths of a per cent of the offsets.
    fn peak_radius(factor: impl Fn(f64) -> f64, guess: f64) -> f64 {
        let mut best = (f64::NEG_INFINITY, guess);
        for i in -1_000..=1_000 {
            let r = guess + f64::from(i);
            let value = factor(r);
            if value > best.0 {
                best = (value, r);
            }
        }
        best.1
    }

    /// At a fixed azimuth the lane's ridge lies inside the young arm's by `d ÷ cos p`, to 5%: each
    /// ridge found as the peak of its own factor along the radius, for the fixture and for drawn
    /// galaxies whose offsets span 300–600 ly.
    #[test]
    fn the_lane_ridge_lies_inside_the_arm_ridge_by_the_offset() {
        let mut all = vec![milky_way()];
        all.extend(drawn(8));
        for (geometry, gas, lanes) in all {
            let young = SharpArm::new(geometry, LightYears::new(300.0), 0.8).unwrap();
            let expected = gas.lane().offset().value() / math::cos(geometry.pitch().value());
            assert!((lanes.shift().value() / expected - 1.0).abs() < 1e-15);
            for r in [20_000.0, 26_000.0, 40_000.0] {
                for j in 0..geometry.count().get() {
                    let theta = geometry.ridge_azimuth(r, j);
                    let arm_ridge =
                        peak_radius(|s| young.factor(&geometry.point_polar(s, theta)), r);
                    let lane_ridge =
                        peak_radius(|s| lanes.factor(s, theta), r - lanes.shift().value());
                    let apart = arm_ridge - lane_ridge;
                    assert!(
                        (apart / expected - 1.0).abs() < 0.05,
                        "at {r} ly, arm {j}: the lane lies {apart} ly inside the arm, not {expected}"
                    );
                }
            }
        }
    }

    /// Well inside the bar, where the arms have not started, the lane factor is 1 at every azimuth:
    /// within 10⁻³ inside half the bar's half-length. The worst case is the shortest bar with the
    /// largest shift, 630 ly, where the fade-in reads the pattern at 0.56 of the half-length and is
    /// about 2 × 10⁻⁴; at a quarter of it the factor is 1 to 10⁻⁶.
    #[test]
    fn the_lanes_are_off_inside_the_bar() {
        let mut all = vec![milky_way()];
        all.extend(drawn(16));
        for (geometry, _, lanes) in all {
            let half = 0.5 * geometry.bar_half_length().value();
            for i in 0..=16 {
                let r = half * f64::from(i) / 16.0;
                for j in 0..256 {
                    let theta = TAU * f64::from(j) / 256.0;
                    let factor = lanes.factor(r, theta);
                    let tolerance = if r <= 0.5 * half { 1e-6 } else { 1e-3 };
                    assert!(
                        (factor - 1.0).abs() < tolerance,
                        "{factor} at {r} ly inside a bar of {half} ly's half"
                    );
                }
            }
        }
    }

    /// Between the arms — half-way in phase between two ridges of the shifted pattern — the lanes
    /// leave `1 − A` of the neutral gas, to 1%, for the fixture at the Sun's radius and outward.
    #[test]
    fn between_the_arms_the_factor_is_one_less_the_lane_share() {
        let (geometry, gas, lanes) = milky_way();
        let fraction = gas.lane().fraction();
        let arms = f64::from(geometry.count().get());
        for r in [26_000.0, 35_000.0, 50_000.0] {
            let shifted = r + lanes.shift().value();
            for j in 0..geometry.count().get() {
                let theta = geometry.ridge_azimuth(shifted, j) + core::f64::consts::PI / arms;
                let factor = lanes.factor(r, theta);
                assert!(
                    (factor / (1.0 - fraction) - 1.0).abs() < 0.01,
                    "{factor} between the arms at {r} ly, against {}",
                    1.0 - fraction
                );
            }
        }
    }

    /// On a lane's ridge at the Sun's radius the fixture's neutral gas is 2.5–3.5 times its mean:
    /// `1 + A (1 ÷ I₀ₑ(k) − 1)` with `k` about 47 at the shifted radius.
    #[test]
    fn the_fixtures_lane_peaks_near_three_at_the_suns_radius() {
        let (geometry, _, lanes) = milky_way();
        let r = 26_000.0;
        let theta = geometry.ridge_azimuth(r + lanes.shift().value(), 0);
        let peak = lanes.factor(r, theta);
        assert!((2.5..=3.5).contains(&peak), "the lane's ridge is {peak}");
        // It is the greatest value around the circle.
        for j in 0..4_096 {
            let other = lanes.factor(r, TAU * f64::from(j) / 4_096.0);
            assert!(
                other <= peak * (1.0 + 1e-12),
                "{other} above the ridge's {peak}"
            );
        }
    }

    /// The bound over a cell holds at every point of it as the factor computes it, and near a ridge
    /// it is not far above the ridge's value.
    #[test]
    fn the_cell_bound_holds_at_every_point_of_the_cell() {
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x1a4e);
        let mut all = vec![milky_way()];
        all.extend(drawn(8));
        for (_, _, lanes) in all {
            for _ in 0..400 {
                let edge: u32 = if lcg.next_below(2) == 0 { 128 } else { 4_096 };
                let span = u64::from(65_536 / edge);
                let mut corner = || {
                    let i = i64::try_from(lcg.next_below(2 * span)).unwrap();
                    let span = i64::try_from(span).unwrap();
                    i32::try_from((i - span) * i64::from(edge)).unwrap()
                };
                let min = [corner(), corner(), 0];
                let cell = CellBox::new(min, edge).unwrap();
                let bound = lanes.sup(&cell);
                for _ in 0..64 {
                    let x = f64::from(min[0]) + f64::from(edge) * lcg.next_f64();
                    let y = f64::from(min[1]) + f64::from(edge) * lcg.next_f64();
                    let factor = lanes.factor((x * x + y * y).sqrt(), math::atan2(y, x));
                    assert!(factor <= bound, "{factor} above {bound} in {cell:?}");
                }
            }
        }
    }
}
