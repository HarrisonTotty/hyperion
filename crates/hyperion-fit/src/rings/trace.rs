//! The photon tracer of the shadowing table (plan R11, R11.T9.b; Design note 15): photons from a
//! star at elevation B′ walked through a frozen [`Field`], scattered by each particle they meet,
//! and their light towards each observer counted by scattering order.
//!
//! - **Scattering.** A particle scatters as a whole by Salo and French 2010's power law (Icarus
//!   210, 785, eq. 11), P(α) = `c_n` (π − α)^n with n = 3.09, Callisto's (Dones et al. 1993, Icarus
//!   105, 184), normalised so that (1 ÷ 4π) ∫ P dΩ = 1 ([`PowerLawPhase`]). The light leaves from
//!   the point where the photon struck the sphere, which keeps the opposition exact (the path out at
//!   α = 0 is the path in), and where the direction out points into the sphere, from that point
//!   mirrored in the plane normal to the direction, so that it leaves from a face the direction
//!   sees (a lean of this task, 2026-11-14: the whole-particle phase holds the particle's own
//!   shadowing, so only the other spheres may block the light). The single-scattering albedo is
//!   factored out: every photon is followed at ϖ = 1 until it leaves the layer, and its light is
//!   tallied by the order of the scattering it came from, so that I/F at any ϖ is `Σ_k` ϖ^k `I_k`.
//! - **The estimate.** At each scattering the light towards each observer is the phase at the
//!   observer's phase angle times 1 where the ray from the point of departure towards the observer
//!   leaves the layer unblocked, 0 where it does not (the next-event estimate of Salo and
//!   Karjalainen 2003, Icarus 164, 428, their "indirect" method). For N photons entering the box's
//!   area, I/F = μ₀ ÷ (4 μ N) × Σ P (the classical layer's ϖ P μ₀ ÷ (4(μ + μ₀)) × [1 − e^(−τ(1/μ +
//!   1/μ₀))] when the spheres are points), with μ₀ = sin B′ and μ = |sin B| the observer's.
//! - **Reproducible for any thread count**, by the crate's convention: photon i draws from its own
//!   counter-based stream, and the photons are tallied in blocks mapped by
//!   [`map_reduce_chunks`](crate::parallel::map_reduce_chunks) and merged in index order.

use core::f64::consts::PI;
use std::num::NonZeroUsize;

use hyperion_sim::math;

use super::layer::{Field, FieldKey, Flight};
use crate::atmosphere::geometry::{Vec3, turned};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::tasks::displaced_forms::births::Draws;

/// The power law's exponent for the main rings' particles: 3.09, Callisto's (Dones et al. 1993,
/// Icarus 105, 184; Salo and French 2010, §3.2).
pub const SALO_FRENCH_PHASE_EXPONENT: f64 = 3.09;

/// The particles' phase function P(α) = `c_n` (π − α)^n, normalised over 4π (Salo and French 2010,
/// eq. 11).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerLawPhase {
    exponent: f64,
    normalisation: f64,
}

impl PowerLawPhase {
    /// The power law of exponent `exponent`.
    ///
    /// # Panics
    ///
    /// If `exponent` is not finite and at least 0: below it the law cannot be normalised over 4π
    /// as this sums it, and at −2 or below its sampler would never accept.
    #[must_use]
    pub fn new(exponent: f64) -> Self {
        assert!(
            exponent.is_finite() && exponent >= 0.0,
            "a power-law phase's exponent is finite and at least 0, not {exponent}"
        );
        Self {
            exponent,
            normalisation: 2.0 / power_law_moment(exponent),
        }
    }

    /// `c_n` = 2 ÷ ∫₀^π u^n sin u du.
    #[cfg(test)]
    #[must_use]
    pub fn normalisation(&self) -> f64 {
        self.normalisation
    }

    /// P at phase angle `alpha_rad`, rad in [0, π].
    #[must_use]
    pub fn value(&self, alpha_rad: f64) -> f64 {
        let u = PI - alpha_rad.clamp(0.0, PI);
        if u <= 0.0 {
            return 0.0;
        }
        self.normalisation * math::powf(u, self.exponent)
    }

