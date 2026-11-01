//! What an observer in the plane sees of a stream's tube (P15.T11.b): the detectability cut.
//!
//! A tube is seen stretch by stretch, a stretch being one of its knot segments (plan 10's
//! [`TubeTable`], 255 to an arm). A stretch counts towards what an observer sees if its middle lies
//! within the cut's distance of the observer, beyond the cut's latitude as the observer measures
//! it, and if the tube's density on its axis there is at least the cut's contrast times the smooth
//! stellar halo's. The stream is seen if the mass of the stretches that count passes the cut's
//! mass. So "mass above 10³ M☉ within 30 kpc" is the mass of the stream inside the cut, not its
//! whole mass.
//!
//! - **The axis density** of a stretch is its mass, the step in the cumulative line density Λ
//!   between its knots, over its length and the cross-section of a Gaussian tube with the knots'
//!   mean widths, `ΔΛ ÷ (ℓ 2π w_n w_b)`: the peak of a two-dimensional Gaussian of those rms
//!   widths, M☉ per cubic light-year. A stretch whose length or widths are not positive and
//!   finite counts for nothing.
//! - **The smooth halo** is the sum of the galaxy's halo components ([`HaloProfile`]) by their law
//!   without the 65,000 ly cut ([`HaloProfile::density_without_cut`]), times the halo's mean system
//!   mass: the cut keeps the placed halo inside the root cube, while a stream 20–38 kpc from the
//!   centre, within 30 kpc of an observer at 8 kpc, is seen against the halo that goes on beyond it.

use hyperion_sim::galaxy::fields::Shape;
use hyperion_sim::galaxy::fields::halo::HaloProfile;
use hyperion_sim::galaxy::global_list::TubeTable;
use hyperion_sim::galaxy::global_list::spray::Arm;
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::math;

/// The detectability cut (plan 15, P15.T11.b), in the generator's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cut {
    /// The mass a stream must show inside the cut, M☉: it is seen above it.
    pub min_mass_solar_masses: f64,
    /// The farthest a counted stretch lies from the observer, ly.
    pub max_distance_ly: f64,
    /// The least ratio of the tube's density on its axis to the smooth halo's there.
    pub min_contrast: f64,
    /// The least galactic latitude of a counted stretch as the observer measures it, radians:
    /// a stretch counts above it.
    pub min_latitude: f64,
}

/// The smooth stellar halo's mass density by its law without the cut (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct SmoothHalo {
    profiles: Vec<HaloProfile>,
    /// The halo's mean system mass, M☉.
    mean_system_mass: f64,
}

impl SmoothHalo {
    /// The smooth halo of `galaxy`: its halo population's components.
    #[must_use]
    pub fn of(galaxy: &Galaxy) -> Self {
        let profiles = galaxy
            .fields()
            .components()
            .iter()
            .filter(|c| c.population() == Population::Halo)
            .filter_map(|c| match c.shape() {
                Shape::Halo(halo) => Some(*halo),
                Shape::Disc(_) | Shape::Bulge(_) | Shape::Bar(_) => None,
            })
            .collect();
        Self {
            profiles,
            mean_system_mass: galaxy.mean_system_mass(Population::Halo).value(),
        }
    }

    /// The density at `p` (ly, about the galactic centre), M☉ per cubic light-year, summed in
    /// component order.
    #[must_use]
    pub fn density(&self, p: [f64; 3]) -> f64 {
        let at = PointLy::new(p[0], p[1], p[2]);
        let systems = self
            .profiles
            .iter()
            .fold(0.0, |sum, h| sum + h.density_without_cut(&at));
        systems * self.mean_system_mass
    }
}

/// `count` observers in the plane `radius_ly` from the centre, at azimuths `2π i ÷ count` from the
/// x axis.
#[must_use]
pub fn observers(radius_ly: f64, count: u32) -> Vec<[f64; 3]> {
    (0..count)
        .map(|i| {
            let azimuth = 2.0 * core::f64::consts::PI * f64::from(i) / f64::from(count);
            let (sin, cos) = math::sin_cos(azimuth);
            [radius_ly * cos, radius_ly * sin, 0.0]
        })
        .collect()
}

/// The mass of `tube` each observer of `observers` sees under `cut` with each contrast of
/// `contrasts` in place of the cut's, M☉: entry `o × contrasts.len() + c` for observer `o` and
/// contrast `c` (module documentation). Summed stretch by stretch, the trailing arm first.
#[must_use]
pub fn seen_masses(
    tube: &TubeTable,
    halo: &SmoothHalo,
    observers: &[[f64; 3]],
    cut: &Cut,
    contrasts: &[f64],
) -> Vec<f64> {
    let mut seen = vec![0.0; observers.len() * contrasts.len()];
    let sin_latitude = math::sin(cut.min_latitude);
    for arm in [Arm::Trailing, Arm::Leading] {
        for pair in tube.arm(arm).knots().windows(2) {
            let (start, end) = (&pair[0], &pair[1]);
            let (from, to) = (
                start.centre().to_light_years_f64(),
                end.centre().to_light_years_f64(),
            );
            let middle: [f64; 3] = core::array::from_fn(|i| f64::midpoint(from[i], to[i]));
            let length = distance(from, to);
            let [normal_start, binormal_start] = start.widths();
            let [normal_end, binormal_end] = end.widths();
            let normal = f64::midpoint(normal_start.value(), normal_end.value());
            let binormal = f64::midpoint(binormal_start.value(), binormal_end.value());
            let mass = end.cumulative_mass().value() - start.cumulative_mass().value();
            if ![length, normal, binormal]
                .iter()
                .all(|x| *x > 0.0 && x.is_finite())
            {
                continue;
            }
            let axis = mass / (length * 2.0 * core::f64::consts::PI * normal * binormal);
            let smooth = halo.density(middle);
            for (o, observer) in observers.iter().enumerate() {
                let far = distance(middle, *observer);
                // |sin b| = |Δz| ÷ d, the observer in the plane.
                if far > cut.max_distance_ly
                    || (middle[2] - observer[2]).abs() <= sin_latitude * far
                {
                    continue;
                }
                for (c, &contrast) in contrasts.iter().enumerate() {
                    if axis >= contrast * smooth {
                        seen[o * contrasts.len() + c] += mass;
                    }
                }
            }
        }
    }
    seen
}

/// The distance between `p` and `q`, ly.
#[must_use]
fn distance(p: [f64; 3], q: [f64; 3]) -> f64 {
    let d: [f64; 3] = core::array::from_fn(|i| q[i] - p[i]);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_observers_ring_the_centre_in_the_plane() {
        let ring = observers(26_000.0, 4);
        assert_eq!(ring.len(), 4);
        for (o, want) in ring.iter().zip([
            [26_000.0, 0.0],
            [0.0, 26_000.0],
            [-26_000.0, 0.0],
            [0.0, -26_000.0],
        ]) {
            assert!((o[0] - want[0]).abs() < 1e-9 && (o[1] - want[1]).abs() < 1e-9);
            assert!(o[2].abs() < f64::MIN_POSITIVE);
        }
    }

    #[test]
    fn distance_is_euclidean() {
        assert!((distance([1.0, 2.0, 3.0], [4.0, 6.0, 3.0]) - 5.0).abs() < 1e-15);
    }
}
