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
    pub min_latitude_rad: f64,
}

/// The smooth stellar halo's mass density by its law without the cut (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct SmoothHalo {
    profiles: Vec<HaloProfile>,
    mean_system_mass_solar_masses: f64,
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
            mean_system_mass_solar_masses: galaxy.mean_system_mass(Population::Halo).value(),
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
        systems * self.mean_system_mass_solar_masses
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

/// One knot segment of a tube, as the cut reads it (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stretch {
    /// The middle of its centre line, ly about the galactic centre.
    pub middle_ly: [f64; 3],
    /// The length of its centre line, ly.
    pub length_ly: f64,
    /// Its mean rms widths along the normal and the binormal, ly.
    pub widths_ly: [f64; 2],
    /// Its mass, the step in Λ between its knots, M☉.
    pub mass_solar_masses: f64,
}

impl Stretch {
    /// The stretches of `tube`, the trailing arm's first, each arm's in the order of its knots.
    pub fn of(tube: &TubeTable) -> impl Iterator<Item = Self> + '_ {
        [Arm::Trailing, Arm::Leading]
            .into_iter()
            .flat_map(|arm| tube.arm(arm).knots().windows(2))
            .map(|pair| {
                let (start, end) = (&pair[0], &pair[1]);
                let (from, to) = (
                    start.centre().to_light_years_f64(),
                    end.centre().to_light_years_f64(),
                );
                let [normal_start, binormal_start] = start.widths();
                let [normal_end, binormal_end] = end.widths();
                Self {
                    middle_ly: core::array::from_fn(|i| f64::midpoint(from[i], to[i])),
                    length_ly: distance(from, to),
                    widths_ly: [
                        f64::midpoint(normal_start.value(), normal_end.value()),
                        f64::midpoint(binormal_start.value(), binormal_end.value()),
                    ],
                    mass_solar_masses: end.cumulative_mass().value()
                        - start.cumulative_mass().value(),
                }
            })
    }

    /// Its density on its axis, `ΔΛ ÷ (ℓ 2π w_n w_b)`, M☉ per cubic light-year; `None` if its
    /// length or a width is not positive and finite.
    #[must_use]
    pub fn axis_density(&self) -> Option<f64> {
        let [normal, binormal] = self.widths_ly;
        [self.length_ly, normal, binormal]
            .iter()
            .all(|x| *x > 0.0 && x.is_finite())
            .then(|| {
                self.mass_solar_masses
                    / (self.length_ly * 2.0 * core::f64::consts::PI * normal * binormal)
            })
    }
}

/// The mass of `tube` each observer of `observers` sees under `cut` with each contrast of
/// `contrasts` in place of the cut's, M☉: entry `o × contrasts.len() + c` for observer `o` and
/// contrast `c` (module documentation), summed stretch by stretch ([`Stretch::of`]).
#[must_use]
pub fn seen_masses(
    tube: &TubeTable,
    halo: &SmoothHalo,
    observers: &[[f64; 3]],
    cut: &Cut,
    contrasts: &[f64],
) -> Vec<f64> {
    let mut seen = vec![0.0; observers.len() * contrasts.len()];
    for stretch in Stretch::of(tube) {
        add_seen(
            &mut seen,
            &stretch,
            halo.density(stretch.middle_ly),
            observers,
            cut,
            contrasts,
        );
    }
    seen
}

/// Adds `stretch`'s mass to `seen`, laid out as [`seen_masses`]' entries, for each observer it
/// lies within the cut's distance and beyond its latitude of, and each contrast its axis density
/// reaches over the smooth halo's `smooth` (M☉ per cubic light-year).
pub fn add_seen(
    seen: &mut [f64],
    stretch: &Stretch,
    smooth: f64,
    observers: &[[f64; 3]],
    cut: &Cut,
    contrasts: &[f64],
) {
    let Some(axis) = stretch.axis_density() else {
        return;
    };
    let sin_latitude = math::sin(cut.min_latitude_rad);
    let middle = stretch.middle_ly;
    for (o, observer) in observers.iter().enumerate() {
        let far = distance(middle, *observer);
        // |sin b| = |Δz| ÷ d, the observer in the plane.
        if far > cut.max_distance_ly || (middle[2] - observer[2]).abs() <= sin_latitude * far {
            continue;
        }
        for (c, &contrast) in contrasts.iter().enumerate() {
            if axis >= contrast * smooth {
                seen[o * contrasts.len() + c] += stretch.mass_solar_masses;
            }
        }
    }
}