    /// A scattering angle θ = π − α drawn from the phase, rad: its density `c_n` θ^n sin θ ÷ 2 on
    /// [0, π], by rejection from θ^(n+1) (sin θ ≤ θ), exact.
    #[must_use]
    fn scattering_angle(&self, draws: &mut Draws) -> f64 {
        let inverse = 1.0 / (self.exponent + 2.0);
        loop {
            let theta = PI * math::powf(draws.uniform(), inverse);
            if draws.uniform() * theta < math::sin(theta) {
                return theta;
            }
        }
    }
}

/// ∫₀^π u^n sin u du, by its series `Σ_k` (−1)^k π^(n+2k+2) ÷ ((2k + 1)! (n + 2k + 2)), which converges
/// as π^(2k) ÷ (2k + 1)! does.
#[must_use]
fn power_law_moment(n: f64) -> f64 {
    let mut sum = 0.0;
    let mut term = math::powf(PI, n + 2.0);
    let mut k = 0.0;
    for even in [true, false].into_iter().cycle().take(60) {
        let value = term / (n + 2.0 * k + 2.0);
        sum += if even { value } else { -value };
        if value < 1e-18 * sum.abs() {
            break;
        }
        term *= PI * PI / ((2.0 * k + 2.0) * (2.0 * k + 3.0));
        k += 1.0;
    }
    sum
}

/// Which face of the ring an observer looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// The face the star lights.
    Lit,
    /// The face away from the star, lit by the light the layer lets through.
    Unlit,
}

/// An observer far off in a direction, with the star at elevation B′ and azimuth 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Observer {
    direction: Vec3,
    phase_rad: f64,
}

impl Observer {
    /// The observer on `face` at elevation `elevation_rad` (above the plane for the lit face, below
    /// it for the unlit) whose phase angle to a star at elevation `star_elevation_rad` is
    /// `phase_rad`, which must lie in the range the elevations allow; at the zenith, the zenith.
    #[must_use]
    pub fn at_phase(
        face: Face,
        elevation_rad: f64,
        star_elevation_rad: f64,
        phase_rad: f64,
    ) -> Self {
        let (sin_b, cos_b) = math::sin_cos(elevation_rad);
        let (sin_s, cos_s) = math::sin_cos(star_elevation_rad);
        let z = match face {
            Face::Lit => sin_b,
            Face::Unlit => -sin_b,
        };
        // cos α = cos B cos B′ cos Δφ + z sin B′.
        let across = cos_b * cos_s;
        let cos_dphi = if across > 1e-12 {
            ((math::cos(phase_rad) - z * sin_s) / across).clamp(-1.0, 1.0)
        } else {
            1.0
        };
        let sin_dphi = (1.0 - cos_dphi * cos_dphi).max(0.0).sqrt();
        let direction = Vec3::new(cos_b * cos_dphi, cos_b * sin_dphi, z);
        let star = Vec3::new(cos_s, 0.0, sin_s);
        Self {
            direction,
            phase_rad: math::acos(direction.dot(star).clamp(-1.0, 1.0)),
        }
    }

    /// The unit vector towards the observer.
    #[must_use]
    pub(crate) fn direction(&self) -> Vec3 {
        self.direction
    }

    /// Its phase angle to the star, rad, as its direction gives it.
    #[must_use]
    pub fn phase_rad(&self) -> f64 {
        self.phase_rad
    }

    /// μ = |sin B|.
    #[must_use]
    pub fn mu(&self) -> f64 {
        self.direction.z.abs()
    }
}

/// How a trace counts orders: the first alone (each photon stops after its first scattering), or
/// each order up to `buckets`, the last holding every order from it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orders {
    /// Single scattering only.
    First,
    /// Every order, in this many buckets (at least 1).
    Buckets(usize),
}

impl Orders {
    /// The buckets the tally holds.
    #[must_use]
    pub fn buckets(self) -> usize {
        match self {
            Self::First => 1,
            Self::Buckets(n) => n.max(1),
        }
    }

    /// The bucket of order `order`, at least 1.
    #[must_use]
    fn bucket(self, order: u32) -> usize {
        usize::try_from(order)
            .expect("orders fit a usize")
            .min(self.buckets())
            - 1
    }
}

/// The most scatterings a photon is followed through; one still in the layer then is counted
/// truncated. At ϖ ≤ 0.9 its light is below 0.9^2000 of the first order's.
pub const MAX_ORDER: u32 = 2_000;

