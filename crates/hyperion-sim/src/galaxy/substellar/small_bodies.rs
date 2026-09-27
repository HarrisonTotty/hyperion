//! Interstellar comets and asteroids as a mean number density (plan 13, P13.T6).
//!
//! Only the density exists: nothing places these bodies, and the tag
//! [`INTERSTELLAR_SMALL_BODIES`](crate::rng::tags::INTERSTELLAR_SMALL_BODIES) is reserved for
//! making them real around a ship, keyed by a cell.

use crate::coords::GalacticPosition;
use crate::galaxy::fields::MAX_COMPONENTS;
use crate::galaxy::{Galaxy, PointLy};
use crate::units::PerCubicAu;

/// Small bodies per cubic astronomical unit at the reference density: 0.1, the brainstorm's
/// "perhaps one for every ten cubic astronomical units" ("Between the stars"). The brainstorm cites
/// no source for the figure; it is the brainstorm's own (plan 13, Design note 13).
pub const SMALL_BODIES_PER_AU3_AT_REFERENCE: f64 = 0.1;

/// The system density the figure above is quoted at, 0.003 per cubic light-year: the brainstorm's
/// reference density of the solar neighbourhood.
pub const REFERENCE_SYSTEMS_PER_LY3: f64 = 0.003;

/// The mean number density of interstellar comets and asteroids at `p`, per cubic astronomical
/// unit.
///
/// Such bodies are ejected from planetary systems, so the density scales with the local total
/// system density, every population's components summed: 0.1 per au³ where the systems number
/// 0.003 per ly³ (plan 13, Design note 13).
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::substellar::interstellar_small_body_density;
///
/// let galaxy = Galaxy::new(Seed::new(2));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
/// let high = GalacticPosition::from_light_years([0.0, 26_000.0, 5_000.0]).expect("in the cube");
/// // Of the order of one for every ten cubic au near the Sun, and far fewer out of the plane.
/// let here = interstellar_small_body_density(&galaxy, &sun).value();
/// assert!((0.01..1.0).contains(&here));
/// assert!(interstellar_small_body_density(&galaxy, &high).value() < 0.1 * here);
/// ```
#[must_use]
pub fn interstellar_small_body_density(galaxy: &Galaxy, p: &GalacticPosition) -> PerCubicAu {
    let mut densities = [0.0; MAX_COMPONENTS];
    let systems = galaxy.fields().densities(&PointLy::from(p), &mut densities);
    PerCubicAu::new(SMALL_BODIES_PER_AU3_AT_REFERENCE * systems / REFERENCE_SYSTEMS_PER_LY3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::params::GalaxyParams;

    fn total_density(galaxy: &Galaxy, p: &GalacticPosition) -> f64 {
        let mut densities = [0.0; MAX_COMPONENTS];
        galaxy.fields().densities(&PointLy::from(p), &mut densities)
    }

    /// 0.1 per au³ where the systems number 0.003 per ly³, found along the plane at the solar
    /// circle's height, and in proportion to the system density at three other places.
    #[test]
    fn the_density_scales_with_the_systems() {
        let galaxy = Galaxy::from_params(Seed::new(0x1306), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral");
        // Bisect outward along +y for the radius where the system density is 0.003 per ly³.
        let density_at = |y: f64| {
            total_density(
                &galaxy,
                &GalacticPosition::from_light_years([0.0, y, 0.0]).unwrap(),
            )
        };
        let (mut inner, mut outer) = (15_000.0, 45_000.0);
        assert!(density_at(inner) > REFERENCE_SYSTEMS_PER_LY3);
        assert!(density_at(outer) < REFERENCE_SYSTEMS_PER_LY3);
        for _ in 0..60 {
            let mid = f64::midpoint(inner, outer);
            if density_at(mid) > REFERENCE_SYSTEMS_PER_LY3 {
                inner = mid;
            } else {
                outer = mid;
            }
        }
        let reference = GalacticPosition::from_light_years([0.0, inner, 0.0]).unwrap();
        let at_reference = interstellar_small_body_density(&galaxy, &reference).value();
        assert!((at_reference - 0.1).abs() < 1e-6, "{at_reference}");
        for at in [
            [0.0, 26_000.0, 3_000.0],
            [700.0, 700.0, 0.0],
            [0.0, -40_000.0, 0.0],
        ] {
            let p = GalacticPosition::from_light_years(at).unwrap();
            let expected = 0.1 * total_density(&galaxy, &p) / 0.003;
            let got = interstellar_small_body_density(&galaxy, &p).value();
            assert!(expected > 0.0);
            assert!(
                (got / expected - 1.0).abs() < 1e-12,
                "{at:?}: {got} vs {expected}"
            );
        }
    }
}