/// The distance between `p` and `q`, ly.
#[must_use]
fn distance(p: [f64; 3], q: [f64; 3]) -> f64 {
    let d: [f64; 3] = core::array::from_fn(|i| q[i] - p[i]);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::params::GalaxyParams;

    use super::*;

    /// The cut of P15.T11.b's manifest: 10³ M☉, 30 kpc as 97,846.9 ly, tenfold, 15°.
    const CUT: Cut = Cut {
        min_mass_solar_masses: 1e3,
        max_distance_ly: 97_846.9,
        min_contrast: 10.0,
        min_latitude_rad: 0.261_799_387_799_149_4,
    };

    /// A stretch 30,000 ly above the plane, 100 ly long and 50 by 40 ly wide, of 20 M☉.
    fn stretch(middle_ly: [f64; 3]) -> Stretch {
        Stretch {
            middle_ly,
            length_ly: 100.0,
            widths_ly: [50.0, 40.0],
            mass_solar_masses: 20.0,
        }
    }

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

    /// A Gaussian tube's peak density is its line density over `2π w_n w_b`, and a stretch with
    /// no length or width has none.
    #[test]
    fn a_stretch_s_axis_density_is_a_gaussian_tube_s_peak() {
        let s = stretch([0.0, 0.0, 30_000.0]);
        let want = 20.0 / 100.0 / (2.0 * core::f64::consts::PI * 50.0 * 40.0);
        assert!((s.axis_density().unwrap() / want - 1.0).abs() < 1e-15);
        for bad in [
            Stretch {
                length_ly: 0.0,
                ..s
            },
            Stretch {
                widths_ly: [0.0, 40.0],
                ..s
            },
            Stretch {
                widths_ly: [50.0, f64::INFINITY],
                ..s
            },
            Stretch {
                length_ly: f64::NAN,
                ..s
            },
        ] {
            assert_eq!(bad.axis_density(), None, "{bad:?}");
        }
    }

    /// A stretch counts for an observer only within the cut's distance and beyond its latitude,
    /// and for a contrast only where its axis is that many times as dense as the smooth halo.
    #[test]
    fn a_stretch_counts_within_the_distance_beyond_the_latitude_above_the_contrast() {
        let eyes = [[26_000.0, 0.0, 0.0], [-26_000.0, 0.0, 0.0]];
        let contrasts = [10.0, 0.0, 5.0, 20.0];
        let axis = stretch([0.0, 0.0, 0.0]).axis_density().unwrap();
        // Over the centre at 30,000 ly: 39,700 ly from either observer, at 49°.
        let high = stretch([0.0, 0.0, 30_000.0]);
        let mut seen = vec![0.0; 8];
        // Its axis eight times the halo's: seen with no contrast and at five times, not at ten or
        // twenty.
        add_seen(&mut seen, &high, axis / 8.0, &eyes, &CUT, &contrasts);
        assert_eq!(seen, [0.0, 20.0, 20.0, 0.0, 0.0, 20.0, 20.0, 0.0]);
        let mut dense = vec![0.0; 8];
        add_seen(&mut dense, &high, axis / 25.0, &eyes, &CUT, &contrasts);
        assert_eq!(dense, [20.0; 8]);
        // In the plane: below the latitude for both.
        let mut flat = vec![0.0; 8];
        let low = stretch([0.0, 30_000.0, 5_000.0]);
        add_seen(&mut flat, &low, 0.0, &eyes, &CUT, &contrasts);
        assert!(flat.iter().all(|m| *m == 0.0), "{flat:?}");
        // Beyond the near observer's distance but within the far one's, and above the plane.
        let far = stretch([110_000.0, 0.0, 40_000.0]);
        let mut one = vec![0.0; 8];
        add_seen(&mut one, &far, 0.0, &eyes, &CUT, &contrasts);
        assert_eq!(one, [20.0, 20.0, 20.0, 20.0, 0.0, 0.0, 0.0, 0.0]);
    }

    /// Inside the halo's cut the smooth halo is the galaxy's own halo density, systems times their
    /// mean mass; beyond it, where the placed halo has none, it goes on.
    #[test]
    fn the_smooth_halo_is_the_galaxy_s_inside_the_cut_and_goes_on_beyond() {
        let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like()).unwrap();
        let halo = SmoothHalo::of(&galaxy);
        let mean = galaxy.mean_system_mass(Population::Halo).value();
        let inside = [26_000.0, 0.0, 20_000.0];
        let placed = galaxy.fields().population_density(
            Population::Halo,
            &PointLy::new(inside[0], inside[1], inside[2]),
        );
        assert!((halo.density(inside) / (placed * mean) - 1.0).abs() < 1e-12);
        let beyond = [0.0, 0.0, 90_000.0];
        let outside = galaxy
            .fields()
            .population_density(Population::Halo, &PointLy::new(0.0, 0.0, 90_000.0));
        assert!(outside.abs() < f64::MIN_POSITIVE);
        assert!(halo.density(beyond) > 0.0 && halo.density(beyond) < halo.density(inside));
    }
}