/// The azimuth of the star's plane against the box's x axis, rad: 1 ÷ φ, so that no ray of the
/// tilt test runs along a row of the box's images, which would meet the same column of spheres at
/// every period.
const BOX_AZIMUTH_RAD: f64 = 0.618_033_988_749_894_8;

/// Photons a block of the map-reduce holds.
const BLOCK_PHOTONS: u64 = 256;

/// How far a point of departure is lifted off its sphere, m, so that its ray does not meet the
/// sphere it leaves.
const LIFT_M: f64 = 1e-7;

/// What a trace counted.
#[derive(Debug, Clone, PartialEq)]
pub struct Tally {
    photons: u64,
    buckets: usize,
    /// Σ P over the departures towards observer j in bucket b, at `j × buckets + b`.
    sums: Vec<f64>,
    /// Photons that struck a sphere.
    struck: u64,
    /// Scatterings in all.
    scatterings: u64,
    /// Photons lost in the layer's wings, or followed past [`MAX_ORDER`].
    lost: u64,
}

impl Tally {
    #[must_use]
    fn empty(observers: usize, buckets: usize) -> Self {
        Self {
            photons: 0,
            buckets,
            sums: vec![0.0; observers * buckets],
            struck: 0,
            scatterings: 0,
            lost: 0,
        }
    }

    /// Adds `other`'s counts to this one's.
    ///
    /// # Panics
    ///
    /// If the two counted other observers or buckets.
    pub fn add(&mut self, other: &Self) {
        assert_eq!(self.sums.len(), other.sums.len(), "tallies of one shape");
        self.photons += other.photons;
        self.struck += other.struck;
        self.scatterings += other.scatterings;
        self.lost += other.lost;
        for (a, b) in self.sums.iter_mut().zip(&other.sums) {
            *a += b;
        }
    }

    /// The photons traced.
    #[must_use]
    pub fn photons(&self) -> u64 {
        self.photons
    }

    /// The photons that struck a sphere.
    #[cfg(test)]
    #[must_use]
    pub fn struck(&self) -> u64 {
        self.struck
    }

    /// The scatterings in all.
    #[cfg(test)]
    #[must_use]
    pub fn scatterings(&self) -> u64 {
        self.scatterings
    }

    /// The photons lost in the wings or truncated.
    #[must_use]
    pub fn lost(&self) -> u64 {
        self.lost
    }

    /// I/F at ϖ = 1 towards `observer` (the `index`th of the trace's) of the orders in `bucket`
    /// (0 for the first), with the star at μ₀ = `mu0`: μ₀ ÷ (4 μ N) × Σ P.
    ///
    /// # Panics
    ///
    /// If `index` or `bucket` is out of range.
    #[must_use]
    pub fn reflectance(&self, mu0: f64, index: usize, observer: &Observer, bucket: usize) -> f64 {
        assert!(bucket < self.buckets, "bucket {bucket} of {}", self.buckets);
        #[expect(
            clippy::cast_precision_loss,
            reason = "photon counts are far below 2⁵³"
        )]
        let n = self.photons as f64;
        mu0 / (4.0 * observer.mu() * n) * self.sums[index * self.buckets + bucket]
    }
}

/// The point a scattering's light towards `direction` leaves from: the point struck, `struck` on
/// the sphere of `centre`, where the direction sees it, else that point mirrored in the plane
/// normal to the direction; lifted off the sphere by [`LIFT_M`].
#[must_use]
fn departure(centre: Vec3, radius_m: f64, struck: Vec3, direction: Vec3) -> Vec3 {
    let normal = (struck - centre) * (1.0 / radius_m);
    let facing = normal.dot(direction);
    let normal = if facing >= 0.0 {
        normal
    } else {
        (normal - direction * (2.0 * facing)).normalised()
    };
    centre + normal * (radius_m + LIFT_M)
}

/// Traces `photons` photons from a star at elevation `star_elevation_rad` through `field` and
/// tallies their light towards each of `observers` by order.
///
/// Photon i enters at the top of the layer at a point uniform over the box and draws from the
/// stream of `key` and i.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
pub fn trace(
    field: &Field,
    star_elevation_rad: f64,
    observers: &[Observer],
    photons: u64,
    orders: Orders,
    key: FieldKey,
    threads: NonZeroUsize,
) -> Result<Tally, BuildThreadPoolError> {
    let phase = PowerLawPhase::new(SALO_FRENCH_PHASE_EXPONENT);
    let buckets = orders.buckets();
    let (sin_s, cos_s) = math::sin_cos(star_elevation_rad);
    // The scene turned off the box's axes, so that no ray runs along a row of the box's images.
    let (sin_t, cos_t) = math::sin_cos(BOX_AZIMUTH_RAD);
    let turn = |v: Vec3| Vec3::new(cos_t * v.x - sin_t * v.y, sin_t * v.x + cos_t * v.y, v.z);
    let inward = turn(Vec3::new(-cos_s, 0.0, -sin_s));
    let directions: Vec<Vec3> = observers.iter().map(|o| turn(o.direction())).collect();
    let mut total = Tally::empty(observers.len(), buckets);
    map_reduce_chunks(
        photons,
        BLOCK_PHOTONS,
        threads,
        |range| {
            let mut tally = Tally::empty(observers.len(), buckets);
            for photon in range {
                walk(
                    field,
                    &phase,
                    inward,
                    &directions,
                    orders,
                    Draws::new(key.seed, key.key, photon),
                    &mut tally,
                );
            }
            tally
        },
        |block| total.add(&block),
    )?;
    Ok(total)
}

/// One photon's walk, from its entry at the top along `inward` until it leaves the layer, its
/// light tallied towards each of `observers`, unit vectors.
fn walk(
    field: &Field,
    phase: &PowerLawPhase,
    inward: Vec3,
    observers: &[Vec3],
    orders: Orders,
    mut draws: Draws,
    tally: &mut Tally,
) {
    let buckets = tally.buckets;
    tally.photons += 1;
    let l = field.box_m();
    let mut position = Vec3::new(l * draws.uniform(), l * draws.uniform(), field.top_m());
    let mut direction = inward;
    let mut order: u32 = 0;
    loop {
        let (distance_m, centre, radius_m) = match field.flight(position, direction) {
            Flight::Hit {
                distance_m,
                centre,
                radius_m,
            } => (distance_m, centre, radius_m),
            Flight::Top | Flight::Bottom => return,
            Flight::Lost => {
                tally.lost += 1;
                return;
            }
        };
        if order == 0 {
            tally.struck += 1;
        }
        order += 1;
        if order > MAX_ORDER {
            tally.lost += 1;
            return;
        }
        tally.scatterings += 1;
        let struck = position + direction * distance_m;
        let bucket = orders.bucket(order);
        for (j, &out) in observers.iter().enumerate() {
            let p = phase.value(math::acos((-direction.dot(out)).clamp(-1.0, 1.0)));
            if p > 0.0 && !field.blocked(departure(centre, radius_m, struck, out), out) {
                tally.sums[j * buckets + bucket] += p;
            }
        }
        if orders == Orders::First {
            return;
        }
        let theta = phase.scattering_angle(&mut draws);
        let next = turned(direction, math::cos(theta), 2.0 * PI * draws.uniform());
        position = field.wrapped(departure(centre, radius_m, struck, next));
        direction = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rings::layer::{PowerLawSizes, SALO_FRENCH_LAW};

    #[test]
    fn the_power_law_phase_is_normalised_over_four_pi() {
        let phase = PowerLawPhase::new(SALO_FRENCH_PHASE_EXPONENT);
        // c_n = 2 ÷ 13.029 for n = 3.09, by the series' sum; and (1/2) ∫ P sin α dα = 1 by a
        // midpoint rule.
        assert!(
            (phase.normalisation() - 0.153_51).abs() < 1e-5,
            "{}",
            phase.normalisation()
        );
        let steps = 200_000;
        let integral: f64 = (0..steps)
            .map(|i| {
                let alpha = (f64::from(i) + 0.5) * PI / f64::from(steps);
                phase.value(alpha) * math::sin(alpha)
            })
            .sum::<f64>()
            * 0.5
            * PI
            / f64::from(steps);
        assert!((integral - 1.0).abs() < 1e-8, "{integral}");
        // The n = 0 law is isotropic, P = 1.
        assert!((PowerLawPhase::new(0.0).value(1.0) - 1.0).abs() < 1e-12);
        assert_eq!(phase.value(PI), 0.0);
    }

    #[test]
    fn the_scattering_angle_follows_the_phase() {
        // The mean of cos θ over the drawn angles against its integral, (1/2) ∫ P cos θ sin θ dθ.
        let phase = PowerLawPhase::new(SALO_FRENCH_PHASE_EXPONENT);
        let mut draws = Draws::new(5, 6, 7);
        let n = 200_000;
        let mean: f64 = (0..n)
            .map(|_| math::cos(phase.scattering_angle(&mut draws)))
            .sum::<f64>()
            / f64::from(n);
        let steps = 100_000;
        let expected: f64 = (0..steps)
            .map(|i| {
                let theta = (f64::from(i) + 0.5) * PI / f64::from(steps);
                // P at phase angle α = π − θ.
                phase.value(PI - theta) * math::cos(theta) * math::sin(theta)
            })
            .sum::<f64>()
            * 0.5
            * PI
            / f64::from(steps);
        assert!(
            expected < -0.5,
            "a backscattering law: ⟨cos θ⟩ = {expected}"
        );
        assert!((mean - expected).abs() < 0.005, "{mean} against {expected}");
    }

    #[test]
    fn the_last_bucket_holds_every_order_from_it_on() {
        let buckets: Vec<usize> = (1..=6).map(|k| Orders::Buckets(4).bucket(k)).collect();
        assert_eq!(buckets, [0, 1, 2, 3, 3, 3]);
        assert_eq!(Orders::First.bucket(1), 0);
        assert_eq!(Orders::Buckets(0).buckets(), 1);
    }

    #[test]
    fn an_observer_sits_at_its_phase_angle_on_its_face() {
        let b = 26_f64.to_radians();
        for (face, alpha) in [(Face::Lit, 6_f64), (Face::Lit, 100.0), (Face::Unlit, 120.0)] {
            let o = Observer::at_phase(face, b, b, alpha.to_radians());
            assert!(
                (o.phase_rad() - alpha.to_radians()).abs() < 1e-9,
                "{face:?} {alpha}"
            );
            assert!((o.mu() - math::sin(b)).abs() < 1e-12);
            assert_eq!(o.direction().z > 0.0, face == Face::Lit);
        }
        // At the zenith, the zenith whatever the phase asked.
        let up = Observer::at_phase(Face::Lit, PI / 2.0, PI / 2.0, 0.3);
        assert!(up.phase_rad().abs() < 1e-6);
    }

    #[test]
    fn the_departure_is_the_point_struck_where_the_direction_sees_it() {
        let c = Vec3::new(0.0, 0.0, 0.0);
        let struck = Vec3::new(0.0, 0.0, 1.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        assert!(
            (departure(c, 1.0, struck, up) - Vec3::new(0.0, 0.0, 1.0 + LIFT_M)).length() < 1e-15
        );
        let side = Vec3::new(1.0, 0.0, 0.0);
        let d = departure(c, 1.0, Vec3::new(-0.6, 0.0, 0.8), side);
        assert!(
            (d - Vec3::new(0.6, 0.0, 0.8) * (1.0 + LIFT_M)).length() < 1e-12,
            "{d:?}"
        );
    }

    #[test]
    fn a_trace_is_the_same_on_any_number_of_threads() {
        let layer = SALO_FRENCH_LAW
            .layer(PowerLawSizes::new(0.5, 2.0).expect("valid radii"), 1.0)
            .expect("valid layer");
        let field = Field::build(&layer, 40.0, FieldKey { seed: 3, key: 4 }).expect("room");
        let b = 20_f64.to_radians();
        let observers = [
            Observer::at_phase(Face::Lit, b, b, 6_f64.to_radians()),
            Observer::at_phase(Face::Unlit, b, b, 140_f64.to_radians()),
        ];
        let key = FieldKey { seed: 3, key: 9 };
        let one = trace(
            &field,
            b,
            &observers,
            600,
            Orders::Buckets(4),
            key,
            NonZeroUsize::MIN,
        )
        .expect("a pool");
        let four = NonZeroUsize::new(4).expect("not zero");
        let many =
            trace(&field, b, &observers, 600, Orders::Buckets(4), key, four).expect("a pool");
        assert_eq!(one, many);
        assert!(one.struck() > 0 && one.scatterings() > one.struck());
        assert!(one.reflectance(math::sin(b), 0, &observers[0], 0) > 0.0);
    }
}
